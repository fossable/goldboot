use darling::FromMeta;
use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{self, parse_macro_input};

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

/// A newtype wrapper around Vec<syn::Path> so darling can parse the list.
#[derive(Debug)]
struct ArchList(Vec<syn::Path>);

impl darling::FromMeta for ArchList {
    fn from_list(items: &[darling::ast::NestedMeta]) -> darling::Result<Self> {
        let paths = items
            .iter()
            .map(|item| match item {
                darling::ast::NestedMeta::Meta(syn::Meta::Path(p)) => Ok(p.clone()),
                _ => Err(darling::Error::unexpected_type("expected identifier")),
            })
            .collect::<darling::Result<Vec<_>>>()?;
        Ok(ArchList(paths))
    }
}

/// Arguments parsed from `#[Os(architectures(Amd64, Arm64))]`
#[derive(Debug, FromMeta)]
struct OsArgs {
    architectures: ArchList,
}

/// OS registration attribute macro.
///
/// Usage: `#[goldboot_macros::Os(architectures = [Amd64, Arm64])]`
///
/// Generates:
/// - `impl OsTrait for StructName { ... }`
/// - `inventory::submit! { OsDescriptor { ... } }`
#[allow(non_snake_case)]
#[proc_macro_attribute]
pub fn Os(args: TokenStream, input: TokenStream) -> TokenStream {
    let attr_args = match darling::ast::NestedMeta::parse_meta_list(args.into()) {
        Ok(v) => v,
        Err(e) => return TokenStream::from(darling::Error::from(e).write_errors()),
    };

    let os_args = match OsArgs::from_list(&attr_args) {
        Ok(v) => v,
        Err(e) => return TokenStream::from(e.write_errors()),
    };

    let input = parse_macro_input!(input as syn::DeriveInput);
    let name = &input.ident;
    let name_str = name.to_string();

    // Check if struct has an `arch` field
    let has_arch_field = match &input.data {
        syn::Data::Struct(data) => match &data.fields {
            syn::Fields::Named(fields) => fields
                .named
                .iter()
                .any(|f| f.ident.as_ref().map(|i| i == "arch").unwrap_or(false)),
            _ => false,
        },
        _ => false,
    };

    // Check if struct has a `size` field
    let has_size_field = match &input.data {
        syn::Data::Struct(data) => match &data.fields {
            syn::Fields::Named(fields) => fields
                .named
                .iter()
                .any(|f| f.ident.as_ref().map(|i| i == "size").unwrap_or(false)),
            _ => false,
        },
        _ => false,
    };

    let arch_impls: Vec<TokenStream2> = os_args
        .architectures
        .0
        .iter()
        .map(|p| {
            quote! { goldboot_image::ImageArch::#p }
        })
        .collect();

    let os_arch_impl = if has_arch_field {
        quote! {
            fn os_arch(&self) -> goldboot_image::ImageArch {
                self.arch.0
            }
        }
    } else {
        // Use first architecture as default
        let first_arch = &arch_impls[0];
        quote! {
            fn os_arch(&self) -> goldboot_image::ImageArch {
                #first_arch
            }
        }
    };

    let os_size_impl = if has_size_field {
        quote! {
            fn os_size(&self) -> u64 {
                self.size.clone().into()
            }
        }
    } else {
        quote! {
            fn os_size(&self) -> u64 {
                0
            }
        }
    };

    let expanded = quote! {
        #input

        impl crate::builder::os::OsTrait for #name {
            fn os_name(&self) -> &'static str {
                #name_str
            }

            fn os_architectures(&self) -> &'static [goldboot_image::ImageArch] {
                &[#(#arch_impls),*]
            }

            #os_arch_impl

            #os_size_impl

            fn serialize_ron(&self, config: &ron::ser::PrettyConfig) -> anyhow::Result<String> {
                Ok(ron::ser::to_string_pretty(self, config.clone())?)
            }
        }

        inventory::submit! {
            crate::builder::os::OsDescriptor {
                name: #name_str,
                architectures: &[#(#arch_impls),*],
                default: || Box::new(#name::default()),
                deserialize_ron: |s| {
                    let opts = ron::Options::default()
                        .with_default_extension(ron::extensions::Extensions::IMPLICIT_SOME);
                    Ok(Box::new(opts.from_str::<#name>(s)?))
                },
            }
        }
    };

    expanded.into()
}
