use std::io::Write as _;
use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Subcommand;

use crate::client::{emit, Api};

#[derive(Subcommand)]
pub enum FilesCmd {
    /// List every file in the account's file store
    List,
    /// Read one file from the file store
    Read {
        /// Path inside the store, e.g. shared/notes/plan.md
        path: String,
        /// Write to this file instead of stdout
        #[arg(long, short = 'o')]
        output: Option<PathBuf>,
    },
}

pub fn run(cmd: FilesCmd, api: &Api) -> Result<()> {
    match cmd {
        FilesCmd::List => emit(api.get("/files")?),
        FilesCmd::Read { path, output } => {
            let (bytes, content_type) = api.download("/files/content", &[("path", path)])?;
            write_file(&bytes, &content_type, output)
        }
    }
}

pub fn write_file(bytes: &[u8], content_type: &str, output: Option<PathBuf>) -> Result<()> {
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
