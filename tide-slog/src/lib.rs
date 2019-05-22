#![feature(async_await)]
#![warn(
    nonstandard_style,
    rust_2018_idioms,
    future_incompatible,
    missing_debug_implementations
)]

use slog::{info, o, trace, Drain, Logger};
use slog_async;
use slog_term;

use futures::future::BoxFuture;
use futures::prelude::*;

use tide_core::{
    middleware::{Middleware, Next},
    Context, Response,
};

/// RequestLogger based on slog.SimpleLogger
#[derive(Debug)]
pub struct RequestLogger {
    // drain: dyn slog::Drain,
    inner: Logger,
}

impl RequestLogger {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn with_logger(logger: Logger) -> Self {
        Self { inner: logger }
    }
}

impl Default for RequestLogger {
    fn default() -> Self {
        let decorator = slog_term::TermDecorator::new().build();
        let drain = slog_term::CompactFormat::new(decorator).build().fuse();
        let drain = slog_async::Async::new(drain).build().fuse();

        let log = Logger::root(drain, o!());
        Self { inner: log }
    }
}

fn request_id() -> String {
    bs58::encode(uuid::Uuid::new_v4().as_bytes()).into_string()
}

/// Stores information during request phase and logs information once the response
/// is generated.
impl<State: Send + Sync + 'static> Middleware<State> for RequestLogger {
    fn handle<'a>(&'a self, mut cx: Context<State>, next: Next<'a, State>) -> BoxFuture<'a, Response> {
        FutureExt::boxed(async move {
            let logger = self.inner.new(o!("request" => request_id()));
            let path = cx.uri().path().to_owned();
            let method = cx.method().as_str().to_owned();
            trace!(logger, "IN => {} {}", method, path);
            let start = std::time::Instant::now();
            cx.extensions_mut().insert(logger.clone());
            let res = next.run(cx).await;
            let status = res.status();
            info!(
                logger,
                "{} {} {} {}ms",
                method,
                path,
                status.as_str(),
                start.elapsed().as_millis()
            );
            res
        })
    }
}

/// An extension to [`Context`] that provides access to a request scoped logger
pub trait ContextExt {
    /// returns a [`Logger`] scoped to this request
    fn logger(&mut self) -> &Logger;
}

impl<State> ContextExt for Context<State> {
    fn logger(&mut self) -> &Logger {
        self.extensions()
            .get::<Logger>()
            .expect("RequestLogger must be used to populate request logger")
            // TODO ^ should this be an expect or return an error?
    }
}
