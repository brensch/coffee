//! The link between the network side (HTTP workers, USB console) and the control
//! loop, built so the control loop never waits on anything the network side holds.
//!
//! - **Requests** go through a bounded queue ([`Link`]). A client sends with
//!   `try_send` (a full queue is reported as [`LinkError::Busy`], never waited on)
//!   and waits on its own reply slot, with a timeout.
//! - **The control loop** drains the queue once per mains cycle with [`Link::serve`],
//!   under a budget: at most `max` requests, and only while `within_budget()` holds.
//!   Anything left waits for the next cycle. The handler runs in the control loop
//!   and should only copy data out; expensive work (JSON encoding) is done by the
//!   client after the reply arrives.
//! - **Replies** carry the request's sequence number, so a reply that arrives after
//!   its client gave up is recognised as stale and dropped by the client's next call.
//! - **Snapshots** ([`Latest`]) carry state the other way (telemetry): the control
//!   loop replaces the value, readers clone it. The critical section is held only to
//!   swap or clone, never across an await.

use core::cell::RefCell;
use core::future::Future;
use critical_section::Mutex;
use embassy_futures::select::{Either, select};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;
use embassy_sync::signal::Signal;

/// A queued request: the client's slot, its sequence number, and the payload.
pub struct Request<Q> {
    pub client: usize,
    pub seq: u32,
    pub body: Q,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkError {
    /// The queue was full: the control loop is behind. Try again later (HTTP 503).
    Busy,
    /// No reply in time (HTTP 504). A late reply is discarded by the next call.
    Timeout,
}

/// Bounded request queue (`DEPTH`) plus one reply slot per client (`CLIENTS`).
pub struct Link<Q, R, const DEPTH: usize, const CLIENTS: usize> {
    requests: Channel<CriticalSectionRawMutex, Request<Q>, DEPTH>,
    replies: [Signal<CriticalSectionRawMutex, (u32, R)>; CLIENTS],
}

impl<Q, R, const DEPTH: usize, const CLIENTS: usize> Default for Link<Q, R, DEPTH, CLIENTS> {
    fn default() -> Self {
        Self::new()
    }
}

impl<Q, R, const DEPTH: usize, const CLIENTS: usize> Link<Q, R, DEPTH, CLIENTS> {
    pub const fn new() -> Self {
        Self {
            requests: Channel::new(),
            replies: [const { Signal::new() }; CLIENTS],
        }
    }

    /// A client handle for reply slot `id` (each concurrent caller needs its own).
    pub fn client(&self, id: usize) -> Client<'_, Q, R, DEPTH, CLIENTS> {
        assert!(id < CLIENTS, "client id {id} out of range");
        Client { link: self, id, seq: 0 }
    }

    /// Control-loop side: handle queued requests while `n < max` and `within_budget()`.
    /// Never waits. Returns how many were handled.
    pub fn serve(&self, max: usize, mut within_budget: impl FnMut() -> bool, mut handle: impl FnMut(Q) -> R) -> usize {
        let mut n = 0;
        while n < max && within_budget() {
            let Ok(req) = self.requests.try_receive() else { break };
            let r = handle(req.body);
            self.replies[req.client].signal((req.seq, r));
            n += 1;
        }
        n
    }

    /// Requests waiting (for status reporting).
    pub fn queued(&self) -> usize {
        self.requests.len()
    }
}

/// One caller's end of the link.
pub struct Client<'a, Q, R, const DEPTH: usize, const CLIENTS: usize> {
    link: &'a Link<Q, R, DEPTH, CLIENTS>,
    id: usize,
    seq: u32,
}

impl<Q, R, const DEPTH: usize, const CLIENTS: usize> Client<'_, Q, R, DEPTH, CLIENTS> {
    /// Queue `body` and wait for its reply, or for `timeout` to finish.
    pub async fn call(&mut self, body: Q, timeout: impl Future<Output = ()>) -> Result<R, LinkError> {
        self.seq = self.seq.wrapping_add(1);
        let slot = &self.link.replies[self.id];
        slot.reset(); // drop any stale reply from an earlier, timed-out call
        self.link
            .requests
            .try_send(Request { client: self.id, seq: self.seq, body })
            .map_err(|_| LinkError::Busy)?;
        let mut timeout = core::pin::pin!(timeout);
        loop {
            match select(slot.wait(), timeout.as_mut()).await {
                Either::First((seq, r)) if seq == self.seq => return Ok(r),
                Either::First(_) => continue, // stale reply that slipped in after reset
                Either::Second(()) => return Err(LinkError::Timeout),
            }
        }
    }
}

