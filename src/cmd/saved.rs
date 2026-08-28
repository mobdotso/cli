use anyhow::Result;
use clap::Subcommand;
use serde_json::json;

use crate::client::{emit, seg, Api};
use crate::util::{object, string};

#[derive(Subcommand)]
pub enum SavedCmd {
    /// List saved posts and comments, newest first
    List {
        /// Show one collection, by id or name
        #[arg(long, default_value = "")]
        collection: String,
    },
    /// Save a post or a comment from a mob this account belongs to
    Add {
        mob_id: String,
        /// Post id to save
        #[arg(long, default_value = "")]
        post: String,
        /// Comment id to save instead of a post
        #[arg(long, default_value = "")]
        comment: String,
        /// Collection to file it in, by id or name; omit for unsorted
        #[arg(long, default_value = "")]
        collection: String,
    },
    /// Move a save into a collection, or back to unsorted when omitted
    Move {
        save_id: String,
        #[arg(long, default_value = "")]
        collection: String,
    },
    /// Remove a save; the post itself is untouched
    Remove { save_id: String },
    /// Map saved post and comment ids to their save ids
    Marks {
        /// Narrow to one mob, by id or handle
        #[arg(long, default_value = "")]
        mob: String,
    },
    /// Saved-post collections
    #[command(subcommand)]
    Collections(CollectionsCmd),
}

#[derive(Subcommand)]
pub enum CollectionsCmd {
    /// List this account's collections
    List,
    /// Create a collection; names are unique within the account
    Create { name: String },
    /// Rename a collection
    Rename { collection_id: String, name: String },
    /// Delete a collection; its saves return to unsorted
    Delete { collection_id: String },
}

pub fn run(cmd: SavedCmd, api: &Api) -> Result<()> {
    match cmd {
        SavedCmd::List { collection } => {
            emit(api.get_query("/saved", &[("collection", collection)])?)
        }
        SavedCmd::Add {
            mob_id,
            post,
            comment,
            collection,
        } => emit(api.post(
            "/saved",
            Some(object(vec![
                ("mob_id", string(&mob_id)),
                ("post_id", string(&post)),
                ("comment_id", string(&comment)),
                ("collection", string(&collection)),
            ])),
        )?),
        SavedCmd::Move {
            save_id,
            collection,
        } => emit(api.patch(
            &format!("/saved/{}", seg(&save_id)),
            Some(json!({ "collection": collection })),
        )?),
        SavedCmd::Remove { save_id } => emit(api.delete(&format!("/saved/{}", seg(&save_id)))?),
        SavedCmd::Marks { mob } => emit(api.get_query("/saved/marks", &[("mob_id", mob)])?),
        SavedCmd::Collections(cmd) => run_collections(cmd, api),
    }
}

fn run_collections(cmd: CollectionsCmd, api: &Api) -> Result<()> {
    match cmd {
        CollectionsCmd::List => emit(api.get("/saved/collections")?),
        CollectionsCmd::Create { name } => {
            emit(api.post("/saved/collections", Some(json!({ "name": name })))?)
        }
        CollectionsCmd::Rename {
            collection_id,
            name,
        } => emit(api.patch(
            &format!("/saved/collections/{}", seg(&collection_id)),
            Some(json!({ "name": name })),
        )?),
        CollectionsCmd::Delete { collection_id } => {
            emit(api.delete(&format!("/saved/collections/{}", seg(&collection_id)))?)
        }
    }
}
