# Introduction

Combadge is a Rust library inspired by [Comlink](https://github.com/GoogleChromeLabs/comlink) which aims to make it as easy as possible for people developing Rust-on-WebAssembly applications to perform remote procedure calls in [web workers](https://developer.mozilla.org/en-US/docs/Web/API/Web_Workers_API/Using_web_workers) while allowing the caller and callee to remain in idiomatic Rust code.

Whereas Comlink was designed to abstract away the `postMessage` interface underlying communication between the main browser thread and web workers, there is still a lot of plumbing when going through the [wasm-bindgen](https://github.com/rustwasm/wasm-bindgen) layer between Comlink and Rust code. The goal of Combadge is to eliminate as much of that overhead as possible while still providing the benefits of being able to offload work to another thread.

# Example

To start, let's define a simple interface that we'd like to offload to another thread:

```rust
trait Interface {
    fn add(&self, a: f32, b: f32) -> f32;
}
```

In order to take advantage of Combadge for this method, the first thing we need to do is apply the `combadge` macro to the trait definition:

```rust
use combadge::prelude::*;

#[combadge]
trait Interface {
    fn add(&self, a: f32, b: f32) -> f32;
}
```

When we apply the `combadge` macro, Combadge will generate two new structs that we can use based on the name of the trait. In this case, we'll get an `InterfaceClient` and an `InterfaceServer`. The server will add a message handler to the worker thread to listen for messages from the client, which will live on the main thread. The server will also take as a constructor argument a local implementation of the `Interface` trait to which it will delegate incoming procedure calls. For this example, this implementation will look like this:

```rust
#[derive(Default)]
pub(super) struct Local {}

impl Interface for Local {
    fn add(&self, a: f32, b: f32) -> f32 {
        a + b
    }
}
```

In order to launch the server, we'll need just a little bit of boilerplate (which can of course be adapted to your particular workflow). Here's what a minimal version of the [sample app](https://how.tavern.works/combadge/) would look like:

```rust
use js_sys::global;
use wasm_bindgen::prelude::*;
use web_sys::DedicatedWorkerGlobalScope;

mod local;

#[wasm_bindgen]
pub fn run() {
    let scope: DedicatedWorkerGlobalScope = global().dyn_into().unwrap();
    InterfaceServer::create(local::Local::default(), scope);
}
```

And in the TypeScript file we'll use to bootstrap the server:

```typescript
import init, { run } from "./worker/pkg";

init().then(() => {
    run();
});
```

How exactly you want to use the client will depend on your particular application, but let's just assume we want to do a single addition once we get everything set up to make sure things are working as expected. First, let's build a Client struct we can call from TypeScript. We'll need the constructor to take the JS `Worker` and use it to initialize the client:

```rust
use wasm_bindgen::prelude::*;
use web_sys::Worker;

#[wasm_bindgen]
pub struct Client {
    client: InterfaceClient<Worker>,
}

#[wasm_bindgen]
impl Client {
    #[wasm_bindgen(constructor)]
    pub fn new(worker: Worker) -> Self {
        Self {
            client: InterfaceClient::new(worker),
        }
    }
}
```

> You might have noticed that `InterfaceClient` is parameterized over `Worker`. This is because under the covers, there's a `combadge::Client` struct that is also used for callbacks, in which case it receives a `MessagePort` instead of a `Worker` instance.

On the TypeScript side, we can create this client like this:

```typescript
import init, { Client } from "./main/pkg";

init().then(() => {
    const worker = new Worker(new URL("./worker.ts", import.meta.url), {
        type: "module",
    });

    const client = new Client(worker);
});
```

Finally, we can add a proxy method to our `Client` struct to actually perform the addition. On the Rust side, it looks like this:

```rust
#[wasm_bindgen]
impl Client {
    // ...

    pub async fn add(&self, a: f32, b: f32) -> Result<f32, String> {
        self.client
            .add(a, b)
            .await
            .map_err(|error| error.to_string())
    }
}
```

"Now wait a minute," you might be saying, "I never said anything in my interface about `add` being an `async` method," and you would be correct. However, given that communication between the main thread and worker threads is asynchronous, any calls across our interface must also be asynchronous. On the client side, this means that our method that returns an `f32` actually returns a `Future<Output = Result<f32, combadge::Error>>`.

> You might be wondering what this means for methods that are asynchronous on the server side. Combadge supports these as well, but in order to simplify the code generation, you have to be specific and have methods return something spelled `Box<dyn Future<Output = ...>>` (without any `type` definitions) so the macro (which can only see the text and not any type information) can detect it and merge it with the `Future` it's already returning.

With this in place, we're almost done. We just need to call this new method in TypeScript:

```typescript
// ...
const client = new Client(worker);

client.add(3, 5).then((sum) => console.log(`3 + 5 = ${sum}`));
```

If you want to see this all together, along with a few more example methods, check out the [source](https://github.com/tavern-works/combadge/tree/main/sample) of the [sample app](https://how.tavern.works/combadge/).
