use crate::messages::{
    ConnectionConfig, ExecuteReply, ExecuteRequest, HelpLink, JupyterMessage, KernelInfoReply,
    LanguageInfo, MessageHeader,
};
use hmac::Mac;
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
                        println!(
                            "Incoming HMAC was: {}",
                            std::str::from_utf8(&frames[2]).unwrap_or("invalid")
                        );

                        // Route based on message type
                        match raw_msg.header.msg_type.as_str() {
                            "kernel_info_request" => {
                                println!(
                                    "Received kernel_info_request with raw_msg: {}",
                                    raw_msg.header.version
                                );
                                // Handle kernel info request
                                let reply = KernelInfoReply {
                                    status: "ok".to_string(),
                                    //protocol_version: "5.3".to_string(),
                                    protocol_version: raw_msg.header.version.clone(), // Match request version? (is this ok?)
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
                                    debugger: false,
                                    help_links: vec![HelpLink {
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
                                    //version: "5.3".to_string(),
                                    version: raw_msg.header.version.clone(), // Match request version?
                                };

                                println!("Sending reply with version: {}", &reply_header.version);
                                println!(
                                    "Reply content: {}",
                                    serde_json::to_string_pretty(&reply)
                                        .unwrap_or("serialize error".to_string())
                                );

                                // Create reply message
                                let reply_msg = JupyterMessage {
                                    header: reply_header,
                                    parent_header: Some(raw_msg.header.clone()),
                                    metadata: serde_json::Value::Object(serde_json::Map::new()),
                                    content: reply,
                                };

                                // Convert to ZMQ frames
                                if let Ok(frames) =
                                    reply_msg.to_multipart(Some(&frames[0]), &config.key)
                                {
                                    // Debug the actual frames being sent
                                    println!("Debug: Sending {} frames", frames.len());
                                    for (i, frame) in frames.iter().enumerate() {
                                        if let Ok(text) = std::str::from_utf8(frame) {
                                            println!("Outgoing Frame {}: {}", i, text);
                                        } else {
                                            println!(
                                                "Outgoing Frame {}: {} bytes (binary)",
                                                i,
                                                frame.len()
                                            );
                                        }
                                    }

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
                                println!("Handling execute_request");
                                // Parse the execute request
                                if let Ok(exec_msg) =
                                    JupyterMessage::<ExecuteRequest>::from_multipart(&frames)
                                {
                                    println!("Executing code: {}", exec_msg.content.code);

                                    // For now, just echo the code back as output
                                    // TODO: Actually compile/execute Aiken code

                                    // Create execute reply
                                    let reply = ExecuteReply {
                                        status: "ok".to_string(),
                                        execution_count: 1, // TODO: Track this properly
                                        user_expressions: None,
                                    };

                                    // Build reply message (same pattern as kernel_info_reply)
                                    let reply_header = MessageHeader {
                                        msg_id: uuid::Uuid::new_v4().to_string(),
                                        session: raw_msg.header.session.clone(),
                                        username: "kernel".to_string(),
                                        date: chrono::Utc::now().to_rfc3339(),
                                        msg_type: "execute_reply".to_string(),
                                        version: "5.3".to_string(),
                                    };

                                    let reply_msg = JupyterMessage {
                                        header: reply_header,
                                        parent_header: Some(raw_msg.header.clone()),
                                        metadata: serde_json::Value::Object(serde_json::Map::new()),
                                        content: reply,
                                    };

                                    // Send execute_reply through shell socket
                                    if let Ok(reply_frames) =
                                        reply_msg.to_multipart(Some(&frames[0]), &config.key)
                                    {
                                        let bytes_frames: Vec<bytes::Bytes> = reply_frames
                                            .into_iter()
                                            .map(|frame| frame.into())
                                            .collect();

                                        match zeromq::ZmqMessage::try_from(bytes_frames) {
                                            Ok(zmq_msg) => {
                                                if let Err(e) = shell_socket.send(zmq_msg).await {
                                                    eprintln!("Failed to send execute_reply: {e}");
                                                } else {
                                                    println!("Sent execute_reply successfully!");
                                                }
                                            }
                                            Err(e) => {
                                                eprintln!("Failed to create execute_reply ZmqMessage: {e}");
                                            }
                                        }
                                    }
                                }
                            }
                            _ => {
                                println!("Unknown message type: {}", raw_msg.header.msg_type);
                            }
                        }
                    } else {
                        println!("Failed to parse message with {} frames", frames.len());
                        for (i, frame) in frames.iter().enumerate() {
                            if let Ok(text) = std::str::from_utf8(frame) {
                                println!("Frame {i}: {text}");
                            } else {
                                println!("Frame {}: {} bytes (binary)", i, frame.len());
                            }
                        }
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

type HmacSha256 = hmac::Hmac<sha2::Sha256>;

pub fn sign_message(
    key: &str,
    header: &[u8],
    parent_header: &[u8],
    metadata: &[u8],
    content: &[u8],
) -> String {
    // println!("HMAC key being used: '{}'", key);
    // println!("HMAC key length: {}", key.len());
    //
    // // Debug: Print what we're signing
    // println!("Header bytes length: {}", header.len());
    // println!("Parent header bytes length: {}", parent_header.len());
    // println!("Metadata bytes length: {}", metadata.len());
    // println!("Content bytes length: {}", content.len());
    //
    // // Show actual JSON being signed
    // println!(
    //     "Header JSON: {}",
    //     std::str::from_utf8(header).unwrap_or("invalid")
    // );
    // println!(
    //     "Parent header JSON: {}",
    //     std::str::from_utf8(parent_header).unwrap_or("invalid")
    // );
    // println!(
    //     "Metadata JSON: {}",
    //     std::str::from_utf8(metadata).unwrap_or("invalid")
    // );
    //
    // // Decode the hex key to bytes
    // let key_bytes = match hex::decode(key.replace("-", "")) {
    //     Ok(bytes) => bytes,
    //     Err(_) => {
    //         // If hex decode fails, use the key as-is (fallback)
    //         println!("Warning: Could not decode key as hex, using as string");
    //         key.as_bytes().to_vec()
    //     }
    // };
    //
    // println!("Decoded key length: {}", key_bytes.len());
    //
    // let mut mac: HmacSha256 = HmacSha256::new_from_slice(&key_bytes).expect("HMAC key error");
    // mac.update(header);
    // mac.update(parent_header);
    // mac.update(metadata);
    // mac.update(content);
    // let result = hex::encode(mac.finalize().into_bytes());
    //
    // println!("Generated HMAC: {}", result);
    //return result

    // WARN: Debugging HMAC signature, delete this afterwards
    String::new()
}
