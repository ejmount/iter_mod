use std::collections::HashMap;

use proc_macro::TokenStream;
use quote::{format_ident, ToTokens};
use syn::{parse_quote, Expr, Ident, Item, ItemConst, ItemMod, ItemStatic, Type};

#[derive(Copy, Clone, PartialEq, Eq)]
enum ItemType {
    Static,
    Const,
}
use ItemType::*;

#[derive(Clone)]
struct MetaType {
    // The name of the type of the item
    name: Ident,
    // The item's type expression
    typ: Type,
    // The syntheszed short name of the item's type
    type_name: Ident,
    // Static or const
    item_type: ItemType,
}

/// Given a module, append the arrays of consts and statics to it
fn append_meta_arrays(items: &mut Vec<Item>) {
    let item_exprs: Vec<_> = items.iter().filter_map(get_metatype_for_item).collect();

    let type_set: HashMap<_, _> = item_exprs
        .iter()
        .map(|mt| (&mt.type_name, (&mt.typ, &mt.item_type)))
        .collect();

    let filled_variants = type_set
        .into_iter()
        .map(|(name, (typ, item_type))| -> syn::Variant {
            let description = typ.to_token_stream().to_string();
            match item_type {
                Const => parse_quote! {
                    #[doc = #description]
                    #name(#typ)
                },
                Static => parse_quote! {
                    #[doc = #description]
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

    let (consts, statics): (Vec<_>, _) =
        item_exprs.into_iter().partition(|mt| mt.item_type == Const);

    let const_count = consts.len();

    let consts_values = consts.into_iter().map(create_expression_for_item);
    let static_values = statics.into_iter().map(create_expression_for_item);

    let cons = parse_quote! {
        pub const CONSTS: [(&'static str, Item); #const_count] = [#(#consts_values,)*];
    };
    let statik = parse_quote! {
        pub static STATICS: &[(&'static str, Item)] = &[#(#static_values,)*];
    };

    items.extend([cons, statik, filled_enum]);
}

fn create_expression_for_item(mt: MetaType) -> syn::Expr {
    let MetaType {
        name,
        type_name,
        item_type,
        ..
    } = mt;
    let value_expr: Expr = match item_type {
        ItemType::Const => parse_quote!(#name),
        ItemType::Static => parse_quote!(&(#name)),
    };
    let name = name.to_string();
    parse_quote! {
        (#name, Item::#type_name(#value_expr))
    }
}

fn get_metatype_for_item(expr: &Item) -> Option<MetaType> {
    let (name, typ, item_type) = match expr {
        Item::Const(ItemConst { ident, ty, .. }) => (ident, &**ty, Const),
        Item::Static(ItemStatic { ident, ty, .. }) => (ident, &**ty, Static),
        _ => return None,
    };
    let type_name = match item_type {
        Static => {
            let ref_type = parse_quote! { &'static #typ };
            name_of_type(&ref_type)
        }
        Const => name_of_type(typ),
    };
    let name = name.clone();
    let typ = typ.clone();

    MetaType {
        name,
        typ,
        type_name,
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
        // Dealing with generics might be tricky because lifetimes need to be ignored
        Type::Ptr(typ) => {
            let inner_name = name_of_type(&typ.elem);
            let mutt = if typ.mutability.is_some() { "Mut" } else { "" };
            format_ident!("{inner_name}{mutt}Ptr")
        }

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
        Type::Slice(slice) => format_ident!("{}Slice", name_of_type(&slice.elem)),
        Type::Reference(r) => format_ident!("{}Ref", name_of_type(&r.elem)),
        Type::Never(_) => unimplemented!("How did you make a Never item"),

        Type::Group(_)
        | Type::ImplTrait(_)
        | Type::Infer(_)
        | Type::Macro(_)
        | Type::Paren(_)
        | Type::TraitObject(_)
        | Type::BareFn(_)
        | Type::Verbatim(_) => {
            unimplemented!("Unsupported type {:?}", typ.to_token_stream())
        }
        _ => unimplemented!("Unsupported future type {:?}", typ.to_token_stream()),
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
///   ** Types differ only in generic parameters
///   ** Multiple types with the same base name are imported from different modules
/// * Complex expressions for an array's length may be incorrectly interpreted - define a new constant to avoid this
///   ** Additionally, literal numbers used as an array length are embedded in the name of an enum variant, meaning changing the value is a breaking change
#[proc_macro_attribute]
pub fn make_items(_attr: TokenStream, module: TokenStream) -> TokenStream {
    let mut output = TokenStream::new().into();
    let Ok(mut module): Result<ItemMod, _> = syn::parse(module) else {
        panic!("Item is not a module")
    };

    let Some((_, items)) = &mut module.content else {
        panic!("Can't apply this macro to inline modules")
    };

    append_meta_arrays(items);
    module.to_tokens(&mut output);

    output.into()
}

/// This runs the doctests in the readme
#[cfg(doctest)]
#[doc(hidden)]
#[doc = include_str!("../readme.md")]
struct ReadMeTest;
