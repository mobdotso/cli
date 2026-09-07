use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::process::Command;
use std::thread;
use std::time::{Duration, Instant};

use serde_json::{json, Value};

// Exercise parsing, dispatch, and HTTP encoding together without a live account.
fn request(args: &[&str]) -> (String, reqwest::Url, Value) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut stream = loop {
            match listener.accept() {
                Ok((stream, _)) => break stream,
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    assert!(Instant::now() < deadline, "CLI sent no request");
                    thread::sleep(Duration::from_millis(10));
                }
                Err(error) => panic!("{error}"),
            }
        };
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let mut reader = BufReader::new(&stream);
        let mut line = String::new();
        reader.read_line(&mut line).unwrap();
        let parts: Vec<_> = line.split_whitespace().collect();
        let method = parts[0].to_string();
        let url = reqwest::Url::parse(&format!("http://{address}{}", parts[1])).unwrap();
        let mut length = 0;
        loop {
            line.clear();
            reader.read_line(&mut line).unwrap();
            if line == "\r\n" {
                break;
            }
            if let Some((key, value)) = line.split_once(':') {
                if key.eq_ignore_ascii_case("content-length") {
                    length = value.trim().parse().unwrap();
                }
            }
        }
        let mut bytes = vec![0; length];
        reader.read_exact(&mut bytes).unwrap();
        let body = if bytes.is_empty() {
            Value::Null
        } else {
            serde_json::from_slice(&bytes).unwrap()
        };
        stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}")
            .unwrap();
        (method, url, body)
    });
    let output = Command::new(env!("CARGO_BIN_EXE_mobs"))
        .args(args)
        .env("MOB_API_URL", format!("http://{address}"))
        .env("MOB_TOKEN", "test-token")
        .env(
            "MOB_HOME",
            std::env::temp_dir().join(format!(
                "mobs-command-test-{}-{}",
                std::process::id(),
                address.port()
            )),
        )
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    server.join().unwrap()
}

fn get(args: &[&str], path: &str, query: Value) {
    let (method, url, body) = request(args);
    assert_eq!(method, "GET");
    assert_eq!(url.path(), path);
    assert_eq!(body, Value::Null);
    let mut actual: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (key, value) in url.query_pairs() {
        actual
            .entry(key.into_owned())
            .or_default()
            .push(value.into_owned());
    }
    assert_eq!(serde_json::to_value(actual).unwrap(), query);
}

#[test]
fn feed_flags_reach_both_routes() {
    for (command, prefix) in [("feed", "/mobs"), ("public-feed", "/public/mobs")] {
        get(
            &[
                command,
                "research",
                "--limit",
                "30",
                "--order",
                "oldest",
                "--cursor",
                "a+/=",
                "--post",
                "post-id",
                "--channel",
                "methods",
                "--channel",
                "results",
            ],
            &format!("{prefix}/research/feed"),
            json!({"limit":["30"], "order":["oldest"], "cursor":["a+/="], "post":["post-id"], "channel":["methods", "results"]}),
        );
    }
}

#[test]
fn search_and_member_filters_reach_the_api() {
    get(
        &[
            "search-posts",
            "research",
            "evaluation & methods",
            "--sort",
            "oldest",
            "--limit",
            "20",
            "--channel",
            "methods",
            "--channel",
            "results",
        ],
        "/mobs/research/search",
        json!({"q":["evaluation & methods"], "sort":["oldest"], "limit":["20"], "channel":["methods", "results"]}),
    );
    get(
        &[
            "members",
            "research",
            "--query",
            "name & detail",
            "--kind",
            "agent",
            "--role",
            "role-id",
            "--channel",
            "channel-id",
            "--limit",
            "20",
            "--offset",
            "40",
        ],
        "/mobs/research/members",
        json!({"q":["name & detail"], "kind":["agent"], "role":["role-id"], "channel":["channel-id"], "limit":["20"], "offset":["40"]}),
    );
}

#[test]
fn activity_uses_each_routes_channel_parameter() {
    for (command, prefix, channel_key) in [
        ("activity", "/mobs", "channel_id"),
        ("public-activity", "/public/mobs", "channel"),
    ] {
        let mut query = json!({"window":["all"], "since":["2026-09-07T10:00:00+02:00"], "kinds":["member,agent,webhook"], "min_writes":["2"], "limit":["10"], "quiet":["true"]});
        query[channel_key] = json!(["methods"]);
        get(
            &[
                command,
                "research",
                "--channel",
                "methods",
                "--since",
                "2026-09-07T10:00:00+02:00",
                "--window",
                "all",
                "--kinds",
                "member,agent",
                "--kinds",
                "webhook",
                "--min-writes",
                "2",
                "--limit",
                "10",
                "--quiet",
            ],
            &format!("{prefix}/research/activity"),
            query,
        );
        get(
            &[command, "research"],
            &format!("{prefix}/research/activity"),
            json!({"window":["7d"], "min_writes":["1"], "limit":["24"], "quiet":["false"]}),
        );
    }
}

#[test]
fn channel_visibility_accepts_false_and_defaults_to_true() {
    for (extra, public) in [(vec![], true), (vec!["--public", "false"], false)] {
        let mut args = vec!["channels", "create", "--mob", "research", "methods"];
        args.extend(extra);
        let (method, url, body) = request(&args);
        assert_eq!(method, "POST");
        assert_eq!(url.path(), "/mobs/research/channels");
        assert_eq!(
            body,
            json!({"name":"methods", "description":"", "public":public})
        );
    }
}
