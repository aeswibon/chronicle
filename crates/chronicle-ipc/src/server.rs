use crate::{DaemonRequest, DaemonResponse};
use std::path::Path;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixListener;

pub struct Server {
    listener: UnixListener,
}

impl Server {
    pub fn bind(path: &str) -> Result<Self, String> {
        if Path::new(path).exists() && socket_is_live(path) {
            return Err(format!(
                "daemon already listening on {path} — stop the running instance first"
            ));
        }
        if std::fs::metadata(path).is_ok() {
            std::fs::remove_file(path).map_err(|e| format!("remove socket failed: {e}"))?;
        }
        let listener =
            UnixListener::bind(path).map_err(|e| format!("bind failed: {e}"))?;
        Ok(Self { listener })
    }

    pub async fn accept(&self) -> Result<Connection, String> {
        let (stream, _) = self
            .listener
            .accept()
            .await
            .map_err(|e| format!("accept failed: {e}"))?;
        Ok(Connection { stream })
    }
}

pub struct Connection {
    stream: tokio::net::UnixStream,
}

impl Connection {
    pub async fn read_request(&mut self) -> Result<DaemonRequest, String> {
        let mut len_buf = [0u8; 4];
        self.stream
            .read_exact(&mut len_buf)
            .await
            .map_err(|e| format!("read failed: {e}"))?;
        let req_len = u32::from_be_bytes(len_buf) as usize;
        let mut req_buf = vec![0u8; req_len];
        self.stream
            .read_exact(&mut req_buf)
            .await
            .map_err(|e| format!("read failed: {e}"))?;
        serde_json::from_slice(&req_buf).map_err(|e| e.to_string())
    }

    pub async fn send_response(&mut self, resp: DaemonResponse) -> Result<(), String> {
        let payload = serde_json::to_vec(&resp).map_err(|e| e.to_string())?;
        let len = (payload.len() as u32).to_be_bytes();
        self.stream
            .write_all(&len)
            .await
            .map_err(|e| format!("write failed: {e}"))?;
        self.stream
            .write_all(&payload)
            .await
            .map_err(|e| format!("write failed: {e}"))?;
        Ok(())
    }
}

fn socket_is_live(path: &str) -> bool {
    use std::io::{Read, Write};
    use std::os::unix::net::UnixStream;
    use std::time::Duration;

    let Ok(mut stream) = UnixStream::connect(path) else {
        return false;
    };
    let _ = stream.set_read_timeout(Some(Duration::from_millis(500)));
    let _ = stream.set_write_timeout(Some(Duration::from_millis(500)));

    let req = br#"{"type":"get_status"}"#;
    let len = (req.len() as u32).to_be_bytes();
    if stream.write_all(&len).is_err() || stream.write_all(req).is_err() {
        return false;
    }

    let mut len_buf = [0u8; 4];
    if stream.read_exact(&mut len_buf).is_err() {
        return false;
    }
    let resp_len = u32::from_be_bytes(len_buf) as usize;
    if resp_len == 0 || resp_len > 1_048_576 {
        return false;
    }
    let mut resp_buf = vec![0u8; resp_len];
    stream.read_exact(&mut resp_buf).is_ok()
}
