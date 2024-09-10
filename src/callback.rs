use std::any::type_name;
use std::cell::RefCell;
use std::future::Future;
use std::marker::PhantomData;
use std::rc::{Rc, Weak};

use futures::FutureExt;
use js_sys::{Array, Promise};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use web_sys::{MessageChannel, MessageEvent, MessagePort};

use crate::message::Transfer;
use crate::{Error, Message, Post};

type AsyncReturn<R> = Box<dyn Future<Output = R>>;
type AsyncReturnWithError<R> = Box<dyn Future<Output = Result<R, Error>>>;
pub type AsyncClosure<A, R> = Box<dyn Fn(A) -> AsyncReturn<R>>;

trait Responder {
    fn respond(&self, arguments: Array, port: MessagePort);
}

impl<A, R> Responder for Box<dyn Fn(A) -> R> {
    fn respond(&self, arguments: Array, port: MessagePort) {}
}

struct CallbackClient {
    /// The client holds a reference to itself so it can keep the Closure alive.
    /// Once it receives the drop message, it releases this reference so the whole client is dropped.
    phylactery: Option<Rc<RefCell<Self>>>,
    on_message: Closure<dyn Fn(MessageEvent)>,
}

impl CallbackClient {
    pub fn create<T: Responder + 'static>(callback: T) -> Result<MessagePort, Error> {
        #[cfg(feature = "log")]
        log::info!("creating messagechannel");
        let channel = MessageChannel::new().map_err(|error| Error::CreationFailed {
            type_name: String::from("MessageChannel"),
            error: format!("{error:?}"),
        })?;

        #[cfg(feature = "log")]
        log::info!("creating client");

