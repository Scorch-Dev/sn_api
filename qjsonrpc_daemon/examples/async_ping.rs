// copyright 2021 maidsafe.net limited.
//
// this safe network software is licensed to you under the general public license (gpl), version 3.
// unless required by applicable law or agreed to in writing, the safe network software distributed
// under the gpl licence is distributed on an "as is" basis, without warranties or conditions of any
// kind, either express or implied. please review the licences for the specific language governing
// permissions and limitations relating to use of the safe network software.

use qjsonrpc::{ClientEndpoint, JsonRpcResponse, Result};
use qjsonrpc_daemon::jsonrpc_daemon;
use serde_json::json;
use tempfile::tempdir;

// hyper params
const LISTEN: &str = "https://localhost:33001";
const TIMEOUT_MS: u64 = 10000;
const METHOD_PING: &str = "ping";

// as per jsonrpc 2.0 spec (see: https://www.jsonrpc.org/specification)
const ERROR_METHOD_NOT_FOUND: isize = -32601;

///  Sets up a minimal client, a server process,
/// and an async qjsonrpc daemon in the middle.
/// The client sends a single ping to the server
/// as usual. The request though is fielded and parsed
/// by the daemon first, and buffered to a `ResponseStream`.
/// At some point later in time, the server retrieves it,
/// buffers an acknowledgement response, to the daemon and
/// exits. The rpc daemon will then forward the buffered
/// ack at some point later in time and then exit as well.
///
/// In general, when the `ResponseStream` is dropped,
/// the rpc daemon knows it's time to shut down,
/// and does so automatically too by returning
/// from run().
///
/// Note the incredible similarity between this and
/// the `qjsonrpc` basic ping.rs example. Though,
/// this does simplify much much of the logic for
/// constructing a server (e.g. no explicit calls
/// to `bind()`, `listen()`, etc.)
#[tokio::main]
async fn main() -> Result<()> {
    let cert_base_dir = tempdir()?;
    let (mut daemon, mut query_stream) = jsonrpc_daemon(cert_base_dir.path(), Some(TIMEOUT_MS))?;
    let client = ClientEndpoint::new(cert_base_dir.path(), Some(TIMEOUT_MS), false)?;

    // client task is same as non-daemon version
    let client_task = async move {
        let mut out_conn = client.bind()?;
        println!("[client] bound");

        // try connect
        let mut out_jsonrpc_req = out_conn.connect(LISTEN, None).await?;
        println!("[client] connected to {}", LISTEN);

        // ping and print the ack
        println!("[client] sending '{}' method to server...", METHOD_PING);
        let ack = out_jsonrpc_req
            .send::<String>(METHOD_PING, json!(null))
            .await?;
        println!("[client] received response '{}'\n", ack);

        Ok(())
    };

    // the rpc daemon task runs quietly on its own thread
    let daemon_task = async move {
        println!("[daemon] running");
        let res = daemon.run(LISTEN).await;
        println!("[daemon] exited cleanly");
        res
    };

    // simple async server handles queries async from when they're received
    let fake_node_task = async move {
        if let Some((req, resp_stream)) = query_stream.get_next().await {
            // service a single ping with an ack
            println!("[server]: handling request {:?} received from daemon", &req);
            let method = req.method.as_ref();
            let resp = match method {
                METHOD_PING => JsonRpcResponse::result(json!("Ack"), req.id),
                _ => {
                    let msg = format!("Unkown method {} received.", method);
                    JsonRpcResponse::error(msg, ERROR_METHOD_NOT_FOUND, Some(req.id))
                }
            };

            println!("[server] buffering response to daemon {:?}", &resp);
            resp_stream.send_oneshot(resp);
        }

        Ok(()) // RequestStream drops here, and daemon will know to close
    };

    // join all
    tokio::try_join!(daemon_task, client_task, fake_node_task).and_then(|_| Ok(()))
}
