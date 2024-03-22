#![forbid(unsafe_code)]
#![warn(clippy::all)]
//! `gday` is a command line line tool for direct file transfer between peers.
//! TODO

mod base32;
mod file;
mod protocol;
mod transfer;

use base32::PeerCode;
use clap::{Parser, Subcommand};
use gday_hole_punch::{
    server_connector::{self, DEFAULT_SERVERS},
    ContactSharer,
};
use log::{error, info};
use owo_colors::OwoColorize;
use rand::Rng;
use std::path::PathBuf;

/// Send files directly to peers
#[derive(Parser, Debug)]
#[command(author, version, about)]
struct Args {
    #[command(subcommand)]
    operation: Command,

    /// Use a custom Gday server with this domain name
    #[arg(short, long)]
    server: Option<String>,

    // Use a custom room code
    #[arg(short, long)]
    room: Option<u64>,

    // Use a custom shared secret
    #[arg(short, long)]
    secret: Option<u64>,

    /// Verbosity. (trace, debug, info, warn, error)
    #[arg(short, long, default_value = "warn")]
    verbosity: log::LevelFilter,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Send files
    Send { paths: Vec<PathBuf> },

    /// Receive files
    Receive { code: String },
}

fn main() {
    let args = Args::parse();

    // initialize logging
    env_logger::builder()
        .format_level(false)
        .format_module_path(false)
        .format_target(false)
        .format_timestamp(None)
        .filter_level(args.verbosity)
        .init();

    if let Err(err) = run(args) {
        error!("{}", err);
    }
}

fn run(args: Args) -> Result<(), Box<dyn std::error::Error>> {
    // use custom server if the user provided one,
    // otherwise pick a random default server
    let (server_connection, server_id) = if let Some(domain_name) = args.server {
        (server_connector::connect_to_domain_name(&domain_name)?, 0)
    } else {
        server_connector::connect_to_random_server(DEFAULT_SERVERS)?
    };

    match args.operation {
        Command::Send { paths } => {
            let files = file::confirm_send(&paths)?;

            // generate random `room_code` and `shared_secret`
            // if the user didn't provide custom ones
            let room_code = if let Some(code) = args.room {
                code
            } else {
                rand::thread_rng().gen_range(0..u32::MAX as u64)
            };
            let shared_secret = if let Some(secret) = args.room {
                secret
            } else {
                rand::thread_rng().gen_range(0..u32::MAX as u64)
            };

            let (contact_sharer, my_contact) =
                ContactSharer::create_room(room_code, server_connection)?;
            let peer_code = PeerCode {
                server_id,
                room_code,
                shared_secret,
            };
            info!("Your contact is: {:?}", my_contact);

            println!(
                "Tell your mate to run \"gday receive {}\"",
                peer_code.to_str().bold()
            );
            let peer_contact = contact_sharer.get_peer_contact()?;
            let (tcp_stream, shared_key) = gday_hole_punch::try_connect_to_peer(
                my_contact.private,
                peer_contact,
                &shared_secret.to_be_bytes(),
            )?;
        }
        Command::Receive { code } => {
            let code = PeerCode::from_str(&code)?;
            let (contact_sharer, local_contact) =
                ContactSharer::join_room(code.room_code, server_connection)?;
            let peer_contact = contact_sharer.get_peer_contact()?;
            let (tcp_stream, shared_key) = gday_hole_punch::try_connect_to_peer(
                local_contact.private,
                peer_contact,
                &code.shared_secret.to_be_bytes(),
            )?;
        }
    }

    Ok(())
}
