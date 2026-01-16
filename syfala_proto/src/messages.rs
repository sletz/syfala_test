//! The main idea is this: Components are organized in a tree hierarchy:
//!
//! nodes -> peers -> streams:
//!  - A node manages manages connections to peers (i.e. _other nodes_)
//!  - A peer represents a connection to a remote peer (a hardware device), and
//! manages a set of streams.
//!  - A stream represents a handle to send to or receive audio data.
//!
//! One could imagine adding new component types (and the new corresponding message types)
//! owned by devices or streams, that manage parameters and controls that need to be communicated
//! with the peer nodes.

use serde::{Deserialize, Serialize};

/// Represents a chunk of raw audio data.
///
/// Note that [`byte_index`](Self::byte_index) is in _bytes_. It is up to you
/// to decode it properly according to the format agreed upon with the peer.
///
/// By extension, [`sample_bytes`](Self::sample_bytes) might have any length, including zero.
/// This means that it might not contain complete frames or samples, __or even a frame or sample
/// boundary at all__.
///
/// If [`byte_index`](Self::byte_index) is greater than the `index + sample_bytes.len()` of the
/// previous message (i.e. a packet was lost), then _all bytes from the last frame boundary to
/// the next frame boundary are considered lost_. Typically you would want to replace them with
/// silence, or use a more elaborate scheme to reduce artifacts.
///
/// if it is less (packet reordering), _the entire packet is considered void_.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct AudioData<'a> {
    pub byte_index: u64,
    #[serde(borrow)]
    pub bytes: &'a [u8],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct AudioPacket<'a> {
    pub stream_idx: u64,
    #[serde(borrow)]
    pub data: AudioData<'a>,
}

// we make sure to keep this enum flat, because, nesting the enums would make
// wire-encoded format take more space

#[derive(Debug, Clone, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum Message<'a> {
    Connect(crate::format::DeviceCapabilities),
    HeartBeat,
    Disconnect,
    StartIO,
    StopIO,
    StartIOFailed,
    StartIOSuccess,
    StopIOFailed,
    StopIOSuccess,
    LocalAudio(#[serde(borrow)] AudioPacket<'a>),
    PeerAudio(#[serde(borrow)] AudioPacket<'a>),
}