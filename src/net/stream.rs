//! A reliable, ordered stream of bytes over a network that loses, repeats and
//! reorders packets. Only the rules live here: no sockets and no clocks of its
//! own, so every bad network can be simulated.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, VecDeque};
use std::time::{Duration, Instant};

/// Bytes carried by one packet: small enough not to be split on the way.
pub const SEGMENT: usize = 1200;
/// Bytes that may wait to be sent before the writer has to wait.
const SEND_BUFFER: usize = 32 * 1024;

/// What one packet says.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Frame {
    /// "Can you hear me?", and with `seen`, "and I heard you".
    Punch {
        seen: bool,
    },
    Data {
        seq: u32,
        bytes: Vec<u8>,
    },
    /// Everything before `next` arrived; bit `i` of `ahead` says that
    /// `next + 1 + i` did too.
    Ack {
        next: u32,
        ahead: u64,
    },
    Ping,
    Close,
}

/// Packets sent and not yet confirmed, at most; also how far ahead of the next
/// expected one a received packet may be.
const MAX_WINDOW: usize = 256;
const START_WINDOW: f32 = 32.0;
/// Losses never shrink it below this while the other end still answers.
const MIN_WINDOW: f32 = 24.0;
/// How many are retried when nothing at all comes back.
const SILENT_WINDOW: f32 = 4.0;
/// What is left of the window after a loss. Home networks lose packets for
/// reasons other than being full, so it is not halved.
const KEPT: f32 = 0.7;
/// How long an answer is waited for before anything is known of the trip.
const START_WAIT: Duration = Duration::from_millis(400);
const MIN_WAIT: Duration = Duration::from_millis(100);
const MAX_WAIT: Duration = Duration::from_secs(2);
/// Later packets confirmed before one still missing is taken as lost.
const OVERTAKEN: usize = 3;
/// Something is said at least this often, so silence means trouble.
const PING_EVERY: Duration = Duration::from_secs(1);
/// Bytes received and not yet read before new ones stop being accepted.
const READ_BUFFER: usize = 512 * 1024;

/// A piece of data on its way.
struct Piece {
    bytes: Vec<u8>,
    /// When it was last sent; `None` asks for it to be sent now.
    sent: Option<Instant>,
    resent: bool,
    confirmed: bool,
}

pub struct Stream {
    unsent: VecDeque<u8>,
    /// Sent and not all confirmed. The first one is number `first`, and the
    /// rest follow in order.
    flight: VecDeque<Piece>,
    first: u32,
    /// How many may be in flight: grows while all goes well and shrinks when
    /// packets are lost.
    window: f32,
    threshold: f32,
    cut: Option<Instant>,
    /// Smoothed round trip and how much it varies.
    trip: Option<(Duration, Duration)>,
    wait: Duration,
    /// The number of the next piece to hand over.
    expected: u32,
    ahead: BTreeMap<u32, Vec<u8>>,
    readable: VecDeque<u8>,
    ack_due: bool,
    answer_punch: bool,
    spoke: Instant,
    heard: Instant,
    closed: bool,
}

impl Stream {
    pub fn new(now: Instant) -> Stream {
        Stream {
            unsent: VecDeque::new(),
            flight: VecDeque::new(),
            first: 0,
            window: START_WINDOW,
            threshold: MAX_WINDOW as f32 / 2.0,
            cut: None,
            trip: None,
            wait: START_WAIT,
            expected: 0,
            ahead: BTreeMap::new(),
            readable: VecDeque::new(),
            ack_due: false,
            answer_punch: false,
            spoke: now,
            heard: now,
            closed: false,
        }
    }

    /// Takes as much of `bytes` as there is room for.
    pub fn write(&mut self, bytes: &[u8]) -> usize {
        let taken = bytes.len().min(SEND_BUFFER - self.unsent.len());
        self.unsent.extend(&bytes[..taken]);
        taken
    }

