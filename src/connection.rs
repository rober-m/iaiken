use crate::messages::{
    ConnectionConfig, HelpLinks, JupyterMessage, KernelInfoReply, LanguageInfo, MessageHeader,
};
use std::fs;
use zeromq::{Socket, SocketRecv, SocketSend};

pub async fn run_kernel(connection_file: String) -> anyhow::Result<()> {
    // 1. Read the connection file
    let config_data = fs::read_to_string(&connection_file).map_err(|e| {
        anyhow::anyhow!(
            "Failed to read connection file '{}': {}",
            connection_file,
            e
        )
    })?;

    // 2. Parse JSON into ConnectionConfig
    let config: ConnectionConfig = serde_json::from_str(&config_data)
        .map_err(|e| anyhow::anyhow!("Failed to parse connection file: {}", e))?;

    // 3. Build ZMQ addresses
    println!("Kernel starting with config:");
    println!("  Shell: {}", config.shell_address());
    println!("  Control: {}", config.control_address());
    println!("  IOPub: {}", config.iopub_address());
    println!("  Stdin: {}", config.stdin_address());
    println!("  Heartbeat: {}", config.hb_address());

    // 4. Create ZMQ context and sockets
    let mut shell_socket = zeromq::RouterSocket::new();
    let mut control_socket = zeromq::RouterSocket::new();
    let mut iopub_socket = zeromq::PubSocket::new();
    let mut stdin_socket = zeromq::RouterSocket::new();
    let mut hb_socket = zeromq::RepSocket::new();

    // 5. Bind to addresses
    shell_socket.bind(&config.shell_address()).await?;
    control_socket.bind(&config.control_address()).await?;
    iopub_socket.bind(&config.iopub_address()).await?;
    stdin_socket.bind(&config.stdin_address()).await?;
    hb_socket.bind(&config.hb_address()).await?;

    println!("All sockets bound successfully!");

    // Spawn shell handler
    let shell_handle = tokio::spawn(async move {
        loop {
            match shell_socket.recv().await {
                Ok(message) => {
                    // Try to parse as a generic message first to get the header
                    let frames: Vec<Vec<u8>> = message.iter().map(|frame| frame.to_vec()).collect();
                    if let Ok(raw_msg) =
                        JupyterMessage::<serde_json::Value>::from_multipart(&frames)
                    {
                        println!("Received message type: {}", raw_msg.header.msg_type);

                        // Route based on message type
                        match raw_msg.header.msg_type.as_str() {
                            "kernel_info_request" => {
                                // Handle kernel info request
                                let reply = KernelInfoReply {
                                    status: "ok".to_string(),
                                    protocol_version: "5.4".to_string(),
                                    implementation: "aiken".to_string(),
                                    implementation_version: "1.0.0".to_string(), // TODO
                                    language_info: LanguageInfo {
                                        name: "aiken".to_string(),
                                        version: "1.0.0".to_string(), // TODO
                                        mimetype: "text/x-aiken".to_string(),
                                        file_extension: ".ak".to_string(),
                                        pygments_lexer: Some("text".to_string()), // TODO
                                        codemirror_mode: Some(serde_json::Value::String(
                                            "aiken".to_string(),
                                        )),
                                        nbconvert_exporter: "script".to_string(), //TODO
                                    },
                                    banner: "Aiken Kernel v0.1.0\nCardano Smart Contract Language"
                                        .to_string(), //TODO
                                    help_links: vec![HelpLinks {
                                        text: "Aiken Documentation".to_string(),
                                        url: "https://aiken-lang.org/".to_string(),
                                    }],
                                    supported_features: None, //TODO
                                };

                                // Build reply header
                                let reply_header = MessageHeader {
                                    msg_id: uuid::Uuid::new_v4().to_string(),
                                    session: raw_msg.header.session.clone(),
                                    username: "kernel".to_string(),
                                    date: chrono::Utc::now().to_rfc3339(),
                                    msg_type: "kernel_info_reply".to_string(),
                                    version: "5.4".to_string(),
                                };

                                // Create reply message
                                let reply_msg = JupyterMessage {
                                    header: reply_header,
                                    parent_header: Some(raw_msg.header.clone()),
                                    metadata: serde_json::Value::Object(serde_json::Map::new()),
                                    content: reply,
                                };

                                // Convert to ZMQ frames
                                if let Ok(frames) = reply_msg.to_multipart() {
                                    // Convert Vec<Vec<u8>> to Vec<Bytes>
                                    let bytes_frames: Vec<bytes::Bytes> =
                                        frames.into_iter().map(|frame| frame.into()).collect();
                                    // Convert Vec<Bytes> to ZmqMessage
                                    match zeromq::ZmqMessage::try_from(bytes_frames) {
                                        Ok(zmq_msg) => {
                                            // Send reply
                                            if let Err(e) = shell_socket.send(zmq_msg).await {
                                                eprintln!("Failed to send kernel_info_reply: {e}",);
                                            } else {
                                                println!("Sent kernel_info_reply successfully!");
                                            }
                                        }
                                        Err(e) => {
                                            eprintln!("Failed to create ZmqMessage: {e}");
                                        }
                                    }
                                } else {
                                    eprintln!("Failed to serialize kernel_info_reply");
                                }
                            }
                            "execute_request" => {
                                // Handle code execution
                                println!("Handling execute_request");
                                // TODO: Send execute_reply
                            }
                            _ => {
                                println!("Unknown message type: {}", raw_msg.header.msg_type);
                            }
                        }
                    } else {
                        println!("Failed to parse message");
                    }
                }
                Err(e) => {
                    eprintln!("Shell receive error: {e}");
                    break;
                }
            }
        }
    });

    // Spawn heartbeat handler
    let heartbeat_handle = tokio::spawn(async move {
        loop {
            // Wait for ping message
            match hb_socket.recv().await {
                Ok(message) => {
                    // Echo it back
                    if let Err(e) = hb_socket.send(message).await {
                        eprintln!("Heartbeat send message error: {e}");
                        break;
                    }
                }
                Err(e) => {
                    eprintln!("Heartbeat receive message error: {e}");
                    break;
                }
            }
        }
    });

    // Wait for either task to complete (they should run forever)
    tokio::select! {
        _ = heartbeat_handle => {},
        _ = shell_handle => {},
    }

    Ok(())
}
