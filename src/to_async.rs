use std::future::Future;

use futures::future::ready;

pub trait ToAsync<T> {
    fn to_async(self: Box<Self>) -> Box<dyn Future<Output = T>>;
}

impl<T: Sized + 'static> ToAsync<T> for T {
    default fn to_async(self: Box<Self>) -> Box<dyn Future<Output = T>> {
        Box::new(ready(*self))
    }
}

impl<T: Sized + 'static> ToAsync<T> for Box<dyn Future<Output = T>> {
    fn to_async(self: Box<Self>) -> Box<dyn Future<Output = T>> {
        *self
    }
}

// impl<T: 'static> From<T> for Box<dyn ToAsync<T>> {
//     fn from(value: T) -> Self {
//         Box::new(value)
//     }
// }

pub trait MaybeAsync<T> {
    fn to_maybe_async(self) -> Box<dyn Future<Output = T>>;
}

impl<T: Sized + 'static> MaybeAsync<T> for T {
    default fn to_maybe_async(self) -> Box<dyn Future<Output = T>> {
        Box::new(ready(self))
    }
}

impl<T: Sized + 'static> MaybeAsync<T> for Box<dyn ToAsync<T>> {
    fn to_maybe_async(self) -> Box<dyn Future<Output = T>> {
        self.to_async()
    }
}
