use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote};
use syn::{spanned::Spanned, Data, DeriveInput, Fields, Ident};

use crate::helpers::{non_enum_error, HasStrumVariantProperties};

pub fn enum_map_inner(ast: &DeriveInput) -> syn::Result<TokenStream> {
    let name = &ast.ident;
    let gen = &ast.generics;
    let vis = &ast.vis;
    let doc_comment = format!("A map over the variants of [{}]", name);

    if gen.lifetimes().count() > 0 {
        return Err(syn::Error::new(
            Span::call_site(),
            "This macro doesn't support enums with lifetimes.",
        ));
    }

    let Data::Enum(data_enum) = &ast.data else {
        return Err(non_enum_error())
    };

    let variants = &data_enum.variants;

    // the identifiers of each variant, in PascalCase
    let mut pascal_idents = Vec::new();
    // the identifiers of each struct field, in snake_case
    let mut snake_idents = Vec::new();
    // match arms in the form `MyEnumMap::Variant => &self.variant,`
    let mut get_matches = Vec::new();
    // match arms in the form `MyEnumMap::Variant => &mut self.variant,`
    let mut get_matches_mut = Vec::new();
    // match arms in the form `MyEnumMap::Variant => self.variant = new_value`
    let mut set_matches = Vec::new();
    // struct fields of the form `variant: func(MyEnum::Variant),*
    let mut closure_fields = Vec::new();
    // struct fields of the form `variant: func(MyEnum::Variant, self.variant),`
    let mut transform_fields = Vec::new();

    for variant in variants {
        // skip disabled variants
        if variant.get_variant_properties()?.disabled.is_some() {
            continue;
        }
        // Error on fields with data
        let Fields::Unit = &variant.fields else {
            return Err(syn::Error::new(
                variant.fields.span(),
                "This macro doesn't support enums with non-unit variants",
            ))
        };

        let pascal_case = &variant.ident;
        pascal_idents.push(pascal_case);
        // switch PascalCase to snake_case. This naively assumes they use PascalCase
        let snake_case = format_ident!(
            "{}",
            pascal_case
                .to_string()
                .chars()
                .enumerate()
                .fold(String::new(), |mut s, (i, c)| {
                    if c.is_uppercase() && i > 0 {
                        s.push('-');
                    }
                    s.push(c.to_ascii_lowercase());
                    s
                })
        );

        get_matches.push(quote! {#name::#pascal_case => &self.#snake_case,});
        get_matches_mut.push(quote! {#name::#pascal_case => &mut self.#snake_case,});
        set_matches.push(quote! {#name::#pascal_case => self.#snake_case = new_value,});
        closure_fields.push(quote!{#snake_case: func(#name::#pascal_case),});
        transform_fields.push(quote!{#snake_case: func(#name::#pascal_case, self.#snake_case),});
        snake_idents.push(snake_case);
    }

    let map_name = syn::parse_str::<Ident>(&format!("{}Map", name)).unwrap();

    Ok(quote! {
        #[doc = #doc_comment]
        #[allow(
            missing_copy_implementations,
        )]
        #[derive(Debug, Clone, Default, PartialEq, Hash)]
        #vis struct #map_name<T> {
            #(#snake_idents: T,)*
        }

        impl<T> #map_name<T> {
            #vis fn new(
                #(#snake_idents: T,)*
            ) -> #map_name<T> {
                #map_name {
                    #(#snake_idents,)*
                }
            }
            
            #vis fn filled(value: T) -> #map_name<T> {
              #map_name {
                #(#snake_idents: value.clone(),)*
              }
            }
            
            #vis fn from_closure<F: Fn(#name) -> T> -> #map_name<T> {
              #map_name {
                #(#closure_fields)*
              }
            }
            
            #vis fn transform<U, F(#name, T) -> U> -> #map_name<U> {
              #map_name {
                #(#transform_fields)*
              }
            }

            // // E.g. so that if you're using EnumIter as well, these functions work nicely
            // fn get(&self, variant: #name) -> &T {
            //     match variant {
            //         #(#get_matches)*
            //     }
            // }

            // fn set(&mut self, variant: #name, new_value: T) {
            //     match variant {
            //         #(#set_matches)*
            //     }
            // }
        }

        impl<T> core::ops::Index<#name> for #map_name<T> {
            type Output = T;

            fn index(&self, idx: #name) -> &T {
                match idx {
                    #(#get_matches)*
                }
            }
        }

        impl<T> core::ops::IndexMut<#name> for #map_name<T> {
            fn index_mut(&mut self, idx: #name) -> &mut T {
                match idx {
                    #(#get_matches_mut)*
                }
            }
        }
    })
}
