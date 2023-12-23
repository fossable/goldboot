use proc_macro::TokenStream;
use quote::quote;
use syn::{self, DataStruct};

/// Automatically implement "Prompt" for all fields in a struct.
#[proc_macro_derive(Prompt)]
pub fn prompt(input: TokenStream) -> TokenStream {
    let ast = syn::parse(input).unwrap();
    impl_prompt(&ast)
}

fn impl_prompt(ast: &syn::DeriveInput) -> TokenStream {
    let name = &ast.ident;
    let fields: Vec<String> = match &ast.data {
        syn::Data::Struct(data) => match &data.fields {
            syn::Fields::Named(fields) => fields
                .named
                .iter()
                .map(|f| f.ident.clone().unwrap().to_string())
                .collect(),
            _ => panic!(),
        },
        _ => panic!(),
    };

    let gen = quote! {
        impl Prompt for #name {
            fn prompt(
                &mut self,
                _: &Foundry,
                theme: impl dialoguer::theme::Theme,
            ) -> anyhow::Result<()> {
                todo!()
            }
        }
    };
    gen.into()
}

// TODO probably need a macro for ImageMold and Fabricator
