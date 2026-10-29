use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{Data, DeriveInput, Fields, GenericParam, parse_macro_input, parse_quote};

#[proc_macro_derive(Record, attributes(record))]
pub fn derive_record(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    match expand(input) {
        Ok(output) => output.into(),
        Err(error) => error.to_compile_error().into(),
    }
}

fn skipped(field: &syn::Field) -> syn::Result<bool> {
    let mut skip = false;
    for attribute in &field.attrs {
        if attribute.path().is_ident("record") {
            attribute.parse_nested_meta(|meta| {
                if meta.path.is_ident("skip") {
                    skip = true;
                    Ok(())
                } else {
                    Err(meta.error("expected record(skip) for derived, nonsemantic state"))
                }
            })?;
        }
    }
    Ok(skip)
}

fn fields(
    fields: &Fields,
) -> syn::Result<(
    Vec<proc_macro2::TokenStream>,
    Vec<proc_macro2::TokenStream>,
    Vec<proc_macro2::TokenStream>,
)> {
    let mut bindings = Vec::new();
    let mut writes = Vec::new();
    let mut reads = Vec::new();
    for (position, field) in fields.iter().enumerate() {
        let binding = format_ident!("value_{}", position);
        let skip = skipped(field)?;
        let pattern = if skip { quote!(_) } else { quote!(#binding) };
        let read = if skip {
            quote!(::core::default::Default::default())
        } else {
            writes.push(quote!(::compiler_storage::Record::write(#binding, output)?;));
            quote!(::compiler_storage::Record::read(input)?)
        };
        if let Some(name) = &field.ident {
            bindings.push(quote!(#name: #pattern));
            reads.push(quote!(#name: #read));
        } else {
            bindings.push(pattern);
            reads.push(read);
        }
    }
    Ok((bindings, writes, reads))
}

fn shape(fields: &Fields, values: &[proc_macro2::TokenStream]) -> proc_macro2::TokenStream {
    match fields {
        Fields::Named(_) => quote!({ #(#values),* }),
        Fields::Unnamed(_) => quote!((#(#values),*)),
        Fields::Unit => quote!(),
    }
}

fn expand(mut input: DeriveInput) -> syn::Result<proc_macro2::TokenStream> {
    let name = &input.ident;
    for parameter in &mut input.generics.params {
        if let GenericParam::Type(parameter) = parameter {
            parameter
                .bounds
                .push(parse_quote!(::compiler_storage::Record));
        }
    }
    let (implementation, arguments, conditions) = input.generics.split_for_impl();
    let (write, read) = match &input.data {
        Data::Struct(data) => {
            let (bindings, writes, reads) = fields(&data.fields)?;
            let binding = shape(&data.fields, &bindings);
            let construction = shape(&data.fields, &reads);
            (
                quote!(let Self #binding = self; #(#writes)* Ok(())),
                quote!(Ok(Self #construction)),
            )
        }
        Data::Enum(data) => {
            let mut writes = Vec::new();
            let mut reads = Vec::new();
            for (position, variant) in data.variants.iter().enumerate() {
                let tag = position as u32;
                let variant_name = &variant.ident;
                let (bindings, fields_write, fields_read) = fields(&variant.fields)?;
                let binding = shape(&variant.fields, &bindings);
                let construction = shape(&variant.fields, &fields_read);
                writes.push(quote!(Self::#variant_name #binding => { ::compiler_storage::Record::write(&#tag, output)?; #(#fields_write)* Ok(()) }));
                reads.push(quote!(#tag => Ok(Self::#variant_name #construction)));
            }
            (
                quote!(match self { #(#writes),* }),
                quote!(match <u32 as ::compiler_storage::Record>::read(input)? { #(#reads,)* _ => Err(input.error(concat!("unknown ", stringify!(#name), " variant"))) }),
            )
        }
        Data::Union(_) => {
            return Err(syn::Error::new_spanned(
                name,
                "compiler records do not store Rust unions",
            ));
        }
    };
    Ok(quote! {
        impl #implementation ::compiler_storage::Record for #name #arguments #conditions {
            fn write(&self, output: &mut ::compiler_storage::RecordWriter) -> Result<(), ::compiler_storage::RecordError> { output.record(|output| { #write }) }
            fn read<__CompilerRecordContext>(input: &mut ::compiler_storage::RecordReader<'_, __CompilerRecordContext>) -> Result<Self, ::compiler_storage::RecordError> { input.record(|input| { #read }) }
        }
    })
}
