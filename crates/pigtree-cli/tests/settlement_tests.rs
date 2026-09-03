use pigtree_cli::{
    settle_scan_outcome, settle_scan_response, OutputFormat, EXIT_CANCELLED,
    EXIT_COVERAGE_GAPS_PRESENT, EXIT_OPERATION_FAILED, EXIT_SUCCESS,
};
use pigtree_ipc::client::ScanCallOutcome;
use pigtree_protocol::protobuf::{
    CoverageGapReport, ExternalReferenceStatusProto, HardLinkObjectReport, ScanResponse,
    ScanRunOutcome, ScopeCoverage,
};

fn make_sample_response(
    run_outcome: ScanRunOutcome,
    scope_coverage: ScopeCoverage,
    allocated_bytes_known: bool,
    coverage_gaps: Vec<CoverageGapReport>,
) -> ScanResponse {
    ScanResponse {
        operation_id: "scan-test-123".to_string(),
        target_path: r#"C:\test\target"#.to_string(),
        run_outcome: run_outcome as i32,
        observation_started_iso: "2026-08-29T12:00:00.000Z".to_string(),
        observation_completed_iso: "2026-08-29T12:00:01.000Z".to_string(),
        scope_coverage: scope_coverage as i32,
        directory_count: 5,
        file_count: 10,
        special_count: 0,
        logical_bytes: 1024,
        allocated_bytes: if allocated_bytes_known { 2048 } else { 0 },
        allocated_bytes_known,
        coverage_gaps,
        duration_ms: 1000,
        referenced_allocated_bytes: if allocated_bytes_known { 2048 } else { 0 },
        unique_allocated_bytes: if allocated_bytes_known { 2048 } else { 0 },
        known_subtotal_allocated_bytes: if allocated_bytes_known { 2048 } else { 0 },
        indeterminate_external_reference_objects: 0,
        hard_links: Vec::new(),
    }
}

