fn main() -> std::io::Result<core::convert::Infallible> {
    let socket = std::net::UdpSocket::bind("0.0.0.0:6910")?;
    socket.set_broadcast(true)?;

    const DEFAULT_RB_LENGTH: core::time::Duration = core::time::Duration::from_secs(4);
    const DEFAULT_DELAY: core::time::Duration = core::time::Duration::ZERO;

    syfala_jack::client::start(
        &socket,
        "255.255.255.255:6910".parse().unwrap(),
        core::time::Duration::from_millis(250),
        DEFAULT_RB_LENGTH,
        DEFAULT_DELAY,
    )
}
