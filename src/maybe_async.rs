use std::future::Future;

use futures::future::ready;

pub trait MaybeAsync<T> {
    fn to_maybe_async(self) -> Box<dyn Future<Output = T>>;
}

impl<T: Sized + 'static> MaybeAsync<T> for T {
    default fn to_maybe_async(self) -> Box<dyn Future<Output = T>> {
        Box::new(ready(self))
    }
}

impl<T: Sized + 'static> MaybeAsync<T> for Box<dyn Future<Output = T>> {
    fn to_maybe_async(self) -> Box<dyn Future<Output = T>> {
        self
    }
}
