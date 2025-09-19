use super::{crypto::sign_message, JupyterMessage, MessageHeader};

pub async fn send_bytes<U: zeromq::Socket + zeromq::SocketSend>(
    socket: &mut U,
    bytes_frames: Vec<bytes::Bytes>,
) -> anyhow::Result<()> {
    match zeromq::ZmqMessage::try_from(bytes_frames) {
        Ok(zmq_msg) => {
            if let Err(e) = socket.send(zmq_msg).await {
                eprintln!("Failed to send reply: {e}");
            }
            Ok(())
        }
        Err(e) => Err(anyhow::anyhow!("Failed to create reply ZmqMessage: {e}")),
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

        super::crypto::verify_incoming_hmac(
            frames,
            config_key,
            config_signature_scheme,
            delim_index,
        )?;

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
}
