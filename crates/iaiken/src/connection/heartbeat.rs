use tokio_util::sync::CancellationToken;
use zeromq::RepSocket;
use zeromq::{SocketRecv, SocketSend};

pub async fn heartbeat_loop(cancel_hb: CancellationToken, hb_socket: &mut RepSocket) {
    loop {
        tokio::select! {
            _ = cancel_hb.cancelled() => {
                  println!("Heartbeat loop cancelled");
                    break;
            }
            msg = hb_socket.recv() => {
                match msg {
                    Ok(message) => {
                        // Echo message back
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
        }
    }
}
