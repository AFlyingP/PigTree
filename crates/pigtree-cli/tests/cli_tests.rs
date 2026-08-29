use pigtree_ipc::win32::*;
use std::collections::HashMap;
use std::os::windows::process::CommandExt;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

fn find_binary(name: &str) -> PathBuf {
    for candidate in &[
        format!("target/debug/{name}"),
        format!("target/release/{name}"),
        format!("../target/debug/{name}"),
        format!("../../target/debug/{name}"),
    ] {
        let p = PathBuf::from(candidate);
        if p.exists() {
            return p.canonicalize().unwrap_or(p);
        }
    }

    if let Ok(cur) = std::env::current_exe() {
        if let Some(parent) = cur.parent() {
            let p = parent.join(name);
            if p.exists() {
                return p;
            }
            if let Some(gp) = parent.parent() {
                let p = gp.join(name);
                if p.exists() {
                    return p;
                }
            }
        }
    }

    panic!("Binary {name} not found");
}

fn get_binaries() -> (PathBuf, PathBuf) {
    (
        find_binary("pigtree.exe"),
        find_binary("pigtree-engine.exe"),
    )
}

fn unique_session_id(prefix: &str) -> String {
    let nonce = pigtree_ipc::security::generate_nonce();
    let hex: String = nonce[0..8].iter().map(|b| format!("{b:02x}")).collect();
    format!("{prefix}-{}-{hex}", std::process::id())
}

// Simple strict JSON value parser to validate output envelopes
#[derive(Debug, PartialEq, Clone)]
enum SimpleJson {
    Null,
    Bool(bool),
    Number(f64),
    Str(String),
    Array(Vec<SimpleJson>),
    Object(HashMap<String, SimpleJson>),
}

impl SimpleJson {
    fn parse(input: &str) -> Result<SimpleJson, String> {
        let chars: Vec<char> = input.trim().chars().collect();
        let mut idx = 0;
        let val = Self::parse_value(&chars, &mut idx)?;
        Self::skip_whitespace(&chars, &mut idx);
        if idx != chars.len() {
            return Err(format!("Trailing characters at index {idx}"));
        }
        Ok(val)
    }

    fn skip_whitespace(chars: &[char], idx: &mut usize) {
        while *idx < chars.len() && (chars[*idx].is_whitespace()) {
            *idx += 1;
        }
    }

    fn parse_value(chars: &[char], idx: &mut usize) -> Result<SimpleJson, String> {
        Self::skip_whitespace(chars, idx);
        if *idx >= chars.len() {
            return Err("Unexpected EOF".to_string());
        }
        match chars[*idx] {
            'n' => {
                if *idx + 4 <= chars.len() && chars[*idx..*idx + 4] == ['n', 'u', 'l', 'l'] {
                    *idx += 4;
                    Ok(SimpleJson::Null)
                } else {
                    Err("Expected null".to_string())
                }
            }
            't' => {
                if *idx + 4 <= chars.len() && chars[*idx..*idx + 4] == ['t', 'r', 'u', 'e'] {
                    *idx += 4;
                    Ok(SimpleJson::Bool(true))
                } else {
                    Err("Expected true".to_string())
                }
            }
            'f' => {
                if *idx + 5 <= chars.len() && chars[*idx..*idx + 5] == ['f', 'a', 'l', 's', 'e'] {
                    *idx += 5;
                    Ok(SimpleJson::Bool(false))
                } else {
                    Err("Expected false".to_string())
                }
            }
            '"' => Self::parse_string(chars, idx).map(SimpleJson::Str),
            '[' => Self::parse_array(chars, idx),
            '{' => Self::parse_object(chars, idx),
            '-' | '0'..='9' => Self::parse_number(chars, idx),
            c => Err(format!("Unexpected character: {c} at index {idx}")),
        }
    }

