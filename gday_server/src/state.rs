use gday_contact_exchange_protocol::FullContact;
use std::{
    collections::HashMap,
    net::{IpAddr, SocketAddr},
    sync::{Arc, Mutex},
    time::Duration,
};
use thiserror::Error;
use tokio::sync::oneshot;

/// Information about a client in a [`Room`].
#[derive(Default, Debug)]
struct Client {
    /// Contact info of this client
    contact: FullContact,
    /// - `None` if the other peer isn't done and
    ///     isn't ready to receive this peer's contacts.
    /// - `Some` if the other peer is done and
    ///     ready to receive this peer's contacts.
    ///
    /// Once this peer is done, and `contact_sender` isn't `None`,
    /// this sender sends [`Self::contact`].
    contact_sender: Option<oneshot::Sender<FullContact>>,
}

/// A room holds 2 [Client]s that want to exchange their contact info
#[derive(Default, Debug)]
struct Room {
    /// The client that created this room
    creator: Client,
    /// The client that joined this room
    joiner: Client,
}

impl Room {
    /// Get a reference to a client from this room
    fn get_client(&mut self, is_creator: bool) -> &Client {
        if is_creator {
            &self.creator
        } else {
            &self.joiner
        }
    }

    /// Get a mutable reference to a client from this room
    fn get_client_mut(&mut self, is_creator: bool) -> &mut Client {
        if is_creator {
            &mut self.creator
        } else {
            &mut self.joiner
        }
    }
}

/// A reference to the server's shared state.
///
/// Can only be used in a tokio runtime.
///
/// Note: Throughout all the functions, only one lock
/// is acquired at any given time. This is to prevent deadlock.
#[derive(Clone, Debug)]
pub struct State {
    /// Maps room_code to rooms
    rooms: Arc<Mutex<HashMap<u64, Room>>>,

    /// Maps IP addresses to the number of requests they sent this minute.
    request_counts: Arc<Mutex<HashMap<IpAddr, u32>>>,

    /// Maximum number of requests an IP address can
    /// send per minute before they're rejected.
    max_requests_per_minute: Arc<u32>,

    /// Seconds before a newly created room is deleted
    room_timeout: Arc<std::time::Duration>,
}

