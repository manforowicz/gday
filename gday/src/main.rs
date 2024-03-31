//! `gday` is a command line line tool for direct file transfer between peers.
//! TODO
#![forbid(unsafe_code)]
#![warn(clippy::all)]

mod base32;
mod dialog;
mod transfer;

use base32::PeerCode;
use clap::{Parser, Subcommand};
use dialog::confirm_receive;
use gday_file_offer_protocol::{from_reader, FileResponseMsg};
use gday_hole_punch::{
    server_connector::{self, DEFAULT_SERVERS},
    ContactSharer,
};
use log::{error, info};
use owo_colors::OwoColorize;
use rand::Rng;
use std::path::PathBuf;

use crate::transfer::encrypt_connection;

use gday_file_offer_protocol::{to_writer, FileMeta, FileOfferMsg};

const TMP_DOWNLOAD_PREFIX: &str = "GDAY_PARTIAL_DOWNLOAD_";

const HOLE_PUNCH_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);

/// Send files directly to peers
#[derive(Parser, Debug)]
#[command(author, version, about)]
struct Args {
    #[command(subcommand)]
    operation: Command,

    /// Use a custom Gday server with this domain name
    #[arg(long)]
    server: Option<String>,

    /// Use a custom room code
    #[arg(long)]
    room: Option<u64>,

    /// Use a custom shared secret
    #[arg(long)]
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
    // read command line arguments
    let args = Args::parse();

    // initialize logging
    env_logger::builder()
        .format_module_path(false)
        .format_target(false)
        .format_timestamp(None)
        .filter_level(args.verbosity)
        .init();

    // catch and log any errors
    if let Err(err) = run(args) {
        error!("{}", err);
    }
}

fn run(args: Args) -> Result<(), Box<dyn std::error::Error>> {
    // use custom server if the user provided one,
    // otherwise pick a random default server
    let (mut server_connection, server_id) = if let Some(domain_name) = args.server {
        (server_connector::connect_to_domain_name(&domain_name)?, 0)
    } else {
        server_connector::connect_to_random_server(DEFAULT_SERVERS)?
    };

    match args.operation {
        // sending files
        Command::Send { paths } => {
            // confirm the user wants to send these files
            let local_files = dialog::confirm_send(&paths)?;

            // generate random `room_code` and `shared_secret`
            // if the user didn't provide custom ones
            let room_code = if let Some(code) = args.room {
                code
            } else {
                rand::thread_rng().gen_range(0..u16::MAX as u64)
            };
            let shared_secret = if let Some(secret) = args.room {
                secret
            } else {
                rand::thread_rng().gen_range(0..u16::MAX as u64)
            };
            let peer_code = PeerCode {
                server_id,
                room_code,
                shared_secret,
            };

            // create a room in the server
            let (contact_sharer, my_contact) =
                ContactSharer::create_room(room_code, &mut server_connection)?;

            info!("Your contact is:\n{my_contact}");

            println!(
                "Tell your mate to run \"gday receive {}\"",
                peer_code.to_str().bold()
            );

            // get peer's contact
            let peer_contact = contact_sharer.get_peer_contact()?;
            info!("Your mate's contact is:\n{peer_contact}");
            info!("Trying TCP hole punching.");

            // connect to the peer
            let (stream, shared_key) = gday_hole_punch::try_connect_to_peer(
                my_contact.private,
                peer_contact,
                &shared_secret.to_be_bytes(),
                HOLE_PUNCH_TIMEOUT,
            )?;

            let mut stream = encrypt_connection(stream, &shared_key, true)?;

            info!("Established authenticated encrypted connection with peer.");

            // get the [`FileMeta`] of the files the user wants to send
            let files = local_files
                .iter()
                .map(|f| FileMeta::from(f.clone()))
                .collect();

            // offer these files to the peer
            to_writer(FileOfferMsg { files }, &mut stream)?;

            println!("Waiting for your mate to respond to your file offer.");

            // receive file offer from peer
            let response: FileResponseMsg = from_reader(&mut stream)?;

            if response.accepted.iter().all(|x| x.is_none()) {
                println!("Your mate rejected all your offered files.");
            } else {
                if !response.accepted.iter().filter_map(|x| *x).all(|x| x == 0) {
                    println!("Resuming transfer of interrupted file(s).");
                }
                transfer::send_files(&mut stream, &local_files, &response.accepted)?;
            }
        }

        // receiving files
        Command::Receive { code } => {
            let code = PeerCode::from_str(&code)?;
            let (contact_sharer, my_contact) =
                ContactSharer::join_room(code.room_code, &mut server_connection)?;

            info!("Your contact is:\n{my_contact}");

            let peer_contact = contact_sharer.get_peer_contact()?;

            info!("Your mate's contact is:\n{peer_contact}");
            info!("Trying TCP hole punching.");

            let (stream, shared_key) = gday_hole_punch::try_connect_to_peer(
                my_contact.private,
                peer_contact,
                &code.shared_secret.to_be_bytes(),
                HOLE_PUNCH_TIMEOUT,
            )?;

            let mut stream = encrypt_connection(stream, &shared_key, false)?;

            info!("Established authenticated encrypted connection with peer.");

            // receive file offer from peer
            let offer: FileOfferMsg = from_reader(&mut stream)?;

            // ask user which files to receive
            let accepted = confirm_receive(&offer.files)?;

            // respond to the file offer
            to_writer(
                FileResponseMsg {
                    accepted: accepted.clone(),
                },
                &mut stream,
            )?;

            if accepted.iter().all(|x| x.is_none()) {
                println!("No files will be downloaded.");
            } else {
                transfer::receive_files(&mut stream, &offer.files, &accepted)?;
            }
        }
    }

    Ok(())
}
