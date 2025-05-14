use anyhow::anyhow;
use combadge::prelude::*;
use interface::{Error, MessageCallback, Postable, Sample, Transferable};
use js_sys::{Date, Promise, Uint32Array, global};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use web_sys::DedicatedWorkerGlobalScope;

fn collatz(value: u64, steps: usize) -> usize {
    if value == 1 {
        return steps;
    }

    if value % 2 == 0 {
        collatz(value / 2, steps + 1)
    } else {
        collatz(3 * value + 1, steps + 1)
    }
}

#[derive(Default)]
pub(super) struct Local {}

impl Sample for Local {
    fn add(&self, a: f32, b: f32) -> f32 {
        a + b
    }

    fn parse(&self, string: String) -> Result<i32, Error> {
        string
            .parse()
            .map_err(|error| anyhow!("failed to parse '{string}': {error}").into())
    }

    fn call_with_message(&self, callback: MessageCallback, message: String) {
        wasm_bindgen_futures::spawn_local(async move {
            if let Err(error) = callback.call(message).await {
                log::error!("error while calling message callback: {error}");
            }
        });
    }

    fn block_thread(&self) -> Result<(), Error> {
        for n in 2..5_000_000 {
            if collatz(n, 0) == 0 {
                return Err(anyhow!("collatz should take at least 1 step").into());
            }
        }

        Ok(())
    }

    fn double_postable(&self, data: Uint32Array) -> Postable {
        data.set_index(100, 200);
        data.into()
    }

    fn double_transferable(&self, data: Uint32Array) -> Transferable {
        data.set_index(100, 200);
        data.into()
    }

    fn get_future(&self) -> Box<dyn Future<Output = String>> {
        let mut resolve = None;
        let promise = Promise::new(&mut |res, _| {
            resolve = Some(res);
        });
        let resolve = resolve.unwrap();

        let future = JsFuture::from(promise);
        let result = async {
            future.await.map_or_else(
                |error| format!("error: {error:?}"),
                |result| result.as_string().unwrap(),
            )
        };

        let scope: DedicatedWorkerGlobalScope = global().dyn_into().unwrap();
        let _ = scope.set_timeout_with_callback_and_timeout_and_arguments_0(
            Closure::once_into_js(move || {
                resolve.call1(&JsValue::NULL, &Date::new_0().to_time_string())
            })
            .as_ref()
            .unchecked_ref(),
            1000,
        );

        Box::new(result)
    }
}
