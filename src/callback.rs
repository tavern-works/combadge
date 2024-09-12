use std::any::type_name;
use std::cell::RefCell;
use std::future::Future;
use std::marker::PhantomData;
use std::rc::{Rc, Weak};

use combadge_macros::{
    build_call_traits, build_callback_from_closure, build_callback_types, build_responder,
    build_to_closure,
};
use futures::{FutureExt, TryFutureExt};
use js_sys::{Array, Promise};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use web_sys::{MessageChannel, MessageEvent, MessagePort};

use crate::message::PostTuple;
use crate::{Error, Message, Post, Transfer};

type AsyncReturn<R> = Box<dyn Future<Output = R>>;
type AsyncReturnWithError<R> = Box<dyn Future<Output = Result<R, Error>>>;
pub type AsyncClosure<A, R> = Box<dyn Fn(A) -> AsyncReturn<R>>;

trait Responder {
    fn respond(&self, arguments: Array, port: MessagePort) -> Result<(), Error>;
}

build_responder!(7);

struct CallbackClient {
    /// The client holds a reference to itself so it can keep the Closure alive.
    /// Once it receives the drop message, it releases this reference so the whole client is dropped.
    phylactery: Option<Rc<RefCell<Self>>>,
    #[expect(
        dead_code,
        reason = "We hold onto this closure's memory until the client is dropped"
    )]
    on_message: Closure<dyn Fn(MessageEvent)>,
}

impl CallbackClient {
    pub fn create<T: Responder + 'static>(callback: T) -> Result<MessagePort, Error> {
        let channel = MessageChannel::new().map_err(|error| Error::CreationFailed {
            type_name: String::from("MessageChannel"),
            error: format!("{error:?}"),
        })?;

        let client = Rc::new_cyclic(|weak_self: &Weak<RefCell<Self>>| {
            let cloned_weak = weak_self.clone();
            let client_port = channel.port1();
            let on_message = Closure::wrap(Box::new(move |message: MessageEvent| {
                let payload: Array = message.data().into();
                let Some(operation) = payload.shift().as_string() else {
                    #[cfg(feature = "log")]
                    log::error!("failed to get operation string in CallbackClient message");
                    return;
                };

                match operation.as_str() {
                    "call" => {
                        if let Err(error) = callback.respond(payload, client_port.clone()) {
                            #[cfg(feature = "log")]
                            log::error!("failed to respond to CallbackClient call: {error}");
                        }
                    }
                    "drop" => {
                        if let Some(client) = Weak::upgrade(&cloned_weak) {
                            if let Ok(mut client) = client.try_borrow_mut() {
                                client.phylactery = None;
                            } else {
                                #[cfg(feature = "log")]
                                log::error!("failed to borrow CallbackClient to drop it");
                            }
                        }
                    }
                    _ => {
                        #[cfg(feature = "log")]
                        log::error!("unknown operation {operation} in CallbackClient message");
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

        client.borrow_mut().phylactery = Some(client.clone());

        Ok(channel.port2())
    }
}

struct CallbackServer<Args, Return> {
    _phantom: PhantomData<(Args, Return)>,
    port: MessagePort,
}

impl<Args: 'static, Return: 'static> CallbackServer<Args, Return>
where
    Message: PostTuple<Args>,
{
    fn new(port: MessagePort) -> Self {
        Self {
            _phantom: PhantomData,
            port,
        }
    }

    fn call(&self, args: Args) -> AsyncReturnWithError<Return> {
        let mut send_result = None;
        let promise = Promise::new(&mut |resolve, _reject| {
            send_result = Some(resolve);
        });
        let send_result = send_result.unwrap();

        let on_message = Closure::once_into_js(move |message: MessageEvent| {
            send_result.call1(&JsValue::NULL, &message.data())
        });
        self.port.set_onmessage(Some(&on_message.unchecked_ref()));
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

        Box::new(async { post }.and_then(|()| {
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

impl<Args, Return> Drop for CallbackServer<Args, Return> {
    fn drop(&mut self) {
        if let Err(error) = self
            .port
            .post_message(&Array::of1(&JsValue::from_str("drop")))
        {
            #[cfg(feature = "log")]
            log::error!("error while posting drop message to client: {error:?}");
        }
    }
}

trait CallbackTypes {
    type Local;
    type Remote;
}

impl<T> CallbackTypes for T {
    default type Local = ();
    default type Remote = ();
}

build_callback_types!(7);

pub struct Callback<Args, Return: 'static> {
    local: Option<<(Args, Return) as CallbackTypes>::Local>,
    remote: Option<<(Args, Return) as CallbackTypes>::Remote>,
}

build_callback_from_closure!(7);

impl<Args: 'static, Return: 'static> Post for Callback<Args, Return>
where
    Message: PostTuple<Args>,
    <(Args, Return) as CallbackTypes>::Local: Responder,
    CallbackServer<Args, Return>: ToClosure,
    <CallbackServer<Args, Return> as ToClosure>::Output: Into<Self>,
{
    const POSTABLE: bool = true;

    fn from_js_value(value: JsValue) -> Result<Self, Error> {
        let server = CallbackServer::<Args, Return>::new(value.into());
        Ok(server.to_closure().into())
    }

    fn to_js_value(self) -> Result<JsValue, Error> {
        let Some(local) = self.local else {
            return Err(Error::SerializeFailed {
                type_name: String::from(type_name::<Self>()),
                error: String::from("can't serialize callback without a local callback"),
            });
        };

        CallbackClient::create(local).map(JsValue::from)
    }
}

impl<Args, Return> Transfer for Callback<Args, Return> {
    const NEEDS_TRANSFER: bool = true;
}

pub mod call_traits {
    use super::*;
    build_call_traits!(7);
}
