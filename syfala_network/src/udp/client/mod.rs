//! Client-side UDP network implementation.
//!
//! This module provides a thin UDP transport layer for clients implementing
//! the `syfala_proto` message model. It handles serialization, deserialization, and
//! basic receive loops, while delegating all protocol logic and state management to
//! user-provided callbacks.

use core::{convert::Infallible, net::SocketAddr};

#[cfg(feature = "generic")]
pub mod generic;

/// A UDP server.
///
/// This type encapsulates a UDP socket, used to communicate with one or more servers.
///  It provides facilities for sending messages to servers, but deliberately
/// does **not** expose a public receive API.
///
/// Message reception is driven through the [`ClientState`] trait, which defines
/// the client's receive loop and callback behavior.
///
/// The client itself is agnostic to whether messages are sent via unicast,
/// multicast, or broadcast addresses.
#[derive(Debug)]
pub struct ClientSocket<T> {
    sock: T,
}

impl<T> ClientSocket<T> {
    /// Creates a new server backed by the given UDP socket.
    #[inline(always)]
    pub fn new(sock: T) -> Self {
        Self { sock }
    }
}

impl<T: crate::SyncUdpSock> ClientSocket<T> {
    #[inline]
    pub fn send_raw_packet(&self, bytes: &[u8], dest_addr: SocketAddr) -> std::io::Result<()> {
        self.sock.send(bytes, dest_addr)
    }

    /// Serializes and sends a client message to the specified destination address.
    ///
    /// The message is encoded using [`postcard`] into the provided buffer and then
    /// sent as a single UDP datagram.
    ///
    /// The destination address may be unicast, multicast, or broadcast.
    #[inline(always)]
    pub fn send_msg(
        &self,
        message: syfala_proto::message::Client,
        server_addr: SocketAddr,
        buf: &mut [u8],
    ) -> std::io::Result<()> {
        crate::client_message_encode(message, buf)
            .map_err(crate::postcard_to_io_err)
            .and_then(|s| self.send_raw_packet(s, server_addr))
    }

    fn set_recv_timeout(&self, timeout: Option<core::time::Duration>) -> std::io::Result<()> {
        self.sock.set_recv_timeout(timeout)
    }

    /// Receives and deserializes a server message from the underlying socket.
    ///
    /// On success, returns the sender’s socket address and an optional decoded
    /// protocol message.
    ///
    /// If a datagram is received but cannot be parsed as a valid protocol message,
    /// the `Option` will be `None`.
    #[inline(always)]
    fn recv<'a>(
        &self,
        buf: &'a mut [u8],
    ) -> std::io::Result<(
        SocketAddr,
        std::time::Instant,
        Option<(syfala_proto::message::Server, &'a [u8])>,
    )> {
        self.sock.recv(buf).map(|(n, server, timestamp)| {
            let buf = &buf[..n];

            (server, timestamp, crate::server_message_decode(buf).ok())
        })
    }

    #[inline]
    pub fn start_discovery_beacon(
        &self,
        period: core::time::Duration,
        dest_addr: SocketAddr,
    ) -> std::io::Result<core::convert::Infallible> {
        // TODO: calculate the payload size and allocate exactly that
        // This is currently an experimentat feature of postcard
        let mut disc_packet_buf = std::io::Cursor::new([0; 2000]);

        // we encode our discovery message only once
        crate::client_message_encode(
            crate::proto::message::Client::Discovery,
            &mut disc_packet_buf,
        )
        .map_err(crate::postcard_to_io_err)?;

        let payload_size = usize::try_from(disc_packet_buf.position()).unwrap();

        let buf = &disc_packet_buf.get_ref()[..payload_size];

        loop {
            let res = self.send_raw_packet(buf, dest_addr);

            match res {
                Err(e) if crate::io_err_is_timeout(e.kind()) => continue,
                Err(e) => return Err(e),
                _ => (),
            };

            std::thread::sleep(period);
        }
    }
}

