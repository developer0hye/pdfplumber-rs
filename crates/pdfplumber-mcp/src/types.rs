//! JSON-RPC 2.0 request and response types.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// A JSON-RPC 2.0 request.
#[derive(Debug, Deserialize)]
pub struct JsonRpcRequest {
    /// JSON-RPC version, always `"2.0"`.
    pub jsonrpc: String,
    /// Request ID. `null` for notifications.
    pub id: Option<Value>,
    /// Method name.
    pub method: String,
    /// Method parameters.
    pub params: Option<Value>,
}

/// A JSON-RPC 2.0 success response.
#[derive(Debug, Serialize)]
pub struct JsonRpcResponse {
    /// JSON-RPC version, always `"2.0"`.
    pub jsonrpc: &'static str,
    /// Mirrored request ID.
    pub id: Value,
    /// Result payload.
    pub result: Value,
}

/// A JSON-RPC 2.0 error object.
#[derive(Debug, Serialize)]
pub struct JsonRpcError {
    /// Numeric error code.
    pub code: i64,
    /// Human-readable message.
    pub message: String,
}

impl JsonRpcResponse {
    /// Construct a success response.
    pub fn ok(id: Value, result: Value) -> Self {
        Self {
            jsonrpc: "2.0",
            id,
            result,
        }
    }
}
