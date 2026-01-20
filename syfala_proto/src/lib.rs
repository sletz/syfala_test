#![no_std]
//! Implementation of a simple protocol for real-time audio communication and discovery.
//! 
//! The main idea is the following: A node in the network is a client, or a server.
//!
//! Typically, a server is the firmware on external hardware devices, and a client is the
//! driver on consumer devices.
//! 
//! The entire protocol is modelled by a set of messages, sent between endpoints.
//! 
//! See [`message`] for more.
//! 
//! # Connection/Discovery Messages
//! 
//! Connection messages are used to by endpoints to establish connections between other endpoints.
//! e.g. When a server receives a [`Client::Connect`](message::Client::Connect) message from an
//! unknown client, if it accepts, it may send back to the client a
//! [`Server::Connect`](message::Server::Connect) message. A "connection" is then established.
//! 
//! Servers indicate their stream formats in their connection messages. Those _do not_ change for
//! the lifetime of a connection. And are the formats of audio streams expected when IO is running.
//! If, for some reason, a client is incompatible with any of the stream formats, it must refuse to
//! connect.
//! 
//! Clients or servers may send said conection messages over broadcast addresses if they wish to
//! be discovered by other endpoints in a network.
//! 
//! # Control messages
//! 
//! Control messages are infrequent, miscellaneous messages endpoints send to perform various
//! actions. In our case, the only kind of control message implemented are those to request to
//! start and stop IO.
//! 
//! # Audio messages
//! 
//! When a connection is active, and IO is active, endpoints must send and expect to receive
//! audio messages.
//! 
//! Audio messages contain raw audio byte data, as well as indices to handle packet
//! loss/reordering. The index of the stream the packet belongs to is also provided for receiving
//! endpoints to know how to dispatch and encode that message.
extern crate alloc;

pub mod format;
pub mod message;
pub use postcard;

use serde::{Serialize, Deserialize};

/// Represents a chunk of raw audio data.
///
/// Note that [`byte_index`](Self::byte_index) is in _bytes_. It is up to you
/// to decode it properly according to the format agreed upon with the peer.
///
/// By extension, [`bytes`](Self::bytes) might have any length, including zero.
/// This means that it might contain incomplete frames or samples, or might not contain, __even
/// a frame or sample boundary at all__.
///
/// If [`byte_index`](Self::byte_index) is greater than the `index + bytes.len()` of the
/// previous message (i.e. a packet was lost), then _all bytes from the last frame boundary to
/// the next frame boundary are considered lost_. Typically you would want to replace them with
/// silence, or use a more elaborate scheme to reduce artifacts.
///
/// if it is less (packet reordering), _the entire packet is considered void_.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct AudioStreamData<'a> {
    pub byte_index: u64,
    #[serde(borrow)]
    pub bytes: &'a [u8],
}

/// Like [`AudioData`], but includes the index of the stream to use:
/// 
/// - Sending clients indicate the index of the server's __output__ stream.
/// - Sending servers indicate the index of their corresponding __input__ stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct AudioData<'a> {
    pub stream_idx: usize,
    #[serde(borrow)]
    pub data: AudioStreamData<'a>,
}
