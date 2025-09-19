use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ShutdownRequest {
    pub restart: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ShutdownReply {
    pub restart: bool,
}
