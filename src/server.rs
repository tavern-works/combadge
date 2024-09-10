use std::cell::RefCell;
use std::rc::Rc;

use wasm_bindgen::prelude::*;
use web_sys::js_sys::global;
use web_sys::{DedicatedWorkerGlobalScope, MessageEvent};

pub struct Server {
    on_message: Closure<dyn Fn(MessageEvent)>,
    scope: DedicatedWorkerGlobalScope,
    client_ready: bool,
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

        Rc::new(RefCell::new(Self {
            on_message,
            scope,
            client_ready: false,
        }))
    }
}
