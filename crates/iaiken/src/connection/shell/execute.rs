use crate::{
    connection::iopub::IopubTx,
    eval::{evaluate_user_expressions, execute_aiken_code},
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
        let request = &exec_msg.content;
        let reply: ExecuteReply;

        // Signal that the kernel is busy
        if let Ok(msg) = raw_msg.to_iopub_status(&config.key, &config.signature_scheme, "busy") {
            if let Err(e) = iopub_tx.send(msg) {
                eprintln!("Failed to send busy status: {}", e);
            }
        }

        // Send execute_input unless silent mode is enabled
        if !request.silent {
            if let Ok(msg) = raw_msg.to_iopub_execute_input(
                &config.key,
                &config.signature_scheme,
                &request.code,
                execution_count,
            ) {
                println!("Sending execute_input with count: {}", execution_count);
                if let Err(e) = iopub_tx.send(msg) {
                    eprintln!("Failed to send execute_input: {}", e);
                }
            } else {
                eprintln!("Failed to create execute_input message");
            }
        }

        // Execute the main code
        match execute_aiken_code(&request.code).await {
            Ok(execution_result) => {
                // Send execute_result unless silent mode is enabled.
                // WARN: Here, we are using the execute_result message, which does the same as
                // display_data, but provides the execution_count field for the frontend to
                // display as Out[] counter. This looks different than other kernels, for example,
                // IPython, but it is the "correct" way to do it as per the spec.
                // If the Out[] counter gets annoying, we can change this to display_data.
                // More info:
                // - https://jupyter-client.readthedocs.io/en/stable/messaging.html#id6
                // - https://jupyter-client.readthedocs.io/en/stable/messaging.html#display-data
                // - https://discourse.jupyter.org/t/jupyter-messaging-display-data-vs-execute-result/21919
                if !request.silent {
                    if let Ok(msg) = raw_msg.to_iopub_execute_result(
                        &config.key,
                        &config.signature_scheme,
                        execution_count,
                        execution_result,
                        serde_json::Value::Null,
                    ) {
                        if let Err(e) = iopub_tx.send(msg) {
                            eprintln!("Failed to send execute_result: {}", e);
                        }
                    } else {
                        eprintln!("Failed to create execute_result message");
                    }
                }

                // Evaluate user expressions if provided
                let user_expressions = if let serde_json::Value::Object(expr_map) =
                    &request.user_expressions
                {
                    if !expr_map.is_empty() {
                        // Convert JSON object to HashMap<String, String>
                        let mut expressions = std::collections::HashMap::new();
                        for (name, expr) in expr_map {
                            if let serde_json::Value::String(expr_str) = expr {
                                expressions.insert(name.clone(), expr_str.clone());
                            }
                        }

                        if !expressions.is_empty() {
                            let results = evaluate_user_expressions(&expressions).await;
                            Some(serde_json::to_value(results).unwrap_or(serde_json::Value::Null))
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                } else {
                    None
                };

                // Create successful execute reply
                reply = ExecuteReply::Ok {
                    execution_count,
                    user_expressions,
                };
            }

            Err(error) => {
                // Extract error details for reply
                let ename = "AikenError"; // Exception name
                let evalue = error.lines().next().unwrap_or("").to_string(); // First line as exception value
                let traceback: Vec<String> = error.lines().map(|line| line.to_string()).collect(); // Split into lines for proper traceback

                // Send error to IOPub
                if let Ok(msg) = raw_msg.to_iopub_error(
                    &config.key,
                    &config.signature_scheme,
                    ename,
                    &evalue,
                    &traceback,
                ) {
                    if let Err(e) = iopub_tx.send(msg) {
                        eprintln!("Failed to send error message: {}", e);
                    }
                } else {
                    eprintln!("Failed to create error message");
                }

                // Create error execute reply
                reply = ExecuteReply::Error {
                    execution_count,
                    ename: ename.to_string(),
                    evalue,
                    traceback,
                };
            }
        }

        // Build execute_reply
        let reply_msg = JupyterMessage {
            header: MessageHeader::new(raw_msg.header.session.clone(), "execute_reply".to_string()),
            parent_header: Some(raw_msg.header.clone()),
            metadata: serde_json::Value::Object(serde_json::Map::new()),
            content: reply,
        };

        // Send execute_reply
        if let Ok(byte_frames) = reply_msg.to_envelope_multipart(
            frames,
            delim_index,
            &config.key,
            &config.signature_scheme,
        ) {
            if let Err(e) = send_bytes(shell_socket, byte_frames).await {
                eprintln!("Failed to send execute_reply: {}", e);
            }
        } else {
            eprintln!("Failed to create execute_reply message");
        }

        // Announce kernel is back to idle
        if let Ok(msg) = raw_msg.to_iopub_status(&config.key, &config.signature_scheme, "idle") {
            if let Err(e) = iopub_tx.send(msg) {
                eprintln!("Failed to send idle status: {}", e);
            }
        }
    }
    Ok(())
}
