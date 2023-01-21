use proc_macro::TokenStream;
use quote::{format_ident, ToTokens};
use structmeta::StructMeta;
use syn::{parse_quote, Expr, Ident, Item, ItemFn, ItemMod, LitStr};

#[derive(StructMeta)]
struct Attributes {
    name: Option<LitStr>,
}

fn append_iterator(items: &mut Vec<Item>, name: Ident) {
    let mut item_names = vec![];
    let mut item_exprs = vec![];
    for item in items.iter() {
        let ident = match item {
            Item::Const(konst) => &konst.ident,
            Item::Static(statik) => &statik.ident,
            _ => {
                continue;
            }
        };
        let expr: Expr = parse_quote!(&(#ident));
        item_exprs.push(expr);
        item_names.push(ident);
    }
    let count = item_names.len();
    let name_strs: Vec<_> = item_names.into_iter().map(syn::Ident::to_string).collect();

    let fn_body: ItemFn = parse_quote! {
        pub(super) fn #name <T:'static>() -> impl Iterator<Item = (&'static str, &'static T)> {
            let NAMES: [&'static str; #count] = [#(#name_strs,)*];
            let VALS: [&'static dyn core::any::Any; #count] = [#(#item_exprs,)*];
            NAMES.into_iter().zip(VALS.into_iter()).filter_map(|(name,any)|
            if let Some(val) = any.downcast_ref() {
                Some((name, val))
            }
            else { None }
            )
        }
    };
    items.push(Item::Fn(fn_body));
}

#[proc_macro_attribute]
pub fn make_iter(attr: TokenStream, module: TokenStream) -> TokenStream {
    let args: Attributes = syn::parse(attr).unwrap();

    let name = args
        .name
        .as_ref()
        .map(LitStr::value)
        .unwrap_or("iter".to_owned());
    let name = format_ident!("{name}");

    let mut output = TokenStream::new().into();

    if let Ok(
        m @ ItemMod {
            content: Some((_, _)),
            ..
        },
    ) = &mut syn::parse(module)
    {
        append_iterator(&mut m.content.as_mut().unwrap().1, name);
        m.to_tokens(&mut output);
    } else {
        panic!("Could not parse item as module");
    }

    output.into()
}

#[cfg(doctest)]
#[doc(hidden)]
#[doc = include_str!("../readme.md")]
struct ReadMeTest;