    fn parse_string(chars: &[char], idx: &mut usize) -> Result<String, String> {
        *idx += 1; // skip opening quote
        let mut s = String::new();
        while *idx < chars.len() {
            let c = chars[*idx];
            *idx += 1;
            if c == '"' {
                return Ok(s);
            } else if c == '\\' {
                if *idx >= chars.len() {
                    return Err("Unfinished escape sequence".to_string());
                }
                let esc = chars[*idx];
                *idx += 1;
                match esc {
                    '"' => s.push('"'),
                    '\\' => s.push('\\'),
                    '/' => s.push('/'),
                    'b' => s.push('\x08'),
                    'f' => s.push('\x0c'),
                    'n' => s.push('\n'),
                    'r' => s.push('\r'),
                    't' => s.push('\t'),
                    'u' => {
                        if *idx + 4 > chars.len() {
                            return Err("Short unicode escape".to_string());
                        }
                        let hex_str: String = chars[*idx..*idx + 4].iter().collect();
                        *idx += 4;
                        let code = u32::from_str_radix(&hex_str, 16)
                            .map_err(|e| format!("Invalid unicode escape: {e}"))?;
                        if let Some(ch) = char::from_u32(code) {
                            s.push(ch);
                        }
                    }
                    other => return Err(format!("Invalid escape char: {other}")),
                }
            } else {
                s.push(c);
            }
        }
        Err("Unterminated string".to_string())
    }

    fn parse_number(chars: &[char], idx: &mut usize) -> Result<SimpleJson, String> {
        let start = *idx;
        while *idx < chars.len()
            && (chars[*idx].is_ascii_digit()
                || chars[*idx] == '.'
                || chars[*idx] == '-'
                || chars[*idx] == '+'
                || chars[*idx] == 'e'
                || chars[*idx] == 'E')
        {
            *idx += 1;
        }
        let num_str: String = chars[start..*idx].iter().collect();
        let num = num_str
            .parse::<f64>()
            .map_err(|e| format!("Invalid number: {e}"))?;
        Ok(SimpleJson::Number(num))
    }

    fn parse_array(chars: &[char], idx: &mut usize) -> Result<SimpleJson, String> {
        *idx += 1; // skip '['
        let mut arr = Vec::new();
        Self::skip_whitespace(chars, idx);
        if *idx < chars.len() && chars[*idx] == ']' {
            *idx += 1;
            return Ok(SimpleJson::Array(arr));
        }
        loop {
            let val = Self::parse_value(chars, idx)?;
            arr.push(val);
            Self::skip_whitespace(chars, idx);
            if *idx >= chars.len() {
                return Err("Unterminated array".to_string());
            }
            if chars[*idx] == ']' {
                *idx += 1;
                return Ok(SimpleJson::Array(arr));
            } else if chars[*idx] == ',' {
                *idx += 1;
            } else {
                return Err(format!("Expected ',' or ']', found {}", chars[*idx]));
            }
        }
    }

    fn parse_object(chars: &[char], idx: &mut usize) -> Result<SimpleJson, String> {
        *idx += 1; // skip '{'
        let mut map = HashMap::new();
        Self::skip_whitespace(chars, idx);
        if *idx < chars.len() && chars[*idx] == '}' {
            *idx += 1;
            return Ok(SimpleJson::Object(map));
        }
        loop {
            Self::skip_whitespace(chars, idx);
            if *idx >= chars.len() || chars[*idx] != '"' {
                return Err("Expected string key in object".to_string());
            }
            let key = Self::parse_string(chars, idx)?;
            Self::skip_whitespace(chars, idx);
            if *idx >= chars.len() || chars[*idx] != ':' {
                return Err("Expected ':' after object key".to_string());
            }
            *idx += 1; // skip ':'
            let val = Self::parse_value(chars, idx)?;
            map.insert(key, val);
            Self::skip_whitespace(chars, idx);
            if *idx >= chars.len() {
                return Err("Unterminated object".to_string());
            }
            if chars[*idx] == '}' {
                *idx += 1;
                return Ok(SimpleJson::Object(map));
            } else if chars[*idx] == ',' {
                *idx += 1;
            } else {
                return Err(format!("Expected ',' or '}}', found {}", chars[*idx]));
            }
        }
    }

    fn get(&self, key: &str) -> Option<&SimpleJson> {
        match self {
            SimpleJson::Object(m) => m.get(key),
            _ => None,
        }
    }

    fn as_str(&self) -> Option<&str> {
        match self {
            SimpleJson::Str(s) => Some(s.as_str()),
            _ => None,
        }
    }

