//! Network implementation: all packet parsing is done here.

use super::*;

// All packets not starting with either of these are not valid as per our protocol.
// This allows us to not only identify and distinguish packets that are part of it,
// but also quickly eliminate any foreign traffic by only inspecting the first 4 bytes

// Client -> Server

// Payload: empty (for now)
const PACKET_TYPE_ID_CLIENT_DISC: [u8; 4] = *b"SyFd";
// Payload: 8 bytes (u64 timestamp) + samples (variable length (0..))
const PACKET_TYPE_ID_CLIENT_AUDIO: [u8; 4] = *b"SyFa";

// Server -> Client

// Payload: 4 bytes (channel count: u32) + 4
const PACKET_TYPE_ID_SERVER_CONFIG: [u8; 4] = *b"SyFc";

// limit packet sizes to this
// Servers should, nonetheless, still accept larger packets
const MAX_DATAGRAM_SIZE: num::NonZeroUsize = nz(1452);

/// Sends a discovery packet to `dest_addr` address using the given `socket`
/// Be sure to enable broadcasting if `dest_addr` is a broadcast address
// Discovery packets are just constant (PACKET_TYPE_ID_CLIENT_DISC) for now
#[inline]
pub fn send_discovery(
    socket: &std::net::UdpSocket,
    dest_addr: core::net::SocketAddr,
) -> io::Result<()> {
    let err = socket.send_to(&PACKET_TYPE_ID_CLIENT_DISC, dest_addr);

    if err? != PACKET_TYPE_ID_CLIENT_DISC.len() {
        Err(io::ErrorKind::Other.into())
    } else {
        Ok(())
    }
}

#[inline(always)]
fn parse_config(packet: &[u8]) -> Option<AudioConfig> {
    let payload = packet
        .split_first_chunk()
        .filter(|&(&message, _)| message == PACKET_TYPE_ID_SERVER_CONFIG)?
        .0;

    let (&n_channels, rem) = payload.split_first_chunk()?;
    let n_channels = u32::from_le_bytes(n_channels).try_into().unwrap();

    let (&buffer_size_frames, _rem) = rem.split_first_chunk()?;
    let buffer_size_frames = u32::from_le_bytes(buffer_size_frames).try_into().unwrap();

    Some(AudioConfig::new(n_channels, buffer_size_frames))
}

const SERVER_CONFIG_PACKET_LEN: usize =
    // Packet id (4 bytes) (little endian)
    PACKET_TYPE_ID_SERVER_CONFIG
        .len()
        // channel count (4 bytes, non zero) (little endian)
        .strict_add(size_of::<u32>())
        // buffer size in frames (4 bytes, non zero) (little endian)
        .strict_add(size_of::<u32>());

/// Attempts to parse a server configuration from this socket. If `None` is returned,
/// a packet has been received that wasn't a configuration packet
#[inline(always)]
pub fn try_recv_config(
    socket: &std::net::UdpSocket,
) -> io::Result<(core::net::SocketAddr, Option<AudioConfig>)> {
    let mut packet_buf = [0u8; SERVER_CONFIG_PACKET_LEN];

    let (bytes_read, peer_addr) = socket.recv_from(&mut packet_buf)?;

    Ok((peer_addr, parse_config(&packet_buf[..bytes_read])))
}

/// Allows for writing iterators of samples over the network
pub struct AudioSender<const N: usize = { MAX_DATAGRAM_SIZE.get() }> {
    // hehehe zero copy yoohoo
    scratch_buffer: arrayvec::ArrayVec<u8, N>,
}

impl<const N: usize> AudioSender<N> {
    #[inline(always)]
    pub fn new() -> Self {
        let mut scratch_buffer = arrayvec::ArrayVec::new_const();
        scratch_buffer.extend(PACKET_TYPE_ID_CLIENT_AUDIO);
        scratch_buffer.extend(0u64.to_le_bytes());

        Self {
            scratch_buffer,
        }
    }

    #[inline(always)]
    fn split(&self) -> (u64, &[u8]) {
        // the buffer always contains at least 12 bytes (id + timestamp)
        let (timestamp, sample_data) = self.scratch_buffer[4..].split_at(size_of::<u64>());
        (
            u64::from_le_bytes(timestamp.try_into().unwrap()),
            sample_data,
        )
    }

    #[inline(always)]
    fn n_stored_samples(&self) -> usize {
        self.split().1.len() / SAMPLE_SIZE
    }

    #[inline(always)]
    pub fn current_timestamp_samples(&self) -> u64 {
        self.split()
            .0
            .strict_add(self.n_stored_samples().try_into().unwrap())
    }

