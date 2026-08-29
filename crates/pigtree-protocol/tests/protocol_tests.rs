use pigtree_protocol::protobuf::{
    command_request, command_response, decode_message, encode_message, AuthHandshakeRequest,
    AuthHandshakeResponse, CancelRequest, CancelResponse, CommandRequest, CommandResponse,
    EchoRequest, EchoResponse, ErrorResponse, HealthRequest, HealthResponse, PingRequest,
    PingResponse, ShutdownRequest, ShutdownResponse, StatusRequest, StatusResponse, VersionRequest,
    VersionResponse,
};
use pigtree_protocol::Message;

#[test]
fn test_auth_handshake_request_roundtrip() {
    let req = CommandRequest {
        request_id: "req-auth-1".to_string(),
        request: Some(command_request::Request::AuthHandshake(
            AuthHandshakeRequest {
                bootstrap_nonce: vec![1, 2, 3, 4, 5, 6, 7, 8],
                client_nonce: vec![9, 10, 11, 12, 13, 14, 15, 16],
                client_pid: 1234,
                client_session_id: 5678,
            },
        )),
    };

    let encoded = encode_message(&req);
    let decoded: CommandRequest = decode_message(&encoded).expect("decode auth handshake request");
    assert_eq!(decoded.request_id, "req-auth-1");
    match decoded.request {
        Some(command_request::Request::AuthHandshake(inner)) => {
            assert_eq!(inner.bootstrap_nonce, vec![1, 2, 3, 4, 5, 6, 7, 8]);
            assert_eq!(inner.client_nonce, vec![9, 10, 11, 12, 13, 14, 15, 16]);
            assert_eq!(inner.client_pid, 1234);
            assert_eq!(inner.client_session_id, 5678);
        }
        other => panic!("expected AuthHandshake variant, got {:?}", other),
    }
}

#[test]
fn test_auth_handshake_response_roundtrip() {
    let resp = CommandResponse {
        request_id: "resp-auth-1".to_string(),
        status: 0,
        error_code: String::new(),
        error_message: String::new(),
        response: Some(command_response::Response::AuthHandshake(
            AuthHandshakeResponse {
                status: 0,
                server_nonce: vec![0xaa, 0xbb, 0xcc, 0xdd],
                server_pid: 4321,
                channel_key_hash: vec![0x11, 0x22, 0x33, 0x44],
                error_message: String::new(),
            },
        )),
    };

    let encoded = encode_message(&resp);
    let decoded: CommandResponse =
        decode_message(&encoded).expect("decode auth handshake response");
    assert_eq!(decoded.request_id, "resp-auth-1");
    assert_eq!(decoded.status, 0);
    match decoded.response {
        Some(command_response::Response::AuthHandshake(inner)) => {
            assert_eq!(inner.status, 0);
            assert_eq!(inner.server_nonce, vec![0xaa, 0xbb, 0xcc, 0xdd]);
            assert_eq!(inner.server_pid, 4321);
            assert_eq!(inner.channel_key_hash, vec![0x11, 0x22, 0x33, 0x44]);
            assert_eq!(inner.error_message, "");
        }
        other => panic!("expected AuthHandshake response variant, got {:?}", other),
    }
}

#[test]
fn test_ping_request_response_roundtrip() {
    let req = CommandRequest {
        request_id: "ping-1".to_string(),
        request: Some(command_request::Request::Ping(PingRequest {
            timestamp_utc_ms: 1700000000123,
            delay_ms: 0,
        })),
    };
    let enc_req = encode_message(&req);
    let dec_req: CommandRequest = decode_message(&enc_req).expect("decode ping request");
    match dec_req.request {
        Some(command_request::Request::Ping(p)) => {
            assert_eq!(p.timestamp_utc_ms, 1700000000123);
        }
        other => panic!("expected Ping variant, got {:?}", other),
    }

    let resp = CommandResponse {
        request_id: "ping-1".to_string(),
        status: 0,
        error_code: String::new(),
        error_message: String::new(),
        response: Some(command_response::Response::Ping(PingResponse {
            timestamp_utc_ms: 1700000000123,
            echo_timestamp_utc_ms: 1700000000456,
        })),
    };
    let enc_resp = encode_message(&resp);
    let dec_resp: CommandResponse = decode_message(&enc_resp).expect("decode ping response");
    match dec_resp.response {
        Some(command_response::Response::Ping(p)) => {
            assert_eq!(p.timestamp_utc_ms, 1700000000123);
            assert_eq!(p.echo_timestamp_utc_ms, 1700000000456);
        }
        other => panic!("expected Ping response variant, got {:?}", other),
    }
}

