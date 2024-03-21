#![warn(clippy::all)]
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
    /// The known private and public socket addresses of this client
    contact: FullContact,
    /// - `None` if the client is still sending their contact info
    /// - `Some` if the client is done sending their contact info.
    ///
    /// Once the other peer is also done, this channel sends
    /// the other peer's contact info.
    peer_contact: Option<oneshot::Sender<FullContact>>,
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
#[derive(Clone, Debug)]
pub struct State {
    /// Maps room_id to rooms
    rooms: Arc<Mutex<HashMap<u64, Room>>>,

    /// Maps IP addresses to the number of requests they sent this minute
    request_counts: Arc<Mutex<HashMap<IpAddr, u32>>>,

    /// Maximum number of requests an IP address can
    /// send per minute before they're rejected.
    max_requests_per_minute: Arc<u32>,

    /// Seconds before a newly created room is deleted
    room_timeout: Arc<u64>,
}

impl State {
    /// Creates a new `State` with the given config settings
    pub fn new(max_requests_per_minute: u32, room_timeout: u64) -> Self {
        let this = Self {
            rooms: Arc::default(),
            request_counts: Arc::default(),
            max_requests_per_minute: Arc::new(max_requests_per_minute),
            room_timeout: Arc::new(room_timeout),
        };

        // spawn a backround thread that clears `request_counts` every minute
        let cloned_self = this.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(60));
            loop {
                interval.tick().await;
                cloned_self.request_counts.lock().unwrap().clear();
            }
        });

        this
    }

    /// Creates a new room, and returns it's room code.
    ///
    /// - Returns [`Error::TooManyRequests`] if the max
    /// allowable number of requests per minute is exceeded.
    pub fn create_room(&mut self, room_code: u64, origin: IpAddr) -> Result<u64, Error> {
        self.increment_request_count(origin)?;

        let mut rooms = self.rooms.lock().unwrap();

        // return error if this room code has been taken
        if rooms.contains_key(&room_code) {
            return Err(Error::RoomCodeTaken);
        }
        rooms.insert(room_code, Room::default());

        // create a thread that will remove this room after the timeout
        let timeout = Duration::from_secs(*self.room_timeout);
        let cloned_self = self.clone();
        tokio::spawn(async move {
            tokio::time::sleep(timeout).await;
            cloned_self.rooms.lock().unwrap().remove(&room_code);
        });

        Ok(room_code)
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

        let mut rooms = self.rooms.lock().unwrap();
        let room = rooms.get_mut(&room_code).ok_or(Error::NoSuchRoomCode)?;
        let full_contact = &mut room.get_client_mut(is_creator).contact;

        let contact = if public {
            &mut full_contact.public
        } else {
            &mut full_contact.private
        };

        match endpoint {
            SocketAddr::V6(addr) => {
                contact.v6 = Some(addr);
            }
            SocketAddr::V4(addr) => {
                contact.v4 = Some(addr);
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

        let mut rooms = self.rooms.lock().unwrap();
        let room = rooms.get_mut(&room_code).ok_or(Error::NoSuchRoomCode)?;

        let (tx, rx) = oneshot::channel();

        room.get_client_mut(is_creator).peer_contact = Some(tx);

        let client_info = room.get_client(is_creator).contact;
        let peer_info = room.get_client(!is_creator).contact;

        // if both peers are waiting for each others' contact info
        if room.get_client(is_creator).peer_contact.is_some()
            && room.get_client(!is_creator).peer_contact.is_some()
        {
            if let Some(client_sender) = room.get_client_mut(is_creator).peer_contact.take() {
                if let Some(peer_sender) = room.get_client_mut(!is_creator).peer_contact.take() {
                    // exchange their info
                    // don't care about error, since nothing critical happens
                    // if the receiver has been dropped.
                    let _ = client_sender.send(peer_info);
                    let _ = peer_sender.send(client_info);

                    // remove their room
                    rooms.remove(&room_code);
                }
            }
        }

        Ok((client_info, rx))
    }

    /// Increments the request count of this IP address.
    /// Returns a [`Error::TooManyRequests`] if [`State::max_requests_per_minute`]
    /// is exceeded.
    fn increment_request_count(&mut self, ip: IpAddr) -> Result<(), Error> {
        let mut request_counts = self.request_counts.lock().unwrap();
        let conns_count = request_counts.entry(ip).or_insert(0);

        if *conns_count > *self.max_requests_per_minute {
            Err(Error::TooManyRequests)
        } else {
            *conns_count += 1;
            Ok(())
        }
    }
}

#[derive(Error, Debug)]
pub enum Error {
    #[error("No room exists with this ID.")]
    NoSuchRoomCode,

    #[error("Exceeded the request limit. Try again in a minute.")]
    TooManyRequests,

    #[error("This room ID is currently in use.")]
    RoomCodeTaken,
}
