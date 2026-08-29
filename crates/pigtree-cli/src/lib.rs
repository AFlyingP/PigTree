//! Command-line decision, formatting, and exit-code settlement library for PigTree.

use pigtree_ipc::client::ScanCallOutcome;
use pigtree_protocol::json::{format_scan_terminal_json, format_scan_terminal_ndjson_event};
use pigtree_protocol::protobuf::{ScanResponse, ScanRunOutcome, ScopeCoverage};
use std::fmt;
use std::str::FromStr;

pub const EXIT_SUCCESS: u8 = 0;
pub const EXIT_OPERATION_FAILED: u8 = 1;
pub const EXIT_COMMAND_ERROR: u8 = 2;
pub const EXIT_CANCELLED: u8 = 3;
pub const EXIT_COVERAGE_GAPS_PRESENT: u8 = 4;

/// Target output format for CLI emissions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Json,
    Ndjson,
}

impl FromStr for OutputFormat {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "json" => Ok(OutputFormat::Json),
            "ndjson" => Ok(OutputFormat::Ndjson),
            other => Err(format!(
                "Invalid value for --format: '{other}'. Expected 'json' or 'ndjson'"
            )),
        }
    }
}

impl fmt::Display for OutputFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OutputFormat::Json => write!(f, "json"),
            OutputFormat::Ndjson => write!(f, "ndjson"),
        }
    }
}

/// Settled scan execution state including exit code and formatted stdout string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanSettlement {
    pub exit_code: u8,
    pub stdout: String,
}

/// Settle a `ScanResponse` into its CLI terminal stdout formatting and process exit code.
pub fn settle_scan_response(
    format: OutputFormat,
    response: &ScanResponse,
    is_cancelled: bool,
    terminal_sequence_number: u64,
) -> ScanSettlement {
    let exit_code = if is_cancelled || response.run_outcome == ScanRunOutcome::Cancelled as i32 {
        EXIT_CANCELLED
    } else if !response.coverage_gaps.is_empty()
        || response.scope_coverage == ScopeCoverage::Partial as i32
    {
        EXIT_COVERAGE_GAPS_PRESENT
    } else if response.run_outcome == ScanRunOutcome::Failed as i32 {
        EXIT_OPERATION_FAILED
    } else {
        EXIT_SUCCESS
    };

    let stdout = match format {
        OutputFormat::Json => format_scan_terminal_json(response),
        OutputFormat::Ndjson => {
            format_scan_terminal_ndjson_event(response, terminal_sequence_number)
        }
    };

    ScanSettlement { exit_code, stdout }
}

/// Settle a `ScanCallOutcome` into its CLI terminal stdout formatting and process exit code.
pub fn settle_scan_outcome(
    format: OutputFormat,
    outcome: &ScanCallOutcome,
    terminal_sequence_number: u64,
) -> ScanSettlement {
    match outcome {
        ScanCallOutcome::Finished(response) => {
            settle_scan_response(format, response, false, terminal_sequence_number)
        }
        ScanCallOutcome::Cancelled(response) => {
            settle_scan_response(format, response, true, terminal_sequence_number)
        }
    }
}

/// Alias for `settle_scan_outcome`.
pub fn settle_scan(
    format: OutputFormat,
    outcome: &ScanCallOutcome,
    terminal_sequence_number: u64,
) -> ScanSettlement {
    settle_scan_outcome(format, outcome, terminal_sequence_number)
}
