fn main() -> std::io::Result<core::convert::Infallible> {
    let socket = std::net::UdpSocket::bind("0.0.0.0:6910")?;
    socket.set_broadcast(true)?;

    syfala_jack::client::start(
        &socket,
        "255.255.255.255:6910".parse().unwrap(),
        core::time::Duration::from_millis(250),
    )
}
