// Test: Agent clipboard module

#[cfg(test)]
mod tests {
    use rdp_agent::clipboard::{ClipboardContent, ClipboardReader, ClipboardWriter};

    #[test]
    fn test_clipboard_reader_new() {
        let reader = ClipboardReader::new();
        assert!(reader.is_ok());
    }

    #[test]
    fn test_clipboard_writer_new() {
        let writer = ClipboardWriter::new();
        assert!(writer.is_ok());
    }

    #[test]
    fn test_clipboard_content_from_string() {
        let content = ClipboardContent::from("Hello World");
        match content {
            ClipboardContent::Text(text) => {
                assert_eq!(text, "Hello World");
            }
        }
    }
}
