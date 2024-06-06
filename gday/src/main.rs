//! `gday` is a command line line tool for peers to send each other files.
//! Features:
//! - Never uses relays. Instead, uses a gday_contact_exchange_server to share socket
//!     addresses with peer, and then performs
//!     [Hole Punching](https://en.wikipedia.org/wiki/TCP_hole_punching)
//!     to establish a direct connection. Note that this may fail when one of the peers
//!     is behind a strict [NAT](https://en.wikipedia.org/wiki/Network_address_translation).
#![forbid(unsafe_code)]
#![warn(clippy::all)]

mod dialog;
mod tests;
mod transfer;

use crate::dialog::ask_receive;
use clap::{Parser, Subcommand};
use gday_file_transfer::{
    encrypt_connection, read_from, write_to, FileMeta, FileMetaLocal, FileOfferMsg, FileResponseMsg,
};
use gday_hole_punch::PeerCode;
use gday_hole_punch::{
    server_connector::{self, DEFAULT_SERVERS},
    ContactSharer,
};
use log::error;
use log::info;
use owo_colors::OwoColorize;
use rand::Rng;
use std::path::PathBuf;

const HOLE_PUNCH_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);

#[derive(Parser, Debug)]
#[command(author, version, about)]
struct Args {
    #[command(subcommand)]
    operation: Command,

    /// Use a custom gday server with this domain name.
    #[arg(short, long)]
    server: Option<String>,

    /// Which server port to connect to.
    #[arg(short, long, requires("server"))]
    port: Option<u16>,

    /// Use unencrypted TCP instead of TLS to the custom server.
    #[arg(short, long, requires("server"))]
    unencrypted: bool,

    /// Verbosity. (trace, debug, info, warn, error)
    #[arg(short, long, default_value = "warn")]
    verbosity: log::LevelFilter,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Send files
    Send {
        /// Custom shared code of form "server_id.room_code.shared_secret" (base 16).
        ///
        /// server_id must be valid, or 0 when custom --server set.
        ///
        /// Doesn't require a checksum digit.
        #[arg(short, long)]
        code: Option<String>,

        /// TODO: Comment
        paths: Vec<PathBuf>,
    },

    /// Receive files. Input the code your peer told you.
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

fn run(args: crate::Args) -> Result<(), Box<dyn std::error::Error>> {
    // get the server port
    let port = if let Some(port) = args.port {
        port
    } else if args.unencrypted {
        gday_hole_punch::DEFAULT_TCP_PORT
    } else {
        gday_hole_punch::DEFAULT_TLS_PORT
    };

    // use custom server if the user provided one,
    // otherwise pick a random default server
    let (mut server_connection, server_id) = if let Some(domain_name) = args.server {
        (
            server_connector::connect_to_domain_name(&domain_name, port, !args.unencrypted)?,
            0,
        )
    } else {
        server_connector::connect_to_random_server(DEFAULT_SERVERS)?
    };

    match args.operation {
        // sending files
        crate::Command::Send { paths, code } => {
            // confirm the user wants to send these files
            let local_files = dialog::ask_send(&paths)?;

            // generate random `room_code` and `shared_secret`
            // if the user didn't provide custom ones
            let peer_code = if let Some(code) = code {
                PeerCode::parse(&code, false)?
            } else {
                let room_code = rand::thread_rng().gen_range(0..u16::MAX as u64);
                let shared_secret = rand::thread_rng().gen_range(0..u16::MAX as u64);
                PeerCode {
                    server_id,
                    room_code,
                    shared_secret,
                }
            };

            // create a room in the server
            let (contact_sharer, my_contact) =
                ContactSharer::create_room(peer_code.room_code, &mut server_connection)?;

            info!("Your contact is:\n{my_contact}");

            println!(
                "Tell your mate to run \"gday receive {}\"",
                peer_code.to_str().bold()
            );

            // get peer's contact
            let peer_contact = contact_sharer.get_peer_contact()?;
            info!("Your mate's contact is:\n{peer_contact}");

            // connect to the peer
            let (stream, shared_key) = gday_hole_punch::try_connect_to_peer(
                my_contact.private,
                peer_contact,
                &peer_code.shared_secret.to_be_bytes(),
                HOLE_PUNCH_TIMEOUT,
            )?;

            let mut stream = encrypt_connection(stream, &shared_key)?;

            info!("Established authenticated encrypted connection with peer.");

            // offer these files to the peer
            write_to(FileOfferMsg::from(&local_files), &mut stream)?;

            println!("Waiting for your mate to respond to your file offer.");

            // receive file offer from peer
            let response: FileResponseMsg = read_from(&mut stream)?;

            let accepted = response.response.iter().filter_map(|&x| x);
            let num_accepted = accepted.clone().count();
            let resumptions = accepted.filter(|&x| x != 0).count();

            println!(
                "Your mate accepted {}/{} files",
                num_accepted,
                local_files.len()
            );

            if num_accepted != 0 {
                if resumptions != 0 {
                    println!("Resuming transfer of {resumptions} previously interrupted file(s).");
                }
                let pairs: Vec<(FileMetaLocal, Option<u64>)> =
                    local_files.into_iter().zip(response.response).collect();
                transfer::send_files(&mut stream, &pairs)?;
            }
        }

        // receiving files
        crate::Command::Receive { code } => {
            let code = PeerCode::parse(&code, true)?;
            let (contact_sharer, my_contact) =
                ContactSharer::join_room(code.room_code, &mut server_connection)?;

            info!("Your contact is:\n{my_contact}");

            let peer_contact = contact_sharer.get_peer_contact()?;

            info!("Your mate's contact is:\n{peer_contact}");

            let (stream, shared_key) = gday_hole_punch::try_connect_to_peer(
                my_contact.private,
                peer_contact,
                &code.shared_secret.to_be_bytes(),
                HOLE_PUNCH_TIMEOUT,
            )?;

            let mut stream = encrypt_connection(stream, &shared_key)?;

            info!("Established authenticated encrypted connection with peer.");

            // receive file offer from peer
            let offer: FileOfferMsg = read_from(&mut stream)?;

            // ask user which files to receive
            let accepted = ask_receive(&offer.files)?;

            // respond to the file offer
            write_to(
                FileResponseMsg {
                    response: accepted.clone(),
                },
                &mut stream,
            )?;

            if accepted.iter().all(|x| x.is_none()) {
                println!("No files will be downloaded.");
            } else {
                let pairs: Vec<(FileMeta, Option<u64>)> =
                    offer.files.into_iter().zip(accepted).collect();
                transfer::receive_files(&mut stream, &pairs)?;
            }
        }
    }

    Ok(())
}
