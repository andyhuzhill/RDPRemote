//! Clipboard content types and manager for RDPRemote
//!
//! This module provides shared types for clipboard synchronization
//! between agent and client.

/// 剪贴板内容类型
#[derive(Debug, Clone, PartialEq)]
pub enum ClipboardContent {
    Text(String),
}

/// 剪贴板管理器
pub struct ClipboardManager {
    last_content: Option<ClipboardContent>,
    last_checked: Option<ClipboardContent>,
}

impl ClipboardManager {
    /// Create a new ClipboardManager
    pub fn new() -> Self {
        Self {
            last_content: None,
            last_checked: None,
        }
    }

    /// 检查剪贴板是否有变化
    /// Returns the new content if there's a change since last check
    pub fn check_changes(&mut self) -> Option<ClipboardContent> {
        if self.last_content != self.last_checked {
            let changed = self.last_content.clone();
            self.last_checked = self.last_content.clone();
            changed
        } else {
            None
        }
    }

    /// 设置剪贴板内容
    pub fn set_content(&mut self, content: ClipboardContent) {
        self.last_content = Some(content);
    }

    /// Get the last known content (for testing)
    pub fn get_last_content(&self) -> Option<&ClipboardContent> {
        self.last_content.as_ref()
    }
}

impl Default for ClipboardManager {
    fn default() -> Self {
        Self::new()
    }
}
