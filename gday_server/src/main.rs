#![forbid(unsafe_code)]
#![warn(clippy::all)]
//! Runs a server for the [`gday_contact_exchange_protocol`].
//! Lets two users exchange their public and (optionally) private socket
//! addresses.

use clap::Parser;
use gday_server::Args;
use log::error;

#[tokio::main]
async fn main() {
    // read command line arguments
    let args = Args::parse();

    match gday_server::start_server(args) {
        Ok((_addr, handle)) => {
            handle.await;
            error!("Server crashed.");
        }
        Err(err) => {
            error!("{err}");
        }
    }
}
