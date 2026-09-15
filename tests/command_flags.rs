use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use serde_json::{json, Value};

// Exercise parsing, dispatch, and HTTP encoding together without a live account.
fn request(args: &[&str]) -> (String, reqwest::Url, Value) {
    request_with_stdin(args, None)
}

fn request_with_stdin(args: &[&str], input: Option<&str>) -> (String, reqwest::Url, Value) {
    let (method, url, body, _) = request_with_response(
        args,
        input,
        b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}",
    );
    (method, url, body)
}

fn request_with_response(
    args: &[&str],
    input: Option<&str>,
    response: &'static [u8],
) -> (String, reqwest::Url, Value, std::process::Output) {
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
        stream.set_nonblocking(false).unwrap();
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
        stream.write_all(response).unwrap();
        (method, url, body)
    });
    let mut child = Command::new(env!("CARGO_BIN_EXE_mobs"))
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
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    if let Some(input) = input {
        child
            .stdin
            .as_mut()
            .unwrap()
            .write_all(input.as_bytes())
            .unwrap();
    }
    drop(child.stdin.take());
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let (method, url, body) = server.join().unwrap();
    (method, url, body, output)
}

#[test]
fn post_creation_requires_a_nonblank_title() {
    for title in [None, Some(""), Some(" \t\n"), Some("\u{2003}\u{00a0}")] {
        let mut command = Command::new(env!("CARGO_BIN_EXE_mobs"));
        command.args([
            "posts",
            "create",
            "--mob",
            "research",
            "--channel",
            "general",
            "--body",
            "Finding",
        ]);
        if let Some(title) = title {
            command.args(["--title", title]);
        }
        let output = command.output().unwrap();
        assert_eq!(output.status.code(), Some(2));
        assert!(String::from_utf8_lossy(&output.stderr).contains("--title"));
    }
    let (method, url, body) = request(&[
        "posts",
        "create",
        "--mob",
        "research",
        "--channel",
        "general",
        "--title",
        "  Finding  ",
        "--attachment",
        "file-id",
    ]);
    assert_eq!(method, "POST");
    assert_eq!(url.path(), "/mobs/research/channels/general/posts");
    assert_eq!(
        body,
        json!({"title": "Finding", "body": "", "attachment_ids": ["file-id"]})
    );
}

#[test]
fn mob_archive_and_restore_use_lifecycle_routes() {
    for action in ["archive", "restore"] {
        let (method, url, body) = request(&[action, "mob-id"]);
        assert_eq!(method, "POST");
        assert_eq!(url.path(), format!("/mobs/mob-id/{action}"));
        assert_eq!(body, Value::Null);
    }
}

#[test]
fn inline_connection_requests_use_stable_ids_and_secret_domains() {
    let (method, url, _) = request(&["connection-requests", "get", "request-id", "--by-id"]);
    assert_eq!(method, "GET");
    assert_eq!(url.path(), "/connection-requests/request-id/form");
    let (method, url, body) = request_with_stdin(
        &[
            "connection-requests",
            "secret",
            "request-id",
            "--by-id",
            "--name",
            "TEST_KEY",
            "--domain",
            "api.example.com",
        ],
        Some("test-only-value\n"),
    );
    assert_eq!(method, "POST");
    assert_eq!(url.path(), "/connection-requests/request-id/form/secret");
    assert_eq!(
        body,
        json!({"name": "TEST_KEY", "value": "test-only-value", "allowed_domains": ["api.example.com"]})
    );
}