#[test]
fn test_health_request_empty_and_populated_roundtrip() {
    let req_false = CommandRequest {
        request_id: "health-0".to_string(),
        request: Some(command_request::Request::Health(HealthRequest {
            include_memory: false,
            delay_ms: 0,
        })),
    };
    let enc_false = encode_message(&req_false);
    let dec_false: CommandRequest = decode_message(&enc_false).expect("decode health false");
    match dec_false.request {
        Some(command_request::Request::Health(h)) => {
            assert!(!h.include_memory);
        }
        other => panic!("expected Health variant, got {:?}", other),
    }

    let req_true = CommandRequest {
        request_id: "health-1".to_string(),
        request: Some(command_request::Request::Health(HealthRequest {
            include_memory: true,
            delay_ms: 0,
        })),
    };
    let enc_true = encode_message(&req_true);
    let dec_true: CommandRequest = decode_message(&enc_true).expect("decode health true");
    match dec_true.request {
        Some(command_request::Request::Health(h)) => {
            assert!(h.include_memory);
        }
        other => panic!("expected Health variant, got {:?}", other),
    }

    let resp = CommandResponse {
        request_id: "health-1".to_string(),
        status: 0,
        error_code: String::new(),
        error_message: String::new(),
        response: Some(command_response::Response::Health(HealthResponse {
            status: "HEALTHY".to_string(),
            uptime_ms: 123456,
            memory_private_bytes: 40960000,
            handle_count: 42,
        })),
    };
    let enc_resp = encode_message(&resp);
    let dec_resp: CommandResponse = decode_message(&enc_resp).expect("decode health response");
    match dec_resp.response {
        Some(command_response::Response::Health(h)) => {
            assert_eq!(h.status, "HEALTHY");
            assert_eq!(h.uptime_ms, 123456);
            assert_eq!(h.memory_private_bytes, 40960000);
            assert_eq!(h.handle_count, 42);
        }
        other => panic!("expected Health response variant, got {:?}", other),
    }
}

#[test]
fn test_empty_status_request_roundtrip_present() {
    let req = CommandRequest {
        request_id: "status-req".to_string(),
        request: Some(command_request::Request::Status(StatusRequest {
            delay_ms: 0,
        })),
    };
    let enc = encode_message(&req);
    let dec: CommandRequest = decode_message(&enc).expect("decode status request");
    assert_eq!(dec.request_id, "status-req");
    assert!(
        matches!(
            dec.request,
            Some(command_request::Request::Status(StatusRequest { .. }))
        ),
        "empty status request must decode as present oneof variant"
    );

    let resp = CommandResponse {
        request_id: "status-req".to_string(),
        status: 0,
        error_code: String::new(),
        error_message: String::new(),
        response: Some(command_response::Response::StatusPayload(StatusResponse {
            state: "IDLE".to_string(),
            active_runs: 2,
            total_observations: 1000,
            session_id: "sess-abc".to_string(),
        })),
    };
    let enc_resp = encode_message(&resp);
    let dec_resp: CommandResponse = decode_message(&enc_resp).expect("decode status response");
    match dec_resp.response {
        Some(command_response::Response::StatusPayload(s)) => {
            assert_eq!(s.state, "IDLE");
            assert_eq!(s.active_runs, 2);
            assert_eq!(s.total_observations, 1000);
            assert_eq!(s.session_id, "sess-abc");
        }
        other => panic!("expected StatusPayload variant, got {:?}", other),
    }
}

