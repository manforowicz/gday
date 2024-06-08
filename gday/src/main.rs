//! Command line tool to securely send files (without a relay or port forwarding).
#![forbid(unsafe_code)]
#![warn(clippy::all)]

mod dialog;
mod tests;
mod transfer;

use crate::dialog::ask_receive;
use clap::{Parser, Subcommand};
use gday_file_transfer::{encrypt_connection, read_from, write_to, FileOfferMsg, FileResponseMsg};
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

/// How long to try hole punching before giving up.
const HOLE_PUNCH_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

/// How long to try connecting to a server before giving up.
const SERVER_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

#[derive(Parser, Debug)]
#[command(author, version, about)]
struct Args {
    #[command(subcommand)]
    operation: Command,

    /// Use a custom gday server with this domain name.
    #[arg(short, long)]
    server: Option<String>,

    /// Connect to a custom server port.
    #[arg(short, long, requires("server"))]
    port: Option<u16>,

    /// Use raw TCP without TLS.
    #[arg(short, long, requires("server"))]
    unencrypted: bool,

    /// Verbosity. (trace, debug, info, warn, error)
    #[arg(short, long, default_value = "warn")]
    verbosity: log::LevelFilter,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Send these files and/or directories.
    Send {
        /// Custom shared code of form "server_id.room_code.shared_secret" (base 16).
        ///
        /// server_id must be valid, or 0 when custom --server set.
        ///
        /// Doesn't require a checksum digit.
        #[arg(short, long)]
        code: Option<String>,

        #[arg(required = true, num_args = 1..)]
        paths: Vec<PathBuf>,
    },

    /// Receive files. Input the code your peer gave you.
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
            server_connector::connect_to_domain_name(
                &domain_name,
                port,
                !args.unencrypted,
                SERVER_TIMEOUT,
            )?,
            0,
        )
    } else {
        server_connector::connect_to_random_server(DEFAULT_SERVERS, SERVER_TIMEOUT)?
    };

    match args.operation {
        // sending files
        crate::Command::Send { paths, code } => {
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

            // get metadata about the files to transfer
            let local_files = gday_file_transfer::get_file_metas(&paths)?;

            // confirm the user wants to send these files
            dialog::ask_send(&local_files)?;

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
            write_to(FileOfferMsg::from(local_files.clone()), &mut stream)?;

            println!("Waiting for your mate to respond to your file offer.");

            // receive file offer from peer
            let response: FileResponseMsg = read_from(&mut stream)?;

            // Total number of files accepted
            let num_accepted = response.get_total_num_accepted();

            // How many of those files are being resumed
            let resumptions = response.get_num_partially_accepted();

            println!(
                "Your mate accepted {}/{} files",
                num_accepted,
                local_files.len()
            );

            if num_accepted != 0 {
                if resumptions != 0 {
                    println!("Resuming transfer of {resumptions} previously interrupted file(s).");
                }
                transfer::send_files(local_files, response, &mut stream)?;
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

            let response = FileResponseMsg {
                response: ask_receive(&offer.files)?,
            };

            // respond to the file offer
            write_to(&response, &mut stream)?;

            if response.get_total_num_accepted() == 0 {
                println!("No files will be downloaded.");
            } else {
                transfer::receive_files(offer, response, &mut stream)?;
            }
        }
    }

    Ok(())
}
