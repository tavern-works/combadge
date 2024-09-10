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
                    log::error!("failed to upgrade weak server in message callback");
                    return;
                };

                let Ok(mut server) = server.try_borrow_mut() else {
                    #[cfg(feature = "log")]
                    log::error!("failed to borrow server in message callback");
                    return;
                };

                let data: Array = event.data().into();
                let procedure = data.shift().as_string().unwrap();

                if procedure == "*handshake" {
                    if let Err(error) = server.scope.post_message(&JsValue::from_str("*handshake"))
                    {
                        #[cfg(feature = "log")]
                        log::error!("error sending handshake: {error:?}");
                    }
                } else {
                    if let Err(error) = (server.dispatcher)(&procedure, data) {
                        #[cfg(feature = "log")]
                        log::error!("error dispatching {procedure}: {error}");
                    }
                }
            });

            scope.set_onmessage(Some(on_message.as_ref().unchecked_ref()));

            if let Err(error) = scope.post_message(&JsValue::from_str("*handshake")) {
                #[cfg(feature = "log")]
                log::error!("error sending handshake: {error:?}");
            }

            RefCell::new(Self {
                weak_self: weak_self.clone(),
                dispatcher,
                on_message,
                scope,
                client_ready: false,
            })
        })
    }
}