/// Encapsulates client-side protocol state and message handling.
///
/// Implementors of this trait define how the client reacts to incoming server
/// messages. The provided [`start`](ClientState::start) method runs a blocking receive
/// loop and dispatches messages to [`on_message`](ClientState::on_message).
///
/// This design allows applications to cleanly separate networking concerns from
/// higher-level protocol logic.
pub trait Client {
    /// Called on every received datagram.
    ///
    /// The `message` parameter is `None` if the datagram could not be decoded as a
    /// valid protocol message.
    fn on_message(
        &mut self,
        client: &ClientSocket<impl crate::SyncUdpSock>,
        server_addr: core::net::SocketAddr,
        timestamp: std::time::Instant,
        message: Option<(syfala_proto::message::Server, &[u8])>,
    ) -> std::io::Result<()>;

    fn on_timeout(&mut self, client: &ClientSocket<impl crate::SyncUdpSock>) -> std::io::Result<()>;

    /// Starts the client receive loop
    ///
    /// This function blocks indefinitely, receiving datagrams and invoking
    /// [`on_message`](ClientState::on_message) for each one.
    ///
    /// The function only returns if a non-recoverable I/O error occurs.
    fn start(&mut self, client: &ClientSocket<impl crate::SyncUdpSock>) -> std::io::Result<Infallible> {
        let mut buf = [0; 5000];

        loop {
            let res = client.recv(&mut buf);

            // don't return on timeout errors...
            match res {
                Ok((addr, timestamp, maybe_msg)) => {
                    self.on_message(client, addr, timestamp, maybe_msg)?
                }
                Err(e) if crate::io_err_is_timeout(e.kind()) => self.on_timeout(client)?,
                Err(e) => return Err(e),
            };
        }
    }
}

/// Type alias representing the `Inactive` IO state for a given client context.
///
/// Resolves to the associated `IOInactive` type of the `ClientContext`.
pub type Inactive<Cx> = <Cx as ClientContext>::IOInactive;

/// Type alias representing the `StartPending` IO state for a given client context.
///
/// Resolves to the associated `IOStartPending` type of the `Inactive` state.
pub type StartPending<Cx> = <Inactive<Cx> as IOInactiveContext>::IOStartPending;

/// Type alias representing the `Active` IO state for a given client context.
///
/// Resolves to the associated `IOActive` type of the `StartPending` state.
pub type Active<Cx> = <StartPending<Cx> as IOStartPendingContext>::IOActive;

/// Type alias representing the `StopPending` IO state for a given client context.
///
/// Resolves to the associated `IOStopPending` type of the `Active` state.
pub type StopPending<Cx> = <Active<Cx> as IOActiveContext>::IOStopPending;

/// Represents the global client context which contains callbacks called on various
/// client-size events
///
/// This trait is implemented by the "application layer" client object, and provides:
/// - The ability to handle new server connections, and return a nother callback manager
/// for said connection
pub trait ClientContext {
    /// A newly connected server with inactive IO.
    type IOInactive: IOInactiveContext<Context = Self>;

    /// Invoked when a new server connection is requested.
    ///
    /// Returns:
    /// - `Ok(IOInactive)` if the connection is accepted, allowing further IO state transitions
    /// - `Err(Error)` if the connection is rejected or fails
    fn connect(
        &mut self,
        addr: core::net::SocketAddr,
        stream_formats: syfala_proto::format::StreamFormats,
    ) -> Result<Self::IOInactive, syfala_proto::message::Error>;

    fn unknown_message(
        &mut self,
        addr: core::net::SocketAddr,
    );
}

/// "Typestate" representing a server with inactive IO.
///
/// Provides a method to poll whether the application wishes to start IO,
/// returning a `StartPending` state if so.
pub trait IOInactiveContext: Sized {
    /// The parent client context associated with this inactive state.
    type Context;

    /// The typestate representing a pending IO start request.
    type IOStartPending: IOStartPendingContext<Context = Self::Context>;