#[test]
fn test_empty_version_request_roundtrip_present() {
    let req = CommandRequest {
        request_id: "version-req".to_string(),
        request: Some(command_request::Request::Version(VersionRequest {
            delay_ms: 0,
        })),
    };
    let enc = encode_message(&req);
    let dec: CommandRequest = decode_message(&enc).expect("decode version request");
    assert_eq!(dec.request_id, "version-req");
    assert!(
        matches!(
            dec.request,
            Some(command_request::Request::Version(VersionRequest { .. }))
        ),
        "empty version request must decode as present oneof variant"
    );

    let resp = CommandResponse {
        request_id: "version-req".to_string(),
        status: 0,
        error_code: String::new(),
        error_message: String::new(),
        response: Some(command_response::Response::Version(VersionResponse {
            engine_version: "0.1.0".to_string(),
            protocol_version: 1,
            build_date: "2026-08-28".to_string(),
            commit_hash: "abcdef0".to_string(),
            capabilities: vec!["scan".to_string(), "query".to_string()],
        })),
    };
    let enc_resp = encode_message(&resp);
    let dec_resp: CommandResponse = decode_message(&enc_resp).expect("decode version response");
    match dec_resp.response {
        Some(command_response::Response::Version(v)) => {
            assert_eq!(v.engine_version, "0.1.0");
            assert_eq!(v.protocol_version, 1);
            assert_eq!(v.build_date, "2026-08-28");
            assert_eq!(v.commit_hash, "abcdef0");
            assert_eq!(v.capabilities, vec!["scan", "query"]);
        }
        other => panic!("expected Version variant, got {:?}", other),
    }
}

#[test]
fn test_empty_shutdown_request_roundtrip_present() {
    let req = CommandRequest {
        request_id: "shutdown-req".to_string(),
        request: Some(command_request::Request::Shutdown(ShutdownRequest {})),
    };
    let enc = encode_message(&req);
    let dec: CommandRequest = decode_message(&enc).expect("decode shutdown request");
    assert_eq!(dec.request_id, "shutdown-req");
    assert!(
        matches!(
            dec.request,
            Some(command_request::Request::Shutdown(ShutdownRequest {}))
        ),
        "empty shutdown request must decode as present oneof variant"
    );

    let resp = CommandResponse {
        request_id: "shutdown-req".to_string(),
        status: 0,
        error_code: String::new(),
        error_message: String::new(),
        response: Some(command_response::Response::Shutdown(ShutdownResponse {
            status: 0,
        })),
    };
    let enc_resp = encode_message(&resp);
    let dec_resp: CommandResponse = decode_message(&enc_resp).expect("decode shutdown response");
    match dec_resp.response {
        Some(command_response::Response::Shutdown(s)) => {
            assert_eq!(s.status, 0);
        }
        other => panic!("expected Shutdown variant, got {:?}", other),
    }
}

#[test]
fn test_cancel_request_response_roundtrip() {
    let req = CommandRequest {
        request_id: "cancel-req".to_string(),
        request: Some(command_request::Request::Cancel(CancelRequest {
            target_request_id: "scan-999".to_string(),
            reason: "User requested abort".to_string(),
        })),
    };
    let enc = encode_message(&req);
    let dec: CommandRequest = decode_message(&enc).expect("decode cancel request");
    match dec.request {
        Some(command_request::Request::Cancel(c)) => {
            assert_eq!(c.target_request_id, "scan-999");
            assert_eq!(c.reason, "User requested abort");
        }
        other => panic!("expected Cancel variant, got {:?}", other),
    }

    let resp = CommandResponse {
        request_id: "cancel-req".to_string(),
        status: 0,
        error_code: String::new(),
        error_message: String::new(),
        response: Some(command_response::Response::Cancel(CancelResponse {
            cancelled: true,
            message: "Scan successfully cancelled".to_string(),
        })),
    };
    let enc_resp = encode_message(&resp);
    let dec_resp: CommandResponse = decode_message(&enc_resp).expect("decode cancel response");
    match dec_resp.response {
        Some(command_response::Response::Cancel(c)) => {
            assert!(c.cancelled);
            assert_eq!(c.message, "Scan successfully cancelled");
        }
        other => panic!("expected Cancel response variant, got {:?}", other),
    }
}

#[test]
fn test_echo_special_text_roundtrips() {
    let special_payloads = [
        "",
        "Hello, World!",
        "PigTree 🐷🌳✨🚀",
        "中文测试 / 日本語テスト / 한국어 테스🐷",
        "\r\n\t\0\"'\\/<>#%^&*()[]{}|=+-_~`",
        "\u{202e}gnitset txet LTR\u{202c}",
        &"a".repeat(65536),
        &"Unicode 🦀 Rustacean ".repeat(1000),
    ];

    for (i, &payload) in special_payloads.iter().enumerate() {
        let req = CommandRequest {
            request_id: format!("echo-special-{i}"),
            request: Some(command_request::Request::Echo(EchoRequest {
                payload: payload.to_string(),
                delay_ms: 0,
            })),
        };
        let encoded = encode_message(&req);
        let decoded: CommandRequest =
            decode_message(&encoded).expect("decode special echo request");
        match decoded.request {
            Some(command_request::Request::Echo(echo)) => {
                assert_eq!(echo.payload, payload, "failed on payload index {i}");
            }
            other => panic!("expected Echo variant for index {i}, got {:?}", other),
        }

        let resp = CommandResponse {
            request_id: format!("echo-special-{i}"),
            status: 0,
            error_code: String::new(),
            error_message: String::new(),
            response: Some(command_response::Response::Echo(EchoResponse {
                payload: payload.to_string(),
            })),
        };
        let encoded_resp = encode_message(&resp);
        let decoded_resp: CommandResponse =
            decode_message(&encoded_resp).expect("decode special echo response");
        match decoded_resp.response {
            Some(command_response::Response::Echo(echo)) => {
                assert_eq!(echo.payload, payload, "failed on resp payload index {i}");
            }
            other => panic!(
                "expected Echo response variant for index {i}, got {:?}",
                other
            ),
        }
    }
}

