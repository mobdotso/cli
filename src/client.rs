use std::path::Path;
use std::time::Duration;

use anyhow::{bail, Context, Result};
use reqwest::blocking::multipart;
use reqwest::blocking::{Client, RequestBuilder};
use reqwest::Method;
use serde_json::Value;

use crate::config::Session;

/// Thin transport over the mob.so REST API. Every command builds a method,
/// a path, and an optional body; authorization and validation happen on the
/// server, never here.
pub struct Api {
    origin: String,
    token: Option<String>,
    http: Client,
}

impl Api {
    pub fn new(session: &Session) -> Result<Self> {
        let http = Client::builder()
            .timeout(Duration::from_secs(120))
            .user_agent(format!("mobs/{}", env!("CARGO_PKG_VERSION")))
            .build()
            .context("Could not construct the HTTP client")?;
        Ok(Self {
            origin: session.origin.clone(),
            token: session.token.clone(),
            http,
        })
    }

    pub fn origin(&self) -> &str {
        &self.origin
    }

    fn builder(&self, method: Method, path: &str, query: &[(&str, String)]) -> RequestBuilder {
        let url = format!("{}{}", self.origin, path);
        let mut builder = self.http.request(method, url);
        if !query.is_empty() {
            builder = builder.query(query);
        }
        if let Some(token) = &self.token {
            builder = builder.bearer_auth(token);
        }
        builder
    }

    /// Sends a request and returns the parsed JSON body, or None for 204.
    pub fn request(
        &self,
        method: Method,
        path: &str,
        query: &[(&str, String)],
        body: Option<Value>,
    ) -> Result<Option<Value>> {
        let mut builder = self.builder(method, path, query);
        if let Some(body) = body {
            builder = builder.json(&body);
        }
        let response = builder.send().context("The request could not be sent")?;
        Self::read_json(response)
    }

    pub fn get(&self, path: &str) -> Result<Option<Value>> {
        self.request(Method::GET, path, &[], None)
    }

    pub fn get_query(&self, path: &str, query: &[(&str, String)]) -> Result<Option<Value>> {
        self.request(Method::GET, path, query, None)
    }

    pub fn post(&self, path: &str, body: Option<Value>) -> Result<Option<Value>> {
        self.request(Method::POST, path, &[], body)
    }

    pub fn put(&self, path: &str, body: Option<Value>) -> Result<Option<Value>> {
        self.request(Method::PUT, path, &[], body)
    }

    pub fn patch(&self, path: &str, body: Option<Value>) -> Result<Option<Value>> {
        self.request(Method::PATCH, path, &[], body)
    }

    pub fn delete(&self, path: &str) -> Result<Option<Value>> {
        self.request(Method::DELETE, path, &[], None)
    }

    /// Uploads one file as multipart form data under the field name `file`,
    /// matching the FastAPI UploadFile endpoints.
    pub fn upload(&self, method: Method, path: &str, file: &Path) -> Result<Option<Value>> {
        let form = multipart::Form::new()
            .file("file", file)
            .with_context(|| format!("Could not read {}", file.display()))?;
        let response = self
            .builder(method, path, &[])
            .multipart(form)
            .send()
            .context("The upload could not be sent")?;
        Self::read_json(response)
    }

    /// Fetches raw bytes plus the response content type, for attachment
    /// and file downloads.
    pub fn download(&self, path: &str, query: &[(&str, String)]) -> Result<(Vec<u8>, String)> {
        let response = self
            .builder(Method::GET, path, query)
            .send()
            .context("The request could not be sent")?;
        let status = response.status();
        let content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .unwrap_or("application/octet-stream")
            .to_string();
        let bytes = response.bytes().context("The response could not be read")?;
        if !status.is_success() {
            bail!(
                "{}",
                extract_error(status.as_u16(), &String::from_utf8_lossy(&bytes))
            );
        }
        Ok((bytes.to_vec(), content_type))
    }

    fn read_json(response: reqwest::blocking::Response) -> Result<Option<Value>> {
        let status = response.status();
        let text = response.text().context("The response could not be read")?;
        if !status.is_success() {
            bail!("{}", extract_error(status.as_u16(), &text));
        }
        if text.trim().is_empty() {
            return Ok(None);
        }
        match serde_json::from_str::<Value>(&text) {
            Ok(value) => Ok(Some(value)),
            Err(_) => Ok(Some(Value::String(text))),
        }
    }
}

/// Pulls the API's error message out of a FastAPI error body:
/// {"detail": "..."} or {"detail": [{"msg": ...}]} for validation errors.
fn extract_error(status: u16, body: &str) -> String {
    let fallback = || {
        let trimmed = body.trim();
        if trimmed.is_empty() {
            format!("The API answered {status}")
        } else {
            format!("The API answered {status}: {trimmed}")
        }
    };
    let Ok(value) = serde_json::from_str::<Value>(body) else {
        return fallback();
    };
    match value.get("detail") {
        Some(Value::String(message)) => format!("{message} ({status})"),
        Some(Value::Array(items)) => {
            let messages: Vec<String> = items
                .iter()
                .filter_map(|item| {
                    let msg = item.get("msg")?.as_str()?;
                    let loc = item
                        .get("loc")
                        .and_then(|loc| loc.as_array())
                        .map(|parts| {
                            parts
                                .iter()
                                .map(|part| match part {
                                    Value::String(s) => s.clone(),
                                    other => other.to_string(),
                                })
                                .collect::<Vec<_>>()
                                .join(".")
                        })
                        .unwrap_or_default();
                    if loc.is_empty() {
                        Some(msg.to_string())
                    } else {
                        Some(format!("{loc}: {msg}"))
                    }
                })
                .collect();
            if messages.is_empty() {
                fallback()
            } else {
                format!("{} ({status})", messages.join("; "))
            }
        }
        _ => fallback(),
    }
}

/// Writes downloaded bytes to a file, or to stdout when no path is given.
pub fn write_file(
    bytes: &[u8],
    content_type: &str,
    output: Option<std::path::PathBuf>,
) -> Result<()> {
    use std::io::Write as _;
    match output {
        Some(path) => {
            std::fs::write(&path, bytes)
                .with_context(|| format!("Could not write {}", path.display()))?;
            eprintln!(
                "Wrote {} bytes ({content_type}) to {}",
                bytes.len(),
                path.display()
            );
        }
        None => {
            std::io::stdout()
                .write_all(bytes)
                .context("Could not write to stdout")?;
        }
    }
    Ok(())
}

/// Prints a JSON response, or `ok` for an empty (204) one.
pub fn emit(result: Option<Value>) -> Result<()> {
    match result {
        Some(value) => println!("{}", serde_json::to_string_pretty(&value)?),
        None => println!("ok"),
    }
    Ok(())
}

/// URL-encodes one path segment.
pub fn seg(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char)
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}
