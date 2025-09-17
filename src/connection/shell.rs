use std::sync::Arc;
use std::sync::atomic::AtomicU32;

use tokio_util::sync::CancellationToken;
use zeromq::SocketRecv;
use zeromq::{PubSocket, RouterSocket};

use crate::messages::wire::delim_index;
use crate::messages::{ConnectionConfig, JupyterMessage};

use super::iopub::IopubTx;

pub async fn shell_loop(
    cancel_shell: CancellationToken,
    shell_socket: &mut RouterSocket,
    iopub_tx: IopubTx,
    config: &ConnectionConfig,
    exec_count: Arc<AtomicU32>,
) {
    loop {
        tokio::select! {
            _ = cancel_shell.cancelled() => {
                println!("Shell loop cancelled");
                break;
            }
        msg = shell_socket.recv() => {
            match msg {
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
                            crate::messages::shell::kernel_info::handle_kernel_info_request(
                                &config,
                                shell_socket,
                                &iopub_tx,
                                raw_msg,
                                frames,
                                delim_index,
                            )
                            .await;
                        }
                        "execute_request" => {
                            // Add +1 to the execution counter
                            let n = exec_count
                                .fetch_add(1, std::sync::atomic::Ordering::SeqCst)
                                + 1;

                            crate::messages::shell::execute::handle_execute_request(
                                &config,
                                shell_socket,
                                &iopub_tx,
                                raw_msg,
                                frames,
                                delim_index,
                                n,
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

        }
    }
}
