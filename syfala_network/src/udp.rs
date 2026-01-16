use core::{convert::Infallible, mem, net::SocketAddr};
use std::collections::hash_map::Entry;
use syfala_proto::DeviceCapabilities;

type ServerMap<V> = std::collections::HashMap<SocketAddr, V>;

#[derive(Debug)]
pub enum IOStateCommand {
    StartIO,
    StopIO,
}

#[derive(Debug)]
pub enum ApplicationCommand {
    IOState {
        addr: SocketAddr,
        command: IOStateCommand,
        confirmation: oneshot::Sender<bool>,
    },
}

#[derive(Debug)]
struct Socket {
    sock: std::net::UdpSocket,
}

impl Socket {
    #[inline(always)]
    fn new(sock: std::net::UdpSocket) -> Self {
        Self { sock }
    }

    #[inline(always)]
    fn send(
        &self,
        message: &syfala_proto::Message<'_>,
        dest_addr: SocketAddr,
        buf: &mut [u8],
    ) -> std::io::Result<()> {
        let left = postcard::to_slice(message, buf)
            .map_err(crate::postcard_to_io_err)?
            .len();

        let ser_len = buf.len().strict_sub(left);

        let res = self.sock.send_to(&mut buf[..ser_len], dest_addr);

        res.and_then(|n| {
            (n == ser_len)
                .then_some(())
                .ok_or(std::io::ErrorKind::FileTooLarge.into())
        })
    }

    #[inline(always)]
    fn recv<'a>(
        &self,
        buf: &'a mut [u8],
    ) -> std::io::Result<(SocketAddr, Option<syfala_proto::Message<'a>>)> {
        self.sock.recv_from(buf).map(|(n, server)| {
            let buf = &buf[..n];
            (server, postcard::from_bytes(buf).ok())
        })
    }
}

#[derive(Debug)]
pub struct Options {
    pub local_addr: SocketAddr,
    pub discovery_dest_addr: Option<SocketAddr>,
}

#[derive(Debug)]
pub struct Node {
    event_rx: std::sync::mpsc::Receiver<ApplicationCommand>,
    sock: Socket,
}

#[derive(Debug, Default)]
enum PeerState<D> {
    #[default]
    Inactive,
    PendingActivation(oneshot::Sender<bool>),
    Active(D),
    PendingDeactivation(oneshot::Sender<bool>),
}

struct PeerEntry<D> {
    pub last_seen: std::time::Instant,
    pub caps: DeviceCapabilities,
    pub state: PeerState<D>,
}

impl<D> PeerEntry<D> {
    #[inline(always)]
    fn new(caps: DeviceCapabilities) -> Self {
        Self {
            last_seen: std::time::Instant::now(),
            caps,
            state: PeerState::Inactive,
        }
    }
}

fn handle_msg<D>(
    msg: ServerMessage,
    mut server_entry: Entry<SocketAddr, D>,
    mut connect: impl FnMut(SocketAddr, &DeviceCapabilities),
    mut disconnect: impl FnMut(SocketAddr),
    mut on_server_connect: impl FnMut(&DeviceCapabilities) -> D,
) {
    let &server = server_entry.key();
    if let Entry::Occupied(e) = &mut server_entry {
        e.get_mut().last_seen = std::time::Instant::now();
    }

    use syfala_proto::{IOStateCommandMessage, IOStateCommandResult, ServerControlMessage};
    match msg {
        ServerMessage::Connect(caps) => match server_entry {
            Entry::Occupied(e) => {
                let p = e.into_mut();
                if &p.caps != &caps {
                    println!("Server at {server} reconnected with new layout!");
                    connect(server, &caps);
                    let _ = mem::replace(p, PeerEntry::new(caps));
                }
            }
            Entry::Vacant(e) => {
                e.insert(PeerEntry::new(caps));
            }
        },
        ServerMessage::Control(ctrl) => match ctrl {
            ServerControlMessage::HeartBeat => (),
            ServerControlMessage::IOStateMessageResult(res) => match res {
                IOStateCommandMessage::StartIO(res) => match res {
                    IOStateCommandResult::Failure => {
                        server_entry.and_modify(|p| {
                            if let PeerState::PendingActivation(m) = mem::take(&mut p.state) {
                                let _ = m.send(false);
                            } else {
                                println!("WARN: StartIOFailed from server not pending activation");
                            }
                        });
                    }
                    IOStateCommandResult::Success => {
                        server_entry.and_modify(|p| {
                            let s = &mut p.state;
                            match mem::take(s) {
                                PeerState::PendingActivation(sender) => {
                                    let _ = sender.send(true);
                                    let inputs: Option<Box<[ServerConsumer<C>]>> = p
                                        .caps
                                        .inputs
                                        .iter()
                                        .map(|f| (&mut create_istream)(f).map(ServerConsumer::new))
                                        .collect();
                                    let outputs: Option<Box<[P]>> = p
                                        .caps
                                        .outputs
                                        .iter()
                                        .map(|f| (&mut create_ostream)(f))
                                        .collect();
                                    if let (Some(inputs), Some(outputs)) = (inputs, outputs) {
                                        *s = PeerState::Active { inputs, outputs }
                                    };
                                }
                                _ => println!(
                                    "WARN: StartIOSuccess from server not pending activation"
                                ),
                            };
                        });
                    }
                },
                IOStateCommandMessage::StopIO(res) => match res {
                    IOStateCommandResult::Failure => {
                        server_entry.and_modify(|p| {
                            if let PeerState::PendingDeactivation(m) = mem::take(&mut p.state) {
                                let _ = m.send(false);
                            } else {
                                println!("WARN: StopIOFailed from server not pending activation");
                            }
                        });
                    }
                    IOStateCommandResult::Success => {
                        server_entry.and_modify(|p| {
                            if let PeerState::PendingDeactivation(m) = mem::take(&mut p.state) {
                                let _ = m.send(true);
                            }
                        });
                    }
                },
            },
            ServerControlMessage::Disconnect => {
                if let Entry::Occupied(e) = server_entry {
                    let (addr, _dat) = e.remove_entry();
                    disconnect(addr)
                }
            }
        },
        ServerMessage::Audio(a) => {
            if let Entry::Occupied(p) = server_entry {
                if let PeerState::Active { outputs, .. } = &mut p.into_mut().state {
                    let syfala_proto::AudioPacket { stream_idx, data } = a;
                    if let Some(s) = outputs.get_mut(usize::try_from(stream_idx).unwrap()) {
                        let syfala_proto::AudioData { byte_index, bytes } = data;
                        s.process(byte_index, bytes);
                    }
                }
            }
        }
    };
}

