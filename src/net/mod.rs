//! Two players over the internet, a local network or a virtual one: rooms, a
//! checked greeting and signed messages. They travel over TCP when the guest can
//! walk in, and over the game's own reliable stream on UDP when the two have to
//! knock through both routers. All the waiting happens on background threads;
//! the game only ever looks at what has already arrived.

pub mod code;
mod direct;
mod pipe;
pub mod reach;
pub mod secure;
mod stream;
mod stun;
mod upnp;

use babel_core::game::events::{GameEvent, Input};
use babel_core::game::sync::{Action, State};
use babel_core::progression::player::PlayerData;
use code::{ReplyCode, RoomCode};
pub use direct::Punching;
use pipe::Pipe;
use reach::{Probe, Reach};
use secure::{Keys, NONCE, PunchKeys, Seal, read_block, write_block};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::collections::{HashMap, HashSet};
use std::io::{self, Read, Write};
use std::net::{
    IpAddr, Ipv4Addr, Shutdown, SocketAddr, SocketAddrV4, TcpListener, TcpStream, UdpSocket,
};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::{Receiver, Sender, TryRecvError, channel, sync_channel};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Changes whenever the messages change.
const PROTOCOL: u16 = 5;
const MAGIC: [u8; 4] = *b"HXBL";
/// Preferred port, so that a firewall rule keeps working.
pub const PORT: u16 = 47612;
/// Wrong greetings one address may make before it is ignored for good.
const MAX_FAILURES_EACH: u32 = 5;
/// Wrong greetings, from anyone, before the room closes.
const MAX_FAILURES: u32 = 200;
/// Greetings going on at once. Each address gets one.
const MAX_GREETING: usize = 4;
/// A wrong answer is given this late, so that guessing is slow.
const FAILURE_DELAY: Duration = Duration::from_millis(700);
const GREETING_TIME: Duration = Duration::from_secs(10);
/// Both ends talk many times a second; this much silence means it is over.
const SILENCE: Duration = Duration::from_secs(20);
const CONNECT_TIME: Duration = Duration::from_secs(6);
const GREETING_LIMIT: usize = 256;
/// Largest message each end accepts.
const TO_GUEST_LIMIT: usize = 4 * 1024 * 1024;
const TO_HOST_LIMIT: usize = 1024 * 1024;
/// Messages that may wait to be sent before the connection is given up.
const BACKLOG: usize = 900;
/// Messages that may wait to be looked at.
const INBOX: usize = 512;

/// A two-way flow of bytes the greeting and the messages can travel on.
pub(crate) trait Wire: Read + Write + Send + Sized + 'static {
    /// Another handle on the same flow, for the other thread.
    fn another(&self) -> io::Result<Self>;
    /// How long a read or a write may wait.
    fn limits(&self, read: Duration, write: Duration) -> io::Result<()>;
    /// Ends the flow from anywhere.
    fn closer(&self) -> io::Result<Box<dyn Fn() + Send + Sync>>;
}

impl Wire for TcpStream {
    fn another(&self) -> io::Result<TcpStream> {
        self.try_clone()
    }

    fn limits(&self, read: Duration, write: Duration) -> io::Result<()> {
        self.set_nodelay(true)?;
        self.set_read_timeout(Some(read))?;
        self.set_write_timeout(Some(write))
    }

    fn closer(&self) -> io::Result<Box<dyn Fn() + Send + Sync>> {
        let stream = self.try_clone()?;
        Ok(Box::new(move || {
            let _ = stream.shutdown(Shutdown::Both);
        }))
    }
}

/// From the host to the guest.
#[derive(Serialize, Deserialize)]
pub enum ToGuest {
    /// The first message after the guest's data arrived: a complete state.
    Start {
        world_seed: u64,
        state: Box<State>,
    },
    State(Box<State>),
    Events(Vec<GameEvent>),
    /// Who has the game paused. It only stops when both do.
    Pause {
        host: bool,
        guest: bool,
    },
    Bye(String),
}

/// From the guest to the host.
#[derive(Serialize, Deserialize)]
pub enum ToHost {
    Hello(Box<PlayerData>),
    Controls(Input),
    Action(Action),
    Paused(bool),
    Bye,
}

/// Both computers must run the same game: the same messages and the same
/// content, since galleries are generated on each side.
fn version() -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(env!("CARGO_PKG_VERSION").as_bytes());
    for (name, text) in babel_core::data::embedded::FILES {
        hasher.update(name.as_bytes());
        // Line endings must not tell two builds apart.
        for line in text.lines() {
            hasher.update(line.as_bytes());
        }
    }
    *hasher.finalize().as_bytes()
}

#[derive(Serialize, Deserialize)]
struct HostHello {
    magic: [u8; 4],
    protocol: u16,
    version: [u8; 32],
    nonce: [u8; NONCE],
}

#[derive(Serialize, Deserialize)]
struct GuestHello {
    nonce: [u8; NONCE],
    proof: [u8; 32],
}

#[derive(Serialize, Deserialize)]
enum Verdict {
    Welcome([u8; 32]),
    Refused,
}

/// Why a connection could not be made.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Failure {
    Unreachable,
    NotAGame,
    DifferentVersion,
    WrongCode,
    TooManyAttempts,
    DirectBlocked,
    Network(String),
}