    /// Hands over what has arrived in order. Zero means nothing yet.
    pub fn read(&mut self, out: &mut [u8]) -> usize {
        std::io::Read::read(&mut self.readable, out).unwrap_or(0)
    }

    pub fn accept(&mut self, frame: Frame, now: Instant) {
        self.heard = now;
        match frame {
            // An answer is not answered, or the two would never stop.
            Frame::Punch { seen } => self.answer_punch |= !seen,
            Frame::Data { seq, bytes } => self.receive(seq, bytes),
            Frame::Ack { next, ahead } => self.confirm(next, ahead, now),
            Frame::Ping => {}
            Frame::Close => self.closed = true,
        }
    }

    fn receive(&mut self, seq: u32, bytes: Vec<u8>) {
        if bytes.is_empty() || bytes.len() > SEGMENT {
            return;
        }
        // Even an old one is confirmed again, or it would keep coming.
        self.ack_due = true;
        let distance = seq.wrapping_sub(self.expected) as usize;
        if distance >= MAX_WINDOW || self.readable.len() >= READ_BUFFER {
            return;
        }
        self.ahead.entry(seq).or_insert(bytes);
        while let Some(bytes) = self.ahead.remove(&self.expected) {
            self.readable.extend(bytes);
            self.expected = self.expected.wrapping_add(1);
        }
    }

    fn confirm(&mut self, next: u32, ahead: u64, now: Instant) {
        let done = next.wrapping_sub(self.first) as usize;
        // More than was ever sent, or older than what is already known.
        if done > self.flight.len() {
            return;
        }
        let mut trips = Vec::new();
        for piece in self.flight.drain(..done) {
            if !piece.confirmed {
                trips.push(piece.sent.filter(|_| !piece.resent));
            }
        }
        self.first = next;
        for bit in 0..64 {
            if ahead >> bit & 1 == 0 {
                continue;
            }
            if let Some(piece) = self.flight.get_mut(bit + 1)
                && !piece.confirmed
            {
                piece.confirmed = true;
                piece.bytes = Vec::new();
                trips.push(piece.sent.filter(|_| !piece.resent));
            }
        }
        for sent in trips {
            self.credit(sent, now);
        }
        self.notice_losses(now);
    }

    /// A packet arrived: measure the trip and allow a little more at once.
    /// One sent twice cannot say which trip was measured, so `sent` is `None`.
    fn credit(&mut self, sent: Option<Instant>, now: Instant) {
        if let Some(sent) = sent {
            let took = now.saturating_duration_since(sent);
            let (mean, spread) = match self.trip {
                None => (took, took / 2),
                Some((mean, spread)) => (
                    mean * 7 / 8 + took / 8,
                    spread * 3 / 4 + mean.abs_diff(took) / 4,
                ),
            };
            self.trip = Some((mean, spread));
            self.wait =
                (mean + (spread * 4).max(Duration::from_millis(20))).clamp(MIN_WAIT, MAX_WAIT);
        }
        let step = if self.window < self.threshold {
            1.0
        } else {
            1.0 / self.window
        };
        self.window = (self.window + step).min(MAX_WINDOW as f32);
    }

    /// A piece is taken as lost, and sent again at once, when several that
    /// left after it have arrived and its own trip has had time to end.
    fn notice_losses(&mut self, now: Instant) {
        let trip = self.trip.map_or(self.wait, |t| t.0);
        // When the latest ones to leave, among those confirmed further on,
        // did so: the most recent first.
        let mut latest: [Option<Instant>; OVERTAKEN] = [None; OVERTAKEN];
        let mut lost = false;
        for piece in self.flight.iter_mut().rev() {
            let Some(sent) = piece.sent else {
                continue;
            };
            if piece.confirmed {
                let mut newcomer = Some(sent);
                for slot in &mut latest {
                    if newcomer > *slot {
                        std::mem::swap(slot, &mut newcomer);
                    }
                }
            } else if latest[OVERTAKEN - 1].is_some_and(|other| other > sent)
                && now.saturating_duration_since(sent) >= trip
            {
                piece.sent = None;
                lost = true;
            }
        }
        if lost && self.cut_once_per_trip(now) {
            self.window = self.threshold;
        }
    }

