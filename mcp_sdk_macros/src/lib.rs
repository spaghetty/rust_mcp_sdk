extern crate proc_macro;
use proc_macro::TokenStream;
use quote::{quote, ToTokens};
use syn::{
    parse_macro_input, Data, DeriveInput, Field, Fields, Ident, LitBool, LitStr, Meta,
    Result as SynResult, Token, Type, // Removed unused: Attribute, Lit, MetaList, MetaNameValue, Path, token
};
use syn::ext::IdentExt; // For Ident::peek_any for parsing keywords
use syn::parse::{Parse, ParseStream};
use proc_macro2::TokenStream as TokenStream2;


#[derive(Default, Debug)]
struct FieldArgs {
    desc: Option<String>,
    rename: Option<String>,
    skip: bool,
    required: Option<bool>,
}

// Custom parser for the contents of #[tool_arg(...)]
impl Parse for FieldArgs {
    fn parse(input: ParseStream) -> SynResult<Self> {
        let mut args = FieldArgs::default();
        while !input.is_empty() {
            let key: Ident = input.call(Ident::parse_any)?; // Use parse_any for keywords

            if key == "desc" {
                input.parse::<Token![=]>()?;
                args.desc = Some(input.parse::<LitStr>()?.value());
            } else if key == "rename" {
                input.parse::<Token![=]>()?;
                args.rename = Some(input.parse::<LitStr>()?.value());
            } else if key == "skip" {
                if input.peek(Token![=]) {
                     input.parse::<Token![=]>()?;
                     let val_bool = input.parse::<LitBool>()?;
                     if val_bool.value {
                        args.skip = true;
                     }
                } else {
                    args.skip = true;
                }
            } else if key == "required" {
                input.parse::<Token![=]>()?;
                args.required = Some(input.parse::<LitBool>()?.value());
            } else {
                return Err(syn::Error::new(key.span(), format!("unknown tool_arg attribute key: {}", key)));
            }

            if !input.is_empty() {
                input.parse::<Token![,]>()?;
            }
        }
        Ok(args)
    }
}


fn parse_field_attributes(field: &Field) -> SynResult<FieldArgs> {
    let mut aggregated_args = FieldArgs::default();

    for attr in &field.attrs {
        if attr.path().is_ident("tool_arg") {
            match &attr.meta {
                Meta::List(meta_list) => {
                    let parsed_args_for_attr = meta_list.parse_args::<FieldArgs>()?;

                    if parsed_args_for_attr.desc.is_some() {
                        aggregated_args.desc = parsed_args_for_attr.desc;
                    }
                    if parsed_args_for_attr.rename.is_some() {
                        aggregated_args.rename = parsed_args_for_attr.rename;
                    }
                    if parsed_args_for_attr.skip {
                        aggregated_args.skip = true;
                    }
                    if parsed_args_for_attr.required.is_some() {
                        aggregated_args.required = parsed_args_for_attr.required;
                    }
                    // If multiple #[tool_arg] attributes exist on a field, this will process the last one's values
                    // for non-boolean flags, or OR the boolean flags. This is reasonable.
                }
                _ => {
                    return Err(syn::Error::new_spanned(
                        attr.meta.to_token_stream(),
                        "Expected #[tool_arg(key = value, ...)] format for tool_arg attribute"
                    ));
                }
            }
        }
    }
    Ok(aggregated_args)
}


fn is_option(ty: &Type) -> bool {
    if let Type::Path(type_path) = ty {
        if type_path.qself.is_none() && type_path.path.segments.len() == 1 {
            let segment = &type_path.path.segments[0];
            return segment.ident == "Option";
        }
    }
    false
}

