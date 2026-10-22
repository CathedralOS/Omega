//! Structural syntax equality must resolve retained names through both owners.
//! Deriving the walk keeps every field and enum variant in the comparison when
//! the representation grows; name leaves supply the contextual comparison.
use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{Data, DeriveInput, Fields, parse_macro_input};

#[proc_macro_derive(SyntaxEquivalent)]
pub fn derive_syntax_equivalent(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;
    let compare = |fields: &Fields, prefix: proc_macro2::TokenStream| {
        let left = fields
            .iter()
            .enumerate()
            .map(|(index, _)| format_ident!("left_{index}"))
            .collect::<Vec<_>>();
        let right = fields
            .iter()
            .enumerate()
            .map(|(index, _)| format_ident!("right_{index}"))
            .collect::<Vec<_>>();
        let pattern = |bindings: &[syn::Ident]| match fields {
            Fields::Named(fields) => {
                let names = fields.named.iter().map(|field| &field.ident);
                quote! { #prefix { #(#names: #bindings),* } }
            }
            Fields::Unnamed(_) => quote! { #prefix(#(#bindings),*) },
            Fields::Unit => quote! { #prefix },
        };
        let left_pattern = pattern(&left);
        let right_pattern = pattern(&right);
        quote! { (#left_pattern, #right_pattern) => true #(&& crate::SyntaxEquivalent::same_syntax(#left, #right, left_text, right_text))* }
    };
    let arms = match &input.data {
        Data::Struct(data) => vec![compare(&data.fields, quote! { Self })],
        Data::Enum(data) => data
            .variants
            .iter()
            .map(|variant| {
                let variant_name = &variant.ident;
                compare(&variant.fields, quote! { Self::#variant_name })
            })
            .collect(),
        Data::Union(_) => {
            return syn::Error::new_spanned(name, "syntax equality does not support unions")
                .to_compile_error()
                .into();
        }
    };
    let fallback = matches!(input.data, Data::Enum(_)).then(|| quote! { _ => false, });
    let (implementation, arguments, requirements) = input.generics.split_for_impl();
    quote! {
        impl #implementation crate::SyntaxEquivalent for #name #arguments #requirements {
            fn same_syntax(&self, other: &Self, left_text: &crate::SyntaxTextOwner, right_text: &crate::SyntaxTextOwner) -> bool {
                match (self, other) { #(#arms,)* #fallback }
            }
        }
    }.into()
}

#[proc_macro_derive(PlainSyntaxEquivalent)]
pub fn derive_plain_syntax_equivalent(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;
    let (implementation, arguments, requirements) = input.generics.split_for_impl();
    quote! {
        impl #implementation crate::SyntaxEquivalent for #name #arguments #requirements {
            fn same_syntax(&self, other: &Self, _: &crate::SyntaxTextOwner, _: &crate::SyntaxTextOwner) -> bool { self == other }
        }
    }.into()
}