#[test]
fn trace_downloads_preserve_json_lines_for_an_agent_or_run() {
    for run in [None, Some("run id")] {
        let mut args = vec!["agents", "runs", "download-traces", "agent id"];
        let mut path = "/agents/agent%20id".to_string();
        if let Some(run_id) = run {
            args.extend(["--run", run_id]);
            path.push_str("/runs/run%20id");
        }
        path.push_str("/traces/download");
        let (method, url, body, output) = request_with_response(
            &args,
            None,
            b"HTTP/1.1 200 OK\r\nContent-Type: application/x-ndjson\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n9\r\n{\"id\":1}\n\r\n9\r\n{\"id\":2}\n\r\n0\r\n\r\n",
        );
        assert_eq!(method, "GET");
        assert_eq!(url.path(), path);
        assert_eq!(body, Value::Null);
        assert_eq!(output.stdout, b"{\"id\":1}\n{\"id\":2}\n");
    }
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
fn connection_create_authorizes_the_account() {
    let (method, url, body) = request(&[
        "connection-requests",
        "create",
        "--provider",
        "mcp",
        "--server-url",
        "https://example.com/mcp",
        "--name",
        "Team tools",
    ]);
    assert_eq!(method, "POST");
    assert_eq!(url.path(), "/connection-requests");
    assert_eq!(
        body,
        json!({
            "provider": "mcp", "server_url": "https://example.com/mcp", "name": "Team tools",
        })
    );
}

#[test]
fn connection_start_sends_oauth_app_credentials() {
    for confidential in [false, true] {
        let mut args = vec![
            "connection-requests",
            "start",
            "link-token",
            "--client-id",
            "app-id",
        ];
        if confidential {
            args.push("--client-secret-stdin");
        }
        let (method, url, body) = request_with_stdin(&args, confidential.then_some("app-secret\n"));
        assert_eq!(method, "POST");
        assert_eq!(url.path(), "/connection-requests/link-token/start");
        assert_eq!(
            body,
            json!({
                "client_id": "app-id",
                "client_secret": if confidential { "app-secret" } else { "" },
            })
        );
    }
}

#[test]
fn connection_start_sends_api_token_and_username() {
    let (method, url, body) = request_with_stdin(
        &[
            "connection-requests",
            "start",
            "link-token",
            "--api-key-stdin",
            "--api-key-username",
            "owner@example.com",
        ],
        Some("test-api-token\n"),
    );
    assert_eq!(method, "POST");
    assert_eq!(url.path(), "/connection-requests/link-token/start");
    assert_eq!(
        body,
        json!({
            "api_key": "test-api-token", "api_key_username": "owner@example.com",
        })
    );
}

#[test]
fn secret_grants_require_domains_and_send_each_hostname() {
    let args = [
        "agents",
        "runtime",
        "secrets",
        "grant",
        "agent-id",
        "--secret-id",
        "secret-id",
    ];
    let output = Command::new(env!("CARGO_BIN_EXE_mobs"))
        .args(args)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("--domain"));

    let mut scoped = args.to_vec();
    scoped.extend([
        "--domain",
        "api.example.com",
        "--domain",
        "files.example.com",
    ]);
    let (method, url, body) = request(&scoped);
    assert_eq!(method, "POST");
    assert_eq!(url.path(), "/agents/agent-id/runtime/secrets");
    assert_eq!(
        body,
        json!({
            "secret_id": "secret-id",
            "allowed_domains": ["api.example.com", "files.example.com"],
        })
    );
}

#[test]
fn account_commands_reach_the_account_routes() {
    get(&["status"], "/account", json!({}));
    get(&["account", "get"], "/account", json!({}));
    get(&["account", "connections"], "/connections", json!({}));

    let (method, url, body) = request(&["account", "update", "--description", "Research"]);
    assert_eq!(method, "PATCH");
    assert_eq!(url.path(), "/account");
    assert_eq!(body, json!({"description": "Research"}));

    let (method, url, body) = request(&["account", "set-handle", "researcher"]);
    assert_eq!(method, "PUT");
    assert_eq!(url.path(), "/account/handle");
    assert_eq!(body, json!({"handle": "researcher"}));

    let (method, url, _) = request(&["account", "unlink", "github", "provider-id"]);
    assert_eq!(method, "DELETE");
    assert_eq!(url.path(), "/account/identities/github/provider-id");
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
        &["channels", "search"],
        "/channels/search",
        json!({"query":[""], "limit":["25"], "offset":["0"]}),
    );
    get(
        &[
            "channels",
            "search",
            "evaluation & methods",
            "--mob",
            "research",
            "--limit",
            "10",
            "--offset",
            "20",
        ],
        "/channels/search",
        json!({"query":["evaluation & methods"], "mob_id":["research"], "limit":["10"], "offset":["20"]}),
    );
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

#[test]
fn invitations_use_the_mob_default_role() {
    let (method, url, body) = request(&["invites", "create", "--mob", "research", "researcher"]);
    assert_eq!(method, "POST");
    assert_eq!(url.path(), "/mobs/research/invites");
    assert_eq!(body, json!({"handle": "researcher"}));
    for command in ["create", "replace"] {
        let mut args = vec!["invites", "links", command, "--mob", "research"];
        let suffix = if command == "replace" {
            args.push("link-id");
            "/link-id/replace"
        } else {
            ""
        };
        let (method, url, body) = request(&args);
        assert_eq!(method, "POST");
        assert_eq!(url.path(), format!("/mobs/research/invite-links{suffix}"));
        assert_eq!(body, json!({}));
    }
}

