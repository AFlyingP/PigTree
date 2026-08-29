//! Standalone disposable scan worker executable (pigtree-scan-worker.exe).

use pigtree_protocol::{ObservationWriter, RunOutcome};
use pigtree_scan_worker::{
    parse_worker_args, scan_directory, PipeWriter, Win32EventCancellation, HANDLE,
};
use std::env;
use std::path::Path;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = env::args().collect();
    let parsed = match parse_worker_args(args.iter().skip(1).cloned()) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("Error parsing arguments: {e}");
            eprintln!("Usage: pigtree-scan-worker.exe --target <path> --pipe-handle <integer> --cancel-event-handle <integer>");
            return ExitCode::from(2);
        }
    };

    // Defensively validate scan target before initializing observation writer
    if let Err(err) = pigtree_ipc::validator::validate_scan_target(&parsed.target) {
        eprintln!("Invalid scan target: {err}");
        return ExitCode::from(2);
    }

    let target_path = Path::new(&parsed.target);
    let pipe_writer = PipeWriter::from_raw(parsed.pipe_handle as HANDLE);
    let cancellation = Win32EventCancellation::from_raw(parsed.cancel_event_handle as HANDLE);

    let mut writer = match ObservationWriter::new(pipe_writer, &parsed.target) {
        Ok(w) => w,
        Err(e) => {
            eprintln!("Failed to initialize observation writer: {e}");
            return ExitCode::from(1);
        }
    };

    match scan_directory(target_path, &mut writer, &cancellation) {
        Ok(term) => match term.outcome {
            RunOutcome::Finished => ExitCode::from(0),
            RunOutcome::Cancelled => ExitCode::from(3),
            RunOutcome::Failed => ExitCode::from(1),
        },
        Err(e) => {
            eprintln!("Scan error: {e}");
            ExitCode::from(1)
        }
    }
}
