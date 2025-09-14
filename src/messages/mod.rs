use crate::messages::crypto::sign_message;
use crate::messages::kernel_info::PROTOCOL_VERSION;
use serde::{Deserialize, Serialize};

pub mod crypto;
pub mod execute;
pub mod kernel_info;

// DOCS: https://jupyter-client.readthedocs.io/en/latest/messaging.html#message-header
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MessageHeader {
    pub msg_id: String,   // UUID for this message
    pub session: String,  // Session UUID
    pub username: String, // Usually "kernel"
    pub date: String,     // ISO 8601 timestamp
    pub msg_type: String, // "execute_request", "kernel_info_request", etc.
    pub version: String,  // Protocol version
}

impl MessageHeader {
    pub fn new(session: String, msg_type: String) -> Self {
        MessageHeader {
            msg_id: uuid::Uuid::new_v4().to_string(),
            session,
            username: "kernel".to_string(),
            date: chrono::Utc::now().to_rfc3339(),
            msg_type,
            version: PROTOCOL_VERSION.to_string(),
        }
    }
}

// DOCS: https://jupyter-client.readthedocs.io/en/latest/messaging.html#general-message-format
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct JupyterMessage<T> {
    pub header: MessageHeader,                // Header for this message
    pub parent_header: Option<MessageHeader>, // Header of the parent message
    pub metadata: serde_json::Value,          // Metadata for this message
    pub content: T,                           // Content specific to the message type
}

impl<T: serde::de::DeserializeOwned> JupyterMessage<T> {
    pub fn from_multipart(
        frames: &[Vec<u8>],
        config_key: &str,
        config_signature_scheme: &str,
    ) -> anyhow::Result<Self> {
        let delim_index = delim_index(frames)?;

        if frames.len() < delim_index + 6 {
            return Err(anyhow::anyhow!(
                "Invalid message format: Only {} frames!",
                frames.len()
            ));
        }

        let header_bytes = &frames[delim_index + 2];
        let parent_bytes = &frames[delim_index + 3];
        let metadata_bytes = &frames[delim_index + 4];
        let content_bytes = &frames[delim_index + 5];

        crypto::verify_incoming_hmac(frames, config_key, config_signature_scheme, delim_index)?;

        // Skip identity and delimiter frames (first 2)
        // Skip HMAC frame (frame 2) for now
        let header: MessageHeader = serde_json::from_slice(header_bytes)?;
        let parent_header: Option<MessageHeader> =
            if parent_bytes.is_empty() || parent_bytes == b"{}" {
                None
            } else {
                Some(serde_json::from_slice(parent_bytes)?)
            };

        let metadata: serde_json::Value = if metadata_bytes.is_empty() || metadata_bytes == b"{}" {
            serde_json::Value::Object(serde_json::Map::new())
        } else {
            serde_json::from_slice(metadata_bytes)?
        };

        let content: T = if content_bytes.is_empty() || content_bytes == b"{}" {
            serde_json::from_str("{}")?
        } else {
            serde_json::from_slice(content_bytes)?
        };

        Ok(JupyterMessage {
            header,
            parent_header,
            metadata,
            content,
        })
    }
}

impl<T: serde::Serialize> JupyterMessage<T> {
    pub fn to_envelope_multipart(
        &self,
        frames: Vec<Vec<u8>>,
        delim_index: usize,
        key: &str,
        scheme: &str,
    ) -> anyhow::Result<Vec<bytes::Bytes>> {
        // Serialize parts
        let header_bytes = serde_json::to_vec(&self.header).unwrap();
        let parent_header_bytes = serde_json::to_vec(&self.parent_header).unwrap();
        let metadata_bytes = serde_json::to_vec(&self.metadata).unwrap();
        let content_bytes = serde_json::to_vec(&self.content).unwrap();

        // Compute HMAC
        let sig = sign_message(
            key,
            scheme,
            &header_bytes,
            &parent_header_bytes,
            &metadata_bytes,
            &content_bytes,
        )
        .into_bytes();

        // Build outgoing frames
        let mut out_frames: Vec<Vec<u8>> = Vec::with_capacity(delim_index + 6);
        out_frames.extend_from_slice(&frames[..=delim_index]);
        out_frames.push(sig);
        out_frames.push(header_bytes);
        out_frames.push(parent_header_bytes);
        out_frames.push(metadata_bytes);
        out_frames.push(content_bytes);

        Ok(out_frames.into_iter().map(|frame| frame.into()).collect())
    }

    pub fn to_iopub_multipart(
        &self,
        key: &str,
        signature_scheme: &str,
        status: String,
    ) -> anyhow::Result<Vec<bytes::Bytes>> {

        // Create message parts
        let status_header = MessageHeader::new(self.header.session.clone(), "status".to_string());
        let parent_header = Some(self.header.clone());
        let metadata = serde_json::Value::Object(serde_json::Map::new());
        let content = serde_json::json!({"execution_state": status});

        // Serialize parts
        let header_bytes = serde_json::to_vec(&status_header)?;
        let parent_header_bytes = serde_json::to_vec(&parent_header)?;
        let metadata_bytes = serde_json::to_vec(&metadata)?;
        let content_bytes = serde_json::to_vec(&content)?;

        let sig = sign_message(
            key,
            signature_scheme,
            &header_bytes,
            &parent_header_bytes,
            &metadata_bytes,
            &content_bytes,
        )
        .into_bytes();

        let out_frames = vec![
            b"<IDS|MSG>".to_vec(),
            sig,
            header_bytes,
            parent_header_bytes,
            metadata_bytes,
            content_bytes,
        ];

        Ok(out_frames.into_iter().map(|frame| frame.into()).collect())
    }
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

// Find the <IDS|MSG> delimiter to support variable identity envelope
pub fn delim_index(frames: &[Vec<u8>]) -> anyhow::Result<usize> {
    match frames.iter().position(|f| f.as_slice() == b"<IDS|MSG>") {
        Some(index) => Ok(index),
        None => Err(anyhow::anyhow!(
            "Malformed message: missing <IDS|MSG> delimiter with {} frames",
            frames.len()
        )),
    }
}
