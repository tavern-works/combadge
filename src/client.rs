use std::cell::RefCell;
use std::rc::{Rc, Weak};

use js_sys::{Array, Promise};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use web_sys::{MessageChannel, MessageEvent, Worker};

use crate::message::Post;
use crate::{Error, Message};

#[derive(Debug)]
pub struct Client {
    weak_self: Weak<RefCell<Self>>,
    on_message: Closure<dyn Fn(MessageEvent)>,
    pub worker: Worker,
    server_ready: bool,
}

impl Client {
    pub fn new(worker: Worker) -> Rc<RefCell<Self>> {
        let client = Rc::new_cyclic(|weak_self| {
            let on_message = Closure::new(|event: MessageEvent| {
                #[cfg(feature = "log")]
                log::info!("Client received message: {event:?}");
            });

            #[cfg(feature = "log")]
            log::info!("Setting onmessage on client");
            worker.set_onmessage(Some(on_message.as_ref().unchecked_ref()));

            #[cfg(feature = "log")]
            log::info!("Sending hello to server");
            worker.post_message(&Array::of1(&JsValue::from_str("hello from client")));

            RefCell::new(Self {
                weak_self: weak_self.clone(),
                on_message,
                worker,
                server_ready: false,
            })
        });

        client
    }

    pub async fn send_message<T>(worker: Worker, mut message: Message) -> Result<T, Error>
    where
        T: Post + std::fmt::Debug,
    {
        let channel = MessageChannel::new().map_err(|error| Error::CreationFailed {
            type_name: String::from("MessageChannel"),
            error: format!("{error:?}"),
        })?;

        let promise = Promise::new(&mut |resolve, _reject| {
            let callback = Closure::once_into_js(move |message: MessageEvent| {
                let _ = resolve.call1(&JsValue::NULL, &message.data());
            });

            channel
                .port2()
                .set_onmessage(Some(callback.as_ref().unchecked_ref()));
        });

        message.transfer(channel.port1());
        message.send(|message, transfer| {
            worker
                .post_message_with_transfer(message, transfer)
                .map_err(|error| Error::PostFailed {
                    error: format!("{error:?}"),
                })
        })?;

        JsFuture::from(promise)
            .await
            .map_err(|error| Error::ReceiveFailed {
                error: format!("{error:?}"),
            })
            .and_then(|result| T::from_js_value(result))
    }
}
