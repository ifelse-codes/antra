use std::net::TcpListener;

/// Find a free port by binding to port 0 and letting the OS assign one.
pub fn find_free_port() -> anyhow::Result<u16> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let port = listener.local_addr()?.port();
    drop(listener);
    Ok(port)
}
