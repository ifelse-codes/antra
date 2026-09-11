use anyhow::Result;
#[cfg(unix)]
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
#[cfg(windows)]
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use super::protocol::*;

/// Check if the daemon is running.
///
/// Connects to the socket instead of just stat-ing the path: a dead
/// daemon's socket file lingers after `kill -9`, and `exists()` alone
/// reported "already running" forever (stale-socket lie).
pub fn is_daemon_running() -> bool {
    #[cfg(unix)]
    {
        let sock_path = super::server::socket_path();
        if !sock_path.exists() {
            return false;
        }
        // Blocking connect is fast for a local socket; success = alive.
        // A bare connect is harmless: the server reads an empty line and
        // returns without touching state.
        std::os::unix::net::UnixStream::connect(&sock_path).is_ok()
    }
    #[cfg(windows)]
    {
        let pipe_name = super::server::pipe_path();
        std::path::Path::new(&pipe_name).exists()
    }
}

/// Remove a stale socket file left by a dead daemon (best-effort).
/// Returns true if a stale file was removed. Used by recovery paths.
#[cfg(unix)]
#[allow(dead_code)]
pub fn remove_stale_socket() -> bool {
    let sock_path = super::server::socket_path();
    if sock_path.exists() && !is_daemon_running() {
        std::fs::remove_file(&sock_path).is_ok()
    } else {
        false
    }
}

/// Send a message to the daemon and wait for a response
pub async fn send_command(payload: IpcPayload) -> Result<IpcMessage> {
    #[cfg(unix)]
    {
        use tokio::net::UnixStream;

        let sock_path = super::server::socket_path();

        if !sock_path.exists() {
            anyhow::bail!("Daemon not running. Start it with: antra proxy start");
        }

        let stream = UnixStream::connect(&sock_path).await.map_err(|e| {
            if e.kind() == std::io::ErrorKind::PermissionDenied {
                return anyhow::anyhow!(
                    "Cannot reach daemon at {} (permission denied — daemon is running as root; use `sudo antra proxy status` / `sudo antra proxy stop`, or stop it and restart unprivileged).",
                    sock_path.display()
                );
            }
            anyhow::anyhow!(
                "Cannot reach daemon at {} ({e}). It may have crashed leaving a stale socket — run `antra proxy stop` then `antra proxy start`.",
                sock_path.display()
            )
        })?;
        let (read_half, mut write_half) = stream.into_split();

        let msg = IpcMessage::new(payload);
        let json = serde_json::to_string(&msg)?;
        write_half.write_all(json.as_bytes()).await?;
        write_half.write_all(b"\n").await?;
        write_half.flush().await?;

        let mut reader = BufReader::new(read_half);
        let mut line = String::new();
        reader.read_line(&mut line).await?;

        if line.is_empty() {
            anyhow::bail!("Daemon closed connection without response");
        }

        let resp: IpcMessage = serde_json::from_str(line.trim())?;
        Ok(resp)
    }

    #[cfg(windows)]
    {
        use tokio::net::windows::named_pipe::ClientOptions;

        let pipe_name = super::server::pipe_path();
        let mut client = ClientOptions::new().open(&pipe_name)?;

        let msg = IpcMessage::new(payload);
        let json = serde_json::to_string(&msg)?;

        // AsyncWrite is implemented for NamedPipeClient (owned), takes &mut self
        client.write_all(json.as_bytes()).await?;
        client.write_all(b"\n").await?;

        // AsyncRead is implemented for NamedPipeClient (owned), takes &mut self
        let mut buf = vec![0u8; 4096];
        let n = client.read(&mut buf).await?;

        if n == 0 {
            anyhow::bail!("Daemon closed connection without response");
        }

        let line = String::from_utf8(buf[..n].to_vec())?;
        let resp: IpcMessage = serde_json::from_str(line.trim())?;
        Ok(resp)
    }
}

/// Synchronous wrapper for send_command (for use in non-async contexts)
pub fn send_command_sync(payload: IpcPayload) -> Result<IpcMessage> {
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(send_command(payload))
}

/// Send a command and check for errors, returning just the success message
pub fn send_command_ok(payload: IpcPayload) -> Result<String> {
    let resp = send_command_sync(payload)?;
    match resp.payload {
        IpcPayload::Ok(ok) => Ok(ok.message),
        IpcPayload::Error(err) => Err(anyhow::anyhow!("{}", err.message)),
        IpcPayload::RoutesList(list) => {
            let mut output = String::new();
            for route in &list.routes {
                output.push_str(&format!(
                    "{} → 127.0.0.1:{} (pid: {:?}, uptime: {}s)\n",
                    route.domain, route.port, route.pid, route.created_at_secs,
                ));
            }
            Ok(output)
        }
        IpcPayload::Status(status) => Ok(format!(
            "Daemon PID: {}\nUptime: {}s\nRoutes: {}\nSocket: {}",
            status.pid, status.uptime_secs, status.route_count, status.socket_path,
        )),
        IpcPayload::Pong => Ok("Pong".to_string()),
        other => Err(anyhow::anyhow!("Unexpected response: {other:?}")),
    }
}

/// Get the daemon startup status (which ports actually bound)
pub fn get_startup_status() -> Result<StartupStatus> {
    let resp = send_command_sync(IpcPayload::GetStartupStatus)?;
    match resp.payload {
        IpcPayload::StartupStatusResponse(status) => Ok(status),
        IpcPayload::Error(err) => Err(anyhow::anyhow!("{}", err.message)),
        other => Err(anyhow::anyhow!("Unexpected response: {other:?}")),
    }
}

/// Async version of get_startup_status for use inside tokio runtimes.
pub async fn get_startup_status_async() -> Result<StartupStatus> {
    let resp = send_command(IpcPayload::GetStartupStatus).await?;
    match resp.payload {
        IpcPayload::StartupStatusResponse(status) => Ok(status),
        IpcPayload::Error(err) => Err(anyhow::anyhow!("{}", err.message)),
        other => Err(anyhow::anyhow!("Unexpected response: {other:?}")),
    }
}
