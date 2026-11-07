use super::CoreError;
use futures_core::Stream;
use std::{future::Future, pin::Pin};
pub type ServiceFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;
pub type EventStream<T> = Pin<Box<dyn Stream<Item = Result<T, CoreError>> + Send>>;
