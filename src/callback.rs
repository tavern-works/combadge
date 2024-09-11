use std::any::type_name;
use std::cell::RefCell;
use std::future::Future;
use std::marker::PhantomData;
use std::rc::{Rc, Weak};

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

impl<A, R> Responder for Box<dyn Fn(A) -> R> {
    fn respond(&self, arguments: Array, port: MessagePort) -> Result<(), Error> {
        let a: A = Post::from_js_value(arguments.shift())?;
        let result = Post::to_js_value(self(a))?;

        if R::NEEDS_TRANSFER {
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

impl<A, B, R> Responder for Box<dyn Fn(A, B) -> R> {
    fn respond(&self, arguments: Array, port: MessagePort) -> Result<(), Error> {
        let a: A = Post::from_js_value(arguments.shift())?;
        let b: B = Post::from_js_value(arguments.shift())?;
        let result = Post::to_js_value(self(a, b))?;

        if R::NEEDS_TRANSFER {
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

struct CallbackServer<A, R> {
    _phantom: PhantomData<(A, R)>,
    port: MessagePort,
}

impl<Args: 'static, R: 'static> CallbackServer<Args, R>
where
    Message: PostTuple<Args>,
{
    fn new(port: MessagePort) -> Self {
        Self {
            _phantom: PhantomData,
            port,
        }
    }

    fn call(&self, args: Args) -> AsyncReturnWithError<R> {
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

impl<A: 'static, R: 'static> ToClosure for CallbackServer<(A,), R> {
    type Output = Box<dyn Fn(A) -> AsyncReturnWithError<R>>;
    fn to_closure(self) -> Box<dyn Fn(A) -> AsyncReturnWithError<R>> {
        Box::new(move |a| self.call((a,)))
    }
}

impl<A: 'static, B: 'static, R: 'static> ToClosure for CallbackServer<(A, B), R> {
    type Output = Box<dyn Fn(A, B) -> AsyncReturnWithError<R>>;
    fn to_closure(self) -> Box<dyn Fn(A, B) -> AsyncReturnWithError<R>> {
        Box::new(move |a, b| self.call((a, b)))
    }
}

impl<Args, R> Drop for CallbackServer<Args, R> {
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

impl<A, R> CallbackTypes for ((A,), R) {
    type Local = Box<dyn Fn(A) -> R>;
    type Remote = Box<dyn Fn(A) -> AsyncReturnWithError<R>>;
}

impl<A, B, R> CallbackTypes for ((A, B), R) {
    type Local = Box<dyn Fn(A, B) -> R>;
    type Remote = Box<dyn Fn(A, B) -> AsyncReturnWithError<R>>;
}

pub struct Callback<A, R: 'static> {
    local: Option<<(A, R) as CallbackTypes>::Local>,
    remote: Option<<(A, R) as CallbackTypes>::Remote>,
}

impl<A, R> Transfer for Callback<A, R> {
    const NEEDS_TRANSFER: bool = true;
}

impl<A, R> From<Box<dyn Fn(A) -> R>> for Callback<(A,), R> {
    fn from(callback: Box<dyn Fn(A) -> R>) -> Self {
        Self {
            local: Some(callback),
            remote: None,
        }
    }
}

impl<A, R> From<Box<dyn Fn(A) -> AsyncReturnWithError<R>>> for Callback<(A,), R> {
    fn from(callback: Box<dyn Fn(A) -> AsyncReturnWithError<R>>) -> Self {
        Self {
            local: None,
            remote: Some(callback),
        }
    }
}

impl<A, B, R> From<Box<dyn Fn(A, B) -> R>> for Callback<(A, B), R> {
    fn from(callback: Box<dyn Fn(A, B) -> R>) -> Self {
        Self {
            local: Some(callback),
            remote: None,
        }
    }
}

impl<A, B, R> From<Box<dyn Fn(A, B) -> AsyncReturnWithError<R>>> for Callback<(A, B), R> {
    fn from(callback: Box<dyn Fn(A, B) -> AsyncReturnWithError<R>>) -> Self {
        Self {
            local: None,
            remote: Some(callback),
        }
    }
}

impl<Args: 'static, R: 'static> Post for Callback<Args, R>
where
    Message: PostTuple<Args>,
    <(Args, R) as CallbackTypes>::Local: Responder,
    CallbackServer<Args, R>: ToClosure,
    <CallbackServer<Args, R> as ToClosure>::Output: Into<Self>,
{
    const POSTABLE: bool = true;

    fn from_js_value(value: JsValue) -> Result<Self, Error> {
        let server = CallbackServer::<Args, R>::new(value.into());
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

pub trait Call1<A, R> {
    fn call(&self, a: A) -> AsyncReturnWithError<R>;
}

impl<A, R: 'static> Call1<A, R> for Callback<(A,), R>
where
    <((A,), R) as CallbackTypes>::Local: Fn(A) -> R,
    <((A,), R) as CallbackTypes>::Remote: Fn(A) -> AsyncReturnWithError<R>,
{
    fn call(&self, a: A) -> AsyncReturnWithError<R> {
        if let Some(remote) = &self.remote {
            remote(a)
        } else if let Some(local) = &self.local {
            let response = local(a);
            Box::new(async { Ok(response) })
        } else {
            Box::new(async {
                Err(Error::CallbackFailed {
                    error: String::from("remote callback not found"),
                })
            })
        }
    }
}

pub trait Call2<A, B, R> {
    fn call(&self, a: A, b: B) -> AsyncReturnWithError<R>;
}

impl<A, B, R: 'static> Call2<A, B, R> for Callback<(A, B), R>
where
    <((A, B), R) as CallbackTypes>::Local: Fn(A, B) -> R,
    <((A, B), R) as CallbackTypes>::Remote: Fn(A, B) -> AsyncReturnWithError<R>,
{
    fn call(&self, a: A, b: B) -> AsyncReturnWithError<R> {
        if let Some(remote) = &self.remote {
            remote(a, b)
        } else if let Some(local) = &self.local {
            let response = local(a, b);
            Box::new(async { Ok(response) })
        } else {
            Box::new(async {
                Err(Error::CallbackFailed {
                    error: String::from("remote callback not found"),
                })
            })
        }
    }
}
