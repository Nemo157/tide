use std::sync::Arc;

use futures::future::BoxFuture;
use futures::prelude::*;

use crate::{endpoint::BoxedEndpoint, Request, Response, RouteMatch};

mod default_headers;
pub mod logger;

pub use self::default_headers::DefaultHeaders;

/// Middleware that wraps around remaining middleware chain.
pub trait Middleware<Data>: Send + Sync {
    /// Asynchronously handle the request, and return a response.
    fn handle<'a>(&'a self, ctx: RequestContext<'a, Data>) -> BoxFuture<'a, Response>;
}

impl<Data, F> Middleware<Data> for F
where
    F: Send + Sync + Fn(RequestContext<Data>) -> BoxFuture<Response>,
{
    fn handle<'a>(&'a self, ctx: RequestContext<'a, Data>) -> BoxFuture<'a, Response> {
        (self)(ctx)
    }
}

pub struct RequestContext<'a, Data> {
    pub app_data: Data,
    pub req: Request,
    pub params: RouteMatch<'a>,
    pub(crate) endpoint: &'a BoxedEndpoint<Data>,
    pub(crate) next_middleware: &'a [Arc<dyn Middleware<Data> + Send + Sync>],
}

impl<'a, Data: Clone + Send> RequestContext<'a, Data> {
    /// Consume this context, and run remaining middleware chain to completion.
    pub fn next(mut self) -> BoxFuture<'a, Response> {
        async move {
            if let Some((current, next)) = self.next_middleware.split_first() {
                self.next_middleware = next;
                await!(current.handle(self))
            } else {
                await!(self
                    .endpoint
                    .call(self.app_data.clone(), self.req, self.params))
            }
        }.boxed()
    }
}
