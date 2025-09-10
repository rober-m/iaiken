use serde::{Serialize, Deserialize};

// DOCS: https://jupyter-client.readthedocs.io/en/latest/messaging.html#message-header
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MessageHeader {
    pub msg_id: String,       // UUID for this message
    pub session: String,      // Session UUID
    pub username: String,     // Usually "kernel"
    pub date: String,         // ISO 8601 timestamp
    pub msg_type: String,     // "execute_request", "kernel_info_request", etc.
    pub version: String,      // Protocol version "5.4"
}

// DOCS: https://jupyter-client.readthedocs.io/en/latest/messaging.html#general-message-format
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct JupyerMessage<T> {
    pub header: MessageHeader,                  // Header for this message
    pub parent_header: Option<MessageHeader>,   // Header of the parent message
    pub metadata: serde_json::Value,            // Metadata for this message
    pub content: T,                             // Content specific to the message type
}

// DOCS: https://jupyter-client.readthedocs.io/en/latest/messaging.html#execute
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ExecuteRequest {
    pub code: String,                        // Source code to be executed by the kernel
    pub silent: bool,                        // If true, execute as quietly as possible
    pub store_history: bool,                 // If true, store this execution in the history
    pub user_expressions: serde_json::Value, // Mapping of names to expressions to evaluate after execution
    pub allow_stdin: bool,                   // If true, code running in the kernel can prompt the user for input
    pub stop_on_error: bool,                 // If true, aborts the execution queue if an exception is encountered.
}

