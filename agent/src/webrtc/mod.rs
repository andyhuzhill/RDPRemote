//! WebRTC module for RDP Agent
//!
//! Provides WebRTC peer connection functionality for video streaming.

pub mod peer;

pub use peer::AgentPeer;

/// Result type for WebRTC operations
pub type Result<T> = std::result::Result<T, Error>;

/// WebRTC error types
#[derive(Debug)]
pub enum Error {
    /// WebRTC API error
    WebRtc(String),
    /// SDP parsing error
    Sdp(String),
    /// ICE candidate error
    Ice(String),
    /// Track error
    Track(String),
    /// Send error
    Send(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::WebRtc(e) => write!(f, "WebRTC error: {}", e),
            Error::Sdp(e) => write!(f, "SDP error: {}", e),
            Error::Ice(e) => write!(f, "ICE error: {}", e),
            Error::Track(e) => write!(f, "Track error: {}", e),
            Error::Send(e) => write!(f, "Send error: {}", e),
        }
    }
}

impl std::error::Error for Error {}

impl From<webrtc::Error> for Error {
    fn from(e: webrtc::Error) -> Self {
        Error::WebRtc(e.to_string())
    }
}