impl Failure {
    pub fn message(&self) -> String {
        match self {
            Failure::Unreachable => {
                "No se encontró la sala. El anfitrión debe tenerla abierta y su \
                 router o firewall permitir la conexión."
                    .into()
            }
            Failure::NotAGame => "Ahí no hay una sala de este juego.".into(),
            Failure::DifferentVersion => {
                "Las dos PC tienen que usar la misma versión del juego.".into()
            }
            Failure::WrongCode => "El código no es correcto.".into(),
            Failure::TooManyAttempts => "La sala se cerró por demasiados intentos fallidos.".into(),
            Failure::DirectBlocked => {
                "No se logró la conexión directa. Si la respuesta ya se pegó, alguna de \
                 las dos redes la bloquea: prueben que aloje la otra persona."
                    .into()
            }
            Failure::Network(e) => format!("Error de red: {e}"),
        }
    }
}

impl From<io::Error> for Failure {
    fn from(e: io::Error) -> Failure {
        Failure::Network(e.to_string())
    }
}

fn say<T: Serialize>(to: &mut impl Write, message: &T) -> Result<(), Failure> {
    let bytes = postcard::to_stdvec(message).map_err(|e| Failure::Network(e.to_string()))?;
    Ok(write_block(to, &bytes)?)
}

fn hear<T: DeserializeOwned>(from: &mut impl Read) -> Result<T, Failure> {
    let bytes = read_block(from, GREETING_LIMIT).map_err(|_| Failure::NotAGame)?;
    postcard::from_bytes(&bytes).map_err(|_| Failure::NotAGame)
}

/// The host's side of the greeting.
fn welcome(stream: &mut impl Wire, room: &RoomCode) -> Result<Keys, Failure> {
    stream.limits(GREETING_TIME, GREETING_TIME)?;
    let nonce = secure::random()?;
    say(
        stream,
        &HostHello {
            magic: MAGIC,
            protocol: PROTOCOL,
            version: version(),
            nonce,
        },
    )?;
    let answer: GuestHello = hear(stream)?;
    let keys = Keys::derive(&room.secret, &nonce, &answer.nonce);
    // `blake3::Hash` compares in constant time.
    if keys.guest_proof != blake3::Hash::from(answer.proof) {
        std::thread::sleep(FAILURE_DELAY);
        let _ = say(stream, &Verdict::Refused);
        return Err(Failure::WrongCode);
    }
    say(stream, &Verdict::Welcome(*keys.host_proof.as_bytes()))?;
    Ok(keys)
}

/// The guest's side of the greeting.
fn knock(stream: &mut impl Wire, room: &RoomCode) -> Result<Keys, Failure> {
    stream.limits(GREETING_TIME, GREETING_TIME)?;
    let hello: HostHello = hear(stream)?;
    if hello.magic != MAGIC {
        return Err(Failure::NotAGame);
    }
    if hello.protocol != PROTOCOL || hello.version != version() {
        return Err(Failure::DifferentVersion);
    }
    let nonce = secure::random()?;
    let keys = Keys::derive(&room.secret, &hello.nonce, &nonce);
    say(
        stream,
        &GuestHello {
            nonce,
            proof: *keys.guest_proof.as_bytes(),
        },
    )?;
    match hear(stream)? {
        // The host must know the secret too, or it is an impostor.
        Verdict::Welcome(proof) if keys.host_proof == blake3::Hash::from(proof) => Ok(keys),
        _ => Err(Failure::WrongCode),
    }
}

/// An open, verified connection that sends `Out` and receives `In`.
pub struct Link<Out, In> {
    /// Encoded messages waiting to be sent, and whether each may be skipped.
    outbox: Sender<(Vec<u8>, bool)>,
    /// Behind a lock because the game state is shared between threads.
    inbox: Mutex<Receiver<In>>,
    waiting: Arc<AtomicUsize>,
    open: Arc<AtomicBool>,
    close: Box<dyn Fn() + Send + Sync>,
    /// Kept for as long as the connection lasts, then let go.
    held: Option<Box<dyn Send + Sync>>,
    sends: std::marker::PhantomData<Out>,
}

impl<Out: Serialize, In: DeserializeOwned + Send + 'static> Link<Out, In> {
    fn start<W: Wire>(
        stream: W,
        send_key: [u8; 32],
        receive_key: [u8; 32],
        receive_limit: usize,
    ) -> io::Result<Link<Out, In>> {
        stream.limits(SILENCE, SILENCE)?;
        let open = Arc::new(AtomicBool::new(true));
        let waiting = Arc::new(AtomicUsize::new(0));
        let (outbox, queued) = channel::<(Vec<u8>, bool)>();
        // A full inbox stops the reading, and with it a sender that floods.
        let (arrived, inbox) = sync_channel(INBOX);

        let (mut to, close_after_sending) = (stream.another()?, stream.closer()?);
        let (alive, count) = (open.clone(), waiting.clone());
        std::thread::Builder::new()
            .name("net-send".into())
            .spawn(move || {
                let mut seal = Seal::new(send_key);
                let mut pending = Vec::new();
                while let Ok(first) = queued.recv() {
                    pending.push(first);
                    pending.extend(queued.try_iter());
                    count.fetch_sub(pending.len(), Ordering::Relaxed);
                    // Of the states that pile up only the newest matters.
                    let newest = pending.iter().rposition(|m| m.1);
                    for (i, (bytes, skippable)) in pending.drain(..).enumerate() {
                        if skippable && Some(i) != newest {
                            continue;
                        }
                        if write_block(&mut to, &seal.seal(bytes)).is_err() {
                            alive.store(false, Ordering::Relaxed);
                            return;
                        }
                    }
                }
                // The link was dropped after saying all it had to say.
                close_after_sending();
            })?;

        let (mut from, close_after_receiving) = (stream.another()?, stream.closer()?);
        let alive = open.clone();
        std::thread::Builder::new()
            .name("net-receive".into())
            .spawn(move || {
                let mut seal = Seal::new(receive_key);
                loop {
                    // Anything unsigned, oversized or unreadable ends it.
                    let message = read_block(&mut from, receive_limit + 16)
                        .and_then(|bytes| seal.open(bytes))
                        .ok()
                        .and_then(|bytes| postcard::from_bytes(&bytes).ok());
                    let delivered = message.is_some_and(|m| arrived.send(m).is_ok());
                    if !delivered {
                        break;
                    }
                }
                alive.store(false, Ordering::Relaxed);
                close_after_receiving();
            })?;

        Ok(Link {
            outbox,
            inbox: Mutex::new(inbox),
            waiting,
            open,
            close: stream.closer()?,
            held: None,
            sends: std::marker::PhantomData,
        })
    }

    fn hold(&mut self, what: impl Send + Sync + 'static) {
        self.held = Some(Box::new(what));
    }

    fn queue(&self, message: &Out, skippable: bool) {
        let Ok(bytes) = postcard::to_stdvec(message) else {
            return;
        };
        if self.waiting.fetch_add(1, Ordering::Relaxed) >= BACKLOG {
            self.hang_up();
            return;
        }
        let _ = self.outbox.send((bytes, skippable));
    }

    pub fn send(&self, message: &Out) {
        self.queue(message, false);
    }

    /// For news that the next one replaces, if the network falls behind.
    pub fn send_latest(&self, message: &Out) {
        self.queue(message, true);
    }

    /// Everything that has arrived, and whether the connection is still open.
    pub fn receive(&self) -> (Vec<In>, bool) {
        // Read the flag first, so that a farewell is never missed.
        let open = self.open.load(Ordering::Relaxed);
        let arrived = self
            .inbox
            .lock()
            // No more than one queue's worth per look, however fast it refills.
            .map_or_else(
                |_| Vec::new(),
                |inbox| inbox.try_iter().take(INBOX).collect(),
            );
        (arrived, open)
    }

    fn hang_up(&self) {
        self.open.store(false, Ordering::Relaxed);
        (self.close)();
    }
}

