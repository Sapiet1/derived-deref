//! A crate for deriving the [`Deref`](https://doc.rust-lang.org/std/ops/trait.Deref.html)
//! and [`DerefMut`](https://doc.rust-lang.org/std/ops/trait.DerefMut.html) traits from the standard
//! library onto structs with at least one field. 
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
use syn::{parse_macro_input, ItemStruct, Field, Fields, FieldsNamed, FieldsUnnamed, Index};
use quote::quote;

#[proc_macro_derive(Deref, attributes(target))]
pub fn derive_deref(input: TokenStream) -> TokenStream {
    // Creates the ItemStruct...
    let item_struct = parse_macro_input!(input as ItemStruct);
    let name = item_struct.ident;
    let (impl_generics, type_generics, where_clause) = item_struct.generics.split_for_impl();

    // ...to then get the desired field, one marked by `#[target]`.
    // However, if there's only one field, being marked is no longer required.
    match item_struct.fields {
        Fields::Named(fields) => {
            let field = match get_field(fields) {
                Ok(field) => field,
                Err(error) => return error.into(),
            };

            let field_name = field.ident.unwrap();
            let field_type = field.ty;

            quote! {
                impl #impl_generics std::ops::Deref for #name #type_generics #where_clause {
                    type Target = #field_type;

                    fn deref(&self) -> &Self::Target {
                        &self.#field_name
                    }
                }
            }
            .into()
        },
        Fields::Unnamed(fields) => {
            let (field_index, field) = match get_field_with_index(fields) {
                Ok(field_with_index) => field_with_index,
                Err(error) => return error.into(),
            };

            let field_index = Index::from(field_index);
            let field_type = field.ty;

            quote! {
                impl #impl_generics std::ops::Deref for #name #type_generics #where_clause {
                    type Target = #field_type;

                    fn deref(&self) -> &Self::Target {
                        &self.#field_index
                    }
                }
            }
            .into()
        },
        Fields::Unit => {
            quote! {
                compile_error!("unable to implement `Deref` for structs of no fields");
            }
            .into()
        },
    }
}

#[proc_macro_derive(DerefMut, attributes(target))]
// Deriving for `DerefMut` is the same as with `Deref`.
pub fn derive_deref_mut(input: TokenStream) -> TokenStream {
    let item_struct = parse_macro_input!(input as ItemStruct);
    let name = item_struct.ident;
    let (impl_generics, type_generics, where_clause) = item_struct.generics.split_for_impl();

    match item_struct.fields {
        Fields::Named(fields) => {
            let field_name = match get_field(fields) {
                Ok(field) => field.ident.unwrap(),
                Err(error) => return error.into(),
            };

            quote! {
                impl #impl_generics std::ops::DerefMut for #name #type_generics #where_clause {
                    fn deref_mut(&mut self) -> &mut Self::Target {
                        &mut self.#field_name
                    }
                }
            }
            .into()
        },
        Fields::Unnamed(fields) => {
            let field_index = match get_field_with_index(fields) {
                Ok(field_with_index) => Index::from(field_with_index.0),
                Err(error) => return error.into(),
            };

            quote! {
                impl #impl_generics std::ops::DerefMut for #name #type_generics #where_clause {
                    fn deref_mut(&mut self) -> &mut Self::Target {
                        &mut self.#field_index
                    }
                }
            }
            .into()
        },
        Fields::Unit => {
            quote! {
                compile_error!("unable to implement `DerefMut` for structs of no fields");
            }
            .into()
        }
    }
}

const ATTRIBUTE: &str = "target";

// Acquires the only field or the marked field from the named fields.  
fn get_field(fields: FieldsNamed) -> Result<Field, AttributeError> {
    let has_one_field = fields.named.len() == 1;
    let mut fields_iter = fields.named
        .into_iter()
        .fuse()
        .filter(|field| {
            field.attrs.is_empty() ||
            field.attrs.iter().any(|attribute| {
                attribute.meta
                    .require_path_only()
                    .is_ok()
            })
        });
    
    if has_one_field {
        // The only case this returns `None` is when `#[target]` is given 
        // invalid input.
        fields_iter.next().ok_or(AttributeError::Invalid)
    } else {
        let mut fields_iter = fields_iter.filter(|field| {
            field.attrs.iter().any(|attribute| {
                // Because of the prior filter, it would be as if what was
                // written was:
                //     attribute.meta
                //         .require_path_only()
                //         .is_ok_and(|path| path.is_ident("target"))
                // This ensures `#[target]` takes no invalid inputs.
                attribute
                    .path()
                    .is_ident(ATTRIBUTE)
            })
        });

        // Takes the next element, only keeping the `Some` if the next take
        // is a `None`. This ensures there's only one field marked `#[target]`.
        fields_iter.next().and_then(|field| {
            fields_iter
                .next()
                .is_none()
                .then_some(field)
        })
        .ok_or(AttributeError::Required)
    }
}

// Is the same as the prior function, but instead also keeping track of the
// indexes of the fields as the input is now unnamed fields.
fn get_field_with_index(fields: FieldsUnnamed) -> Result<(usize, Field), AttributeError> {
    let has_one_field = fields.unnamed.len() == 1;
    let mut fields_iter = fields.unnamed
        .into_iter()
        .fuse()
        .enumerate()
        .filter(|(_, field)| {
            field.attrs.is_empty() ||
            field.attrs.iter().any(|attribute| {
                attribute.meta
                    .require_path_only()
                    .is_ok()
            })
        });

    if has_one_field {
        fields_iter.next().ok_or(AttributeError::Invalid)
    } else {
        let mut fields_iter = fields_iter.filter(|(_, field)| {
            field.attrs.iter().any(|attribute| {
                attribute
                    .path()
                    .is_ident(ATTRIBUTE)
                })
        });

        fields_iter.next().and_then(|field_with_index| {
            fields_iter
                .next()
                .is_none()
                .then_some(field_with_index)
        })
        .ok_or(AttributeError::Required)
    }
}

// The error to facilitate the differences of when there are invalid `#[target]`
// notation in structs with one and more fields.
enum AttributeError {
    Invalid,
    Required,
}

impl From<AttributeError> for TokenStream {
    fn from(value: AttributeError) -> Self {
        match value {
            AttributeError::Invalid => {
                quote! {
                    compile_error!("`#[target]` of invalid input exists");
                }
                .into()
            },
            AttributeError::Required => {
                quote! {
                    compile_error!("valid `#[target]` is required for one and only one field");
                }
                .into()
            },
        }
    }
}