    fn as_f64(&self) -> Option<f64> {
        match self {
            SimpleJson::Number(n) => Some(*n),
            _ => None,
        }
    }
}

fn check_stderr_diagnostics(stderr: &str) {
    if stderr.trim().is_empty() {
        return;
    }
    for line in stderr.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let json = SimpleJson::parse(line).expect("stderr line must be valid NDJSON diagnostic");
        assert!(json.get("level").is_some(), "diagnostic missing level");
        assert!(json.get("target").is_some(), "diagnostic missing target");
        assert!(json.get("message").is_some(), "diagnostic missing message");
    }
}

#[test]
fn test_cli_commands_spawn_engine_and_strict_json() {
    let (cli_exe, engine_exe) = get_binaries();

    // 1. Ping
    let output = Command::new(&cli_exe)
        .args(["ping", "--engine-path", engine_exe.to_str().unwrap()])
        .output()
        .expect("exec ping");
    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8(output.stdout).expect("utf8 stdout");
    let json = SimpleJson::parse(&stdout).expect("ping stdout strict JSON");
    assert_eq!(json.get("version").and_then(|v| v.as_str()), Some("1.0"));
    assert_eq!(
        json.get("request_id").and_then(|v| v.as_str()),
        Some("ping")
    );
    assert_eq!(json.get("status").and_then(|v| v.as_str()), Some("success"));
    let data = json.get("data").expect("ping data");
    assert!(
        data.get("timestamp_utc_ms")
            .and_then(|v| v.as_f64())
            .unwrap()
            > 0.0
    );
    assert!(
        data.get("echo_timestamp_utc_ms")
            .and_then(|v| v.as_f64())
            .unwrap()
            > 0.0
    );
    check_stderr_diagnostics(&String::from_utf8_lossy(&output.stderr));

    // 2. Echo with quotes, backslashes, special characters
    let echo_str = r#"Hello "world"  test ' special / chars"#;
    let output = Command::new(&cli_exe)
        .args([
            "echo",
            echo_str,
            "--engine-path",
            engine_exe.to_str().unwrap(),
        ])
        .output()
        .expect("exec echo");
    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8(output.stdout).expect("utf8 stdout");
    let json = SimpleJson::parse(&stdout).expect("echo stdout strict JSON");
    assert_eq!(json.get("status").and_then(|v| v.as_str()), Some("success"));
    let data = json.get("data").expect("echo data");
    assert_eq!(data.get("payload").and_then(|v| v.as_str()), Some(echo_str));
    check_stderr_diagnostics(&String::from_utf8_lossy(&output.stderr));

    // 3. Health
    let output = Command::new(&cli_exe)
        .args([
            "health",
            "--include-memory",
            "--engine-path",
            engine_exe.to_str().unwrap(),
        ])
        .output()
        .expect("exec health");
    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8(output.stdout).expect("utf8 stdout");
    let json = SimpleJson::parse(&stdout).expect("health stdout strict JSON");
    assert_eq!(json.get("status").and_then(|v| v.as_str()), Some("success"));
    let data = json.get("data").expect("health data");
    assert_eq!(data.get("status").and_then(|v| v.as_str()), Some("HEALTHY"));
    assert!(data.get("uptime_ms").and_then(|v| v.as_f64()).is_some());
    assert!(data
        .get("memory_private_bytes")
        .and_then(|v| v.as_f64())
        .is_some());
    assert!(data.get("handle_count").and_then(|v| v.as_f64()).unwrap() > 0.0);
    check_stderr_diagnostics(&String::from_utf8_lossy(&output.stderr));

    // 4. Status
    let output = Command::new(&cli_exe)
        .args(["status", "--engine-path", engine_exe.to_str().unwrap()])
        .output()
        .expect("exec status");
    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8(output.stdout).expect("utf8 stdout");
    let json = SimpleJson::parse(&stdout).expect("status stdout strict JSON");
    assert_eq!(json.get("status").and_then(|v| v.as_str()), Some("success"));
    let data = json.get("data").expect("status data");
    assert_eq!(data.get("state").and_then(|v| v.as_str()), Some("IDLE"));
    assert_eq!(data.get("active_runs").and_then(|v| v.as_f64()), Some(0.0));
    assert!(!data
        .get("session_id")
        .and_then(|v| v.as_str())
        .unwrap()
        .is_empty());
    check_stderr_diagnostics(&String::from_utf8_lossy(&output.stderr));

    // 5. Version
    let output = Command::new(&cli_exe)
        .args(["version", "--engine-path", engine_exe.to_str().unwrap()])
        .output()
        .expect("exec version");
    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8(output.stdout).expect("utf8 stdout");
    let json = SimpleJson::parse(&stdout).expect("version stdout strict JSON");
    assert_eq!(json.get("status").and_then(|v| v.as_str()), Some("success"));
    let data = json.get("data").expect("version data");
    assert_eq!(
        data.get("engine_version").and_then(|v| v.as_str()),
        Some("0.1.0")
    );
    assert_eq!(
        data.get("protocol_version").and_then(|v| v.as_f64()),
        Some(1.0)
    );
    assert_ne!(data.get("build_date").and_then(|v| v.as_str()), Some(""));
    assert_ne!(data.get("commit_hash").and_then(|v| v.as_str()), Some(""));
    check_stderr_diagnostics(&String::from_utf8_lossy(&output.stderr));

    // 6. CLI Version flag (-V)
    let output = Command::new(&cli_exe).arg("-V").output().expect("exec -V");
    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8(output.stdout).expect("utf8 stdout");
    let json = SimpleJson::parse(&stdout).expect("cli version strict JSON");
    assert_eq!(json.get("status").and_then(|v| v.as_str()), Some("success"));
    let data = json.get("data").expect("data");
    assert_eq!(
        data.get("cli_version").and_then(|v| v.as_str()),
        Some("0.1.0")
    );
    assert_eq!(
        data.get("protocol_version").and_then(|v| v.as_f64()),
        Some(1.0)
    );
}

