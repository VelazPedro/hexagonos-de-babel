//! Meeting through two routers. A router lets packets in from somewhere only
//! after something went out to there, so both computers send to each other at
//! once until each hears the other. Every packet is signed with a key of the
//! room: nothing else is listened to.

use super::secure::{DATAGRAM, PunchKeys, open_datagram, seal_datagram};
use super::stream::Frame;
use super::stun;
use std::io::ErrorKind;
use std::net::{SocketAddr, SocketAddrV4, UdpSocket};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Receiver;
use std::time::{Duration, Instant};

/// How long the host keeps knocking after getting a reply code.
pub const HOST_PATIENCE: Duration = Duration::from_secs(45);
/// How long the guest waits for the reply code to reach the host.
const GUEST_PATIENCE: Duration = Duration::from_secs(300);
const GUEST_EVERY: Duration = Duration::from_millis(300);
const HOST_EVERY: Duration = Duration::from_millis(200);
/// Routers forget a quiet socket after about half a minute.
const KEEPALIVE: Duration = Duration::from_secs(20);
/// How long one wait for a packet lasts, and so how quickly a change is seen.
const LISTEN: Duration = Duration::from_millis(20);

/// Who answered, and in which attempt of theirs.
#[derive(Clone, Copy, Debug)]
pub struct Punched {
    pub peer: SocketAddr,
    pub session: u64,
}

/// How the host's knocking is going.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Punching {
    /// Waiting for a reply code.
    #[default]
    Idle,
    Trying,
    /// Nothing came back: one of the two networks does not let it through.
    Blocked,
}

pub struct HostSide<'a> {
    pub socket: &'a UdpSocket,
    pub keys: &'a PunchKeys,
    /// Where each reply code says the guest is.
    pub replies: &'a Receiver<SocketAddrV4>,
    /// Asked now and then, so the router keeps remembering this socket.
    pub keepalive: Option<SocketAddr>,
    pub closing: &'a AtomicBool,
    /// Set once somebody got in by another way.
    pub filled: &'a AtomicBool,
    pub punching: &'a Mutex<Punching>,
    pub patience: Duration,
}

/// One knock: "can you hear me?", or with `seen`, "and I heard you".
fn send(socket: &UdpSocket, key: &[u8; 32], session: u64, to: SocketAddr, seen: bool) {
    let packet = seal_datagram(key, session, &Frame::Punch { seen });
    // One that cannot leave is no worse than one lost on the way.
    let _ = socket.send_to(&packet, to);
}

/// Waits a moment for a packet signed with `key`; everything else is ignored.
fn listen(socket: &UdpSocket, key: &[u8; 32]) -> Option<(u64, Frame, SocketAddr)> {
    let mut buffer = [0u8; DATAGRAM + 1];
    match socket.recv_from(&mut buffer) {
        Ok((size, from)) => {
            let (session, frame) = open_datagram(key, &buffer[..size])?;
            Some((session, frame, from))
        }
        Err(e) => {
            // Windows reports here that something sent earlier found no one,
            // and does so at once: waiting avoids spinning on it.
            if !matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) {
                std::thread::sleep(Duration::from_millis(2));
            }
            None
        }
    }
}

fn due(last: &mut Option<Instant>, every: Duration) -> bool {
    let now = Instant::now();
    let due = last.is_none_or(|at| now.duration_since(at) >= every);
    if due {
        *last = Some(now);
    }
    due
}

