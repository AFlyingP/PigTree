//! Structured JSON output formatting for CLI stdout envelopes and stderr diagnostics.

use crate::protobuf::{
    CoverageGapReport, DirectoryEntryNode, EchoResponse, HardLinkObjectReport, HealthResponse,
    LinkCountKnowledgeProto, LinkCountKnowledgeStatus, PingResponse, ScanProgress, ScanResponse,
    StatusResponse, VersionResponse,
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

/// Converts a ScanRunOutcome protobuf enum integer to its lowercase RFC 8259 string.
pub fn scan_run_outcome_to_str(outcome: i32) -> &'static str {
    match outcome {
        1 => "finished",
        2 => "cancelled",
        3 => "failed",
        _ => "failed",
    }
}

/// Converts a ScopeCoverage protobuf enum integer to its lowercase RFC 8259 string.
pub fn scope_coverage_to_str(coverage: i32) -> &'static str {
    match coverage {
        1 => "complete",
        2 => "partial",
        3 => "indeterminate",
        _ => "indeterminate",
    }
}

fn format_coverage_gaps_array(gaps: &[CoverageGapReport]) -> String {
    let mut out = String::from("[");
    for (i, gap) in gaps.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push_str(&format!(
            r#"{{"path":{},"status":{},"native_error":{},"message":{}}}"#,
            escape_json_string(&gap.display_path),
            escape_json_string(&gap.status_code),
            gap.native_error,
            escape_json_string(&gap.error_message)
        ));
    }
    out.push(']');
    out
}

/// Formats a ScanResponse as a standalone terminal JSON document for CLI JSON mode.
pub fn format_scan_terminal_json(resp: &ScanResponse) -> String {
    let outcome_str = scan_run_outcome_to_str(resp.run_outcome);
    let coverage_str = scope_coverage_to_str(resp.scope_coverage);
    let directory_entries = resp.directory_count + resp.file_count + resp.special_count;
    let knowledge_str = allocation_knowledge_str(resp.allocated_bytes_known);
    let gaps_json = format_coverage_gaps_array(&resp.coverage_gaps);
    let hard_links_json = format_hard_links_array(&resp.hard_links);

    format!(
        r#"{{"operation_id":{},"schema_version":"2.0","run_outcome":{},"observation_interval":{{"started_at":{},"completed_at":{}}},"scope_coverage":{},"directory_entries":{},"directories":{},"files":{},"special_objects":{},"logical_bytes":{},"referenced_allocated_bytes":{{"value":{},"knowledge":{}}},"unique_allocated_bytes":{{"value":{},"knowledge":{}}},"known_subtotal_allocated_bytes":{},"indeterminate_external_reference_objects":{},"hard_links":{},"coverage_gaps":{}}}"#,
        escape_json_string(&resp.operation_id),
        escape_json_string(outcome_str),
        escape_json_string(&resp.observation_started_iso),
        escape_json_string(&resp.observation_completed_iso),
        escape_json_string(coverage_str),
        directory_entries,
        resp.directory_count,
        resp.file_count,
        resp.special_count,
        resp.logical_bytes,
        resp.referenced_allocated_bytes,
        escape_json_string(knowledge_str),
        resp.unique_allocated_bytes,
        escape_json_string(knowledge_str),
        resp.known_subtotal_allocated_bytes,
        resp.indeterminate_external_reference_objects,
        hard_links_json,
        gaps_json
    )
}

/// Returns the canonical Value Knowledge string for an allocation aggregate flag.
fn allocation_knowledge_str(known: bool) -> &'static str {
    if known {
        "known"
    } else {
        "not_observed"
    }
}

/// Formats an in-flight ScanProgress update as a single-line NDJSON event envelope.
pub fn format_scan_progress_ndjson_event(p: &ScanProgress) -> String {
    format!(
        r#"{{"operation_id":{},"sequence_number":{},"timestamp":{},"schema_version":"2.0","phase":{},"channel":"progress","provenance":"win32_directory_traversal","payload":{{"observed_directories":{},"observed_files":{},"observed_logical_bytes":{},"observed_referenced_allocated_bytes":{},"coverage_gaps":{},"current_directory":{}}}}}"#,
        escape_json_string(&p.operation_id),
        p.sequence_number,
        escape_json_string(&p.timestamp_iso),
        escape_json_string(&p.current_phase),
        p.observed_directories,
        p.observed_files,
        p.observed_logical_bytes,
        p.observed_referenced_allocated_bytes,
        p.coverage_gaps,
        escape_json_string(&p.current_directory)
    )
}

