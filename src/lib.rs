//! A crate for deriving the [`Deref`](https://doc.rust-lang.org/std/ops/trait.Deref.html)
//! and [`DerefMut`](https://doc.rust-lang.org/std/ops/trait.DerefMut.html) 
//! traits from the standard library onto structs with at least one field. 
//! Fields with references are passed directly.
//!
//! # Examples
//!
//! ```rust
//! use derived_deref::{Deref, DerefMut};
//!
//! #[derive(Deref, DerefMut)]
//! struct StringWithCount {
//!     // Annotation of `#[target]` is required when there are two+ fields.
//!     #[target] inner: String,
//!     count: usize,
//! }
//! 
//!
//! // When there is only one field, annotation is optional instead.
//!
//! #[derive(Deref, DerefMut)]
//! struct StringWrapper(String);
//!
//! #[derive(Deref, DerefMut)]
//! struct CountWrapper(#[target] usize);
//! ```

extern crate proc_macro;

use proc_macro::TokenStream;
use syn::{parse_macro_input, ItemStruct, Ident, Generics, Field, Fields, Index, Type, punctuated::Punctuated, token::Comma};
use quote::quote;
use proc_macro2::TokenStream as TokenStream2;

/// Derives the [`Deref`](https://doc.rust-lang.org/std/ops/trait.Deref.html)
/// trait, passing the field directly if a reference type.
#[proc_macro_derive(Deref, attributes(target))]
pub fn derive_deref(input: TokenStream) -> TokenStream {
    // Creates the ItemStruct...
    let item_struct = parse_macro_input!(input as ItemStruct);
    let name = item_struct.ident;
    let generics = item_struct.generics;

    // ...to then get the desired field, one marked by `#[target]`.
    // However, if there's only one field, being marked is no longer required.
    match extract_field_parameters(item_struct.fields, "Deref") {
        Ok((field_name, field_type, is_mut_reference)) => impl_deref(name, generics, field_name, Some(field_type), is_mut_reference),
        Err(error) => error,
    }
}

/// Derives the [`DerefMut`](https://doc.rust-lang.org/std/ops/trait.DerefMut.html) 
/// trait, passing the field directly if a reference type. This will fail to
/// compile if the chosen field is an immutable reference type.
#[proc_macro_derive(DerefMut, attributes(target))]
// Deriving for `DerefMut` is the same as with `Deref` with the exception that
// `Target` does not need to be defined.
pub fn derive_deref_mut(input: TokenStream) -> TokenStream {
    let item_struct = parse_macro_input!(input as ItemStruct);
    let name = item_struct.ident;
    let generics = item_struct.generics;

    match extract_field_parameters(item_struct.fields, "DerefMut") {
        Ok((field_name, _, is_mut_reference)) => impl_deref(name, generics, field_name, None, is_mut_reference),
        Err(error) => error,
    }
}

// Acquires the only field or the marked field coupled with its index.
fn get_field(fields: Punctuated<Field, Comma>) -> Result<(usize, Field), TokenStream> {
    let attribute_name = "target";
    let error = || quote! { compile_error!("`#[target]` is required for one field"); }.into();
    
    let has_one_field = fields.len() == 1;
    let mut fields_iter = fields.into_iter().fuse().enumerate();
    
    if has_one_field {
        // An infallible call to take the first field.
        fields_iter.next().ok_or_else(error)
    } else {
        // Below filters for the fields marked correctly with `#[target]`.
        let mut fields_iter = fields_iter.filter(|(_, field)| {
            field.attrs.iter().any(|attribute| {
                attribute.meta
                    .require_path_only()
                    .is_ok_and(|path| path.is_ident(attribute_name))
            })
        });

        // Takes the next element, only keeping the `Some` if the next take
        // is a `None`. This ensures there's only one field marked `#[target]`.
        fields_iter.next().filter(|_| {
            fields_iter
                .next()
                .is_none()
        })
        .ok_or_else(error)
    }
}

fn extract_field_parameters(fields: Fields, trait_name: &str) -> Result<(TokenStream2, Type, Option<bool>), TokenStream> {
    match fields {
        Fields::Named(fields) => {
            let (_, field) = get_field(fields.named)?;
            let field_name = field.ident.unwrap();
            let (field_type, is_mut_reference) = match field.ty {
                Type::Reference(reference_type) => (*reference_type.elem, Some(reference_type.mutability.is_some())),
                field_type => (field_type, None),
            };

            Ok((quote! { #field_name }, field_type, is_mut_reference))
        },
        Fields::Unnamed(fields) => {
            let (field_index, field) = get_field(fields.unnamed)?;
            let field_index = Index::from(field_index);
            let (field_type, is_mut_reference) = match field.ty {
                Type::Reference(reference_type) => (*reference_type.elem, Some(reference_type.mutability.is_some())),
                field_type => (field_type, None),
            };

            Ok((quote! { #field_index }, field_type, is_mut_reference))
        },
        Fields::Unit => {
            let error = &format!("unable to implement `{}` trait for struct of no fields", trait_name)[..];

            Err(quote! { compile_error!(#error); }.into())
        }
    }
}

fn impl_deref(
    struct_name: Ident,
    struct_generics: Generics,
    field_name: TokenStream2,
    // Only whenever there is no need for `field_type` does it mean `Deref` is 
    // being implemented with its mutable counterpart.
    field_type: Option<Type>,
    // For if the field is a reference: `Some` if it is and `None` otherwise.
    // The boolean is `true` when it is mutable and `false` otherwise.
    is_mut_reference: Option<bool>,
) -> TokenStream 
{
    let (impl_generics, type_generics, where_clause) = struct_generics.split_for_impl();

    match field_type {
        Some(field_type) => {
            // If not a reference, "&" is passed. If it is, nothing is instead. 
            let reference = is_mut_reference.map_or_else(|| Some(quote!(&)), |_| None);
            
            quote! {
                impl #impl_generics core::ops::Deref for #struct_name #type_generics #where_clause {
                    type Target = #field_type;

                    fn deref(&self) -> &Self::Target {
                        #reference self.#field_name
                    }
                }
            }
        },
        None => {
            let reference = match is_mut_reference {
                Some(true) => None,
                Some(false) => return quote! { compile_error!("`#[target]` is unable to be of an immutable reference"); }.into(),
                None => Some(quote!(&mut)),
            };
            
            quote! {
                impl #impl_generics core::ops::DerefMut for #struct_name #type_generics #where_clause {
                    fn deref_mut(&mut self) -> &mut Self::Target {
                        #reference self.#field_name
                    }
                }
            }
        },
    }
    .into()
}
