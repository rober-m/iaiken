use crate::messages::ConnectionConfig;
use std::{fs, str::from_utf8};
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
            // Wait for message
            match shell_socket.recv().await {
                Ok(message) => {
                    // Jupyter ROUTER messages have multiple frames
                    // For now, let's just print what we receive
                    println!("Shell received: {} frames", message.len());
                    for (i, frame) in message.iter().enumerate() {
                        if let Ok(text) = from_utf8(frame) {
                            println!("Frame: {i}: {text}");
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Shell receive message error: {e}");
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
