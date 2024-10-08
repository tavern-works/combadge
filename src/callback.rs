use std::any::type_name;
use std::cell::RefCell;
use std::collections::VecDeque;
use std::future::{ready, Future};
use std::marker::PhantomData;
use std::pin::Pin;
use std::rc::{Rc, Weak};

use combadge_macros::{
    build_call_traits, build_callback_from_closure, build_callback_types, build_responder,
    build_to_closure,
};
use futures::{FutureExt, TryFutureExt};
use js_sys::{Array, Function, Promise};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::{spawn_local, JsFuture};
use web_sys::{MessageChannel, MessageEvent, MessagePort};

use crate::message::PostTuple;
use crate::{log_error, Error, Message, Post, Transfer};

type AsyncReturn<R> = Pin<Box<dyn Future<Output = R> + 'static>>;
type AsyncReturnWithError<R> = Pin<Box<dyn Future<Output = Result<R, Error>> + 'static>>;

trait Responder {
    fn respond(&self, arguments: Array, port: MessagePort) -> Result<(), Error>;
}

build_responder!(7);

struct CallbackServer {
    /// The server holds a reference to itself so it can keep the Closure alive.
    /// Once it receives the drop message, it releases this reference so the whole server is dropped.
    phylactery: Option<Rc<RefCell<Self>>>,
    #[expect(
        dead_code,
        reason = "We hold onto this closure's memory until the server is dropped"
    )]
    on_message: Closure<dyn Fn(MessageEvent)>,
}

impl CallbackServer {
    pub fn create<T: Responder + 'static>(callback: T) -> Result<MessagePort, Error> {
        let channel = MessageChannel::new().map_err(|error| Error::CreationFailed {
            type_name: String::from("MessageChannel"),
            error: format!("{error:?}"),
        })?;

        let server = Rc::new_cyclic(|weak_self: &Weak<RefCell<Self>>| {
            let cloned_weak = weak_self.clone();
            let server_port = channel.port1();
            let on_message = Closure::wrap(Box::new(move |message: MessageEvent| {
                let payload: Array = message.data().into();
                let Some(operation) = payload.shift().as_string() else {
                    log_error!("failed to get operation string in CallbackServer message");
                    return;
                };

                match operation.as_str() {
                    "call" => {
                        if let Err(error) = callback.respond(payload, server_port.clone()) {
                            log_error!("failed to respond to CallbackServer call: {error}");
                        }
                    }
                    "drop" => {
                        if let Some(client) = Weak::upgrade(&cloned_weak) {
                            if let Ok(mut client) = client.try_borrow_mut() {
                                client.phylactery = None;
                            } else {
                                log_error!("failed to borrow CallbackServer to drop it");
                            }
                        }
                    }
                    _ => {
                        log_error!("unknown operation {operation} in CallbackServer message");
                    }
                }
            }) as Box<dyn Fn(MessageEvent)>);

            channel
                .port1()
                .set_onmessage(Some(on_message.as_ref().unchecked_ref()));

            RefCell::new(Self {
                phylactery: None,
                on_message,
            })
        });

        server.borrow_mut().phylactery = Some(server.clone());

        Ok(channel.port2())
    }
}

struct CallbackClient<Args, Return> {
    _phantom: PhantomData<(Args, Return)>,
    port: MessagePort,
    #[expect(
        dead_code,
        reason = "We hold onto this closure's memory until the server is dropped"
    )]
    on_message: Closure<dyn Fn(MessageEvent)>,
    results: Rc<RefCell<VecDeque<Function>>>,
}

