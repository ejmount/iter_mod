use proc_macro::TokenStream;
use quote::{format_ident, ToTokens};
use structmeta::StructMeta;
use syn::{parse_quote, Expr, Ident, Item, ItemMod, LitStr, Type};

#[derive(StructMeta)]
struct Attributes {
    name: Option<LitStr>,
}

fn append_iterator(items: &mut Vec<Item>, name: Ident) {
    let mut item_names = vec![];
    let mut item_types = vec![];
    let mut item_exprs = vec![];
    for item in items.iter() {
        let ident = match item {
            Item::Const(konst) => &konst.ident,
            Item::Static(statik) => &statik.ident,
            _ => {
                continue;
            }
        };
        let typ = match item {
            Item::Const(konst) => &konst.ty,
            Item::Static(statik) => &statik.ty,
            _ => {
                continue;
            }
        };
        let type_ident = name_of_type(typ);
        let expr: Expr = parse_quote!(&(#ident));
        item_exprs.push(expr);
        item_names.push(ident);
        item_types.push(type_ident);
    }
    let count = item_names.len();
    let name_strs: Vec<_> = item_names.into_iter().map(syn::Ident::to_string).collect();

    let strukt = parse_quote! {
        pub(super) struct ModIterator<T:'static> {
            NAMES: [&'static str; #count],
            VALS: [&'static dyn core::any::Any; #count],
            _phantom: std::marker::PhantomData<T>,
            index: usize,
        }
    };

    let imp = parse_quote! {
        impl<T: 'static> ModIterator<T> {
            const fn new() -> ModIterator<T> {
                ModIterator {
                    NAMES: [#(#name_strs,)*],
                    VALS: [#(#item_exprs,)*],
                    _phantom: std::marker::PhantomData,
                    index: 0,
                }
            }
            pub(super) const  fn next(&mut self) -> Option<(&'static str, &'static T)> {
                while self.index < #count {
                    let val: &'static T = unsafe { std::mem::transmute(self.VALS[self.index])  };
                    let item = (self.NAMES[self.index], val);
                    self.index += 1;
                    return Some(item);

                }
                //else {
                    return None;
                //}
            }
        }
    };

    let iterator_impl = parse_quote! {
        impl<T> Iterator for ModIterator<T> {
            type Item =  (&'static str, &'static T);

            fn next(&mut self) -> Option<(&'static str, &'static T)> {
                self.next()
            }
        }
    };

    let fn_body = parse_quote! {
        pub(super) const fn #name <T:'static>() -> ModIterator<T> {
            ModIterator::new()
        }
    };

    items.push(Item::Struct(strukt));
    items.push(Item::Impl(imp));
    items.push(Item::Impl(iterator_impl));
    items.push(Item::Fn(fn_body));
}

fn name_of_type(typ: &Type) -> Ident {
    match typ {
        Type::Array(typ) => name_of_type(&typ.elem),
        Type::Path(typ) => typ
            .path
            .segments
            .last()
            .expect("Got empty path??")
            .ident
            .clone(),
        Type::Ptr(typ) => {
            let inner_name = name_of_type(&typ.elem);
            let mutt = if typ.mutability.is_some() { "Mut" } else { "" };
            format_ident!("{inner_name}{mutt}Ptr")
        }
        Type::Never(_) => format_ident!("Never"),
        Type::Tuple(typ) => {
            let names: Vec<_> = typ
                .elems
                .iter()
                .map(name_of_type)
                .map(|i| i.to_string())
                .collect();
            format_ident!("{}", names.join(""))
        }
        _ => panic!("Unsupported type {:?}", typ.to_token_stream()),
    }
}

///
/// This macro generates a function inside the module it's applied to, with the provided name. (Defaulting to "iter")
/// The function returns an Iterator of name-reference tuples for each applicable const or static value in the module.
/// The module's values may be heterogenous, so the function takes a generic `T`, and returns only the values of the given type.
///
///
///
/// generic type parameter `T`, and returns an opaque `Iterator` of `(&'static str, &'static T)` pairs, one for each
/// module member of type `T`. The provided type must be concrete, i.e. not a trait, and match the member exactly - no
/// conversions via `From`, `Deref`, `Borrow`, etc are available.
/// An additional caveat is that there is currently limited support for managing privacy - the generated function is marked
/// `pub(super)`, and all members of the given type are returned, including any that may not otherwise be visible to the caller.
///
///

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
