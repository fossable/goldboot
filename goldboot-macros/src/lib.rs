use proc_macro::TokenStream;
use quote::{quote, ToTokens};
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

/// Generate a Starlark constructor function definition for this type.
/// This is used at build time to generate goldboot_dsl.star
#[proc_macro_derive(StarlarkConstructor, attributes(starlark_doc))]
pub fn starlark_constructor(input: TokenStream) -> TokenStream {
    let ast = syn::parse(input).unwrap();
    impl_starlark_constructor(&ast)
}

fn type_to_starlark_hint(ty: &syn::Type) -> String {
    let ty_string = quote!(#ty).to_string();

    // Handle Option<T>
    if ty_string.starts_with("Option <") {
        // Extract the inner type
        let inner = ty_string.trim_start_matches("Option <").trim_end_matches('>').trim();
        return format!("{} | None", map_rust_type_to_starlark(inner));
    }

    map_rust_type_to_starlark(&ty_string)
}

fn map_rust_type_to_starlark(rust_type: &str) -> String {
    match rust_type.trim() {
        "String" | "& str" | "str" => "str".to_string(),
        "bool" => "bool".to_string(),
        "i32" | "i64" | "u32" | "u64" | "usize" => "int".to_string(),
        "f32" | "f64" => "float".to_string(),
        t if t.starts_with("Vec <") => "list".to_string(),
        "Url" | "url :: Url" => "str".to_string(), // URLs are passed as strings in Starlark
        "Size" => "str".to_string(), // Size is passed as string like "20GiB"
        "Arch" => "str".to_string(), // Arch is a wrapper around ImageArch, treat as string
        _ => "dict".to_string(), // Default to dict for custom types
    }
}

fn impl_starlark_constructor(ast: &syn::DeriveInput) -> TokenStream {
    let name = &ast.ident;

    // Extract fields
    let mut fields: Vec<(syn::Ident, syn::Type, bool)> = match &ast.data {
        syn::Data::Struct(data) => match &data.fields {
            syn::Fields::Named(fields) => fields
                .named
                .iter()
                .filter_map(|f| {
                    let is_flattened = f.attrs.iter().any(|attr| {
                        attr.path().is_ident("serde")
                            && attr.meta.to_token_stream().to_string().contains("flatten")
                    });

                    let has_default = f.attrs.iter().any(|attr| {
                        attr.path().is_ident("serde")
                            && attr.meta.to_token_stream().to_string().contains("default")
                    });

                    // Skip fields with defaults (they're optional in Starlark)
                    if has_default {
                        return None;
                    }

                    // For flattened Hostname, expand to a String field
                    if is_flattened {
                        let ty_string = quote!(#f.ty).to_string();
                        if ty_string.contains("Hostname") {
                            return Some((
                                syn::Ident::new("hostname", f.ident.as_ref().unwrap().span()),
                                syn::parse_quote!(String),
                                false
                            ));
                        }
                        // Skip other flattened fields
                        return None;
                    }

                    Some((f.ident.clone().unwrap(), f.ty.clone(), false))
                })
                .collect(),
            _ => panic!("StarlarkConstructor only works on structs with named fields"),
        },
        _ => panic!("StarlarkConstructor only works on structs"),
    };

    // Sort fields: required first, then optional
    // This ensures Starlark parameter ordering is correct
    let (required_fields, optional_fields): (Vec<_>, Vec<_>) = fields.iter()
        .partition(|(_, ty, _)| !is_option_type(ty));

    // Generate parameter list with type hints
    let mut params: Vec<String> = required_fields.iter().map(|(field, ty, _)| {
        let field_name = field.to_string();
        let type_hint = type_to_starlark_hint(ty);
        format!("{}: {}", field_name, type_hint)
    }).collect();

    params.extend(optional_fields.iter().map(|(field, ty, _)| {
        let field_name = field.to_string();
        let type_hint = type_to_starlark_hint(ty);
        format!("{}: {} = None", field_name, type_hint)
    }));

    let params_str = params.join(",\n    ");

    let mut required_entries: Vec<String> = fields.iter()
        .filter(|(_, ty, _)| !is_option_type(ty))
        .map(|(field, _, _)| format!(r#"        "{field}": {field},"#, field = field.to_string()))
        .collect();

    // Check if this is an OS type (needs "os" discriminator)
    // OS types typically have 'iso' field and the name doesn't end with common helper type names
    let field_names: Vec<String> = fields.iter().map(|(f, _, _)| f.to_string()).collect();
    let name_str = name.to_string();
    let is_helper_type = name_str == "Iso"
        || name_str.ends_with("Edition")
        || name_str.ends_with("Release")
        || name_str.ends_with("Config")
        || name_str.ends_with("Path");

    let is_os_type = field_names.contains(&"iso".to_string()) && !is_helper_type;

    if is_os_type {
        // Add "os" discriminator as the first field
        required_entries.insert(0, format!(r#"        "os": "{name}","#, name = name));
    }

    let required_entries_str = required_entries.join("\n");

    let optional_checks: String = fields.iter()
        .filter(|(_, ty, _)| is_option_type(ty))
        .map(|(field, _, _)| {
            let field_name = field.to_string();
            format!(
                r#"    if {field} != None:
        config["{field}"] = {field}"#,
                field = field_name
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    let starlark_fn = format!(
        r#"def {name}(
    {params}
) -> dict:
    """Create a {name} configuration."""
    config = {{
{required_entries}
    }}
{optional_checks}
    return config
"#,
        name = name,
        params = params_str,
        required_entries = required_entries_str,
        optional_checks = optional_checks
    );

    // Store the function definition as a const
    let const_name = syn::Ident::new(&format!("STARLARK_FN_{}", name.to_string().to_uppercase()), name.span());
    let registration_name = syn::Ident::new(&format!("STARLARK_FN_{}_REGISTRATION", name.to_string().to_uppercase()), name.span());

    let syntax = quote! {
        #[doc(hidden)]
        pub const #const_name: &str = #starlark_fn;

        #[linkme::distributed_slice(crate::config::starlark_dsl::STARLARK_DSL_FUNCTIONS)]
        #[linkme(crate = linkme)]
        static #registration_name: &str = #const_name;
    };

    syntax.into()
}

/// Generate Starlark constructor functions for each variant of an enum.
/// Each variant becomes a separate function that returns a dict with the variant name as a key.
#[proc_macro_derive(StarlarkEnumConstructors)]
pub fn starlark_enum_constructors(input: TokenStream) -> TokenStream {
    let ast = syn::parse(input).unwrap();
    impl_starlark_enum_constructors(&ast)
}

fn impl_starlark_enum_constructors(ast: &syn::DeriveInput) -> TokenStream {
    let enum_name = &ast.ident;

    let variants: Vec<_> = match &ast.data {
        syn::Data::Enum(data) => data.variants.iter().collect(),
        _ => panic!("StarlarkEnumConstructors only works on enums"),
    };

    let mut generated_consts = vec![];

    for variant in variants {
        let variant_name = &variant.ident;

        // Handle different variant types
        let starlark_fn = match &variant.fields {
            // Tuple variant with one field: Plaintext(String)
            syn::Fields::Unnamed(fields) if fields.unnamed.len() == 1 => {
                let field_type = &fields.unnamed.first().unwrap().ty;
                let type_hint = type_to_starlark_hint(field_type);

                format!(
                    r#"def {variant_name}(value: {type_hint}) -> dict:
    """Create a {variant_name} {enum_name}."""
    return {{"{snake_case}": value}}
"#,
                    variant_name = variant_name,
                    type_hint = type_hint,
                    enum_name = enum_name,
                    snake_case = to_snake_case(&variant_name.to_string())
                )
            }
            // Unit variant or other types
            _ => {
                format!(
                    r#"def {variant_name}() -> dict:
    """Create a {variant_name} {enum_name}."""
    return {{"{snake_case}": None}}
"#,
                    variant_name = variant_name,
                    enum_name = enum_name,
                    snake_case = to_snake_case(&variant_name.to_string())
                )
            }
        };

        let const_name = syn::Ident::new(
            &format!("STARLARK_FN_{}_{}", enum_name.to_string().to_uppercase(), variant_name.to_string().to_uppercase()),
            variant_name.span()
        );
        let registration_name = syn::Ident::new(
            &format!("STARLARK_FN_{}_{}_REGISTRATION", enum_name.to_string().to_uppercase(), variant_name.to_string().to_uppercase()),
            variant_name.span()
        );

        generated_consts.push(quote! {
            #[doc(hidden)]
            pub const #const_name: &str = #starlark_fn;

            #[linkme::distributed_slice(crate::config::starlark_dsl::STARLARK_DSL_FUNCTIONS)]
            #[linkme(crate = linkme)]
            static #registration_name: &str = #const_name;
        });
    }

    let syntax = quote! {
        #(#generated_consts)*
    };

    syntax.into()
}

fn to_snake_case(s: &str) -> String {
    let mut result = String::new();
    for (i, ch) in s.chars().enumerate() {
        if ch.is_uppercase() && i > 0 {
            result.push('_');
        }
        result.push(ch.to_ascii_lowercase());
    }
    result
}
