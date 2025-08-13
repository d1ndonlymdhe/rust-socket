#[derive(Debug,Clone,PartialEq)]
pub struct Message {
    pub peer: usize,
    pub content: Vec<u8>,
    pub has_more: bool,
}

#[derive(Debug, PartialEq)]
pub enum ParseError {
    PeerLessThanFour,
    PeerNotUsize,
    NoContent,
    NoEnding,
}

impl Message {
    pub fn new(peer: usize, content: Vec<u8>, has_more: bool) -> Self {
        Message { peer, content, has_more }
    }

    pub fn parse(input: &Vec<u8>) -> Result<Self, ParseError> {
        let mut input = input.iter();
        let peer = input.by_ref().take(4).cloned().collect::<Vec<_>>();
        let peer = String::from_utf8(peer);
        if peer.is_err() {
            return Err(ParseError::PeerNotUsize);
        }
        let peer = peer.unwrap();
        if peer.len() != 4 {
            return Err(ParseError::PeerLessThanFour);
        }
        let peer = peer.parse::<usize>();
        if peer.is_err() {
            return Err(ParseError::PeerNotUsize);
        }
        let peer = peer.unwrap();
        let content = input.collect::<Vec<_>>();
        if content.is_empty() {
            return Err(ParseError::NoContent);
        }
        let length = content.len();
        if *content[length - 1] == b'1' || *content[length - 1] == b'0' {
            let has_more = *content[length - 1] == b'1';

            let content = content
                .into_iter()
                .take(length - 1)
                .cloned()
                .collect::<Vec<_>>();
            return Ok(Message {
                peer,
                content,
                has_more,
            });
        }
        return Err(ParseError::NoEnding);
    }

    fn encode_inner(peer: usize, content: &Vec<u8>)->(Vec<u8>,bool){
        let peer = format!("{:04}", peer);
        let has_more = if content.len() > 1024 { b'1' } else { b'0' };
        let content = content.iter().take(1024);
        let res = peer.as_bytes().into_iter().chain(content).chain(vec![has_more].iter()).cloned().collect::<Vec<u8>>();
        (res, has_more == b'1')
    }