impl State {
    /// Creates a new [`State`] with the given config settings
    pub fn new(max_requests_per_minute: u32, room_timeout: std::time::Duration) -> Self {
        let this = Self {
            rooms: Arc::default(),
            request_counts: Arc::default(),
            max_requests_per_minute: Arc::new(max_requests_per_minute),
            room_timeout: Arc::new(room_timeout),
        };

        // spawn a backround thread that clears `request_counts` every minute
        let request_counts = this.request_counts.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(60));
            loop {
                interval.tick().await;
                request_counts
                    .lock()
                    .expect("Couldn't acquire state lock.")
                    .clear();
            }
        });

        this
    }

    /// Creates a new room with `room_code`.
    ///
    /// - Returns [`Error::TooManyRequests`] if the max
    /// allowable number of requests per minute is exceeded.
    pub fn create_room(&mut self, room_code: u64, origin: IpAddr) -> Result<(), Error> {
        self.increment_request_count(origin)?;

        {
            let mut rooms = self.rooms.lock().expect("Couldn't acquire state lock.");

            // return error if this room code has been taken
            if rooms.contains_key(&room_code) {
                return Err(Error::RoomCodeTaken);
            }
            rooms.insert(room_code, Room::default());
        }

        // spawn a thread that will remove this room after the timeout
        let timeout = *self.room_timeout;
        let rooms = self.rooms.clone();
        tokio::spawn(async move {
            tokio::time::sleep(timeout).await;
            rooms
                .lock()
                .expect("Couldn't acquire state lock.")
                .remove(&room_code);
        });

        Ok(())
    }

    /// Updates the contact information of a client in the room with `room_code`.
    ///
    /// - Returns [`Error::NoSuchRoomCode`] if no room with `room_code` exists.
    /// - Returns [`Error::TooManyRequests`] if the max
    /// allowable number of requests per minute is exceeded.
    pub fn update_client(
        &mut self,
        room_code: u64,
        is_creator: bool,
        endpoint: SocketAddr,
        public: bool,
        origin: IpAddr,
    ) -> Result<(), Error> {
        self.increment_request_count(origin)?;

        // get a mutable reference to the client in question.
        let mut rooms = self.rooms.lock().expect("Couldn't acquire state lock.");
        let room = rooms.get_mut(&room_code).ok_or(Error::NoSuchRoomCode)?;
        let full_contact = &mut room.get_client_mut(is_creator).contact;

        let contact = if public {
            &mut full_contact.public
        } else {
            &mut full_contact.local
        };

        // update the client's contact from `endpoint`
        match endpoint {
            SocketAddr::V4(addr) => {
                contact.v4 = Some(addr);
            }
            SocketAddr::V6(addr) => {
                contact.v6 = Some(addr);
            }
        };

        Ok(())
    }

    /// Returns this client's contact info and a
    /// [`oneshot::Receiver`] that will send the other peer's contact info
    /// once that peer is also ready.
    ///
    /// - Returns [`Error::TooManyRequests`] if the max
    /// allowable number of requests per minute is exceeded.
    pub fn set_client_done(
        &mut self,
        room_code: u64,
        is_creator: bool,
        origin: IpAddr,
    ) -> Result<(FullContact, oneshot::Receiver<FullContact>), Error> {
        self.increment_request_count(origin)?;

        let mut rooms = self.rooms.lock().expect("Couldn't acquire state lock.");
        let room = rooms.get_mut(&room_code).ok_or(Error::NoSuchRoomCode)?;

        let (tx, rx) = oneshot::channel();

        // Give the peer a contact sender.
        // Once the peer gets `set_client_done()` called,
        // they will send their own contact info via this sender.
        let peer = room.get_client_mut(!is_creator);
        peer.contact_sender = Some(tx);

        let client_contact = room.get_client(is_creator).contact;
        let peer_contact = room.get_client(!is_creator).contact;

        // if this client has a contact sender, that means
        // the peer must have given it to us. That means the peer
        // is also ready to exchange contacts.
        if room.get_client(is_creator).contact_sender.is_some() {
            // note: both of these `if let` will always pass
            if let Some(client_sender) = room.get_client_mut(is_creator).contact_sender.take() {
                if let Some(peer_sender) = room.get_client_mut(!is_creator).contact_sender.take() {
                    // exchange their info
                    client_sender
                        .send(client_contact)
                        .expect("Unrecoverable: RX dropped!");
                    peer_sender
                        .send(peer_contact)
                        .expect("Unrecoverable: RX dropped!");

                    // remove their room
                    rooms.remove(&room_code);
                }
            }
        }

        Ok((client_contact, rx))
    }

    /// Increments the request count of this IP address.
    ///
    /// Returns an [`Error::TooManyRequests`] if [`State::max_requests_per_minute`]
    /// is exceeded.
    fn increment_request_count(&mut self, ip: IpAddr) -> Result<(), Error> {
        let mut request_counts = self
            .request_counts
            .lock()
            .expect("Couldn't acquire state lock.");
        let conns_count = request_counts.entry(ip).or_insert(0);

        if *conns_count >= *self.max_requests_per_minute {
            Err(Error::TooManyRequests)
        } else {
            *conns_count += 1;
            Ok(())
        }
    }
}

/// Error while trying to update the global server state.
#[derive(Error, Debug)]
pub enum Error {
    /// No room exists with this code.
    #[error("No room exists with this code.")]
    NoSuchRoomCode,

    /// Exceeded the request per minute limit. Try again in a minute.
    #[error("Exceeded the request per minute limit. Try again in a minute.")]
    TooManyRequests,

    /// This room code is currently taken.
    #[error("This room code is currently taken.")]
    RoomCodeTaken,
}

#[cfg(test)]
mod tests {
    use super::Error;
    use super::State;
    use gday_contact_exchange_protocol::Contact;
    use gday_contact_exchange_protocol::FullContact;
    use std::{net::IpAddr, time::Duration};

