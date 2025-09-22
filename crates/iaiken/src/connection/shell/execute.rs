use crate::{
    connection::iopub::IopubTx,
    eval::execute_aiken_code,
    messages::{
        ConnectionConfig, JupyterMessage, MessageHeader,
        shell::execute::{ExecuteReply, ExecuteRequest},
        wire::send_bytes,
    },
};
use zeromq::RouterSocket;

pub async fn handle_execute_request(
    config: &ConnectionConfig,
    shell_socket: &mut RouterSocket,
    iopub_tx: &IopubTx,
    raw_msg: JupyterMessage<serde_json::Value>,
    frames: Vec<Vec<u8>>,
    delim_index: usize,
    execution_count: u32,
) -> anyhow::Result<()> {
    println!("Handling execute_request");
    // Parse the execute request
    if let Ok(exec_msg) = JupyterMessage::<ExecuteRequest>::from_multipart(
        &frames,
        &config.key,
        &config.signature_scheme,
    ) {
        println!("Executing code: {}", exec_msg.content.code);

        let _ = raw_msg
            .to_iopub_status(&config.key, &config.signature_scheme, "busy")
            .and_then(|f| Ok(iopub_tx.send(f)));

        let execution_result = execute_aiken_code(&exec_msg.content.code).await;

        let _ = raw_msg
            .to_iopub_stream(
                &config.key,
                &config.signature_scheme,
                "stdout",
                &execution_result,
            )
            .and_then(|f| Ok(iopub_tx.send(f)));

        // Create execute reply
        let reply = ExecuteReply {
            status: "ok".to_string(),
            execution_count,
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

        let _ = raw_msg
            .to_iopub_status(&config.key, &config.signature_scheme, "idle")
            .and_then(|f| Ok(iopub_tx.send(f)));
    }
    Ok(())
}
