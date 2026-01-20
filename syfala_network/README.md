# `syfala_network`

This crate implements the model defined in [`syfala_proto`](../syfala_proto), that enables sending
and receiving the messages defined in it over UDP sockets. It also provides simple traits to listen
to said sockets, and run callbacks for each message type.