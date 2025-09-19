use tokio::sync::mpsc::UnboundedSender;

pub type IopubTx = UnboundedSender<Vec<bytes::Bytes>>;
