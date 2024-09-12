use js_sys::Function;
use wasm_bindgen::JsValue;
use web_sys::{DedicatedWorkerGlobalScope, MessagePort, Worker};

pub trait Port {
    fn set_onmessage(&self, value: Option<&Function>);
    fn post_message(&self, message: &JsValue) -> Result<(), JsValue>;
    fn post_message_with_transfer(
        &self,
        message: &JsValue,
        transfer: &JsValue,
    ) -> Result<(), JsValue>;
}

impl Port for DedicatedWorkerGlobalScope {
    fn set_onmessage(&self, value: Option<&Function>) {
        self.set_onmessage(value)
    }

    fn post_message(&self, message: &JsValue) -> Result<(), JsValue> {
        self.post_message(message)
    }

    fn post_message_with_transfer(
        &self,
        message: &JsValue,
        transfer: &JsValue,
    ) -> Result<(), JsValue> {
        self.post_message_with_transfer(message, transfer)
    }
}

impl Port for MessagePort {
    fn set_onmessage(&self, value: Option<&Function>) {
        self.set_onmessage(value)
    }

    fn post_message(&self, message: &JsValue) -> Result<(), JsValue> {
        self.post_message(message)
    }

    fn post_message_with_transfer(
        &self,
        message: &JsValue,
        transfer: &JsValue,
    ) -> Result<(), JsValue> {
        self.post_message_with_transferable(message, transfer)
    }
}

impl Port for Worker {
    fn set_onmessage(&self, value: Option<&Function>) {
        self.set_onmessage(value)
    }

    fn post_message(&self, message: &JsValue) -> Result<(), JsValue> {
        self.post_message(message)
    }

    fn post_message_with_transfer(
        &self,
        message: &JsValue,
        transfer: &JsValue,
    ) -> Result<(), JsValue> {
        self.post_message_with_transfer(message, transfer)
    }
}
