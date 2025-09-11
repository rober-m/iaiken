use serde::{Deserialize, Serialize};

// DOCS: https://jupyter-client.readthedocs.io/en/latest/messaging.html#message-header
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MessageHeader {
    pub msg_id: String,   // UUID for this message
    pub session: String,  // Session UUID
    pub username: String, // Usually "kernel"
    pub date: String,     // ISO 8601 timestamp
    pub msg_type: String, // "execute_request", "kernel_info_request", etc.
    pub version: String,  // Protocol version "5.4"
}

// DOCS: https://jupyter-client.readthedocs.io/en/latest/messaging.html#general-message-format
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct JupyerMessage<T> {
    pub header: MessageHeader,                // Header for this message
    pub parent_header: Option<MessageHeader>, // Header of the parent message
    pub metadata: serde_json::Value,          // Metadata for this message
    pub content: T,                           // Content specific to the message type
}

// DOCS: https://jupyter-client.readthedocs.io/en/latest/messaging.html#execute
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ExecuteRequest {
    pub code: String,                        // Source code to be executed by the kernel
    pub silent: bool,                        // If true, execute as quietly as possible
    pub store_history: bool,                 // If true, store this execution in the history
    pub user_expressions: serde_json::Value, // Mapping of names to expressions to evaluate after execution
    pub allow_stdin: bool, // If true, code running in the kernel can prompt the user for input
    pub stop_on_error: bool, // If true, aborts the execution queue if an exception is encountered.
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ConnectionConfig {
    pub transport: String,        // Usually "tcp"
    pub ip: String,               // Usually "127.0.0.1"
    pub signature_scheme: String, // "hmac-sha256"
    pub key: String,              // For HMAC signing
    pub control_port: u16,
    pub shell_port: u16,
    pub stdin_port: u16,
    pub hb_port: u16, // heartbeat
    pub iopub_port: u16,
}

impl ConnectionConfig {
    pub fn shell_address(&self) -> String {
        format!("{}://{}:{}", self.transport, self.ip, self.shell_port)
    }

    pub fn control_address(&self) -> String {
        format!("{}://{}:{}", self.transport, self.ip, self.control_port)
    }

    pub fn stdin_address(&self) -> String {
        format!("{}://{}:{}", self.transport, self.ip, self.stdin_port)
    }

    pub fn hb_address(&self) -> String {
        format!("{}://{}:{}", self.transport, self.ip, self.hb_port)
    }

    pub fn iopub_address(&self) -> String {
        format!("{}://{}:{}", self.transport, self.ip, self.iopub_port)
    }
}

// DOCS: https://jupyter-client.readthedocs.io/en/latest/messaging.html#kernel-info
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct KernelInfoRequest {}

// DOCS: https://jupyter-client.readthedocs.io/en/latest/messaging.html#kernel-info
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct KernelInfoReply {
    pub status: String, // 'ok' if the request succeeded or 'error', with error information
    pub protocol_version: String, // Version of messaging protocol. Format X.Y.Z
    pub implementation: String, // The kernel implementation name
    pub implementation_version: String, // The kernel implementation version. Format X.Y.Z
    pub language_info: LanguageInfo,
    pub banner: String, // A banner of information about the kernel
    pub help_links: Vec<HelpLinks>,
    pub supported_features: Option<Vec<String>>, // A list of optional features such as 'debugger' and 'kernel subshells'.
}

// DOCS: https://jupyter-client.readthedocs.io/en/latest/messaging.html#kernel-info
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LanguageInfo {
    pub name: String,     // Name of the programming language that the kernel implements
    pub version: String,  // Language version number. Format X.Y.Z
    pub mimetype: String, // mimetype for script files in this language
    pub file_extension: String, // Extension including the dot, e.g. '.py' or '.ak'
    pub pygments_lexer: Option<String>, // Pygments lexer, for highlighting. Only needed if it differs from the 'name' field.
    pub codemirror_mode: Option<serde_json::Value>, // Codemirror mode, for highlighting in the notebook.. Only needed if it differs from the 'name' field.
    pub nbconvert_exporter: String,                 // Nbconvert exporter
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct HelpLinks {
    pub text: String,
    pub url: String,
}
