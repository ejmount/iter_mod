use std::collections::HashMap;

use proc_macro::TokenStream;
use quote::{format_ident, ToTokens};
use syn::{parse_quote, Expr, Ident, Item, ItemMod, Type};

#[derive(Clone)]
struct MetaType {
    name: Ident,
    typ: Type,
    type_name: Ident,
    expr: Expr,
}

/// Given a module, append the array of item references to it
fn append_iterator(items: &mut Vec<Item>) {
    let item_exprs: Vec<_> = items.iter().filter_map(get_metatype_for_item).collect();

    let type_set: HashMap<_, _> = item_exprs
        .iter()
        .cloned()
        .map(|mt| (mt.type_name, mt.typ))
        .collect();

    let filled_variants = type_set.into_iter().map(|(name, typ)| -> syn::Variant {
        parse_quote! {
            #name(&'static (#typ))
        }
    });

    let filled_enum: syn::ItemEnum = parse_quote! {
        #[non_exhaustive]
        pub enum Item {
            #(#filled_variants,)*
        }
    };

    let item_values = item_exprs.into_iter().map(create_item_reference);

    let list: syn::ItemStatic = parse_quote! {
        pub static ITEMS: &[(&'static str, Item)] = &[#(#item_values,)*];
    };

    items.push(Item::Static(list));
    items.push(Item::Enum(filled_enum));
}

fn create_item_reference(mt: MetaType) -> syn::Expr {
    let MetaType {
        name,
        expr,
        type_name,
        ..
    } = mt;
    let name = name.to_string();
    parse_quote! {
        (#name, Item::#type_name(#expr))
    }
}

fn get_metatype_for_item(expr: &Item) -> Option<MetaType> {
    let (name, typ) = match expr {
        Item::Const(expr) => (&expr.ident, &expr.ty),
        Item::Static(expr) => (&expr.ident, &expr.ty),
        _ => return None,
    };
    let type_name = name_of_type(typ);
    let expr: Expr = parse_quote!(&(#name));
    MetaType {
        name: name.clone(),
        typ: *typ.clone(),
        type_name,
        expr,
    }
    .into()
}

/// Given a type, constructs a name suitable for an enum variant representing it.
/// For ADTs, this is called recursively to build up, e.g. the name of a tuple from component names
/// Uppercases the return value to avoid warnings from rustfmt
/// TODO: Improve that
fn name_of_type(typ: &Type) -> Ident {
    let name = match typ {
        Type::Array(typ) => {
            let count = typ.len.to_token_stream().to_string();
            // Not great, liable to break if something complicated is used for the length
            format_ident!("{}_{}", name_of_type(&typ.elem), count)
        }
        Type::Path(typ) => typ
            .path
            .segments
            .last()
            .expect("Got empty path??")
            .ident
            .clone(),
        // Get the last segment of the path as the name
        // Might cause problems if multiple types in scope have the same name
        // TODO: Also doesnt deal with generics at all
        Type::Ptr(typ) => {
            let inner_name = name_of_type(&typ.elem);
            let mutt = if typ.mutability.is_some() { "Mut" } else { "" };
            format_ident!("{inner_name}{mutt}Ptr")
        }
        Type::Never(_) => format_ident!("Never"), // ! also isn't a valid variant name
        Type::Tuple(typ) => {
            if typ.elems.is_empty() {
                format_ident!("Unit") // () is not a valid identifier for enum variants
            } else {
                let names: Vec<_> = typ
                    .elems
                    .iter()
                    .map(name_of_type)
                    .map(|i| i.to_string())
                    .collect();
                format_ident!("{}", names.join(""))
            }
        }
        Type::Reference(r) => format_ident!("{}Ref", name_of_type(&r.elem)),
        _ => panic!("Unsupported type {:?}", typ.to_token_stream(),),
    };
    let mut name_str = name.to_string();
    let (begin, _) = name_str.split_at_mut(1);
    begin.make_ascii_uppercase();
    Ident::new(&name_str, name.span())
}

/// This attribute generates two additional items in the module it is applied to:
/// * An enum called `Item`, which has a variant for each unique type among the constant and static items in the module. It is marked `[non_exhaustive]` so that adding items in the future is not breaking.
/// * An array called `ITEMS`, consisting of pairs of a `&'static str` denoting the name of the item, and a `Item` instance containing a reference to the value. The values are in source order.
///
///
/// This currently has several caveats:
/// * Not all possible types are supported - if you have a usecase that's not supported, please file a bug
/// * Faulty output may result from the type's base name being the same - use type aliases to distinguish the type names as seen by the macro. This can occur when:
/// ** Types differ only in generic parameters
/// ** Multiple types with the same base name are imported from different modules
#[proc_macro_attribute]
pub fn make_items(_attr: TokenStream, module: TokenStream) -> TokenStream {
    let mut output = TokenStream::new().into();

    if let Ok(
        m @ ItemMod {
            content: Some((_, _)),
            ..
        },
    ) = &mut syn::parse(module)
    {
        append_iterator(&mut m.content.as_mut().unwrap().1);
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
