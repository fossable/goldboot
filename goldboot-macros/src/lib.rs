use proc_macro::TokenStream;
use quote::quote;
use syn::{self};

/// Automatically implement "Prompt" for all fields in a struct.
#[proc_macro_derive(Prompt)]
pub fn prompt(input: TokenStream) -> TokenStream {
    let ast = syn::parse(input).unwrap();
    impl_prompt(&ast)
}

fn impl_prompt(ast: &syn::DeriveInput) -> TokenStream {
    let name = &ast.ident;

    let fields_to_prompt: Vec<_> = match &ast.data {
        syn::Data::Struct(data) => match &data.fields {
            syn::Fields::Named(fields) => fields
                .named
                .iter()
                .filter_map(|f| {
                    // Skip fields with #[serde(flatten)]
                    let is_flattened = f.attrs.iter().any(|attr| {
                        attr.path().is_ident("serde")
                            && attr
                                .parse_args::<syn::Ident>()
                                .map(|i| i == "flatten")
                                .unwrap_or(false)
                    });

                    if is_flattened {
                        None
                    } else {
                        Some((f.ident.clone().unwrap(), f.ty.clone()))
                    }
                })
                .collect(),
            _ => panic!("Prompt derive only works on structs with named fields"),
        },
        _ => panic!("Prompt derive only works on structs"),
    };

    let prompt_calls = fields_to_prompt.iter().map(|(field, ty)| {
        // Check if the type is an Option
        if is_option_type(ty) {
            quote! {
                if let Some(ref mut value) = self.#field {
                    value.prompt(foundry)?;
                }
            }
        } else {
            quote! {
                self.#field.prompt(foundry)?;
            }
        }
    });

    let syntax = quote! {
        impl crate::cli::prompt::Prompt for #name {
            fn prompt(
                &mut self,
                foundry: &crate::foundry::Foundry,
            ) -> anyhow::Result<()> {
                #(#prompt_calls)*
                Ok(())
            }
        }
    };
    syntax.into()
}

fn is_option_type(ty: &syn::Type) -> bool {
    if let syn::Type::Path(type_path) = ty {
        if let Some(segment) = type_path.path.segments.last() {
            return segment.ident == "Option";
        }
    }
    false
}

// TODO probably need a macro for ImageMold and Fabricator