#[test]
fn test_finished_json_reports_hard_link_objects() {
    let mut resp = make_sample_response(
        ScanRunOutcome::Finished,
        ScopeCoverage::Complete,
        true,
        vec![],
    );
    resp.hard_links.push(HardLinkObjectReport {
        volume_guid: vec![0xAB; 16],
        file_id_hi: 0,
        file_id_lo: 77,
        observed_alias_count: 2,
        total_link_count: Some(pigtree_protocol::protobuf::LinkCountKnowledgeProto {
            status: pigtree_protocol::protobuf::LinkCountKnowledgeStatus::Known as i32,
            count: 2,
        }),
        external_reference_status:
            ExternalReferenceStatusProto::ExternalReferenceStatusConfirmedNone as i32,
        entry_paths: vec![r"C:\test\target\a.dat".to_string()],
    });

    let settlement = settle_scan_response(OutputFormat::Json, &resp, false, 1);
    assert_eq!(settlement.exit_code, EXIT_SUCCESS);
    assert!(settlement.stdout.contains(r#""hard_links":[{"volume_guid":"abababababababababababababababab","file_id":"77","observed_alias_count":2,"total_link_count":{"value":2,"knowledge":"known"},"external_reference_status":"confirmed_none","entry_paths":["C:\\test\\target\\a.dat"]}]"#));
}

#[test]
fn test_finished_complete_no_gaps_exits_0_json_and_ndjson() {
    let resp = make_sample_response(
        ScanRunOutcome::Finished,
        ScopeCoverage::Complete,
        false,
        vec![],
    );
    let outcome = ScanCallOutcome::Finished(resp.clone());

    // JSON format
    let settlement_json = settle_scan_outcome(OutputFormat::Json, &outcome, 1);
    assert_eq!(settlement_json.exit_code, EXIT_SUCCESS);
    assert!(settlement_json
        .stdout
        .contains(r#""operation_id":"scan-test-123""#));
    assert!(settlement_json
        .stdout
        .contains(r#""run_outcome":"finished""#));
    assert!(settlement_json
        .stdout
        .contains(r#""scope_coverage":"complete""#));
    assert!(settlement_json.stdout.contains(r#""coverage_gaps":[]"#));

    // NDJSON format
    let settlement_ndjson = settle_scan_outcome(OutputFormat::Ndjson, &outcome, 7);
    assert_eq!(settlement_ndjson.exit_code, EXIT_SUCCESS);
    assert!(settlement_ndjson.stdout.contains(r#""sequence_number":7"#));
    assert!(settlement_ndjson.stdout.contains(r#""channel":"data""#));
    assert!(settlement_ndjson.stdout.contains(r#""phase":"finalizing""#));
    assert!(settlement_ndjson
        .stdout
        .contains(r#""run_outcome":"finished""#));
    assert!(settlement_ndjson
        .stdout
        .contains(r#""scope_coverage":"complete""#));
}

#[test]
fn test_finished_partial_coverage_exits_4() {
    let resp = make_sample_response(
        ScanRunOutcome::Finished,
        ScopeCoverage::Partial,
        false,
        vec![],
    );
    let outcome = ScanCallOutcome::Finished(resp.clone());

    let settlement_json = settle_scan_outcome(OutputFormat::Json, &outcome, 1);
    assert_eq!(settlement_json.exit_code, EXIT_COVERAGE_GAPS_PRESENT);
    assert!(settlement_json
        .stdout
        .contains(r#""run_outcome":"finished""#));
    assert!(settlement_json
        .stdout
        .contains(r#""scope_coverage":"partial""#));

    let settlement_ndjson = settle_scan_outcome(OutputFormat::Ndjson, &outcome, 2);
    assert_eq!(settlement_ndjson.exit_code, EXIT_COVERAGE_GAPS_PRESENT);
    assert!(settlement_ndjson
        .stdout
        .contains(r#""run_outcome":"finished""#));
    assert!(settlement_ndjson
        .stdout
        .contains(r#""scope_coverage":"partial""#));
}

#[test]
fn test_finished_nonempty_gaps_exits_4() {
    let gap = CoverageGapReport {
        display_path: r#"C:\test\target\denied_folder"#.to_string(),
        status_code: "ACCESS_DENIED".to_string(),
        native_error: 5,
        error_message: "Access is denied.".to_string(),
    };
    let resp = make_sample_response(
        ScanRunOutcome::Finished,
        ScopeCoverage::Complete,
        false,
        vec![gap],
    );
    let outcome = ScanCallOutcome::Finished(resp.clone());

    let settlement_json = settle_scan_outcome(OutputFormat::Json, &outcome, 1);
    assert_eq!(settlement_json.exit_code, EXIT_COVERAGE_GAPS_PRESENT);
    assert!(settlement_json.stdout.contains("denied_folder"));
    assert!(settlement_json.stdout.contains("ACCESS_DENIED"));

    let settlement_ndjson = settle_scan_outcome(OutputFormat::Ndjson, &outcome, 3);
    assert_eq!(settlement_ndjson.exit_code, EXIT_COVERAGE_GAPS_PRESENT);
    assert!(settlement_ndjson.stdout.contains("denied_folder"));
    assert!(settlement_ndjson.stdout.contains("ACCESS_DENIED"));
}

#[test]
fn test_cancelled_outcome_exits_3() {
    let resp = make_sample_response(
        ScanRunOutcome::Cancelled,
        ScopeCoverage::Partial,
        false,
        vec![],
    );

    // Cancelled variant
    let outcome = ScanCallOutcome::Cancelled(resp.clone());
    let settlement_json = settle_scan_outcome(OutputFormat::Json, &outcome, 1);
    assert_eq!(settlement_json.exit_code, EXIT_CANCELLED);
    assert!(settlement_json
        .stdout
        .contains(r#""run_outcome":"cancelled""#));

    let settlement_ndjson = settle_scan_outcome(OutputFormat::Ndjson, &outcome, 4);
    assert_eq!(settlement_ndjson.exit_code, EXIT_CANCELLED);
    assert!(settlement_ndjson.stdout.contains(r#""sequence_number":4"#));
    assert!(settlement_ndjson
        .stdout
        .contains(r#""run_outcome":"cancelled""#));

    // Finished variant with run_outcome = Cancelled
    let outcome_finished_flagged = ScanCallOutcome::Finished(resp);
    let settlement_flagged = settle_scan_outcome(OutputFormat::Json, &outcome_finished_flagged, 1);
    assert_eq!(settlement_flagged.exit_code, EXIT_CANCELLED);
}

#[test]
fn test_failed_outcome_exits_1() {
    let resp = make_sample_response(
        ScanRunOutcome::Failed,
        ScopeCoverage::Indeterminate,
        false,
        vec![],
    );
    let outcome = ScanCallOutcome::Finished(resp.clone());

    let settlement_json = settle_scan_outcome(OutputFormat::Json, &outcome, 1);
    assert_eq!(settlement_json.exit_code, EXIT_OPERATION_FAILED);
    assert!(settlement_json.stdout.contains(r#""run_outcome":"failed""#));

    let settlement_ndjson = settle_scan_outcome(OutputFormat::Ndjson, &outcome, 5);
    assert_eq!(settlement_ndjson.exit_code, EXIT_OPERATION_FAILED);
    assert!(settlement_ndjson.stdout.contains(r#""sequence_number":5"#));
    assert!(settlement_ndjson
        .stdout
        .contains(r#""run_outcome":"failed""#));
}

#[test]
fn test_allocated_not_observed_remains_intact() {
    // 1. Not observed
    let resp_not_obs = make_sample_response(
        ScanRunOutcome::Finished,
        ScopeCoverage::Complete,
        false,
        vec![],
    );
    let settlement_not_obs_json = settle_scan_response(OutputFormat::Json, &resp_not_obs, false, 1);
    assert!(settlement_not_obs_json
        .stdout
        .contains(r#""unique_allocated_bytes":{"value":0,"knowledge":"not_observed"}"#));

    let settlement_not_obs_ndjson =
        settle_scan_response(OutputFormat::Ndjson, &resp_not_obs, false, 1);
    assert!(settlement_not_obs_ndjson
        .stdout
        .contains(r#""unique_allocated_bytes":{"value":0,"knowledge":"not_observed"}"#));

    // 2. Known allocated bytes
    let resp_known = make_sample_response(
        ScanRunOutcome::Finished,
        ScopeCoverage::Complete,
        true,
        vec![],
    );
    let settlement_known_json = settle_scan_response(OutputFormat::Json, &resp_known, false, 1);
    assert!(settlement_known_json
        .stdout
        .contains(r#""unique_allocated_bytes":{"value":2048,"knowledge":"known"}"#));

    let settlement_known_ndjson = settle_scan_response(OutputFormat::Ndjson, &resp_known, false, 1);
    assert!(settlement_known_ndjson
        .stdout
        .contains(r#""unique_allocated_bytes":{"value":2048,"knowledge":"known"}"#));
}

#[test]
fn test_output_format_parse_and_display() {
    assert_eq!("json".parse::<OutputFormat>(), Ok(OutputFormat::Json));
    assert_eq!("ndjson".parse::<OutputFormat>(), Ok(OutputFormat::Ndjson));
    assert!("invalid".parse::<OutputFormat>().is_err());
    assert_eq!(OutputFormat::Json.to_string(), "json");
    assert_eq!(OutputFormat::Ndjson.to_string(), "ndjson");
}
