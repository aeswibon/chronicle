use crate::{DaemonRequest, DaemonResponse};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;

pub struct Client {
    stream: UnixStream,
}

impl Client {
    pub async fn connect(path: &str) -> Result<Self, String> {
        let stream = UnixStream::connect(path)
            .await
            .map_err(|e| format!("UDS connect failed: {e}"))?;
        Ok(Self { stream })
    }

    pub async fn request(&mut self, req: DaemonRequest) -> Result<DaemonResponse, String> {
        let payload = serde_json::to_vec(&req).map_err(|e| e.to_string())?;
        let len = (payload.len() as u32).to_be_bytes();
        self.stream
            .write_all(&len)
            .await
            .map_err(|e| format!("write failed: {e}"))?;
        self.stream
            .write_all(&payload)
            .await
            .map_err(|e| format!("write failed: {e}"))?;

        let mut len_buf = [0u8; 4];
        self.stream
            .read_exact(&mut len_buf)
            .await
            .map_err(|e| format!("read failed: {e}"))?;
        let resp_len = u32::from_be_bytes(len_buf) as usize;
        let mut resp_buf = vec![0u8; resp_len];
        self.stream
            .read_exact(&mut resp_buf)
            .await
            .map_err(|e| format!("read failed: {e}"))?;
        serde_json::from_slice(&resp_buf).map_err(|e| e.to_string())
    }
}
