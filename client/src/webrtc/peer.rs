use anyhow::{Context, Result};
use std::sync::Arc;
use tokio::sync::mpsc;
use webrtc::api::{APIBuilder, API};
use webrtc::ice_transport::ice_candidate::RTCIceCandidateInit;
use webrtc::ice_transport::ice_connection_state::RTCIceConnectionState;
use webrtc::ice_transport::ice_gathering_state::RTCIceGatheringState;
use webrtc::interceptor::registry::Registry;
use webrtc::peer_connection::configuration::RTCConfiguration;
use webrtc::peer_connection::peer_connection_state::RTCPeerConnectionState;
use webrtc::peer_connection::RTCPeerConnection;
use webrtc::rtp_transceiver::rtp_codec::RTPCodecType;
use webrtc::rtp_transceiver::rtp_receiver::RTCRtpReceiver;
use webrtc::rtp_transceiver::rtp_transceiver_direction::RTCRtpTransceiverDirection;
use webrtc::rtp_transceiver::RTCRtpTransceiverInit;
use webrtc::track::track_remote::TrackRemote;

/// Received video frame from WebRTC peer
#[derive(Debug, Clone)]
pub struct ReceivedVideoFrame {
    pub data: Vec<u8>,
    pub timestamp_us: u64,
    pub is_keyframe: bool,
}

/// WebRTC client peer for receiving video stream
pub struct ClientPeer {
    peer_connection: Arc<RTCPeerConnection>,
    video_sender: mpsc::Sender<ReceivedVideoFrame>,
    video_receiver: Option<mpsc::Receiver<ReceivedVideoFrame>>,
}

impl ClientPeer {
    /// Create a new WebRTC client peer
    pub async fn new() -> Result<Self> {
        let (video_tx, video_rx) = mpsc::channel(100);

        // Create interceptor registry
        let interceptor_registry = Registry::new();

        // Create API with interceptor registry
        let api: API = APIBuilder::default()
            .with_interceptor_registry(interceptor_registry)
            .build();

        // Create peer connection configuration
        let config = RTCConfiguration::default();

        // Create peer connection
        let peer_connection = api.new_peer_connection(config).await?;

        Ok(Self {
            peer_connection: Arc::new(peer_connection),
            video_sender: video_tx,
            video_receiver: Some(video_rx),
        })
    }

    /// Get a clone of the video frame sender channel
    pub fn video_sender(&self) -> mpsc::Sender<ReceivedVideoFrame> {
        self.video_sender.clone()
    }

    /// Take ownership of the video receiver channel
    pub fn take_video_receiver(&mut self) -> Option<mpsc::Receiver<ReceivedVideoFrame>> {
        self.video_receiver.take()
    }

    /// Create an answer SDP for the given offer
    /// Must be called after set_offer() and add_video_transceiver()
    pub async fn create_answer(&self) -> Result<String> {
        // Create answer
        let answer = self.peer_connection.create_answer(None).await?;

        Ok(answer.sdp)
    }

    /// Set the remote offer SDP
    pub async fn set_offer(&self, sdp: &str) -> Result<()> {
        use webrtc::peer_connection::sdp::session_description::RTCSessionDescription;

        let offer = RTCSessionDescription::offer(sdp.to_string())
            .context("Failed to parse offer SDP")?;

        self.peer_connection
            .set_remote_description(offer)
            .await
            .context("Failed to set remote offer")?;

        Ok(())
    }

    /// Set the local description (answer)
    pub async fn set_local_description(&self, sdp: &str) -> Result<()> {
        use webrtc::peer_connection::sdp::session_description::RTCSessionDescription;

        let answer = RTCSessionDescription::answer(sdp.to_string())
            .context("Failed to parse answer SDP")?;

        self.peer_connection
            .set_local_description(answer)
            .await
            .context("Failed to set local description")?;

        Ok(())
    }

    /// Add a video transceiver for receiving video (call before create_answer)
    pub async fn add_video_transceiver(&self) -> Result<()> {
        let transceiver_init = RTCRtpTransceiverInit {
            direction: RTCRtpTransceiverDirection::Recvonly,
            send_encodings: vec![],
        };

        self.peer_connection
            .add_transceiver_from_kind(RTPCodecType::Video, Some(transceiver_init))
            .await
            .context("Failed to add video transceiver")?;

        Ok(())
    }

