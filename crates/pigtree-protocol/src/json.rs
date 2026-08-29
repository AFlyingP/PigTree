//! Structured JSON output formatting for CLI stdout envelopes and stderr diagnostics.

use crate::protobuf::{
    EchoResponse, HealthResponse, PingResponse, StatusResponse, VersionResponse,
};

/// Escapes a string for JSON output according to RFC 8259.
pub fn escape_json_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str(r#"\""#),
            '\\' => out.push_str(r#"\\"#),
            '\n' => out.push_str(r#"\n"#),
            '\r' => out.push_str(r#"\r"#),
            '\t' => out.push_str(r#"\t"#),
            '\x08' => out.push_str(r#"\b"#),
            '\x0c' => out.push_str(r#"\f"#),
            c if (c as u32) < 0x20 => {
                let code = c as u32;
                use std::fmt::Write;
                let _ = write!(out, r#"\u{:04x}"#, code);
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Formats a diagnostic log entry for stderr as a single NDJSON line.
pub fn format_diagnostic(level: &str, target: &str, message: &str) -> String {
    format!(
        r#"{{"level":{},"target":{},"message":{}}}"#,
        escape_json_string(level),
        escape_json_string(target),
        escape_json_string(message)
    )
}

/// Formats a successful CLI stdout JSON envelope.
pub fn format_success_envelope(request_id: &str, data_json: &str) -> String {
    format!(
        r#"{{"version":"1.0","request_id":{},"status":"success","data":{}}}"#,
        escape_json_string(request_id),
        data_json
    )
}

/// Formats an error CLI stdout JSON envelope.
pub fn format_error_envelope(request_id: &str, error_code: &str, error_message: &str) -> String {
    format!(
        r#"{{"version":"1.0","request_id":{},"status":"error","error":{{"code":{},"message":{}}}}}"#,
        escape_json_string(request_id),
        escape_json_string(error_code),
        escape_json_string(error_message)
    )
}

/// Formats a cancelled CLI stdout JSON envelope.
pub fn format_cancelled_envelope(request_id: &str, reason: &str) -> String {
    format!(
        r#"{{"version":"1.0","request_id":{},"status":"cancelled","error":{{"code":"OPERATION_CANCELLED","message":{}}}}}"#,
        escape_json_string(request_id),
        escape_json_string(reason)
    )
}

/// Formats a PingResponse into a compact RFC 8259 JSON payload object.
pub fn format_ping_response(resp: &PingResponse) -> String {
    format!(
        r#"{{"timestamp_utc_ms":{},"echo_timestamp_utc_ms":{}}}"#,
        resp.timestamp_utc_ms, resp.echo_timestamp_utc_ms
    )
}

/// Formats an EchoResponse into a compact RFC 8259 JSON payload object.
pub fn format_echo_response(resp: &EchoResponse) -> String {
    format!(r#"{{"payload":{}}}"#, escape_json_string(&resp.payload))
}

/// Formats a HealthResponse into a compact RFC 8259 JSON payload object.
pub fn format_health_response(resp: &HealthResponse) -> String {
    format!(
        r#"{{"status":{},"uptime_ms":{},"memory_private_bytes":{},"handle_count":{}}}"#,
        escape_json_string(&resp.status),
        resp.uptime_ms,
        resp.memory_private_bytes,
        resp.handle_count
    )
}

/// Formats a StatusResponse into a compact RFC 8259 JSON payload object.
pub fn format_status_response(resp: &StatusResponse) -> String {
    format!(
        r#"{{"state":{},"active_runs":{},"total_observations":{},"session_id":{}}}"#,
        escape_json_string(&resp.state),
        resp.active_runs,
        resp.total_observations,
        escape_json_string(&resp.session_id)
    )
}

/// Formats a VersionResponse into a compact RFC 8259 JSON payload object.
pub fn format_version_response(resp: &VersionResponse) -> String {
    format!(
        r#"{{"engine_version":{},"protocol_version":{},"build_date":{},"commit_hash":{}}}"#,
        escape_json_string(&resp.engine_version),
        resp.protocol_version,
        escape_json_string(&resp.build_date),
        escape_json_string(&resp.commit_hash)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_escape_json_string() {
        assert_eq!(escape_json_string("hello"), r#""hello""#);
        assert_eq!(
            escape_json_string(r#"hello "world""#),
            r#""hello \"world\"""#
        );
    }

    #[test]
    fn test_escape_json_special_characters_regression() {
        // Quotes, backslashes, newlines, carriage returns, tabs, backspaces, form feeds
        let input =
            "quote: \" | backslash: \\ | newline: \n | cr: \r | tab: \t | bs: \x08 | ff: \x0c";
        let expected =
            r#""quote: \" | backslash: \\ | newline: \n | cr: \r | tab: \t | bs: \b | ff: \f""#;
        assert_eq!(escape_json_string(input), expected);

        // Control characters < 0x20
        let ctrl = "\x00\x01\x1f";
        assert_eq!(escape_json_string(ctrl), r#""\u0000\u0001\u001f""#);

        // Unicode preservation
        let unicode = "PigTree 🌲 🐷 - 目录 - äöü";
        assert_eq!(
            escape_json_string(unicode),
            r#""PigTree 🌲 🐷 - 目录 - äöü""#
        );

        // Empty string
        assert_eq!(escape_json_string(""), r#""""#);

        // Special characters inside diagnostic and envelopes
        let special_err = "Error on path C:\\Users\\test\nDetails: \"permission denied\"\t\x00";
        let diag = format_diagnostic("ERROR", "test_mod", special_err);
        assert!(diag.contains(r#"C:\\Users\\test\nDetails: \"permission denied\"\t\u0000"#));

        let err_env = format_error_envelope("req-123", "ERR_CODE", special_err);
        assert!(err_env.contains(r#"C:\\Users\\test\nDetails: \"permission denied\"\t\u0000"#));
    }

    #[test]
    fn test_diagnostic_formatting() {
        let diag = format_diagnostic("INFO", "pigtree_cli", "Starting engine session");
        assert_eq!(
            diag,
            r#"{"level":"INFO","target":"pigtree_cli","message":"Starting engine session"}"#
        );
    }

    #[test]
    fn test_envelopes() {
        let success = format_success_envelope("req-1", r#"{"uptime_ms":1234}"#);
        assert_eq!(
            success,
            r#"{"version":"1.0","request_id":"req-1","status":"success","data":{"uptime_ms":1234}}"#
        );

        let err = format_error_envelope("req-2", "COMMAND_ERROR", "Invalid argument");
        assert_eq!(
            err,
            r#"{"version":"1.0","request_id":"req-2","status":"error","error":{"code":"COMMAND_ERROR","message":"Invalid argument"}}"#
        );

        let cancelled = format_cancelled_envelope("req-3", "Cancelled by user");
        assert_eq!(
            cancelled,
            r#"{"version":"1.0","request_id":"req-3","status":"cancelled","error":{"code":"OPERATION_CANCELLED","message":"Cancelled by user"}}"#
        );
    }

    #[test]
    fn test_typed_response_formatters() {
        let ping = PingResponse {
            timestamp_utc_ms: 100,
            echo_timestamp_utc_ms: 200,
        };
        assert_eq!(
            format_ping_response(&ping),
            r#"{"timestamp_utc_ms":100,"echo_timestamp_utc_ms":200}"#
        );

        let echo = EchoResponse {
            payload: "hello \"world\"".to_string(),
        };
        assert_eq!(
            format_echo_response(&echo),
            r#"{"payload":"hello \"world\""}"#
        );

        let health = HealthResponse {
            status: "HEALTHY".to_string(),
            uptime_ms: 1234,
            memory_private_bytes: 5678,
            handle_count: 90,
        };
        assert_eq!(
            format_health_response(&health),
            r#"{"status":"HEALTHY","uptime_ms":1234,"memory_private_bytes":5678,"handle_count":90}"#
        );

        let status = StatusResponse {
            state: "IDLE".to_string(),
            active_runs: 1,
            total_observations: 2,
            session_id: "sess-1".to_string(),
        };
        assert_eq!(
            format_status_response(&status),
            r#"{"state":"IDLE","active_runs":1,"total_observations":2,"session_id":"sess-1"}"#
        );

        let ver = VersionResponse {
            engine_version: "0.1.0".to_string(),
            protocol_version: 1,
            build_date: "2026-08-28".to_string(),
            commit_hash: "abc1234".to_string(),
            capabilities: vec![],
        };
        assert_eq!(
            format_version_response(&ver),
            r#"{"engine_version":"0.1.0","protocol_version":1,"build_date":"2026-08-28","commit_hash":"abc1234"}"#
        );
    }
}
