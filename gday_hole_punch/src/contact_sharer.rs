use crate::{server_connector::ServerConnection, Error};
use gday_contact_exchange_protocol::{
    read_from_async, write_to_async, ClientMsg, FullContact, ServerMsg,
};

/// Used to exchange socket addresses with a peer via a Gday server.
#[derive(Debug)]
pub struct ContactSharer<'a> {
    room_code: u64,
    is_creator: bool,
    connection: &'a mut ServerConnection,
}

impl<'a> ContactSharer<'a> {
    /// Enters a new room with `room_code` in the gday server
    /// that `server_connection` connects to.
    ///
    /// If `is_creator`, tries creating the room, otherwise tries joining it.
    ///
    /// Sends local socket addresses to the server.
    ///
    /// Panics if both `v4` and `v6` in `server_connection` are `None`.
    ///
    /// Returns
    /// - The [`ContactSharer`].
    /// - The [`FullContact`] of this endpoint, as
    ///   determined by the server
    pub async fn enter_room(
        server_connection: &'a mut ServerConnection,
        room_code: u64,
        is_creator: bool,
    ) -> Result<(Self, FullContact), Error> {
        // set reuse addr and reuse port, so that these sockets
        // can be later reused for hole punching
        server_connection.configure()?;

        if is_creator {
            // choose a stream to talk to the server with
            let messenger = &mut server_connection.streams()[0];

            // try creating a room in the server
            write_to_async(ClientMsg::CreateRoom { room_code }, messenger).await?;
            let response: ServerMsg = read_from_async(messenger).await?;
            if response != ServerMsg::RoomCreated {
                return Err(Error::UnexpectedServerReply(response));
            }
        }

        let mut this = Self {
            room_code,
            is_creator,
            connection: server_connection,
        };

        // send personal socket addresses to the server
        let contact = this.share_contact().await?;

        Ok((this, contact))
    }

    /// Private helper function.
    /// Sends personal contact information the the server, and
    /// returns it's response.
    async fn share_contact(&mut self) -> Result<FullContact, Error> {
        let local_contact = self.connection.local_contact()?;

        // Get all connections to the server
        let mut streams = self.connection.streams();

        // For each connection, have the server record its
        // public address
        for stream in &mut streams {
            let msg = ClientMsg::RecordPublicAddr {
                room_code: self.room_code,
                is_creator: self.is_creator,
            };
            write_to_async(msg, stream).await?;
            let reply: ServerMsg = read_from_async(stream).await?;
            if reply != ServerMsg::ReceivedAddr {
                return Err(Error::UnexpectedServerReply(reply));
            }
        }

        // tell the server that we're done
        // sending socket addresses
        let msg = ClientMsg::ReadyToShare {
            room_code: self.room_code,
            is_creator: self.is_creator,
            local_contact,
        };
        write_to_async(msg, streams[0]).await?;

        // Get our local contact info from the server
        let reply: ServerMsg = read_from_async(streams[0]).await?;
        let ServerMsg::ClientContact(my_contact) = reply else {
            return Err(Error::UnexpectedServerReply(reply));
        };

        Ok(my_contact)
    }

    /// Blocks until the Gday server sends the contact information the
    /// other peer submitted. Returns the peer's [`FullContact`], as
    /// determined by the server
    pub async fn get_peer_contact(self) -> Result<FullContact, Error> {
        // This is the same stream we used to send DoneSending,
        // so the server should respond on it,
        // once the other peer is also done.
        let stream = &mut self.connection.streams()[0];
        let reply: ServerMsg = read_from_async(stream).await?;
        let ServerMsg::PeerContact(peer) = reply else {
            return Err(Error::UnexpectedServerReply(reply));
        };

        Ok(peer)
    }
}
