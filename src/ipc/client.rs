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
        // On Windows, try to connect to the named pipe
        use std::ffi::OsStr;
        use std::os::windows::ffi::OsStrExt;

        let pipe_name = super::server::pipe_path();
        let wide: Vec<u16> = OsStr::new(&pipe_name)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();

        unsafe {
            let handle = windows_sys::Win32::Storage::FileSystem::CreateFileW(
                wide.as_ptr(),
                0, // GENERIC_READ
                0, // FILE_SHARE_NONE
                std::ptr::null_mut(),
                3, // OPEN_EXISTING
                0,
                0,
            );
            if handle != windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE {
                windows_sys::Win32::Foundation::CloseHandle(handle);
                true
            } else {
                false
            }
        }
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

        // Read response
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

        let client = ClientOptions::new().open(&pipe_name)?;

        let mut reader = BufReader::new(&client);
        let mut writer = client;

        let msg = IpcMessage::new(payload);
        let json = serde_json::to_string(&msg)?;
        writer.write_all(json.as_bytes()).await?;
        writer.write_all(b"\n").await?;
        writer.flush().await?;

        // Read response
        let mut line = String::new();
        reader.read_line(&mut line).await?;

        if line.is_empty() {
            anyhow::bail!("Daemon closed connection without response");
        }

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
            // Format routes as string
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
