use serde::{Deserialize, Serialize};
use zeromq::{PubSocket, RouterSocket};

use crate::{connection::send_bytes, messages::MessageHeader};

use super::{ConnectionConfig, JupyterMessage};

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
pub struct ExecuteReply {
    pub status: String,       // "ok" or "error"
    pub execution_count: u32, // Incremental counter
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_expressions: Option<serde_json::Value>,
}

pub async fn handle_execute_request(
    config: &ConnectionConfig,
    shell_socket: &mut RouterSocket,
    iopub_socket: &mut PubSocket,
    raw_msg: JupyterMessage<serde_json::Value>,
    frames: Vec<Vec<u8>>,
    delim_index: usize,
) {
    println!("Handling execute_request");
    // Parse the execute request
    if let Ok(exec_msg) = JupyterMessage::<ExecuteRequest>::from_multipart(
        &frames,
        &config.key,
        &config.signature_scheme,
    ) {
        println!("Executing code: {}", exec_msg.content.code);

        // IOPub: status busy
        if let Ok(bytes_frames) =
            raw_msg.to_iopub_multipart(&config.key, &config.signature_scheme, "busy".to_string())
        {
            let _ = send_bytes(iopub_socket, bytes_frames).await;
        }

        // For now, just echo the code back as output
        // TODO: Actually compile/execute Aiken code

        // Create execute reply
        let reply = ExecuteReply {
            status: "ok".to_string(),
            execution_count: 1, // TODO: Track this properly
            user_expressions: None,
        };

        let reply_header =
            MessageHeader::new(raw_msg.header.session.clone(), "execute_reply".to_string());

        let reply_msg = JupyterMessage {
            header: reply_header,
            parent_header: Some(raw_msg.header.clone()),
            metadata: serde_json::Value::Object(serde_json::Map::new()),
            content: reply,
        };

        if let Ok(bytes_frames) = reply_msg.to_envelope_multipart(
            frames,
            delim_index,
            &config.key,
            &config.signature_scheme,
        ) {
            send_bytes(shell_socket, bytes_frames).await.unwrap();
        };

        // IOPub: status idle
        if let Ok(bytes_frames) =
            raw_msg.to_iopub_multipart(&config.key, &config.signature_scheme, "idle".to_string())
        {
            let _ = send_bytes(iopub_socket, bytes_frames).await;
        }
    }
}
