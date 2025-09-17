use crate::messages::ConnectionConfig;
use control::control_loop;
use heartbeat::heartbeat_loop;
use shell::shell_loop;
use std::fs;
use tokio::sync::mpsc::unbounded_channel;
use tokio_util::sync::CancellationToken;
use zeromq::Socket;

mod control;
mod heartbeat;
mod iopub;
mod shell;

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
    // TODO: Why can't I just reference the original config and that's it?
    let shell_config = config.clone();
    let control_config = config.clone();

    // 3. Build ZMQ addresses
    println!("Kernel starting with config:");
    println!("  Shell: {}", config.shell_address());
    println!("  Control: {}", config.control_address());
    println!("  IOPub: {}", config.iopub_address());
    println!("  Stdin: {}", config.stdin_address());
    println!("  Heartbeat: {}", config.hb_address());

    let (iopub_tx, mut iopub_rx) = unbounded_channel::<Vec<bytes::Bytes>>();

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

    // Initiate code execution count
    let exec_count = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));

    //Prepare cancelation tokens
    let cancel = CancellationToken::new();
    let cancel_iopub = cancel.clone();
    let cancel_shell = cancel.clone();
    let cancel_hb = cancel.clone();
    let cancel_ctrl = cancel.clone();

    let iopub_handle = tokio::spawn(async move {
        loop {
            tokio::select! {
                _ = cancel_iopub.cancelled() => {
                    println!("IOPub loop cancelled");
                    break;
                }
                Some(frames) = iopub_rx.recv() => {
                    // frames are already multipart bytes
                    let _ = crate::messages::wire::send_bytes(&mut iopub_socket, frames).await;
                }
                else => break,
            }
        }
    });

    // Spawn shell handler
    let shell_iopub_tx = iopub_tx.clone();
    let shell_handle = tokio::spawn(async move {
        shell_loop(
            cancel_shell,
            &mut shell_socket,
            shell_iopub_tx,
            &shell_config,
            exec_count,
        )
        .await
    });

    // Spawn heartbeat handler
    let heartbeat_handle =
        tokio::spawn(async move { heartbeat_loop(cancel_hb, &mut hb_socket).await });

    // Spawn control handler
    let control_iopub_tx = iopub_tx.clone();
    let control_handler = tokio::spawn(async move {
        control_loop(
            cancel,
            cancel_ctrl,
            &mut control_socket,
            control_iopub_tx,
            &control_config,
        )
        .await
    });

    // Wait for tasks (they should run until cancelled)
    let _ = tokio::join!(
        heartbeat_handle,
        shell_handle,
        control_handler,
        iopub_handle
    );

    Ok(())
}
