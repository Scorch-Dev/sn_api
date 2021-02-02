// copyright 2021 maidsafe.net limited.
//
// this safe network software is licensed to you under the general public license (gpl), version 3.
// unless required by applicable law or agreed to in writing, the safe network software distributed
// under the gpl licence is distributed on an "as is" basis, without warranties or conditions of any
// kind, either express or implied. please review the licences for the specific language governing
// permissions and limitations relating to use of the safe network software.

///! A module for stream types to asynchronously service json
///! rpc requests received by the daemon.
///! One `RequestStream` is associated with each daemon,
///! and can be used to get requests asynchronously.
///! Each request yielded from the `RequestStream` has
///! it's own `ResponseStream` which *must* be used
///! to reply to the request.
///! (see `ResponseStream.send_oneshot()`)
use qjsonrpc::{JsonRpcRequest, JsonRpcResponse};
use tokio::sync::mpsc;

/// A stream to asynchronously get parsed json rpc requests
/// Requests are yielded via get_next() which gives the method,
/// the params, and the associated `ResponseStream` used to service
/// the request.
pub struct RequestStream {
    /// Buffered stream for outgoing responses
    response_tx: mpsc::UnboundedSender<JsonRpcResponse>,

    /// Buffered stream for incoming queries
    request_rx: mpsc::UnboundedReceiver<JsonRpcRequest>,
}

impl RequestStream {
    /// Ctor
    pub fn new(
        response_tx: mpsc::UnboundedSender<JsonRpcResponse>,
        request_rx: mpsc::UnboundedReceiver<JsonRpcRequest>,
    ) -> Self {
        Self {
            response_tx,
            request_rx,
        }
    }

    /// Returns tuple (request, ResponseStream) or None if all senders
    /// are dropped.
    pub async fn get_next(&mut self) -> Option<(JsonRpcRequest, ResponseStream)> {
        let req = self.request_rx.recv().await?;
        let resp_stream = ResponseStream::new(self.response_tx.clone(), req.id);
        Some((req, resp_stream))
    }
}

/// An associated type for Request stream used to respond to
/// requests once they've been serviced.
///
/// The function `send_oneshot()` *must* be called by the server
/// before the response stream is dropped, as the current `qjsonrpc`
/// implementation doesn't support requests with no response.
pub struct ResponseStream {
    /// channel to buffer the output stream
    response_tx: mpsc::UnboundedSender<JsonRpcResponse>,

    /// ensure response id matches the original request id
    id: u32,

    /// ensure that one_shot() was called before `drop()`
    was_consumed: bool,
}

impl ResponseStream {
    /// Ctor
    fn new(response_tx: mpsc::UnboundedSender<JsonRpcResponse>, id: u32) -> Self {
        let was_consumed = false;
        Self {
            response_tx,
            id,
            was_consumed,
        }
    }

    /// Send a response along the pipeline
    /// and consumes the stream in the process.
    /// The response id should match the id of the request
    /// yielded along with it by `RequestStream.get_next()`.
    ///
    /// Implementation Note:
    /// `response_tx.send()` can error, but it should
    /// never do so here logically (hence the `assert!`).
    /// If send does `Error` we have one of two cases.
    ///     a) `RpcDaemon.run()` returned already.
    ///         This case isn't possible because it only exits
    ///         when all senders (like this stream) are dropped
    ///     b) You somehow got a `ResponseStream` that wasn't
    ///        yielded from a `RequestStream` (also not possible)
    pub fn send_oneshot(mut self, resp: JsonRpcResponse) {
        assert_eq!(resp.id().unwrap(), self.id);
        self.was_consumed = true;
        let r = self.response_tx.send(resp);
        assert!(r.is_ok());
    }
}

impl Drop for ResponseStream {
    /// If a response stream was yielded from `get_next()`
    /// the server is not allowed to ignore it
    /// as per the JSON RPC 2.0 spec
    fn drop(&mut self) {
        assert!(self.was_consumed);
    }
}
