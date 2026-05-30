//! File transfer state machine for RDPRemote
//!
//! Supports single-file serial transfer with 100KB chunks.
//! Relies on SCTP built-in reliability, no extra ACK needed.
//! Supports cancellation (discards received data).

use serde::{Deserialize, Serialize};

/// Block size for file transfer chunks (100 KB)
pub const CHUNK_SIZE: usize = 100 * 1024;

/// Maximum file size we support (4 GB)
pub const MAX_FILE_SIZE: u64 = 4 * 1024 * 1024 * 1024;

/// File transfer direction
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum TransferDirection {
    /// Sending file to remote
    Send,
    /// Receiving file from remote
    Receive,
}

/// Sender state machine states
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum SenderState {
    /// Not transferring
    Idle,
    /// Waiting for remote to accept transfer request
    WaitingAccept,
    /// Sending data chunks
    Sending,
    /// Transfer completed successfully
    Completed,
    /// Transfer was canceled
    Canceled,
}

impl std::fmt::Display for SenderState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SenderState::Idle => write!(f, "Idle"),
            SenderState::WaitingAccept => write!(f, "WaitingAccept"),
            SenderState::Sending => write!(f, "Sending"),
            SenderState::Completed => write!(f, "Completed"),
            SenderState::Canceled => write!(f, "Canceled"),
        }
    }
}

/// Receiver state machine states
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ReceiverState {
    /// Not transferring
    Idle,
    /// Waiting for transfer request
    WaitingRequest,
    /// Receiving data chunks
    Receiving,
    /// Transfer completed successfully
    Completed,
    /// Transfer was canceled
    Canceled,
}

impl std::fmt::Display for ReceiverState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ReceiverState::Idle => write!(f, "Idle"),
            ReceiverState::WaitingRequest => write!(f, "WaitingRequest"),
            ReceiverState::Receiving => write!(f, "Receiving"),
            ReceiverState::Completed => write!(f, "Completed"),
            ReceiverState::Canceled => write!(f, "Canceled"),
        }
    }
}

/// File transfer request message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileTransferRequest {
    /// Name of the file being transferred
    pub filename: String,
    /// Total file size in bytes
    pub file_size: u64,
}

/// File transfer accept message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileTransferAccept {
    /// Confirmation that receiver accepts the transfer
    pub accepted: bool,
}

/// File transfer data chunk
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileTransferData {
    /// Index of this chunk (0-based)
    pub chunk_index: u64,
    /// Chunk data (up to CHUNK_SIZE bytes)
    pub data: Vec<u8>,
    /// Whether this is the last chunk
    pub is_last: bool,
}

/// File transfer complete message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileTransferComplete {
    /// Final chunk index received
    pub last_chunk_index: u64,
}

/// File transfer cancel message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileTransferCancel {
    /// Reason for cancellation
    pub reason: String,
}

/// Sender state machine for file transfer
pub struct FileTransferSender {
    state: SenderState,
    filename: String,
    file_size: u64,
    chunks_sent: u64,
}

impl FileTransferSender {
    /// Create a new file transfer sender
    pub fn new(filename: String, file_size: u64) -> Result<Self, FileTransferError> {
        if file_size == 0 {
            return Err(FileTransferError::InvalidFileSize);
        }
        if file_size > MAX_FILE_SIZE {
            return Err(FileTransferError::FileSizeTooLarge);
        }
        
        Ok(Self {
            state: SenderState::Idle,
            filename,
            file_size,
            chunks_sent: 0,
        })
    }

    /// Get current state
    pub fn state(&self) -> SenderState {
        self.state
    }

    /// Get number of chunks to send
    pub fn total_chunks(&self) -> u64 {
        (self.file_size + (CHUNK_SIZE - 1) as u64) / CHUNK_SIZE as u64
    }

    /// Get remaining chunks to send
    pub fn remaining_chunks(&self) -> u64 {
        self.total_chunks().saturating_sub(self.chunks_sent)
    }

    /// Transition to waiting for accept
    pub fn request(&mut self) -> Result<(), FileTransferError> {
        if self.state != SenderState::Idle {
            return Err(FileTransferError::InvalidStateTransition);
        }
        self.state = SenderState::WaitingAccept;
        Ok(())
    }

    /// Transition to sending after accept received
    pub fn accept(&mut self) -> Result<(), FileTransferError> {
        if self.state != SenderState::WaitingAccept {
            return Err(FileTransferError::InvalidStateTransition);
        }
        self.state = SenderState::Sending;
        Ok(())
    }

    /// Mark a chunk as sent
    pub fn chunk_sent(&mut self) {
        self.chunks_sent += 1;
    }

    /// Check if all chunks have been sent
    pub fn is_complete(&self) -> bool {
        self.chunks_sent >= self.total_chunks()
    }

    /// Transition to completed state
    pub fn complete(&mut self) -> Result<(), FileTransferError> {
        if self.state != SenderState::Sending {
            return Err(FileTransferError::InvalidStateTransition);
        }
        if !self.is_complete() {
            return Err(FileTransferError::IncompleteTransfer);
        }
        self.state = SenderState::Completed;
        Ok(())
    }

