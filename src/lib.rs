use std::collections::HashMap;

use proc_macro::TokenStream;
use quote::{format_ident, ToTokens};
use syn::{parse_quote, Expr, Ident, Item, ItemMod, Lifetime, Type};

#[derive(Clone, PartialEq, Eq)]
enum ItemType {
    Static,
    Const,
}

#[derive(Clone)]
struct MetaType {
    name: Ident,
    typ: Type,
    type_name: Ident,
    expr: Expr,
    item_type: ItemType,
}

/// Given a module, append the arrays of consts and statics to it
fn append_iterator(items: &mut Vec<Item>) {
    let item_exprs: Vec<_> = items.iter().filter_map(get_metatype_for_item).collect();

    let const_count = item_exprs
        .iter()
        .filter(|mt| mt.item_type == ItemType::Const)
        .count();

    let type_set: HashMap<_, _> = item_exprs
        .iter()
        .cloned()
        .map(|mt| (mt.type_name, (mt.typ, mt.item_type)))
        .collect();

    let filled_variants = type_set
        .into_iter()
        .map(|(name, (typ, item_type))| -> syn::Variant {
            let typ = if let Type::Reference(mut typ) = typ {
                let span = typ.and_token.span;
                typ.lifetime = Some(Lifetime::new("'static", span));
                Type::Reference(typ)
            } else {
                typ
            };

            match item_type {
                ItemType::Const => parse_quote! {
                    #name(#typ)
                },
                ItemType::Static => parse_quote! {
                    #name(&'static #typ)
                },
            }
        });

    let filled_enum = parse_quote! {
        #[non_exhaustive]
        pub enum Item {
            #(#filled_variants,)*
        }
    };

    let (consts, statics): (Vec<_>, _) = item_exprs
        .into_iter()
        .partition(|mt| mt.item_type == ItemType::Const);

    let consts_values = consts.into_iter().map(create_item_reference);
    let static_values = statics.into_iter().map(create_item_reference);

    let cons = parse_quote! {
        pub const CONSTS: [(&'static str, Item); #const_count] = [#(#consts_values,)*];
    };
    let statik = parse_quote! {
        pub static STATICS: &[(&'static str, Item)] = &[#(#static_values,)*];
    };

    items.push(cons);
    items.push(statik);
    items.push(filled_enum);
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
    let (name, typ, item_type) = match expr {
        Item::Const(expr) => (&expr.ident, *expr.ty.clone(), ItemType::Const),
        Item::Static(expr) => (&expr.ident, *expr.ty.clone(), ItemType::Static),
        _ => return None,
    };
    let type_name = name_of_type(&typ);
    let expr: Expr = match expr {
        Item::Const(_) => parse_quote!(#name),
        Item::Static(_) => parse_quote!(&(#name)),
        _ => unreachable!(),
    };
    MetaType {
        name: name.clone(),
        typ,
        type_name,
        expr,
        item_type,
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

/// This attribute generates three additional items in the module it is applied to:
/// * An enum called `Item`, which has a variant for each unique type among the constant and static items in the module. It is marked `[non_exhaustive]` so that adding items in the future is not breaking.
/// * A const array called `CONSTS`, consisting of pairs of a `&'static str` denoting the name of the constant, and a `Item` instance containing the value. The values are in source order.
/// * A static array called `STATICS`, which is similar to `CONSTS`, except that the `Item` instances contain *references* to the corresponding static value.
///
/// This currently has several caveats:
/// * Not all possible types are supported - if you have a usecase that's not supported, please file a bug
/// * Faulty output may result from distinct types' base name being the same - use type aliases to distinguish the type names as seen by the macro. This can occur when:
/// ** Types differ only in generic parameters
/// ** Multiple types with the same base name are imported from different modules
/// * Complex expressions for an array's length may be incorrectly interpreted - define a new constant to avoid this
/// ** Additionally, literal numbers used as an array length are embedded in the name of an enum variant, meaning changing the value is a breaking change
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
