// //! Here we implement a device in our protocol

// const MTU: usize = 1440;

// pub struct Device {
//     inputs: Box<[()]>,
//     outputs: Box<[()]>,
// }

// pub struct Node {
//     us: Device,
//     peers: std::collections::HashMap<core::net::SocketAddr, Device>,
// }

fn main() -> std::io::Result<core::convert::Infallible> {
    //     let socket = std::net::UdpSocket::bind("0.0.0.0:6910")?;
    //     socket.set_broadcast(true)?;
    //     socket.set_read_timeout(Some(core::time::Duration::from_secs(1)))?;
    //     let socket = syfala_net::udp::ClientSocket::new(socket);
    //     let mut message_buf = const { [0; MTU] };

    //     let node = Node::default();

    //     loop {
    //         let res = socket.recv(&mut message_buf);

    //         if res
    //             .as_ref()
    //             .err()
    //             .map(std::io::Error::kind)
    //             .is_some_and(syfala_utils::io_err_is_timeout)
    //         {
    //             continue;
    //         }

    //         let (peer, msg) = res?;

    //         let Some(msg) = msg else {
    //             println!("received unknown message from peer: {peer}");
    //             continue;
    //         };
    //     }

    //     // let socket = std::net::UdpSocket::bind("0.0.0.0:6910")?;
    //     // socket.set_broadcast(true)?;

    //     // const RB_LEN: core::time::Duration = core::time::Duration::from_secs(4);
    //     // const DELAY: core::time::Duration = core::time::Duration::from_millis(10);

    //     // syfala_jack::client::start(
    //     //     &socket,
    //     //     "255.255.255.255:6911".parse().unwrap(),
    //     //     core::time::Duration::from_millis(250),
    //     //     |_, _| RB_LEN,
    //     //     |_, _| DELAY,
    //     // )
    loop {}
}