pub type HostLink = Link<ToGuest, ToHost>;
pub type GuestLink = Link<ToHost, ToGuest>;

/// Who is allowed to knock, and how often.
#[derive(Default)]
struct Gate {
    failures: HashMap<IpAddr, u32>,
    greeting: HashSet<IpAddr>,
    total: u32,
}

impl Gate {
    /// Whether this visitor may start a greeting now.
    fn admit(&mut self, who: IpAddr) -> bool {
        self.failures
            .get(&who)
            .is_none_or(|n| *n < MAX_FAILURES_EACH)
            && self.greeting.len() < MAX_GREETING
            && self.greeting.insert(who)
    }

    /// A greeting ended. Whether there have been too many wrong ones.
    fn done(&mut self, who: IpAddr, failed: bool) -> bool {
        self.greeting.remove(&who);
        if failed {
            *self.failures.entry(who).or_default() += 1;
            self.total += 1;
        }
        self.total >= MAX_FAILURES
    }
}

/// A room waiting for its second player. Dropping it closes the port.
pub struct Room {
    /// How to reach it from this network.
    pub code: RoomCode,
    arrivals: Mutex<Receiver<Result<HostLink, Failure>>>,
    closing: Arc<AtomicBool>,
    reach: Arc<Mutex<Reach>>,
    /// The router's forwarding, once made. It goes to the connection that
    /// arrives, so that it ends with it.
    mapping: Arc<Mutex<Option<upnp::Mapping>>>,
    /// Whether somebody already got in, by either way.
    filled: Arc<AtomicBool>,
    arrived: Sender<Result<HostLink, Failure>>,
    /// The direct way, once there is one.
    direct: Arc<Mutex<Option<DirectDoor>>>,
    punching: Arc<Mutex<Punching>>,
}

/// What the game needs to pass a reply code on to the direct way.
struct DirectDoor {
    keys: Arc<PunchKeys>,
    replies: Sender<SocketAddrV4>,
}

/// Why a reply code cannot be used.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnswerError {
    /// It answers some other room.
    OtherRoom,
    /// This room has no direct way, or not yet.
    NotReady,
}

impl Drop for Room {
    fn drop(&mut self) {
        self.closing.store(true, Ordering::Relaxed);
        // Taken out here so that the router is told, unless it went to a guest.
        let _ = self.mapping.lock().map(|mut m| m.take());
    }
}

/// The address other computers of the local network reach this one by.
pub fn local_address() -> Ipv4Addr {
    // Choosing a route sends nothing. 192.0.2.1 is reserved for examples.
    let found = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)).and_then(|s| {
        s.connect((Ipv4Addr::new(192, 0, 2, 1), 9))?;
        s.local_addr()
    });
    match found.map(|a| a.ip()) {
        Ok(IpAddr::V4(ip)) if !ip.is_unspecified() => ip,
        _ => Ipv4Addr::LOCALHOST,
    }
}

/// Greets one visitor, on a thread of their own, and hands over the link if
/// they know the secret.
fn greet(
    mut stream: TcpStream,
    who: IpAddr,
    code: RoomCode,
    gate: &Mutex<Gate>,
    arrived: &Sender<Result<HostLink, Failure>>,
    filled: &AtomicBool,
) {
    let greeted = stream
        .set_nonblocking(false)
        .map_err(Failure::from)
        .and_then(|()| welcome(&mut stream, &code))
        .and_then(|keys| {
            Link::start(stream, keys.to_guest, keys.to_host, TO_HOST_LIMIT).map_err(Failure::from)
        });
    let too_many = gate
        .lock()
        .map_or(true, |mut gate| gate.done(who, greeted.is_err()));
    match greeted {
        // Whoever is first takes the place.
        Ok(link) => {
            if !filled.swap(true, Ordering::SeqCst) {
                let _ = arrived.send(Ok(link));
            }
        }
        Err(_) if too_many => {
            if !filled.swap(true, Ordering::SeqCst) {
                let _ = arrived.send(Err(Failure::TooManyAttempts));
            }
        }
        Err(_) => {}
    }
}

