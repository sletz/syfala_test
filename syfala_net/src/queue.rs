//! Utilities for sending audio streams to other threads using real-time ring buffers.
//! With periodic waking functionality,
//!
//! Re-exports [`rtrb`] for convenience.
use super::*;

/// Convenience re-export of rtrb
pub use rtrb;

use core::iter;

/// Sends audio data over a ring buffer, with an internal sample timer to track missed samples.
///
/// Note that everything here is in __samples__, for multichannel data, some extra bookkeeping
/// might be needed.
pub struct Sender {
    tx: rtrb::Producer<f32>,
    counter: timing::WakingCounter,
}

impl Sender {
    #[inline(always)]
    pub fn new(tx: rtrb::Producer<f32>) -> Self {
        Self {
            tx,
            counter: timing::WakingCounter::default(),
        }
    }

    #[inline(always)]
    pub fn with_waker(tx: rtrb::Producer<f32>, waker: Waker) -> Self {
        Self {
            tx,
            counter: timing::WakingCounter::with_waker(waker),
        }
    }

    #[inline(always)]
    pub const fn set_counter_value(&mut self, val: u64) {
        self.counter.set_value(val);
    }

    #[inline(always)]
    pub const fn get_counter_value(&self) -> u64 {
        self.counter.get_value()
    }

    #[inline(always)]
    pub fn is_abandoned(&self) -> bool {
        self.tx.is_abandoned()
    }

    #[inline(always)]
    pub fn capacity_samples(&self) -> usize {
        self.tx.buffer().capacity()
    }

    #[inline(always)]
    pub fn available_samples(&self) -> usize {
        self.tx.slots()
    }

    #[inline(always)]
    pub fn waker(&self) -> &Waker {
        self.counter.waker()
    }

    #[inline(always)]
    pub fn waker_mut(&mut self) -> &mut Waker {
        self.counter.waker_mut()
    }
}

impl TimedSender for Sender {
    /// Writes the elements in `samples` into the sender's ring buffer, `timestamp` is used to
    /// pad with silence, or skip samples when necessary.
    #[inline]
    fn send(&mut self, timestamp: u64, samples: impl IntoIterator<Item = f32>) -> usize {
        let drift = self.counter.drift(timestamp).unwrap();

        let mut n_in_samples_skipped = 0;
        let mut n_out_samples_skipped = 0;

        if let Some(drift) = drift {
            *if drift.is_negative() {
                &mut n_in_samples_skipped
            } else {
                &mut n_out_samples_skipped
            } = drift.abs().get();
        }

        let out_iter = iter::chain(
            iter::repeat_n(0., n_out_samples_skipped),
            samples.into_iter().skip(n_in_samples_skipped),
        );

        let n_pushed_samples = self
            .tx
            .write_chunk_uninit(self.tx.slots())
            .unwrap()
            .fill_from_iter(out_iter);

        self.counter.advance_timer(n_pushed_samples);

        n_pushed_samples
    }
}

/// Sends audio data from a ring buffer, with an internal sample timer to track missed samples.
///
/// Note that everything here is in _samples_, for multichannel data, some extra bookkeeping
/// might be needed.
pub struct Receiver {
    rx: rtrb::Consumer<f32>,
    counter: timing::WakingCounter,
}

impl Receiver {
    #[inline(always)]
    pub fn new(rx: rtrb::Consumer<f32>) -> Self {
        Self {
            rx,
            counter: timing::WakingCounter::default(),
        }
    }

    #[inline(always)]
    pub fn with_waker(rx: rtrb::Consumer<f32>, waker: Waker) -> Self {
        Self {
            rx,
            counter: timing::WakingCounter::with_waker(waker),
        }
    }

    #[inline(always)]
    pub const fn set_counter_value(&mut self, val: u64) {
        self.counter.set_value(val);
    }

    #[inline(always)]
    pub fn is_abandoned(&self) -> bool {
        self.rx.is_abandoned()
    }

    #[inline(always)]
    pub fn capacity_samples(&self) -> usize {
        self.rx.buffer().capacity()
    }

    #[inline(always)]
    pub fn n_available_samples(&self) -> usize {
        self.rx.slots()
    }

    #[inline(always)]
    pub fn waker(&self) -> &Waker {
        self.counter.waker()
    }

    #[inline(always)]
    pub fn waker_mut(&mut self) -> &mut Waker {
        self.counter.waker_mut()
    }
}

impl TimedReceiver for Receiver {
    /// Attempts to read samples from the ring buffer, `sample_idx` is used to
    /// pad with silence, or skip samples when necessary.
    #[inline]
    fn recv(&mut self, sample_idx: u64) -> impl Iterator<Item = f32> {
        let drift = self.counter.drift(sample_idx).unwrap();

        // notice how neither are positive at the same time
        let mut n_out_samples_skipped = 0;
        let mut n_in_samples_skipped = 0;

        if let Some(drift) = drift {
            *if drift.is_negative() {
                &mut n_out_samples_skipped
            } else {
                &mut n_in_samples_skipped
            } = drift.abs().get();
        }

        let in_samples = self.rx.read_chunk(self.rx.slots()).unwrap();

        iter::chain(
            iter::repeat_n(0., n_out_samples_skipped),
            ReadChunksIterWaker::new(in_samples, &mut self.counter).skip(n_in_samples_skipped),
        )
    }
}

struct ReadChunksIterWaker<'a, T> {
    waker: &'a mut timing::WakingCounter,
    iter: rtrb::chunks::ReadChunkIntoIter<'a, T>,
    initial_len: usize,
}

impl<'a, T> ReadChunksIterWaker<'a, T> {
    fn new(chunk: rtrb::chunks::ReadChunk<'a, T>, waker: &'a mut timing::WakingCounter) -> Self {
        Self {
            initial_len: chunk.len(),
            iter: chunk.into_iter(),
            waker,
        }
    }
}

impl<'a, T> Iterator for ReadChunksIterWaker<'a, T> {
    type Item = T;

    #[inline(always)]
    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next()
    }
}

impl<'a, T> ExactSizeIterator for ReadChunksIterWaker<'a, T> {
    fn len(&self) -> usize {
        self.iter.len()
    }
}

impl<T> Drop for ReadChunksIterWaker<'_, T> {
    fn drop(&mut self) {
        self.waker
            // sent samples = initial samples - remaining samples
            .advance_timer(self.initial_len.strict_sub(self.iter.len()));
    }
}