fn handle_io_state_command<D>(
    cmd: syfala_proto::IOStateCommandMessage,
    confirmation: oneshot::Sender<bool>,
    server_state: &mut PeerState<D>,
) {
    match cmd {
        syfala_proto::IOStateCommandMessage::StartIO(_) => {
            if !matches!(server_state, PeerState::Inactive) {
                println!("WARN: Recvd start IO event for non inactive server");
            }

            *server_state = PeerState::PendingActivation(confirmation);
        }
        syfala_proto::IOStateCommandMessage::StopIO(_) => {
            if !matches!(server_state, PeerState::Active { .. }) {
                println!("WARN: Recvd stop IO event for inactive server");
            }

            *server_state = PeerState::PendingDeactivation(confirmation);
        }
    }
}

#[derive(Debug)]
struct ServerConsumer<P> {
    pub byte_index: u64,
    pub consumer: P,
}

impl<P> ServerConsumer<P> {
    #[inline(always)]
    pub const fn new(consumer: P) -> Self {
        Self {
            consumer,
            byte_index: 0,
        }
    }
}

impl Node {
    #[inline(always)]
    pub fn new(
        options: Options,
    ) -> std::io::Result<(std::sync::mpsc::SyncSender<ApplicationCommand>, Self)> {
        let (event_tx, event_rx) = std::sync::mpsc::sync_channel(256);

        let sock = std::net::UdpSocket::bind(options.local_addr)?;
        sock.set_read_timeout(Some(core::time::Duration::from_micros(100)))?;

        if let Some(core::net::IpAddr::V4(ip)) =
            options.discovery_dest_addr.as_ref().map(SocketAddr::ip)
            && ip.is_broadcast()
        {
            sock.set_broadcast(true)?;
        }
        let sock = Socket::new(sock);
        Ok((event_tx, Self { event_rx, sock }))
    }

    pub fn start<D>(
        self,
        mut connect: impl FnMut(SocketAddr, &DeviceCapabilities),
        mut disconnect: impl FnMut(SocketAddr),
        mut on_peer_connect: impl FnMut(&DeviceCapabilities) -> D,
    ) -> std::io::Result<Infallible> {
        let mut servers = ServerMap::<D>::new();

        let mut buf = [0; 5000];

        loop {
            let res = sock.recv(buf);

            if res
                .as_ref()
                .map_err(std::io::Error::kind)
                .is_err_and(crate::io_err_is_timeout)
            {
                // we have exhausted all events
                break;
            }

            let (server, maybe_msg) = res?;
            let Some(msg) = maybe_msg else {
                println!("Received unknown packet from {server}");
                continue;
            };

            let server_entry = servers.entry(server);
            handle_msg(
                msg,
                server_entry,
                &mut connect,
                &mut disconnect,
                &mut create_istream,
                &mut create_ostream,
            );

            while let Ok(e) = self.event_rx.try_recv() {
                let ApplicationCommand::IOState {
                    addr,
                    command,
                    confirmation,
                } = e;
                if let Some(p) = servers.get_mut(&addr) {
                    handle_io_state_command(command, confirmation, &mut p.state);
                }
            }

            for (&addr, server) in &mut servers {
                if let PeerState::Active { inputs, .. } = &mut server.state {
                    for (i, consumer) in inputs.iter_mut().enumerate() {
                        let bytes = consumer.consumer.get_bytes();
                        let send_res = self.sock.send(
                            &syfala_proto::ClientMessage::Audio(syfala_proto::AudioPacket {
                                stream_idx: i.try_into().unwrap(),
                                data: syfala_proto::AudioData {
                                    byte_index: consumer.byte_index,
                                    bytes,
                                },
                            }),
                            addr,
                            &mut buf,
                        );

                        if send_res
                            .as_ref()
                            .map_err(std::io::Error::kind)
                            .is_err_and(crate::io_err_is_timeout)
                        {
                            continue;
                        }

                        send_res?
                    }
                }
            }
        }
    }
}