/// The latest value of something the control loop produces (telemetry), for readers
/// on the network side. Writers swap, readers clone; both inside a short critical
/// section, nothing held across an await.
pub struct Latest<T>(Mutex<RefCell<Option<T>>>);

impl<T> Default for Latest<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> Latest<T> {
    pub const fn new() -> Self {
        Latest(Mutex::new(RefCell::new(None)))
    }

    /// Replace the value. The old one is dropped after the critical section.
    pub fn publish(&self, v: T) {
        let old = critical_section::with(|cs| self.0.borrow_ref_mut(cs).replace(v));
        drop(old);
    }

    pub fn get(&self) -> Option<T>
    where
        T: Clone,
    {
        critical_section::with(|cs| self.0.borrow_ref(cs).clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::cell::Cell;
    use embassy_futures::{block_on, join::join, yield_now};

    type L = Link<u32, u32, 4, 2>;

    /// A timeout that fires after `n` polls.
    async fn polls(n: usize) {
        for _ in 0..n {
            yield_now().await;
        }
    }

    #[test]
    fn reply_goes_to_its_caller() {
        let link = L::new();
        let mut c = link.client(1);
        let (r, served) = block_on(join(c.call(20, polls(10)), async {
            yield_now().await; // let the client queue first
            link.serve(4, || true, |q| q + 1)
        }));
        assert_eq!(r, Ok(21));
        assert_eq!(served, 1);
    }

    #[test]
    fn full_queue_is_busy_not_a_wait() {
        let link = L::new();
        let mut clients: alloc::vec::Vec<_> = (0..2).map(|i| link.client(i)).collect();
        // Fill the 4-deep queue with requests nobody serves (each call times out at once).
        for i in 0..4 {
            let r = block_on(clients[i % 2].call(i as u32, async {}));
            assert_eq!(r, Err(LinkError::Timeout));
        }
        assert_eq!(link.queued(), 4);
        assert_eq!(block_on(clients[0].call(9, async {})), Err(LinkError::Busy));
    }

    #[test]
    fn serve_respects_max_and_budget() {
        let link = L::new();
        let mut c = link.client(0);
        for i in 0..4 {
            let _ = block_on(c.call(i, async {}));
        }
        assert_eq!(link.serve(2, || true, |q| q), 2);
        assert_eq!(link.queued(), 2);
        // Budget already spent: nothing handled, nothing lost.
        assert_eq!(link.serve(10, || false, |q| q), 0);
        assert_eq!(link.queued(), 2);
        // Budget runs out after one request.
        let left = Cell::new(1u32);
        let n = link.serve(10, || {
            let ok = left.get() > 0;
            left.set(left.get().saturating_sub(1));
            ok
        }, |q| q);
        assert_eq!(n, 1);
        assert_eq!(link.queued(), 1);
    }

    #[test]
    fn late_reply_is_not_taken_for_the_next_one() {
        let link = L::new();
        let mut c = link.client(0);
        // First call times out before the control loop gets to it.
        assert_eq!(block_on(c.call(1, async {})), Err(LinkError::Timeout));
        // The loop now serves it (reply for seq 1 lands in the slot).
        assert_eq!(link.serve(4, || true, |q| q * 100), 1);
        // The next call must get its own reply (seq 2), not the stale 100.
        let (r, _) = block_on(join(c.call(2, polls(10)), async {
            yield_now().await;
            link.serve(4, || true, |q| q * 100)
        }));
        assert_eq!(r, Ok(200));
    }

    #[test]
    fn latest_swaps_and_clones() {
        let l: Latest<alloc::string::String> = Latest::new();
        assert_eq!(l.get(), None);
        l.publish("a".into());
        l.publish("b".into());
        assert_eq!(l.get().as_deref(), Some("b"));
    }
}
