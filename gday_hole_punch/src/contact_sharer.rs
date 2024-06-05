use crate::{server_connector::ServerConnection, Error};
use gday_contact_exchange_protocol::{read_from, write_to, ClientMsg, FullContact, ServerMsg};

/// Used to exchange socket addresses with a peer via a Gday server.
pub struct ContactSharer<'a> {
    room_code: u64,
    is_creator: bool,
    connection: &'a mut ServerConnection,
}

impl<'a> ContactSharer<'a> {
    /// Creates a new room with `room_code` in the Gday server
    /// that `server_connection` connects to.
    ///
    /// Sends local socket addresses to the server
    ///
    /// Returns
    /// - The [`ContactSharer`]
    /// - The [`FullContact`] of this endpoint, as
    ///   determined by the server
    pub fn create_room(
        room_code: u64,
        server_connection: &'a mut ServerConnection,
    ) -> Result<(Self, FullContact), Error> {
        server_connection.configure()?;

        let messenger = &mut server_connection.streams()[0];

        write_to(ClientMsg::CreateRoom { room_code }, messenger)?;
        let response: ServerMsg = read_from(messenger)?;

        if response != ServerMsg::RoomCreated {
            return Err(Error::UnexpectedServerReply(response));
        }

        let mut this = Self {
            room_code,
            is_creator: true,
            connection: server_connection,
        };

        let contact = this.share_contact()?;

        Ok((this, contact))
    }

    /// Joins a room with `room_code` in the Gday server
    /// that `server_connection` connects to.
    ///
    /// Sends local socket addresses to the server
    ///
    /// Returns
    /// - The [`ContactSharer`]
    /// - The [`FullContact`] of this endpoint, as
    ///   determined by the server
    pub fn join_room(
        room_code: u64,
        server_connection: &'a mut ServerConnection,
    ) -> Result<(Self, FullContact), Error> {
        server_connection.configure()?;

        let mut this = Self {
            room_code,
            is_creator: false,
            connection: server_connection,
        };

        let contact = this.share_contact()?;

        Ok((this, contact))
    }

    /// Private helper function.
    /// Sends personal contact information the the server, and
    /// returns it's response.
    fn share_contact(&mut self) -> Result<FullContact, Error> {
        let mut streams = self.connection.streams();

        for stream in &mut streams {
            let private_addr = Some(stream.local_addr()?);
            let msg = ClientMsg::SendAddr {
                room_code: self.room_code,
                is_creator: self.is_creator,
                private_addr,
            };
            write_to(msg, stream)?;
            let reply: ServerMsg = read_from(stream)?;
            if reply != ServerMsg::ReceivedAddr {
                return Err(Error::UnexpectedServerReply(reply));
            }
        }

        let msg = ClientMsg::DoneSending {
            room_code: self.room_code,
            is_creator: self.is_creator,
        };

        write_to(msg, streams[0])?;

        let reply: ServerMsg = read_from(streams[0])?;

        let ServerMsg::ClientContact(my_contact) = reply else {
            return Err(Error::UnexpectedServerReply(reply));
        };

        Ok(my_contact)
    }

    /// Blocks until the Gday server sends the contact information the
    /// other peer submitted. Returns the peer's [`FullContact`], as
    /// determined by the server
    pub fn get_peer_contact(self) -> Result<FullContact, Error> {
        let stream = &mut self.connection.streams()[0];
        let reply: ServerMsg = read_from(stream)?;
        let ServerMsg::PeerContact(peer) = reply else {
            return Err(Error::UnexpectedServerReply(reply));
        };

        Ok(peer)
    }
}
