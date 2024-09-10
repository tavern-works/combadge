extern crate combadge_macros;

use std::cell::RefCell;
use std::rc::{Rc, Weak};

use wasm_bindgen::prelude::*;
use web_sys::js_sys::global;
use web_sys::{DedicatedWorkerGlobalScope, MessageEvent, Worker};

pub mod prelude {
    pub use ::web_sys;
    pub use combadge_macros::combadge;

    pub use crate::{Client, Server};
}

#[derive(Debug)]
pub struct Client {
    weak_self: Weak<RefCell<Self>>,
    on_message: Closure<dyn Fn(MessageEvent)>,
    worker: Worker,
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
            })
        });

        client
    }
}

pub struct Server {
    on_message: Closure<dyn Fn(MessageEvent)>,
}

impl Server {
    pub fn new() -> Rc<RefCell<Self>> {
        let global: JsValue = global().into();
        let scope: DedicatedWorkerGlobalScope = global.into();

        let on_message = Closure::new(|event: MessageEvent| {
            #[cfg(feature = "log")]
            log::info!("Server received message: {event:?}");
        });

        #[cfg(feature = "log")]
        log::info!("Setting onmessage on server");
        scope.set_onmessage(Some(on_message.as_ref().unchecked_ref()));

        #[cfg(feature = "log")]
        log::info!("Sending hello to client");
        scope.post_message(&JsValue::from_str("hello from server"));

        Rc::new(RefCell::new(Self { on_message }))
    }
}