    #[tokio::test]
    async fn test_general() {
        let mut state1 = State::new(100, Duration::from_secs(100));
        let mut state2 = state1.clone();

        // Origins are only used to limit requests,
        // and we're not testing that here,
        // so these are meaningless
        let origin1 = IpAddr::V4(123.into());
        let origin2 = IpAddr::V6(456.into());

        let contact1 = FullContact {
            local: Contact {
                v4: Some("1.8.3.1:2304".parse().unwrap()),
                v6: Some("[ab:41::b:43]:92".parse().unwrap()),
            },
            public: Contact {
                v4: Some("12.98.11.20:11".parse().unwrap()),
                v6: Some("[12:1::9:ab]:56".parse().unwrap()),
            },
        };

        let contact2 = FullContact {
            local: Contact {
                v4: None,
                v6: Some("[12:ef::2:55]:1000".parse().unwrap()),
            },
            public: Contact {
                v4: Some("5.20.100.50:2".parse().unwrap()),
                v6: None,
            },
        };

        const ROOM: u64 = 1234;

        // Client 1 creates a new room
        state1.create_room(ROOM, origin1).unwrap();

        // Verify that a room with the same ID
        // can't be created
        assert!(matches!(
            state2.create_room(ROOM, origin2),
            Err(Error::RoomCodeTaken)
        ));

        // Client 1 sends over their contact info
        if let Some(addr) = contact1.local.v4 {
            state1
                .update_client(ROOM, true, addr.into(), false, origin1)
                .unwrap();
        }
        if let Some(addr) = contact1.local.v6 {
            state1
                .update_client(ROOM, true, addr.into(), false, origin1)
                .unwrap();
        }
        if let Some(addr) = contact1.public.v4 {
            state1
                .update_client(ROOM, true, addr.into(), true, origin1)
                .unwrap();
        }
        if let Some(addr) = contact1.public.v6 {
            state1
                .update_client(ROOM, true, addr.into(), true, origin1)
                .unwrap();
        }

        // Client 2 sends over their contact info
        if let Some(addr) = contact2.local.v4 {
            state1
                .update_client(ROOM, false, addr.into(), false, origin2)
                .unwrap();
        }
        if let Some(addr) = contact2.local.v6 {
            state1
                .update_client(ROOM, false, addr.into(), false, origin2)
                .unwrap();
        }
        if let Some(addr) = contact2.public.v4 {
            state1
                .update_client(ROOM, false, addr.into(), true, origin2)
                .unwrap();
        }
        if let Some(addr) = contact2.public.v6 {
            state1
                .update_client(ROOM, false, addr.into(), true, origin2)
                .unwrap();
        }

        let (reported_contact1, rx1) = state1.set_client_done(ROOM, true, origin1).unwrap();

        let (reported_contact2, rx2) = state2.set_client_done(ROOM, false, origin2).unwrap();

        assert_eq!(reported_contact1, contact1);
        assert_eq!(reported_contact2, contact2);

        assert_eq!(rx1.await.unwrap(), contact2);
        assert_eq!(rx2.await.unwrap(), contact1);
    }

    #[tokio::test]
    async fn test_request_limit() {
        let mut state1 = State::new(100, Duration::from_secs(100));
        let mut state2 = state1.clone();

        let origin1 = IpAddr::V4(123.into());
        let origin2 = IpAddr::V4(456.into());

        // 100 requests
        for i in 1..=100 {
            state1.create_room(i, origin1).unwrap();

            // unrelated requests that shouldn't hit limit
            state2.create_room(i + 1000, origin2).unwrap();
        }

        // 101th request should hit limit
        assert!(matches!(
            state2.create_room(101, origin1),
            Err(Error::TooManyRequests)
        ));
    }

    #[tokio::test]
    async fn test_room_timeout() {
        let mut state1 = State::new(100, Duration::from_millis(10));
        let mut state2 = state1.clone();

        let origin1 = IpAddr::V4(123.into());
        let origin2 = IpAddr::V4(456.into());

        let example_endpoint = "12.213.31.13:342".parse().unwrap();

        const ROOM: u64 = 1234;

        state1.create_room(ROOM, origin1).unwrap();

        // Confirm this room is taken
        assert!(matches!(
            state2.create_room(ROOM, origin2),
            Err(Error::RoomCodeTaken)
        ));

        // confirm that this room works
        state2
            .update_client(ROOM, false, example_endpoint, true, origin2)
            .unwrap();

        // wait for the room to time out
        tokio::time::sleep(Duration::from_millis(20)).await;

        // confirm this room has been removed
        let result = state2.update_client(ROOM, false, example_endpoint, false, origin2);
        assert!(matches!(result, Err(Error::NoSuchRoomCode)))
    }
}
