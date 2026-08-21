use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hasher};
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::time::{Duration, Instant};

use anyhow::{bail, Context, Result};
use colored::Colorize;
use serde_json::Value;

use crate::client::Api;
use crate::config::{self, Session, StoredContext, DEFAULT_ORIGIN};

const LOGIN_TIMEOUT: Duration = Duration::from_secs(600);

pub struct LoginArgs {
    pub browserless: bool,
    pub token: Option<String>,
    pub name: Option<String>,
    pub origin: Option<String>,
}

pub fn login(args: LoginArgs) -> Result<()> {
    let origin = args
        .origin
        .or_else(|| std::env::var("MOB_API_URL").ok().filter(|o| !o.is_empty()))
        .unwrap_or_else(|| DEFAULT_ORIGIN.to_string());
    let origin = origin.trim_end_matches('/').to_string();

    let token = if let Some(token) = args.token {
        token
    } else if args.browserless {
        prompt_for_token()?
    } else {
        browser_flow(&origin)?
    };

    finish_login(&origin, token, args.name)
}

/// Verifies the credential against /auth/me and stores it as a context.
/// Agent client keys (mob_ag_*) land here too; the API reports their kind.
fn finish_login(origin: &str, token: String, name: Option<String>) -> Result<()> {
    let token = token.trim().to_string();
    if token.is_empty() {
        bail!("No credential was provided");
    }

    let session = Session {
        origin: origin.to_string(),
        token: Some(token.clone()),
        context_name: None,
    };
    let api = Api::new(&session)?;
    let me = api
        .get("/auth/me")?
        .context("The API returned an empty response for /auth/me")?;
    let handle = me
        .get("handle")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let kind = me
        .get("kind")
        .and_then(Value::as_str)
        .unwrap_or("user")
        .to_string();

    let context_name = name.unwrap_or_else(|| {
        if handle.is_empty() {
            "default".to_string()
        } else {
            handle.clone()
        }
    });

    let mut config = config::load()?;
    config.contexts.insert(
        context_name.clone(),
        StoredContext {
            token,
            kind: kind.clone(),
            handle: handle.clone(),
            origin: origin.to_string(),
        },
    );
    config.active = context_name.clone();
    config::save(&config)?;

    println!(
        "  {} Logged in as {} ({kind}) on {origin}",
        "✓".green(),
        format!("@{handle}").bold()
    );
    println!(
        "  {} Saved as context {} and made it active.",
        "✓".green(),
        context_name.bold()
    );
    Ok(())
}

fn prompt_for_token() -> Result<String> {
    eprintln!(
        "  {} Paste a service key ({}) or agent client key ({}).",
        "→".bold(),
        "mob_sk_*".bold(),
        "mob_ag_*".bold()
    );
    eprintln!(
        "    Service keys are issued on {}",
        format!("{DEFAULT_ORIGIN}/dashboard/connect")
            .bold()
            .underline()
    );
    eprint!("  Key: ");
    std::io::stderr().flush().ok();
    let mut line = String::new();
    std::io::stdin()
        .read_line(&mut line)
        .context("Could not read the key from stdin")?;
    Ok(line.trim().to_string())
}

/// Opens {origin}/cli-login in a browser and waits for the page to deliver
/// a freshly issued service key to a localhost callback. The pairing code
/// printed here also renders in the browser so the two can be matched.
fn browser_flow(origin: &str) -> Result<String> {
    let listener = TcpListener::bind("127.0.0.1:0").context("Could not open a localhost port")?;
    let port = listener.local_addr()?.port();
    let code = pairing_code();

    let url = format!("{origin}/cli-login?port={port}&code={code}");
    println!();
    println!("  {} Pairing code: {}", "→".bold(), code.bold());
    println!(
        "  {} Opening your browser to log in — finish there.",
        "→".bold()
    );
    println!("    {}", url.bold().underline());
    println!();
    println!("  If the browser does not open, visit the link yourself.");
    if let Err(error) = open_browser(&url) {
        eprintln!("  {} Could not open a browser: {error}", "!".yellow());
    }

    listener
        .set_nonblocking(true)
        .context("Could not configure the localhost listener")?;
    let deadline = Instant::now() + LOGIN_TIMEOUT;

    loop {
        match listener.accept() {
            Ok((stream, _)) => {
                if let Some(token) = handle_callback(stream, &code)? {
                    return Ok(token);
                }
            }
            Err(ref error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                if Instant::now() >= deadline {
                    bail!("Timed out waiting for the browser login. Run `mobs login --browserless` to paste a key instead.");
                }
                std::thread::sleep(Duration::from_millis(100));
            }
            Err(error) => return Err(error).context("The localhost listener failed"),
        }
    }
}

