use exaterm_core::daemon::LocalBeachheadClient;
use std::ffi::OsString;
use std::fs;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const REMOTE_STATE_SUBDIR: &str = ".local/state/exaterm";
const REMOTE_RUNTIME_SUBDIR: &str = ".local/state/exaterm/runtime";
const REMOTE_BIN_SUBDIR: &str = ".local/state/exaterm/bin";
const CONTROL_SOCKET_NAME: &str = "beachhead-control.sock";
const RAW_SOCKET_NAME: &str = "beachhead-stream.sock";

pub struct RemoteBeachheadBridge {
    forward_process: Child,
    local_socket_dir: PathBuf,
}

impl Drop for RemoteBeachheadBridge {
    fn drop(&mut self) {
        let _ = self.forward_process.kill();
        let _ = self.forward_process.wait();
        let _ = fs::remove_dir_all(&self.local_socket_dir);
    }
}

struct RemoteHostInfo {
    os: String,
    arch: String,
    home: String,
}

pub fn connect_remote(target: &str) -> Result<(LocalBeachheadClient, RemoteBeachheadBridge), String> {
    let info = probe_remote_host(target)?;
    ensure_supported_remote(&info)?;

    let local_exatermd = local_exatermd_path()?;
    let remote_root = format!("{}/{}", info.home, REMOTE_STATE_SUBDIR);
    let remote_bin_dir = format!("{}/{}", info.home, REMOTE_BIN_SUBDIR);
    let remote_runtime_dir = format!("{}/{}", info.home, REMOTE_RUNTIME_SUBDIR);
    let remote_bin = format!("{remote_bin_dir}/exatermd");
    let remote_control = format!("{remote_runtime_dir}/exaterm/{CONTROL_SOCKET_NAME}");
    let remote_raw = format!("{remote_runtime_dir}/exaterm/{RAW_SOCKET_NAME}");

    ensure_remote_dirs(target, &remote_root, &remote_bin_dir, &remote_runtime_dir)?;
    upload_remote_exatermd(target, &local_exatermd, &remote_bin)?;
    launch_remote_daemon(target, &remote_bin, &remote_runtime_dir, &remote_control, &remote_raw)?;

    let local_socket_dir = unique_local_socket_dir("ssh-bridge");
    fs::create_dir_all(&local_socket_dir)
        .map_err(|error| format!("create local socket dir: {error}"))?;
    let local_control = local_socket_dir.join("control.sock");
    let local_raw = local_socket_dir.join("stream.sock");

    let mut forward = Command::new("ssh");
    forward
        .arg("-o")
        .arg("ExitOnForwardFailure=yes")
        .arg("-o")
        .arg("StreamLocalBindUnlink=yes")
        .arg("-N")
        .arg("-L")
        .arg(format!("{}:{}", local_control.display(), remote_control))
        .arg("-L")
        .arg(format!("{}:{}", local_raw.display(), remote_raw))
        .arg(target)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let forward_process = forward
        .spawn()
        .map_err(|error| format!("failed to start SSH socket forwarder: {error}"))?;

    struct ForwarderCleanup {
        forward_process: Option<Child>,
        local_socket_dir: Option<PathBuf>,
    }

    impl Drop for ForwarderCleanup {
        fn drop(&mut self) {
            if let Some(mut forward_process) = self.forward_process.take() {
                let _ = forward_process.kill();
                let _ = forward_process.wait();
            }
            if let Some(local_socket_dir) = self.local_socket_dir.take() {
                let _ = fs::remove_dir_all(local_socket_dir);
            }
        }
    }

    let mut cleanup = ForwarderCleanup {
        forward_process: Some(forward_process),
        local_socket_dir: Some(local_socket_dir),
    };

    let (control, raw) = wait_for_forwarded_sockets(
        &local_control,
        &local_raw,
        cleanup.forward_process.as_mut().expect("forwarder should exist"),
    )?;
    let client = LocalBeachheadClient::connect_streams(control, raw)?;
    let forward_process = cleanup.forward_process.take().expect("forwarder should exist");
    let local_socket_dir = cleanup.local_socket_dir.take().expect("socket dir should exist");
    Ok((
        client,
        RemoteBeachheadBridge {
            forward_process,
            local_socket_dir,
        },
    ))
}

