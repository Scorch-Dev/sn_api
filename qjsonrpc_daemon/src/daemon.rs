// copyright 2021 maidsafe.net limited.
//
// this safe network software is licensed to you under the general public license (gpl), version 3.
// unless required by applicable law or agreed to in writing, the safe network software distributed
// under the gpl licence is distributed on an "as is" basis, without warranties or conditions of any
// kind, either express or implied. please review the licences for the specific language governing
// permissions and limitations relating to use of the safe network software.

///! A JSON RPC over quic daemon module which allows
///! asynchronous servicing and response to qjsonrpc reqeusts.
///!
///! The daemon asynchronously receives JsonRpcRequests
///! and buffers them to a `RequestStream` for later servicing.
///!
///! When a server process calls `RequestStream.get_next()`,
///! it is handed the request as well as the `ResponseStream`
///! to buffer a response to that request.
///! A received `JsonRpcRequest` *must* be responded to via
///! `ResponseStream.send_oneshot()`.
///!
///! At some later point in time, the daemon will
///! pick up the `Response` and forward it to the Client
use crate::RequestStream;
use qjsonrpc::{
    Endpoint, Error, IncomingJsonRpcRequest, JsonRpcRequest, JsonRpcResponse,
    JsonRpcResponseStream, Result,
};
use std::{collections::HashMap, path::Path, sync::Arc};
use tokio::sync::{
    mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender},
    Mutex,
};
use url::Url;

/// A JSON RPC over quic daemon process.
/// Acts as an async intermediary between the
/// client and server
pub struct JsonRpcDaemon {
    /// Underlying qjsonrpc Endpoint
    endpoint: Endpoint,

    /// stream to buffer queries to the server
    request_tx: UnboundedSender<JsonRpcRequest>,

    /// stream to buffer responses from the server
    response_rx: UnboundedReceiver<JsonRpcResponse>,

    /// Maps request id to the response stream
    open_streams: Arc<Mutex<HashMap<u32, JsonRpcResponseStream>>>,
}

impl JsonRpcDaemon {
    /// ctor
    /// Don't use this directly. Instead see `jsonrpc_daemon()`
    fn new<P: AsRef<Path>>(
        cert_base_path: P,
        idle_timeout: Option<u64>,
        request_tx: UnboundedSender<JsonRpcRequest>,
        response_rx: UnboundedReceiver<JsonRpcResponse>,
    ) -> Result<Self> {
        let endpoint = Endpoint::new(cert_base_path, idle_timeout)?;
        let open_streams = Arc::new(Mutex::new(HashMap::new()));
        Ok(Self {
            endpoint,
            request_tx,
            response_rx,
            open_streams,
        })
    }

    /// Runs the service on the current thread
    /// This will loop, servicing incoming requests from
    /// clients and responses from the server.
    /// The function returns when it detects that
    /// the `RequestStream` associated with `self` (created along with
    /// `self` using `rpc_daemon()`) and all
    /// outstanding `ResponseStream`s are dropped.
    pub async fn run(&mut self, listen_addr_raw: &str) -> Result<()> {
        // parse and bind the socket address
        let listen_socket_addr = Url::parse(listen_addr_raw)
            .map_err(|_| Error::GeneralError("Invalid endpoint address".to_string()))?
            .socket_addrs(|| None)
            .map_err(|_| Error::GeneralError("Invalid endpoint address".to_string()))?[0];

        let mut incoming_conn = self
            .endpoint
            .bind(&listen_socket_addr)
            .map_err(|err| Error::GeneralError(format!("Failed to bind endpoint: {}", err)))?;

        // Service Loop
        let mut done = false;
        while !done {
            tokio::select!(

                // service a new qjsonrpc connection
                Some(incoming_req) = incoming_conn.get_next() => {
                    let _ = tokio::spawn(
                        Self::handle_connection(
                            self.request_tx.clone(),
                            self.open_streams.clone(),
                            incoming_req
                        )
                    );
                },

                // service event from server
                maybe_response = self.response_rx.recv() => {

                    match maybe_response {
                        // followup an exisxting connection
                        Some(response) => {
                            let _ = tokio::spawn(
                                Self::handle_response(
                                    response,
                                    self.open_streams.clone()
                                )
                            );

                        },
                        // All senders were dropped, time to exit
                        None => {
                            done = true;
                        }
                    }
                },
            );
        }

        Ok(())
    }

    /// Handle a `Response` from the server
    /// by serializing it to a `JsonRpcResponse`
    /// and sending it back to the original sender
    async fn handle_response(
        response: JsonRpcResponse,
        open_streams: Arc<Mutex<HashMap<u32, JsonRpcResponseStream>>>,
    ) -> Result<()> {
        // retreive the stream
        let mut open_streams_lock = open_streams.lock().await;
        let stream_opt = open_streams_lock.remove(&response.id().unwrap()); // can't fail
        drop(open_streams_lock);

        // For now, it's logically impossible to reply to a stream twice
        // Since `qjsonrpc` doesn't support batching yet.
        // If this changes in the future, this will likely panic
        let mut resp_stream = stream_opt.unwrap();

        resp_stream.respond(&response).await?;
        resp_stream.finish().await
    }

    /// Handle an incoming `JsonRpcRequest` from a new client
    /// by converting it to a `Query` and buffering it
    /// to the `QueryStream`
    async fn handle_connection(
        query_tx: UnboundedSender<JsonRpcRequest>,
        open_streams: Arc<Mutex<HashMap<u32, JsonRpcResponseStream>>>,
        mut incoming_req: IncomingJsonRpcRequest,
    ) -> Result<()> {
        // Each stream initiated by the client constitutes a new request
        // in the current `qjsonrpc` implementation (batches not supported yet)
        while let Some((jsonrpc_req, resp_stream)) = incoming_req.get_next().await {
            // cache the connection and buffer request to the `RequestStream`
            let mut open_streams_lock = open_streams.lock().await;
            let _ = open_streams_lock.insert(jsonrpc_req.id, resp_stream);
            drop(open_streams_lock);

            query_tx
                .send(jsonrpc_req)
                .map_err(|e| Error::GeneralError(format!("{}", e)))?;
        }

        Ok(())
    }
}

/// Initializes a new RpcDaemon object and gives back a stream for
/// server internal queries coming from it
pub fn jsonrpc_daemon<P: AsRef<Path>>(
    cert_base_dir: P,
    idle_timeout: Option<u64>,
) -> Result<(JsonRpcDaemon, RequestStream)> {
    let (request_tx, request_rx) = unbounded_channel::<JsonRpcRequest>();
    let (response_tx, response_rx) = unbounded_channel::<JsonRpcResponse>();
    let daemon = JsonRpcDaemon::new(cert_base_dir, idle_timeout, request_tx, response_rx)?;
    let request_stream = RequestStream::new(response_tx, request_rx);
    Ok((daemon, request_stream))
}
