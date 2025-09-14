use crate::messages::{
    delim_index, ConnectionConfig, ExecuteReply, ExecuteRequest, HelpLink, JupyterMessage, KernelInfoReply, LanguageInfo, MessageHeader
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
                    let delim_index = match delim_index(&frames) {
                        Ok(ix) => ix,
                        Err(e) => { eprintln!("{e}"); continue; }
                    };

                    if let Ok(raw_msg) = JupyterMessage::<serde_json::Value>::from_multipart(
                        &frames,
                        &config.key,
                        &config.signature_scheme,
                    ) {
                        println!("Received message type: {}", raw_msg.header.msg_type);

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
                                let reply_header = MessageHeader::new(
                                    raw_msg.header.session.clone(),
                                    "kernel_info_reply".to_string(),
                                );

                                // IOPub: status busy
                                if let Ok(zmq_msg) = build_iopub_status_msg(
                                    &raw_msg,
                                    &config.key,
                                    &config.signature_scheme,
                                    "busy".to_string(),
                                ) {
                                    let _ = iopub_socket.send(zmq_msg).await;
                                }

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

                                // Serialize message parts
                                let header_bytes = serde_json::to_vec(&reply_msg.header).unwrap();
                                let parent_header_bytes =
                                    serde_json::to_vec(&reply_msg.parent_header).unwrap();
                                let metadata_bytes =
                                    serde_json::to_vec(&reply_msg.metadata).unwrap();
                                let content_bytes = serde_json::to_vec(&reply_msg.content).unwrap();

                                // Compute HMAC
                                let sig = sign_message(
                                    &config.key,
                                    &config.signature_scheme,
                                    &header_bytes,
                                    &parent_header_bytes,
                                    &metadata_bytes,
                                    &content_bytes,
                                )
                                .into_bytes();

                                // Build outgoing frames: reuse full identity envelope up to and including delimiter
                                let mut out_frames: Vec<Vec<u8>> = Vec::new();
                                out_frames.extend_from_slice(&frames[..=delim_index]);
                                out_frames.push(sig);
                                out_frames.push(header_bytes);
                                out_frames.push(parent_header_bytes);
                                out_frames.push(metadata_bytes);
                                out_frames.push(content_bytes);

                                // Debug the actual frames being sent
                                println!("Debug: Sending {} frames", out_frames.len());
                                for (i, frame) in out_frames.iter().enumerate() {
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

                                // Convert and send
                                let bytes_frames: Vec<bytes::Bytes> =
                                    out_frames.into_iter().map(|frame| frame.into()).collect();
                                match zeromq::ZmqMessage::try_from(bytes_frames) {
                                    Ok(zmq_msg) => {
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

                                // IOPub: status idle
                                if let Ok(zmq_msg) = build_iopub_status_msg(
                                    &raw_msg,
                                    &config.key,
                                    &config.signature_scheme,
                                    "idle".to_string(),
                                ) {
                                    let _ = iopub_socket.send(zmq_msg).await;
                                }
                            }
                            "execute_request" => {
                                println!("Handling execute_request");
                                // Parse the execute request
                                if let Ok(exec_msg) =
                                    JupyterMessage::<ExecuteRequest>::from_multipart(
                                        &frames,
                                        &config.key,
                                        &config.signature_scheme,
                                    )
                                {
                                    println!("Executing code: {}", exec_msg.content.code);

                                    // IOPub: status busy
                                    if let Ok(zmq_msg) = build_iopub_status_msg(
                                        &raw_msg,
                                        &config.key,
                                        &config.signature_scheme,
                                        "busy".to_string(),
                                    ) {
                                        let _ = iopub_socket.send(zmq_msg).await;
                                    }

                                    // For now, just echo the code back as output
                                    // TODO: Actually compile/execute Aiken code

                                    // Create execute reply
                                    let reply = ExecuteReply {
                                        status: "ok".to_string(),
                                        execution_count: 1, // TODO: Track this properly
                                        user_expressions: None,
                                    };

                                    // Build reply message (same pattern as kernel_info_reply)

                                    let reply_header = MessageHeader::new(
                                        raw_msg.header.session.clone(),
                                        "execute_reply".to_string(),
                                    );

                                    let reply_msg = JupyterMessage {
                                        header: reply_header,
                                        parent_header: Some(raw_msg.header.clone()),
                                        metadata: serde_json::Value::Object(serde_json::Map::new()),
                                        content: reply,
                                    };

                                    // Serialize parts
                                    let header_bytes =
                                        serde_json::to_vec(&reply_msg.header).unwrap();
                                    let parent_header_bytes =
                                        serde_json::to_vec(&reply_msg.parent_header).unwrap();
                                    let metadata_bytes =
                                        serde_json::to_vec(&reply_msg.metadata).unwrap();
                                    let content_bytes =
                                        serde_json::to_vec(&reply_msg.content).unwrap();

                                    // Compute HMAC
                                    let sig = sign_message(
                                        &config.key,
                                        &config.signature_scheme,
                                        &header_bytes,
                                        &parent_header_bytes,
                                        &metadata_bytes,
                                        &content_bytes,
                                    )
                                    .into_bytes();

                                    // Build outgoing frames
                                    let mut out_frames: Vec<Vec<u8>> = Vec::new();
                                    out_frames.extend_from_slice(&frames[..=delim_index]);
                                    out_frames.push(sig);
                                    out_frames.push(header_bytes);
                                    out_frames.push(parent_header_bytes);
                                    out_frames.push(metadata_bytes);
                                    out_frames.push(content_bytes);

                                    let bytes_frames: Vec<bytes::Bytes> =
                                        out_frames.into_iter().map(|frame| frame.into()).collect();

                                    match zeromq::ZmqMessage::try_from(bytes_frames) {
                                        Ok(zmq_msg) => {
                                            if let Err(e) = shell_socket.send(zmq_msg).await {
                                                eprintln!("Failed to send execute_reply: {e}");
                                            } else {
                                                println!("Sent execute_reply successfully!");
                                            }
                                        }
                                        Err(e) => {
                                            eprintln!(
                                                "Failed to create execute_reply ZmqMessage: {e}"
                                            );
                                        }
                                    }

                                    // IOPub: status idle
                                    if let Ok(zmq_msg) = build_iopub_status_msg(
                                        &raw_msg,
                                        &config.key,
                                        &config.signature_scheme,
                                        "idle".to_string(),
                                    ) {
                                        let _ = iopub_socket.send(zmq_msg).await;
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
    signature_scheme: &str,
    key: &str,
    header: &[u8],
    parent_header: &[u8],
    metadata: &[u8],
    content: &[u8],
) -> String {
    if key.is_empty() {
        return String::new();
    }
    // TODO: Is this check right?
    if signature_scheme != "hmac-sha256" {
        eprintln!("wrong signature schema: {signature_scheme}")
    }

    let mut mac: HmacSha256 = HmacSha256::new_from_slice(key.as_bytes()).expect("HMAC key error");
    mac.update(header);
    mac.update(parent_header);
    mac.update(metadata);
    mac.update(content);
    hex::encode(mac.finalize().into_bytes())
}

fn build_iopub_status_msg(
    raw_msg: &JupyterMessage<serde_json::Value>,
    config_key: &str,
    config_signature_scheme: &str,
    status: String,
) -> anyhow::Result<zeromq::ZmqMessage, zeromq::ZmqEmptyMessageError> {
    let status_header = MessageHeader::new(raw_msg.header.session.clone(), "status".to_string());

    let parent_header = Some(raw_msg.header.clone());
    let metadata = serde_json::Value::Object(serde_json::Map::new());
    let content = serde_json::json!({"execution_state":status});

    let h = serde_json::to_vec(&status_header).unwrap();
    let p = serde_json::to_vec(&parent_header).unwrap();
    let m = serde_json::to_vec(&metadata).unwrap();
    let c = serde_json::to_vec(&content).unwrap();
    let sig = sign_message(config_key, config_signature_scheme, &h, &p, &m, &c).into_bytes();

    let mut iopub_frames: Vec<Vec<u8>> = Vec::new();
    iopub_frames.push(b"<IDS|MSG>".to_vec());
    iopub_frames.push(sig);
    iopub_frames.push(h);
    iopub_frames.push(p);
    iopub_frames.push(m);
    iopub_frames.push(c);

    let bytes_frames: Vec<bytes::Bytes> =
        iopub_frames.into_iter().map(|frame| frame.into()).collect();
    zeromq::ZmqMessage::try_from(bytes_frames)
}
