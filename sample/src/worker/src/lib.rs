use interface::SampleServer;
use js_sys::global;
use wasm_bindgen::prelude::*;
use web_sys::DedicatedWorkerGlobalScope;

mod local;

#[wasm_bindgen]
pub fn run() {
    if let Err(error) = console_log::init_with_level(log::Level::Trace) {
        log::error!("error initializing log: {:?}", error);
    }

    std::panic::set_hook(Box::new(|info| log::error!("{}", info)));

    let scope: DedicatedWorkerGlobalScope = global().dyn_into().unwrap();
    SampleServer::create(local::Local::default(), scope);
}
