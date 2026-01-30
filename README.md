# Syfala Protocol

A simple network protocol for audio endpoint discovery, and communication of packed, uncompressed, interleaved audio streams.

Requires the latest stable version of the [Rust](https://rust-lang.org/tools/install/) compiler.

Currently supports UDP for discovery, audio streaming, and control messages.

Multiple implementations of this protocol, for different backends, can be found in this repository.

## Crates

 - [`syfala_utils`](syfala_utils) A bunch of small utilities used throughout the rest of the crates in this repository.
 - [`syfala_proto`](syfala_proto) Type-based definitions of the protocol and it's communication model.
 - [`syfala_network`](syfala_network) Basic types implementing the protocol for different network protocols (currently only UDP). Also contains a typestate-based implementation, to easily implemnent the protocol for
 different backends.

### Backends

 - [`syfala_jack`](syfala_jack) For the JACK backend.
 - [`syfala_coreaudio`](syfala_coreaudio) For the coreaudio backend

Beware of clock drift, an issue unaddressed in all of the backends.

You can run:

```shell
cargo doc --open -p <package_name>
```

(replacing <package_name> with one of the crates listed above)

To view the documentation for any of the crates above.

#### TODOs

- CoreAudio Backend, JACK Backend, AXI(lite) backend.
- Write Generic UDP _server_
- Generic UDP client has some missing functionality as well.
- Add tests, expecially to syfala_util, and syfala_net.

Some more TODOs (marked with `TODO`) in the source code.

#### Future improvements

Currently a UDP-only, fully synchronous (blocking), single-threaded design has been chosen for network clients and servers. While this provides some level of simplicity. It is definitely not scalable, and complexifies scheduling network tasks with application layer tasks. For example, in CoreAudio, the driver must tell the server when is IO supposed to start/stop, this means that our UDP server must also be capable of receving messages from an external application i.e. another thread, through channels and message passing, on top of also constanly listening to the network. More importantly, it requires dealing with UDP's imperfections/non-guarantees (implementing manual retransmission/retry mechanisms, manual connection timeout mechanisms etc...) for (the already infrequent) control messages, which are better sent/received over TCP.

Ideally, one should design a client and a server where all control-plane communication (Connection, configuration, stream format declaration/acceptance, starting/stopping audio IO, potentially parameters/toggles) must be performed over TCP by an asynchronous, non-blocking runtime, like `tokio`, that would own and manage all network protocol-related state. And then, more importantly, only when IO is ready to start, delegating audio IO to a seperate _fully blocking/synchronous UDP threads_ (usually just one for all outgoing audio, and one for all incoming audio is enough) that would then send/receive audio data to/from the relevant real-time audio threads of your audio system of choice. All of this must be done while making sure communication between those components is efficent, and doesn't stall the rest of the system. This entails using the following tools:

 - Real-time, inter-thread, SPSC ring buffers, like `rtrb`, between the UDP thread(s) and the RT audio threads, as well as between the TCP threads/tasks and the UDP thread(s). We already provide utilities for using those in `syfala_utils`
 - Flexible, async-compatible, MPSC, or MPMC channels for communication between the application layer to the TCP Threads/Tasks. By flexible, I mean that allow sending and receiving data both with and without async, so as to not enforce a specific architecture on external applications (Can't use async inside CoreAudio driver routines...). `tokio`'s channels in the `sync` module seem to meet those requirements.
 - Careful design of the TCP threads/tasks (e.g. clients spawning at least one task per server, servers spawn at least one task per client, etc...). Listening to both application (channels...) _and_ network (TCP stream socket...) messages at the same time, (this is where you should use `tokio::select!`). Handling those messages, and eventually sending outgoing TCP messages, as quickly as possible, without blocking other tasks.

It took quite some time to come to those conclusions. Specifically, integrating our protocol to CoreAudio's HAL Plugin API is what mandates such flexibility, as CoreAudio, while very powerful and feature-complete, enforces strong architectural constraints while making very little, or very vague guarantees (about threading and state management, mostly...) to the developer.