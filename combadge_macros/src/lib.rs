extern crate proc_macro;

use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{parse, parse_macro_input, FnArg, Ident, ItemTrait, LitInt, Pat, ReturnType, TraitItem};

fn parse_count(item: TokenStream) -> usize {
    let Ok(count) = parse::<LitInt>(item.clone().into()) else {
        panic!("expected an integer literal");
    };

    let Ok(count) = count.base10_parse::<usize>() else {
        panic!("failed to parse {count} as usize");
    };

    if count == 0 {
        panic!("must generate at least 1 variable");
    }

    if count > 26 {
        panic!("can only generate up to 26 variables without running out of letters");
    }

    count
}

fn build_variables(count: usize) -> (Vec<Ident>, Vec<Ident>) {
    let type_name = (0..count)
        .map(|i| char::from(b'A' + i as u8))
        .collect::<Vec<_>>();

    let variable_name = type_name
        .iter()
        .map(|t| format_ident!("{}", t.to_ascii_lowercase()))
        .collect::<Vec<_>>();

    let type_name = type_name
        .iter()
        .map(|t| format_ident!("{}", t))
        .collect::<Vec<_>>();

    (type_name, variable_name)
}

#[proc_macro]
pub fn build_responders(item: TokenStream) -> TokenStream {
    let max_count = parse_count(item);

    let mut responders = quote! {};
    for count in 1..=max_count {
        let (type_name, variable_name) = build_variables(count);

        responders = quote! {
            #responders

            impl<#(#type_name),*, Return> Responder for Box<dyn Fn(#(#type_name),*) -> Return> {
                fn respond(&self, arguments: Array, port: MessagePort) -> Result<(), Error> {
                    #(
                        let #variable_name: #type_name = Post::from_js_value(arguments.shift())?;
                    )*
                    let result = Post::to_js_value(self(#(#variable_name),*))?;
    
                    if Return::NEEDS_TRANSFER {
                        port.post_message_with_transferable(&result, &result)
                            .map_err(|error| Error::PostFailed {
                                error: format!("failed to respond in Responder: {error:?}"),
                            })?;
                    } else {
                        port.post_message(&result)
                            .map_err(|error| Error::PostFailed {
                                error: format!("failed to respond in Responder: {error:?}"),
                            })?;
                    }
    
                    Ok(())
                }
            }
        }
    }

    responders.into()
}

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

    let output = functions
        .iter()
        .map(|function| function.sig.output.clone())
        .collect::<Vec<_>>();

    let return_type = output
        .iter()
        .map(|output| match output {
            ReturnType::Default => quote! { () },
            ReturnType::Type(_, t) => quote! { #t },
        })
        .collect::<Vec<_>>();

    let return_with_error = output
        .iter()
        .map(|output| match output {
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
                pub fn #name(#(#argument),*) -> impl std::future::Future<Output = #return_with_error> {
                    use ::combadge::reexports::futures::future::FutureExt;
                    use ::combadge::reexports::futures::future::TryFutureExt;

                    let message = Ok(::combadge::Message::new(#name_string));
                    #(
                        const _: () = assert!(<#non_receiver_type as ::combadge::Post>::POSTABLE);
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
                            let message = client.map(|mut client| client.send_message::<#return_type>(message));
                            async { message }.try_flatten().map(|result| {
                                let result: #return_with_error = result.map(std::convert::Into::into);
                                result
                            })
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
                        const _: () = assert!(<#non_receiver_type as ::combadge::Post>::POSTABLE);
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