    #[inline(always)]
    fn n_remaining_chunk_samples(&self, chunk_size_spls: num::NonZeroUsize) -> num::NonZeroUsize {
        let chunk_size_samples = num::NonZeroU64::try_from(chunk_size_spls).unwrap();
        // Never zero, we always flush at least as soon as the buffer is full
        let max_samples_left = num::NonZeroU64::new(
            (self.scratch_buffer.remaining_capacity() / SAMPLE_SIZE)
                .try_into()
                .unwrap(),
        )
        .unwrap();

        // Never zero, we always flush at least at chunk boundaries
        let chunk_samples_left = num::NonZeroU64::new(
            chunk_size_samples
                .get()
                .strict_sub(self.current_timestamp_samples() % chunk_size_samples),
        )
        .unwrap();

        max_samples_left.min(chunk_samples_left).try_into().unwrap()
    }

    /// It is not recommended to call this function directly. using `send` provides
    /// more consistent buffer. We still provide it however, if needed.
    #[inline]
    pub fn flush(
        &mut self,
        socket: &std::net::UdpSocket,
        addr: &core::net::SocketAddr,
    ) -> io::Result<()> {
        let (timestamp, sample_data) = self.split();

        let n_samples = u64::try_from(sample_data.len() / SAMPLE_SIZE).unwrap();

        socket.send_to(self.scratch_buffer.as_slice(), addr)?;

        let _ = self.scratch_buffer.drain(4..).count();

        self.scratch_buffer
            .extend(u64::to_le_bytes(timestamp.strict_add(n_samples)));
        Ok(())
    }

    /// Trys to send the provided iterator of samples using the given `socket`.
    ///
    /// Returns `true` if data was flushed from the buffer. `false` means that not enough
    /// data was given.
    #[inline]
    pub fn send(
        &mut self,
        socket: &std::net::UdpSocket,
        addr: &core::net::SocketAddr,
        chunk_size_spls: num::NonZeroUsize,
        samples: impl IntoIterator<Item = f32>,
    ) -> io::Result<bool> {
        let mut rem = self.n_remaining_chunk_samples(chunk_size_spls);
        let mut used_network = false;

        for sample in samples {
            self.scratch_buffer.extend(sample.to_le_bytes());

            rem = if let Some(next) = num::NonZeroUsize::new(rem.get() - 1) {
                next
            } else {
                self.flush(socket, addr)?;
                used_network = true;
                self.n_remaining_chunk_samples(chunk_size_spls)
            };
        }

        Ok(used_network)
    }
}

pub enum ServerMessage<'a> {
    ClientDiscovery,
    ClientAudio {
        timestamp: u64,
        sample_bytes: &'a [u8],
    },
}

#[inline]
pub fn recv_message<'b>(
    socket: &std::net::UdpSocket,
    buf: &'b mut [u8],
) -> io::Result<(core::net::SocketAddr, Option<ServerMessage<'b>>)> {
    let (bytes_read, peer_addr) = socket.recv_from(buf)?;

    let message = buf[..bytes_read]
        .split_first_chunk()
        .and_then(|(&id, payload)| {
            if id == PACKET_TYPE_ID_CLIENT_AUDIO {
                let Some((&timestamp, sample_bytes)) = payload.split_first_chunk() else {
                    return None;
                };

                let timestamp = u64::from_le_bytes(timestamp);

                Some(ServerMessage::ClientAudio {
                    timestamp,
                    sample_bytes,
                })
            } else if id == PACKET_TYPE_ID_CLIENT_DISC {
                Some(ServerMessage::ClientDiscovery)
            } else {
                None
            }
        });

    return Ok((peer_addr, message));
}

#[inline]
pub fn send_config(
    socket: &std::net::UdpSocket,
    dest_addr: core::net::SocketAddr,
    config: AudioConfig,
) -> io::Result<()> {
    let mut packet_buf = [0u8; SERVER_CONFIG_PACKET_LEN];

    let (packet_type, rem) = packet_buf.split_first_chunk_mut().unwrap();
    *packet_type = PACKET_TYPE_ID_SERVER_CONFIG;

    let (channel_count, rem) = rem.split_first_chunk_mut().unwrap();
    *channel_count = config.n_channels().get().to_le_bytes();

    let (buffer_size, rem) = rem.split_first_chunk_mut().unwrap();
    *buffer_size = config.chunk_size_frames().get().to_le_bytes();

    assert!(rem.is_empty(), "ERROR: missing fields");

    let res = socket.send_to(&packet_buf, dest_addr);

    if res? != SERVER_CONFIG_PACKET_LEN {
        Err(io::ErrorKind::Other.into())
    } else {
        Ok(())
    }
}