/// Formats a final ScanResponse as a non-coalescible terminal NDJSON event envelope line.
pub fn format_scan_terminal_ndjson_event(resp: &ScanResponse, sequence_number: u64) -> String {
    let outcome_str = scan_run_outcome_to_str(resp.run_outcome);
    let coverage_str = scope_coverage_to_str(resp.scope_coverage);
    let directory_entries = resp.directory_count + resp.file_count + resp.special_count;
    let knowledge_str = allocation_knowledge_str(resp.allocated_bytes_known);
    let gaps_json = format_coverage_gaps_array(&resp.coverage_gaps);
    let hard_links_json = format_hard_links_array(&resp.hard_links);

    format!(
        r#"{{"operation_id":{},"sequence_number":{},"timestamp":{},"schema_version":"2.0","phase":"finalizing","channel":"data","provenance":"win32_directory_traversal","payload":{{"run_outcome":{},"observation_interval":{{"started_at":{},"completed_at":{}}},"scope_coverage":{},"directory_entries":{},"directories":{},"files":{},"special_objects":{},"logical_bytes":{},"referenced_allocated_bytes":{{"value":{},"knowledge":{}}},"unique_allocated_bytes":{{"value":{},"knowledge":{}}},"known_subtotal_allocated_bytes":{},"indeterminate_external_reference_objects":{},"hard_links":{},"coverage_gaps":{}}}}}"#,
        escape_json_string(&resp.operation_id),
        sequence_number,
        escape_json_string(&resp.observation_completed_iso),
        escape_json_string(outcome_str),
        escape_json_string(&resp.observation_started_iso),
        escape_json_string(&resp.observation_completed_iso),
        escape_json_string(coverage_str),
        directory_entries,
        resp.directory_count,
        resp.file_count,
        resp.special_count,
        resp.logical_bytes,
        resp.referenced_allocated_bytes,
        escape_json_string(knowledge_str),
        resp.unique_allocated_bytes,
        escape_json_string(knowledge_str),
        resp.known_subtotal_allocated_bytes,
        resp.indeterminate_external_reference_objects,
        hard_links_json,
        gaps_json
    )
}

/// Formats the hard-link object reports of a scan as a JSON array. The 16-byte
/// volume GUID is hex-encoded and the 128-bit File ID is decimal; both as
/// strings so no consumer can lose precision on either half.
fn format_hard_links_array(reports: &[HardLinkObjectReport]) -> String {
    let mut out = String::from("[");
    for (i, r) in reports.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        let guid_hex: String =
            r.volume_guid.iter().map(|b| format!("{b:02x}")).collect();
        let link_json = format_link_count_knowledge(r.total_link_count.as_ref());
        let status_str = external_reference_status_to_str(r.external_reference_status);
        let paths: Vec<String> = r
            .entry_paths
            .iter()
            .map(|p| escape_json_string(p))
            .collect();
        out.push_str(&format!(
            r#"{{"volume_guid":{},"file_id":{},"observed_alias_count":{},"total_link_count":{},"external_reference_status":{},"entry_paths":[{}]}}"#,
            escape_json_string(&guid_hex),
            escape_json_string(&format!("{}", u128::from(r.file_id_hi) << 64 | u128::from(r.file_id_lo))),
            r.observed_alias_count,
            link_json,
            escape_json_string(status_str),
            paths.join(",")
        ));
    }
    out.push(']');
    out
}

/// Converts an entry_kind integer (1=directory, 2=file, 3=special) to its lowercase string.
pub fn entry_kind_to_str(kind: u32) -> &'static str {
    match kind {
        1 => "directory",
        2 => "file",
        3 => "special",
        _ => "unknown",
    }
}

/// Converts a LinkCountKnowledgeStatus protobuf enum integer to its canonical lowercase string.
pub fn link_count_knowledge_to_str(status: i32) -> &'static str {
    match status {
        1 => "known",
        2 => "not_observed",
        3 => "unavailable",
        4 => "not_applicable",
        _ => "not_observed",
    }
}

/// Converts an ExternalReferenceStatusProto protobuf enum integer to its canonical lowercase string.
pub fn external_reference_status_to_str(status: i32) -> &'static str {
    match status {
        1 => "confirmed_none",
        2 => "confirmed_external",
        3 => "indeterminate",
        4 => "inconsistent_evidence",
        5 => "not_applicable",
        _ => "indeterminate",
    }
}