#[test]
fn test_error_variant_roundtrip() {
    let error_inner = ErrorResponse {
        code: "ERR_PERMISSION_DENIED".to_string(),
        message: "Access to path 'C:\\System Volume Information' is denied".to_string(),
        details: "Win32 error code 5: ERROR_ACCESS_DENIED".to_string(),
    };

    // Standalone ErrorResponse roundtrip
    let enc_inner = encode_message(&error_inner);
    let dec_inner: ErrorResponse =
        decode_message(&enc_inner).expect("decode standalone ErrorResponse");
    assert_eq!(dec_inner.code, error_inner.code);
    assert_eq!(dec_inner.message, error_inner.message);
    assert_eq!(dec_inner.details, error_inner.details);

    // CommandResponse with ErrorResponse variant
    let resp = CommandResponse {
        request_id: "error-req-42".to_string(),
        status: 5,
        error_code: "ERR_PERMISSION_DENIED".to_string(),
        error_message: "Access to path 'C:\\System Volume Information' is denied".to_string(),
        response: Some(command_response::Response::Error(error_inner.clone())),
    };

    let encoded = encode_message(&resp);
    let decoded: CommandResponse = decode_message(&encoded).expect("decode error CommandResponse");
    assert_eq!(decoded.request_id, "error-req-42");
    assert_eq!(decoded.status, 5);
    assert_eq!(decoded.error_code, "ERR_PERMISSION_DENIED");
    assert_eq!(
        decoded.error_message,
        "Access to path 'C:\\System Volume Information' is denied"
    );
    match decoded.response {
        Some(command_response::Response::Error(err)) => {
            assert_eq!(err.code, error_inner.code);
            assert_eq!(err.message, error_inner.message);
            assert_eq!(err.details, error_inner.details);
        }
        other => panic!("expected Error variant in CommandResponse, got {:?}", other),
    }
}

#[test]
fn test_invalid_bytes_fail() {
    // 1. Completely bogus / truncated varint
    let invalid_varint = [
        0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x01,
    ];
    let res_req: Result<CommandRequest, _> = decode_message(&invalid_varint);
    assert!(res_req.is_err(), "decoding invalid varint should fail");

    let res_resp: Result<CommandResponse, _> = decode_message(&invalid_varint);
    assert!(
        res_resp.is_err(),
        "decoding invalid varint should fail for response"
    );

    // 2. Truncated length-delimited payload
    let truncated_string = [0x0A, 100, b'a', b'b'];
    let res_trunc: Result<CommandRequest, _> = decode_message(&truncated_string);
    assert!(res_trunc.is_err(), "decoding truncated string should fail");

    // 3. Truncated embedded message
    let truncated_embedded = [0x12, 50, 0x08];
    let res_emb: Result<CommandRequest, _> = decode_message(&truncated_embedded);
    assert!(
        res_emb.is_err(),
        "decoding truncated embedded message should fail"
    );

    // 4. Invalid UTF-8 in string field
    let invalid_utf8 = [0x0A, 0x02, 0xFF, 0xFF];
    let res_utf8: Result<CommandRequest, _> = decode_message(&invalid_utf8);
    assert!(
        res_utf8.is_err(),
        "decoding invalid UTF-8 string should fail"
    );

    // 5. prost::Message::decode directly on empty/short byte slice vs invalid struct
    let res_raw = CommandRequest::decode(&truncated_string[..]);
    assert!(
        res_raw.is_err(),
        "prost decode directly on truncated bytes should fail"
    );
}
