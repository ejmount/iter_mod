# iter_mod

This crate provides an macro that allows for iterating over the constant and static members of a module, by both name and value. To demonstrate with a slightly modified example from [the reference](https://doc.rust-lang.org/reference/items/constant-items.html):

```rust
#[iter_mod::make_iter(name="iter")]
mod example {
    const BIT1: u32 = 1 << 0;
    const BIT2: u32 = 1 << 1;

    const BITS: [u32; 2] = [BIT1, BIT2];
    const STRING: &'static str = "bitstring";

    pub struct BitsNStrings<'a> {
        pub mybits: [u32; 2],
        pub mystring: &'a str,
    }

    const BITS_N_STRINGS: BitsNStrings<'static> = BitsNStrings {
        mybits: BITS,
        mystring: STRING,
    };
}

fn main() {
    let numbers = example::iter::<u32>();
    assert_eq!(numbers.count(), 2);
    let mut structs = example::iter::<example::BitsNStrings>();
    let (name, value) = structs.next().unwrap();
    assert_eq!(name, "BITS_N_STRINGS");
    assert_eq!(value.mybits, [1, 2]);
}
```

When a module is tagged with the `make_iter` macro, a function will be generated within the module with the provided name. (The name is optional and defaults to `"iter"`) Because the members may be of heterogenous types, this function accepts a generic type parameter `T: 'static`, and returns an opaque `Iterator` of `(&'static str, &'static T)` pairs, one for each module member of type `T`. The provided type must be concrete, i.e. not a trait, and match the member exactly - no conversions via `From`, `Deref`, `Borrow`, etc are available.

An additional caveat is that there is currently limited support for managing privacy - the generated function is marked `pub(super)`, and all matching members are returned, including any that may not otherwise be visible to the caller.
