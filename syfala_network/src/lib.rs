pub mod udp;

#[inline(always)]
fn postcard_to_io_err(e: postcard::Error) -> std::io::Error {
    match e {
        postcard::Error::DeserializeUnexpectedEnd => std::io::ErrorKind::UnexpectedEof.into(),
        _ => std::io::ErrorKind::Other.into(),
    }
}

#[inline(always)]
fn io_err_is_timeout(e: std::io::ErrorKind) -> bool {
    use std::io::ErrorKind::*;
    [WouldBlock, TimedOut].contains(&e)
}

pub trait Node {
    fn capabilities(&self) -> (&[syfala_proto::Format], &[syfala_proto::Format]);
}

pub trait NetworkNode: Node {

    fn local_addr(&self) -> core::net::SocketAddr;
    fn dest_discovery_addr(&self) -> Option<core::net::SocketAddr>;
    fn on_audio_packet_recv(&mut self, audio: syfala_proto::AudioPacket<'_>);
}