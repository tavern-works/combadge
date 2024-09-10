extern crate proc_macro;
use proc_macro::TokenStream;

use quote::{format_ident, quote};
use syn::{parse, parse_macro_input, FnArg, ItemTrait, TraitItem};

#[proc_macro_attribute]
pub fn combadge(attr: TokenStream, item: TokenStream) -> TokenStream {
    let mut item: ItemTrait = parse_macro_input!(item);

    let Some(TraitItem::Fn(constructor)) = item.items.iter().find(|item| match item {
        TraitItem::Fn(function) => function.sig.ident == "new",
        _ => false,
    }) else {
        panic!("Failed to find new function in {}", item.ident);
    };

    let constructor_args = &constructor.sig.inputs;

    if constructor_args
        .iter()
        .any(|arg| matches!(arg, FnArg::Receiver(_)))
    {
        panic!("Expected new function to not have a self argument");
    };

    println!("Found constructor args {constructor_args:?}");

    let constructor_args = constructor_args.iter();

    let client_name = format_ident!("{}Client", item.ident);
    let client = quote! {
        #[derive(Debug)]
        pub struct #client_name {
            client: std::rc::Rc<std::cell::RefCell<::combadge::prelude::Client>>,
        }

        impl #client_name {
            pub fn new(worker: ::combadge::prelude::web_sys::Worker #(, #constructor_args)*) -> Self {
                Self { client: ::combadge::prelude::Client::new(worker) }
            }
        }
    };

    let server_name = format_ident!("{}Server", item.ident);
    let server = quote! {
        pub struct #server_name {
            server: std::rc::Rc<std::cell::RefCell<::combadge::prelude::Server>>,
        }

        impl #server_name {
            pub fn new() -> Self {
                log::info!("inside ss new");
                Self {
                    server: Server::new(),
                }
            }
        }
    };

    let result: TokenStream = quote! {
        #item
        #client
        #server
    }
    .into();

    // println!("{}", prettyplease::unparse(&parse(result.clone()).unwrap()));

    result
}
