use std::cell::RefCell;
use std::rc::{Rc, Weak};

use js_sys::Array;
use wasm_bindgen::prelude::*;
use web_sys::js_sys::global;
use web_sys::{DedicatedWorkerGlobalScope, MessageEvent};

use crate::Error;

type Dispatcher = Box<dyn FnMut(&str, Array) -> Result<(), Error>>;

pub struct Server {
    weak_self: Weak<RefCell<Self>>,
    dispatcher: Dispatcher,
    on_message: Closure<dyn Fn(MessageEvent)>,
    scope: DedicatedWorkerGlobalScope,
    client_ready: bool,
}

impl Server {
    pub fn new(dispatcher: Dispatcher) -> Rc<RefCell<Self>> {
        let global: JsValue = global().into();
        let scope: DedicatedWorkerGlobalScope = global.into();

        Rc::new_cyclic(|weak_self: &Weak<RefCell<Self>>| {
            let cloned_weak_self = weak_self.clone();
            let on_message = Closure::new(move |event: MessageEvent| {
                let Some(server) = Weak::upgrade(&cloned_weak_self) else {
                    #[cfg(feature = "log")]
                    log::error!("Failed to upgrade weak server in message callback");
                    return;
                };

                let Ok(mut server) = server.try_borrow_mut() else {
                    #[cfg(feature = "log")]
                    log::error!("Failed to borrow server in message callback");
                    return;
                };

                if let Err(error) = server.dispatch(event) {
                    #[cfg(feature = "log")]
                    log::error!("error while dispatching message: {error}");
                }
            });

            #[cfg(feature = "log")]
            log::info!("Setting onmessage on server");
            scope.set_onmessage(Some(on_message.as_ref().unchecked_ref()));

            #[cfg(feature = "log")]
            log::info!("Sending hello to client");
            scope.post_message(&JsValue::from_str("hello from server"));

            RefCell::new(Self {
                weak_self: weak_self.clone(),
                dispatcher,
                on_message,
                scope,
                client_ready: false,
            })
        })
    }

    pub fn dispatch(&mut self, message: MessageEvent) -> Result<(), Error> {
        #[cfg(feature = "log")]
        log::info!("Server received message: {message:?} {:?}", message.data());

        let data: Array = message.data().into();
        let procedure = data.shift().as_string().unwrap();
        (self.dispatcher)(&procedure, data)
    }
}
