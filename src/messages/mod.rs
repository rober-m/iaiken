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
pub struct JupyterMessage<T> {
    pub header: MessageHeader,                // Header for this message
    pub parent_header: Option<MessageHeader>, // Header of the parent message
    pub metadata: serde_json::Value,          // Metadata for this message
    pub content: T,                           // Content specific to the message type
}

impl<T> JupyterMessage<T> 
where
    T: serde::de::DeserializeOwned, 
    {
        pub fn from_multipart(frames: &[Vec<u8>]) -> anyhow::Result<Self>{
            if frames.len() < 6 {
                return Err(anyhow::anyhow!("Invalid message format: Too few frames!"));
            }

            // Skip identity and delimiter frames (first 2)
            // Skip HMAC frame (frame 2) for now
            let header: MessageHeader = serde_json::from_slice(&frames[3])?;
            let parent_header: Option<MessageHeader> = if frames[4].is_empty() {
                None
            } else {
                Some(serde_json::from_slice(&frames[4])?)
            };

            let metadata: serde_json::Value = serde_json::from_slice(&frames[5])?;
            let content: T = serde_json::from_slice(&frames[6])?;
            Ok(JupyterMessage { header, parent_header, metadata, content })
        }
    }

impl<T> JupyterMessage<T>
  where
      T: serde::Serialize,
{
      pub fn to_multipart(&self) -> anyhow::Result<Vec<Vec<u8>>> {
          let mut frames = Vec::new();

          // Frame 0: Identity (empty for now)
          frames.push(b"kernel".to_vec());

          // Frame 1: Delimiter
          frames.push(b"<IDS|MSG>".to_vec());

          // Frame 2: HMAC signature (empty for now)
          frames.push(Vec::new());

          // Frame 3: Header
          frames.push(serde_json::to_vec(&self.header)?);

          // Frame 4: Parent header
          frames.push(serde_json::to_vec(&self.parent_header)?);

          // Frame 5: Metadata
          frames.push(serde_json::to_vec(&self.metadata)?);

          // Frame 6: Content
          frames.push(serde_json::to_vec(&self.content)?);

          Ok(frames)
      }
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


// Kernel specification for installation
// DOCS: https://jupyter-client.readthedocs.io/en/latest/kernels.html#kernel-specs
  #[derive(Serialize, Deserialize, Debug)]
  pub struct KernelSpec {
      pub argv: Vec<String>,     // A list of command line arguments used to start the kernel
      pub display_name: String,  // The kernel’s name as it should be displayed in the UI
      pub language: String,      // The name of the language of the kernel
      #[serde(skip_serializing_if = "Option::is_none")]
      pub env: Option<std::collections::HashMap<String, String>>, // A dictionary of environment variables to set for the kernel
  }

impl KernelSpec {
      pub fn new(executable_path: &str) -> Self {
          Self {
              argv: vec![
                  executable_path.to_string(),
                  "--connection-file".to_string(),
                  "{connection_file}".to_string(),
              ],
              display_name: "Aiken".to_string(),
              language: "aiken".to_string(),
              env: None,
          }
      }
  }
