use futures::{
    channel::oneshot,
    future::{BoxFuture, FutureExt, TryFutureExt},
};
use http::status::StatusCode;
use std::{
    any::Any,
    future::Future,
    panic::{AssertUnwindSafe, RefUnwindSafe},
    pin::Pin,
    sync::Mutex,
    task::{self, Poll},
};
use tide_core::{
    middleware::{Middleware, Next},
    response::IntoResponse,
    Context, Response,
};

/// A [`Middleware`] that will catch any panics from later middleware or handlers and route them to
/// a handle to be resumed elsewhere.
#[derive(Debug)]
pub struct PropagateUnwind {
    tx: Mutex<Option<oneshot::Sender<Box<dyn Any + Send + 'static>>>>,
}

#[must_use]
#[derive(Debug)]
/// A handle to the panic caught by a [`PropagateUnwind`]. Implements [`Future`] and will resume
/// unwinding with the panic if one is caught.
pub struct UnwindHandle {
    rx: oneshot::Receiver<Box<dyn Any + Send + 'static>>,
}

impl PropagateUnwind {
    /// Create a [`PropagateUnwind`] along with its associated [`UnwindHandle`].
    pub fn new() -> (Self, UnwindHandle) {
        let (tx, rx) = oneshot::channel();
        let tx = Mutex::new(Some(tx));
        (Self { tx }, UnwindHandle { rx })
    }
}

impl<State: RefUnwindSafe + 'static> Middleware<State> for PropagateUnwind {
    fn handle<'a>(&'a self, cx: Context<State>, next: Next<'a, State>) -> BoxFuture<'a, Response> {
        AssertUnwindSafe(next.run(cx))
            .catch_unwind()
            .unwrap_or_else(move |err| {
                let tx = self
                    .tx
                    .lock()
                    .ok()
                    .and_then(|mut guard| guard.take())
                    .unwrap();
                tx.send(err).unwrap();
                "Internal server error"
                    .with_status(StatusCode::INTERNAL_SERVER_ERROR)
                    .into_response()
            })
            .boxed()
    }
}

impl Future for UnwindHandle {
    type Output = Box<dyn Any + Send + 'static>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut task::Context<'_>) -> Poll<Self::Output> {
        self.rx.poll_unpin(cx).map(|o| o.expect("polled after canceled"))
    }
}
