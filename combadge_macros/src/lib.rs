extern crate proc_macro;
use proc_macro::TokenStream;

use quote::{format_ident, quote};
use syn::{parse_macro_input, FnArg, ItemTrait, Pat, ReturnType, TraitItem};

#[proc_macro_attribute]
pub fn combadge(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let item: ItemTrait = parse_macro_input!(item);
    let trait_name = item.ident.clone();

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
                .filter_map(|arg| match arg {
                    FnArg::Receiver(_) => None,
                    FnArg::Typed(typed) => Some(typed.clone()),
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

    let non_receiver_name = non_receiver
        .iter()
        .map(|non_receiver| {
            non_receiver
                .iter()
                .filter_map(|item| match item.pat.as_ref() {
                    Pat::Ident(ident) => Some(ident.clone()),
                    _ => None,
                })
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();

    let non_receiver_type = non_receiver
        .iter()
        .map(|non_receiver| {
            non_receiver
                .iter()
                .map(|item| item.ty.clone())
                .collect::<Vec<_>>()
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
            pub fn new(worker: web_sys::Worker) -> Self {
                Self { client: ::combadge::Client::new(worker) }
            }

            #(
                #[expect(clippy::future_not_send)]
                pub fn #name(#(#argument),*) -> impl std::future::Future<Output = #result> {
                    use ::combadge::reexports::futures::future::FutureExt;
                    use ::combadge::reexports::futures::future::TryFutureExt;

                    let message = Ok(::combadge::Message::new(#name_string));
                    #(
                        ::combadge::reexports::static_assertions::assert_impl_any!(#non_receiver_type: Into<::combadge::reexports::wasm_bindgen::JsValue>, ::combadge::reexports::serde::Serialize);
                        let message = message.and_then(|mut message| {
                            message.post(#non_receiver_name)?;
                            Ok(message)
                        });
                    )*

                    let server_ready = match self
                        .client
                        .try_borrow_mut()
                        .map_err(|_| ::combadge::Error::ClientUnavailable)
                    {
                        Ok(mut client) => client.wait_for_server().map(|()| Ok(())).left_future(),
                        Err(error) => async { Err(error) }.right_future(),
                    };

                    let client_clone = self.client.clone();
                    server_ready.then(move |result| {
                        let message = result.and(message);
                        async { message }.and_then(move |message| {
                            let client = client_clone
                                .try_borrow_mut()
                                .map_err(|_| ::combadge::Error::ClientUnavailable);
                            let message = client.map(|mut client| client.send_message(message));
                            async { message }.try_flatten()
                        })
                    })
                }
            )*
        }
    };

    let server_name = format_ident!("{}Server", item.ident);
    let server = quote! {
        pub struct #server_name {
            server: std::rc::Rc<std::cell::RefCell<::combadge::Server>>,
        }

        impl #server_name {
            pub fn new(mut local: Box<dyn #trait_name>) -> Self {
                let dispatch = Box::new(move |procedure: &str, data| {
                    match procedure {
                        #(
                            #name_string => Self::#name(local.as_mut(), data),
                        )*
                        _ => Err(::combadge::Error::UnknownProcedure{ name: String::from(procedure) })
                    }
                });

                Self {
                    server: ::combadge::Server::new(dispatch),
                }
            }

            #(
                fn #name(local: &mut dyn #trait_name, data: ::combadge::reexports::js_sys::Array) -> Result<(), ::combadge::Error> {
                    #(
                        ::combadge::reexports::static_assertions::assert_impl_any!(#non_receiver_type: Into<::combadge::reexports::wasm_bindgen::JsValue>, ::combadge::reexports::serde::de::DeserializeOwned);
                        let #non_receiver = ::combadge::Post::from_js_value(data.shift())?;
                    )*
                    let result = local.#name(#(#non_receiver_name),*);
                    let port: ::combadge::reexports::web_sys::MessagePort = data.shift().into();
                    port.post_message(&::combadge::Post::to_js_value(result)?).map_err(|error| ::combadge::Error::PostFailed{ error: format!("{error:?}")})
                }
            )*
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
