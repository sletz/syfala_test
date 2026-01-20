# `syfala_proto`

Implementation of a simple protocol for real-time audio communication and discovery.

The main idea is the following: A node in the network is a client, or a server.
Typically, a server is the firmware on external hardware devices, and a client is the
driver on consumer devices.

The entire protocol is modelled by a set of messages, sent between endpoints.

More details can be found in the rustdoc-generated docs.

## Connection/Discovery Messages

Connection messages are used to by endpoints to establish connections between other endpoints.
e.g. When a server receives a `Client::Connect` message from an
unknown client, if it accepts, it may send back to the client a
`Server::Connect` message. A "connection" is then established.

Servers indicate their stream formats in their connection messages. Those _do not_ change for
the lifetime of a connection. And are the formats of audio streams expected when IO is running.
If, for some reason, a client is incompatible with any of the stream formats, it must refuse to
connect.

Clients or servers may send said conection messages over broadcast addresses if they wish to
be discovered by other endpoints in a network.

## Control messages

Control messages, are infrequent, miscellaneous messages endpoints send to perform various
actions. In our case, the only kind of control message implemented are those used to request to
start and stop IO.

## Audio messages

When a connection is active, and IO is active, endpoints must send and expect to receive
audio messages.

Audio messages contain raw audio byte data, as well as indices to handle packet
loss/reordering. The index of the stream the packet belongs to is also provided for receiving
endpoints to know how to dispatch and encode that message.


The types in this crate already implement `serde`'s `Serialize` and `Deserialize` traits, for the user to conveniently plug into other `serde` backends.