    /// Many losses of the same moment count as one.
    fn cut_once_per_trip(&mut self, now: Instant) -> bool {
        let trip = self.trip.map_or(self.wait, |t| t.0);
        if self
            .cut
            .is_some_and(|at| now.saturating_duration_since(at) < trip)
        {
            return false;
        }
        self.cut = Some(now);
        self.threshold = (self.window * KEPT).max(MIN_WINDOW);
        true
    }

    /// The next packet to send now, if any. Call until there is none.
    pub fn transmit(&mut self, now: Instant) -> Option<Frame> {
        let frame = self.next_frame(now)?;
        self.spoke = now;
        Some(frame)
    }

    fn next_frame(&mut self, now: Instant) -> Option<Frame> {
        if std::mem::take(&mut self.answer_punch) {
            return Some(Frame::Punch { seen: true });
        }
        if std::mem::take(&mut self.ack_due) {
            let ahead = (0..64u32)
                .filter(|bit| {
                    self.ahead
                        .contains_key(&self.expected.wrapping_add(bit + 1))
                })
                .fold(0u64, |mask, bit| mask | 1 << bit);
            return Some(Frame::Ack {
                next: self.expected,
                ahead,
            });
        }
        if let Some(frame) = self.resend(now) {
            return Some(frame);
        }
        let travelling = self.flight.iter().filter(|p| !p.confirmed).count();
        if !self.unsent.is_empty()
            && travelling < self.window as usize
            && self.flight.len() < MAX_WINDOW
        {
            let size = self.unsent.len().min(SEGMENT);
            let bytes: Vec<u8> = self.unsent.drain(..size).collect();
            let seq = self.first.wrapping_add(self.flight.len() as u32);
            self.flight.push_back(Piece {
                bytes: bytes.clone(),
                sent: Some(now),
                resent: false,
                confirmed: false,
            });
            return Some(Frame::Data { seq, bytes });
        }
        (now.saturating_duration_since(self.spoke) >= PING_EVERY).then_some(Frame::Ping)
    }

    /// A piece that is known lost, or has waited too long for its answer.
    fn resend(&mut self, now: Instant) -> Option<Frame> {
        let wait = self.wait;
        // Only the oldest ones, so that a silent link is not flooded.
        let (due, piece) = self
            .flight
            .iter()
            .enumerate()
            .filter(|(_, p)| !p.confirmed)
            .take(self.window as usize)
            .find(|(_, p)| {
                p.sent
                    .is_none_or(|sent| now.saturating_duration_since(sent) >= wait)
            })?;
        if let Some(sent) = piece.sent
            && self.cut_once_per_trip(now)
        {
            if self.heard > sent {
                // The other end is still there: an ordinary loss.
                self.window = self.threshold;
            } else {
                self.window = SILENT_WINDOW;
                self.wait = (self.wait * 2).min(MAX_WAIT);
            }
        }
        let seq = self.first.wrapping_add(due as u32);
        let piece = &mut self.flight[due];
        piece.sent = Some(now);
        piece.resent = true;
        Some(Frame::Data {
            seq,
            bytes: piece.bytes.clone(),
        })
    }

    /// Whether the other end confirmed everything written.
    pub fn delivered(&self) -> bool {
        self.unsent.is_empty() && self.flight.is_empty()
    }

    pub fn peer_closed(&self) -> bool {
        self.closed
    }

