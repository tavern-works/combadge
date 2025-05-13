use anyhow::anyhow;
use interface::{Error, SampleClient};
use js_sys::{Function, Uint32Array};
use wasm_bindgen::prelude::*;
use web_sys::Worker;

#[wasm_bindgen]
pub struct Client {
    client: SampleClient<Worker>,
}

#[wasm_bindgen]
impl Client {
    #[wasm_bindgen(constructor)]
    pub fn new(worker: Worker) -> Self {
        if let Err(error) = console_log::init_with_level(log::Level::Trace) {
            log::error!("error initializing log: {:?}", error);
        }

        std::panic::set_hook(Box::new(|info| log::error!("{}", info)));

        let client = SampleClient::new(worker);
        Self { client }
    }

    pub async fn add(&self, a: f32, b: f32) -> Result<f32, Error> {
        self.client
            .add(a, b)
            .await
            .map_err(Error::from)
            .and_then(|r| r)
    }

    pub async fn parse(&self, string: String) -> Result<i32, Error> {
        self.client
            .parse(string)
            .await
            .map_err(Error::from)
            .and_then(|r| r)
    }

    #[wasm_bindgen(js_name = callWithMessage)]
    pub async fn call_with_message(&self, callback: Function, message: String) {
        let callback = Box::new(move |message: String| {
            callback
                .call1(&JsValue::NULL, &message.into())
                .map(|_| ())
                .map_err(|error| Error::from(anyhow!("{error:?}")))
        }) as Box<dyn Fn(String) -> Result<(), Error>>;

        if let Err(error) = self
            .client
            .call_with_message(callback.into(), message)
            .await
        {
            log::error!("error while calling callback: {error}")
        };
    }

    #[wasm_bindgen(js_name = blockThread)]
    pub async fn block_thread(&self) -> Result<(), Error> {
        self.client
            .block_thread()
            .await
            .map_err(Error::from)
            .and_then(|r| r)
    }

    #[wasm_bindgen(js_name = doublePostable)]
    pub async fn double_postable(&self, data: Uint32Array) -> Result<Uint32Array, Error> {
        self.client
            .double_postable(data)
            .await
            .map_err(Error::from)
            .and_then(|result| result.map(|postable| postable.data))
    }

    #[wasm_bindgen(js_name = doubleTransferable)]
    pub async fn double_transferable(&self, data: Uint32Array) -> Result<Uint32Array, Error> {
        self.client
            .double_transferable(data)
            .await
            .map_err(Error::from)
            .and_then(|result| result.map(|transferable| transferable.data))
    }
}