#[test]
fn test_cli_malformed_and_unknown_args_exit_2() {
    let (cli_exe, _) = get_binaries();

    // 1. Unknown subcommand
    let output = Command::new(&cli_exe)
        .arg("unknown_subcommand_123")
        .output()
        .expect("exec unknown");
    assert_eq!(output.status.code(), Some(2));
    let stdout = String::from_utf8(output.stdout).expect("utf8 stdout");
    let json = SimpleJson::parse(&stdout).expect("strict JSON error envelope");
    assert_eq!(json.get("status").and_then(|v| v.as_str()), Some("error"));
    let err = json.get("error").expect("error object");
    assert_eq!(
        err.get("code").and_then(|v| v.as_str()),
        Some("COMMAND_ERROR")
    );
    check_stderr_diagnostics(&String::from_utf8_lossy(&output.stderr));

    // 2. Missing value for --engine-path
    let output = Command::new(&cli_exe)
        .arg("--engine-path")
        .output()
        .expect("exec missing engine-path value");
    assert_eq!(output.status.code(), Some(2));
    let stdout = String::from_utf8(output.stdout).expect("utf8 stdout");
    let json = SimpleJson::parse(&stdout).expect("strict JSON");
    assert_eq!(json.get("status").and_then(|v| v.as_str()), Some("error"));
    check_stderr_diagnostics(&String::from_utf8_lossy(&output.stderr));

    // 3. Missing value for --format
    let output = Command::new(&cli_exe)
        .arg("--format")
        .output()
        .expect("exec missing format value");
    assert_eq!(output.status.code(), Some(2));
    let stdout = String::from_utf8(output.stdout).expect("utf8 stdout");
    let json = SimpleJson::parse(&stdout).expect("strict JSON error envelope");
    assert_eq!(json.get("status").and_then(|v| v.as_str()), Some("error"));
    let err = json.get("error").expect("error object");
    assert_eq!(
        err.get("code").and_then(|v| v.as_str()),
        Some("COMMAND_ERROR")
    );
    assert!(err
        .get("message")
        .and_then(|v| v.as_str())
        .unwrap()
        .contains("Missing value for --format"));
    check_stderr_diagnostics(&String::from_utf8_lossy(&output.stderr));

    // 4. Invalid value for --format
    let output = Command::new(&cli_exe)
        .args(["--format", "unsupported_fmt", "ping"])
        .output()
        .expect("exec invalid format");
    assert_eq!(output.status.code(), Some(2));
    let stdout = String::from_utf8(output.stdout).expect("utf8 stdout");
    let json = SimpleJson::parse(&stdout).expect("strict JSON error envelope");
    assert_eq!(json.get("status").and_then(|v| v.as_str()), Some("error"));
    let err = json.get("error").expect("error object");
    assert_eq!(
        err.get("code").and_then(|v| v.as_str()),
        Some("COMMAND_ERROR")
    );
    assert!(err
        .get("message")
        .and_then(|v| v.as_str())
        .unwrap()
        .contains("Invalid value for --format"));
    check_stderr_diagnostics(&String::from_utf8_lossy(&output.stderr));

    // 5. No subcommand at all
    let output = Command::new(&cli_exe).output().expect("exec bare CLI");
    assert_eq!(output.status.code(), Some(2));
    let stdout = String::from_utf8(output.stdout).expect("utf8 stdout");
    let json = SimpleJson::parse(&stdout).expect("strict JSON");
    assert_eq!(json.get("status").and_then(|v| v.as_str()), Some("error"));
    check_stderr_diagnostics(&String::from_utf8_lossy(&output.stderr));
}

