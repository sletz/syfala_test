// mod interleaver;
// use core::{cell, num};

// struct ProcessSender {
//     tx: syfala_utils::queue::WakingProducer<f32>,
//     interleaver: Box<interleaver::Interleaver<jack::AudioIn>>,
//     /// This, in conjunction with `Scope::last_frame_time`, is used to track
//     /// skipped cycles due to xruns and the like.
//     current_timestamp: cell::OnceCell<u32>,
// }

// impl ProcessSender {
//     #[inline(always)]
//     fn new(
//         tx: syfala_utils::queue::WakingProducer<f32>,
//         ports: impl IntoIterator<Item = jack::Port<jack::AudioIn>>,
//     ) -> Option<Self> {
//         interleaver::Interleaver::new(ports).map(|interleaver| Self {
//             tx,
//             interleaver,
//             current_timestamp: cell::OnceCell::new(),
//         })
//     }
// }

// impl jack::ProcessHandler for ProcessSender {
//     #[inline]
//     fn process(&mut self, _client: &jack::Client, scope: &jack::ProcessScope) -> jack::Control {
//         let idx = scope.last_frame_time();
//         let &prev_spl_idx = self.current_timestamp.get_or_init(|| idx);

//         let samples_skipped = idx
//             .checked_sub(prev_spl_idx)
//             // At the time of writing, PipeWire's JACK shim is susceptible of triggering this
//             .expect("JACK ERROR: buggy frame counter")
//             .strict_mul(self.interleaver.len().get().try_into().unwrap());

//         let available_spls = self.interleaver.interleave(scope).copied();

//         // maybe replace thisw with simething more sophisticated
//         let padding_spls = core::iter::repeat_n(0., samples_skipped.try_into().unwrap());

//         let (_wakes, n_spls_sent) = self.tx.fill_in_one_go(padding_spls.chain(available_spls));

//         // NIGHTLY: #[feature(once_cell_get_mut)]: use get_or_init_mut
//         let c = self.current_timestamp.get_mut().unwrap();
//         *c = c.strict_add(n_spls_sent.try_into().unwrap());

//         jack::Control::Continue
//     }
// }

// struct ProcessReceiver {
//     rx: syfala_utils::queue::rtrb::Consumer<f32>,
//     interleaver: Box<interleaver::Interleaver<jack::AudioOut>>,
//     current_timestamp: cell::OnceCell<u32>,
// }

// impl ProcessReceiver {
//     #[inline(always)]
//     pub fn new(
//         rx: syfala_utils::queue::rtrb::Consumer<f32>,
//         ports: impl IntoIterator<Item = jack::Port<jack::AudioOut>>,
//     ) -> Option<Self> {
//         interleaver::Interleaver::new(ports).map(|interleaver| Self {
//             interleaver,
//             current_timestamp: cell::OnceCell::new(),
//             rx,
//         })
//     }
// }

// impl jack::ProcessHandler for ProcessReceiver {
//     fn process(&mut self, _client: &jack::Client, scope: &jack::ProcessScope) -> jack::Control {
//         let idx = scope.last_frame_time();

//         let n_channels = self.interleaver.len().get();
//         let interleaved = self.interleaver.interleave(scope);

//         let slots = self.rx.slots();

//         // don't start counting until we receive our first audio sample
//         if slots == 0 {
//             return jack::Control::Continue;
//         }

//         let &prev_spl_idx = self.current_timestamp.get_or_init(|| idx);

//         let n_samples_skipped = usize::try_from(
//             idx.checked_sub(prev_spl_idx)
//                 // At the time of writing, PipeWire's JACK shim is susceptible of triggering this
//                 .expect("JACK ERROR: buggy frame counter"),
//         )
//         .unwrap()
//         .strict_mul(n_channels);

//         let chunk_iter = self
//             .rx
//             .read_chunk(slots)
//             .unwrap()
//             .into_iter()
//             .skip(n_samples_skipped);

//         for (jack_sample, rb_sample) in core::iter::zip(interleaved, chunk_iter) {
//             *jack_sample = rb_sample;
//         }

//         jack::Control::Continue
//     }
// }

// struct JackReceiver {
//     // we store this here to keep the jack client alive
//     #[allow(dead_code)]
//     async_client: jack::AsyncClient<(), ProcessSender>,
//     audio_rx: queue::Receiver,
// }

// impl TimedReceiver for JackReceiver {
//     fn recv(&mut self, sample_idx: u64) -> impl Iterator<Item = f32> {
//         self.audio_rx.recv(sample_idx)
//     }
// }

// pub fn start(
//     socket: &std::net::UdpSocket,
//     beacon_dest: core::net::SocketAddr,
//     beacon_period: core::time::Duration,
//     mut rb_length: impl FnMut(core::net::SocketAddr, StreamConfig) -> core::time::Duration,
//     mut delay: impl FnMut(core::net::SocketAddr, StreamConfig) -> core::time::Duration,
// ) -> io::Result<Infallible> {
//     syfala_net::start_client(
//         socket,
//         beacon_dest,
//         beacon_period,
//         None,
//         |addr, config, handle| {
//             let n_ports = num::NonZeroUsize::try_from(config.n_channels).unwrap();
//             let chunk_size_spls = num::NonZeroUsize::try_from(config.chunk_size_samples()).unwrap();

//             println!("Creating JACK client...");
//             let name = format!("Server\n{}\n{}", addr.ip(), addr.port());

//             let (jack_client, _status) =
//                 jack::Client::new(name.as_str(), jack::ClientOptions::NO_START_SERVER).ok()?;

//             let sr = jack_client.sample_rate() as u128;

//             let rb_size_frames =
//                 ((rb_length(addr, config).as_nanos() * sr) / 1_000_000_000) as usize;

//             let rb_size_spls = rb_size_frames.checked_mul(n_ports.get()).unwrap();

//             let delay_frames =
//                 usize::try_from((delay(addr, config).as_nanos() * sr) / 1_000_000_000).unwrap();
//             let delay_samples = delay_frames.strict_mul(n_ports.get());

//             println!("Allocating Ring Buffer ({rb_size_spls} samples)");
//             println!("Adding delay ({delay_samples} samples)");

//             let (mut tx, rx) =
//                 queue::rtrb::RingBuffer::<f32>::new(rb_size_spls.strict_add(delay_samples));

//             tx.write_chunk_uninit(delay_samples)
//                 .unwrap()
//                 .fill_from_iter(iter::repeat(0.));

//             let waker = syfala_net::Waker::new(handle.clone(), chunk_size_spls);

//             let sender = ProcessSender::new(
//                 tx,
//                 waker,
//                 (1..=n_ports.get()).map(|i| {
//                     jack_client
//                         .register_port(&format!("input_{i}"), jack::AudioIn::default())
//                         .unwrap()
//                 }),
//             )
//             .unwrap();

//             let async_client = jack_client.activate_async((), sender).ok()?;
//             let audio_rx = queue::Receiver::new(rx);

//             Some(JackReceiver {
//                 async_client,
//                 audio_rx,
//             })
//         },
//     )
// }
