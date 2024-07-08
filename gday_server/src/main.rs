//! Note: this crate is still in early-development, so expect breaking changes.
//!
//! Runs a server for the [`gday_contact_exchange_protocol`].
//! Lets two users exchange their public and (optionally) private socket addresses.
#![forbid(unsafe_code)]
#![warn(clippy::all)]

use clap::Parser;
use gday_server::Args;
use log::error;

#[tokio::main]
async fn main() {
    // read command line arguments
    let args = Args::parse();

    match gday_server::start_server(args) {
        Ok((server, _addr)) => server.await,
        Err(err) => error!("{err}"),
    }
}