impl Room {
    /// Opens a room. `bind` is where to listen and `advertised` the address
    /// written into the code.
    pub fn open(bind: Ipv4Addr, advertised: Ipv4Addr) -> Result<Room, Failure> {
        let listener = TcpListener::bind((bind, PORT)).or_else(|_| TcpListener::bind((bind, 0)))?;
        listener.set_nonblocking(true)?;
        let port = listener.local_addr()?.port();
        let code = RoomCode {
            address: SocketAddrV4::new(advertised, port),
            udp: 0,
            secret: secure::random()?,
        };
        let (arrived, arrivals) = channel();
        let closing = Arc::new(AtomicBool::new(false));
        let stop = closing.clone();
        let filled = Arc::new(AtomicBool::new(false));
        let (taken, announce) = (filled.clone(), arrived.clone());
        std::thread::Builder::new()
            .name("net-room".into())
            .spawn(move || {
                let gate = Arc::new(Mutex::new(Gate::default()));
                let (filled, arrived) = (taken, announce);
                // The listener, and with it the port, lives only in here.
                while !stop.load(Ordering::Relaxed) && !filled.load(Ordering::SeqCst) {
                    let Ok((stream, peer)) = listener.accept() else {
                        std::thread::sleep(Duration::from_millis(40));
                        continue;
                    };
                    let who = peer.ip();
                    let admitted = gate.lock().is_ok_and(|mut gate| gate.admit(who));
                    if !admitted {
                        // Not even a greeting is wasted on them.
                        let _ = stream.shutdown(Shutdown::Both);
                        continue;
                    }
                    let (gate, arrived, filled) = (gate.clone(), arrived.clone(), filled.clone());
                    let started = std::thread::Builder::new()
                        .name("net-greeting".into())
                        .spawn({
                            let gate = gate.clone();
                            move || greet(stream, who, code, &gate, &arrived, &filled)
                        });
                    if started.is_err() {
                        let _ = gate.lock().map(|mut gate| gate.done(who, false));
                    }
                }
            })?;
        Ok(Room {
            code,
            arrivals: Mutex::new(arrivals),
            closing,
            reach: Arc::new(Mutex::new(Reach::Searching)),
            mapping: Arc::new(Mutex::new(None)),
            filled,
            arrived,
            direct: Arc::new(Mutex::new(None)),
            punching: Arc::new(Mutex::new(Punching::Idle)),
        })
    }

    /// Starts finding out how to reach this room from other networks, and then
    /// waits on the direct way for whoever cannot walk in.
    pub fn reach_the_internet(&self, probe: Probe) {
        let (code, closing) = (self.code, self.closing.clone());
        let (reach, mapping) = (self.reach.clone(), self.mapping.clone());
        let (filled, arrived) = (self.filled.clone(), self.arrived.clone());
        let (door, punching) = (self.direct.clone(), self.punching.clone());
        let _ = std::thread::Builder::new()
            .name("net-reach".into())
            .spawn(move || {
                let local = if probe.loopback {
                    Ipv4Addr::LOCALHOST
                } else {
                    Ipv4Addr::UNSPECIFIED
                };
                let Ok(socket) = UdpSocket::bind((local, code.address.port()))
                    .or_else(|_| UdpSocket::bind((local, 0)))
                else {
                    let _ = reach.lock().map(|mut now| *now = Reach::Offline);
                    return;
                };
                // Slow on purpose, so it is done here and not on the game's thread.
                let keys = Arc::new(PunchKeys::derive(&code.secret));
                let found = reach::find(&probe, &code, &socket);
                let way = matches!(found.reach, Reach::Open(c) | Reach::Direct(c) if c.udp != 0);
                let (reply_to, replies) = channel();
                // Under the lock, so that closing the room cannot slip between
                // the check and the storing: the router would keep the port.
                if let Ok(mut slot) = mapping.lock()
                    && !closing.load(Ordering::Relaxed)
                {
                    *slot = found.mapping;
                    if way && let Ok(mut door) = door.lock() {
                        *door = Some(DirectDoor {
                            keys: keys.clone(),
                            replies: reply_to,
                        });
                    }
                    if let Ok(mut now) = reach.lock() {
                        *now = found.reach;
                    }
                }
                if way {
                    DirectHost {
                        socket,
                        keys,
                        room: code,
                        replies,
                        keepalive: found.keepalive,
                        closing,
                        filled,
                        punching,
                        arrived,
                    }
                    .serve();
                }
            });
    }

    /// Takes the guest's reply code and starts knocking back.
    pub fn answer(&self, reply: ReplyCode) -> Result<(), AnswerError> {
        let door = self.direct.lock().map_err(|_| AnswerError::NotReady)?;
        let door = door.as_ref().ok_or(AnswerError::NotReady)?;
        if !reply.belongs_to(&door.keys) {
            return Err(AnswerError::OtherRoom);
        }
        if let Ok(mut now) = self.punching.lock() {
            *now = Punching::Trying;
        }
        door.replies
            .send(reply.address())
            .map_err(|_| AnswerError::NotReady)
    }

    /// How knocking back is going.
    pub fn punching(&self) -> Punching {
        self.punching.lock().map_or(Punching::Idle, |p| *p)
    }

    #[cfg(test)]
    pub fn pretend(&self, outcome: Reach) {
        if let Ok(mut now) = self.reach.lock() {
            *now = outcome;
        }
    }