impl<Args: 'static, Return: 'static> CallbackClient<Args, Return>
where
    Message: PostTuple<Args>,
{
    fn new(port: MessagePort) -> Self {
        let results: Rc<RefCell<VecDeque<Function>>> = Rc::default();
        let cloned_results = results.clone();

        let on_message = Closure::wrap(Box::new(move |message: MessageEvent| {
            let Ok(mut results) = cloned_results.try_borrow_mut() else {
                log_error!("failed to borrow results queue in CallbackClient on_message");
                return;
            };

            let Some(send_result) = results.pop_front() else {
                log_error!("no result function found in CallbackClient");
                return;
            };

            if let Err(error) = send_result.call1(&JsValue::NULL, &message.data()) {
                log_error!("error while calling send_result in CallbackClient::call: {error:?}");
            }
        }) as Box<dyn Fn(MessageEvent)>);

        port.set_onmessage(Some(&on_message.as_ref().unchecked_ref()));

        Self {
            _phantom: PhantomData,
            port,
            on_message,
            results,
        }
    }

    fn call(&self, args: Args) -> AsyncReturnWithError<Return> {
        let mut send_result = None;
        let promise = Promise::new(&mut |resolve, _reject| {
            send_result = Some(resolve);
        });
        let send_result = send_result.unwrap();

        let Ok(mut results) = self.results.try_borrow_mut() else {
            return Box::pin(ready(Err(Error::CallbackFailed {
                error: String::from("failed to borrow results queue"),
            })));
        };

        results.push_back(send_result);

        let mut message = Message::new("call");
        let post = message.post_tuple(args).and_then(|()| {
            message.send(|message, transfer| {
                self.port
                    .post_message_with_transferable(message, transfer)
                    .map_err(|error| Error::CallbackFailed {
                        error: format!("failed to post message: {error:?}"),
                    })
            })
        });

        Box::pin(async { post }.and_then(|()| {
            JsFuture::from(promise).map(|result| {
                result
                    .map(|result| Post::from_js_value(result))
                    .map_err(|error| Error::CallbackFailed {
                        error: format!("promise rejected: {error:?}"),
                    })
                    .flatten()
            })
        }))
    }
}

trait ToClosure {
    type Output;
    fn to_closure(self) -> Self::Output;
}

build_to_closure!(7);

impl<Args, Return> Drop for CallbackClient<Args, Return> {
    fn drop(&mut self) {
        if let Err(error) = self
            .port
            .post_message(&Array::of1(&JsValue::from_str("drop")))
        {
            log_error!("error while posting drop message to server: {error:?}");
        }
    }
}

trait CallbackTypes {
    type Local;
    type AsyncLocal;
    type Remote;
}

impl<T> CallbackTypes for T {
    default type Local = ();
    default type AsyncLocal = ();
    default type Remote = ();
}

build_callback_types!(7);

pub struct Callback<Args, Return: 'static> {
    local: Option<<(Args, Return) as CallbackTypes>::Local>,
    async_local: Option<<(Args, Return) as CallbackTypes>::AsyncLocal>,
    remote: Option<<(Args, Return) as CallbackTypes>::Remote>,
}

build_callback_from_closure!(7);

impl<Args: 'static, Return: 'static> Post for Callback<Args, Return>
where
    Message: PostTuple<Args>,
    <(Args, Return) as CallbackTypes>::Local: Responder,
    <(Args, Return) as CallbackTypes>::AsyncLocal: Responder,
    CallbackClient<Args, Return>: ToClosure,
    <CallbackClient<Args, Return> as ToClosure>::Output: Into<Self>,
{
    const POSTABLE: bool = true;

    fn from_js_value(value: JsValue) -> Result<Self, Error> {
        let client = CallbackClient::<Args, Return>::new(value.into());
        Ok(client.to_closure().into())
    }

    fn to_js_value(self) -> Result<JsValue, Error> {
        if let Some(local) = self.local {
            CallbackServer::create(local).map(JsValue::from)
        } else if let Some(async_local) = self.async_local {
            CallbackServer::create(async_local).map(JsValue::from)
        } else {
            return Err(Error::SerializeFailed {
                type_name: String::from(type_name::<Self>()),
                error: String::from("can't serialize callback without a local callback"),
            });
        }
    }
}

impl<Args, Return> Transfer for Callback<Args, Return> {
    fn get_transferable(js_value: &JsValue) -> Option<Array> {
        Some(Array::of1(js_value))
    }
}

pub mod call_traits {
    use super::*;
    build_call_traits!(7);
}