    pub fn encode(peer: usize, content: &Vec<u8>) -> Vec<Vec<u8>> {
        let mut res = Vec::new();
        let mut content = content.clone();
        let (part, mut has_more) = Self::encode_inner(peer, &content);
        content = content.into_iter().skip(1024).collect::<Vec<_>>();
        res.push(part);
        while has_more {
            let (part, has_more_inner) = Self::encode_inner(peer, &content);
            res.push(part);
            content = content.into_iter().skip(1024).collect::<Vec<_>>();
            has_more = has_more_inner;
        }
        return res;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_valid_simple_message() {
        // Format: "0042" + "Hello!" + "0"
        let mut input = "0042".as_bytes().to_vec();
        input.extend_from_slice("Hello!".as_bytes());
        input.push(b'0');
        
        let message = Message::parse(&input).unwrap();
        assert_eq!(message.peer, 42);
        assert_eq!(message.content, "Hello!".as_bytes().to_vec());
        assert_eq!(message.has_more, false);
    }

    #[test]
    fn test_parse_valid_with_more_flag() {
        // Format: "0123" + "data" + "1" (has more)
        let mut input = "0123".as_bytes().to_vec();
        input.extend_from_slice("data".as_bytes());
        input.push(b'1');
        
        let message = Message::parse(&input).unwrap();
        assert_eq!(message.peer, 123);
        assert_eq!(message.content, "data".as_bytes().to_vec());
        assert_eq!(message.has_more, true);
    }

    #[test]
    fn test_parse_peer_not_usize() {
        // Invalid peer (contains non-digit)
        let mut input = "abcd".as_bytes().to_vec();
        input.extend_from_slice("Hello!".as_bytes());
        input.push(b'0');
        
        let result = Message::parse(&input);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), ParseError::PeerNotUsize);
    }

    #[test]
    fn test_parse_peer_less_than_four() {
        // Input too short for peer
        let input = "12".as_bytes().to_vec();
        
        let result = Message::parse(&input);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), ParseError::PeerLessThanFour);
    }

    #[test]
    fn test_parse_no_content() {
        // Only peer, no content or flag
        let input = "0042".as_bytes().to_vec();
        
        let result = Message::parse(&input);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), ParseError::NoContent);
    }

    #[test]
    fn test_parse_invalid_flag() {
        // Invalid flag (not '0' or '1')
        let mut input = "0042".as_bytes().to_vec();
        input.extend_from_slice("Hello!".as_bytes());
        input.push(b'x'); // Invalid flag
        
        let result = Message::parse(&input);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), ParseError::NoContent);
    }

    #[test]
    fn test_parse_binary_content() {
        // Test with binary data
        let mut input = "0001".as_bytes().to_vec();
        input.extend_from_slice(&[0xFF, 0x00, 0xAA, 0x55]); // Binary data
        input.push(b'0');
        
        let message = Message::parse(&input).unwrap();
        assert_eq!(message.peer, 1);
        assert_eq!(message.content, vec![0xFF, 0x00, 0xAA, 0x55]);
        assert_eq!(message.has_more, false);
    }

    #[test]
    fn test_encode_simple_message() {
        let content = "Hello, World!".as_bytes().to_vec();
        let result = Message::encode(42, &content);
        
        assert_eq!(result.len(), 1); // Single chunk
        
        let expected = "0042Hello, World!0".as_bytes().to_vec();
        assert_eq!(result[0], expected);
    }

    #[test]
    fn test_encode_large_message() {
        // Create content larger than 1024 bytes
        let content = vec![b'X'; 2000];
        let result = Message::encode(123, &content);
        
        assert_eq!(result.len(), 2); // Should be split into 2 chunks
        
        // First chunk: "0123" + 1024 'X's + "1"
        let mut expected_first = "0123".as_bytes().to_vec();
        expected_first.extend(vec![b'X'; 1024]);
        expected_first.push(b'1');
        
        // Second chunk: "0123" + remaining 976 'X's + "0"
        let mut expected_second = "0123".as_bytes().to_vec();
        expected_second.extend(vec![b'X'; 976]);
        expected_second.push(b'0');
        
        assert_eq!(result[0], expected_first);
        assert_eq!(result[1], expected_second);
    }

    #[test]
    fn test_encode_exactly_1024_bytes() {
        // Content exactly 1024 bytes
        let content = vec![b'A'; 1024];
        let result = Message::encode(1, &content);
        
        assert_eq!(result.len(), 1); // Single chunk
        
        let mut expected = "0001".as_bytes().to_vec();
        expected.extend(vec![b'A'; 1024]);
        expected.push(b'0'); // No more chunks
        
        assert_eq!(result[0], expected);
    }

    #[test]
    fn test_encode_empty_content() {
        let content = vec![];
        let result = Message::encode(999, &content);
        
        assert_eq!(result.len(), 1);
        
        let expected = "0999".as_bytes().to_vec();
        let mut expected = expected;
        expected.push(b'0');
        
        assert_eq!(result[0], expected);
    }

    #[test]
    fn test_roundtrip_simple() {
        // Test encode -> parse roundtrip
        let original_peer = 42;
        let original_content = "Hello, World!".as_bytes().to_vec();
        
        let encoded = Message::encode(original_peer, &original_content);
        assert_eq!(encoded.len(), 1);
        
        let parsed = Message::parse(&encoded[0]).unwrap();
        assert_eq!(parsed.peer, original_peer);
        assert_eq!(parsed.content, original_content);
        assert_eq!(parsed.has_more, false);
    }

    #[test]
    fn test_roundtrip_large_message() {
        // Test encode -> parse roundtrip for chunked message
        let original_peer = 123;
        let original_content = vec![b'X'; 1500]; // > 1024 bytes
        
        let encoded = Message::encode(original_peer, &original_content);
        assert_eq!(encoded.len(), 2);
        
        // Parse first chunk
        let first_chunk = Message::parse(&encoded[0]).unwrap();
        assert_eq!(first_chunk.peer, original_peer);
        assert_eq!(first_chunk.content.len(), 1024);
        assert_eq!(first_chunk.has_more, true);
        
        // Parse second chunk
        let second_chunk = Message::parse(&encoded[1]).unwrap();
        assert_eq!(second_chunk.peer, original_peer);
        assert_eq!(second_chunk.content.len(), 476); // 1500 - 1024
        assert_eq!(second_chunk.has_more, false);
        
        // Reconstruct original content
        let mut reconstructed = first_chunk.content;
        reconstructed.extend(second_chunk.content);
        assert_eq!(reconstructed, original_content);
    }
}
