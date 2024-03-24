use crate::{server_connector::ServerConnection, Error};
use gday_contact_exchange_protocol::{
    deserialize_from, serialize_into, ClientMsg, FullContact, ServerMsg, MAX_MSG_SIZE,
};

/// Used to exchange contact information with a peer via the `gday_server`.
pub struct ContactSharer {
    room_code: u64,
    is_creator: bool,
    connection: ServerConnection,
}

impl ContactSharer {
    /// Creates a new room in the `gday_server` that the given streams connect to.
    /// Sends contact information to the server.
    ///
    /// Returns (
    /// - The [`ContactSharer`]
    /// - The [`FullContact`] of this endpoint, as
    ///   determined by the server
    /// )
    pub fn create_room(
        room_code: u64,
        mut server_connection: ServerConnection,
    ) -> Result<(Self, FullContact), Error> {
        server_connection.configure()?;

        let messenger = &mut server_connection.streams()[0];

        let buf = &mut [0; MAX_MSG_SIZE];

        serialize_into(ClientMsg::CreateRoom { room_code }, messenger)?;
        let response = deserialize_from(messenger, buf)?;

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

    /// Joins a room in the `gday_server` that the given streams connect to.
    /// `room_id` should be the code provided by the other peer who called `create_room`.
    /// Sends contact information to the server.
    ///
    /// Returns (
    /// - The `ContactSharer`
    /// - The [`FullContact`] that the server returned.
    ///     Contains this user's public and private socket addresses.
    ///
    /// )
    pub fn join_room(
        room_code: u64,
        server_connection: ServerConnection,
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
        let buf = &mut [0; MAX_MSG_SIZE];

        for stream in &mut streams {
            let private_addr = Some(stream.get_ref().local_addr()?);
            let msg = ClientMsg::SendAddr {
                room_code: self.room_code,
                is_creator: self.is_creator,
                private_addr,
            };
            serialize_into(msg, stream)?;
            let reply = deserialize_from(stream, buf)?;
            if reply != ServerMsg::ReceivedAddr {
                return Err(Error::UnexpectedServerReply(reply));
            }
        }

        let msg = ClientMsg::DoneSending {
            room_code: self.room_code,
            is_creator: self.is_creator,
        };

        serialize_into(msg, streams[0])?;

        let reply = deserialize_from(streams[0], buf)?;

        let ServerMsg::ClientContact(my_contact) = reply else {
            return Err(Error::UnexpectedServerReply(reply));
        };

        Ok(my_contact)
    }

    /// Waits for the `gday_server` to send the contact information the
    /// other peer submitted.
    /// Then returns a [`HolePuncher`] which can be used to try establishing
    /// an authenticated TCP connection with that peer.
    pub fn get_peer_contact(mut self) -> Result<FullContact, Error> {
        let buf = &mut [0; MAX_MSG_SIZE];
        let stream = &mut self.connection.streams()[0];
        let reply = deserialize_from(stream, buf)?;
        let ServerMsg::PeerContact(peer) = reply else {
            return Err(Error::UnexpectedServerReply(reply));
        };

        Ok(peer)
    }
}
