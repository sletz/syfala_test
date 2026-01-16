// TODO: create common adapters for AudioData<'a> -> padded samples
// for different sample types

use core::num;

pub use rtrb;

pub trait AudioConsumer {
    fn try_get_bytes(&mut self) -> Option<syfala_proto::AudioPacket<'_>>;
}

pub trait AudioProducer {
    fn process(&mut self, audio: syfala_proto::AudioPacket);
}

#[derive(Debug)]
pub struct WakingProducer<T> {
    tx: rtrb::Producer<T>,
    waker: std::thread::Thread,
    waking_period: num::NonZeroUsize,
    n_sent_items: u64,
}

impl<T> WakingProducer<T> {
    #[inline(always)]
    pub const fn new(
        tx: rtrb::Producer<T>,
        waking_period: num::NonZeroUsize,
        waker: std::thread::Thread,
    ) -> Self {
        Self {
            tx,
            waker,
            waking_period,
            n_sent_items: 0,
        }
    }

    pub const fn waking_period(&self) -> num::NonZeroUsize {
        self.waking_period
    }

    pub fn advance_conter(&mut self, n: num::NonZeroUsize) -> usize {
        let wp = self.waking_period();

        let current_chunk_idx =
            usize::try_from(self.n_sent_items % num::NonZeroU64::try_from(wp).unwrap()).unwrap();

        let wake = usize::try_from(current_chunk_idx.strict_add(n.get()) / wp).unwrap();

        if wake != 0 {
            self.waker.unpark();
        }

        wake
    }

    #[inline(always)]
    pub fn fill_in_one_go(&mut self, it: impl IntoIterator<Item = T>) -> (usize, usize) {
        let n_items_sent = self
            .tx
            .write_chunk_uninit(self.tx.slots())
            .unwrap()
            .fill_from_iter(it);

        let waken = num::NonZeroUsize::new(n_items_sent)
            .map(|n| self.advance_conter(n))
            .unwrap_or(0);

        (waken, n_items_sent)
    }
}
