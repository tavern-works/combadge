use std::cell::RefCell;
use std::rc::{Rc, Weak};

use js_sys::Array;
use wasm_bindgen::prelude::*;
use web_sys::MessageEvent;

use crate::{log_error, Error, Port};

type Dispatcher = Box<dyn FnMut(&str, Array) -> Result<(), Error>>;

pub struct Server<P: Port> {
    phylactery: Option<Rc<RefCell<Self>>>,
    dispatcher: Dispatcher,
    #[expect(
        dead_code,
        reason = "We hold onto this closure's memory until the server is dropped"
    )]
    on_message: Closure<dyn Fn(MessageEvent)>,
    port: P,
}

impl<P: Port + 'static> Server<P> {
    pub fn create(port: P, dispatcher: Dispatcher) {
        let server = Rc::new_cyclic(|weak_self: &Weak<RefCell<Self>>| {
            let cloned_weak_self = weak_self.clone();
            let on_message = Closure::new(move |event: MessageEvent| {
                let Some(server) = Weak::upgrade(&cloned_weak_self) else {
                    log_error!("failed to upgrade weak server in message callback");
                    return;
                };

                let Ok(mut server) = server.try_borrow_mut() else {
                    log_error!("failed to borrow server in message callback");
                    return;
                };

                let data: Array = event.data().into();
                let procedure = data.shift().as_string().unwrap();

                if procedure == "*handshake" {
                    if let Err(error) = server.port.post_message(&JsValue::from_str("*handshake")) {
                        log_error!("error sending handshake: {error:?}");
                    }
                } else {
                    if let Err(error) = (server.dispatcher)(&procedure, data) {
                        log_error!("error dispatching {procedure}: {error}");
                    }
                }
            });

            port.set_onmessage(Some(on_message.as_ref().unchecked_ref()));

            if let Err(error) = port.post_message(&JsValue::from_str("*handshake")) {
                log_error!("error sending handshake: {error:?}");
            }

            RefCell::new(Self {
                phylactery: None,
                dispatcher,
                on_message,
                port,
            })
        });

        server.borrow_mut().phylactery = Some(server.clone());
    }
}
