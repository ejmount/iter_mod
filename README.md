# iter_mod

This crate provides an macro that generates an array of the static and constant items in a given module. To demonstrate with a slightly modified [example from the reference](https://doc.rust-lang.org/reference/items/constant-items.html):

```rust
#[iter_mod::make_items]
mod example {
    const BIT1: u32 = 1 << 0;
    const BIT2: u32 = 1 << 1;

    const BITS: [u32; 2] = [BIT1, BIT2];
    const STRING: &'static str = "bitstring";
    static STATIC: &'static str = "static string";

    #[derive(Debug, PartialEq, Eq)]
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
    use example::{Item, CONSTS, STATICS};
    assert_eq!(CONSTS.len(), 5);
    assert_eq!(STATICS.len(), 1);

    let uints = CONSTS
        .iter()
        .filter(|(_, b)| matches!(b, Item::U32(_)))
        .count();
    assert_eq!(uints, 2);

    let pairs = CONSTS
        .iter()
        .filter(|(_, b)| matches!(b, Item::U32_2(_)))
        .count();
    assert_eq!(pairs, 1);

    let (_, Item::BitsNStrings(struct_value)) = CONSTS
        .iter()
        .find(|(_, b)| matches!(b, Item::BitsNStrings(_)))
        .unwrap()
    else {
        unreachable!()
    };
    assert_eq!(
        *struct_value,
        example::BitsNStrings {
            mybits: [1, 2],
            mystring: "bitstring"
        }
    );

    assert_eq!(CONSTS[3].0, "STRING");

    let (_, Item::StrRef(s)) = CONSTS[3] else {
        unreachable!()
    };
    assert_eq!(s, "bitstring");
}
```
