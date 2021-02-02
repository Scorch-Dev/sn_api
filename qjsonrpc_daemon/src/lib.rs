// copyright 2021 maidsafe.net limited.
//
// this safe network software is licensed to you under the general public license (gpl), version 3.
// unless required by applicable law or agreed to in writing, the safe network software distributed
// under the gpl licence is distributed on an "as is" basis, without warranties or conditions of any
// kind, either express or implied. please review the licences for the specific language governing
// permissions and limitations relating to use of the safe network software.

///! This lib provides a convenient `qjsonrpc` daemon `JsonRpcDaemon`
///! which services request and response types asynchronously.
///!
///! This pattern is often useful when adding an RPC
///! interface to existing code. For one, it allow the main process
///! to quickly and easily set up a `qjsonrpc::Endpoint`
///! with minimal extra code compared to manually setting up a
///! `qjsonrpc::Endpoint` manually. Secondly, it lets the main
///! process to not have to spend time on communication-specific
///! tasks like serializing/deserializing, error handling, and waiting
///! for TLS handshaking. This frees it up to do other things
mod daemon;
mod stream;

pub use daemon::{jsonrpc_daemon, JsonRpcDaemon};
pub use stream::{RequestStream, ResponseStream};
