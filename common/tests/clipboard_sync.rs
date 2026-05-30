// Test: Clipboard sync integration

use rdp_common::clipboard::{ClipboardContent, ClipboardManager};
use rdp_common::signaling::SignalingMessage;

#[test]
fn test_clipboard_sync_integration() {
    // Simulate agent reading clipboard and sending via signaling
    let mut agent_manager = ClipboardManager::new();
    
    // Agent detects clipboard change
    agent_manager.set_content(ClipboardContent::Text("Shared content".to_string()));
    let changed = agent_manager.check_changes();
    assert!(changed.is_some());
    
    // Create signaling message
    let msg = SignalingMessage::Clipboard {
        content: "Shared content".to_string(),
    };
    
    // Serialize for transmission
    let serialized = serde_json::to_string(&msg).unwrap();
    assert!(serialized.contains(r#""type":"clipboard""#));
    
    // Client receives and deserializes
    let received: SignalingMessage = serde_json::from_str(&serialized).unwrap();
    
    // Client sets clipboard
    let mut client_manager = ClipboardManager::new();
    match received {
        SignalingMessage::Clipboard { content } => {
            client_manager.set_content(ClipboardContent::Text(content));
        }
        _ => panic!("Expected Clipboard message"),
    }
    
    // Verify client received the content
    let client_changed = client_manager.check_changes();
    assert!(client_changed.is_some());
    match client_changed.unwrap() {
        ClipboardContent::Text(text) => {
            assert_eq!(text, "Shared content");
        }
    }
}

#[test]
fn test_clipboard_no_duplicate_sync() {
    let mut manager = ClipboardManager::new();
    
    // First check returns None (no previous content)
    assert!(manager.check_changes().is_none());
    
    // Set content and check
    manager.set_content(ClipboardContent::Text("Content".to_string()));
    assert!(manager.check_changes().is_some());
    
    // Second check should return None (no change since last check)
    assert!(manager.check_changes().is_none());
}
