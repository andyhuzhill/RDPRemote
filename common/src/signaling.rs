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
    #[serde(rename = "input")]
    Input { event: InputEvent },
}

/// 输入事件类型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum InputEvent {
    #[serde(rename = "mouse_move")]
    MouseMove { x: i32, y: i32 },
    #[serde(rename = "mouse_button")]
    MouseButton { button: MouseButton, pressed: bool },
    #[serde(rename = "mouse_wheel")]
    MouseWheel { delta: i32 },
    #[serde(rename = "keyboard")]
    Keyboard { key: u16, pressed: bool },
}

/// 鼠标按钮类型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MouseButton {
    #[serde(rename = "left")]
    Left,
    #[serde(rename = "right")]
    Right,
    #[serde(rename = "middle")]
    Middle,
}