    /// How the room can be reached from outside this network.
    pub fn reach(&self) -> Reach {
        self.reach.lock().map_or(Reach::Offline, |r| *r)
    }

    /// The code to give a friend, and the one for the same network if that is
    /// another.
    pub fn codes(&self) -> (RoomCode, Option<RoomCode>) {
        match self.reach() {
            Reach::Open(code) | Reach::Direct(code) => (code, Some(self.code)),
            _ => (self.code, None),
        }
    }

    /// The guest, once one has come in.
    pub fn arrival(&self) -> Option<Result<HostLink, Failure>> {
        let mut outcome = self.arrivals.lock().ok()?.try_recv().ok()?;
        if let Ok(link) = &mut outcome
            && let Some(mapping) = self.mapping.lock().ok().and_then(|mut m| m.take())
        {
            link.hold(mapping);
        }
        Some(outcome)
    }
}

/// The host's end of the direct way, on its own thread.
struct DirectHost {
    socket: UdpSocket,
    keys: Arc<PunchKeys>,
    room: RoomCode,
    replies: Receiver<SocketAddrV4>,
    keepalive: Option<SocketAddr>,
    closing: Arc<AtomicBool>,
    filled: Arc<AtomicBool>,
    punching: Arc<Mutex<Punching>>,
    arrived: Sender<Result<HostLink, Failure>>,
}

impl DirectHost {
    fn serve(self) {
        let waiting = direct::HostSide {
            socket: &self.socket,
            keys: &self.keys,
            replies: &self.replies,
            keepalive: self.keepalive,
            closing: &self.closing,
            filled: &self.filled,
            punching: &self.punching,
            patience: direct::HOST_PATIENCE,
        };
        while let Some(met) = waiting.wait() {
            let greeted = self
                .socket
                .try_clone()
                .map_err(Failure::from)
                .and_then(|socket| {
                    let mut pipe = Pipe::start(
                        socket,
                        met.peer,
                        met.session,
                        self.keys.from_host,
                        self.keys.from_guest,
                    )?;
                    let keys = welcome(&mut pipe, &self.room)?;
                    Ok(Link::start(
                        pipe,
                        keys.to_guest,
                        keys.to_host,
                        TO_HOST_LIMIT,
                    )?)
                });
            match greeted {
                Ok(link) => {
                    if !self.filled.swap(true, Ordering::SeqCst) {
                        let _ = self.arrived.send(Ok(link));
                    }
                    return;
                }
                // Another reply may still work.
                Err(_) => {
                    let _ = self.punching.lock().map(|mut p| *p = Punching::Blocked);
                }
            }
        }
    }
}

/// A connection being made in the background. Dropping it calls it off.
pub struct Joining {
    result: Mutex<Receiver<Result<GuestLink, Failure>>>,
    reply: Arc<Mutex<Option<ReplyCode>>>,
    cancel: Arc<AtomicBool>,
}

impl Drop for Joining {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}

/// Walks in through the front door.
fn join_by_tcp(room: &RoomCode) -> Result<GuestLink, Failure> {
    let mut stream = TcpStream::connect_timeout(&SocketAddr::V4(room.address), CONNECT_TIME)
        .map_err(|_| Failure::Unreachable)?;
    let keys = knock(&mut stream, room)?;
    Ok(Link::start(
        stream,
        keys.to_host,
        keys.to_guest,
        TO_GUEST_LIMIT,
    )?)
}

/// Knocks from this side while the host knocks from theirs, which they can do
/// once they have the reply code left in `reply`.
fn join_directly(
    room: &RoomCode,
    probe: &Probe,
    reply: &Mutex<Option<ReplyCode>>,
    cancel: &AtomicBool,
) -> Result<GuestLink, Failure> {
    let local = if probe.loopback {
        Ipv4Addr::LOCALHOST
    } else {
        Ipv4Addr::UNSPECIFIED
    };
    let socket = UdpSocket::bind((local, 0))?;
    let keys = PunchKeys::derive(&room.secret);
    let (seen, _) = probe.mapped_address(&socket).ok_or(Failure::Unreachable)?;
    if let Ok(mut slot) = reply.lock() {
        *slot = Some(ReplyCode::new(seen, &keys));
    }
    // Zero stands for "not known yet".
    let session = u64::from_le_bytes(secure::random()?).max(1);
    let host = SocketAddr::V4(SocketAddrV4::new(*room.address.ip(), room.udp));
    let met = direct::knock_from_guest(&socket, &keys, host, session, cancel)
        .ok_or(Failure::DirectBlocked)?;
    let mut pipe = Pipe::start(
        socket,
        met.peer,
        met.session,
        keys.from_guest,
        keys.from_host,
    )?;
    let keys = knock(&mut pipe, room)?;
    Ok(Link::start(
        pipe,
        keys.to_host,
        keys.to_guest,
        TO_GUEST_LIMIT,
    )?)
}

impl Joining {
    pub fn start(room: RoomCode, probe: Probe) -> Joining {
        let (done, result) = channel();
        let reply = Arc::new(Mutex::new(None));
        let cancel = Arc::new(AtomicBool::new(false));
        let (answer, stop) = (reply.clone(), cancel.clone());
        let job = move || {
            let connected = join_by_tcp(&room).or_else(|e| match e {
                // The front door is closed, or something else answered there.
                Failure::Unreachable | Failure::NotAGame if room.udp != 0 => {
                    join_directly(&room, &probe, &answer, &stop)
                }
                other => Err(other),
            });
            let _ = done.send(connected);
        };
        let _ = std::thread::Builder::new()
            .name("net-join".into())
            .spawn(job);
        Joining {
            result: Mutex::new(result),
            reply,
            cancel,
        }
    }

