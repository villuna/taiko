use proc_macro::{self, TokenStream};
use proc_macro2::{Ident, Span};
use quote::quote;
use syn::{parse_macro_input, punctuated::Punctuated, token::Comma, DeriveInput, Fields};

#[proc_macro_derive(MapStrToOwned)]
pub fn map_str_to_owned(input: TokenStream) -> TokenStream {
    let DeriveInput {
        ident,
        data,
        generics,
        ..
    } = parse_macro_input!(input);

    let enum_data = match data {
        syn::Data::Enum(e) => e,
        _ => panic!("derive target must be an enum"),
    };

    let generic_type = {
        let params = &generics.params;

        if params.len() != 1 {
            panic!("only one generic parameter allowed");
        }

        match params.first().unwrap() {
            syn::GenericParam::Type(t) => &t.ident,
            _ => panic!("generic param must be a type"),
        }
    };

    let match_arms = enum_data
        .variants
        .iter()
        .map(|variant| {
            let v_ident = &variant.ident;

            match &variant.fields {
                Fields::Named(fields) => {
                    let arguments_values = fields.named.iter().enumerate().map(|(i, typename)| {
                        let is_generic_type = match &typename.ty {
                            syn::Type::Path(pathname) => pathname.path.is_ident(generic_type),
                            // TODO: map other equivalent types?
                            _ => false,
                        };

                        let field_name = typename.ident.as_ref().unwrap();

                        let ident_name =
                            Ident::new(&format!("__enum_variant{}", i), Span::call_site());

                        let value = if is_generic_type {
                            quote! { #ident_name.to_string() }
                        } else {
                            quote! { #ident_name }
                        };

                        (
                            quote! { #field_name: #ident_name },
                            quote! { #field_name: #value },
                        )
                    });

                    let arguments = arguments_values
                        .clone()
                        .map(|(name, _)| name)
                        .collect::<Punctuated<proc_macro2::TokenStream, Comma>>();

                    let values = arguments_values
                        .map(|(_, value)| value)
                        .collect::<Punctuated<proc_macro2::TokenStream, Comma>>();

                    quote! {
                        #ident::#v_ident { #arguments } => #ident::#v_ident { #values }
                    }
                }

                Fields::Unnamed(fields) => {
                    let arguments_values =
                        fields.unnamed.iter().enumerate().map(|(i, typename)| {
                            let is_generic_type = match &typename.ty {
                                syn::Type::Path(pathname) => pathname.path.is_ident(generic_type),
                                // TODO: map other equivalent types?
                                _ => false,
                            };

                            let ident_name =
                                Ident::new(&format!("__enum_variant{}", i), Span::call_site());

                            let value = if is_generic_type {
                                quote! { #ident_name.to_string() }
                            } else {
                                quote! { #ident_name }
                            };

                            (quote! { #ident_name }, quote! { #value })
                        });

                    let arguments = arguments_values
                        .clone()
                        .map(|(name, _)| name)
                        .collect::<Punctuated<proc_macro2::TokenStream, Comma>>();

                    let values = arguments_values
                        .map(|(_, value)| value)
                        .collect::<Punctuated<proc_macro2::TokenStream, Comma>>();

                    quote! {
                        #ident::#v_ident(#arguments) => #ident::#v_ident(#values)
                    }
                }
                Fields::Unit => quote! {
                    #ident::#v_ident => #ident::#v_ident
                },
            }
        })
        .collect::<Punctuated<proc_macro2::TokenStream, Comma>>();

    quote! {
        impl From<#ident<&str>> for #ident<String> {
            fn from(value: #ident<&str>) -> Self {
                match value {
                    #match_arms,
                }
            }
        }
    }
    .into()
}
