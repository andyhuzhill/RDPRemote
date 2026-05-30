// Test: ClipboardManager basic functionality

use rdp_common::clipboard::{ClipboardContent, ClipboardManager};

#[test]
fn test_clipboard_manager_new() {
    let manager = ClipboardManager::new();
    assert!(manager.get_last_content().is_none());
}

#[test]
fn test_clipboard_content_text() {
    let content = ClipboardContent::Text("Hello World".to_string());
    match content {
        ClipboardContent::Text(text) => {
            assert_eq!(text, "Hello World");
        }
    }
}

#[test]
fn test_clipboard_manager_set_content() {
    let mut manager = ClipboardManager::new();
    let content = ClipboardContent::Text("Test content".to_string());
    manager.set_content(content.clone());
    
    assert!(manager.get_last_content().is_some());
    match manager.get_last_content().as_ref().unwrap() {
        ClipboardContent::Text(text) => {
            assert_eq!(text, "Test content");
        }
    }
}

#[test]
fn test_clipboard_manager_check_changes_detects_change() {
    let mut manager = ClipboardManager::new();
    
    // First check should return None (no previous content)
    assert!(manager.check_changes().is_none());
    
    // Set some content
    let content = ClipboardContent::Text("New content".to_string());
    manager.set_content(content.clone());
    
    // Check should return the new content
    let changed = manager.check_changes();
    assert!(changed.is_some());
    match changed.unwrap() {
        ClipboardContent::Text(text) => {
            assert_eq!(text, "New content");
        }
    }
    
    // Second check should return None (no change since last check)
    assert!(manager.check_changes().is_none());
}

#[test]
fn test_clipboard_manager_check_changes_detects_multiple_changes() {
    let mut manager = ClipboardManager::new();
    
    // Set first content
    manager.set_content(ClipboardContent::Text("First".to_string()));
    let changed = manager.check_changes();
    assert!(changed.is_some());
    
    // Set second content
    manager.set_content(ClipboardContent::Text("Second".to_string()));
    let changed = manager.check_changes();
    assert!(changed.is_some());
    match changed.unwrap() {
        ClipboardContent::Text(text) => {
            assert_eq!(text, "Second");
        }
    }
}