impl HostSide<'_> {
    fn say(&self, state: Punching) {
        if let Ok(mut now) = self.punching.lock() {
            *now = state;
        }
    }

    /// Waits for reply codes and knocks until a guest is heard. `None` once
    /// there is nothing left to wait for.
    pub fn wait(&self) -> Option<Punched> {
        self.socket.set_read_timeout(Some(LISTEN)).ok()?;
        // The address of the latest reply code, and since when.
        let mut target: Option<(SocketAddr, Instant)> = None;
        // Where a knock of theirs came from, and in which attempt.
        let mut heard: Option<(SocketAddr, u64)> = None;
        let mut knocked = None;
        let mut poked = Some(Instant::now());
        while !self.closing.load(Ordering::Relaxed) && !self.filled.load(Ordering::SeqCst) {
            if let Some(address) = self.replies.try_iter().last() {
                target = Some((address.into(), Instant::now()));
                heard = None;
                knocked = None;
                self.say(Punching::Trying);
            }
            if let Some(server) = self.keepalive
                && due(&mut poked, KEEPALIVE)
            {
                stun::poke(self.socket, server);
            }
            if target.is_some_and(|(_, since)| since.elapsed() >= self.patience) {
                target = None;
                self.say(Punching::Blocked);
            }
            // Packets go only to an address the player pasted, or back to
            // where one signed with the room's key came from.
            if let Some((address, _)) = target
                && due(&mut knocked, HOST_EVERY)
            {
                let session = heard.map_or(0, |h| h.1);
                let key = &self.keys.from_host;
                send(self.socket, key, session, address, false);
                if let Some((from, _)) = heard {
                    send(self.socket, key, session, from, true);
                }
            }
            let Some((session, frame, from)) = listen(self.socket, &self.keys.from_guest) else {
                continue;
            };
            if target.is_none() || session == 0 {
                continue;
            }
            match frame {
                // The newest attempt is the one answered.
                Frame::Punch { seen: false } => {
                    heard = Some((from, session));
                    send(self.socket, &self.keys.from_host, session, from, true);
                }
                // Anything else shows that they hear this side too.
                _ => {
                    // Said before leaving: this may be their first knock that
                    // was taken in, and then they have had no proof yet.
                    send(self.socket, &self.keys.from_host, session, from, true);
                    self.say(Punching::Idle);
                    return Some(Punched {
                        peer: from,
                        session,
                    });
                }
            }
        }
        None
    }
}

