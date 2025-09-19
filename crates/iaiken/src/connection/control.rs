use tokio_util::sync::CancellationToken;
use zeromq::RouterSocket;
use zeromq::SocketRecv;

use crate::messages::control::shutdown::{ShutdownReply, ShutdownRequest};
use crate::messages::wire::{delim_index, send_bytes};
use crate::messages::{ConnectionConfig, JupyterMessage, MessageHeader};

use super::iopub::IopubTx;

pub async fn control_loop(
    cancel: CancellationToken,
    cancel_ctrl: CancellationToken,
    control_socket: &mut RouterSocket,
    iopub_tx: IopubTx,
    config: &ConnectionConfig,
) {
    loop {
        tokio::select! {
            _ = cancel_ctrl.cancelled() => {
                println!("Control loop cancelled");
                break;
            }
            recv = control_socket.recv() => {
                match recv {
                    Ok(message) => {
                        let frames: Vec<Vec<u8>> = message.iter().map(|f| f.to_vec()).collect();
                        let ix = match delim_index(&frames) {
                            Ok(i) => i,
                            Err(e) => { eprintln!("{e}"); continue; }
                        };
                        // Parse as ShutdownRequest
                        if let Ok(raw_msg) = JupyterMessage::<serde_json::Value>::from_multipart(
                            &frames, &config.key, &config.signature_scheme
                        ) {
                            match raw_msg.header.msg_type.as_str()  {
                                "shutdown_request" => {

                                 if let Ok(frames) = raw_msg.to_iopub_status(&config.key, &config.signature_scheme, "busy") {
                                    let _ = iopub_tx.send(frames);
                                }

                                let req = JupyterMessage::<ShutdownRequest>::from_multipart(
                                    &frames, &config.key, &config.signature_scheme
                                ).ok();
                                let restart = req.as_ref().map(|m| m.content.restart).unwrap_or(false);

                                // Build reply
                                let reply_header = MessageHeader::new(
                                    raw_msg.header.session.clone(),
                                    "shutdown_reply".to_string()
                                );
                                let reply = ShutdownReply { restart };
                                let reply_msg = JupyterMessage {
                                    header: reply_header,
                                    parent_header: Some(raw_msg.header.clone()),
                                    metadata: serde_json::Value::Object(serde_json::Map::new()),
                                    content: reply,
                                };
                                // Reuse identity envelope to send reply
                                if let Ok(bytes_frames) = reply_msg.to_envelope_multipart(
                                    frames, ix, &config.key, &config.signature_scheme
                                ) {
                                    // Send reply then cancel
                                    if let Err(e) = send_bytes(control_socket, bytes_frames).await {
                                        eprintln!("Failed to send shutdown_reply: {e}");
                                    }
                                    cancel.cancel(); // Shutdown! (cancell all loops)
                                    break;
                                }


                                if let Ok(frames) = raw_msg.to_iopub_status(&config.key, &config.signature_scheme, "idle") {
                                    let _ = iopub_tx.send(frames);
                                }
                            },
                            _ => {
                                println!("\n\nUnhandled control message type: {}\n\n", raw_msg.header.msg_type);
                                //Unhandled control message type: kernel_info_request
                            }
                            }
                        }
                    }
                    Err(e) => { eprintln!("Control receive error: {e}"); break; }
                }
            }
        }
    }
}