/// Reads one HTTP request off the socket. Returns the token when the request
/// is the expected callback; answers 404 and returns None for anything else
/// (favicon probes, mismatched codes).
fn handle_callback(stream: TcpStream, expected_code: &str) -> Result<Option<String>> {
    stream.set_read_timeout(Some(Duration::from_secs(5))).ok();
    let mut reader = BufReader::new(stream);
    let mut request_line = String::new();
    if reader.read_line(&mut request_line).is_err() {
        return Ok(None);
    }
    // Drain the headers so the browser sees a clean close.
    let mut header = String::new();
    while reader.read_line(&mut header).is_ok() {
        if header == "\r\n" || header == "\n" || header.is_empty() {
            break;
        }
        header.clear();
    }
    let mut stream = reader.into_inner();

    let path = request_line.split_whitespace().nth(1).unwrap_or("");
    let (route, query) = match path.split_once('?') {
        Some((route, query)) => (route, query),
        None => (path, ""),
    };

    if route != "/callback" {
        respond(
            &mut stream,
            404,
            "Not found",
            "The mobs CLI only answers its login callback here.",
        );
        return Ok(None);
    }

    let mut code = None;
    let mut token = None;
    for pair in query.split('&') {
        let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
        match key {
            "code" => code = Some(percent_decode(value)),
            "token" => token = Some(percent_decode(value)),
            _ => {}
        }
    }

    if code.as_deref() != Some(expected_code) {
        respond(
            &mut stream,
            403,
            "The pairing code did not match",
            "Return to the terminal and run mobs login again.",
        );
        return Ok(None);
    }
    let Some(token) = token.filter(|token| !token.is_empty()) else {
        respond(
            &mut stream,
            400,
            "The callback carried no key",
            "Return to the terminal and run mobs login again.",
        );
        return Ok(None);
    };

    respond(
        &mut stream,
        200,
        "You are logged in",
        "Return to the terminal. This tab can be closed.",
    );
    Ok(Some(token))
}

/// The mob.so glyph, inlined because this page is served from 127.0.0.1 and
/// loads no remote assets. `currentColor` picks up the scheme's ink.
const BRAND_GLYPH: &str = "<svg width=\"40\" height=\"40\" viewBox=\"0 0 40 40\" xmlns=\"http://www.w3.org/2000/svg\" aria-hidden=\"true\"><path fill-rule=\"evenodd\" clip-rule=\"evenodd\" d=\"M8 0H16V8H24V0H32V8H40V40H0V8H8V0ZM8 24C11.2032 21.3782 12.9713 21.3074 16 24V32H8V24ZM24 24C27.2449 21.3941 28.9219 21.4813 32 24V32H24V24Z\" fill=\"currentColor\"/></svg>";

/// Mirrors the mob.so console palette: chalk canvas, white card, dark ink,
/// with the dark scheme following the browser preference.
const PAGE_STYLE: &str = "\
:root{color-scheme:light dark;--bg:#f6f4ee;--card:#ffffff;--ink:#17181a;--muted:#5f6670;--line:#deddd7;--shadow:0 12px 32px rgba(13,13,16,.07)}\
@media (prefers-color-scheme:dark){:root{--bg:#141519;--card:#1e2025;--ink:#f4f5f7;--muted:#a6adb7;--line:#33363d;--shadow:0 12px 32px rgba(0,0,0,.5)}}\
body{margin:0;min-height:100dvh;display:grid;place-items:center;background:var(--bg);color:var(--ink);font-family:-apple-system,BlinkMacSystemFont,'Segoe UI',Helvetica,Arial,sans-serif}\
main{box-sizing:border-box;display:grid;gap:28px;justify-items:center;width:100%;max-width:28rem;padding:20px 20px 48px}\
.brand{display:flex;align-items:center;gap:12px;font-size:20px;font-weight:700;letter-spacing:-.04em}\
.brand svg{display:block}\
.card{box-sizing:border-box;width:100%;padding:20px;background:var(--card);border:1px solid var(--line);border-radius:12px;box-shadow:var(--shadow)}\
h1{margin:0;font-size:16px;font-weight:700;letter-spacing:-.01em}\
p{margin:8px 0 0;font-size:14px;line-height:1.55;color:var(--muted)}";

fn respond(stream: &mut TcpStream, status: u16, title: &str, message: &str) {
    let reason = match status {
        200 => "OK",
        400 => "Bad Request",
        403 => "Forbidden",
        _ => "Not Found",
    };
    let body = format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width, initial-scale=1\"><title>{title} | mob.so</title><style>{PAGE_STYLE}</style></head><body><main><div class=\"brand\">{BRAND_GLYPH}<span>mob.so</span></div><div class=\"card\"><h1>{title}</h1><p>{message}</p></div></main></body></html>"
    );
    let response = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(response.as_bytes()).ok();
    stream.flush().ok();
}

fn percent_decode(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' if i + 2 < bytes.len() => {
                if let Ok(byte) = u8::from_str_radix(&value[i + 1..i + 3], 16) {
                    out.push(byte);
                    i += 3;
                } else {
                    out.push(b'%');
                    i += 1;
                }
            }
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            byte => {
                out.push(byte);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Random pairing code from the OS-seeded SipHash state; it matches the
/// browser tab to this process, while the key itself only ever travels to
/// 127.0.0.1.
fn pairing_code() -> String {
    let mut out = String::new();
    for _ in 0..2 {
        let mut hasher = RandomState::new().build_hasher();
        hasher.write_u64(std::process::id() as u64);
        out.push_str(&format!("{:016x}", hasher.finish()));
    }
    out
}

fn open_browser(url: &str) -> Result<()> {
    #[cfg(target_os = "macos")]
    let mut command = {
        let mut command = std::process::Command::new("open");
        command.arg(url);
        command
    };
    #[cfg(target_os = "windows")]
    let mut command = {
        let mut command = std::process::Command::new("cmd");
        command.args(["/C", "start", "", url]);
        command
    };
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let mut command = {
        let mut command = std::process::Command::new("xdg-open");
        command.arg(url);
        command
    };
    let status = command
        .status()
        .context("The browser opener was not found")?;
    if !status.success() {
        bail!("The browser opener exited with {status}");
    }
    Ok(())
}
