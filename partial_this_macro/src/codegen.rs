use proc_macro2::{Literal, Span, TokenStream};
use quote::{format_ident, quote};
use std::collections::HashSet;
use syn::{Error, Fields, GenericParam, Ident, ItemStruct, Type, parse_quote};

use crate::config::PartialConfig;

/// Generates the full `partial` macro output for a struct.
///
/// The output consists of:
/// - the original struct,
/// - a private module containing one marker struct per field, plus the
///   `ThisPtr`/`Drop` impls for those markers,
/// - a nested `private` module holding the `Partial<N>` builder, the
///   `PartialThis`/`State` impls, and the per-field builder/accessor methods,
/// - a re-export of the builder type `PartialFoo` into the current module.
pub(crate) fn generate(item: &ItemStruct, cfg: &PartialConfig) -> syn::Result<TokenStream> {
    let struct_ident = &item.ident;
    let krate = cfg.crate_name();
    let fields = collect_fields(item)?;
    let module_ident = cfg.module_name(item);
    let names = allocate_names(&item.generics);
    let n = &names.n;
    let u = &names.u;

    let struct_generics = &item.generics;
    let (struct_impl_gen, struct_ty_gen, struct_wc) = struct_generics.split_for_impl();
    let struct_ty_generics = quote!(#struct_ty_gen);
    let struct_where = match struct_wc {
        Some(wc) => quote!(#wc),
        None => quote!(),
    };
    let struct_type = quote!(#struct_ident #struct_ty_generics);

    let n_fields = fields.len();
    let all_mask = (1usize << n_fields) - 1;
    let partial_alias = format_ident!("Partial{}", struct_ident);
    let typenum_use = gen_typenum_use(&krate, n_fields);
    let ty_args = ty_args(&item.generics);

    let markers = gen_markers(&krate, &fields, struct_generics, &struct_type, n);
    let thisptr_impls =
        gen_marker_thisptr(&krate, &fields, struct_generics, &struct_type, &ty_args, n);
    let drop_impls = gen_marker_drop(&krate, &fields, struct_generics, &struct_type, &ty_args, n);
    let state_blanket = gen_state_blanket(&krate, struct_generics, &struct_type, u);
    let state_impls = gen_state_impls(&krate, &fields, struct_generics, &struct_type, &ty_args, n);
    let builder_impls =
        gen_builder_impls(&krate, &fields, struct_generics, &struct_type, &ty_args, n);
    let accessor_impls = gen_accessor_impls(&krate, &fields, struct_generics, &struct_type, n);
    let done_impl = gen_done_impl(&krate, struct_generics, &struct_type, n, all_mask);

    // A fully `pub` struct whose fields are all `pub` can safely re-export the
    // generated builder type. If any private type would leak into a public
    // interface (a private struct, or a non-`pub` field exposing a private
    // type), fall back to a module-local `use` instead of `pub use`.
    let struct_pub = matches!(item.vis, syn::Visibility::Public(_));
    let all_fields_pub = fields.iter().all(|f| f.is_pub);
    let safe_to_pub_use = cfg.pub_use() && struct_pub && all_fields_pub;

    let partial_reexport = quote! { pub use private::Partial as #partial_alias; };
    let top_reexport = if safe_to_pub_use {
        quote! { pub use #module_ident::#partial_alias; }
    } else {
        quote! { use #module_ident::#partial_alias; }
    };

    Ok(quote! {
        #item
        #[allow(nonstandard_style)]
        mod #module_ident {
            // Bring the struct and any custom field types from the parent
            // module into scope.
            use super::*;
            use ::#krate::ThisPtr;

            #markers

            #thisptr_impls

            #drop_impls

            #partial_reexport

            mod private {
                use super::*;
                // Field types live in the parent of the generated module; glob
                // importing them here makes the builder/accessor signatures work.
                use super::super::*;
                #typenum_use
                use ::#krate::{PartialThis, ThisPtr, UninitThis};
                use ::core::mem::ManuallyDrop;
                use ::core::ops::{BitAnd, BitOr};

                #[derive(Debug)]
                pub struct Partial<#n>(#n);

                impl #struct_impl_gen PartialThis for #struct_type #struct_where {
                    type Partial<#n> = Partial<#n>;

                    fn partial<#u>(this: #u) -> Self::Partial<#u>
                    where
                        #u: ::#krate::UninitThis<Target = Self>,
                    {
                        Partial(this)
                    }
                }

                pub trait State: ThisPtr {
                    type Flags;
                    type Inited;

                    unsafe fn assume_init(self) -> Self::Inited;
                }

                #state_blanket

                #state_impls

                #builder_impls

                #accessor_impls

                #done_impl
            }
        }
        #[allow(unused_imports)]
        #top_reexport
    })
}

/// A field of the struct, with the data needed to generate its impls.
struct FieldInfo {
    /// Identifier used for generated method and marker names (`foo` for named
    /// fields, `_0`/`_1` for tuple fields).
    name: Ident,
    /// Token used to access the field on the struct (`foo` for named fields,
    /// `0`/`1` for tuple fields).
    access: TokenStream,
    ty: Type,
    /// Field index, starting at `1`.
    index: usize,
    /// Whether the field is declared `pub`.
    is_pub: bool,
}

fn collect_fields(item: &ItemStruct) -> syn::Result<Vec<FieldInfo>> {
    let mut fields = Vec::new();

    match &item.fields {
        Fields::Named(named) => {
            for (i, field) in named.named.iter().enumerate() {
                let name = field
                    .ident
                    .clone()
                    .ok_or_else(|| Error::new_spanned(field, "field is missing an identifier"))?;
                let access = quote!(#name);
                // Field indices start at `U1`.
                fields.push(FieldInfo {
                    name,
                    access,
                    ty: field.ty.clone(),
                    index: i + 1,
                    is_pub: matches!(field.vis, syn::Visibility::Public(_)),
                });
            }
        }
        Fields::Unnamed(unnamed) => {
            for (i, field) in unnamed.unnamed.iter().enumerate() {
                let name = format_ident!("_{}", i);
                // Tuple fields are accessed by an unsuffixed numeric index.
                let access = {
                    let lit = Literal::usize_unsuffixed(i);
                    quote!(#lit)
                };
                // Field indices start at `U1`.
                fields.push(FieldInfo {
                    name,
                    access,
                    ty: field.ty.clone(),
                    index: i + 1,
                    is_pub: matches!(field.vis, syn::Visibility::Public(_)),
                });
            }
        }
        Fields::Unit => {}
    }

    Ok(fields)
}

/// Builds the comma-separated list of a struct's generic type arguments.
fn ty_args(generics: &syn::Generics) -> Vec<TokenStream> {
    let mut args = Vec::new();
    for param in &generics.params {
        match param {
            GenericParam::Lifetime(lp) => {
                let lt = &lp.lifetime;
                args.push(quote!(#lt));
            }
            GenericParam::Type(tp) => {
                let id = &tp.ident;
                args.push(quote!(#id));
            }
            GenericParam::Const(cp) => {
                let id = &cp.ident;
                args.push(quote!(#id));
            }
        }
    }
    args
}

/// Builds the type of a field marker: `name<'a, T, N>` or `name<N>`.
fn marker_ty(fname: &Ident, ty_args: &[TokenStream], n: &Ident) -> TokenStream {
    if ty_args.is_empty() {
        quote!(#fname<#n>)
    } else {
        quote!(#fname<#(#ty_args),*, #n>)
    }
}

/// Generates the marker structs, one per field.
///
/// Each marker carries the struct's generic parameters and the
/// `N: ThisPtr<Target = Struct>` bound so its `Drop` impl can be written
/// without adding new requirements (see E0367).
fn gen_markers(
    krate: &Ident,
    fields: &[FieldInfo],
    struct_generics: &syn::Generics,
    struct_type: &TokenStream,
    n: &Ident,
) -> TokenStream {
    let mut defs = TokenStream::new();
    for f in fields {
        let fname = &f.name;
        let (marker_gen, marker_where) = impl_parts(
            struct_generics,
            &[quote!(#n: ::#krate::ThisPtr<Target = #struct_type>)],
            &[],
        );
        defs.extend(quote! {
            #[derive(Debug)]
            pub struct #fname #marker_gen(
                #n,
                ::core::marker::PhantomData<#struct_type>,
            ) #marker_where;
        });
    }
    defs
}

/// Computes the impl generic parameters and where clause for a block that adds
/// the given extra params and predicates on top of the struct's generics.
fn impl_parts(
    struct_generics: &syn::Generics,
    extra_params: &[TokenStream],
    extra_preds: &[TokenStream],
) -> (TokenStream, TokenStream) {
    let mut gt = struct_generics.clone();
    for p in extra_params {
        gt.params.push(parse_quote!(#p));
    }
    let wc = gt.make_where_clause();
    for p in extra_preds {
        wc.predicates.push(parse_quote!(#p));
    }
    let (impl_gen, _, impl_where) = gt.split_for_impl();
    (
        quote!(#impl_gen),
        match impl_where {
            Some(wc) => quote!(#wc),
            None => quote!(),
        },
    )
}

/// Generates the `ThisPtr` impls delegating to the wrapped storage.
fn gen_marker_thisptr(
    krate: &Ident,
    fields: &[FieldInfo],
    struct_generics: &syn::Generics,
    struct_type: &TokenStream,
    ty_args: &[TokenStream],
    n: &Ident,
) -> TokenStream {
    let mut defs = TokenStream::new();
    for f in fields {
        let fname = &f.name;
        let mty = marker_ty(fname, ty_args, n);
        let (impl_gen, impl_where) = impl_parts(
            struct_generics,
            &[quote!(#n: ::#krate::ThisPtr<Target = #struct_type>)],
            &[],
        );
        defs.extend(quote! {
            impl #impl_gen ThisPtr for #mty #impl_where {
                type Target = #struct_type;

                #[cfg_attr(not(debug_assertions), inline(always))]
                fn this(&self) -> &::core::mem::MaybeUninit<Self::Target> {
                    self.0.this()
                }

                #[cfg_attr(not(debug_assertions), inline(always))]
                fn this_mut(&mut self) -> &mut ::core::mem::MaybeUninit<Self::Target> {
                    self.0.this_mut()
                }
            }
        });
    }
    defs
}

/// Generates the `Drop` impls that drop the already-initialized field value.
fn gen_marker_drop(
    krate: &Ident,
    fields: &[FieldInfo],
    struct_generics: &syn::Generics,
    struct_type: &TokenStream,
    ty_args: &[TokenStream],
    n: &Ident,
) -> TokenStream {
    let mut defs = TokenStream::new();
    for f in fields {
        let fname = &f.name;
        let access = &f.access;
        let mty = marker_ty(fname, ty_args, n);
        let (impl_gen, impl_where) = impl_parts(
            struct_generics,
            &[quote!(#n: ::#krate::ThisPtr<Target = #struct_type>)],
            &[],
        );
        defs.extend(quote! {
            impl #impl_gen Drop for #mty #impl_where {
                #[cfg_attr(not(debug_assertions), inline(always))]
                fn drop(&mut self) {
                    unsafe {
                        ::core::ptr::drop_in_place(&mut (*self.0.this_mut().as_mut_ptr()).#access)
                    }
                }
            }
        });
    }
    defs
}

/// Generates the blanket `State` impl for the uninitialized storage.
fn gen_state_blanket(
    krate: &Ident,
    struct_generics: &syn::Generics,
    struct_type: &TokenStream,
    u: &Ident,
) -> TokenStream {
    let (impl_gen, impl_where) = impl_parts(
        struct_generics,
        &[quote!(#u: ::#krate::UninitThis<Target = #struct_type>)],
        &[],
    );
    quote! {
        impl #impl_gen State for #u #impl_where {
            type Flags = U0;
            type Inited = <#u as ::#krate::UninitThis>::Inited;

            #[cfg_attr(not(debug_assertions), inline(always))]
            unsafe fn assume_init(self) -> Self::Inited {
                unsafe { <#u as ::#krate::UninitThis>::assume_init(self) }
            }
        }
    }
}

/// The typenum bit-mask type for a field at 1-based `index`.
fn mask_ty(index: usize) -> Ident {
    let bit = 1usize << (index - 1);
    format_ident!("U{}", bit)
}

/// Generates the per-marker `State` impls that accumulate the flags.
fn gen_state_impls(
    krate: &Ident,
    fields: &[FieldInfo],
    struct_generics: &syn::Generics,
    struct_type: &TokenStream,
    ty_args: &[TokenStream],
    n: &Ident,
) -> TokenStream {
    let mut defs = TokenStream::new();
    for f in fields {
        let fname = &f.name;
        let mask = mask_ty(f.index);
        let mty = marker_ty(fname, ty_args, n);
        let (impl_gen, impl_where) = impl_parts(
            struct_generics,
            &[quote!(#n)],
            &[
                quote!(#n: ::#krate::ThisPtr<Target = #struct_type>),
                quote!(#n: State),
                quote!(<#n as State>::Flags: BitOr<#mask>),
            ],
        );
        defs.extend(quote! {
            impl #impl_gen State for #mty #impl_where {
                type Flags = Or<<#n as State>::Flags, #mask>;
                type Inited = <#n as State>::Inited;

                #[cfg_attr(not(debug_assertions), inline(always))]
                unsafe fn assume_init(self) -> Self::Inited {
                    unsafe {
                        let this = ManuallyDrop::new(self);
                        ::core::ptr::read(&this.0).assume_init()
                    }
                }
            }
        });
    }
    defs
}

/// Generates the builder impls: `field(value)` and `assume_init_field`.
fn gen_builder_impls(
    krate: &Ident,
    fields: &[FieldInfo],
    struct_generics: &syn::Generics,
    struct_type: &TokenStream,
    ty_args: &[TokenStream],
    n: &Ident,
) -> TokenStream {
    let mut defs = TokenStream::new();
    for f in fields {
        let fname = &f.name;
        let fty = &f.ty;
        let access = &f.access;
        let mask = mask_ty(f.index);
        let mty = marker_ty(fname, ty_args, n);
        let assume_name = format_ident!("assume_init_{}", fname);
        let (impl_gen, impl_where) = impl_parts(
            struct_generics,
            &[quote!(#n)],
            &[
                quote!(#n: ::#krate::ThisPtr<Target = #struct_type>),
                quote!(#n: State),
                quote!(<#n as State>::Flags: BitAnd<#mask, Output = U0>),
            ],
        );
        defs.extend(quote! {
            impl #impl_gen Partial<#n> #impl_where {
                #[cfg_attr(not(debug_assertions), inline(always))]
                pub unsafe fn #assume_name(self) -> Partial<#mty> {
                    Partial(#fname(self.0, ::core::marker::PhantomData))
                }

                #[cfg_attr(not(debug_assertions), inline(always))]
                pub fn #fname(mut self, v: #fty) -> Partial<#mty> {
                    unsafe {
                        ::core::ptr::write(
                            &mut (*self.0.this_mut().as_mut_ptr()).#access,
                            v,
                        )
                    };
                    Partial(#fname(self.0, ::core::marker::PhantomData))
                }
            }
        });
    }
    defs
}

/// Generates the accessor impls: `get_field`, `get_field_mut`, `set_field`.
fn gen_accessor_impls(
    krate: &Ident,
    fields: &[FieldInfo],
    struct_generics: &syn::Generics,
    struct_type: &TokenStream,
    n: &Ident,
) -> TokenStream {
    let mut defs = TokenStream::new();
    for f in fields {
        let fname = &f.name;
        let fty = &f.ty;
        let access = &f.access;
        let mask = mask_ty(f.index);
        let set_name = format_ident!("set_{}", fname);
        let get_name = format_ident!("get_{}", fname);
        let get_mut_name = format_ident!("get_{}_mut", fname);
        let (impl_gen, impl_where) = impl_parts(
            struct_generics,
            &[quote!(#n)],
            &[
                quote!(#n: ::#krate::ThisPtr<Target = #struct_type>),
                quote!(#n: State),
                quote!(<#n as State>::Flags: BitAnd<#mask, Output = #mask>),
            ],
        );
        defs.extend(quote! {
            impl #impl_gen Partial<#n> #impl_where {
                #[cfg_attr(not(debug_assertions), inline(always))]
                pub fn #set_name(&mut self, v: #fty) {
                    *self.#get_mut_name() = v;
                }

                #[cfg_attr(not(debug_assertions), inline(always))]
                pub fn #get_name(&self) -> &#fty {
                    unsafe { &(*self.0.this().as_ptr()).#access }
                }

                #[cfg_attr(not(debug_assertions), inline(always))]
                pub fn #get_mut_name(&mut self) -> &mut #fty {
                    unsafe { &mut (*self.0.this_mut().as_mut_ptr()).#access }
                }
            }
        });
    }
    defs
}

/// Generates the `done` impl that finalizes the builder once all fields are set.
fn gen_done_impl(
    krate: &Ident,
    struct_generics: &syn::Generics,
    struct_type: &TokenStream,
    n: &Ident,
    all_mask: usize,
) -> TokenStream {
    let all_type = format_ident!("U{}", all_mask);
    let (impl_gen, impl_where) = impl_parts(
        struct_generics,
        &[quote!(#n)],
        &[
            quote!(#n: ::#krate::ThisPtr<Target = #struct_type>),
            quote!(#n: State<Flags = #all_type>),
        ],
    );
    quote! {
        impl #impl_gen Partial<#n> #impl_where {
            #[cfg_attr(not(debug_assertions), inline(always))]
            pub fn done(self) -> <#n as State>::Inited {
                unsafe { self.0.assume_init() }
            }
        }
    }
}

/// Builds the `use` line importing the typenum literals used in the generated
/// `private` module.
fn gen_typenum_use(krate: &Ident, n_fields: usize) -> TokenStream {
    let mut vals: Vec<usize> = vec![0];
    if n_fields > 0 {
        for i in 0..n_fields {
            vals.push(1usize << i);
        }
        vals.push((1usize << n_fields) - 1);
    }
    vals.sort_unstable();
    vals.dedup();
    let lits = vals.into_iter().map(|v| {
        let u = format_ident!("U{}", v);
        quote!(#u)
    });
    quote! {
        use ::#krate::typenum::{Or, #(#lits),*};
    }
}

/// The internal generic parameter names used by the generated code.
///
/// These are allocated so they never collide with the struct's own generic
/// parameter names (e.g. a struct with an `N` or `U` type parameter).
struct Names {
    u: Ident,
    n: Ident,
}

/// Allocates unique internal generic names, avoiding the struct's own params.
fn allocate_names(generics: &syn::Generics) -> Names {
    let mut used: HashSet<String> = HashSet::new();
    for param in &generics.params {
        match param {
            GenericParam::Type(tp) => {
                used.insert(tp.ident.to_string());
            }
            GenericParam::Const(cp) => {
                used.insert(cp.ident.to_string());
            }
            GenericParam::Lifetime(_) => {}
        }
    }

    Names {
        u: unique_ident("U", &mut used),
        n: unique_ident("N", &mut used),
    }
}

/// Returns `base` if unused, otherwise a suffixed variant that is unused.
fn unique_ident(base: &str, used: &mut HashSet<String>) -> Ident {
    if used.insert(base.to_string()) {
        return Ident::new(base, Span::call_site());
    }
    let mut i = 0;
    loop {
        let candidate = format!("{base}_{i}");
        if !used.contains(&candidate) {
            used.insert(candidate.clone());
            return Ident::new(&candidate, Span::call_site());
        }
        i += 1;
    }
}