    /// Polls whether the application requests starting IO.
    ///
    /// - Returns `Ok(IOStartPending)` if a start request was made
    /// - Returns `Err(Self)` if no request was made, leaving the state unchanged
    fn poll_start_io(self, cx: &mut Self::Context) -> Result<Self::IOStartPending, Self>;
}

/// "Typestate" representing a server whose IO start request is pending.
///
/// Provides methods to handle the server’s response to the start request.
pub trait IOStartPendingContext: Sized {
    /// The parent client context associated with this state.
    type Context: ClientContext;

    /// The typestate representing an active IO session.
    type IOActive: IOActiveContext<Context = Self::Context>;

    /// Called when the server acknowledges the start request successfully.
    ///
    /// Returns the `Active` IO typestate.
    fn start_io(self, cx: &mut Self::Context) -> Self::IOActive;

    /// Called when the server permanently refuses the start request.
    ///
    /// Returns to the `Inactive` state, allowing the client to retry later.
    fn start_io_refused(self, cx: &mut Self::Context) -> <Self::Context as ClientContext>::IOInactive;

    /// Called when the server reports a temporary failure to start IO.
    ///
    /// The implementation may perform retries or log diagnostics. The current state
    /// remains in `StartPending`.
    fn start_io_failed(&mut self, cx: &mut Self::Context);
}

/// "Typestate" representing a server with active IO.
///
/// Provides methods to handle audio messages and to poll for stop requests.
pub trait IOActiveContext: Sized {
    /// The parent client context associated with this state.
    type Context;

    /// The typestate representing a pending IO stop request.
    type IOStopPending: IOStopPendingConxtext<Context = Self::Context>;

    /// Called when an audio message is received from the server.
    ///
    /// - `timestamp` is the time the packet was received
    /// - `header` is the audio message header
    /// - `data` is the raw audio payload
    ///
    /// The application can process, store, or forward the audio as needed.
    fn on_audio(
        &mut self,
        cx: &mut Self::Context,
        timestamp: std::time::Instant,
        header: syfala_proto::AudioMessageHeader,
        data: &[u8],
    );

    /// Polls whether the application requests stopping the active IO.
    ///
    /// - Returns `Ok(IOStopPending)` if a stop request was made
    /// - Returns `Err(Self)` if no stop request was made, leaving the state unchanged
    fn poll_stop_io(self, cx: &mut Self::Context) -> Result<Self::IOStopPending, Self>;
}

/// "Typestate" representing a server whose IO stop request is pending.
///
/// Provides methods to handle the server’s response to the stop request.
pub trait IOStopPendingConxtext: Sized {
    /// The parent client context associated with this state.
    type Context: ClientContext;

    /// Called when the server acknowledges the stop request successfully.
    ///
    /// Returns to the `Inactive` state.
    fn stop_io(self, cx: &mut Self::Context) -> <Self::Context as ClientContext>::IOInactive;

    /// Called when the server permanently refuses the stop request.
    ///
    /// Returns to the `Active` state, leaving IO running.
    fn stop_io_refused(self, cx: &mut Self::Context) -> Active<Self::Context>;

    /// Called when the server reports a temporary failure to stop IO.
    ///
    /// The implementation may perform retries or log diagnostics. The current state
    /// remains in `StopPending`.
    fn stop_io_failed(&mut self, cx: &mut Self::Context);
}


// Note: Comments that should be logs are marked with (*)

/// Represents the IO state machine for a connected server.
///
/// This enum wraps the different typestate objects representing:
/// - Inactive IO
/// - Pending start request
/// - Active IO
/// - Pending stop request
///
/// All state transitions are driven by incoming messages or application requests.
enum ServerIOState<Cx: ClientContext + ?Sized> {
    Inactive(Inactive<Cx>),
    PendingStart(StartPending<Cx>),
    Active(Active<Cx>),
    PendingStop(StopPending<Cx>),
}
