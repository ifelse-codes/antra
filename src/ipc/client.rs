use anyhow::Result;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

use super::protocol::*;

/// Check if the daemon is running
pub fn is_daemon_running() -> bool {
    #[cfg(unix)]
    {
        let sock_path = super::server::socket_path();
        sock_path.exists()
    }
    #[cfg(windows)]
    {
        let pipe_name = super::server::pipe_path();
        std::path::Path::new(&pipe_name).exists()
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

        let stream = UnixStream::connect(&sock_path).await?;
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
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        let pipe_name = super::server::pipe_path();
        let client = ClientOptions::new().open(&pipe_name)?;

        let msg = IpcMessage::new(payload);
        let json = serde_json::to_string(&msg)?;

        // NamedPipeClient implements AsyncWrite for &NamedPipeClient
        // so we need &mut &client to call write_all (which takes &mut self)
        let mut writer = &client;
        writer.write_all(json.as_bytes()).await?;
        writer.write_all(b"\n").await?;

        // NamedPipeClient implements AsyncRead for &NamedPipeClient
        let mut reader = &client;
        let mut buf = vec![0u8; 4096];
        let n = reader.read(&mut buf).await?;

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
