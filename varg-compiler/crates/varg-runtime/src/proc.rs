// Wave 28: Process Management
//
// Spawned-process handles with bidirectional communication.
// Provides what `exec` cannot: long-lived child processes, stdin writing,
// line-by-line stdout reading, waiting, and killing.
//
// Use cases: MCP server subprocesses, language servers, external agents,
// interactive child processes.

use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdout, Command, Stdio};
use std::sync::{Arc, Mutex};

pub struct ProcState {
    pub child: Child,
    pub stdout_reader: Option<BufReader<ChildStdout>>,
}

impl std::fmt::Debug for ProcState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProcState")
            .field("pid", &self.child.id())
            .finish()
    }
}

pub type ProcHandle = Arc<Mutex<ProcState>>;

/// Run a command through the platform shell and hand back what it printed.
///
/// Two things were wrong with the version inlined in the codegen. On Windows the command went
/// through Rust's argument escaping, which `cmd.exe` does not understand ~ so a command
/// containing quotes, which any path with a space needs, arrived mangled and the shell reported
/// a program it could not find. And only stdout was returned, so a command that failed came back
/// as an empty string with no reason and no failure: `exec("cmd-that-does-not-exist")` answered
/// `Ok("")`.
#[cfg(windows)]
fn shell_output(cmd: &str) -> std::io::Result<std::process::Output> {
    use std::os::windows::process::CommandExt;
    // raw_arg passes the string to cmd.exe exactly as written; the surrounding pair is cmd's own
    // convention. Given a line that begins with a quote it strips the first and the last one, so
    // `"C:/Program Files/x.exe" --flag` lost its quoting and became a program it could not find.
    // The extra pair is what it removes instead.
    Command::new("cmd")
        .arg("/C")
        .raw_arg(format!("\"{}\"", cmd))
        .output()
}

#[cfg(not(windows))]
fn shell_output(cmd: &str) -> std::io::Result<std::process::Output> {
    Command::new("sh").args(["-c", cmd]).output()
}

pub fn __varg_exec(cmd: &str) -> Result<String, String> {
    let out = shell_output(cmd).map_err(|e| e.to_string())?;
    if !out.status.success() {
        let complaint = String::from_utf8_lossy(&out.stderr).trim().to_string();
        return Err(if complaint.is_empty() {
            format!("`{}` exited with {}", cmd, out.status.code().unwrap_or(-1))
        } else {
            complaint
        });
    }
    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}

/// Spawn a child process via the platform shell.
/// Stdin, stdout, stderr are all piped.
pub fn __varg_proc_spawn(cmd: &str) -> Result<ProcHandle, String> {
    let mut command = if cfg!(target_os = "windows") {
        let mut c = Command::new("cmd");
        c.args(["/C", cmd]);
        c
    } else {
        let mut c = Command::new("sh");
        c.args(["-c", cmd]);
        c
    };
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        // stderr is inherited (goes to the parent's terminal) rather than piped: nothing
        // ever drains a piped stderr, so a child that writes more than the ~64 KB pipe buffer
        // to stderr (e.g. cargo/rustc progress) would deadlock waiting for it to be read.
        .stderr(Stdio::inherit());

    let mut child = command
        .spawn()
        .map_err(|e| format!("proc_spawn failed: {}", e))?;
    let stdout = child.stdout.take();
    let stdout_reader = stdout.map(BufReader::new);

    Ok(Arc::new(Mutex::new(ProcState {
        child,
        stdout_reader,
    })))
}

/// Spawn a child process directly (program + args) without a shell.
/// Safer than `proc_spawn` when arguments come from untrusted sources
/// because there is no shell interpretation.
pub fn __varg_proc_spawn_args(program: &str, args: Vec<String>) -> Result<ProcHandle, String> {
    let mut command = Command::new(program);
    command.args(&args);
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        // stderr is inherited (goes to the parent's terminal) rather than piped: nothing
        // ever drains a piped stderr, so a child that writes more than the ~64 KB pipe buffer
        // to stderr (e.g. cargo/rustc progress) would deadlock waiting for it to be read.
        .stderr(Stdio::inherit());

    let mut child = command
        .spawn()
        .map_err(|e| format!("proc_spawn_args failed: {}", e))?;
    let stdout = child.stdout.take();
    let stdout_reader = stdout.map(BufReader::new);

    Ok(Arc::new(Mutex::new(ProcState {
        child,
        stdout_reader,
    })))
}

/// Write a string to the child's stdin and flush.
pub fn __varg_proc_write_stdin(handle: &ProcHandle, data: &str) -> Result<(), String> {
    let mut state = handle
        .lock()
        .map_err(|e| format!("proc lock poisoned: {}", e))?;
    if let Some(stdin) = state.child.stdin.as_mut() {
        stdin
            .write_all(data.as_bytes())
            .map_err(|e| format!("stdin write: {}", e))?;
        stdin.flush().map_err(|e| format!("stdin flush: {}", e))?;
        Ok(())
    } else {
        Err("stdin not available".to_string())
    }
}

