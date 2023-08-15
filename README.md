# derived-deref

A crate for deriving the [`Deref`](https://doc.rust-lang.org/std/ops/trait.Deref.html)
and [`DerefMut`](https://doc.rust-lang.org/std/ops/trait.DerefMut.html) 
traits from the standard library onto structs with at least one field. 
Fields with references are passed directly.

# Examples

```rust
use derived_deref::{Deref, DerefMut};

#[derive(Deref, DerefMut)]
struct StringWithCount {
    // Annotation of `#[target]` is required when there are two+ fields.
    #[target] inner: String,
    count: usize,
}


// When there is only one field, annotation is optional instead.

#[derive(Deref, DerefMut)]
struct StringWrapper(String);

#[derive(Deref, DerefMut)]
struct CountWrapper(#[target] usize);
```
