use super::*;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct Counter {
    curr: u64,
}

impl Default for Counter {
    #[inline(always)]
    fn default() -> Self {
        Self::new()
    }
}

impl Counter {
    #[inline(always)]
    const fn new() -> Self {
        Self { curr: 0 }
    }

    #[inline(always)]
    const fn current(&self) -> u64 {
        self.curr
    }

    #[inline(always)]
    fn chunk_time(&self, chunk_size: num::NonZeroUsize) -> usize {
        // elapsed % chunk_size
        let elapsed = self.current();
        usize::try_from(elapsed % num::NonZeroU64::try_from(chunk_size).unwrap()).unwrap()
    }

    #[inline(always)]
    fn remaining_chunk_time(&self, chunk_size: num::NonZeroUsize) -> num::NonZeroUsize {
        // chunk_size - elapsed % chunk_size

        num::NonZeroUsize::new(chunk_size.get().strict_sub(self.chunk_time(chunk_size))).unwrap()
    }

    #[inline(always)]
    fn advance(&mut self, time: usize) {
        self.curr = self.curr.strict_add(u64::try_from(time).unwrap());
    }

    #[inline(always)]
    const fn set_value(&mut self, val: u64) {
        self.curr = val;
    } 

    #[inline(always)]
    fn drift(&self, next_idx: u64) -> Result<Option<Drift>, num::TryFromIntError> {
        Drift::new(self, next_idx)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct Drift {
    negative: bool,
    abs: num::NonZeroUsize,
}

impl Drift {
    #[inline(always)]
    fn new(timer: &Counter, next_idx: u64) -> Result<Option<Self>, num::TryFromIntError> {
        let c = timer.current();
        usize::try_from(next_idx.abs_diff(c)).map(|abs| {
            num::NonZeroUsize::new(abs).map(|abs| Self {
                negative: next_idx < c,
                abs,
            })
        })
    }

    #[inline(always)]
    pub(crate) const fn is_negative(&self) -> bool {
        self.negative
    }

    #[inline(always)]
    pub(crate) const fn abs(&self) -> num::NonZeroUsize {
        self.abs
    }
}

#[derive(Debug, Clone)]
pub(crate) struct WakingCounter {
    counter: timing::Counter,
    waker: Waker,
}

impl Default for WakingCounter {
    #[inline(always)]
    fn default() -> Self {
        Self::new()
    }
}

impl WakingCounter {
    #[inline(always)]
    pub(crate) fn new() -> Self {
        Self::with_waker(Waker::useless())
    }

    #[inline(always)]
    pub(crate) const fn with_waker(waker: Waker) -> Self {
        Self {
            counter: timing::Counter::new(),
            waker,
        }
    }

    #[inline(always)]
    pub(crate) fn advance_timer(&mut self, delta: usize) {
        let rem_spls = self
            .counter
            .remaining_chunk_time(self.waker.chunk_size_samples());
        if delta >= rem_spls.get() {
            self.waker.wake();
        }

        self.counter.advance(delta);
    }

    #[inline(always)]
    pub(crate) const fn waker(&self) -> &Waker {
        &self.waker
    }

    #[inline(always)]
    pub(crate) const fn waker_mut(&mut self) -> &mut Waker {
        &mut self.waker
    }

    #[inline(always)]
    pub(crate) const fn set_value(&mut self, val: u64) {
        self.counter.set_value(val);
    }

    #[inline(always)]
    pub(crate) const fn get_value(&self) -> u64 {
        self.counter.current()
    }

    #[inline(always)]
    pub(crate) fn drift(&self, next_idx: u64) -> Result<Option<Drift>, num::TryFromIntError> {
        self.counter.drift(next_idx)
    }
}
