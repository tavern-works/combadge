use std::cell::RefCell;
use std::rc::{Rc, Weak};

use wasm_bindgen::prelude::*;
use web_sys::{MessageEvent, Worker};

use crate::message::Post;
use crate::{Error, Message};

#[derive(Debug)]
pub struct Client {
    weak_self: Weak<RefCell<Self>>,
    on_message: Closure<dyn Fn(MessageEvent)>,
    worker: Worker,
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
            worker.post_message(&JsValue::from_str("hello from client"));

            RefCell::new(Self {
                weak_self: weak_self.clone(),
                on_message,
                worker,
                server_ready: false,
            })
        });

        client
    }

    pub async fn send_message<T>(&self, message: Message) -> Result<T, Error>
    where
        T: Post,
    {
        unimplemented!()
    }
}