/// Formats a LinkCountKnowledgeProto into a typed JSON object representing four-state Value Knowledge.
fn format_link_count_knowledge(proto: Option<&LinkCountKnowledgeProto>) -> String {
    match proto {
        Some(p) if p.status == LinkCountKnowledgeStatus::Known as i32 => {
            format!(r#"{{"value":{},"knowledge":"known"}}"#, p.count)
        }
        Some(p) => {
            let status_str = link_count_knowledge_to_str(p.status);
            format!(
                r#"{{"value":null,"knowledge":{}}}"#,
                escape_json_string(status_str)
            )
        }
        None => r#"{"value":null,"knowledge":"not_observed"}"#.to_string(),
    }
}

/// Formats a DirectoryEntryNode into a canonical RFC 8259 JSON object at the protocol automation seam.
pub fn format_directory_entry_json(node: &DirectoryEntryNode) -> String {
    let entry_kind_str = entry_kind_to_str(node.entry_kind);
    let ext_status_str = external_reference_status_to_str(node.external_reference_status);
    let link_json = format_link_count_knowledge(node.total_link_count.as_ref());
    let streams_json = format_content_streams(node);

    format!(
        r#"{{"schema_version":"2.0","id":{},"parent_id":{},"name":{},"entry_kind":{},"logical_bytes":{},"referenced_allocated_bytes":{},"unique_allocated_bytes":{},"allocated_size_known":{},"known_subtotal_allocated_bytes":{},"child_count":{},"has_children":{},"observed_alias_count":{},"total_link_count":{},"external_reference_status":{}{}}}"#,
        node.id,
        node.parent_id,
        escape_json_string(&node.name),
        escape_json_string(entry_kind_str),
        node.logical_bytes,
        node.referenced_allocated_bytes,
        node.unique_allocated_bytes,
        node.allocated_size_known,
        node.known_subtotal_allocated_bytes,
        node.child_count,
        node.has_children,
        node.observed_alias_count,
        link_json,
        escape_json_string(ext_status_str),
        streams_json
    )
}

/// Renders the optional content stream breakdown of an entry's owning object.
/// The field is omitted entirely unless a profile or enrichment observed streams.
fn format_content_streams(node: &DirectoryEntryNode) -> String {
    if node.content_streams.is_empty() {
        return String::new();
    }
    let parts: Vec<String> = node
        .content_streams
        .iter()
        .map(|s| {
            format!(
                r#"{{"name":{},"logical_bytes":{},"allocated_bytes":{},"allocated_size_known":{}}}"#,
                escape_json_string(&s.name),
                s.logical_bytes,
                s.allocated_bytes,
                s.allocated_size_known
            )
        })
        .collect();
    format!(r#","content_streams":[{}]"#, parts.join(","))
}

/// Formats a DirectoryEntryNode as a single-line NDJSON record.
pub fn format_directory_entry_ndjson(node: &DirectoryEntryNode) -> String {
    format_directory_entry_json(node)
}

/// Formats a DirectoryEntryNode as an enveloped NDJSON data event under the ADR 0003 specification.
pub fn format_directory_entry_ndjson_event(
    operation_id: &str,
    sequence_number: u64,
    timestamp_iso: &str,
    node: &DirectoryEntryNode,
) -> String {
    let entry_json = format_directory_entry_json(node);
    format!(
        r#"{{"operation_id":{},"sequence_number":{},"timestamp":{},"schema_version":"2.0","phase":"finalizing","channel":"data","provenance":"win32_directory_traversal","payload":{}}}"#,
        escape_json_string(operation_id),
        sequence_number,
        escape_json_string(timestamp_iso),
        entry_json
    )
}

/// Formats a SystemTime into a strict UTC ISO-8601 string (e.g. 2026-08-29T12:34:56.789Z).
pub fn format_utc_iso(time: std::time::SystemTime) -> String {
    let dur = time
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let total_secs = dur.as_secs();
    let millis = dur.subsec_millis();

    let days = total_secs / 86400;
    let seconds_of_day = total_secs % 86400;
    let hours = seconds_of_day / 3600;
    let minutes = (seconds_of_day % 3600) / 60;
    let seconds = seconds_of_day % 60;

    let z = days as i64 + 719468;
    let era = (if z >= 0 { z } else { z - 146096 }) / 146097;
    let doe = (z - era * 146097) as u32;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };

    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:03}Z",
        y, m, d, hours, minutes, seconds, millis
    )
}