    /// Add an ICE candidate
    pub async fn add_ice_candidate(&self, candidate: &str, sdp_mline_index: u16, sdp_mid: &str) -> Result<()> {
        let candidate_init = RTCIceCandidateInit {
            candidate: candidate.to_string(),
            sdp_mline_index: Some(sdp_mline_index),
            sdp_mid: Some(sdp_mid.to_string()),
            username_fragment: None,
        };

        self.peer_connection
            .add_ice_candidate(candidate_init)
            .await
            .context("Failed to add ICE candidate")?;

        Ok(())
    }

    /// Register a callback to receive ICE candidates generated locally
    ///
    /// This method sets up a handler that will be called whenever a new
    /// ICE candidate is gathered. The callback should forward the candidate
    /// to the remote peer via signaling.
    ///
    /// # Arguments
    /// * `callback` - A boxed async closure that receives the ICE candidate
    pub fn on_ice_candidate<F>(&self, callback: F)
    where
        F: Fn(webrtc::ice_transport::ice_candidate::RTCIceCandidate) -> futures_util::future::BoxFuture<'static, ()> + Send + Sync + Clone + 'static,
    {
        let peer_connection = Arc::clone(&self.peer_connection);

        peer_connection.on_ice_candidate(Box::new(move |candidate: Option<webrtc::ice_transport::ice_candidate::RTCIceCandidate>| {
            let callback = callback.clone();
            Box::pin(async move {
                if let Some(candidate) = candidate {
                    callback(candidate).await;
                }
            })
        }));
    }

    /// Start receiving video frames from the peer connection
    /// Sets up the on_track handler to receive incoming video tracks
    pub fn start_receiving_video(&self) {
        let video_sender = self.video_sender.clone();
        let peer_connection = Arc::clone(&self.peer_connection);

        peer_connection.on_track(Box::new(move |track: Arc<TrackRemote>, _receiver: Arc<RTCRtpReceiver>, _transceiver: Arc<webrtc::rtp_transceiver::RTCRtpTransceiver>| {
            let sender = video_sender.clone();
            
            // Spawn a task to read frames from the track
            Box::pin(async move {
                let mut buf = vec![0u8; 1460]; // MTU size buffer
                
                loop {
                    match track.read(&mut buf).await {
                        Ok((pkt, _attributes)) => {
                            // Extract payload data from RTP packet
                            let frame_data = pkt.payload.to_vec();
                            let frame = ReceivedVideoFrame {
                                data: frame_data,
                                timestamp_us: 0, // Webrtc doesn't expose timestamp directly in read
                                is_keyframe: false, // Would need to parse NAL units for H.264
                            };
                            
                            // Send frame through channel (non-blocking)
                            if let Err(_) = sender.try_send(frame) {
                                tracing::warn!("Video frame channel full, dropping frame");
                            }
                        }
                        Err(e) => {
                            tracing::error!("Error reading from track: {}", e);
                            break;
                        }
                    }
                }
            })
        }));
    }

    /// Get the current ICE gathering state
    pub fn ice_gathering_state(&self) -> RTCIceGatheringState {
        self.peer_connection.ice_gathering_state()
    }

    /// Get the current ICE connection state
    pub fn ice_connection_state(&self) -> RTCIceConnectionState {
        self.peer_connection.ice_connection_state()
    }

    /// Get the current peer connection state
    pub fn connection_state(&self) -> RTCPeerConnectionState {
        self.peer_connection.connection_state()
    }

    /// Get the current signaling state
    pub fn signaling_state(&self) -> webrtc::peer_connection::signaling_state::RTCSignalingState {
        self.peer_connection.signaling_state()
    }

    /// Close the peer connection
    pub async fn close(&self) -> Result<()> {
        self.peer_connection.close().await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_received_video_frame_struct() {
        let frame = ReceivedVideoFrame {
            data: vec![1, 2, 3, 4],
            timestamp_us: 123456,
            is_keyframe: true,
        };
        
        assert_eq!(frame.data, vec![1, 2, 3, 4]);
        assert_eq!(frame.timestamp_us, 123456);
        assert!(frame.is_keyframe);
    }

    #[tokio::test]
    async fn test_client_peer_creation() {
        let peer = ClientPeer::new().await;
        assert!(peer.is_ok());
    }
}