#[test]
fn test_cli_bad_engine_path_exit_1() {
    let (cli_exe, _) = get_binaries();

    let output = Command::new(&cli_exe)
        .args([
            "--engine-path",
            r#"C:\nonexistent_dir_123\pigtree-engine.exe"#,
            "ping",
        ])
        .output()
        .expect("exec bad engine path");
    assert_eq!(output.status.code(), Some(1));
    let stdout = String::from_utf8(output.stdout).expect("utf8 stdout");
    let json = SimpleJson::parse(&stdout).expect("strict JSON");
    assert_eq!(json.get("status").and_then(|v| v.as_str()), Some("error"));
    let err = json.get("error").expect("error object");
    assert_eq!(
        err.get("code").and_then(|v| v.as_str()),
        Some("OPERATION_FAILED")
    );
    check_stderr_diagnostics(&String::from_utf8_lossy(&output.stderr));
}

#[test]
fn test_cli_ctrl_c_cancellation_exits_3() {
    use std::io::{BufRead, BufReader};

    let (cli_exe, engine_exe) = get_binaries();

    let mut child = Command::new(&cli_exe)
        .args([
            "ping",
            "--test-delay-ms",
            "5000",
            "--engine-path",
            engine_exe.to_str().unwrap(),
        ])
        .creation_flags(CREATE_NEW_PROCESS_GROUP)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn CLI child");

    let pid = child.id();
    let stderr_pipe = child.stderr.take().expect("take stderr");
    let stdout_pipe = child.stdout.take().expect("take stdout");

    let (tx, rx) = std::sync::mpsc::channel();
    let stderr_thread = std::thread::spawn(move || {
        let mut reader = BufReader::new(stderr_pipe);
        let mut line = String::new();
        let mut full_stderr = String::new();
        let mut session_established = false;
        while reader.read_line(&mut line).unwrap_or(0) > 0 {
            if line.contains("Established authenticated session") && !session_established {
                session_established = true;
                let _ = tx.send(());
            }
            full_stderr.push_str(&line);
            line.clear();
        }
        full_stderr
    });

    let stdout_thread = std::thread::spawn(move || {
        let mut reader = BufReader::new(stdout_pipe);
        let mut full_stdout = String::new();
        let mut line = String::new();
        while reader.read_line(&mut line).unwrap_or(0) > 0 {
            full_stdout.push_str(&line);
            line.clear();
        }
        full_stdout
    });

    // Wait until engine is confirmed started and authenticated (up to 5s)
    rx.recv_timeout(Duration::from_millis(5000))
        .expect("CLI must establish authenticated session before signal");

    // Send console control event to process group
    unsafe {
        let res = GenerateConsoleCtrlEvent(CTRL_BREAK_EVENT, pid);
        assert_ne!(res, 0, "GenerateConsoleCtrlEvent failed");
    }

    let status = child.wait().expect("wait on child");
    assert_eq!(
        status.code(),
        Some(3),
        "exit code must be 3 on cancellation"
    );

    let full_stderr = stderr_thread.join().expect("join stderr");
    let full_stdout = stdout_thread.join().expect("join stdout");

    let json = SimpleJson::parse(&full_stdout).expect("strict JSON cancelled envelope");
    assert_eq!(json.get("version").and_then(|v| v.as_str()), Some("1.0"));
    assert_eq!(
        json.get("status").and_then(|v| v.as_str()),
        Some("cancelled")
    );
    let err = json.get("error").expect("error object");
    assert_eq!(
        err.get("code").and_then(|v| v.as_str()),
        Some("OPERATION_CANCELLED")
    );

    check_stderr_diagnostics(&full_stderr);
}