/// Formats a minimal normative cancelled scan terminal NDJSON event when a full ScanResponse is unavailable.
pub fn format_scan_cancelled_ndjson_event(
    operation_id: &str,
    sequence_number: u64,
    timestamp_iso: Option<&str>,
    message: &str,
) -> String {
    let ts = match timestamp_iso {
        Some(t) if !t.is_empty() => t.to_string(),
        _ => format_utc_iso(std::time::SystemTime::now()),
    };
    format!(
        r#"{{"operation_id":{},"sequence_number":{},"timestamp":{},"schema_version":"2.0","phase":"finalizing","channel":"data","provenance":"win32_directory_traversal","payload":{{"run_outcome":"cancelled","status":"cancelled","message":{}}}}}"#,
        escape_json_string(operation_id),
        sequence_number,
        escape_json_string(&ts),
        escape_json_string(message)
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

    #[test]
    fn test_scan_terminal_and_ndjson_formatters() {
        let resp = ScanResponse {
            operation_id: "scan-42".to_string(),
            target_path: r#"C:	est	arget"#.to_string(),
            run_outcome: 1, // finished
            observation_started_iso: "2026-08-29T10:00:00.000Z".to_string(),
            observation_completed_iso: "2026-08-29T10:00:01.500Z".to_string(),
            scope_coverage: 1, // complete
            directory_count: 5,
            file_count: 10,
            special_count: 1,
            logical_bytes: 1024,
            allocated_bytes: 2048,
            allocated_bytes_known: true,
            coverage_gaps: vec![CoverageGapReport {
                display_path: r#"C:\test\inaccessible"#.to_string(),
                status_code: "ERROR_ACCESS_DENIED".to_string(),
                native_error: 5,
                error_message: "Access is denied.".to_string(),
            }],
            duration_ms: 1500,
            referenced_allocated_bytes: 2048,
            unique_allocated_bytes: 1536,
            known_subtotal_allocated_bytes: 2048,
            indeterminate_external_reference_objects: 2,
            hard_links: Vec::new(),
        };

        let json_doc = format_scan_terminal_json(&resp);
        assert!(json_doc.contains(r#""operation_id":"scan-42""#));
        assert!(json_doc.contains(r#""schema_version":"2.0""#));
        assert!(json_doc.contains(r#""run_outcome":"finished""#));
        assert!(json_doc.contains(r#""scope_coverage":"complete""#));
        assert!(json_doc.contains(r#""directory_entries":16"#));
        assert!(json_doc.contains(r#""directories":5"#));
        assert!(json_doc.contains(r#""files":10"#));
        assert!(json_doc.contains(r#""special_objects":1"#));
        assert!(json_doc.contains(r#""logical_bytes":1024"#));
        assert!(
            json_doc.contains(r#""referenced_allocated_bytes":{"value":2048,"knowledge":"known"}"#)
        );
        assert!(json_doc.contains(r#""unique_allocated_bytes":{"value":1536,"knowledge":"known"}"#));
        assert!(json_doc.contains(r#""known_subtotal_allocated_bytes":2048"#));
        assert!(json_doc.contains(r#""indeterminate_external_reference_objects":2"#));
        assert!(json_doc.contains(r#""observation_interval":{"started_at":"2026-08-29T10:00:00.000Z","completed_at":"2026-08-29T10:00:01.500Z"}"#));
        assert!(json_doc.contains(r##""path":"C:\\test\\inaccessible""##));
        assert!(json_doc.contains(r#""status":"ERROR_ACCESS_DENIED""#));
        assert!(json_doc.contains(r#""native_error":5"#));
        assert!(json_doc.contains(r#""message":"Access is denied.""#));

        let progress = ScanProgress {
            operation_id: "scan-42".to_string(),
            sequence_number: 1,
            timestamp_iso: "2026-08-29T10:00:00.500Z".to_string(),
            observed_directories: 2,
            observed_files: 4,
            observed_logical_bytes: 512,
            observed_referenced_allocated_bytes: 1024,
            coverage_gaps: 0,
            current_phase: "traversing".to_string(),
            current_directory: r#"C:	est	arget"#.to_string(),
        };
        let ndjson_prog = format_scan_progress_ndjson_event(&progress);
        assert!(ndjson_prog.contains(r#""channel":"progress""#));
        assert!(ndjson_prog.contains(r#""sequence_number":1"#));
        assert!(ndjson_prog.contains(r#""phase":"traversing""#));
        assert!(ndjson_prog.contains(r#""provenance":"win32_directory_traversal""#));
        assert!(ndjson_prog.contains(r#""observed_referenced_allocated_bytes":1024"#));
        assert!(ndjson_prog.contains(r#""current_directory":"C:\test\target""#));

        let ndjson_term = format_scan_terminal_ndjson_event(&resp, 2);
        assert!(ndjson_term.contains(r#""channel":"data""#));
        assert!(ndjson_term.contains(r#""sequence_number":2"#));
        assert!(ndjson_term.contains(r#""phase":"finalizing""#));
        assert!(ndjson_term.contains(r#""provenance":"win32_directory_traversal""#));
        assert!(ndjson_term.contains(r#""directory_entries":16"#));
        assert!(ndjson_term
            .contains(r#""referenced_allocated_bytes":{"value":2048,"knowledge":"known"}"#));
        assert!(
            ndjson_term.contains(r#""unique_allocated_bytes":{"value":1536,"knowledge":"known"}"#)
        );
        assert!(ndjson_term.contains(r#""indeterminate_external_reference_objects":2"#));
    }

    #[test]
    fn test_format_utc_iso() {
        let epoch = std::time::UNIX_EPOCH;
        assert_eq!(format_utc_iso(epoch), "1970-01-01T00:00:00.000Z");
        let later = epoch + std::time::Duration::from_millis(1700000000123);
        let iso = format_utc_iso(later);
        assert!(iso.ends_with(".123Z"));
    }

    #[test]
    fn test_format_scan_cancelled_ndjson_event() {
        let event = format_scan_cancelled_ndjson_event(
            "scan-1",
            3,
            Some("2026-08-29T10:00:00.000Z"),
            "Scan operation cancelled by user",
        );
        assert_eq!(
            event,
            r#"{"operation_id":"scan-1","sequence_number":3,"timestamp":"2026-08-29T10:00:00.000Z","schema_version":"2.0","phase":"finalizing","channel":"data","provenance":"win32_directory_traversal","payload":{"run_outcome":"cancelled","status":"cancelled","message":"Scan operation cancelled by user"}}"#
        );
    }

    #[test]
    fn test_scan_terminal_allocated_bytes_knowledge_canonical_strings() {
        // 1. Known with non-zero bytes
        let resp_known = ScanResponse {
            operation_id: "scan-known".to_string(),
            target_path: r#"C:	arget"#.to_string(),
            run_outcome: 1,
            observation_started_iso: "2026-08-29T10:00:00.000Z".to_string(),
            observation_completed_iso: "2026-08-29T10:00:01.000Z".to_string(),
            scope_coverage: 1,
            directory_count: 1,
            file_count: 2,
            special_count: 0,
            logical_bytes: 500,
            allocated_bytes: 4096,
            allocated_bytes_known: true,
            coverage_gaps: vec![],
            duration_ms: 1000,
            referenced_allocated_bytes: 4096,
            unique_allocated_bytes: 4096,
            known_subtotal_allocated_bytes: 4096,
            indeterminate_external_reference_objects: 0,
            hard_links: Vec::new(),
        };
        let json_known = format_scan_terminal_json(&resp_known);
        assert!(json_known
            .contains(r#""referenced_allocated_bytes":{"value":4096,"knowledge":"known"}"#));
        assert!(
            json_known.contains(r#""unique_allocated_bytes":{"value":4096,"knowledge":"known"}"#)
        );
        assert!(!json_known.contains("unknown"));

        let ndjson_known = format_scan_terminal_ndjson_event(&resp_known, 1);
        assert!(
            ndjson_known.contains(r#""unique_allocated_bytes":{"value":4096,"knowledge":"known"}"#)
        );
        assert!(!ndjson_known.contains("unknown"));

        // 2. Known with zero bytes (e.g. empty file set or known zero-allocated files)
        let resp_known_zero = ScanResponse {
            operation_id: "scan-zero".to_string(),
            target_path: r#"C:	arget"#.to_string(),
            run_outcome: 1,
            observation_started_iso: "2026-08-29T10:00:00.000Z".to_string(),
            observation_completed_iso: "2026-08-29T10:00:01.000Z".to_string(),
            scope_coverage: 1,
            directory_count: 1,
            file_count: 1,
            special_count: 0,
            logical_bytes: 100,
            allocated_bytes: 0,
            allocated_bytes_known: true,
            coverage_gaps: vec![],
            duration_ms: 1000,
            referenced_allocated_bytes: 0,
            unique_allocated_bytes: 0,
            known_subtotal_allocated_bytes: 0,
            indeterminate_external_reference_objects: 0,
            hard_links: Vec::new(),
        };
        let json_zero = format_scan_terminal_json(&resp_known_zero);
        assert!(json_zero.contains(r#""unique_allocated_bytes":{"value":0,"knowledge":"known"}"#));
        assert!(!json_zero.contains("unknown"));

        // 3. Not observed (when any file has unavailable allocated size)
        let resp_not_observed = ScanResponse {
            operation_id: "scan-not-obs".to_string(),
            target_path: r#"C:	arget"#.to_string(),
            run_outcome: 1,
            observation_started_iso: "2026-08-29T10:00:00.000Z".to_string(),
            observation_completed_iso: "2026-08-29T10:00:01.000Z".to_string(),
            scope_coverage: 1,
            directory_count: 1,
            file_count: 2,
            special_count: 0,
            logical_bytes: 500,
            allocated_bytes: 2048, // Sum of known Some observations
            allocated_bytes_known: false,
            coverage_gaps: vec![],
            duration_ms: 1000,
            referenced_allocated_bytes: 2048,
            unique_allocated_bytes: 2048,
            known_subtotal_allocated_bytes: 2048,
            indeterminate_external_reference_objects: 0,
            hard_links: Vec::new(),
        };
        let json_not_obs = format_scan_terminal_json(&resp_not_observed);
        assert!(json_not_obs
            .contains(r#""referenced_allocated_bytes":{"value":2048,"knowledge":"not_observed"}"#));
        assert!(json_not_obs
            .contains(r#""unique_allocated_bytes":{"value":2048,"knowledge":"not_observed"}"#));
        assert!(!json_not_obs.contains("unknown"));

        let ndjson_not_obs = format_scan_terminal_ndjson_event(&resp_not_observed, 5);
        assert!(ndjson_not_obs
            .contains(r#""unique_allocated_bytes":{"value":2048,"knowledge":"not_observed"}"#));
        assert!(!ndjson_not_obs.contains("unknown"));
    }

    #[test]
    fn test_format_directory_entry_node_canonical_fields_and_knowledge_variants() {
        use crate::protobuf::{
            DirectoryEntryNode, ExternalReferenceStatusProto, LinkCountKnowledgeProto,
            LinkCountKnowledgeStatus,
        };

        // 1. Variant with Known link count and ConfirmedNone
        let node_known = DirectoryEntryNode {
            id: 10,
            parent_id: 1,
            name: "sample_file.dat".to_string(),
            entry_kind: 2, // File
            logical_bytes: 1048576,
            referenced_allocated_bytes: 2097152,
            allocated_size_known: true,
            child_count: 0,
            has_children: false,
            unique_allocated_bytes: 1048576,
            observed_alias_count: 2,
            total_link_count: Some(LinkCountKnowledgeProto {
                status: LinkCountKnowledgeStatus::Known as i32,
                count: 2,
            }),
            external_reference_status:
                ExternalReferenceStatusProto::ExternalReferenceStatusConfirmedNone as i32,
            known_subtotal_allocated_bytes: 2097152,
            content_streams: Vec::new(),
        };

        let json_known = format_directory_entry_json(&node_known);
        assert!(json_known.contains(r#""schema_version":"2.0""#));
        assert!(json_known.contains(r#""id":10"#));
        assert!(json_known.contains(r#""parent_id":1"#));
        assert!(json_known.contains(r#""name":"sample_file.dat""#));
        assert!(json_known.contains(r#""entry_kind":"file""#));
        assert!(json_known.contains(r#""logical_bytes":1048576"#));
        assert!(json_known.contains(r#""referenced_allocated_bytes":2097152"#));
        assert!(json_known.contains(r#""unique_allocated_bytes":1048576"#));
        assert!(json_known.contains(r#""allocated_size_known":true"#));
        assert!(json_known.contains(r#""known_subtotal_allocated_bytes":2097152"#));
        assert!(json_known.contains(r#""child_count":0"#));
        assert!(json_known.contains(r#""has_children":false"#));
        assert!(json_known.contains(r#""observed_alias_count":2"#));
        assert!(json_known.contains(r#""total_link_count":{"value":2,"knowledge":"known"}"#));
        assert!(!json_known.contains(r#""count":2"#));
        assert!(!json_known.contains(r#""status":"known""#));
        assert!(json_known.contains(r#""external_reference_status":"confirmed_none""#));

        // 2. Variant with NotObserved link count and Indeterminate status
        let node_not_obs = DirectoryEntryNode {
            id: 11,
            parent_id: 1,
            name: "unobserved.bin".to_string(),
            entry_kind: 2,
            logical_bytes: 512,
            referenced_allocated_bytes: 4096,
            allocated_size_known: false,
            child_count: 0,
            has_children: false,
            unique_allocated_bytes: 4096,
            observed_alias_count: 1,
            total_link_count: Some(LinkCountKnowledgeProto {
                status: LinkCountKnowledgeStatus::NotObserved as i32,
                count: 0,
            }),
            external_reference_status:
                ExternalReferenceStatusProto::ExternalReferenceStatusIndeterminate as i32,
            known_subtotal_allocated_bytes: 0,
            content_streams: Vec::new(),
        };

        let json_not_obs = format_directory_entry_json(&node_not_obs);
        assert!(json_not_obs
            .contains(r#""total_link_count":{"value":null,"knowledge":"not_observed"}"#));
        assert!(!json_not_obs.contains(r#""status":"not_observed""#));
        assert!(json_not_obs.contains(r#""external_reference_status":"indeterminate""#));
        assert!(json_not_obs.contains(r#""allocated_size_known":false"#));

        // 3. Variant with Unavailable link count and InconsistentEvidence
        let node_unavail = DirectoryEntryNode {
            id: 12,
            parent_id: 1,
            name: "inconsistent.bin".to_string(),
            entry_kind: 2,
            logical_bytes: 100,
            referenced_allocated_bytes: 512,
            allocated_size_known: true,
            child_count: 0,
            has_children: false,
            unique_allocated_bytes: 512,
            observed_alias_count: 3,
            total_link_count: Some(LinkCountKnowledgeProto {
                status: LinkCountKnowledgeStatus::Unavailable as i32,
                count: 0,
            }),
            external_reference_status:
                ExternalReferenceStatusProto::ExternalReferenceStatusInconsistentEvidence as i32,
            known_subtotal_allocated_bytes: 512,
            content_streams: Vec::new(),
        };

        let json_unavail = format_directory_entry_json(&node_unavail);
        assert!(
            json_unavail.contains(r#""total_link_count":{"value":null,"knowledge":"unavailable"}"#)
        );
        assert!(!json_unavail.contains(r#""status":"unavailable""#));
        assert!(json_unavail.contains(r#""external_reference_status":"inconsistent_evidence""#));

        // 4. Variant with NotApplicable link count and NotApplicable status (directory entry)
        let node_not_app = DirectoryEntryNode {
            id: 1,
            parent_id: 0,
            name: "sub_dir".to_string(),
            entry_kind: 1, // Directory
            logical_bytes: 2048,
            referenced_allocated_bytes: 8192,
            allocated_size_known: true,
            child_count: 5,
            has_children: true,
            unique_allocated_bytes: 4096,
            observed_alias_count: 1,
            total_link_count: Some(LinkCountKnowledgeProto {
                status: LinkCountKnowledgeStatus::NotApplicable as i32,
                count: 0,
            }),
            external_reference_status:
                ExternalReferenceStatusProto::ExternalReferenceStatusNotApplicable as i32,
            known_subtotal_allocated_bytes: 8192,
            content_streams: Vec::new(),
        };

        let json_not_app = format_directory_entry_json(&node_not_app);
        assert!(json_not_app.contains(r#""entry_kind":"directory""#));
        assert!(json_not_app.contains(r#""child_count":5"#));
        assert!(json_not_app.contains(r#""has_children":true"#));
        assert!(json_not_app
            .contains(r#""total_link_count":{"value":null,"knowledge":"not_applicable"}"#));
        assert!(!json_not_app.contains(r#""status":"not_applicable""#));
        assert!(json_not_app.contains(r#""external_reference_status":"not_applicable""#));

        // 5. Variant with ConfirmedExternal and None (unspecified) link count
        let node_ext = DirectoryEntryNode {
            id: 14,
            parent_id: 1,
            name: "external_link.dat".to_string(),
            entry_kind: 2,
            logical_bytes: 5000,
            referenced_allocated_bytes: 8192,
            allocated_size_known: true,
            child_count: 0,
            has_children: false,
            unique_allocated_bytes: 8192,
            observed_alias_count: 1,
            total_link_count: None, // None -> maps to not_observed
            external_reference_status:
                ExternalReferenceStatusProto::ExternalReferenceStatusConfirmedExternal as i32,
            known_subtotal_allocated_bytes: 8192,
            content_streams: Vec::new(),
        };

        let json_ext = format_directory_entry_json(&node_ext);
        assert!(
            json_ext.contains(r#""total_link_count":{"value":null,"knowledge":"not_observed"}"#)
        );
        assert!(json_ext.contains(r#""external_reference_status":"confirmed_external""#));

        // 6. Proto unspecified external reference status fails-closed to indeterminate (never confirmed_none)
        let node_unspecified = DirectoryEntryNode {
            id: 15,
            parent_id: 1,
            name: "special_symlink".to_string(),
            entry_kind: 3, // Special
            logical_bytes: 0,
            referenced_allocated_bytes: 0,
            allocated_size_known: true,
            child_count: 0,
            has_children: false,
            unique_allocated_bytes: 0,
            observed_alias_count: 1,
            total_link_count: None,
            external_reference_status:
                ExternalReferenceStatusProto::ExternalReferenceStatusUnspecified as i32,
            known_subtotal_allocated_bytes: 0,
            content_streams: Vec::new(),
        };

        let json_unspecified = format_directory_entry_json(&node_unspecified);
        assert!(json_unspecified.contains(r#""entry_kind":"special""#));
        assert!(json_unspecified.contains(r#""external_reference_status":"indeterminate""#));
        assert!(!json_unspecified.contains(r#""external_reference_status":"confirmed_none""#));

        // 7. Unknown numeric enum integer fails-closed to indeterminate (never confirmed_none)
        let node_unknown = DirectoryEntryNode {
            id: 16,
            parent_id: 1,
            name: "unknown_status_file.dat".to_string(),
            entry_kind: 2,
            logical_bytes: 128,
            referenced_allocated_bytes: 512,
            allocated_size_known: true,
            child_count: 0,
            has_children: false,
            unique_allocated_bytes: 512,
            observed_alias_count: 1,
            total_link_count: None,
            external_reference_status: 99, // Unknown numeric value
            known_subtotal_allocated_bytes: 512,
            content_streams: Vec::new(),
        };

        let json_unknown = format_directory_entry_json(&node_unknown);
        assert!(json_unknown.contains(r#""external_reference_status":"indeterminate""#));
        assert!(!json_unknown.contains(r#""external_reference_status":"confirmed_none""#));
    }

    #[test]
    fn test_format_directory_entry_escaping_and_ndjson_event() {
        use crate::protobuf::{
            DirectoryEntryNode, LinkCountKnowledgeProto, LinkCountKnowledgeStatus,
        };

        let challenging_name = "funny \"quotes\" & \\backslashes\\ \n \r \t \x08 \x0c \x1f test";
        let node = DirectoryEntryNode {
            id: 99,
            parent_id: 10,
            name: challenging_name.to_string(),
            entry_kind: 2,
            logical_bytes: 42,
            referenced_allocated_bytes: 4096,
            allocated_size_known: true,
            child_count: 0,
            has_children: false,
            unique_allocated_bytes: 4096,
            observed_alias_count: 1,
            total_link_count: Some(LinkCountKnowledgeProto {
                status: LinkCountKnowledgeStatus::Known as i32,
                count: 1,
            }),
            external_reference_status: 1,
            known_subtotal_allocated_bytes: 4096,
            content_streams: Vec::new(),
        };

        // Standalone JSON
        let json_line = format_directory_entry_json(&node);
        // NDJSON single line must not have unescaped newlines or carriage returns
        assert!(!json_line.contains('\n'));
        assert!(!json_line.contains('\r'));
        assert!(json_line.contains(r#"\backslashes\"#));
        assert!(json_line.contains(r#"\n"#));
        assert!(json_line.contains(r#"\r"#));
        assert!(json_line.contains(r#"\t"#));
        assert!(json_line.contains(r#"\b"#));
        assert!(json_line.contains(r#"\f"#));
        assert!(json_line.contains(r#"\u001f"#));
        let ndjson_line = format_directory_entry_ndjson(&node);
        assert_eq!(json_line, ndjson_line);

        // NDJSON event envelope
        let event =
            format_directory_entry_ndjson_event("op-12345", 42, "2026-08-29T12:00:00.000Z", &node);
        assert!(!event.contains('\n'));
        assert!(!event.contains('\r'));
        assert!(event.contains(r#""operation_id":"op-12345""#));
        assert!(event.contains(r#""sequence_number":42"#));
        assert!(event.contains(r#""timestamp":"2026-08-29T12:00:00.000Z""#));
        assert!(event.contains(r#""schema_version":"2.0""#));
        assert!(event.contains(r#""phase":"finalizing""#));
        assert!(event.contains(r#""channel":"data""#));
        assert!(event.contains(r#""provenance":"win32_directory_traversal""#));
        assert!(event.contains(r#""payload":{"schema_version":"2.0","id":99,"parent_id":10"#));
    }

    #[test]
    fn test_format_directory_entry_content_streams_breakdown() {
        use crate::protobuf::{ContentStreamProto, DirectoryEntryNode};

        let mut node = DirectoryEntryNode {
            id: 5,
            parent_id: 1,
            name: "host.dat".to_string(),
            entry_kind: 2,
            logical_bytes: 100,
            referenced_allocated_bytes: 4096,
            allocated_size_known: true,
            child_count: 0,
            has_children: false,
            unique_allocated_bytes: 4096,
            observed_alias_count: 1,
            total_link_count: None,
            external_reference_status: 0,
            known_subtotal_allocated_bytes: 4096,
            content_streams: Vec::new(),
        };

        // Un-enriched scans omit the field entirely so output stays stable.
        let json_without = format_directory_entry_json(&node);
        assert!(!json_without.contains("content_streams"));

        node.content_streams.push(ContentStreamProto {
            name: "zone.identifier".to_string(),
            logical_bytes: 42,
            allocated_bytes: 512,
            allocated_size_known: true,
        });
        node.content_streams.push(ContentStreamProto {
            name: "cache".to_string(),
            logical_bytes: 4096,
            allocated_bytes: 0,
            allocated_size_known: false,
        });

        let json_with = format_directory_entry_json(&node);
        assert!(json_with.contains(
            r#""content_streams":[{"name":"zone.identifier","logical_bytes":42,"allocated_bytes":512,"allocated_size_known":true},{"name":"cache","logical_bytes":4096,"allocated_bytes":0,"allocated_size_known":false}]"#
        ));
        assert!(format_directory_entry_ndjson(&node) == json_with);
    }

    #[test]
    fn test_scan_terminal_json_hard_links_array() {
        use crate::protobuf::{
            ExternalReferenceStatusProto, HardLinkObjectReport, LinkCountKnowledgeProto,
            LinkCountKnowledgeStatus, ScanResponse,
        };

        let base = |hard_links: Vec<HardLinkObjectReport>| ScanResponse {
            operation_id: "scan-7".to_string(),
            target_path: r"C:\Root".to_string(),
            run_outcome: 1,
            observation_started_iso: "2026-08-29T10:00:00.000Z".to_string(),
            observation_completed_iso: "2026-08-29T10:00:01.000Z".to_string(),
            scope_coverage: 1,
            directory_count: 2,
            file_count: 2,
            special_count: 0,
            logical_bytes: 150,
            allocated_bytes: 4096,
            allocated_bytes_known: true,
            coverage_gaps: vec![],
            duration_ms: 1000,
            referenced_allocated_bytes: 6144,
            unique_allocated_bytes: 5120,
            known_subtotal_allocated_bytes: 6144,
            indeterminate_external_reference_objects: 1,
            hard_links,
        };

        let empty = format_scan_terminal_json(&base(Vec::new()));
        assert!(empty.contains(r#""hard_links":[]"#));

        let report = HardLinkObjectReport {
            volume_guid: vec![0xAB; 16],
            file_id_hi: 0,
            file_id_lo: 1001,
            observed_alias_count: 2,
            total_link_count: Some(LinkCountKnowledgeProto {
                status: LinkCountKnowledgeStatus::NotObserved as i32,
                count: 0,
            }),
            external_reference_status:
                ExternalReferenceStatusProto::ExternalReferenceStatusIndeterminate as i32,
            entry_paths: vec![r"C:\Root\a.dat".to_string(), r"C:\Root\Sub\b.dat".to_string()],
        };
        let populated = format_scan_terminal_json(&base(vec![report.clone()]));
        assert!(populated.contains(
            r#""hard_links":[{"volume_guid":"abababababababababababababababab","file_id":"1001","observed_alias_count":2,"total_link_count":{"value":null,"knowledge":"not_observed"},"external_reference_status":"indeterminate","entry_paths":["C:\\Root\\a.dat","C:\\Root\\Sub\\b.dat"]}]"#
        ));

        let event = format_scan_terminal_ndjson_event(&base(vec![report]), 9);
        assert!(event.contains(r#""hard_links":[{"volume_guid":"abab"#));
        assert!(event.contains(r#""entry_paths":["C:\\Root\\a.dat""#));
    }
}
