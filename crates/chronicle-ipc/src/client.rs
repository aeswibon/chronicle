use crate::{DaemonRequest, DaemonResponse};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;

pub struct Client {
    stream: UnixStream,
}

pub struct EventStream {
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
        write_request(&mut self.stream, &req).await?;
        read_response(&mut self.stream).await
    }

    pub async fn subscribe(mut self, event_types: Vec<String>) -> Result<EventStream, String> {
        write_request(
            &mut self.stream,
            &DaemonRequest::Subscribe { event_types },
        )
        .await?;
        Ok(EventStream {
            stream: self.stream,
        })
    }
}

impl EventStream {
    pub async fn next(&mut self) -> Result<DaemonResponse, String> {
        read_response(&mut self.stream).await
    }
}

async fn write_request(stream: &mut UnixStream, req: &DaemonRequest) -> Result<(), String> {
    let payload = serde_json::to_vec(req).map_err(|e| e.to_string())?;
    let len = (payload.len() as u32).to_be_bytes();
    stream
        .write_all(&len)
        .await
        .map_err(|e| format!("write failed: {e}"))?;
    stream
        .write_all(&payload)
        .await
        .map_err(|e| format!("write failed: {e}"))?;
    Ok(())
}

async fn read_response(stream: &mut UnixStream) -> Result<DaemonResponse, String> {
    let mut len_buf = [0u8; 4];
    stream
        .read_exact(&mut len_buf)
        .await
        .map_err(|e| format!("read failed: {e}"))?;
    let resp_len = u32::from_be_bytes(len_buf) as usize;
    if resp_len > 16 * 1024 * 1024 {
        return Err("response too large".into());
    }
    let mut resp_buf = vec![0u8; resp_len];
    stream
        .read_exact(&mut resp_buf)
        .await
        .map_err(|e| format!("read failed: {e}"))?;
    serde_json::from_slice(&resp_buf).map_err(|e| e.to_string())
}
