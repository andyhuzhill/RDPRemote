use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum SignalingMessage {
    #[serde(rename = "offer")]
    Offer { sdp: String },
    #[serde(rename = "answer")]
    Answer { sdp: String },
    #[serde(rename = "ice-candidate")]
    IceCandidate { candidate: String, sdp_mid: String, sdp_m_line_index: u16 },
    #[serde(rename = "register")]
    Register { device_id: String },
    #[serde(rename = "connect")]
    Connect { target_device_id: String },
    #[serde(rename = "error")]
    Error { message: String },
}
