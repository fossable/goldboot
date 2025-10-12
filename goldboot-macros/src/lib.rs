use proc_macro::TokenStream;
use quote::quote;
use syn::{self};

/// Automatically implement "Prompt" for all fields in a struct.
#[proc_macro_derive(Prompt)]
pub fn prompt(input: TokenStream) -> TokenStream {
    let ast = syn::parse(input).unwrap();
    impl_prompt(&ast)
}

/// Automatically implement the "size()" method from BuildImage trait.
/// This assumes the struct has a field named "size" of type Size.
#[proc_macro_derive(BuildImageSize)]
pub fn build_image_size(input: TokenStream) -> TokenStream {
    let ast = syn::parse(input).unwrap();
    impl_build_image_size(&ast)
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
                    value.prompt(builder)?;
                }
            }
        } else {
            quote! {
                self.#field.prompt(builder)?;
            }
        }
    });

    let syntax = quote! {
        impl crate::cli::prompt::Prompt for #name {
            fn prompt(
                &mut self,
                builder: &crate::builder::Builder,
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

fn impl_build_image_size(ast: &syn::DeriveInput) -> TokenStream {
    let name = &ast.ident;

    // Verify that the struct has a field named "size"
    let has_size_field = match &ast.data {
        syn::Data::Struct(data) => match &data.fields {
            syn::Fields::Named(fields) => fields
                .named
                .iter()
                .any(|f| f.ident.as_ref().map(|i| i == "size").unwrap_or(false)),
            _ => false,
        },
        _ => false,
    };

    if !has_size_field {
        panic!("BuildImageSize derive requires a field named 'size'");
    }

    let syntax = quote! {
        impl BuildImage for #name {
            fn size(&self) -> &crate::builder::options::size::Size {
                &self.size
            }
        }
    };
    syntax.into()
}

// TODO add pyclass
