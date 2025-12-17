fn main() -> std::io::Result<core::convert::Infallible> {
    let socket = std::net::UdpSocket::bind("0.0.0.0:6910")?;

    const RB_LEN: core::time::Duration = core::time::Duration::from_secs(4);
    const DELAY: core::time::Duration = core::time::Duration::ZERO;

    syfala_jack::server::start(
        &socket,
        syfala_net::AudioConfig::new(1.try_into().unwrap(), 16.try_into().unwrap()),
        RB_LEN,
        DELAY
    )
}
