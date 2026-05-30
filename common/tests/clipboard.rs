// Test: clipboard message serialization

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
enum TestClipboardMessage {
    #[serde(rename = "clipboard")]
    Clipboard { content: String },
}

#[test]
fn test_clipboard_message_serialization() {
    let msg = TestClipboardMessage::Clipboard {
        content: "Hello World".to_string(),
    };
    
    let serialized = serde_json::to_string(&msg).unwrap();
    assert!(serialized.contains(r#""type":"clipboard""#));
    assert!(serialized.contains(r#""content":"Hello World""#));
    
    let deserialized: TestClipboardMessage = serde_json::from_str(&serialized).unwrap();
    match deserialized {
        TestClipboardMessage::Clipboard { content } => {
            assert_eq!(content, "Hello World");
        }
    }
}

// Test: clipboard sync flow
#[test]
fn test_clipboard_sync_flow() {
    // Agent side: read clipboard, send via signaling
    let clipboard_content = "Test content from agent";
    let msg = TestClipboardMessage::Clipboard {
        content: clipboard_content.to_string(),
    };
    
    // Verify message structure
    match &msg {
        TestClipboardMessage::Clipboard { content } => {
            assert_eq!(content, clipboard_content);
        }
    }
    
    // Client side: receive and set clipboard
    let serialized = serde_json::to_string(&msg).unwrap();
    let received: TestClipboardMessage = serde_json::from_str(&serialized).unwrap();
    
    match received {
        TestClipboardMessage::Clipboard { content } => {
            assert_eq!(content, clipboard_content);
        }
    }
}
