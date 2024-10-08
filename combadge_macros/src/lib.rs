extern crate proc_macro;

use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{
    parse, parse_macro_input, FnArg, GenericArgument, Ident, ImplItem, Index, ItemImpl, ItemTrait,
    LitInt, Pat, PathArguments, ReturnType, TraitItem, Type, TypeParamBound, Visibility,
};

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
pub fn build_call_traits(item: TokenStream) -> TokenStream {
    let max_count = parse_count(item);

    let mut call_traits = quote! {};
    for count in 1..=max_count {
        let (type_name, variable_name) = build_variables(count);
        let trait_name = format_ident!("Call{}", count);

        call_traits = quote! {
            #call_traits

            pub trait #trait_name<#(#type_name),*, Return> {
                fn call(&self, #(#variable_name: #type_name),*) -> AsyncReturnWithError<Return>;
            }

            impl<#(#type_name),*, Return: 'static> #trait_name<#(#type_name),*, Return> for Callback<(#(#type_name),*,), Return>
            where
                <((#(#type_name),*,), Return) as CallbackTypes>::Local: Fn(#(#type_name),*) -> Return,
                <((#(#type_name),*,), Return) as CallbackTypes>::AsyncLocal: Fn(#(#type_name),*) -> AsyncReturn<Return>,
                <((#(#type_name),*,), Return) as CallbackTypes>::Remote: Fn(#(#type_name),*) -> AsyncReturnWithError<Return>,
            {
                fn call(&self, #(#variable_name: #type_name),*) -> AsyncReturnWithError<Return> {
                    if let Some(remote) = &self.remote {
                        remote(#(#variable_name),*)
                    } else if let Some(local) = &self.local {
                        let response = local(#(#variable_name),*);
                        Box::pin(async { Ok(response) })
                    } else if let Some(async_local) = &self.async_local {
                        let result = async_local(#(#variable_name),*);
                        Box::pin(async move {
                            let result = result.await;
                            Ok(result)
                        })
                    } else {
                        Box::pin(async {
                            Err(Error::CallbackFailed {
                                error: String::from("callbacks (both remote and local) not found"),
                            })
                        })
                    }
                }
            }
        }
    }

    call_traits.into()
}

#[proc_macro]
pub fn build_callback_from_closure(item: TokenStream) -> TokenStream {
    let max_count = parse_count(item);

    let mut callback_from_closure = quote! {};
    for count in 1..=max_count {
        let (type_name, _) = build_variables(count);

        callback_from_closure = quote! {
            #callback_from_closure

            impl<#(#type_name),*, Return> From<Box<dyn Fn(#(#type_name),*) -> Return>> for Callback<(#(#type_name),*,), Return> {
                fn from(callback: Box<dyn Fn(#(#type_name),*) -> Return>) -> Self {
                    Self {
                        local: Some(callback),
                        async_local: None,
                        remote: None,
                    }
                }
            }

            impl<#(#type_name),*, Return> From<Box<dyn Fn(#(#type_name),*) -> AsyncReturn<Return>>> for Callback<(#(#type_name),*,), Return> {
                fn from(callback: Box<dyn Fn(#(#type_name),*) -> AsyncReturn<Return>>) -> Self {
                    Self {
                        local: None,
                        async_local: Some(callback),
                        remote: None,
                    }
                }
            }

            impl<#(#type_name),*, Return> From<Box<dyn Fn(#(#type_name),*) -> AsyncReturnWithError<Return>>> for Callback<(#(#type_name),*,), Return> {
                fn from(callback: Box<dyn Fn(#(#type_name),*) -> AsyncReturnWithError<Return>>) -> Self {
                    Self {
                        local: None,
                        async_local: None,
                        remote: Some(callback),
                    }
                }
            }
        }
    }

    callback_from_closure.into()
}

#[proc_macro]
pub fn build_callback_types(item: TokenStream) -> TokenStream {
    let max_count = parse_count(item);

    let mut callback_types = quote! {};
    for count in 1..=max_count {
        let (type_name, _) = build_variables(count);

        callback_types = quote! {
            #callback_types

            impl<#(#type_name),*, Return> CallbackTypes for ((#(#type_name),*,), Return) {
                type Local = Box<dyn Fn(#(#type_name),*) -> Return>;
                type AsyncLocal = Box<dyn Fn(#(#type_name),*) -> AsyncReturn<Return>>;
                type Remote = Box<dyn Fn(#(#type_name),*) -> AsyncReturnWithError<Return>>;
            }
        }
    }

    callback_types.into()
}

#[proc_macro]
pub fn build_post_tuple(item: TokenStream) -> TokenStream {
    let max_count = parse_count(item);

    let mut post_tuple = quote! {};
    for count in 1..=max_count {
        let (type_name, _) = build_variables(count);
        let index = (0..count).map(Index::from).collect::<Vec<_>>();

        post_tuple = quote! {
            #post_tuple

            impl<#(#type_name),*> PostTuple<(#(#type_name),*,)> for Message {
                fn post_tuple(&mut self, tuple: (#(#type_name),*,)) -> Result<(), Error> {
                    #(
                        self.post(tuple.#index)?;
                    )*
                    Ok(())
                }
            }
        }
    }

    post_tuple.into()
}

#[proc_macro]
pub fn build_responder(item: TokenStream) -> TokenStream {
    let max_count = parse_count(item);

    let mut responder = quote! {};
    for count in 1..=max_count {
        let (type_name, variable_name) = build_variables(count);

        responder = quote! {
            #responder

            impl<#(#type_name),*, Return> Responder for Box<dyn Fn(#(#type_name),*) -> Return> {
                default fn respond(&self, arguments_: Array, port_: MessagePort) -> Result<(), Error> {
                    #(
                        let #variable_name: #type_name = Post::from_js_value(arguments_.shift())?;
                    )*
                    let result = Post::to_js_value(self(#(#variable_name),*))?;

                    if let Some(transferable) = <Return as Transfer>::get_transferable(&result) {
                        port_.post_message_with_transferable(&result, &transferable)
                            .map_err(|error| Error::PostFailed {
                                error: format!("failed to respond in Responder: {error:?}"),
                            })?;
                    } else {
                        port_.post_message(&result)
                            .map_err(|error| Error::PostFailed {
                                error: format!("failed to respond in Responder: {error:?}"),
                            })?;
                    }

                    Ok(())
                }
            }

            impl<#(#type_name),*, Return: 'static> Responder for Box<dyn Fn(#(#type_name),*) -> Box<dyn Future<Output = Return>>> {
                fn respond(&self, arguments_: Array, port_: MessagePort) -> Result<(), Error> {
                    #(
                        let #variable_name: #type_name = Post::from_js_value(arguments_.shift())?;
                    )*
                    let result = self(#(#variable_name),*);
                    let future_result = async move {
                        let result = Box::into_pin(result).await;
                        let value = match Post::to_js_value(result) {
                            Ok(value) => value,
                            Err(error) => {
                                crate::log_error!("error while converting to JsValue in future: {error:?}");
                                return;
                            }
                        };

                        if let Err(error) = Return::get_transferable(&value).map_or_else(
                            || port_.post_message(&value),
                            |transferable| port_.post_message_with_transferable(&value, &Array::of1(&value))
                        ) {
                            crate::log_error!("error while posting async: {error:?}");
                        }
                    };
                    spawn_local(future_result);
                    Ok(())
                }
            }
        }
    }

    responder.into()
}

#[proc_macro]
pub fn build_to_closure(item: TokenStream) -> TokenStream {
    let max_count = parse_count(item);

    let mut to_closure = quote! {};
    for count in 1..=max_count {
        let (type_name, variable_name) = build_variables(count);

        to_closure = quote! {
            #to_closure

            impl<#(#type_name: 'static),*, Return: 'static> ToClosure for CallbackClient<(#(#type_name),*,), Return> {
                type Output = Box<dyn Fn(#(#type_name),*) -> AsyncReturnWithError<Return>>;
                fn to_closure(self) -> Box<dyn Fn(#(#type_name),*) -> AsyncReturnWithError<Return>> {
                    Box::new(move |#(#variable_name),*| self.call((#(#variable_name),*,)))
                }
            }
        }
    }

    to_closure.into()
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

    let internal_type = output
        .iter()
        .map(|output| match output {
            ReturnType::Default => quote! { () },
            ReturnType::Type(_, t) => match t.as_ref() {
                Type::Path(path) => {
                    if path.path.segments.len() > 1
                        || path.path.segments.get(0).unwrap().ident != "Box"
                    {
                        return quote! { #t };
                    }
                    let segment = path.path.segments.get(0).unwrap();
                    match &segment.arguments {
                        PathArguments::AngleBracketed(arguments) => {
                            if arguments.args.len() > 1 {
                                return quote! { #t };
                            }
                            let argument = arguments.args.get(0).unwrap();
                            match argument {
                                GenericArgument::Type(generic_type) => match generic_type {
                                    Type::TraitObject(trait_) => {
                                        if trait_.dyn_token.is_none() || trait_.bounds.len() > 1 {
                                            return quote! { #t };
                                        }

                                        match trait_.bounds.get(0).unwrap() {
                                            TypeParamBound::Trait(bound) => {
                                                if bound.path.segments.len() > 1 {
                                                    return quote! { #t };
                                                }

                                                let segment = bound.path.segments.get(0).unwrap();
                                                if segment.ident != "Future" {
                                                    return quote! { #t };
                                                }

                                                if let PathArguments::AngleBracketed(arguments) =
                                                    &segment.arguments
                                                {
                                                    if arguments.args.len() > 1 {
                                                        return quote! { #t };
                                                    }

                                                    match arguments.args.get(0).unwrap() {
                                                        GenericArgument::AssocType(assoc) => {
                                                            if assoc.ident != "Output" {
                                                                return quote! { #t };
                                                            }

                                                            let generic_type = &assoc.ty;
                                                            quote! { #generic_type }
                                                        }
                                                        _ => quote! { #t },
                                                    }
                                                } else {
                                                    quote! { #t }
                                                }
                                            }
                                            _ => quote! { #t },
                                        }
                                    }
                                    _ => quote! { #t },
                                },
                                _ => quote! { #t},
                            }
                        }
                        _ => quote! { #t },
                    }
                }
                _ => quote! { #t },
            },
        })
        .collect::<Vec<_>>();

    let client_name = format_ident!("{}Client", item.ident);
    let client = quote! {
        #[derive(Clone, Debug)]
        pub struct #client_name<P: ::combadge::Port + 'static> {
            client: std::rc::Rc<std::cell::RefCell<::combadge::Client::<P>>>,
        }

        impl<P: ::combadge::Port + 'static> #client_name<P> {
            pub fn new(port: P) -> Self {
                Self { client: ::combadge::Client::new(port) }
            }

            #(
                #[expect(clippy::future_not_send)]
                pub fn #name(#(#argument),*) -> impl std::future::Future<Output = Result<#internal_type, ::combadge::Error>> {
                    use ::combadge::reexports::futures::future::FutureExt;
                    use ::combadge::reexports::futures::future::TryFutureExt;
                    const _: () = assert!(<#internal_type as ::combadge::Post>::POSTABLE);

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
                            let message = client.map(|mut client| client.send_message::<#internal_type>(message));
                            async { message }.try_flatten().map(|result| {
                                let result: Result<#internal_type, ::combadge::Error> = result.map(std::convert::Into::into);
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
        pub struct #server_name<P: ::combadge::Port + 'static> {
            server: std::rc::Rc<std::cell::RefCell<::combadge::Server<P>>>,
        }

        impl<P: ::combadge::Port + 'static> #server_name<P> {
            pub fn create<L: #trait_name + 'static>(mut local: L, port: P) {
                let dispatch = Box::new(move |procedure: &str, data| {
                    match procedure {
                        #(
                            #name_string => Self::#name(&mut local, data),
                        )*
                        _ => Err(::combadge::Error::UnknownProcedure{ name: String::from(procedure) })
                    }
                });

                ::combadge::Server::create(port, dispatch);
            }

            #(
                fn #name(local_: &mut dyn #trait_name, data_: ::combadge::reexports::js_sys::Array) -> Result<(), ::combadge::Error> {
                    use ::combadge::reexports::wasm_bindgen_futures::spawn_local;

                    #(
                        const _: () = assert!(<#non_receiver_type as ::combadge::Post>::POSTABLE);
                        let #non_receiver = ::combadge::Post::from_js_value(data_.shift())?;
                    )*
                    let result = local_.#name(#(#non_receiver_name),*);
                    let port: ::combadge::reexports::web_sys::MessagePort = data_.shift().into();
                    let async_result = ::combadge::MaybeAsync::to_maybe_async(result);
                    let future_result = async move {
                        let result: #internal_type = Box::into_pin(async_result).await;
                        let value = match ::combadge::Post::to_js_value(result) {
                            Ok(value) => value,
                            Err(error) => {
                                ::combadge::log_error!("error while converting to JsValue in future: {error:?}");
                                return;
                            }
                        };

                        if let Err(error) = <#internal_type as ::combadge::Transfer>::get_transferable(&value).map_or_else(
                            || port.post_message(&value),
                            |transferable| port.post_message_with_transferable(&value, &transferable))
                        {
                            ::combadge::log_error!("error while posting {value:?} {} in {} async: {error:?}", std::any::type_name::<#internal_type>(), #name_string);
                        }
                    };
                    spawn_local(future_result);
                    Ok(())
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

#[proc_macro_attribute]
pub fn proxy(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let item_impl: ItemImpl = parse_macro_input!(item);
    let Type::Path(path) = &*item_impl.self_ty else {
        panic!("proxy expected to find a path in impl");
    };

    if path.qself.is_some() {
        panic!("can't proxy an impl with a qualified type");
    }

    if path.path.segments.len() > 1 {
        panic!("can't proxy an impl with a multi-segment path")
    }

    let struct_name = path.path.segments.get(0).unwrap().ident.clone();
    let trait_name = format_ident!("{}Proxy", struct_name);
    let local_name = format_ident!("{}Local", struct_name);
    let client_name = format_ident!("{}Client", trait_name);
    let server_name = format_ident!("{}Server", trait_name);

    let functions = item_impl
        .items
        .iter()
        .filter_map(|item| match item {
            ImplItem::Fn(f) => {
                if matches!(f.vis, Visibility::Public(_)) {
                    Some(f)
                } else {
                    None
                }
            }
            _ => None,
        })
        .collect::<Vec<_>>();

    let argument = functions
        .iter()
        .map(|function| function.sig.inputs.iter().collect::<Vec<_>>())
        .collect::<Vec<_>>();

    let name = functions
        .iter()
        .map(|function| function.sig.ident.clone())
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

    let name = functions
        .iter()
        .map(|function| function.sig.ident.clone())
        .collect::<Vec<_>>();

    let argument = functions
        .iter()
        .map(|function| function.sig.inputs.iter().collect::<Vec<_>>())
        .collect::<Vec<_>>();

    quote! {
        #item_impl

        #[combadge]
        trait #trait_name {
            #(
                fn #name(#(#argument),*) -> #return_type;
            )*
        }

        struct #local_name {
            local: #struct_name
        }

        impl #local_name {
            fn new(local: #struct_name) -> Self {
                Self { local }
            }
        }

        impl #trait_name for #local_name {
            #(
                fn #name(#(#argument),*) -> #return_type {
                    self.local.#name(#(#non_receiver_name),*)
                }
            )*
        }

        impl ::combadge::AsHandle<#struct_name> for #struct_name {
            type Client = #client_name<::combadge::reexports::web_sys::MessagePort>;
            type Server = #server_name<::combadge::reexports::web_sys::MessagePort>;

            fn into_client(port: ::combadge::reexports::web_sys::MessagePort) -> Self::Client {
                Self::Client::new(port)
            }

            fn create_server(local: #struct_name, port: ::combadge::reexports::web_sys::MessagePort)  {
                Self::Server::create(#local_name::new(local), port);
            }
        }

    }
    .into()
}
