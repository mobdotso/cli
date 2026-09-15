use std::path::PathBuf;

use anyhow::Result;
use clap::Subcommand;
use reqwest::Method;
use serde_json::json;

use crate::client::{emit, seg, Api};
use crate::util::{strings, CursorArgs};

#[derive(Subcommand)]
pub enum PostsCmd {
    /// List a channel's posts
    List {
        #[arg(long)]
        mob: String,
        #[arg(long)]
        channel: String,
        #[command(flatten)]
        page: CursorArgs,
    },
    /// Create a post
    Create {
        #[arg(long)]
        mob: String,
        #[arg(long)]
        channel: String,
        #[arg(long, value_parser = post_title)]
        title: String,
        #[arg(long, default_value = "")]
        body: String,
        /// Attachment id from `mobs attachments upload` (repeatable)
        #[arg(long = "attachment")]
        attachments: Vec<String>,
    },
    /// Show a post with its comments
    Thread {
        #[arg(long)]
        mob: String,
        post_id: String,
        #[command(flatten)]
        page: CursorArgs,
        /// Include full comment bodies on this page
        #[arg(long)]
        full_comments: bool,
    },
    /// Read a public post with a page of comments
    PublicThread {
        #[arg(long)]
        mob: String,
        post_id: String,
        #[command(flatten)]
        page: CursorArgs,
        #[arg(long)]
        full_comments: bool,
    },
    /// Read a complete comment body
    ReadComment {
        #[arg(long)]
        mob: String,
        comment_id: String,
        /// Read from a public channel
        #[arg(long)]
        public: bool,
    },
    /// Read or change a post's link access
    Sharing {
        #[arg(long)]
        mob: String,
        post_id: String,
        /// Enable or revoke access for anyone with the link (requires posts.share)
        #[arg(long, action = clap::ArgAction::Set)]
        enabled: Option<bool>,
    },
    /// Read a post shared with anyone who has its link
    Shared {
        #[arg(long)]
        mob: String,
        post_id: String,
    },
    /// Show the like count and whether this account likes the post
    Likes {
        #[arg(long)]
        mob: String,
        post_id: String,
    },
    /// Like a post as this account
    Like {
        #[arg(long)]
        mob: String,
        post_id: String,
    },
    /// Remove this account's like
    Unlike {
        #[arg(long)]
        mob: String,
        post_id: String,
    },
    /// Delete a post
    Delete {
        #[arg(long)]
        mob: String,
        post_id: String,
    },
    /// Comment on a post
    Comment {
        #[arg(long)]
        mob: String,
        post_id: String,
        #[arg(long, default_value = "")]
        body: String,
        /// Attachment id from `mobs attachments upload` (repeatable)
        #[arg(long = "attachment")]
        attachments: Vec<String>,
    },
    /// Delete a comment
    DeleteComment {
        #[arg(long)]
        mob: String,
        comment_id: String,
    },
}

fn post_title(value: &str) -> Result<String, String> {
    let title = value.trim();
    if title.is_empty() || title.chars().count() > 200 {
        return Err("Title must contain 1 to 200 characters".to_string());
    }
    Ok(title.to_string())
}

pub fn run(cmd: PostsCmd, api: &Api) -> Result<()> {
    match cmd {
        PostsCmd::List { mob, channel, page } => emit(api.get_query(
            &format!("/mobs/{}/channels/{}/posts", seg(&mob), seg(&channel)),
            &page.query(),
        )?),
        PostsCmd::Create {
            mob,
            channel,
            title,
            body,
            attachments,
        } => emit(api.post(
            &format!("/mobs/{}/channels/{}/posts", seg(&mob), seg(&channel)),
            Some(json!({
                "title": title,
                "body": body,
                "attachment_ids": strings(&attachments),
            })),
        )?),
        PostsCmd::Thread {
            mob,
            post_id,
            page,
            full_comments,
        } => {
            let mut query = page.query();
            query.push(("summary", (!full_comments).to_string()));
            emit(api.get_query(
                &format!("/mobs/{}/posts/{}", seg(&mob), seg(&post_id)),
                &query,
            )?)
        }
        PostsCmd::PublicThread {
            mob,
            post_id,
            page,
            full_comments,
        } => {
            let mut query = page.query();
            query.push(("summary", (!full_comments).to_string()));
            emit(api.get_query(
                &format!("/public/mobs/{}/posts/{}", seg(&mob), seg(&post_id)),
                &query,
            )?)
        }
        PostsCmd::ReadComment {
            mob,
            comment_id,
            public,
        } => emit(api.get(&format!(
            "{}/mobs/{}/comments/{}",
            if public { "/public" } else { "" },
            seg(&mob),
            seg(&comment_id)
        ))?),
        PostsCmd::Sharing {
            mob,
            post_id,
            enabled,
        } => {
            let path = format!("/mobs/{}/posts/{}/sharing", seg(&mob), seg(&post_id));
            emit(match enabled {
                Some(enabled) => api.put(&path, Some(json!({"enabled": enabled})))?,
                None => api.get(&path)?,
            })
        }
        PostsCmd::Shared { mob, post_id } => emit(api.get(&format!(
            "/public/mobs/{}/posts/{}",
            seg(&mob),
            seg(&post_id)
        ))?),
        PostsCmd::Likes { mob, post_id } => emit(api.get(&format!(
            "/mobs/{}/posts/{}/likes",
            seg(&mob),
            seg(&post_id)
        ))?),
        PostsCmd::Like { mob, post_id } => emit(api.post(
            &format!("/mobs/{}/posts/{}/likes", seg(&mob), seg(&post_id)),
            None,
        )?),
        PostsCmd::Unlike { mob, post_id } => emit(api.delete(&format!(
            "/mobs/{}/posts/{}/likes",
            seg(&mob),
            seg(&post_id)
        ))?),
        PostsCmd::Delete { mob, post_id } => {
            emit(api.delete(&format!("/mobs/{}/posts/{}", seg(&mob), seg(&post_id)))?)
        }
        PostsCmd::Comment {
            mob,
            post_id,
            body,
            attachments,
        } => emit(api.post(
            &format!("/mobs/{}/posts/{}/comments", seg(&mob), seg(&post_id)),
            Some(json!({
                "body": body,
                "attachment_ids": strings(&attachments),
            })),
        )?),
        PostsCmd::DeleteComment { mob, comment_id } => emit(api.delete(&format!(
            "/mobs/{}/comments/{}",
            seg(&mob),
            seg(&comment_id)
        ))?),
    }
}

#[derive(Subcommand)]
pub enum AttachmentsCmd {
    /// Upload a file for use in a post or comment
    Upload {
        #[arg(long)]
        mob: String,
        file: PathBuf,
    },
    /// Download an attachment
    Download {
        #[arg(long)]
        mob: String,
        attachment_id: String,
        /// Write to this file instead of stdout
        #[arg(long, short = 'o')]
        output: Option<PathBuf>,
    },
}

pub fn run_attachments(cmd: AttachmentsCmd, api: &Api) -> Result<()> {
    match cmd {
        AttachmentsCmd::Upload { mob, file } => emit(api.upload(
            Method::POST,
            &format!("/mobs/{}/attachments", seg(&mob)),
            &file,
        )?),
        AttachmentsCmd::Download {
            mob,
            attachment_id,
            output,
        } => {
            let (bytes, content_type) = api.download(
                &format!("/mobs/{}/attachments/{}", seg(&mob), seg(&attachment_id)),
                &[],
            )?;
            crate::client::write_file(&bytes, &content_type, output)
        }
    }
}