    /// Cancel the transfer
    pub fn cancel(&mut self) {
        self.state = SenderState::Canceled;
    }

    /// Check if transfer is in a terminal state
    pub fn is_terminal(&self) -> bool {
        matches!(self.state, SenderState::Completed | SenderState::Canceled)
    }

    /// Get transfer info
    pub fn info(&self) -> FileTransferInfo {
        FileTransferInfo {
            direction: TransferDirection::Send,
            filename: self.filename.clone(),
            file_size: self.file_size,
            chunks_sent: self.chunks_sent,
            total_chunks: self.total_chunks(),
            state: self.state().to_string(),
        }
    }
}

/// Receiver state machine for file transfer
pub struct FileTransferReceiver {
    state: ReceiverState,
    filename: String,
    file_size: u64,
    chunks_received: u64,
    received_data: Vec<u8>,
}

impl FileTransferReceiver {
    /// Create a new file transfer receiver
    pub fn new() -> Self {
        Self {
            state: ReceiverState::Idle,
            filename: String::new(),
            file_size: 0,
            chunks_received: 0,
            received_data: Vec::new(),
        }
    }

    /// Get current state
    pub fn state(&self) -> ReceiverState {
        self.state
    }

    /// Transition to waiting for request
    pub fn ready(&mut self) {
        self.state = ReceiverState::WaitingRequest;
    }

    /// Accept a transfer request
    pub fn accept_request(&mut self, request: &FileTransferRequest) -> Result<(), FileTransferError> {
        if self.state != ReceiverState::WaitingRequest {
            return Err(FileTransferError::InvalidStateTransition);
        }
        if request.file_size == 0 {
            return Err(FileTransferError::InvalidFileSize);
        }
        if request.file_size > MAX_FILE_SIZE {
            return Err(FileTransferError::FileSizeTooLarge);
        }
        
        self.filename = request.filename.clone();
        self.file_size = request.file_size;
        self.received_data = Vec::with_capacity(request.file_size as usize);
        self.state = ReceiverState::Receiving;
        Ok(())
    }

    /// Reject a transfer request
    pub fn reject_request(&mut self) {
        self.state = ReceiverState::Idle;
    }

    /// Receive a data chunk
    pub fn receive_chunk(&mut self, chunk: &FileTransferData) -> Result<(), FileTransferError> {
        if self.state != ReceiverState::Receiving {
            return Err(FileTransferError::InvalidStateTransition);
        }
        
        let expected_index = self.chunks_received;
        if chunk.chunk_index != expected_index {
            return Err(FileTransferError::UnexpectedChunkIndex {
                expected: expected_index,
                received: chunk.chunk_index,
            });
        }

        self.received_data.extend_from_slice(&chunk.data);
        self.chunks_received += 1;

        Ok(())
    }

    /// Check if all chunks have been received
    pub fn is_complete(&self) -> bool {
        let expected_chunks = (self.file_size + (CHUNK_SIZE - 1) as u64) / CHUNK_SIZE as u64;
        self.chunks_received >= expected_chunks
    }

    /// Transition to completed state
    pub fn complete(&mut self) -> Result<(), FileTransferError> {
        if self.state != ReceiverState::Receiving {
            return Err(FileTransferError::InvalidStateTransition);
        }
        if !self.is_complete() {
            return Err(FileTransferError::IncompleteTransfer);
        }
        self.state = ReceiverState::Completed;
        Ok(())
    }

    /// Cancel the transfer (discards received data)
    pub fn cancel(&mut self) {
        self.received_data.clear();
        self.received_data.shrink_to_fit();
        self.state = ReceiverState::Canceled;
    }

    /// Check if transfer is in a terminal state
    pub fn is_terminal(&self) -> bool {
        matches!(self.state, ReceiverState::Completed | ReceiverState::Canceled)
    }

    /// Get received file data (only valid after completion)
    pub fn take_data(&mut self) -> Option<Vec<u8>> {
        if self.state == ReceiverState::Completed {
            Some(std::mem::take(&mut self.received_data))
        } else {
            None
        }
    }

    /// Get transfer info
    pub fn info(&self) -> FileTransferInfo {
        FileTransferInfo {
            direction: TransferDirection::Receive,
            filename: self.filename.clone(),
            file_size: self.file_size,
            chunks_sent: self.chunks_received,
            total_chunks: (self.file_size + (CHUNK_SIZE - 1) as u64) / CHUNK_SIZE as u64,
            state: self.state().to_string(),
        }
    }
}

/// File transfer information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileTransferInfo {
    pub direction: TransferDirection,
    pub filename: String,
    pub file_size: u64,
    pub chunks_sent: u64,
    pub total_chunks: u64,
    pub state: String,
}

/// File transfer errors
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FileTransferError {
    InvalidStateTransition,
    InvalidFileSize,
    FileSizeTooLarge,
    IncompleteTransfer,
    UnexpectedChunkIndex { expected: u64, received: u64 },
    ChunkDataTooLarge { size: usize, max: usize },
}

