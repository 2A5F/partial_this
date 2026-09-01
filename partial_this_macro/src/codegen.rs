use proc_macro2::{Literal, Span, TokenStream};
use quote::{format_ident, quote};
use std::collections::HashSet;
use syn::{Error, Fields, GenericParam, Ident, ItemStruct, Type, parse_quote};

use crate::config::PartialConfig;

/// Generates the full `partial` macro output for a struct.
///
/// The output consists of:
/// - the original struct,
/// - a module containing the `PartialThis` impl, the per-field `Field`
///   descriptors, and the field builder/accessor traits,
/// - a `pub use` re-export so the generated traits are brought into scope.
pub(crate) fn generate(item: &ItemStruct, cfg: &PartialConfig) -> syn::Result<TokenStream> {
    let struct_ident = &item.ident;
    let krate = cfg.crate_name();
    let fields = collect_fields(item)?;
    let module_ident = cfg.module_name(item);
    let names = allocate_names(&item.generics);

    let struct_generics = &item.generics;
    let (struct_impl_gen, struct_ty_gen, struct_wc) = struct_generics.split_for_impl();
    let struct_impl_generics = quote!(#struct_impl_gen);
    let struct_ty_generics = quote!(#struct_ty_gen);
    let struct_where = match struct_wc {
        Some(wc) => quote!(#wc),
        None => quote!(),
    };
    let struct_type = quote!(#struct_ident #struct_ty_generics);
    let is_generic = !struct_generics.params.is_empty();

    // Impl generics for `PartialThis`: the struct's params plus the `U` bound.
    let u = &names.u;
    let mut partial_impl_gt = struct_generics.clone();
    partial_impl_gt
        .params
        .push(parse_quote!(#u: ::#krate::UninitThis<Target = #struct_type>));
    let (partial_gen, _, partial_wc) = partial_impl_gt.split_for_impl();
    let partial_impl_generics = quote!(#partial_gen);
    let partial_where = match partial_wc {
        Some(wc) => quote!(#wc),
        None => quote!(),
    };

    // Build the nested `Field<false, ..., U>` output type and the matching
    // constructor. Fields are folded in declaration order, which makes the
    // last declared field the outermost layer.
    let mut output_ty = quote!(#u);
    let mut ctor = quote!(this);
    for f in &fields {
        let fname = &f.name;
        output_ty = quote!(chain::Field<false, fields::#fname #struct_ty_generics, #output_ty>);
        ctor = quote!(chain::Field::uninit(#ctor));
    }

    let fields_defs = gen_fields_module(
        &krate,
        &fields,
        is_generic,
        &struct_impl_generics,
        &struct_ty_generics,
        &struct_where,
        &struct_type,
        &names.init,
    );
    let uninit_defs = gen_uninit_module(&krate, item, &fields, &struct_type, &names);
    let inited_defs = gen_inited_module(&krate, item, &fields, &struct_type, &names);

    Ok(quote! {
        #item
        #[allow(nonstandard_style)]
        mod #module_ident {
            use ::#krate::{ PartialThis, UninitThis, chain::{self} };
            use super::*;

            impl #partial_impl_generics PartialThis<#u> for #struct_type #partial_where {
                type Output = #output_ty;
                fn partial(this: #u) -> Self::Output {
                    #ctor
                }
            }

            mod fields {
                use ::#krate::chain::traits::Field;
                #fields_defs
            }

            pub use uninit_fields::*;
            mod uninit_fields {
                use super::*;
                use ::#krate::chain::{self, traits::MapInit};
                #uninit_defs
            }

            pub use inited_fields::*;
            mod inited_fields {
                use super::*;
                use ::#krate::chain::{self, traits::GetField};
                #inited_defs
            }
        }
        pub use #module_ident::*;
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
    index: usize,
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
                });
            }
        }
        Fields::Unit => {}
    }

    Ok(fields)
}

/// Generates the `fields` submodule: a marker struct per field plus its `Field` impl.
///
/// For generic structs the marker carries the struct's params so the `Field`
/// impl (and thus `Target`) can name `Foo<'a, T>`.
fn gen_fields_module(
    krate: &Ident,
    fields: &[FieldInfo],
    is_generic: bool,
    struct_impl_generics: &TokenStream,
    struct_ty_generics: &TokenStream,
    struct_where: &TokenStream,
    struct_type: &TokenStream,
    init: &Ident,
) -> TokenStream {
    let mut defs = TokenStream::new();

    for f in fields {
        let fname = &f.name;
        let access = &f.access;
        let fty = &f.ty;
        let uid = format_ident!("U{}", f.index);

        let marker_type = quote!(#fname #struct_ty_generics);
        let marker_def = if is_generic {
            quote!(
                #[derive(Debug)]
                pub struct #fname #struct_impl_generics(
                    ::core::marker::PhantomData<super::#struct_type>,
                ) #struct_where;
            )
        } else {
            quote!(
                #[derive(Debug)]
                pub struct #fname;
            )
        };

        defs.extend(quote! {
            #marker_def

            impl #struct_impl_generics Field for #marker_type #struct_where {
                type Target = super::#struct_type;
                type Type = #fty;
                type Id = ::#krate::typenum::#uid;

                unsafe fn drop<const #init: bool>(
                    this: &mut ::core::mem::MaybeUninit<Self::Target>,
                ) {
                    if #init {
                        unsafe { ::core::ptr::drop_in_place(&mut (*this.as_mut_ptr()).#access) };
                    }
                }

                unsafe fn init(
                    this: &mut ::core::mem::MaybeUninit<Self::Target>,
                    v: Self::Type,
                ) {
                    unsafe { ::core::ptr::write(&mut (*this.as_mut_ptr()).#access, v) }
                }

                unsafe fn get(
                    this: &::core::mem::MaybeUninit<Self::Target>,
                ) -> &Self::Type {
                    unsafe { &(*this.as_ptr()).#access }
                }

                unsafe fn get_mut(
                    this: &mut ::core::mem::MaybeUninit<Self::Target>,
                ) -> &mut Self::Type {
                    unsafe { &mut (*this.as_mut_ptr()).#access }
                }
            }
        });
    }

    defs
}

/// Generates the `uninit_fields` submodule: the field builder traits and impls.
fn gen_uninit_module(
    krate: &Ident,
    item: &ItemStruct,
    fields: &[FieldInfo],
    struct_type: &TokenStream,
    names: &Names,
) -> TokenStream {
    let struct_ident = &item.ident;
    let struct_generics = &item.generics;
    let mut defs = TokenStream::new();

    let init = &names.init;
    let f = &names.f;
    let n = &names.n;
    let c = &names.c;

    for fld in fields {
        let fname = &fld.name;
        let fty = &fld.ty;
        let uid = format_ident!("U{}", fld.index);
        let trait_name = format_ident!("{}_uninit_{}", struct_ident, fname);
        let assume_name = format_ident!("assume_init_{}", fname);

        let TraitParts {
            trait_impl_generics,
            trait_ty_generics,
            trait_where,
        } = trait_generics(struct_generics, names);
        let ImplParts {
            impl_generics,
            impl_where,
        } = field_impl_generics(
            struct_generics,
            struct_type,
            quote!(MapInit<#fty, ::#krate::typenum::#uid, #c>),
            names,
        );

        defs.extend(quote! {
            pub trait #trait_name #trait_impl_generics #trait_where {
                type Output;
                fn #fname(self, value: #fty) -> Self::Output;
                fn #assume_name(self) -> Self::Output;
            }

            impl #impl_generics #trait_name #trait_ty_generics
                for chain::Field<#init, #f, #n>
                #impl_where
            {
                type Output =
                    <Self as MapInit<#fty, ::#krate::typenum::#uid, #c>>::Result;

                fn #fname(self, value: #fty) -> Self::Output {
                    unsafe { Self::map_init(self, value) }
                }

                fn #assume_name(self) -> Self::Output {
                    unsafe { Self::assume_init(self) }
                }
            }
        });
    }

    defs
}

/// Generates the `inited_fields` submodule: the field accessor traits and impls.
fn gen_inited_module(
    krate: &Ident,
    item: &ItemStruct,
    fields: &[FieldInfo],
    struct_type: &TokenStream,
    names: &Names,
) -> TokenStream {
    let struct_ident = &item.ident;
    let struct_generics = &item.generics;
    let mut defs = TokenStream::new();

    let init = &names.init;
    let f = &names.f;
    let n = &names.n;
    let c = &names.c;

    for fld in fields {
        let fname = &fld.name;
        let fty = &fld.ty;
        let uid = format_ident!("U{}", fld.index);
        let trait_name = format_ident!("{}_inited_{}", struct_ident, fname);
        let mut_name = format_ident!("{}_mut", fname);

        let TraitParts {
            trait_impl_generics,
            trait_ty_generics,
            trait_where,
        } = trait_generics(struct_generics, names);
        let ImplParts {
            impl_generics,
            impl_where,
        } = field_impl_generics(
            struct_generics,
            struct_type,
            quote!(GetField<#fty, ::#krate::typenum::#uid, #c>),
            names,
        );

        defs.extend(quote! {
            pub trait #trait_name #trait_impl_generics #trait_where {
                fn #fname(&self) -> &#fty;
                fn #mut_name(&mut self) -> &mut #fty;
            }

            impl #impl_generics #trait_name #trait_ty_generics
                for chain::Field<#init, #f, #n>
                #impl_where
            {
                fn #fname(&self) -> &#fty {
                    unsafe { Self::get(self) }
                }

                fn #mut_name(&mut self) -> &mut #fty {
                    unsafe { Self::get_mut(self) }
                }
            }
        });
    }

    defs
}

/// Computes the generic tokens for a per-field trait: the struct's params plus
/// the trailing `C` type parameter.
fn trait_generics(struct_generics: &syn::Generics, names: &Names) -> TraitParts {
    let c = &names.c;
    let mut gt = struct_generics.clone();
    gt.params.push(parse_quote!(#c));
    let (impl_gen, ty_gen, where_clause) = gt.split_for_impl();
    TraitParts {
        trait_impl_generics: quote!(#impl_gen),
        trait_ty_generics: quote!(#ty_gen),
        trait_where: match where_clause {
            Some(wc) => quote!(#wc),
            None => quote!(),
        },
    }
}

/// Computes the generic tokens and where clause for a per-field impl:
/// the struct's params plus `const INIT`, `F`, `N`, `C`, and the `MapInit`/`GetField`
/// bounds.
fn field_impl_generics(
    struct_generics: &syn::Generics,
    struct_type: &TokenStream,
    field_bound: TokenStream,
    names: &Names,
) -> ImplParts {
    let init = &names.init;
    let f = &names.f;
    let n = &names.n;
    let c = &names.c;

    let mut gt = struct_generics.clone();
    gt.params.push(parse_quote!(const #init: bool));
    gt.params.push(parse_quote!(#f));
    gt.params.push(parse_quote!(#n));
    gt.params.push(parse_quote!(#c));

    let wc = gt.make_where_clause();
    wc.predicates
        .push(parse_quote!(#n: chain::traits::ThisPtr<Target = #struct_type>));
    wc.predicates
        .push(parse_quote!(#f: chain::traits::Field<Target = #struct_type>));
    wc.predicates.push(parse_quote!(Self: #field_bound));

    let (impl_gen, _, impl_where) = gt.split_for_impl();
    ImplParts {
        impl_generics: quote!(#impl_gen),
        impl_where: match impl_where {
            Some(wc) => quote!(#wc),
            None => quote!(),
        },
    }
}

/// The internal generic parameter names used by the generated code.
///
/// These are allocated so they never collide with the struct's own generic
/// parameter names (e.g. a struct with a `F`/`N`/`C`/`INIT`/`U` type or const
/// parameter).
struct Names {
    u: Ident,
    f: Ident,
    n: Ident,
    c: Ident,
    init: Ident,
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
        f: unique_ident("F", &mut used),
        n: unique_ident("N", &mut used),
        c: unique_ident("C", &mut used),
        init: unique_ident("INIT", &mut used),
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

struct TraitParts {
    trait_impl_generics: TokenStream,
    trait_ty_generics: TokenStream,
    trait_where: TokenStream,
}

struct ImplParts {
    impl_generics: TokenStream,
    impl_where: TokenStream,
}
