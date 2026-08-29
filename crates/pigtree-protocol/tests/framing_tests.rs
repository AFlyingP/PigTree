use pigtree_protocol::crc32c::compute_crc32c;
use pigtree_protocol::frame::{
    decode_frame, encode_frame, read_frame, write_frame, ChannelTag, FrameFlags, FrameHeader,
    FrameParseError, HEADER_SIZE, MAX_PAYLOAD_SIZE,
};

#[test]
fn test_crc32c_known_vectors() {
    // Standard RFC 3720 test vector: 32 bytes of 0x00 -> 0x8a9136aa (or inverted/reflected form)
    // 32 bytes of 0xFF -> 0x62a8ab43
    // "123456789" -> 0xe3069283
    let data = b"123456789";
    let crc = compute_crc32c(data);
    assert_eq!(crc, 0xe3069283);
}

#[test]
fn test_roundtrip_valid_frame() {
    let header = FrameHeader {
        channel_tag: ChannelTag::Command,
        flags: FrameFlags::END_OF_STREAM,
        sequence_number: 42,
    };
    let payload = b"Hello PigTree Protocol!";
    let encoded = encode_frame(&header, payload).expect("encoding should succeed");

    assert_eq!(encoded.len(), 20 + payload.len() + 4);

    let (decoded_header, decoded_payload) =
        decode_frame(&encoded).expect("decoding should succeed");
    assert_eq!(decoded_header.channel_tag, ChannelTag::Command);
    assert_eq!(decoded_header.flags, FrameFlags::END_OF_STREAM);
    assert_eq!(decoded_header.sequence_number, 42);
    assert_eq!(decoded_payload, payload);
}

#[test]
fn test_fail_closed_on_bad_magic() {
    let header = FrameHeader {
        channel_tag: ChannelTag::Command,
        flags: FrameFlags::empty(),
        sequence_number: 1,
    };
    let mut encoded = encode_frame(&header, b"test").unwrap();
    encoded[0] = b'X'; // Corrupt magic

    match decode_frame(&encoded) {
        Err(FrameParseError::InvalidMagic([b'X', b'T'])) => {}
        other => panic!("Expected InvalidMagic, got {:?}", other),
    }
}

#[test]
fn test_fail_closed_on_bad_checksum() {
    let header = FrameHeader {
        channel_tag: ChannelTag::Command,
        flags: FrameFlags::empty(),
        sequence_number: 1,
    };
    let mut encoded = encode_frame(&header, b"test").unwrap();
    let last = encoded.len() - 1;
    encoded[last] ^= 0xFF; // Corrupt CRC

    match decode_frame(&encoded) {
        Err(FrameParseError::ChecksumMismatch { .. }) => {}
        other => panic!("Expected ChecksumMismatch, got {:?}", other),
    }
}

#[test]
fn test_fail_closed_on_oversized_payload() {
    let mut buf = Vec::new();
    buf.extend_from_slice(&[0x50, 0x54]); // Magic
    buf.extend_from_slice(&1u16.to_le_bytes()); // Version
    buf.push(1); // ChannelTag::Command
    buf.push(0); // Flags
    buf.extend_from_slice(&0u16.to_le_bytes()); // Reserved
    buf.extend_from_slice(&1u64.to_le_bytes()); // Seq
    buf.extend_from_slice(&(MAX_PAYLOAD_SIZE as u32 + 1).to_le_bytes()); // 4 MiB + 1

    match decode_frame(&buf) {
        Err(FrameParseError::PayloadTooLarge(len)) => {
            assert_eq!(len, MAX_PAYLOAD_SIZE + 1);
        }
        other => panic!("Expected PayloadTooLarge, got {:?}", other),
    }
}

#[test]
fn test_read_frame_clean_eof() {
    let empty_buf: &[u8] = &[];
    let mut cursor = std::io::Cursor::new(empty_buf);
    let res = read_frame(&mut cursor).expect("clean EOF should return Ok(None)");
    assert!(res.is_none());
}

#[test]
fn test_read_frame_partial_header_premature_eof() {
    let partial_buf = &[0x50, 0x54, 0x01, 0x00]; // Only 4 bytes of 20-byte header
    let mut cursor = std::io::Cursor::new(&partial_buf[..]);
    match read_frame(&mut cursor) {
        Err(FrameParseError::PrematureEof) => {}
        other => panic!("Expected PrematureEof, got {:?}", other),
    }
}

#[test]
fn test_read_frame_partial_payload_premature_eof() {
    let header = FrameHeader {
        channel_tag: ChannelTag::Command,
        flags: FrameFlags::empty(),
        sequence_number: 1,
    };
    let payload = b"1234567890";
    let encoded = encode_frame(&header, payload).unwrap();

    // Truncate in the middle of the payload
    let truncated = &encoded[..HEADER_SIZE + 5];
    let mut cursor = std::io::Cursor::new(truncated);
    match read_frame(&mut cursor) {
        Err(FrameParseError::PrematureEof) => {}
        other => panic!("Expected PrematureEof, got {:?}", other),
    }
}

#[test]
fn test_read_frame_partial_crc_premature_eof() {
    let header = FrameHeader {
        channel_tag: ChannelTag::Command,
        flags: FrameFlags::empty(),
        sequence_number: 1,
    };
    let payload = b"test";
    let encoded = encode_frame(&header, payload).unwrap();

    // Truncate in the middle of the 4-byte CRC
    let truncated = &encoded[..encoded.len() - 2];
    let mut cursor = std::io::Cursor::new(truncated);
    match read_frame(&mut cursor) {
        Err(FrameParseError::PrematureEof) => {}
        other => panic!("Expected PrematureEof, got {:?}", other),
    }
}

#[test]
fn test_read_frame_success_and_subsequent_clean_eof() {
    let header = FrameHeader {
        channel_tag: ChannelTag::LosslessDomain,
        flags: FrameFlags::END_OF_STREAM,
        sequence_number: 99,
    };
    let payload = b"streaming frame data";
    let mut stream_data = Vec::new();
    write_frame(&mut stream_data, &header, payload).expect("write frame");

    let mut cursor = std::io::Cursor::new(stream_data);

    // 1st read: full frame
    let frame_opt = read_frame(&mut cursor).expect("read frame should succeed");
    assert!(frame_opt.is_some());
    let frame = frame_opt.unwrap();
    assert_eq!(frame.header.channel_tag, ChannelTag::LosslessDomain);
    assert_eq!(frame.header.flags, FrameFlags::END_OF_STREAM);
    assert_eq!(frame.header.sequence_number, 99);
    assert_eq!(frame.payload, payload);

    // 2nd read: clean EOF
    let eof = read_frame(&mut cursor).expect("subsequent read should be Ok(None)");
    assert!(eof.is_none());
}
