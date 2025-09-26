use std::sync::Arc;
use std::sync::atomic::AtomicU32;
use std::sync::atomic::Ordering;

use tokio_util::sync::CancellationToken;
use zeromq::RouterSocket;
use zeromq::SocketRecv;

use crate::messages::wire::delim_index;
use crate::messages::{ConnectionConfig, JupyterMessage};

use super::iopub::IopubTx;

mod execute;
mod kernel_info;

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
                            kernel_info::handle_kernel_info_request(
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
                            // Increment execution counter and get the new value
                            // The `Ordering` is probably too strict for this case.
                            exec_count.fetch_add(1, Ordering::SeqCst);
                            let n = exec_count.load(Ordering::SeqCst);

                            execute::handle_execute_request(
                                &config,
                                shell_socket,
                                &iopub_tx,
                                raw_msg,
                                frames,
                                delim_index,
                                n,
                            )
                            .await.unwrap();
                        }
                        _ => {
                            println!("\n\nUnhandled shell message type: {}\n\n", raw_msg.header.msg_type);
                            //TODO: Hanlde `history_request`?
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