#[test]
fn content_and_run_pages_forward_cursors_and_full_reads() {
    for (args, path, query) in [
        (
            vec![
                "posts",
                "list",
                "--mob",
                "research",
                "--channel",
                "general",
                "--cursor",
                "older",
            ],
            "/mobs/research/channels/general/posts",
            json!({"limit":["10"], "cursor":["older"]}),
        ),
        (
            vec![
                "posts",
                "thread",
                "--mob",
                "research",
                "post-id",
                "--limit",
                "2",
                "--cursor",
                "next",
                "--full-comments",
            ],
            "/mobs/research/posts/post-id",
            json!({"limit":["2"], "cursor":["next"], "summary":["false"]}),
        ),
        (
            vec!["posts", "public-thread", "--mob", "research", "post-id"],
            "/public/mobs/research/posts/post-id",
            json!({"limit":["10"], "cursor":[""], "summary":["true"]}),
        ),
        (
            vec![
                "posts",
                "read-comment",
                "--mob",
                "research",
                "comment-id",
                "--public",
            ],
            "/public/mobs/research/comments/comment-id",
            json!({}),
        ),
        (
            vec!["agents", "runs", "list", "agent-id", "--cursor", "older"],
            "/agents/agent-id/runs",
            json!({"limit":["10"], "cursor":["older"]}),
        ),
        (
            vec![
                "agents", "runs", "get", "agent-id", "run-id", "--cursor", "later",
            ],
            "/agents/agent-id/runs/run-id",
            json!({"limit":["10"], "cursor":["later"]}),
        ),
    ] {
        get(&args, path, query);
    }
}

#[test]
fn notification_commands_encode_preferences_and_owner_messages() {
    let (method, url, body) = request(&["notifications", "research", "--level", "all"]);
    assert_eq!(method, "PATCH");
    assert_eq!(url.path(), "/mobs/research/notifications");
    assert_eq!(body, json!({ "notification_level": "all" }));
    let (method, url, _) = request(&["notifications", "research"]);
    assert_eq!(method, "GET");
    assert_eq!(url.path(), "/mobs/research/notifications");
    let (method, url, body) = request(&["notify-owner", "Research is ready"]);
    assert_eq!(method, "POST");
    assert_eq!(url.path(), "/runtime/notify-owner");
    assert_eq!(body, json!({ "body": "Research is ready" }));
}

#[test]
fn codelens_uses_repository_and_connection_filters() {
    get(
        &["codelens", "repositories"],
        "/codelens/repositories",
        json!({}),
    );
    get(
        &[
            "codelens",
            "search",
            "lookup",
            "--repository",
            "acme/tools",
            "--connection",
            "connection-id",
            "--limit",
            "5",
        ],
        "/codelens/search",
        json!({"q":["lookup"], "repository":["acme/tools"], "connection_id":["connection-id"], "limit":["5"]}),
    );
    get(
        &[
            "codelens",
            "read",
            "acme/tools",
            "src/main.py",
            "--connection",
            "connection-id",
            "--start-line",
            "2",
            "--end-line",
            "8",
        ],
        "/codelens/file",
        json!({"repository":["acme/tools"], "path":["src/main.py"], "connection_id":["connection-id"], "start_line":["2"], "end_line":["8"]}),
    );
    let (method, url, body) = request(&[
        "codelens",
        "index",
        "acme/tools",
        "--connection",
        "connection-id",
    ]);
    assert_eq!(method, "POST");
    assert_eq!(url.path(), "/codelens/repositories");
    assert_eq!(
        body,
        json!({"repository":"acme/tools", "connection_id":"connection-id"})
    );
}

#[test]
fn connected_service_commands_use_agent_routes() {
    get(&["connections", "list"], "/runtime/connections", json!({}));
    for command in ["tools", "repositories"] {
        get(
            &["connections", command, "connection-id"],
            &format!("/runtime/connections/connection-id/{command}"),
            json!({}),
        );
    }
    get(
        &[
            "connections",
            "resource",
            "connection-id",
            "ui://service/app",
        ],
        "/runtime/connections/connection-id/resources",
        json!({"uri":["ui://service/app"]}),
    );
    let (method, url, body) = request(&[
        "connections",
        "call",
        "connection-id",
        "report",
        "--arguments",
        "{\"query\":\"sales\"}",
    ]);
    assert_eq!(
        (method.as_str(), url.path()),
        ("POST", "/runtime/connections/connection-id/tools/report")
    );
    assert_eq!(body, json!({"arguments":{"query":"sales"}}));
    let (method, url, body) =
        request(&["connections", "request", "connection-id", "GET", "/profile"]);
    assert_eq!(
        (method.as_str(), url.path()),
        ("POST", "/runtime/connections/connection-id/request")
    );
    assert_eq!(
        body,
        json!({"method":"GET", "path":"/profile", "query":null, "body":null})
    );
}

#[test]
fn account_resource_deletion_uses_owner_routes() {
    for (group, id) in [("secrets", "secret-id"), ("connections", "connection-id")] {
        let (method, url, _) = request(&[group, "delete", id]);
        assert_eq!(method, "DELETE");
        assert_eq!(url.path(), format!("/{group}/{id}"));
    }
}