/// The guest's side: knocks at the host's door until heard or called off.
pub fn knock_from_guest(
    socket: &UdpSocket,
    keys: &PunchKeys,
    host: SocketAddr,
    session: u64,
    cancel: &AtomicBool,
) -> Option<Punched> {
    socket.set_read_timeout(Some(LISTEN)).ok()?;
    let started = Instant::now();
    let mut knocked = None;
    // Where the host's packets really come from, once one has arrived.
    let mut heard: Option<SocketAddr> = None;
    while !cancel.load(Ordering::Relaxed) && started.elapsed() < GUEST_PATIENCE {
        if due(&mut knocked, GUEST_EVERY) {
            let to = heard.unwrap_or(host);
            send(socket, &keys.from_guest, session, to, heard.is_some());
        }
        let Some((theirs, frame, from)) = listen(socket, &keys.from_host) else {
            continue;
        };
        // Before hearing this side the host does not know the attempt yet.
        let hello = theirs == 0 && matches!(frame, Frame::Punch { .. });
        if theirs != session && !hello {
            continue;
        }
        heard = Some(from);
        send(socket, &keys.from_guest, session, from, true);
        // Only a packet that names this attempt shows the host heard it.
        if !hello && !matches!(frame, Frame::Punch { seen: false }) {
            return Some(Punched {
                peer: from,
                session,
            });
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::super::tests::eventually;
    use super::*;
    use std::net::Ipv4Addr;
    use std::sync::Arc;
    use std::sync::mpsc::{Sender, channel};

    fn socket() -> UdpSocket {
        UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).expect("socket")
    }

    fn at(socket: &UdpSocket) -> SocketAddrV4 {
        match socket.local_addr().expect("address") {
            SocketAddr::V4(at) => at,
            SocketAddr::V6(at) => panic!("{at}"),
        }
    }

    /// A cancel flag that goes up on its own.
    fn give_up_after(seconds: u64) -> Arc<AtomicBool> {
        let cancel = Arc::new(AtomicBool::new(false));
        let stop = cancel.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_secs(seconds));
            stop.store(true, Ordering::Relaxed);
        });
        cancel
    }

    /// A host waiting on its own thread.
    struct Waiting {
        reply: Sender<SocketAddrV4>,
        at: SocketAddrV4,
        closing: Arc<AtomicBool>,
        punching: Arc<Mutex<Punching>>,
        done: std::thread::JoinHandle<Option<Punched>>,
    }

    fn waiting(keys: Arc<PunchKeys>, patience: Duration) -> Waiting {
        let socket = socket();
        let address = at(&socket);
        let (reply, replies) = channel();
        let closing = Arc::new(AtomicBool::new(false));
        let punching = Arc::new(Mutex::new(Punching::Idle));
        let (stop, status) = (closing.clone(), punching.clone());
        let done = std::thread::spawn(move || {
            HostSide {
                socket: &socket,
                keys: &keys,
                replies: &replies,
                keepalive: None,
                closing: &stop,
                filled: &AtomicBool::new(false),
                punching: &status,
                patience,
            }
            .wait()
        });
        Waiting {
            reply,
            at: address,
            closing,
            punching,
            done,
        }
    }

    #[test]
    fn two_sides_that_know_where_the_other_is_meet() {
        let keys = Arc::new(PunchKeys::derive(&[4; 6]));
        let host = waiting(keys.clone(), Duration::from_secs(10));
        let guest = socket();
        host.reply.send(at(&guest)).expect("reply");
        let met = knock_from_guest(&guest, &keys, host.at.into(), 77, &give_up_after(10))
            .expect("the guest is heard");
        assert_eq!(met.session, 77);
        assert_eq!(met.peer, SocketAddr::V4(host.at));
        let found = host.done.join().expect("thread").expect("the host hears");
        assert_eq!(found.session, 77);
        assert_eq!(found.peer, guest.local_addr().expect("address"));
    }

    #[test]
    fn the_host_confirms_before_it_stops_knocking() {
        // The guest's first knock can arrive before the host has the reply, and
        // is dropped. What comes next already says "I heard you", and the host
        // must still answer it: the guest has no other proof of being heard.
        let keys = Arc::new(PunchKeys::derive(&[4; 6]));
        let host = waiting(keys.clone(), Duration::from_secs(10));
        let guest = socket();
        guest
            .set_read_timeout(Some(Duration::from_secs(3)))
            .expect("timeout");
        host.reply.send(at(&guest)).expect("reply");
        let mut buffer = [0u8; 2048];
        guest.recv_from(&mut buffer).expect("the host's hello");
        let heard = seal_datagram(&keys.from_guest, 5, &Frame::Punch { seen: true });
        guest.send_to(&heard, host.at).expect("send");
        assert!(host.done.join().expect("thread").is_some());
        let mut confirmed = false;
        while let Ok((size, _)) = guest.recv_from(&mut buffer) {
            let said = open_datagram(&keys.from_host, &buffer[..size]);
            if said == Some((5, Frame::Punch { seen: true })) {
                confirmed = true;
                break;
            }
        }
        assert!(confirmed, "the host left without saying it heard");
    }

    #[test]
    fn the_host_says_when_nothing_gets_through_and_takes_another_reply() {
        let keys = Arc::new(PunchKeys::derive(&[4; 6]));
        let host = waiting(keys, Duration::from_millis(400));
        let nobody = at(&socket());
        let state = || *host.punching.lock().expect("status");
        host.reply.send(nobody).expect("reply");
        eventually(|| (state() == Punching::Blocked).then_some(()));
        host.reply.send(nobody).expect("reply");
        eventually(|| (state() == Punching::Trying).then_some(()));
        host.closing.store(true, Ordering::Relaxed);
        assert!(host.done.join().expect("thread").is_none());
    }

    #[test]
    fn a_guest_who_calls_it_off_stops_knocking() {
        let keys = PunchKeys::derive(&[4; 6]);
        let cancel = Arc::new(AtomicBool::new(false));
        let stop = cancel.clone();
        let started = Instant::now();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(300));
            stop.store(true, Ordering::Relaxed);
        });
        let listener = socket();
        let met = knock_from_guest(&socket(), &keys, at(&listener).into(), 5, &cancel);
        assert!(met.is_none());
        assert!(started.elapsed() >= Duration::from_millis(250));
        assert!(started.elapsed() < Duration::from_secs(3));
        // Meanwhile it really was knocking.
        listener
            .set_read_timeout(Some(Duration::from_millis(100)))
            .expect("timeout");
        let mut buffer = [0u8; 2048];
        assert!(listener.recv_from(&mut buffer).is_ok());
    }
}
