#![doc = include_str!("../README.md")]
// Opt-in to allowed-by-default rustc lints
// Reference: https://doc.rust-lang.org/rustc/lints/groups.html
#![warn(
    future_incompatible,
    let_underscore,
    missing_copy_implementations,
    missing_debug_implementations,
    missing_docs,
    // must_not_suspend, UNSTABLE: https://github.com/rust-lang/rust/issues/83310
    non_ascii_idents,
    nonstandard_style,
    noop_method_call,
    // unnameable_types, UNSTABLE: https://github.com/rust-lang/rust/issues/48054
    unreachable_pub,
    unused,
    unused_crate_dependencies,
    unused_lifetimes
)]
#![deny(
    // fuzzy_provenance_casts, UNSTABLE: https://github.com/rust-lang/rust/issues/95228
    // lossy_provenance_casts, UNSTABLE: https://github.com/rust-lang/rust/issues/95228
    unsafe_code // Exceptions must be discussed and deemed indispensable and use `#![deny(invalid_reference_casting, unsafe_op_in_unsafe_fn)]`.
)]
// Opt-in to allowed-by-default clippy lints
// Reference: https://rust-lang.github.io/rust-clippy/stable/
#![warn(clippy::pedantic, clippy::cargo)]
#![allow(clippy::multiple_crate_versions)] // Member of the `clippy::cargo` lint group.

use std::sync::Arc;

use color_eyre::{eyre::Result, Report};
use reqwest::Url;
use tracing::debug;

use crate::state::State;
use crate::ui::Ui;

mod args;
mod error;
mod extract;
mod process;
mod state;
mod trace;
mod ui;
mod util;

#[tokio::main]
async fn main() -> Result<()> {
    error::color_eyre_install()?;

    let args = args::parse();

    let (_appender_guard, _telemetry_guard) = trace::init(&args)?;

    let state = Arc::new(State::new(args.downloader, args.downloader_options));
    let ui = Ui::new();

    ui.event_loop(state.clone(), args.tick, async move {
        let url = Url::parse(&args.url)?;
        debug!("Parsed page URL: {url:#?}");

        if extract::player::is_player_url(&url) {
            extract::player::download_from_player(url, args.referer.as_deref(), state.clone())
                .await?;
        } else {
            extract::embeds::extract_and_download_embeds(url, state.clone()).await?;
        }

        state.set_stage_done().await;

        Ok::<(), Report>(())
    })
    .await?;

    Ok(())
}