fn probe_remote_host(target: &str) -> Result<RemoteHostInfo, String> {
    let output = Command::new("ssh")
        .arg(target)
        .arg("sh")
        .arg("-lc")
        .arg("printf '%s\\t%s\\t%s\\n' \"$(uname -s)\" \"$(uname -m)\" \"$HOME\"")
        .output()
        .map_err(|error| format!("failed to probe remote host: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "remote probe failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut parts = stdout.trim().split('\t');
    let os = parts.next().unwrap_or_default().to_string();
    let arch = parts.next().unwrap_or_default().to_string();
    let home = parts.next().unwrap_or_default().to_string();
    if os.is_empty() || arch.is_empty() || home.is_empty() {
        return Err("remote probe returned incomplete host info".into());
    }
    Ok(RemoteHostInfo { os, arch, home })
}

fn ensure_supported_remote(info: &RemoteHostInfo) -> Result<(), String> {
    if info.os != "Linux" {
        return Err(format!("remote beachhead currently supports Linux only, got {}", info.os));
    }
    let local_arch = std::env::consts::ARCH;
    if info.arch != local_arch {
        return Err(format!(
            "remote architecture {} does not match local exatermd build architecture {}",
            info.arch, local_arch
        ));
    }
    Ok(())
}

fn local_exatermd_path() -> Result<PathBuf, String> {
    let current_exe = std::env::current_exe()
        .map_err(|error| format!("failed to resolve current executable: {error}"))?;
    let candidate = current_exe.with_file_name("exatermd");
    if candidate.exists() {
        return Ok(candidate);
    }
    Err(format!(
        "missing sibling exatermd at {}; build it first with `make`",
        candidate.display()
    ))
}

fn ensure_remote_dirs(
    target: &str,
    remote_root: &str,
    remote_bin_dir: &str,
    remote_runtime_dir: &str,
) -> Result<(), String> {
    let script = format!(
        "set -eu; mkdir -p {} {} {}",
        shell_quote(remote_root),
        shell_quote(remote_bin_dir),
        shell_quote(remote_runtime_dir),
    );
    run_remote_shell(target, &script).map(|_| ())
}

fn upload_remote_exatermd(target: &str, local_exatermd: &Path, remote_bin: &str) -> Result<(), String> {
    let status = Command::new("scp")
        .arg("-q")
        .arg(local_exatermd)
        .arg(format!("{target}:{remote_bin}"))
        .status()
        .map_err(|error| format!("failed to upload remote exatermd: {error}"))?;
    if status.success() {
        Ok(())
    } else {
        Err("scp upload of remote exatermd failed".into())
    }
}

fn launch_remote_daemon(
    target: &str,
    remote_bin: &str,
    remote_runtime_dir: &str,
    remote_control: &str,
    remote_raw: &str,
) -> Result<(), String> {
    let mut exports = vec![format!(
        "export EXATERM_RUNTIME_DIR={}",
        shell_quote(remote_runtime_dir)
    )];
    for key in [
        "OPENAI_API_KEY",
        "EXATERM_OPENAI_BASE_URL",
        "OPENAI_BASE_URL",
        "EXATERM_SUMMARY_MODEL",
        "EXATERM_NAMING_MODEL",
        "EXATERM_NUDGE_MODEL",
    ] {
        if let Some(value) = std::env::var_os(key) {
            exports.push(format!(
                "export {key}={}",
                shell_quote_os(&value)
            ));
        }
    }

    let script = format!(
        "set -eu; chmod +x {bin}; {exports}; nohup {bin} >/dev/null 2>&1 < /dev/null & \
         i=0; while [ \"$i\" -lt 50 ]; do \
           if [ -S {control} ] && [ -S {raw} ]; then exit 0; fi; \
           i=$((i+1)); sleep 0.1; \
         done; \
         echo 'remote beachhead sockets did not appear' >&2; exit 1",
        bin = shell_quote(remote_bin),
        exports = exports.join("; "),
        control = shell_quote(remote_control),
        raw = shell_quote(remote_raw),
    );
    run_remote_shell(target, &script).map(|_| ())
}

fn run_remote_shell(target: &str, script: &str) -> Result<std::process::Output, String> {
    let output = Command::new("ssh")
        .arg(target)
        .arg("sh")
        .arg("-lc")
        .arg(script)
        .output()
        .map_err(|error| format!("failed to run remote command: {error}"))?;
    if output.status.success() {
        Ok(output)
    } else {
        Err(format!(
            "remote command failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ))
    }
}

fn wait_for_forwarded_sockets(
    local_control: &Path,
    local_raw: &Path,
    forward_process: &mut Child,
) -> Result<(UnixStream, UnixStream), String> {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Some(status) = forward_process
            .try_wait()
            .map_err(|error| format!("failed to poll SSH forwarder: {error}"))?
        {
            return Err(format!("SSH forwarder exited early with status {status}"));
        }
        match (UnixStream::connect(local_control), UnixStream::connect(local_raw)) {
            (Ok(control), Ok(raw)) => return Ok((control, raw)),
            _ if Instant::now() < deadline => thread::sleep(Duration::from_millis(50)),
            _ => {
                return Err("timed out waiting for forwarded remote beachhead sockets".into());
            }
        }
    }
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

fn shell_quote_os(value: &OsString) -> String {
    shell_quote(&value.to_string_lossy())
}

fn unique_local_socket_dir(label: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock should be after epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("exaterm-{label}-{nanos}"))
}

#[cfg(test)]
mod tests {
    use super::shell_quote;

    #[test]
    fn shell_quote_escapes_single_quotes() {
        assert_eq!(shell_quote("a'b"), "'a'\\''b'");
    }
}
