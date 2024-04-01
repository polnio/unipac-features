extern crate proc_macro;
use proc_macro::{Group, Ident, Literal, Span, TokenStream, TokenTree};
use quote::ToTokens;
use std::str::FromStr as _;
use unipac_core::{MANAGERS, PACKAGES};

fn replace(input: String, into: (&&str, &&str)) -> String {
    input
        .replace("__manager", into.0)
        .replace("__Manager", into.1)
        .replace("__MANAGER", into.0.to_uppercase().as_str())
}

fn replace_token(input: TokenTree, into: (&&str, &&str)) -> TokenTree {
    let output = match input {
        TokenTree::Ident(ident) => {
            TokenTree::Ident(Ident::new(&replace(ident.to_string(), into), ident.span()))
        }
        TokenTree::Group(group) => TokenTree::Group(Group::new(
            group.delimiter(),
            TokenStream::from_iter(
                group
                    .stream()
                    .into_iter()
                    .map(|item| replace_token(item, into)),
            ),
        )),
        TokenTree::Literal(literal) => {
            TokenTree::Literal(Literal::from_str(&replace(literal.to_string(), into)).unwrap())
        }
        tt => tt,
    };

    output
}

#[proc_macro]
pub fn for_all(item: TokenStream) -> TokenStream {
    /* println!("{:#?}", item);
    let mut result = TokenStream::new();
    let item = item.to_string();
    result.extend(
        MANAGERS
            .iter()
            .zip(PACKAGES.iter())
            .map(|(manager, package)| {
                item.replace("__manager", manager)
                    .replace("__Manager", package)
                    .replace("__MANAGER", manager.to_uppercase().as_str())
                    .parse::<TokenStream>()
                    .unwrap()
            }),
    );
    result */

    TokenStream::from_iter(MANAGERS.iter().zip(PACKAGES.iter()).flat_map(|into| {
        item.clone()
            .into_iter()
            .map(move |input| replace_token(input, into))
    }))
}

#[proc_macro_attribute]
pub fn for_all_attrs(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let item = syn::parse_macro_input!(item as syn::Item);
    match item {
        syn::Item::Struct(item) => {
            let old_fields = match item.fields {
                syn::Fields::Named(fields) => fields.named,
                _ => panic!("Only named fields are supported"),
            };
            let fields =
                syn::punctuated::Punctuated::from_iter(old_fields.into_iter().flat_map(|field| {
                    if !field
                        .to_token_stream()
                        .to_string()
                        .to_lowercase()
                        .contains("__manager")
                    {
                        return vec![field];
                    }
                    MANAGERS
                        .iter()
                        .zip(PACKAGES.iter())
                        .map(move |into| syn::Field {
                            ident: field.ident.clone().map(|ident| {
                                syn::Ident::new(
                                    &replace(ident.to_string(), into),
                                    Span::call_site().into(),
                                )
                            }),
                            ty: syn::Type::Verbatim(
                                replace(field.ty.to_token_stream().to_string(), into)
                                    .parse()
                                    .unwrap(),
                            ),
                            ..field.clone()
                        })
                        .collect::<Vec<_>>()
                }));
            let new_struct = syn::ItemStruct {
                fields: syn::FieldsNamed {
                    brace_token: syn::token::Brace::default(),
                    named: fields,
                }
                .into(),
                ..item
            };
            new_struct.to_token_stream().into()
        }
        syn::Item::Enum(item) => {
            let new_variants: syn::punctuated::Punctuated<syn::Variant, syn::token::Comma> =
                syn::punctuated::Punctuated::from_iter(item.variants.into_iter().flat_map(
                    |variant| {
                        if !variant
                            .to_token_stream()
                            .to_string()
                            .to_lowercase()
                            .contains("__manager")
                        {
                            return vec![variant];
                        }
                        MANAGERS
                            .iter()
                            .zip(PACKAGES.iter())
                            .map(move |into| syn::Variant {
                                ident: syn::Ident::new(
                                    &replace(variant.ident.to_string(), into),
                                    variant.ident.span(),
                                ),
                                fields: match variant.fields.clone() {
                                    syn::Fields::Unnamed(fields) => {
                                        let new_fields =
                                            fields.unnamed.into_iter().map(|field| syn::Field {
                                                ty: syn::Type::Verbatim(
                                                    replace(
                                                        field.ty.to_token_stream().to_string(),
                                                        into,
                                                    )
                                                    .parse()
                                                    .unwrap(),
                                                ),
                                                ..field
                                            });
                                        let new_fields = syn::FieldsUnnamed {
                                            unnamed: syn::punctuated::Punctuated::from_iter(
                                                new_fields,
                                            ),
                                            ..fields
                                        };
                                        syn::Fields::Unnamed(new_fields)
                                    }
                                    fields => fields,
                                },
                                ..variant.clone()
                            })
                            .collect::<Vec<_>>()
                    },
                ));
            let new_enum = syn::ItemEnum {
                variants: new_variants,
                ..item
            };
            new_enum.to_token_stream().into()
        }
        syn::Item::Fn(item) => {
            let item_str = item.to_token_stream().to_string();
            let mut new_fns = TokenStream::new();
            new_fns.extend(
                MANAGERS
                    .iter()
                    .zip(PACKAGES.iter())
                    .map(|(manager, package)| {
                        item_str
                            .replace("__manager", manager)
                            .replace("__Manager", package)
                            .replace("__MANAGER", manager.to_uppercase().as_str())
                            .parse::<TokenStream>()
                            .unwrap()
                    }),
            );
            new_fns
        }
        _ => panic!("Only structs, enum and functions are supported"),
    }
}
