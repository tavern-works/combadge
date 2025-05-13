use anyhow::anyhow;
use combadge::prelude::*;
use interface::{Error, MessageCallback, Postable, Sample, Transferable};
use js_sys::Uint32Array;

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
    fn add(&self, a: f32, b: f32) -> Result<f32, Error> {
        Ok(a + b)
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

    fn double_postable(&self, data: Uint32Array) -> Result<Postable, Error> {
        data.set_index(100, 200);
        Ok(data.into())
    }

    fn double_transferable(&self, data: Uint32Array) -> Result<Transferable, Error> {
        data.set_index(100, 200);
        Ok(data.into())
    }
}
