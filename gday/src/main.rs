//! Command line tool to securely send files (without a relay or port forwarding).
#![forbid(unsafe_code)]
#![warn(clippy::all)]

mod dialog;
mod transfer;

use crate::dialog::ask_receive;
use clap::{Parser, Subcommand};
use gday_encryption::EncryptedStream;
use gday_file_transfer::{read_from, write_to, FileOfferMsg, FileResponseMsg};
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
use std::str::FromStr;

/// How long to try hole punching before giving up.
const HOLE_PUNCH_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

/// How long to try connecting to a server before giving up.
const SERVER_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);

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
    /// Send files and/or directories.
    Send {
        /// Custom shared code of form "server_id.room_code.shared_secret" (base 16).
        ///
        /// server_id must be valid, or 0 when custom --server set.
        ///
        /// Doesn't require a checksum digit.
        #[arg(short, long)]
        code: Option<String>,

        /// Files and/or directories to send.
        #[arg(required = true, num_args = 1..)]
        paths: Vec<PathBuf>,
    },

    /// Receive files.
    Get {
        /// Directory where to save the files.
        /// By default, saves them in the current directory.
        #[arg(short, long, default_value = ".")]
        path: PathBuf,

        /// The code that your peer gave you.
        code: String,
    },
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
    } else {
        server_connector::DEFAULT_PORT
    };

    // use custom server if the user provided one,
    // otherwise pick a random default server
    let (mut server_connection, server_id) = if let Some(domain_name) = args.server {
        if args.unencrypted {
            (
                server_connector::connect_tcp(format!("{domain_name}:{port}"), SERVER_TIMEOUT)?,
                0,
            )
        } else {
            (
                server_connector::connect_tls(domain_name, port, SERVER_TIMEOUT)?,
                0,
            )
        }
    } else {
        server_connector::connect_to_random_server(DEFAULT_SERVERS, SERVER_TIMEOUT)?
    };

    match args.operation {
        // sending files
        crate::Command::Send { paths, code } => {
            // generate random `room_code` and `shared_secret`
            // if the user didn't provide custom ones
            let peer_code = if let Some(code) = code {
                PeerCode::from_str(&code)?
            } else {
                let mut rng = rand::thread_rng();
                PeerCode {
                    server_id,
                    room_code: rng.gen_range(0..u16::MAX as u64),
                    shared_secret: rng.gen_range(0..u16::MAX as u64),
                }
            };

            // get metadata about the files to transfer
            let local_files = gday_file_transfer::get_file_metas(&paths)?;
            let offer_msg = FileOfferMsg::from(local_files.clone());

            // confirm the user wants to send these files
            if !dialog::confirm_send(&offer_msg)? {
                // Send aborted
                return Ok(());
            }

            // create a room in the server
            let (contact_sharer, my_contact) =
                ContactSharer::enter_room(&mut server_connection, peer_code.room_code, true)?;

            info!("Your contact is:\n{my_contact}");

            println!("Tell your mate to run \"gday get {}\"", peer_code.bold());

            // get peer's contact
            let peer_contact = contact_sharer.get_peer_contact()?;
            info!("Your mate's contact is:\n{peer_contact}");

            // connect to the peer
            let (stream, shared_key) = gday_hole_punch::try_connect_to_peer(
                my_contact.local,
                peer_contact,
                &peer_code.shared_secret.to_be_bytes(),
                HOLE_PUNCH_TIMEOUT,
            )?;

            let mut stream = EncryptedStream::encrypt_connection(stream, &shared_key)?;

            info!("Established authenticated encrypted connection with peer.");

            // offer these files to the peer
            write_to(offer_msg, &mut stream)?;

            println!("File offer sent to mate. Waiting on response.");

            // receive file offer from peer
            let response: FileResponseMsg = read_from(&mut stream)?;

            // Total number of files accepted
            let num_accepted = response.get_num_not_rejected();

            // How many of those files are being resumed
            let resumptions = response.get_num_partially_accepted();

            println!(
                "Your mate accepted {}/{} files",
                num_accepted,
                local_files.len()
            );

            if resumptions != 0 {
                println!("Resuming transfer of {resumptions} previously interrupted file(s).");
            }

            if num_accepted != 0 {
                transfer::send_files(local_files, response, &mut stream)?;
            }
        }

        // receiving files
        crate::Command::Get { path, code } => {
            let code = PeerCode::from_str(&code)?;
            let (contact_sharer, my_contact) =
                ContactSharer::enter_room(&mut server_connection, code.room_code, false)?;

            info!("Your contact is:\n{my_contact}");

            let peer_contact = contact_sharer.get_peer_contact()?;

            info!("Your mate's contact is:\n{peer_contact}");

            let (stream, shared_key) = gday_hole_punch::try_connect_to_peer(
                my_contact.local,
                peer_contact,
                &code.shared_secret.to_be_bytes(),
                HOLE_PUNCH_TIMEOUT,
            )?;

            let mut stream = EncryptedStream::encrypt_connection(stream, &shared_key)?;

            info!("Established authenticated encrypted connection with peer.");

            // receive file offer from peer
            let offer: FileOfferMsg = read_from(&mut stream)?;

            let response = ask_receive(&offer, &path)?;

            // respond to the file offer
            write_to(&response, &mut stream)?;

            if response.get_num_not_rejected() == 0 {
                println!("No files will be downloaded.");
            } else {
                transfer::receive_files(offer, response, &path, &mut stream)?;
            }
        }
    }

    server_connection.notify_tls_close()?;

    Ok(())
}
