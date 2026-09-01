use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{Error, Fields, Ident, ItemStruct, Type};

use crate::config::PartialConfig;

/// Generates the full `partial` macro output for a struct.
///
/// The output consists of:
/// - the original struct,
/// - a module containing the `PartialThis` impl, the per-field `Field`
///   descriptors, and the field builder/accessor traits,
/// - a `pub use` re-export so the generated traits are brought into scope.
pub(crate) fn generate(item: &ItemStruct, cfg: &PartialConfig) -> syn::Result<TokenStream> {
    ensure_supported(item)?;

    let struct_ident = &item.ident;
    let krate = cfg.crate_name();
    let fields = collect_fields(item)?;
    let module_ident = cfg.module_name(item);

    // Build the nested `Field<false, ..., U>` output type and the matching
    // constructor. Fields are folded in declaration order, which makes the
    // last declared field the outermost layer.
    let mut output_ty = quote!(U);
    let mut ctor = quote!(this);
    for f in &fields {
        let fname = &f.ident;
        output_ty = quote!(chain::Field<false, fields::#fname, #output_ty>);
        ctor = quote!(chain::Field::uninit(#ctor));
    }

    let fields_defs = gen_fields_module(&krate, item, &fields);
    let uninit_defs = gen_uninit_module(&krate, item, &fields);
    let inited_defs = gen_inited_module(&krate, item, &fields);

    Ok(quote! {
        #item
        #[allow(nonstandard_style)]
        mod #module_ident {
            use ::#krate::{ PartialThis, UninitThis, chain::{self} };
            use super::*;

            impl<U: UninitThis<Target = #struct_ident>> PartialThis<U> for #struct_ident {
                type Output = #output_ty;
                fn partial(this: U) -> Self::Output {
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

/// Collects the named fields of a struct with their 1-based index.
struct FieldInfo {
    ident: Ident,
    ty: Type,
    index: usize,
}

/// Rejects structs that are currently unsupported (generics or non-named fields).
fn ensure_supported(item: &ItemStruct) -> syn::Result<()> {
    if item.generics.params.iter().next().is_some() || item.generics.where_clause.is_some() {
        return Err(Error::new_spanned(
            &item.generics,
            "`partial` does not support generic structs yet",
        ));
    }
    Ok(())
}

fn collect_fields(item: &ItemStruct) -> syn::Result<Vec<FieldInfo>> {
    let named = match &item.fields {
        Fields::Named(named) => named.named.iter().collect::<Vec<_>>(),
        _ => {
            return Err(Error::new_spanned(
                item,
                "`partial` only supports structs with named fields",
            ));
        }
    };

    let mut fields = Vec::with_capacity(named.len());
    for (i, field) in named.into_iter().enumerate() {
        let ident = field
            .ident
            .clone()
            .ok_or_else(|| Error::new_spanned(field, "field is missing an identifier"))?;
        // Field indices start at `U1`.
        fields.push(FieldInfo {
            ident,
            ty: field.ty.clone(),
            index: i + 1,
        });
    }
    Ok(fields)
}

/// Generates the `fields` submodule: a unit struct per field plus its `Field` impl.
fn gen_fields_module(krate: &Ident, item: &ItemStruct, fields: &[FieldInfo]) -> TokenStream {
    let struct_ident = &item.ident;
    let mut defs = TokenStream::new();

    for f in fields {
        let fname = &f.ident;
        let fty = &f.ty;
        let uid = format_ident!("U{}", f.index);
        defs.extend(quote! {
            #[derive(Debug)]
            pub struct #fname;

            impl Field for #fname {
                type Target = super::#struct_ident;
                type Type = #fty;
                type Id = ::#krate::typenum::#uid;

                unsafe fn drop<const INIT: bool>(
                    this: &mut ::core::mem::MaybeUninit<Self::Target>,
                ) {
                    if INIT {
                        unsafe { ::core::ptr::drop_in_place(&mut (*this.as_mut_ptr()).#fname) };
                    }
                }

                unsafe fn init(
                    this: &mut ::core::mem::MaybeUninit<Self::Target>,
                    value: Self::Type,
                ) {
                    unsafe { ::core::ptr::write(&mut (*this.as_mut_ptr()).#fname, value) }
                }

                unsafe fn get(
                    this: &::core::mem::MaybeUninit<Self::Target>,
                ) -> &Self::Type {
                    unsafe { &(*this.as_ptr()).#fname }
                }

                unsafe fn get_mut(
                    this: &mut ::core::mem::MaybeUninit<Self::Target>,
                ) -> &mut Self::Type {
                    unsafe { &mut (*this.as_mut_ptr()).#fname }
                }
            }
        });
    }

    defs
}

/// Generates the `uninit_fields` submodule: the field builder traits and impls.
fn gen_uninit_module(krate: &Ident, item: &ItemStruct, fields: &[FieldInfo]) -> TokenStream {
    let struct_ident = &item.ident;
    let mut defs = TokenStream::new();

    for f in fields {
        let fname = &f.ident;
        let fty = &f.ty;
        let uid = format_ident!("U{}", f.index);
        let trait_name = format_ident!("{}_uninit_{}", struct_ident, fname);
        let assume_name = format_ident!("assume_init_{}", fname);

        defs.extend(quote! {
            pub trait #trait_name<T> {
                type Output;
                fn #fname(self, value: #fty) -> Self::Output;
                fn #assume_name(self) -> Self::Output;
            }

            impl<const INIT: bool, F, N, C> #trait_name<C> for chain::Field<INIT, F, N>
            where
                N: chain::traits::ThisPtr<Target = #struct_ident>,
                F: chain::traits::Field<Target = #struct_ident>,
                Self: MapInit<#fty, ::#krate::typenum::#uid, C>,
            {
                type Output = <Self as MapInit<#fty, ::#krate::typenum::#uid, C>>::Result;

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
fn gen_inited_module(krate: &Ident, item: &ItemStruct, fields: &[FieldInfo]) -> TokenStream {
    let struct_ident = &item.ident;
    let mut defs = TokenStream::new();

    for f in fields {
        let fname = &f.ident;
        let fty = &f.ty;
        let uid = format_ident!("U{}", f.index);
        let trait_name = format_ident!("{}_inited_{}", struct_ident, fname);
        let mut_name = format_ident!("{}_mut", fname);

        defs.extend(quote! {
            pub trait #trait_name<T> {
                fn #fname(&self) -> &#fty;
                fn #mut_name(&mut self) -> &mut #fty;
            }

            impl<const INIT: bool, F, N, C> #trait_name<C> for chain::Field<INIT, F, N>
            where
                N: chain::traits::ThisPtr<Target = #struct_ident>,
                F: chain::traits::Field<Target = #struct_ident>,
                Self: GetField<#fty, ::#krate::typenum::#uid, C>,
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