    /// How long nothing has arrived.
    pub fn silent_for(&self, now: Instant) -> Duration {
        now.saturating_duration_since(self.heard)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use babel_core::babel::seeds::Dice;

    const TICK: Duration = Duration::from_millis(5);

    /// Two streams joined by a network as bad as asked for.
    struct Net {
        ends: [Stream; 2],
        start: Instant,
        now: Instant,
        /// Packets on their way: when they arrive, and to which end.
        wire: Vec<(Instant, usize, Frame)>,
        dice: Dice,
        loss: f32,
        copies: f32,
        delay_ms: (u32, u32),
        sent: usize,
    }

    impl Net {
        fn new(loss: f32, copies: f32, delay_ms: (u32, u32)) -> Net {
            let now = Instant::now();
            Net {
                ends: [Stream::new(now), Stream::new(now)],
                start: now,
                now,
                wire: Vec::new(),
                dice: Dice::from_u64(11),
                loss,
                copies,
                delay_ms,
                sent: 0,
            }
        }

        fn tick(&mut self) {
            for from in 0..2 {
                let mut burst = 0;
                while let Some(frame) = self.ends[from].transmit(self.now) {
                    self.sent += 1;
                    burst += 1;
                    assert!(burst < 2000, "transmit never runs dry");
                    if self.dice.chance(self.loss) {
                        continue;
                    }
                    let times = 1 + usize::from(self.dice.chance(self.copies));
                    for _ in 0..times {
                        let spread = self.delay_ms.1 - self.delay_ms.0 + 1;
                        let ms = self.delay_ms.0 + self.dice.below(spread);
                        let at = self.now + Duration::from_millis(u64::from(ms));
                        self.wire.push((at, 1 - from, frame.clone()));
                    }
                }
            }
            self.now += TICK;
            let now = self.now;
            let mut due: Vec<_> = Vec::new();
            self.wire.retain(|packet| {
                let arrives = packet.0 <= now;
                if arrives {
                    due.push(packet.clone());
                }
                !arrives
            });
            due.sort_by_key(|packet| packet.0);
            for (_, to, frame) in due {
                self.ends[to].accept(frame, now);
            }
        }

        /// Sends both messages at once and returns what each end received.
        fn exchange(&mut self, messages: [&[u8]; 2], seconds: u64) -> [Vec<u8>; 2] {
            let mut written = [0, 0];
            let mut got = [Vec::new(), Vec::new()];
            let began = self.now;
            let mut buffer = [0u8; 4096];
            while got[0].len() < messages[1].len() || got[1].len() < messages[0].len() {
                for end in 0..2 {
                    written[end] += self.ends[end].write(&messages[end][written[end]..]);
                    loop {
                        let n = self.ends[end].read(&mut buffer);
                        if n == 0 {
                            break;
                        }
                        got[end].extend_from_slice(&buffer[..n]);
                    }
                }
                self.tick();
                assert!(
                    self.now - began < Duration::from_secs(seconds),
                    "after {seconds} s: {} of {} and {} of {}",
                    got[1].len(),
                    messages[0].len(),
                    got[0].len(),
                    messages[1].len()
                );
            }
            got
        }
    }

    fn noise(length: usize, seed: u64) -> Vec<u8> {
        let mut dice = Dice::from_u64(seed);
        (0..length).map(|_| dice.below(256) as u8).collect()
    }

    #[test]
    fn bytes_arrive_whole_and_in_order_on_a_perfect_link() {
        let mut net = Net::new(0.0, 0.0, (10, 10));
        let (a, b) = (noise(100_000, 1), noise(10, 2));
        let got = net.exchange([&a, &b], 10);
        assert_eq!(got[1], a);
        assert_eq!(got[0], b);
    }

    #[test]
    fn a_megabyte_survives_loss_reordering_and_duplicates() {
        let mut net = Net::new(0.2, 0.1, (20, 120));
        let (a, b) = (noise(1 << 20, 3), noise(200_000, 4));
        let got = net.exchange([&a, &b], 120);
        assert!(got[1] == a, "the long message arrived changed");
        assert!(got[0] == b, "the short message arrived changed");
    }

    #[test]
    fn game_traffic_keeps_up_on_a_lossy_home_link() {
        // 100 ms there and back with 3 % lost. Two players need well under
        // 200 kB each second; here that much must get through.
        let mut net = Net::new(0.03, 0.0, (45, 55));
        let message = noise(2_000_000, 7);
        let began = net.now;
        let got = net.exchange([&message, b"x"], 60);
        assert!(got[1] == message);
        let took = (net.now - began).as_secs_f32();
        assert!(took < 10.0, "2 MB took {took:.1} s");
    }

    #[test]
    fn a_full_buffer_refuses_more_until_it_drains() {
        let now = Instant::now();
        let mut stream = Stream::new(now);
        let chunk = [7u8; 1000];
        let mut taken = 0;
        for _ in 0..2000 {
            let n = stream.write(&chunk);
            taken += n;
            while stream.transmit(now).is_some() {}
            if n == 0 {
                break;
            }
        }
        assert_eq!(stream.write(&chunk), 0, "there is no end to it");
        assert!((SEGMENT..400_000).contains(&taken), "{taken} bytes taken");
    }

    #[test]
    fn a_dead_link_is_retried_calmly() {
        let mut net = Net::new(1.0, 0.0, (10, 10));
        net.ends[0].write(&noise(30_000, 5));
        while net.now - net.start < Duration::from_secs(10) {
            net.tick();
        }
        assert!(net.sent < 200, "{} packets into the void", net.sent);
        assert!(net.sent > 10, "it gave up: {} packets", net.sent);
        let silent = net.ends[0].silent_for(net.now);
        assert!(silent >= Duration::from_secs(9), "{silent:?}");
    }

    #[test]
    fn an_idle_link_keeps_talking() {
        let mut net = Net::new(0.0, 0.0, (10, 10));
        while net.now - net.start < Duration::from_secs(10) {
            net.tick();
        }
        assert!((16..=24).contains(&net.sent), "{} pings", net.sent);
        assert!(net.ends[1].silent_for(net.now) < Duration::from_millis(1500));
    }

    #[test]
    fn nonsense_acks_and_far_sequences_are_ignored() {
        let mut net = Net::new(0.0, 0.0, (10, 10));
        let now = net.now;
        let stream = &mut net.ends[1];
        stream.accept(
            Frame::Ack {
                next: 1000,
                ahead: u64::MAX,
            },
            now,
        );
        for seq in [5000, u32::MAX, 256] {
            stream.accept(
                Frame::Data {
                    seq,
                    bytes: vec![1; 10],
                },
                now,
            );
        }
        stream.accept(
            Frame::Data {
                seq: 0,
                bytes: vec![1; SEGMENT + 1],
            },
            now,
        );
        stream.accept(
            Frame::Data {
                seq: 0,
                bytes: Vec::new(),
            },
            now,
        );
        assert_eq!(stream.read(&mut [0; 64]), 0);
        let message = noise(5000, 6);
        let got = net.exchange([&message, &message], 10);
        assert_eq!(got[1], message);
        assert_eq!(got[0], message);
    }

    #[test]
    fn delivered_turns_true_only_when_everything_was_confirmed() {
        let mut net = Net::new(0.3, 0.0, (30, 30));
        assert!(net.ends[0].delivered(), "nothing was written yet");
        let farewell = b"El anfitrion cerro la sala.";
        assert_eq!(net.ends[0].write(farewell), farewell.len());
        assert!(!net.ends[0].delivered());
        while !net.ends[0].delivered() {
            net.tick();
            assert!(net.now - net.start < Duration::from_secs(30));
        }
        let mut buffer = [0u8; 64];
        let n = net.ends[1].read(&mut buffer);
        assert_eq!(&buffer[..n], farewell);
    }

    #[test]
    fn punches_are_answered_and_a_close_is_noticed() {
        let now = Instant::now();
        let mut stream = Stream::new(now);
        stream.accept(Frame::Punch { seen: false }, now);
        assert_eq!(stream.transmit(now), Some(Frame::Punch { seen: true }));
        // An answer is not answered, or the two would never stop.
        stream.accept(Frame::Punch { seen: true }, now);
        assert_eq!(stream.transmit(now), None);
        assert!(!stream.peer_closed());
        stream.accept(Frame::Close, now);
        assert!(stream.peer_closed());
    }
}