    /// The code the host needs, once the direct way is being tried.
    pub fn reply(&self) -> Option<ReplyCode> {
        self.reply.lock().ok().and_then(|r| *r)
    }

    pub fn outcome(&self) -> Option<Result<GuestLink, Failure>> {
        let Ok(result) = self.result.lock() else {
            return Some(Err(Failure::Unreachable));
        };
        match result.try_recv() {
            Ok(outcome) => Some(outcome),
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => Some(Err(Failure::Unreachable)),
        }
    }
}

/// The `--net-check` tool: says whether a room opened from this computer
/// could be reached from another network, by really trying it for a moment.
pub fn check() {
    let lan = local_address();
    println!("Dirección de esta PC en tu red: {lan}");
    println!("Buscando el router y la dirección pública…");
    let room = RoomCode {
        address: SocketAddrV4::new(lan, PORT),
        udp: 0,
        secret: [0; code::SECRET_BYTES],
    };
    let probe = Probe::real(lan);
    let Ok(socket) = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)) else {
        println!("No se pudo usar la red.");
        return;
    };
    let found = reach::find(&probe, &room, &socket);
    match found.reach {
        Reach::Open(code) => {
            println!(
                "LISTO: el router abrió el puerto {} solo. Una sala creada acá se alcanza \
                 desde internet sin pasos extra.",
                code.address.port()
            );
            println!("Dirección pública: {}", code.address.ip());
        }
        Reach::Direct(code) => {
            println!("El router no abrió el puerto solo (UPnP apagado o ausente).");
            println!("Dirección pública: {}", code.address.ip());
            println!("Se usará la conexión directa con código de respuesta.");
        }
        Reach::Offline | Reach::Searching => {
            println!("No se encontró internet: solo se puede jugar en tu misma red.");
        }
    }
    let names: Vec<&str> = probe.stun.iter().map(String::as_str).collect();
    match stun::keeps_port(&socket, &names) {
        Some(true) => println!("Conexión directa: esta red es apta (conserva el puerto)."),
        Some(false) => println!(
            "Conexión directa: esta red cambia de puerto según el destino; puede fallar. \
             Si falla, que aloje la otra persona."
        ),
        None => println!("Conexión directa: no se pudo comprobar."),
    }
    // Dropping it closes the port again.
    drop(found.mapping);
    println!("Puerto cerrado de nuevo.");
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::time::Instant;

    /// Waits for something to happen on a background thread.
    pub(crate) fn eventually<T>(mut ready: impl FnMut() -> Option<T>) -> T {
        let started = Instant::now();
        loop {
            if let Some(value) = ready() {
                return value;
            }
            assert!(started.elapsed() < Duration::from_secs(20), "timed out");
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    fn room() -> Room {
        Room::open(Ipv4Addr::LOCALHOST, Ipv4Addr::LOCALHOST).expect("room")
    }

    fn joining(code: RoomCode) -> Joining {
        Joining::start(code, Probe::loopback())
    }

    fn pair() -> (Room, HostLink, GuestLink) {
        let room = room();
        let joining = joining(room.code);
        let guest = eventually(|| joining.outcome()).expect("joined");
        let host = eventually(|| room.arrival()).expect("arrived");
        (room, host, guest)
    }

    #[test]
    fn messages_travel_both_ways_in_order() {
        let (_room, host, guest) = pair();
        for i in 0..200 {
            guest.send(&ToHost::Paused(i % 2 == 0));
        }
        host.send(&ToGuest::Bye("chau".into()));
        let mut got = Vec::new();
        eventually(|| {
            got.extend(host.receive().0);
            (got.len() == 200).then_some(())
        });
        let ordered = got
            .iter()
            .enumerate()
            .all(|(i, m)| matches!(m, ToHost::Paused(p) if *p == (i % 2 == 0)));
        assert!(ordered);
        let farewell = eventually(|| guest.receive().0.pop());
        assert!(matches!(farewell, ToGuest::Bye(text) if text == "chau"));
    }

    #[test]
    fn dropping_one_end_is_noticed_by_the_other() {
        let (_room, host, guest) = pair();
        guest.send(&ToHost::Bye);
        drop(guest);
        let mut farewell = false;
        eventually(|| {
            let (messages, open) = host.receive();
            farewell |= messages.iter().any(|m| matches!(m, ToHost::Bye));
            (!open).then_some(())
        });
        farewell |= host.receive().0.iter().any(|m| matches!(m, ToHost::Bye));
        assert!(farewell, "the goodbye arrived before the connection closed");
    }

    #[test]
    fn a_wrong_secret_is_turned_away_and_the_room_stays_open() {
        let room = room();
        let mut wrong = room.code;
        wrong.secret[0] ^= 1;
        let intruder = joining(wrong);
        let refused = eventually(|| intruder.outcome());
        assert_eq!(refused.err(), Some(Failure::WrongCode));
        assert!(room.arrival().is_none());
        let friend = joining(room.code);
        assert!(eventually(|| friend.outcome()).is_ok());
    }

    #[test]
    fn a_stranger_who_keeps_guessing_is_ignored_and_a_friend_still_gets_in() {
        let room = room();
        let address = room.code.address;
        // Garbage instead of a greeting, from this one address.
        let mut ignored = false;
        for _ in 0..30 {
            if let Ok(mut stream) = TcpStream::connect(address) {
                let _ = stream.set_read_timeout(Some(Duration::from_secs(3)));
                let mut first = [0u8; 4];
                // A greeting is offered first; an ignored address gets nothing.
                if stream.read(&mut first).unwrap_or(0) == 0 {
                    ignored = true;
                    break;
                }
                let _ = stream.write_all(&[0xFF; 64]);
                let _ = stream.read(&mut [0; 8]);
            }
        }
        assert!(ignored, "after a few failures the address is not greeted");
        assert!(room.arrival().is_none(), "and the room is still open");
    }

    #[test]
    fn the_gate_limits_each_address_and_all_of_them() {
        let mut gate = Gate::default();
        let one: IpAddr = "203.0.113.1".parse().expect("ip");
        assert!(gate.admit(one));
        assert!(!gate.admit(one), "one greeting at a time each");
        for _ in 0..MAX_FAILURES_EACH {
            gate.done(one, true);
        }
        assert!(!gate.admit(one), "too many failures");
        let others: Vec<IpAddr> = (2..=5)
            .map(|n| format!("203.0.113.{n}").parse().expect("ip"))
            .collect();
        assert_eq!(
            others.iter().filter(|ip| gate.admit(**ip)).count(),
            MAX_GREETING
        );
        // Many addresses together close the room in the end.
        let mut gate = Gate::default();
        let closed = (0..MAX_FAILURES).any(|n| {
            let ip: IpAddr = format!("198.51.{}.{}", n / 250, n % 250 + 1)
                .parse()
                .expect("ip");
            gate.admit(ip);
            gate.done(ip, true)
        });
        assert!(closed);
    }

    #[test]
    fn closing_a_room_closes_its_port() {
        let room = room();
        let address = room.code.address;
        assert!(TcpStream::connect(address).is_ok());
        drop(room);
        eventually(|| TcpStream::connect(address).is_err().then_some(()));
    }

    #[test]
    fn a_flood_waits_instead_of_filling_memory() {
        let (_room, host, guest) = pair();
        for _ in 0..800 {
            guest.send(&ToHost::Paused(true));
        }
        std::thread::sleep(Duration::from_millis(300));
        let first = eventually(|| {
            let (messages, _) = host.receive();
            (!messages.is_empty()).then_some(messages)
        });
        assert!(first.len() <= INBOX);
        let mut total = first.len();
        eventually(|| {
            total += host.receive().0.len();
            (total == 800).then_some(())
        });
    }

    fn reaching_out_through(router: &upnp::fake::Fake) -> Probe {
        Probe {
            search: router.search,
            wait: Duration::from_millis(300),
            stun: Vec::new(),
            lan: Ipv4Addr::LOCALHOST,
            loopback: false,
        }
    }

    fn mappings(router: &upnp::fake::Fake) -> usize {
        router.state.lock().expect("router").mappings.len()
    }

    #[test]
    fn the_forwarded_port_stays_with_the_connection_and_ends_with_it() {
        let router = upnp::fake::router("93.184.216.34");
        let room = room();
        room.reach_the_internet(reaching_out_through(&router));
        eventually(|| matches!(room.reach(), Reach::Open(_)).then_some(()));
        let (friends, same_network) = room.codes();
        assert_eq!(*friends.address.ip(), Ipv4Addr::new(93, 184, 216, 34));
        assert_eq!(same_network, Some(room.code));
        assert_eq!(mappings(&router), 1);

        let joining = joining(room.code);
        let guest = eventually(|| joining.outcome()).expect("joined");
        let host = eventually(|| room.arrival()).expect("arrived");
        // The room is gone; the router keeps forwarding for the game in progress.
        drop(room);
        assert_eq!(mappings(&router), 1);
        drop(host);
        assert_eq!(mappings(&router), 0, "closed with the connection");
        drop(guest);
    }

    #[test]
    fn closing_a_room_nobody_entered_closes_the_forwarded_port() {
        let router = upnp::fake::router("93.184.216.34");
        let room = room();
        room.reach_the_internet(reaching_out_through(&router));
        eventually(|| matches!(room.reach(), Reach::Open(_)).then_some(()));
        assert_eq!(mappings(&router), 1);
        drop(room);
        assert_eq!(mappings(&router), 0);

        // Closed while still looking: nothing is left behind either.
        let room = self::room();
        room.reach_the_internet(reaching_out_through(&router));
        drop(room);
        std::thread::sleep(Duration::from_millis(1500));
        assert_eq!(mappings(&router), 0);
    }

    #[test]
    fn an_unsigned_message_ends_the_connection() {
        let room = room();
        let mut stream = TcpStream::connect(room.code.address).expect("connect");
        knock(&mut stream, &room.code).expect("greeting");
        let host = eventually(|| room.arrival()).expect("arrived");
        let forged = postcard::to_stdvec(&ToHost::Paused(true)).expect("encode");
        let mut unsigned = forged;
        unsigned.extend([0u8; 16]);
        write_block(&mut stream, &unsigned).expect("write");
        let mut delivered = 0;
        eventually(|| {
            let (messages, open) = host.receive();
            delivered += messages.len();
            (!open).then_some(())
        });
        assert_eq!(delivered, 0);
    }

    /// A room on this computer whose front door is closed: only the direct way
    /// leads in. Returns the code its owner would pass along.
    fn behind_walls() -> (Room, RoomCode) {
        let room = room();
        room.reach_the_internet(Probe::loopback());
        let code = eventually(|| match room.reach() {
            Reach::Direct(code) => Some(code),
            _ => None,
        });
        (room, code)
    }

    /// A guest who could not walk in and now has a reply code to send back.
    fn knocking(code: RoomCode) -> (Joining, ReplyCode) {
        let joining = joining(code);
        let reply = eventually(|| {
            if let Some(early) = joining.outcome() {
                panic!("ended before any reply: {:?}", early.err());
            }
            joining.reply()
        });
        (joining, reply)
    }

    /// The host takes the reply and both end up connected.
    fn let_in(room: &Room, joining: &Joining, reply: ReplyCode) -> (HostLink, GuestLink) {
        room.answer(reply).expect("answer");
        let guest = eventually(|| joining.outcome()).expect("joined");
        let host = eventually(|| room.arrival()).expect("arrived");
        (host, guest)
    }

    fn through_the_walls() -> (Room, HostLink, GuestLink) {
        let (room, code) = behind_walls();
        let (joining, reply) = knocking(code);
        let (host, guest) = let_in(&room, &joining, reply);
        (room, host, guest)
    }

    #[test]
    fn a_closed_front_door_is_passed_by_with_a_reply_code() {
        let (_room, host, guest) = through_the_walls();
        for i in 0..200 {
            guest.send(&ToHost::Paused(i % 2 == 0));
        }
        let long: String = "babel".repeat(120_000);
        host.send(&ToGuest::Bye(long.clone()));
        let mut got = Vec::new();
        eventually(|| {
            got.extend(host.receive().0);
            (got.len() == 200).then_some(())
        });
        let ordered = got
            .iter()
            .enumerate()
            .all(|(i, m)| matches!(m, ToHost::Paused(p) if *p == (i % 2 == 0)));
        assert!(ordered);
        let arrived = eventually(|| guest.receive().0.pop());
        assert!(matches!(arrived, ToGuest::Bye(text) if text == long));
    }

    #[test]
    fn a_farewell_sent_just_before_leaving_still_arrives() {
        for from_host in [true, false] {
            let (_room, host, guest) = through_the_walls();
            let mut farewell = false;
            if from_host {
                host.send(&ToGuest::Bye("chau".into()));
                drop(host);
                eventually(|| {
                    let (messages, open) = guest.receive();
                    farewell |= messages.iter().any(|m| matches!(m, ToGuest::Bye(_)));
                    (!open).then_some(())
                });
                farewell |= !guest.receive().0.is_empty();
            } else {
                guest.send(&ToHost::Bye);
                drop(guest);
                eventually(|| {
                    let (messages, open) = host.receive();
                    farewell |= messages.iter().any(|m| matches!(m, ToHost::Bye));
                    (!open).then_some(())
                });
                farewell |= !host.receive().0.is_empty();
            }
            assert!(farewell, "from the host: {from_host}");
        }
    }

    #[test]
    fn a_reply_from_another_room_is_refused_and_the_room_stays() {
        let (room, code) = behind_walls();
        let (joining, reply) = knocking(code);
        let other = PunchKeys::derive(&[0; code::SECRET_BYTES]);
        let stray = ReplyCode::new(reply.address(), &other);
        assert_eq!(room.answer(stray), Err(AnswerError::OtherRoom));
        assert_eq!(room.punching(), Punching::Idle);
        assert!(room.arrival().is_none());
        let_in(&room, &joining, reply);
    }

    #[test]
    fn a_room_with_no_direct_way_takes_no_reply() {
        let room = room();
        let keys = PunchKeys::derive(&room.code.secret);
        let reply = ReplyCode::new("127.0.0.1:9".parse().expect("address"), &keys);
        assert_eq!(room.answer(reply), Err(AnswerError::NotReady));
    }

    #[test]
    fn nobody_gets_in_before_the_host_pastes_the_reply() {
        let (room, code) = behind_walls();
        let (joining, _reply) = knocking(code);
        std::thread::sleep(Duration::from_millis(900));
        assert!(room.arrival().is_none());
        assert!(joining.outcome().is_none());
    }

    #[test]
    fn a_guest_who_gives_up_and_tries_again_gets_in() {
        let (room, code) = behind_walls();
        let (first, old) = knocking(code);
        drop(first);
        // The host pastes the old reply before the new one arrives: nobody is
        // there any more, and Windows says so with an error on the socket.
        std::thread::sleep(Duration::from_millis(200));
        room.answer(old).expect("answer");
        std::thread::sleep(Duration::from_millis(600));
        let (second, new) = knocking(code);
        let_in(&room, &second, new);
    }

    #[test]
    fn strangers_datagrams_are_ignored() {
        let (room, code) = behind_walls();
        let door = (Ipv4Addr::LOCALHOST, code.udp);
        let stranger = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).expect("stranger");
        let forged = secure::seal_datagram(&[1; 32], 5, &stream::Frame::Punch { seen: true });
        let noise = |stranger: &UdpSocket| {
            for packet in [&forged[..], &[0xAA; 900], &[1, 2, 3], &[]] {
                stranger.send_to(packet, door).expect("send");
            }
        };
        noise(&stranger);
        let (joining, reply) = knocking(code);
        room.answer(reply).expect("answer");
        for _ in 0..20 {
            noise(&stranger);
        }
        let guest = eventually(|| joining.outcome()).expect("joined");
        let host = eventually(|| room.arrival()).expect("arrived");
        noise(&stranger);
        guest.send(&ToHost::Paused(true));
        let heard = eventually(|| host.receive().0.pop());
        assert!(matches!(heard, ToHost::Paused(true)));
    }

    #[test]
    fn closing_the_room_frees_both_ports() {
        let (room, code) = behind_walls();
        let front = room.code.address;
        drop(room);
        eventually(|| TcpStream::connect(front).is_err().then_some(()));
        eventually(|| UdpSocket::bind((Ipv4Addr::LOCALHOST, code.udp)).ok());
    }

    #[test]
    fn a_code_without_a_direct_way_fails_as_before() {
        let mut code = room().code;
        code.address.set_port(1);
        let joining = joining(code);
        let outcome = eventually(|| joining.outcome());
        assert_eq!(outcome.err(), Some(Failure::Unreachable));
        assert_eq!(joining.reply(), None);
    }
}