#[test]
fn test_normal_completion_leaves_no_engine() {
    let (cli_exe, engine_exe) = get_binaries();

    let output = Command::new(&cli_exe)
        .args(["ping", "--engine-path", engine_exe.to_str().unwrap()])
        .stderr(Stdio::piped())
        .output()
        .expect("exec ping");
    assert_eq!(output.status.code(), Some(0));

    let stderr = String::from_utf8_lossy(&output.stderr);
    // Find engine PID from diagnostic stderr: e.g. "Engine PID 1234"
    let pos = stderr
        .find("Engine PID ")
        .expect("Engine PID must be present in stderr diagnostics");
    let rest = &stderr[pos + 11..];
    let num_str: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
    let pid: u32 = num_str.parse().expect("Valid engine PID in stderr");
    assert!(pid > 0, "Discovered PID must be positive");

    // Check process handle to verify process is terminated
    unsafe {
        let h_proc = OpenProcess(PROCESS_QUERY_INFORMATION, FALSE, pid);
        if !h_proc.is_null() && h_proc != INVALID_HANDLE_VALUE {
            let mut exit_code: DWORD = 0;
            GetExitCodeProcess(h_proc, &mut exit_code);
            CloseHandle(h_proc);
            assert_ne!(
                exit_code, 259,
                "Engine process {pid} must be terminated (not STILL_ACTIVE 259)"
            );
        }
    }
}

#[test]
fn test_kill_on_job_close_reaps_engine_on_abrupt_parent_termination() {
    let (_, engine_exe) = get_binaries();

    let job = pigtree_ipc::job::JobObject::create_kill_on_close().expect("create job object");
    let bootstrap_nonce = pigtree_ipc::security::generate_nonce();
    let mut bootstrap = pigtree_ipc::bootstrap::BootstrapPipe::create().expect("create bootstrap");
    bootstrap
        .write_nonce(&bootstrap_nonce)
        .expect("write nonce");

    let sess_id = unique_session_id("jobclose");
    let pipe_name = pigtree_ipc::pipe::format_pipe_name(&sess_id);

    let child =
        pigtree_ipc::bootstrap::spawn_engine(&engine_exe, &pipe_name, &sess_id, &bootstrap, &job)
            .expect("spawn engine");

    let engine_pid = child.pid;

    // Verify engine is alive
    unsafe {
        let mut exit_code: DWORD = 0;
        GetExitCodeProcess(child.h_process, &mut exit_code);
        assert_eq!(exit_code, 259, "Engine should initially be STILL_ACTIVE");
    }

    // Drop job object (simulating parent process abrupt death / handle close)
    drop(job);
    drop(child);

    // Wait bounded time (<1000ms) and verify engine is reaped by kernel
    let start = Instant::now();
    let mut reaped = false;
    while start.elapsed() < Duration::from_millis(1500) {
        unsafe {
            let h_proc = OpenProcess(PROCESS_QUERY_INFORMATION, FALSE, engine_pid);
            if h_proc.is_null() || h_proc == INVALID_HANDLE_VALUE {
                reaped = true;
                break;
            }
            let mut exit_code: DWORD = 0;
            GetExitCodeProcess(h_proc, &mut exit_code);
            CloseHandle(h_proc);
            if exit_code != 259 {
                reaped = true;
                break;
            }
        }
        std::thread::sleep(Duration::from_millis(20));
    }

    assert!(
        reaped,
        "Engine process {engine_pid} was not reaped by KILL_ON_JOB_CLOSE within deadline"
    );
}

