use crate::{
    connection::iopub::IopubTx,
    messages::{
        ConnectionConfig, JupyterMessage, MessageHeader, shell::kernel_info::KernelInfoReply,
        wire::send_bytes,
    },
};

use zeromq::RouterSocket;

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
