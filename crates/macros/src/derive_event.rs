use std::collections::HashMap;

use proc_macro2::TokenStream;
use quote::quote;
use syn::{
    DeriveInput, Ident, LitStr,
    parse::{Parse, ParseStream},
};

#[derive(Debug)]
pub struct DeriveEvent {
    ident: Ident,
    event_type: LitStr,
    domain_ids: HashMap<Ident, LitStr>,
}

impl DeriveEvent {
    pub fn expand(self) -> TokenStream {
        let Self {
            ident,
            event_type,
            domain_ids,
        } = self;

        let domain_ids_inserts = domain_ids.into_iter().map(|(ident, domain_id)| {
            quote! {
                ids.insert(#domain_id, ::esruntime_sdk::domain_id::DomainIdValue::from(::std::clone::Clone::clone(&self.#ident)));
            }
        });

        quote! {
            #[automatically_derived]
            impl ::esruntime_sdk::event::Event for #ident {
                const EVENT_TYPE: &'static str = #event_type;

                fn to_bytes(&self) -> ::std::result::Result<::std::vec::Vec<u8>, ::esruntime_sdk::error::SerializationError> {
                    ::esruntime_sdk::__private::serde_json::to_vec(self).map_err(::std::convert::Into::into)
                }

                fn from_bytes(data: &[u8]) -> ::std::result::Result<Self, ::esruntime_sdk::error::SerializationError> {
                    ::esruntime_sdk::__private::serde_json::from_slice(data).map_err(::std::convert::Into::into)
                }

                fn domain_ids(&self) -> ::esruntime_sdk::domain_id::DomainIdValues {
                    let mut ids = ::std::collections::HashMap::new();
                    #( #domain_ids_inserts )*
                    ids
                }
            }

        }
    }
}

impl Parse for DeriveEvent {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let input: DeriveInput = input.parse()?;

        let event_type = input
            .attrs
            .iter()
            .find_map(|attr| {
                if attr.path().is_ident("event_type") {
                    Some(attr.parse_args())
                } else {
                    None
                }
            })
            .transpose()?
            .unwrap_or_else(|| LitStr::new(&input.ident.to_string(), input.ident.span()));

        let domain_ids = match input.data {
            syn::Data::Struct(data) => data
                .fields
                .into_iter()
                .filter_map(|field| {
                    let attr = field
                        .attrs
                        .into_iter()
                        .find(|attr| attr.path().is_ident("domain_id"))?;

                    match attr.meta {
                        syn::Meta::Path(_) => {
                            let ident = field.ident?;
                            let domain_id = LitStr::new(&ident.to_string(), ident.span());
                            Some(Ok((ident, domain_id)))
                        }
                        syn::Meta::List(list) => {
                            let ident = field.ident?;
                            match list.parse_args() {
                                Ok(domain_id) => Some(Ok((ident, domain_id))),
                                Err(err) => Some(Err(err)),
                            }
                        }
                        syn::Meta::NameValue(_) => None,
                    }
                })
                .collect::<Result<_, _>>()?,
            _ => HashMap::new(),
        };

        Ok(DeriveEvent {
            ident: input.ident,
            event_type,
            domain_ids,
        })
    }
}
