use crate::messages::{
    delim_index, ConnectionConfig, JupyterMessage };
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
                        Err(e) => {
                            eprintln!("{e}");
                            continue;
                        }
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
                                crate::messages::kernel_info::handle_kernel_info_request(
                                    &config,
                                    &mut shell_socket,
                                    &mut iopub_socket,
                                    raw_msg,
                                    frames,
                                    delim_index,
                                )
                                .await;
                            }
                            "execute_request" => {
                                crate::messages::execute::handle_execute_request(
                                    &config,
                                    &mut shell_socket,
                                    &mut iopub_socket,
                                    raw_msg,
                                    frames,
                                    delim_index,
                                )
                                .await;
                            }
                            _ => {
                                println!("Unknown message type: {}", raw_msg.header.msg_type);
                            }
                        }
                    } else {
                        println!("Failed to parse message with {} frames", frames.len());
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
