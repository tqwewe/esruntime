use proc_macro2::TokenStream;
use quote::quote;
use syn::{
    DeriveInput, Ident, Type,
    parse::{Parse, ParseStream},
    spanned::Spanned,
};

#[derive(Debug)]
pub struct DeriveEventSet {
    ident: Ident,
    events: Vec<(Ident, Type)>,
}

impl DeriveEventSet {
    pub fn expand(self) -> TokenStream {
        let Self { ident, events } = self;

        let event_types = events.iter().map(|(_, ty)| ty);
        let match_arms = events.iter().map(|(variant_ident, ty)| {
            quote! {
                <#ty as ::esruntime_sdk::event::Event>::EVENT_TYPE => {
                    ::std::option::Option::Some(<#ty as ::esruntime_sdk::event::Event>::from_bytes(data).map(#ident::#variant_ident))
                }
            }
        });

        quote! {
            #[automatically_derived]
            impl ::esruntime_sdk::event::EventSet for #ident {
                const EVENT_TYPES: &'static [&'static str] = &[ #( <#event_types as ::esruntime_sdk::event::Event>::EVENT_TYPE, )* ];

                fn from_event(event_type: &str, data: &[u8]) -> std::option::Option<std::result::Result<Self, ::esruntime_sdk::error::SerializationError>> {
                    match event_type {
                        #( #match_arms )*
                        _ => None
                    }
                }
            }
        }
    }
}

impl Parse for DeriveEventSet {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let input: DeriveInput = input.parse()?;

        let events = match input.data {
            syn::Data::Enum(data) => data
                .variants
                .into_iter()
                .map(|variant| match variant.fields {
                    syn::Fields::Unnamed(unnamed) if unnamed.unnamed.len() == 1 => {
                        let field = unnamed.unnamed.into_iter().next().unwrap();
                        Ok((variant.ident, field.ty))
                    }
                    _ => Err(syn::Error::new(
                        variant.fields.span(),
                        "EventSet requires one unnamed field per event type",
                    )),
                })
                .collect::<Result<_, _>>()?,
            _ => {
                return Err(syn::Error::new(
                    input.span(),
                    "EventSet can only be derived on enums",
                ));
            }
        };

        Ok(DeriveEventSet {
            ident: input.ident,
            events,
        })
    }
}