fn type_to_schema(ty: &Type, struct_name: &Ident) -> TokenStream2 {
    if is_option(ty) {
        if let Type::Path(type_path) = ty {
            if let syn::PathArguments::AngleBracketed(angle_args) = &type_path.path.segments[0].arguments {
                if let Some(syn::GenericArgument::Type(inner_ty)) = angle_args.args.first() {
                    return type_to_schema(inner_ty, struct_name);
                }
            }
        }
        return quote! { ::serde_json::json!({ "type": "null" }) };
    }

    match ty {
        Type::Path(type_path) => {
            if type_path.qself.is_none() {
                let path = &type_path.path; // path is syn::Path
                if path.is_ident("String") {
                    quote! { ::serde_json::json!({ "type": "string" }) }
                } else if path.is_ident("i8") || path.is_ident("i16") || path.is_ident("i32") || path.is_ident("i64") || path.is_ident("isize") ||
                          path.is_ident("u8") || path.is_ident("u16") || path.is_ident("u32") || path.is_ident("u64") || path.is_ident("usize") {
                    quote! { ::serde_json::json!({ "type": "integer" }) }
                } else if path.is_ident("f32") || path.is_ident("f64") {
                    quote! { ::serde_json::json!({ "type": "number" }) }
                } else if path.is_ident("bool") {
                    quote! { ::serde_json::json!({ "type": "boolean" }) }
                } else if path.segments.len() == 1 && path.segments[0].ident == "Vec" {
                     if let syn::PathArguments::AngleBracketed(angle_args) = &path.segments[0].arguments {
                        if let Some(syn::GenericArgument::Type(inner_ty)) = angle_args.args.first() {
                            let items_schema = type_to_schema(inner_ty, struct_name);
                            return quote! { ::serde_json::json!({ "type": "array", "items": #items_schema }) };
                        }
                    }
                    quote! { compile_error!("Unsupported Vec inner type or Vec format") }
                } else {
                    let type_ident_str = quote!(#path).to_string().replace(' ', "");
                    let struct_name_str = struct_name.to_string();
                    if type_ident_str == struct_name_str {
                         quote! { compile_error!(concat!("Recursive type definition for schema not supported directly for type: ", #type_ident_str)) }
                    } else {
                         quote! { #path::mcp_input_schema() }
                    }
                }
            } else {
                quote! { compile_error!("Unsupported qualified type path (e.g. <T as Trait>::Type)") }
            }
        }
        _ => {
            let error_msg = format!("Unsupported field type for ToolArguments schema generation: {:?}", ty);
            quote! { compile_error!(#error_msg) }
        }
    }
}

#[proc_macro_derive(ToolArguments, attributes(tool_arg))]
pub fn tool_arguments_derive(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;

    let fields = match &input.data {
        Data::Struct(s) => match &s.fields {
            Fields::Named(named_fields) => &named_fields.named,
            _ => {
                return TokenStream::from(quote! {
                    compile_error!("ToolArguments can only be derived for structs with named fields.");
                });
            }
        },
        _ => {
            return TokenStream::from(quote! {
                compile_error!("ToolArguments can only be derived for structs.");
            });
        }
    };

    let mut properties_map_inserts = Vec::new();
    let mut required_field_names = Vec::new();
    let mut compile_errors = TokenStream2::new();

    for field in fields {
        let field_name_ident = field.ident.as_ref().unwrap();
        let field_type = &field.ty;

        match parse_field_attributes(field) {
            Ok(field_attrs) => {
                if field_attrs.skip {
                    continue;
                }

                let actual_field_name_str = field_attrs.rename.clone().unwrap_or_else(|| field_name_ident.to_string());

                let base_schema_ts = type_to_schema(field_type, name);

                let property_schema_ts = if let Some(desc_str) = &field_attrs.desc {
                    quote! {
                        {
                            let mut schema = #base_schema_ts;
                            if let Some(obj) = schema.as_object_mut() {
                                obj.insert("description".to_string(), ::serde_json::json!(#desc_str));
                            }
                            schema
                        }
                    }
                } else {
                    base_schema_ts
                };

                properties_map_inserts.push(quote! {
                    map.insert(#actual_field_name_str.to_string(), #property_schema_ts);
                });

                let is_field_required = field_attrs.required.unwrap_or(!is_option(field_type));
                if is_field_required {
                    required_field_names.push(quote! { #actual_field_name_str.to_string() });
                }
            }
            Err(err) => {
                compile_errors.extend(err.to_compile_error());
            }
        }
    }

    if !compile_errors.is_empty() {
        return TokenStream::from(compile_errors);
    }

    let expanded = quote! {
        impl #name {
            pub fn mcp_input_schema() -> ::serde_json::Value {
                static SCHEMA: ::once_cell::sync::Lazy<::serde_json::Value> = ::once_cell::sync::Lazy::new(|| {
                    let mut map = ::serde_json::Map::new();
                    #(#properties_map_inserts)*

                    let mut schema_obj = ::serde_json::json!({
                        "type": "object",
                        "properties": map,
                    });

                    let required_arr: Vec<String> = vec![ #(#required_field_names),* ];
                    if !required_arr.is_empty() {
                       schema_obj.as_object_mut().unwrap().insert("required".to_string(), ::serde_json::json!(required_arr));
                    }
                    schema_obj
                });
                SCHEMA.clone()
            }
        }

        impl ::mcp_sdk::ToolArgumentsDescriptor for #name {
            fn mcp_input_schema() -> ::serde_json::Value {
                Self::mcp_input_schema()
            }
        }
    };

    TokenStream::from(expanded)
}