        let client = Rc::new_cyclic(|weak_self: &Weak<RefCell<Self>>| {
            let cloned_weak = weak_self.clone();
            let client_port = channel.port1();
            let on_message = Closure::wrap(Box::new(move |message: MessageEvent| {
                #[cfg(feature = "log")]
                log::info!("client received message {message:?} {:?}", message.data());

                let payload: Array = message.data().into();
                let Some(operation) = payload.get(0).as_string() else {
                    #[cfg(feature = "log")]
                    log::error!("failed to get operation string in CallbackClient message");
                    return;
                };

                match operation.as_str() {
                    "call" => {
                        let arguments: Array = payload.get(1).into();
                        callback.respond(arguments, client_port.clone())
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

            #[cfg(feature = "log")]
            log::info!("setting port1 on_message");

            channel
                .port1()
                .set_onmessage(Some(on_message.as_ref().unchecked_ref()));

            #[cfg(feature = "log")]
            log::info!("returning self");

            RefCell::new(Self {
                phylactery: None,
                on_message,
            })
        });

        client.borrow_mut().phylactery = Some(client.clone());

        Ok(channel.port2())
    }
}

impl Drop for CallbackClient {
    fn drop(&mut self) {
        #[cfg(feature = "log")]
        log::info!("dropping client");
    }
}

struct CallbackServer<A, R> {
    _phantom: PhantomData<(A, R)>,
    port: MessagePort,
}

impl<A: 'static, R: 'static> CallbackServer<A, R> {
    fn new(port: MessagePort) -> Self {
        Self {
            _phantom: PhantomData,
            port,
        }
    }

    fn call(&self, a: A) -> AsyncReturnWithError<R> {
        #[cfg(feature = "log")]
        log::info!("server call");

        let mut send_result = None;
        let promise = Promise::new(&mut |resolve, _reject| {
            send_result = Some(resolve);
        });
        let send_result = send_result.unwrap();

        let on_message = Closure::once_into_js(move |message: MessageEvent| {
            send_result.call1(&JsValue::NULL, &message.data())
        });
        self.port.set_onmessage(Some(&on_message.unchecked_ref()));
        let mut message = Message::new("callback");
        message.post(a);
        message.send(|message, transfer| {
            self.port
                .post_message_with_transferable(message, transfer)
                .map_err(|error| Error::CallbackFailed {
                    error: format!("failed to post message: {error:?}"),
                })
        });

        Box::new(JsFuture::from(promise).map(|result| {
            result
                .map(|result| Post::from_js_value(result))
                .map_err(|error| Error::CallbackFailed {
                    error: format!("promise rejected: {error:?}"),
                })
                .flatten()
        }))
    }

    fn to_closure(self) -> Box<dyn Fn(A) -> AsyncReturnWithError<R>> {
        Box::new(move |a| self.call(a))
    }
}

impl<A, R> Drop for CallbackServer<A, R> {
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

pub struct Callback1<A, R> {
    local: Option<Box<dyn Fn(A) -> R>>,
    remote: Option<Box<dyn Fn(A) -> AsyncReturnWithError<R>>>,
}

impl<A, R: 'static> Callback1<A, R> {
    pub fn call(&self, a: A) -> AsyncReturnWithError<R> {
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

impl<A, R> From<Box<dyn Fn(A) -> R>> for Callback1<A, R> {
    fn from(callback: Box<dyn Fn(A) -> R>) -> Self {
        Self {
            local: Some(callback),
            remote: None,
        }
    }
}

impl<A, R> From<Box<dyn Fn(A) -> AsyncReturnWithError<R>>> for Callback1<A, R> {
    fn from(callback: Box<dyn Fn(A) -> AsyncReturnWithError<R>>) -> Self {
        Self {
            local: None,
            remote: Some(callback),
        }
    }
}

// impl<T: Responder + 'static> Post for Callback<T> {
//     default fn from_js_value(value: JsValue) -> Result<Self, Error> {
//         Err(Error::ClientUnavailable)
//     }

//     default fn to_js_value(self) -> Result<JsValue, Error> {
//         Ok(CallbackClient::create(self.0)?.into())
//     }
// }

impl<A: 'static, R: 'static> Post for Callback1<A, R> {
    fn from_js_value(value: JsValue) -> Result<Self, Error> {
        #[cfg(feature = "log")]
        log::info!("callback from_js_value");
        let server = CallbackServer::new(value.into());
        Ok(server.to_closure().into())
    }

    fn to_js_value(self) -> Result<JsValue, Error> {
        #[cfg(feature = "log")]
        log::info!("callback to_js_value");
        let Some(local) = self.local else {
            return Err(Error::SerializeFailed {
                type_name: String::from(type_name::<Self>()),
                error: String::from("can't serialize callback without a local callback"),
            });
        };

        #[cfg(feature = "log")]
        log::info!("creating client");
        let result = CallbackClient::create(local).map(JsValue::from);
        #[cfg(feature = "log")]
        log::info!("returning {result:?}");
        result
    }
}

impl<A, R> Transfer for Callback1<A, R> {
    const NEEDS_TRANSFER: bool = true;
}

// impl<T: Responder + 'static> Into<JsValue> for Callback1<T> {
//     fn into(self) -> JsValue {
//         CallbackClient::create(self.0)
//             .map(JsValue::from)
//             .unwrap_or_else(|error| {
//                 #[cfg(feature = "log")]
//                 log::error!("failed to create CallbackClient: {error}");
//                 JsValue::NULL
//             })
//     }
// }

// impl<A: 'static, R: 'static> From<JsValue> for Callback<AsyncClosure<A, R>> {
//     fn from(port: JsValue) -> Self {
//         Callback::from(CallbackServer::new(port.into()).to_closure())
//     }
// }

// impl<A, RT, RE> Post for Callback1<A, Result<RT, RE>> {
//     fn from_js_value(value: wasm_bindgen::JsValue) -> Result<Self, Error> {
//         Err(Error::ClientUnavailable)
//     }

//     fn to_js_value(self) -> Result<wasm_bindgen::JsValue, Error> {
//         Err(Error::ClientUnavailable)
//     }
// }
