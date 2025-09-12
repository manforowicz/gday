use std::{
    ops::Deref,
    path::PathBuf,
    pin::Pin,
    sync::{Arc, Mutex},
    task::{Context, Poll},
};

use gday_encryption::EncryptedStream;
use gday_file_transfer::{FileOfferMsg, FileRequestsMsg, LocalFileOffer, TransferReport};
use gday_hole_punch::{FullContact, PeerCode, server_connector::DEFAULT_SERVERS};
use tokio::net::TcpStream;

use crate::View;

pub struct MyHandle<T>(pub tokio::task::JoinHandle<T>);

impl<T> Drop for MyHandle<T> {
    fn drop(&mut self) {
        self.0.abort();
    }
}

impl<T> Deref for MyHandle<T> {
    type Target = tokio::task::JoinHandle<T>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> Future for MyHandle<T> {
    type Output = Result<T, tokio::task::JoinError>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        Pin::new(&mut self.0).poll(cx)
    }
}

pub async fn send1(paths: &[PathBuf]) -> anyhow::Result<View> {
    let (mut conn, server_id) =
        gday_hole_punch::server_connector::connect_to_random_server(DEFAULT_SERVERS).await?;
    let peer_code = PeerCode::random(server_id, 6);
    let room_code = peer_code.room_code().to_string();

    let peer_contact_handle = tokio::spawn(async move {
        let (my_contact, peer_contact_fut) =
            gday_hole_punch::share_contacts(&mut conn, &room_code, true).await?;
        Ok((my_contact, peer_contact_fut.await?))
    });
    let peer_contact_handle = MyHandle(peer_contact_handle);
    let offer = gday_file_transfer::create_file_offer(paths)?;

    Ok(View::Send2 {
        offer,
        peer_code,
        peer_contact_handle,
    })
}

pub async fn send2(
    my_contact: FullContact,
    peer_contact: FullContact,
    shared_secret: String,
    offer: LocalFileOffer,
    transfer_report: Arc<Mutex<TransferReport>>,
) -> anyhow::Result<()> {
    let (tcp, key) =
        gday_hole_punch::try_connect_to_peer(my_contact.local, peer_contact, &shared_secret)
            .await?;
    let mut peer_conn = gday_encryption::EncryptedStream::encrypt_connection(tcp, &key).await?;
    gday_file_transfer::write_to_async(&offer.offer, &mut peer_conn).await?;
    let reply = gday_file_transfer::read_from_async(&mut peer_conn).await?;
    gday_file_transfer::send_files(&offer, &reply, &mut peer_conn, |report| {
        transfer_report.lock().unwrap().clone_from(report)
    })
    .await?;
    Ok(())
}

pub async fn receive1(peer_code: PeerCode) -> anyhow::Result<View> {
    let mut conn = gday_hole_punch::server_connector::connect_to_server_id(
        DEFAULT_SERVERS,
        peer_code.server_id(),
    )
    .await?;

    let (my_contact, peer_contact_fut) =
        gday_hole_punch::share_contacts(&mut conn, peer_code.room_code(), false).await?;
    let peer_contact = peer_contact_fut.await?;

    let (tcp, key) = gday_hole_punch::try_connect_to_peer(
        my_contact.local,
        peer_contact,
        peer_code.shared_secret(),
    )
    .await?;
    let mut peer_conn = gday_encryption::EncryptedStream::encrypt_connection(tcp, &key).await?;
    let offer = gday_file_transfer::read_from_async(&mut peer_conn).await?;

    Ok(View::Receive3 { peer_conn, offer })
}

pub async fn receive2(
    mut conn: EncryptedStream<TcpStream>,
    offer: FileOfferMsg,
    save_path: PathBuf,
    transfer_report: Arc<Mutex<TransferReport>>,
) -> anyhow::Result<()> {
    let request = FileRequestsMsg::accept_all_files(&offer);
    gday_file_transfer::write_to_async(&request, &mut conn).await?;
    gday_file_transfer::receive_files(&offer, &request, &save_path, conn, |report| {
        transfer_report.lock().unwrap().clone_from(report)
    })
    .await?;

    Ok(())
}
