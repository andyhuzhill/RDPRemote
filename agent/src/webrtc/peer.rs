//! WebRTC peer connection implementation for RDP Agent
//!
//! This module provides the `AgentPeer` struct for establishing
//! WebRTC peer connections and sending video frames.

use std::sync::Arc;
use webrtc::{
    api::APIBuilder,
    peer_connection::{
        configuration::RTCConfiguration,
        sdp::session_description::RTCSessionDescription,
        RTCPeerConnection,
    },
    track::track_local::track_local_static_sample::TrackLocalStaticSample,
    interceptor::registry::Registry,
    media::Sample,
    rtp_transceiver::rtp_codec::RTCRtpCodecCapability,
};

use crate::webrtc::{Error, Result};

/// AgentPeer represents a WebRTC peer connection for video streaming
pub struct AgentPeer {
    peer_connection: Arc<RTCPeerConnection>,
    video_track: Arc<TrackLocalStaticSample>,
}

impl AgentPeer {
    /// Create a new AgentPeer instance
    ///
    /// This sets up the WebRTC peer connection with the necessary
    /// interceptors and creates a video track for sending frames.
    pub async fn new() -> Result<Self> {
        // Create interceptor registry with default interceptors
        let registry = Registry::new();

        // Build API with interceptors
        let api = APIBuilder::new()
            .with_interceptor_registry(registry)
            .build();

        // Create peer connection configuration (no STUN/TURN for direct connection)
        let config = RTCConfiguration {
            ice_servers: vec![],
            ..Default::default()
        };

        // Create peer connection
        let peer_connection = api
            .new_peer_connection(config)
            .await
            .map_err(|e| Error::WebRtc(format!("Failed to create peer connection: {}", e)))?;

        // Create video codec capability for VP8
        let codec = RTCRtpCodecCapability {
            mime_type: "video/VP8".to_string(),
            clock_rate: 90000,
            channels: 0,
            sdp_fmtp_line: "level-asymmetry-allowed=1;packetization-mode=1".to_string(),
            rtcp_feedback: vec![],
        };

        // Create video track for sending
        let video_track = Arc::new(
            TrackLocalStaticSample::new(codec, "video".to_string(), "video-rdp".to_string())
        );

        // Add track to peer connection
        peer_connection
            .add_track(Arc::clone(&video_track) as Arc<dyn webrtc::track::track_local::TrackLocal + Send + Sync>)
            .await
            .map_err(|e| Error::WebRtc(format!("Failed to add track: {}", e)))?;

        Ok(Self {
            peer_connection: Arc::new(peer_connection),
            video_track,
        })
    }

    /// Create an SDP offer for the peer connection
    ///
    /// This generates an offer that describes the local media capabilities
    /// and must be sent to the remote peer via signaling.
    pub async fn create_offer(&self) -> Result<String> {
        let offer = self
            .peer_connection
            .create_offer(None)
            .await
            .map_err(|e| Error::WebRtc(format!("Failed to create offer: {}", e)))?;

        self.peer_connection
            .set_local_description(offer.clone())
            .await
            .map_err(|e| Error::WebRtc(format!("Failed to set local description: {}", e)))?;

        // Convert to string for signaling
        let sdp = offer.sdp;
        Ok(sdp)
    }

    /// Set the remote SDP answer
    ///
    /// After receiving an answer from the remote peer via signaling,
    /// call this to complete the connection setup.
    pub async fn set_answer(&self, sdp: String) -> Result<()> {
        let description = RTCSessionDescription::answer(sdp)
            .map_err(|e| Error::Sdp(format!("Failed to create answer: {}", e)))?;

        self.peer_connection
            .set_remote_description(description)
            .await
            .map_err(|e| Error::WebRtc(format!("Failed to set remote description: {}", e)))?;

        Ok(())
    }

    /// Send a video frame over the WebRTC connection
    ///
    /// # Arguments
    /// * `data` - The encoded video frame data (e.g., VP8/VP9)
    /// * `duration_us` - Frame duration in microseconds
    /// * `is_keyframe` - Whether this is a keyframe (I-frame)
    pub async fn send_video_frame(
        &self,
        data: Vec<u8>,
        duration_us: u64,
        _is_keyframe: bool,
    ) -> Result<()> {
        let sample = Sample {
            data: bytes::Bytes::from(data),
            duration: std::time::Duration::from_micros(duration_us),
            ..Default::default()
        };

        self.video_track
            .write_sample(&sample)
            .await
            .map_err(|e| Error::Send(format!("Failed to write sample: {}", e)))?;

        Ok(())
    }

    /// Add an ICE candidate received from the remote peer
    ///
    /// # Arguments
    /// * `candidate` - The ICE candidate string (SDP format)
    /// * `sdp_mid` - The SDP media stream identifier
    /// * `sdp_m_line_index` - The index of the media description in the SDP
    pub async fn add_ice_candidate(
        &self,
        candidate: String,
        sdp_mid: String,
        sdp_m_line_index: u16,
    ) -> Result<()> {
        use webrtc::ice_transport::ice_candidate::RTCIceCandidateInit;

        let candidate_init = RTCIceCandidateInit {
            candidate,
            sdp_mid: Some(sdp_mid),
            sdp_mline_index: Some(sdp_m_line_index),
            ..Default::default()
        };

        self.peer_connection
            .add_ice_candidate(candidate_init)
            .await
            .map_err(|e| Error::Ice(format!("Failed to add ICE candidate: {}", e)))?;

        Ok(())
    }

    /// Get the current connection state
    pub fn connection_state(&self) -> webrtc::peer_connection::peer_connection_state::RTCPeerConnectionState {
        self.peer_connection.connection_state()
    }

    /// Get the ICE connection state
    pub fn ice_connection_state(&self) -> webrtc::ice_transport::ice_connection_state::RTCIceConnectionState {
        self.peer_connection.ice_connection_state()
    }

    /// Check if the peer connection is connected
    pub fn is_connected(&self) -> bool {
        use webrtc::peer_connection::peer_connection_state::RTCPeerConnectionState;
        matches!(
            self.connection_state(),
            RTCPeerConnectionState::Connected
        )
    }
}

impl std::fmt::Debug for AgentPeer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AgentPeer")
            .field("connection_state", &self.connection_state())
            .field("ice_connection_state", &self.ice_connection_state())
            .finish()
    }
}
