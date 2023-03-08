# iter_mod

This crate provides an macro that generates an array of the static and constant items in a given module. To demonstrate with a slightly modified [example from the reference](https://doc.rust-lang.org/reference/items/constant-items.html):

```rust
#[iter_mod::make_items]
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
    use example::{STATICS, CONSTS}; 
    use example::Item; 
    assert_eq!(CONSTS.len(), 5);
    assert_eq!(STATICS.len(), 0);

    let uints = CONSTS.iter().filter(|(_, b)| matches!(b, Item::U32(_))).count();
    assert_eq!(uints, 2);

    let uints = example::CONSTS.iter().filter(|(_, b)| matches!(b, Item::U32_2(_))).count();
    assert_eq!(uints, 1);

    let uints = example::CONSTS.iter().filter(|(_, b)| matches!(b, Item::BitsNStrings(_))).count();
    assert_eq!(uints, 1);

    assert_eq!("STRING", CONSTS[3].0);

    let example::Item::StrRef(s) = CONSTS[3].1 else { panic!() };
    assert_eq!("bitstring", s);
}
```
