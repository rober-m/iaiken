use crate::messages::{ConnectionConfig, JupyterMessage, MessageHeader, wire::send_bytes};

use serde::{Deserialize, Serialize};
use zeromq::RouterSocket;

use tokio::sync::mpsc::UnboundedSender;

// TODO: I'm repeating this type cause I don't know how to import it from connection::iopub. :/
pub type IopubTx = UnboundedSender<Vec<bytes::Bytes>>;

pub const PROTOCOL_VERSION: &str = "5.3";
const KI_STATUS: &str = "ok"; // TODO: Handle error status
const KI_IMPLEMENTATION: &str = "aiken";
const KI_IMPLEMENTATION_VERSION: &str = "0.0.1";
const KI_BANNER: &str = "Aiken Kernel v0.1.0\nCardano Smart Contract Language";
const KI_DEBUGGER: bool = false;
const KI_LI_NAME: &str = "aiken";
const KI_LI_VERSION: &str = "1.0.0"; //TODO
const KI_LI_MIMETYPE: &str = "text/x-aiken"; //TODO
const KI_LI_FILE_EXT: &str = ".ak";

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
    pub debugger: bool, // if the kernel supports debugging in the notebook.
    pub help_links: Vec<HelpLink>,
    pub supported_features: Option<Vec<String>>, // A list of optional features such as 'debugger' and 'kernel subshells'.
}

impl KernelInfoReply {
    pub fn new() -> Self {
        KernelInfoReply {
            status: KI_STATUS.to_string(),
            protocol_version: PROTOCOL_VERSION.to_string(),
            implementation: KI_IMPLEMENTATION.to_string(),
            implementation_version: KI_IMPLEMENTATION_VERSION.to_string(),
            language_info: LanguageInfo {
                name: KI_LI_NAME.to_string(),
                version: KI_LI_VERSION.to_string(),
                mimetype: KI_LI_MIMETYPE.to_string(),
                file_extension: KI_LI_FILE_EXT.to_string(),
                pygments_lexer: Some("text".to_string()),
                codemirror_mode: Some(serde_json::Value::String("aiken".to_string())),
                nbconvert_exporter: "script".to_string(),
            },
            banner: KI_BANNER.to_string(),
            debugger: KI_DEBUGGER,
            help_links: vec![HelpLink {
                text: "Aiken Documentation".to_string(),
                url: "https://aiken-lang.org/".to_string(),
            }],
            supported_features: None, //TODO
        }
    }
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
pub struct HelpLink {
    pub text: String,
    pub url: String,
}

pub async fn handle_kernel_info_request(
    config: &ConnectionConfig,
    shell_socket: &mut RouterSocket,
    iopub_tx: &IopubTx,
    raw_msg: JupyterMessage<serde_json::Value>,
    frames: Vec<Vec<u8>>,
    delim_index: usize,
) {
    println!(
        "Received kernel_info_request with raw_msg: {}",
        raw_msg.header.version
    );
    // Handle kernel info request
    let reply = KernelInfoReply::new();

    // Build reply header
    let reply_header = MessageHeader::new(
        raw_msg.header.session.clone(),
        "kernel_info_reply".to_string(),
    );

    if let Ok(frames) = raw_msg.to_iopub_status(&config.key, &config.signature_scheme, "busy") {
        let _ = iopub_tx.send(frames);
    }

    println!("Sending reply with version: {}", &reply_header.version);
    println!(
        "Reply content: {}",
        serde_json::to_string_pretty(&reply).unwrap_or("serialize error".to_string())
    );

    // Create reply message
    let reply_msg = JupyterMessage {
        header: reply_header,
        parent_header: Some(raw_msg.header.clone()),
        metadata: serde_json::Value::Object(serde_json::Map::new()),
        content: reply,
    };

    if let Ok(bytes_frames) =
        reply_msg.to_envelope_multipart(frames, delim_index, &config.key, &config.signature_scheme)
    {
        send_bytes(shell_socket, bytes_frames).await.unwrap();
    }

    if let Ok(frames) = raw_msg.to_iopub_status(&config.key, &config.signature_scheme, "idle") {
        let _ = iopub_tx.send(frames);
    }
}