/// Close stdin so the child sees EOF.
pub fn __varg_proc_close_stdin(handle: &ProcHandle) -> Result<(), String> {
    let mut state = handle
        .lock()
        .map_err(|e| format!("proc lock poisoned: {}", e))?;
    drop(state.child.stdin.take());
    Ok(())
}

/// Read one line from the child's stdout.
/// Returns an empty string on EOF.
pub fn __varg_proc_read_line(handle: &ProcHandle) -> Result<String, String> {
    let mut state = handle
        .lock()
        .map_err(|e| format!("proc lock poisoned: {}", e))?;
    if let Some(reader) = state.stdout_reader.as_mut() {
        let mut line = String::new();
        match reader.read_line(&mut line) {
            Ok(0) => Ok(String::new()),
            Ok(_) => Ok(line
                .trim_end_matches(|c: char| c == '\n' || c == '\r')
                .to_string()),
            Err(e) => Err(format!("stdout read: {}", e)),
        }
    } else {
        Err("stdout not available".to_string())
    }
}

/// Wait for the child to exit and return its exit code.
/// Closes stdin first so the child is not blocked waiting on input.
pub fn __varg_proc_wait(handle: &ProcHandle) -> Result<i64, String> {
    let mut state = handle
        .lock()
        .map_err(|e| format!("proc lock poisoned: {}", e))?;
    drop(state.child.stdin.take());
    let status = state
        .child
        .wait()
        .map_err(|e| format!("proc wait: {}", e))?;
    Ok(status.code().unwrap_or(-1) as i64)
}

/// Kill the child process.
pub fn __varg_proc_kill(handle: &ProcHandle) -> Result<(), String> {
    let mut state = handle
        .lock()
        .map_err(|e| format!("proc lock poisoned: {}", e))?;
    state
        .child
        .kill()
        .map_err(|e| format!("proc kill: {}", e))
}

/// Check if the child is still running (non-blocking).
pub fn __varg_proc_is_alive(handle: &ProcHandle) -> bool {
    let mut state = match handle.lock() {
        Ok(s) => s,
        Err(_) => return false,
    };
    match state.child.try_wait() {
        Ok(Some(_)) => false,  // exited
        Ok(None) => true,       // still running
        Err(_) => false,
    }
}

/// Return the PID of the child process.
pub fn __varg_proc_pid(handle: &ProcHandle) -> i64 {
    let state = match handle.lock() {
        Ok(s) => s,
        Err(_) => return -1,
    };
    state.child.id() as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    fn echo_cmd(text: &str) -> String {
        if cfg!(target_os = "windows") {
            format!("echo {}", text)
        } else {
            format!("echo '{}'", text)
        }
    }

    #[test]
    fn test_proc_spawn_echo_and_read_line() {
        let handle = __varg_proc_spawn(&echo_cmd("hello_varg")).unwrap();
        let line = __varg_proc_read_line(&handle).unwrap();
        assert!(line.contains("hello_varg"), "got: {:?}", line);
    }

    #[test]
    fn test_proc_wait_returns_exit_code() {
        let handle = __varg_proc_spawn(&echo_cmd("done")).unwrap();
        // Drain stdout first so the child can exit cleanly on all platforms
        let _ = __varg_proc_read_line(&handle);
        let code = __varg_proc_wait(&handle).unwrap();
        assert_eq!(code, 0);
    }

    #[test]
    fn test_proc_pid_is_positive() {
        let handle = __varg_proc_spawn(&echo_cmd("x")).unwrap();
        let pid = __varg_proc_pid(&handle);
        assert!(pid > 0, "pid should be positive, got {}", pid);
        let _ = __varg_proc_wait(&handle);
    }

    #[test]
    fn test_proc_read_line_eof_returns_empty() {
        let handle = __varg_proc_spawn(&echo_cmd("one")).unwrap();
        // First line has content
        let _first = __varg_proc_read_line(&handle).unwrap();
        // Subsequent reads should eventually return EOF (empty)
        let second = __varg_proc_read_line(&handle).unwrap();
        assert_eq!(second, "");
        let _ = __varg_proc_wait(&handle);
    }

    #[test]
    fn test_proc_spawn_args_no_shell() {
        // Use `cmd /C echo` on Windows, `sh -c echo` on Unix via direct args
        let handle = if cfg!(target_os = "windows") {
            __varg_proc_spawn_args("cmd", vec!["/C".to_string(), "echo".to_string(), "argmode".to_string()]).unwrap()
        } else {
            __varg_proc_spawn_args("sh", vec!["-c".to_string(), "echo argmode".to_string()]).unwrap()
        };
        let line = __varg_proc_read_line(&handle).unwrap();
        assert!(line.contains("argmode"), "got: {:?}", line);
        let _ = __varg_proc_wait(&handle);
    }
}