#[test]
fn test_engine_command_line_during_hidden_delay_has_no_secrets() {
    let (cli_exe, engine_exe) = get_binaries();

    let mut child = Command::new(&cli_exe)
        .args([
            "ping",
            "--test-delay-ms",
            "3000",
            "--engine-path",
            engine_exe.to_str().unwrap(),
        ])
        .stderr(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn CLI");

    // Wait for engine to start
    std::thread::sleep(Duration::from_millis(300));

    // Inspect child processes or command lines of engine
    // We can query engine by scanning or querying via get_process_command_line
    // CLI spawns engine as child, let's query all processes named pigtree-engine.exe
    // or inspect command line
    let engine_cmd_line = {
        let mut found_cmd: Option<String> = None;
        for _ in 0..10 {
            unsafe {
                #[link(name = "kernel32")]
                extern "system" {
                    fn CreateToolhelp32Snapshot(dwFlags: DWORD, th32ProcessID: DWORD) -> HANDLE;
                    fn Process32FirstW(hSnapshot: HANDLE, lppe: *mut PROCESSENTRY32W) -> BOOL;
                    fn Process32NextW(hSnapshot: HANDLE, lppe: *mut PROCESSENTRY32W) -> BOOL;
                }
                #[repr(C)]
                #[allow(non_snake_case)]
                struct PROCESSENTRY32W {
                    dwSize: DWORD,
                    cntUsage: DWORD,
                    th32ProcessID: DWORD,
                    th32DefaultHeapID: usize,
                    th32ModuleID: DWORD,
                    cntThreads: DWORD,
                    th32ParentProcessID: DWORD,
                    pcPriClassBase: i32,
                    dwFlags: DWORD,
                    szExeFile: [u16; 260],
                }

                let snap = CreateToolhelp32Snapshot(0x00000002 /* SNAPPROCESS */, 0);
                if snap != INVALID_HANDLE_VALUE {
                    let mut pe: PROCESSENTRY32W = std::mem::zeroed();
                    pe.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as DWORD;
                    if Process32FirstW(snap, &mut pe) != 0 {
                        loop {
                            let name = String::from_utf16_lossy(&pe.szExeFile);
                            if name.starts_with("pigtree-engine.exe") {
                                if let Ok(cmd) = get_process_command_line(pe.th32ProcessID) {
                                    if cmd.contains("--pipe-name") {
                                        found_cmd = Some(cmd);
                                        break;
                                    }
                                }
                            }
                            if Process32NextW(snap, &mut pe) == 0 {
                                break;
                            }
                        }
                    }
                    CloseHandle(snap);
                }
            }
            if found_cmd.is_some() {
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        found_cmd
    };

    let cmd =
        engine_cmd_line.expect("Must discover running pigtree-engine process and its command line");

    // Assert no 64-hex char secret / 32-byte hex nonce appears in command line
    assert!(cmd.contains("--pipe-name"), "must contain --pipe-name");
    assert!(cmd.contains("--session-id"), "must contain --session-id");
    assert!(
        cmd.contains("--bootstrap-handle"),
        "must contain --bootstrap-handle"
    );

    // Verify that no 64-hex character sequence exists
    let re_64_hex = |s: &str| {
        let chars: Vec<char> = s.chars().collect();
        for window in chars.windows(64) {
            if window.iter().all(|c| c.is_ascii_hexdigit()) {
                return true;
            }
        }
        false
    };
    assert!(
        !re_64_hex(&cmd),
        "Command line must not contain 64-hex secret: {cmd}"
    );

    let _ = child.wait();
}

#[test]
fn test_first_pipe_instance_flag_prevents_pipe_squatting() {
    let sess_id = unique_session_id("squat");
    let pipe_name = pigtree_ipc::pipe::format_pipe_name(&sess_id);

    // 1. Pipe squatting prevention: first instance succeeds, second instance must fail closed
    let server1 =
        pigtree_ipc::pipe::NamedPipeServer::create(&pipe_name).expect("server 1 creates pipe");
    let server2_res = pigtree_ipc::pipe::NamedPipeServer::create(&pipe_name);
    assert!(
        server2_res.is_err(),
        "Duplicate server creation must fail closed (FILE_FLAG_FIRST_PIPE_INSTANCE)"
    );

    drop(server1);
}
