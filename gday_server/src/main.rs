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
        Ok((_addr, mut joinset)) => {
            joinset
                .join_next()
                .await
                .expect("No addresses provided.")
                .expect("Server thread panicked.");
        }
        Err(err) => {
            error!("{err}");
        }
    }
}
