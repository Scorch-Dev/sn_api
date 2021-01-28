// Copyright 2020 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under the MIT license <LICENSE-MIT
// http://opensource.org/licenses/MIT> or the Modified BSD license <LICENSE-BSD
// https://opensource.org/licenses/BSD-3-Clause>, at your option. This file may not be copied,
// modified, or distributed except according to those terms. Please review the Licences for the
// specific language governing permissions and limitations relating to use of the SAFE Network
// Software.
use crate::{JsonRpcRequest, JsonRpcResponse};
use serde::{de::DeserializeOwned, ser::Serialize};
use std::convert::{From, TryFrom};

/// Trait used to allow compile-time-checked
/// requests which can be readily constructed
/// from JsonRpcRequests. Should the conversion fail,
/// the error is the error response
pub trait StructuredRequest:
    TryFrom<JsonRpcRequest, Error = JsonRpcResponse> + Serialize + DeserializeOwned
{
    /// consume self to get a JsonRpcRequest
    /// Allows JsonRpcRequest to automatically implement
    /// From<StructuredResponse>
    fn to_jsonrpc_request(self) -> JsonRpcRequest;
}

/// Trait used to allow compile-time-checked
/// responses to JSON RPC requests
pub trait StructuredResponse: From<JsonRpcResponse> + Serialize + DeserializeOwned {
    /// consume self to get a JsonRpcResponse
    /// Allows JsonRpcResponse to automatically implement
    /// From<StructuredResponse>
    fn to_jsonrpc_response(self) -> JsonRpcResponse;
}
