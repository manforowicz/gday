#![forbid(unsafe_code)]
#![warn(clippy::all)]
//! Command line tool to securely send files (without a relay or port
//! forwarding).

mod dialog;
mod transfer;

use anstream::println;
use anstyle::{AnsiColor, Color, Style};
use clap::{Parser, Subcommand};
use gday_encryption::EncryptedStream;
use gday_file_transfer::{FileOfferMsg, FileRequestsMsg, read_from_async, write_to_async};
use gday_hole_punch::server_connector::{self, DEFAULT_SERVERS};
use gday_hole_punch::{PeerCode, share_contacts};
use log::{error, info};
use std::path::PathBuf;

const BOLD: Style = Style::new().bold();
const GREEN: Style = BOLD.fg_color(Some(Color::Ansi(AnsiColor::Green)));
const RED: Style = BOLD.fg_color(Some(Color::Ansi(AnsiColor::Red)));

/// How long to try hole punching before giving up.
const HOLE_PUNCH_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);

#[derive(Parser, Debug)]
#[command(author, version, about)]
struct Args {
    #[command(subcommand)]
    command: Command,

    /// Use a custom gday server with this domain name.
    #[arg(short, long)]
    server: Option<String>,

    /// Connect to a custom server port.
    #[arg(short, long, requires("server"))]
    port: Option<u16>,

    /// Connect to server with TCP instead of TLS.
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
        /// Files and/or directories to send.
        #[arg(required = true, num_args = 1..)]
        paths: Vec<PathBuf>,

        /// Custom shared code of form "server_id.room_code.shared_secret".
        ///
        /// A server_id of 0 causes a random server to be used.
        /// server_id ignored when custom --server set.
        #[arg(short, long, conflicts_with = "length")]
        code: Option<PeerCode>,

        /// Length of room_code and shared_secret to generate.
        #[arg(short, long, default_value = "5", conflicts_with = "code")]
        length: usize,
    },

    /// Receive files.
    Get {
        /// The code your peer gave you (of form
        /// "server_id.room_code.shared_secret")
        code: PeerCode,

        /// Directory where to save the files.
        #[arg(short, long, default_value = ".")]
        path: PathBuf,
    },
}

#[tokio::main]
async fn main() {
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
    if let Err(err) = run(args).await {
        error!("{err}");
    }
}

async fn run(args: crate::Args) -> Result<(), Box<dyn std::error::Error>> {
    // Get the server port
    let port = if let Some(port) = args.port {
        port
    } else {
        server_connector::DEFAULT_PORT
    };

    // Connect to a custom server if the user chose one.
    let custom_server = if let Some(domain_name) = args.server {
        if args.unencrypted {
            Some(server_connector::connect_tcp(format!("{domain_name}:{port}")).await?)
        } else {
            Some(server_connector::connect_tls(domain_name, port).await?)
        }
    } else {
        None
    };

    match args.command {
        crate::Command::Send {
            paths,
            code,
            length,
        } => {
            // If the user chose a custom server
            let (mut server_connection, server_id) = if let Some(custom_server) = custom_server {
                (custom_server, 0)

            // If the user chose a custom code
            } else if let Some(code) = &code {
                if code.server_id() == 0 {
                    server_connector::connect_to_random_server(DEFAULT_SERVERS).await?
                } else {
                    (
                        server_connector::connect_to_server_id(DEFAULT_SERVERS, code.server_id())
                            .await?,
                        code.server_id(),
                    )
                }

            // Otherwise, pick a random server
            } else {
                server_connector::connect_to_random_server(DEFAULT_SERVERS).await?
            };

            // generate random `room_code` and `shared_secret`
            // if the user didn't provide custom ones
            let peer_code = if let Some(code) = code {
                PeerCode::new(
                    server_id,
                    code.room_code().to_string(),
                    code.shared_secret().to_string(),
                )
                .unwrap()
            } else {
                PeerCode::random(server_id, length)
            };

            // get metadata about the files to transfer
            let local_file_offer = gday_file_transfer::create_file_offer(&paths)?;

            // pretty-print the files to be sent
            dialog::display_send(&local_file_offer.offer);

            // create a room in the server
            let (my_contact, peer_contact_fut) =
                share_contacts(&mut server_connection, peer_code.room_code(), true).await?;

            println!("Tell your mate to run \"gday get {BOLD}{peer_code}{BOLD:#}\"",);

            // get peer's contact
            let peer_contact = peer_contact_fut.await?;

            // connect to the peer
            let (stream, shared_key) = tokio::time::timeout(
                HOLE_PUNCH_TIMEOUT,
                gday_hole_punch::try_connect_to_peer(
                    my_contact.local,
                    peer_contact,
                    peer_code.shared_secret(),
                ),
            )
            .await
            .map_err(|_| gday_hole_punch::Error::HolePunchTimeout)??;

            // Gracefully terminate TLS
            server_connection.shutdown().await?;

            let mut stream = EncryptedStream::encrypt_connection(stream, &shared_key).await?;

            info!("Established authenticated encrypted connection with peer.");

            // offer these files to the peer
            write_to_async(&local_file_offer.offer, &mut stream).await?;

            println!("File offer sent to mate. Waiting on response.");

            // receive response from peer
            let response: FileRequestsMsg = read_from_async(&mut stream).await?;

            // Total number of files accepted
            let num_accepted = response.get_num_not_rejected();

            // How many of those files are being resumed
            let resumptions = response.get_num_partially_accepted();

            println!(
                "Your mate accepted {}/{} files",
                num_accepted,
                local_file_offer.offer.offer.len()
            );

            if resumptions != 0 {
                println!("Resuming transfer of {resumptions} previously interrupted file(s).");
            }

            if num_accepted != 0 {
                transfer::send_files(local_file_offer, response, &mut stream).await?;
            }
        }

        // receiving files
        crate::Command::Get { path, code } => {
            let mut server_connection = if let Some(custom_server) = custom_server {
                custom_server
            } else {
                server_connector::connect_to_server_id(DEFAULT_SERVERS, code.server_id()).await?
            };

            let (my_contact, peer_contact_fut) =
                share_contacts(&mut server_connection, code.room_code(), false).await?;

            let peer_contact = peer_contact_fut.await?;

            let (stream, shared_key) = tokio::time::timeout(
                HOLE_PUNCH_TIMEOUT,
                gday_hole_punch::try_connect_to_peer(
                    my_contact.local,
                    peer_contact,
                    code.shared_secret(),
                ),
            )
            .await
            .map_err(|_| gday_hole_punch::Error::HolePunchTimeout)??;

            // Gracefully terminate TLS
            server_connection.shutdown().await?;

            let mut stream = EncryptedStream::encrypt_connection(stream, &shared_key).await?;

            info!("Established authenticated encrypted connection with peer.");

            // receive file offer from peer
            let offer: FileOfferMsg = read_from_async(&mut stream).await?;

            let response = dialog::ask_receive(&offer, &path)?;

            // respond to the file offer
            write_to_async(&response, &mut stream).await?;

            if response.get_num_not_rejected() == 0 {
                println!("No files will be downloaded.");
            } else {
                transfer::receive_files(offer, response, &path, &mut stream).await?;
            }
        }
    }

    Ok(())
}
