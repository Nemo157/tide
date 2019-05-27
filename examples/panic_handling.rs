#![feature(async_await)]

//! An example of different panic handling strategies.
//!
//! This example runs two different web applications on different ports, on port 8000 it uses the
//! [`CatchUnwind`] middleware to convert any panics caught into an internal server error; on port
//! 8001 it uses the [`PropagateUnwind`] middleware to catch any panics and then tear down the
//! application and exit.
//!
//! When running it notice that you can make multiple requests on port 8000, but after the first
//! request on 8001 it will no longer be there:
//!
//! ```console
//! > cargo run --example panic_handling
//! ```
//!
//! ```console
//! > curl -i http://localhost:8000
//! HTTP/1.1 500 Internal Server Error
//! content-type: text/plain; charset=utf-8
//! transfer-encoding: chunked
//! date: Mon, 27 May 2019 18:18:10 GMT
//!
//! Internal server error
//!
//! > curl -i http://localhost:8000
//! HTTP/1.1 500 Internal Server Error
//! content-type: text/plain; charset=utf-8
//! transfer-encoding: chunked
//! date: Mon, 27 May 2019 18:18:12 GMT
//!
//! Internal server error
//!
//! > curl -i http://localhost:8001
//! HTTP/1.1 500 Internal Server Error
//! content-type: text/plain; charset=utf-8
//! transfer-encoding: chunked
//! date: Mon, 27 May 2019 18:18:14 GMT
//!
//! Internal server error
//!
//! > curl -i http://localhost:8001
//! curl: (7) Failed to connect to localhost port 8001: Connection refused
//! ```

use futures::{
    future::{FutureExt, TryFutureExt},
    select,
};
use tide_panic::{CatchUnwind, PropagateUnwind};
use pin_utils::pin_mut;

fn main() {
    tokio::run(
        async {
            let mut soft_fail = tide::App::new();
            soft_fail.middleware(CatchUnwind::new());
            soft_fail
                .at("/")
                .get(async move |_| panic!("Hello, world!"));

            let mut hard_fail = tide::App::new();
            let (propagate, panic_handle) = PropagateUnwind::new();
            hard_fail.middleware(propagate);
            hard_fail
                .at("/")
                .get(async move |_| panic!("Hello, world!"));

            let soft_fail = soft_fail.serve("127.0.0.1:8000").fuse();
            let hard_fail = hard_fail.serve("127.0.0.1:8001").fuse();

            pin_mut!(soft_fail);
            pin_mut!(hard_fail);
            let mut panic_handle = panic_handle.fuse();
            select! {
                _ = soft_fail => {
                    panic!("soft fail application unexpectedly exited");
                }
                _ = hard_fail => {
                    panic!("hard fail application unexpectedly exited");
                }
                err = panic_handle => {
                    std::panic::resume_unwind(err);
                }
            }
        }
            .boxed()
            .unit_error()
            .compat(),
    );
}
