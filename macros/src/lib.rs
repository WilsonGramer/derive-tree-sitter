use darling::{FromDeriveInput, FromField, FromVariant};
use proc_macro::TokenStream;
use quote::quote;
use syn::{DeriveInput, Generics, TypeParamBound, WhereClause, parse_macro_input};

#[derive(FromDeriveInput)]
#[darling(attributes(tree_sitter))]
struct Options {
    ident: syn::Ident,
    generics: Generics,
    data: darling::ast::Data<VariantOptions, FieldOptions>,
}

#[derive(FromField)]
#[darling(attributes(tree_sitter))]
struct FieldOptions {
    ident: Option<syn::Ident>,
    ty: syn::Type,
    #[darling(default)]
    rule: Option<String>,
}

#[derive(FromVariant)]
#[darling(attributes(tree_sitter))]
struct VariantOptions {
    ident: syn::Ident,
    #[darling(default)]
    rule: Option<String>,
}

#[proc_macro_derive(FromNode, attributes(tree_sitter))]
pub fn derive_from_tree_sitter(input: TokenStream) -> TokenStream {
    let derive_input = parse_macro_input!(input as DeriveInput);

    let options = match Options::from_derive_input(&derive_input) {
        Ok(options) => options,
        Err(err) => return err.write_errors().into(),
    };

    let ident = options.ident;

    let mut generics = options.generics.clone();
    add_bounds(&mut generics);
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    let body = match options.data {
        darling::ast::Data::Struct(data) => {
            let field_inits = data
                .fields
                .iter()
                .map(|field| {
                    let ident = field.ident.as_ref().unwrap();
                    let name = ident.to_string().trim_start_matches("r#").to_string();
                    let rule = field.rule.as_deref();

                    let ty_ident = match &field.ty {
                        syn::Type::Path(ty) => ty
                            .path
                            .segments
                            .last()
                            .map(|segment| segment.ident.to_string()),
                        _ => None,
                    };

                    let method = match ty_ident.as_deref() {
                        Some("Range") => quote!(node.span()),
                        Some("String") => quote!(node.slice()),
                        Some("Option") => {
                            let rule = rule.unwrap_or(name.as_str());
                            quote!(node.try_child(#rule))
                        }
                        Some("Vec") => {
                            if let Some(rule) = rule {
                                quote!(node.children(#rule))
                            } else if name.ends_with('s') {
                                let rule = &name.to_string()[..(name.len() - 1)];
                                quote!(node.children(#rule))
                            } else {
                                syn::Error::new_spanned(
                                    &field.ident,
                                    "`Vec` fields must end with 's' or specify a rule",
                                )
                                .to_compile_error()
                            }
                        }
                        Some("bool") => {
                            let rule = rule.unwrap_or(name.as_str());
                            quote!(node.has_child(#rule))
                        }
                        _ => {
                            let rule = rule.unwrap_or(name.as_str());
                            quote!(node.child(#rule))
                        }
                    };

                    quote! { #ident: #method }
                })
                .collect::<Vec<_>>();

            quote! {
                #ident {
                    #(#field_inits,)*
                }
            }
        }
        darling::ast::Data::Enum(data) => {
            let arms = data.iter().map(|variant| {
                let variant_ident = &variant.ident;

                match variant.rule.as_deref() {
                    Some(rule) => {
                        quote! {
                            #rule => #ident::#variant_ident(
                                ::derive_tree_sitter::FromNode::from_node(node),
                            ),
                        }
                    }
                    None => {
                        let error = syn::Error::new_spanned(
                            &variant.ident,
                            "enum variants must specify `#[tree_sitter(rule = \"...\")]`",
                        )
                        .to_compile_error();

                        quote! {
                            _ => #error,
                        }
                    }
                }
            });

            quote! {
                match node.kind() {
                    #(#arms)*
                    other => panic!("cannot convert node '{}' to `{}`", other, stringify!(#ident)),
                }
            }
        }
    };

    quote! {
        impl #impl_generics ::derive_tree_sitter::FromNode for #ident #ty_generics
        #where_clause
        {
            fn from_node(node: &mut ::derive_tree_sitter::Node<'_, '_>) -> Self {
                #body
            }
        }
    }
    .into()
}

fn add_bounds(generics: &mut Generics) {
    let where_clause: &mut WhereClause = generics.where_clause.get_or_insert_with(|| WhereClause {
        where_token: Default::default(),
        predicates: Default::default(),
    });

    for param in &generics.params {
        if let syn::GenericParam::Type(param) = param {
            let ident = &param.ident;
            let bound: TypeParamBound = syn::parse_quote!(::derive_tree_sitter::FromNode);
            let predicate: syn::WherePredicate = syn::parse_quote!(#ident: #bound);
            where_clause.predicates.push(predicate);
        }
    }
}
