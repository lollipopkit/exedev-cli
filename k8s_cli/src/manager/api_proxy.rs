use super::state::random_token;
use anyhow::{Context, Result, bail};
use exedev_core::{client::ExeDevClient, shell};
use serde::Deserialize;
use std::{
    io::{Read, Write},
    net::{Shutdown, TcpListener, TcpStream},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle},
    time::Duration,
};

#[derive(Debug)]
pub(super) struct ApiProxy {
    url: String,
    token: String,
    port: u16,
    stop: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ApiRequest {
    fn_name: String,
    #[serde(default)]
    params: Vec<String>,
}

impl ApiProxy {
    pub(super) fn start(endpoint: String, api_key: String) -> Result<Self> {
        let listener = TcpListener::bind(("127.0.0.1", 0)).context("failed to bind API proxy")?;
        listener
            .set_nonblocking(false)
            .context("failed to configure API proxy listener")?;
        let port = listener
            .local_addr()
            .context("failed to read API proxy address")?
            .port();
        let token = random_token();
        let stop = Arc::new(AtomicBool::new(false));
        let thread_stop = Arc::clone(&stop);
        let thread_token = token.clone();
        let handle = thread::spawn(move || {
            run_api_proxy(listener, endpoint, api_key, thread_token, thread_stop);
        });
        Ok(Self {
            url: format!("http://127.0.0.1:{port}/exec"),
            token,
            port,
            stop,
            handle: Some(handle),
        })
    }

    pub(super) fn url(&self) -> &str {
        &self.url
    }

    pub(super) fn token(&self) -> &str {
        &self.token
    }

    pub(super) fn port(&self) -> u16 {
        self.port
    }
}

impl Drop for ApiProxy {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        let _ = TcpStream::connect(("127.0.0.1", self.port));
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

fn run_api_proxy(
    listener: TcpListener,
    endpoint: String,
    api_key: String,
    token: String,
    stop: Arc<AtomicBool>,
) {
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(err) => {
            eprintln!("failed to start exe.dev API proxy runtime: {err}");
            return;
        }
    };
    while !stop.load(Ordering::SeqCst) {
        match listener.accept() {
            Ok((mut stream, _)) => {
                if stop.load(Ordering::SeqCst) {
                    let _ = stream.shutdown(Shutdown::Both);
                    break;
                }
                let response =
                    handle_api_stream(&mut stream, &runtime, &endpoint, &api_key, &token)
                        .unwrap_or_else(|err| (500, format!("API proxy error: {err}")));
                let _ = write_http_response(&mut stream, response.0, &response.1);
            }
            Err(err) => {
                eprintln!("exe.dev API proxy accept failed: {err}");
                thread::sleep(Duration::from_millis(100));
            }
        }
    }
}

fn handle_api_stream(
    stream: &mut TcpStream,
    runtime: &tokio::runtime::Runtime,
    endpoint: &str,
    api_key: &str,
    token: &str,
) -> Result<(u16, String)> {
    let request = read_http_request(stream)?;
    if !authorization_matches(&request.headers, token) {
        return Ok((403, "missing or invalid API proxy token".into()));
    }
    let api_request: ApiRequest =
        serde_json::from_slice(&request.body).context("invalid API proxy JSON body")?;
    if api_request.fn_name.trim().is_empty() {
        return Ok((400, "fnName must not be empty".into()));
    }
    let mut words = vec![api_request.fn_name];
    words.extend(api_request.params);
    let command = shell::shell_join(&words);
    let client = ExeDevClient::new(endpoint.to_string(), api_key.to_string());
    match runtime.block_on(client.exec(&command)) {
        Ok(output) => Ok((200, output)),
        Err(err) => Ok((502, err.to_string())),
    }
}

fn authorization_matches(headers: &str, token: &str) -> bool {
    headers.lines().any(|line| {
        let line = line.trim();
        line.strip_prefix("Authorization: Bearer ")
            .or_else(|| line.strip_prefix("authorization: Bearer "))
            == Some(token)
    })
}

struct HttpRequest {
    headers: String,
    body: Vec<u8>,
}

fn read_http_request(stream: &mut TcpStream) -> Result<HttpRequest> {
    let mut buffer = Vec::new();
    let mut header_end = None;
    loop {
        let mut chunk = [0_u8; 1024];
        let read = stream
            .read(&mut chunk)
            .context("failed to read API proxy request")?;
        if read == 0 {
            break;
        }
        buffer.extend_from_slice(&chunk[..read]);
        if let Some(index) = find_header_end(&buffer) {
            header_end = Some(index);
            break;
        }
        if buffer.len() > 64 * 1024 {
            bail!("API proxy request headers are too large");
        }
    }
    let header_end = header_end.context("API proxy request did not include HTTP headers")?;
    let headers = String::from_utf8_lossy(&buffer[..header_end]).to_string();
    if !headers.starts_with("POST ") {
        bail!("API proxy only accepts POST requests");
    }
    let content_length = content_length(&headers)?;
    let body_start = header_end + 4;
    let mut body = buffer[body_start..].to_vec();
    while body.len() < content_length {
        let mut chunk = vec![0_u8; content_length - body.len()];
        let read = stream
            .read(&mut chunk)
            .context("failed to read API proxy body")?;
        if read == 0 {
            break;
        }
        body.extend_from_slice(&chunk[..read]);
    }
    body.truncate(content_length);
    Ok(HttpRequest { headers, body })
}

fn find_header_end(buffer: &[u8]) -> Option<usize> {
    buffer.windows(4).position(|window| window == b"\r\n\r\n")
}

fn content_length(headers: &str) -> Result<usize> {
    for line in headers.lines() {
        if let Some(value) = line
            .strip_prefix("Content-Length:")
            .or_else(|| line.strip_prefix("content-length:"))
        {
            return value
                .trim()
                .parse::<usize>()
                .context("invalid Content-Length");
        }
    }
    Ok(0)
}

fn write_http_response(stream: &mut TcpStream, status: u16, body: &str) -> Result<()> {
    let reason = match status {
        200 => "OK",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        500 => "Internal Server Error",
        502 => "Bad Gateway",
        _ => "OK",
    };
    write!(
        stream,
        "HTTP/1.1 {status} {reason}\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    )
    .context("failed to write API proxy response")
}
