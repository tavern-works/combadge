extern crate proc_macro;
use proc_macro::TokenStream;

use quote::{format_ident, quote};
use syn::{parse, parse_macro_input, FnArg, ItemTrait, Pat, ReturnType, TraitItem};

#[proc_macro_attribute]
pub fn combadge(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let item: ItemTrait = parse_macro_input!(item);

    let functions = item
        .items
        .iter()
        .filter_map(|item| match item {
            TraitItem::Fn(f) => Some(f),
            _ => None,
        })
        .collect::<Vec<_>>();

    let name = functions
        .iter()
        .map(|function| function.sig.ident.clone())
        .collect::<Vec<_>>();

    let name_string = name.iter().map(|name| name.to_string()).collect::<Vec<_>>();

    let argument = functions
        .iter()
        .map(|function| function.sig.inputs.iter().collect::<Vec<_>>())
        .collect::<Vec<_>>();

    let non_receiver = argument
        .iter()
        .enumerate()
        .map(|(index, arguments)| {
            let non_receiver = arguments
                .iter()
                .filter_map(|arg| {
                    let pat = match arg {
                        FnArg::Receiver(_) => None,
                        FnArg::Typed(typed) => Some(typed.pat.clone()),
                    }?;
                    match *pat {
                        Pat::Ident(ident) => Some(ident.ident),
                        _ => None,
                    }
                })
                .collect::<Vec<_>>();

            if non_receiver.len() == arguments.len() {
                panic!(
                    "expected {} to have a receiver (self parameter)",
                    name[index]
                )
            }

            non_receiver
        })
        .collect::<Vec<_>>();

    let result = functions
        .iter()
        .map(|function| match &function.sig.output {
            ReturnType::Default => quote! { Result<(), ::combadge::Error> },
            ReturnType::Type(_, t) => quote! { Result<#t, ::combadge::Error> },
        })
        .collect::<Vec<_>>();

    let client_name = format_ident!("{}Client", item.ident);
    let client = quote! {
        #[derive(Debug)]
        pub struct #client_name {
            client: std::rc::Rc<std::cell::RefCell<::combadge::Client>>,
        }

        impl #client_name {
            pub fn new(worker: ::combadge::prelude::web_sys::Worker) -> Self {
                Self { client: ::combadge::Client::new(worker) }
            }

            #(
                pub async fn #name(#(#argument),*) -> #result {
                    let mut message = ::combadge::Message::new(#name_string);
                    #(
                        message.post(#non_receiver)?;
                    )*
                    let client = self.client.try_borrow().map_err(|_| ::combadge::Error::ClientUnavailable)?;
                    client.send_message(message).await
                }
            )*
        }
    };

    println!("{}", client);

    let server_name = format_ident!("{}Server", item.ident);
    let server = quote! {
        pub struct #server_name {
            server: std::rc::Rc<std::cell::RefCell<::combadge::Server>>,
        }

        impl #server_name {
            pub fn new() -> Self {
                log::info!("inside ss new");
                Self {
                    server: ::combadge::Server::new(),
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

    println!("{}", prettyplease::unparse(&parse(result.clone()).unwrap()));

    result
}
