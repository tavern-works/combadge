use std::cell::RefCell;
use std::future::{ready, Future};
use std::rc::{Rc, Weak};

use futures::{FutureExt, TryFutureExt};
use js_sys::{Array, Function, Promise};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use web_sys::{MessageChannel, MessageEvent, Worker};

use crate::message::Post;
use crate::{Error, Message};

#[derive(Debug)]
pub struct Client {
    weak_self: Weak<RefCell<Self>>,
    #[expect(
        unused,
        reason = "The closure needs to be held in memory even though it isn't read"
    )]
    on_message: Closure<dyn Fn(MessageEvent)>,
    pub worker: Worker,
    server_ready: bool,
    on_ready: Vec<Function>,
}

impl Client {
    pub fn new(worker: Worker) -> Rc<RefCell<Self>> {
        Rc::new_cyclic(|weak_self: &Weak<RefCell<Self>>| {
            let cloned_weak_self = weak_self.clone();
            let on_message = Closure::new(move |event: MessageEvent| {
                if let Some(message) = event.data().as_string() {
                    if message == "*handshake" {
                        #[cfg(feature = "log")]
                        log::info!("received handshake from server");

                        let Some(client) = Weak::upgrade(&cloned_weak_self) else {
                            #[cfg(feature = "log")]
                            log::error!("failed to upgrade weak client in message callback");
                            return;
                        };

                        // Contain the borrow to a smaller scope so that the callbacks aren't called until
                        // after we drop it
                        let on_ready = {
                            let Ok(mut client) = client.try_borrow_mut() else {
                                #[cfg(feature = "log")]
                                log::error!("failed to borrow client in message callback");
                                return;
                            };

                            client.server_ready = true;
                            client.on_ready.drain(..).collect::<Vec<_>>()
                        };

                        for on_ready in on_ready {
                            if let Err(error) = on_ready.call0(&JsValue::NULL) {
                                #[cfg(feature = "log")]
                                log::error!("failed to call on_ready callback in message callback: {error:?}");
                            }
                        }
                    }
                }
            });

            #[cfg(feature = "log")]
            log::info!("setting onmessage on client");
            worker.set_onmessage(Some(on_message.as_ref().unchecked_ref()));

            #[cfg(feature = "log")]
            log::info!("sending handshake to server");
            if let Err(error) = worker.post_message(&Array::of1(&JsValue::from_str("*handshake"))) {
                #[cfg(feature = "log")]
                log::error!("error sending handshake: {error:?}");
            }

            RefCell::new(Self {
                weak_self: weak_self.clone(),
                on_message,
                worker,
                server_ready: false,
                on_ready: Vec::new(),
            })
        })
    }

    pub fn wait_for_server(&mut self) -> impl Future<Output = ()> {
        if self.server_ready {
            return ready(()).left_future();
        }

        let mut on_ready = None;
        let promise = Promise::new(&mut |resolve, _reject| {
            on_ready = Some(resolve);
        });

        if let Some(on_ready) = on_ready {
            self.on_ready.push(on_ready)
        }

        let future = JsFuture::from(promise).map(|result| {
            result.map_or_else(
                |error| {
                    #[cfg(feature = "log")]
                    log::error!("error in wait_for_server future: {error:?}");
                },
                |_| (),
            )
        });

        future.right_future()
    }

    pub fn send_message<T>(
        &mut self,
        mut message: Message,
    ) -> impl Future<Output = Result<T, Error>>
    where
        T: Post + Sized + 'static,
    {
        let channel = MessageChannel::new().map_err(|error| Error::CreationFailed {
            type_name: String::from("MessageChannel"),
            error: format!("{error:?}"),
        });

        let promise = channel.and_then(|channel| {
            let promise = Promise::new(&mut |resolve, _reject| {
                let callback = Closure::once_into_js(move |message: MessageEvent| {
                    let _ = resolve.call1(&JsValue::NULL, &message.data());
                });

                channel
                    .port2()
                    .set_onmessage(Some(callback.as_ref().unchecked_ref()));
            });

            message.transfer(channel.port1()).and_then(|()| {
                message
                    .send(|message, transfer| {
                        self.worker
                            .post_message_with_transfer(message, transfer)
                            .map_err(|error| Error::PostFailed {
                                error: format!("{error:?}"),
                            })
                    })
                    .and_then(|()| Ok(promise))
            })
        });

        async {
            promise.map(|promise| {
                JsFuture::from(promise).map(|result| {
                    result
                        .map_err(|error| Error::ReceiveFailed {
                            error: format!("{error:?}"),
                        })
                        .and_then(|result| T::from_js_value(result))
                })
            })
        }
        .try_flatten()
        .into_future()
    }
}