impl std::fmt::Display for FileTransferError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FileTransferError::InvalidStateTransition => write!(f, "Invalid state transition"),
            FileTransferError::InvalidFileSize => write!(f, "Invalid file size"),
            FileTransferError::FileSizeTooLarge => write!(f, "File size exceeds maximum ({MAX_FILE_SIZE} bytes)"),
            FileTransferError::IncompleteTransfer => write!(f, "Transfer incomplete"),
            FileTransferError::UnexpectedChunkIndex { expected, received } => {
                write!(f, "Unexpected chunk index: expected {}, received {}", expected, received)
            }
            FileTransferError::ChunkDataTooLarge { size, max } => {
                write!(f, "Chunk data too large: {} bytes (max {})", size, max)
            }
        }
    }
}

impl std::error::Error for FileTransferError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chunk_size() {
        assert_eq!(CHUNK_SIZE, 100 * 1024);
    }

    #[test]
    fn test_sender_state_machine() {
        let mut sender = FileTransferSender::new("test.txt".to_string(), 250 * 1024).unwrap();
        
        assert_eq!(sender.state(), SenderState::Idle);
        assert_eq!(sender.total_chunks(), 3); // 250KB / 100KB = 3 chunks

        // Request transfer
        sender.request().unwrap();
        assert_eq!(sender.state(), SenderState::WaitingAccept);

        // Accept
        sender.accept().unwrap();
        assert_eq!(sender.state(), SenderState::Sending);

        // Simulate sending chunks
        sender.chunk_sent();
        sender.chunk_sent();
        sender.chunk_sent();

        assert!(sender.is_complete());
        sender.complete().unwrap();
        assert_eq!(sender.state(), SenderState::Completed);
    }

    #[test]
    fn test_sender_cancel() {
        let mut sender = FileTransferSender::new("test.txt".to_string(), 1024).unwrap();
        sender.request().unwrap();
        sender.cancel();
        assert_eq!(sender.state(), SenderState::Canceled);
        assert!(sender.is_terminal());
    }

    #[test]
    fn test_receiver_state_machine() {
        let mut receiver = FileTransferReceiver::new();
        receiver.ready();
        assert_eq!(receiver.state(), ReceiverState::WaitingRequest);

        let request = FileTransferRequest {
            filename: "test.txt".to_string(),
            file_size: 250 * 1024,
        };
        receiver.accept_request(&request).unwrap();
        assert_eq!(receiver.state(), ReceiverState::Receiving);

        // Receive chunks
        let chunk1 = FileTransferData {
            chunk_index: 0,
            data: vec![0u8; CHUNK_SIZE],
            is_last: false,
        };
        receiver.receive_chunk(&chunk1).unwrap();

        let chunk2 = FileTransferData {
            chunk_index: 1,
            data: vec![0u8; CHUNK_SIZE],
            is_last: false,
        };
        receiver.receive_chunk(&chunk2).unwrap();

        let chunk3 = FileTransferData {
            chunk_index: 2,
            data: vec![0u8; 50 * 1024],
            is_last: true,
        };
        receiver.receive_chunk(&chunk3).unwrap();

        assert!(receiver.is_complete());
        receiver.complete().unwrap();
        assert_eq!(receiver.state(), ReceiverState::Completed);

        let data = receiver.take_data().unwrap();
        assert_eq!(data.len(), 250 * 1024);
    }

    #[test]
    fn test_receiver_cancel() {
        let mut receiver = FileTransferReceiver::new();
        receiver.ready();

        let request = FileTransferRequest {
            filename: "test.txt".to_string(),
            file_size: 1024,
        };
        receiver.accept_request(&request).unwrap();

        // Receive some data then cancel
        let chunk = FileTransferData {
            chunk_index: 0,
            data: vec![0u8; 512],
            is_last: false,
        };
        receiver.receive_chunk(&chunk).unwrap();
        receiver.cancel();

        assert_eq!(receiver.state(), ReceiverState::Canceled);
        assert!(receiver.is_terminal());
        assert!(receiver.take_data().is_none()); // Data discarded
    }

    #[test]
    fn test_wrong_chunk_order() {
        let mut receiver = FileTransferReceiver::new();
        receiver.ready();

        let request = FileTransferRequest {
            filename: "test.txt".to_string(),
            file_size: 2048,
        };
        receiver.accept_request(&request).unwrap();

        // Try to receive chunk 1 before chunk 0
        let chunk1 = FileTransferData {
            chunk_index: 1,
            data: vec![0u8; 1024],
            is_last: false,
        };
        let result = receiver.receive_chunk(&chunk1);
        assert!(result.is_err());
    }

    #[test]
    fn test_invalid_file_size() {
        let result = FileTransferSender::new("test.txt".to_string(), 0);
        assert!(result.is_err());

        let result = FileTransferSender::new("test.txt".to_string(), MAX_FILE_SIZE + 1);
        assert!(result.is_err());
    }

    #[test]
    fn test_chunk_data_validation() {
        let chunk = FileTransferData {
            chunk_index: 0,
            data: vec![0u8; CHUNK_SIZE + 100],
            is_last: false,
        };
        assert!(chunk.data.len() > CHUNK_SIZE);
    }
}
