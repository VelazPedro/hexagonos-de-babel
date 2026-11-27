//! A [`Stream`](super::stream::Stream) on a real socket, with a thread that
//! listens and resends. From outside it reads and writes like a connection.

use super::secure::{DATAGRAM, open_datagram, seal_datagram};
use super::stream::{Frame, Stream};
use super::{SILENCE, Wire};
use std::io::{self, ErrorKind, Read, Write};
use std::net::{SocketAddr, UdpSocket};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::time::{Duration, Instant};

/// How often the thread looks for something to resend when nothing arrives.
const TICK: Duration = Duration::from_millis(5);
/// After closing, how long what was written may take to arrive.
const LINGER: Duration = Duration::from_secs(3);
/// And how long nothing new must be written before saying goodbye: the thread
/// that sends may still be handing over the last messages.
const QUIET: Duration = Duration::from_millis(200);

/// One handle on the connection. It ends when the last handle is dropped.
#[derive(Clone)]
pub struct Pipe {
    shared: Arc<Shared>,
    _last: Arc<CloseOnDrop>,
}

struct CloseOnDrop(Arc<Shared>);

impl Drop for CloseOnDrop {
    fn drop(&mut self) {
        self.0.close();
    }
}

struct Shared {
    socket: UdpSocket,
    peer: SocketAddr,
    session: u64,
    send_key: [u8; 32],
    receive_key: [u8; 32],
    state: Mutex<State>,
    /// Told whenever something changed that a reader or writer waits for.
    news: Condvar,
}

struct State {
    stream: Stream,
    read_wait: Duration,
    write_wait: Duration,
    /// Since when this end wants to close.
    closing: Option<Instant>,
    wrote: Instant,
    /// Nothing more will be sent or received.
    over: bool,
}

impl Shared {
    fn lock(&self) -> io::Result<MutexGuard<'_, State>> {
        self.state.lock().map_err(|_| ErrorKind::BrokenPipe.into())
    }

    fn send(&self, frame: &Frame) {
        let packet = seal_datagram(&self.send_key, self.session, frame);
        // One that cannot leave is resent like one lost on the way.
        let _ = self.socket.send_to(&packet, self.peer);
    }

    fn flush(&self, state: &mut State) {
        while let Some(frame) = state.stream.transmit(Instant::now()) {
            self.send(&frame);
        }
    }

    fn close(&self) {
        if let Ok(mut state) = self.state.lock() {
            state.closing.get_or_insert_with(Instant::now);
        }
        self.news.notify_all();
    }

    /// The thread: takes packets in, sends what is due and notices the end.
    fn run(&self) {
        let mut buffer = [0u8; DATAGRAM + 1];
        loop {
            let got = self.socket.recv_from(&mut buffer);
            // Windows reports here that something sent earlier found no one,
            // and does so at once: waiting avoids spinning on it.
            if let Err(e) = &got
                && !matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut)
            {
                std::thread::sleep(TICK);
            }
            let Ok(mut state) = self.state.lock() else {
                return;
            };
            if let Ok((size, _)) = got
                && let Some((session, frame)) = open_datagram(&self.receive_key, &buffer[..size])
                && session == self.session
            {
                state.stream.accept(frame, Instant::now());
            }
            self.flush(&mut state);
            let now = Instant::now();
            let gone = state.stream.peer_closed() || state.stream.silent_for(now) > SILENCE;
            let done = state.closing.is_some_and(|since| {
                let settled = state.stream.delivered()
                    && now.saturating_duration_since(state.wrote.max(since)) > QUIET;
                settled || now.saturating_duration_since(since) > LINGER
            });
            if done && !gone {
                // Said more than once, since nobody will resend it.
                for _ in 0..3 {
                    self.send(&Frame::Close);
                }
            }
            state.over = gone || done;
            let over = state.over;
            drop(state);
            self.news.notify_all();
            if over {
                return;
            }
        }
    }
}

impl Pipe {
    /// Talks to `peer` through `socket`: `send_key` signs what goes out and
    /// only what carries `receive_key` and this `session` is taken in.
    pub fn start(
        socket: UdpSocket,
        peer: SocketAddr,
        session: u64,
        send_key: [u8; 32],
        receive_key: [u8; 32],
    ) -> io::Result<Pipe> {
        socket.set_read_timeout(Some(TICK))?;
        let now = Instant::now();
        let shared = Arc::new(Shared {
            socket,
            peer,
            session,
            send_key,
            receive_key,
            state: Mutex::new(State {
                stream: Stream::new(now),
                read_wait: SILENCE,
                write_wait: SILENCE,
                closing: None,
                wrote: now,
                over: false,
            }),
            news: Condvar::new(),
        });
        let running = shared.clone();
        std::thread::Builder::new()
            .name("net-pipe".into())
            .spawn(move || running.run())?;
        Ok(Pipe {
            _last: Arc::new(CloseOnDrop(shared.clone())),
            shared,
        })
    }

    #[cfg(test)]
    fn socket(&self) -> &UdpSocket {
        &self.shared.socket
    }

    /// Sleeps until there is news, or fails once `deadline` passes.
    fn wait<'a>(
        &self,
        state: MutexGuard<'a, State>,
        deadline: Instant,
    ) -> io::Result<MutexGuard<'a, State>> {
        let left = deadline
            .checked_duration_since(Instant::now())
            .ok_or(ErrorKind::TimedOut)?;
        let (state, _) = self
            .shared
            .news
            .wait_timeout(state, left)
            .map_err(|_| ErrorKind::BrokenPipe)?;
        Ok(state)
    }
}

impl Read for Pipe {
    /// Zero means the connection ended.
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        let mut state = self.shared.lock()?;
        let deadline = Instant::now() + state.read_wait;
        loop {
            if state.closing.is_some() || out.is_empty() {
                return Ok(0);
            }
            let count = state.stream.read(out);
            if count > 0 {
                return Ok(count);
            }
            if state.over || state.stream.peer_closed() {
                return Ok(0);
            }
            state = self.wait(state, deadline)?;
        }
    }
}

impl Write for Pipe {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let mut state = self.shared.lock()?;
        let deadline = Instant::now() + state.write_wait;
        loop {
            if state.over || state.stream.peer_closed() {
                return Err(ErrorKind::BrokenPipe.into());
            }
            let count = state.stream.write(bytes);
            if count > 0 || bytes.is_empty() {
                state.wrote = Instant::now();
                // At once, without waiting for the thread's next look.
                self.shared.flush(&mut state);
                return Ok(count);
            }
            state = self.wait(state, deadline)?;
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl Wire for Pipe {
    fn another(&self) -> io::Result<Pipe> {
        Ok(self.clone())
    }

    fn limits(&self, read: Duration, write: Duration) -> io::Result<()> {
        let mut state = self.shared.lock()?;
        state.read_wait = read;
        state.write_wait = write;
        Ok(())
    }

    fn closer(&self) -> io::Result<Box<dyn Fn() + Send + Sync>> {
        let shared = self.shared.clone();
        Ok(Box::new(move || shared.close()))
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::eventually;
    use super::*;
    use std::net::Ipv4Addr;

    fn socket() -> UdpSocket {
        UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).expect("socket")
    }

    /// Two ends joined on this computer, and the port of the first.
    fn two() -> (Pipe, Pipe, u16) {
        let (a, b) = (socket(), socket());
        let (at_a, at_b) = (
            a.local_addr().expect("address"),
            b.local_addr().expect("address"),
        );
        let first = Pipe::start(a, at_b, 9, [1; 32], [2; 32]).expect("pipe");
        let second = Pipe::start(b, at_a, 9, [2; 32], [1; 32]).expect("pipe");
        (first, second, at_a.port())
    }

    fn send_and_expect(from: &mut Pipe, to: &mut Pipe, message: &[u8]) {
        to.limits(Duration::from_secs(10), Duration::from_secs(10))
            .expect("limits");
        let mut got = vec![0; message.len()];
        std::thread::scope(|scope| {
            scope.spawn(|| from.write_all(message).expect("write"));
            to.read_exact(&mut got).expect("read");
        });
        assert!(got == message, "{} bytes arrived changed", got.len());
    }

    #[test]
    fn bytes_travel_both_ways_through_real_sockets() {
        let (mut a, mut b, _) = two();
        let long: Vec<u8> = (0..700_000u32).map(|i| (i * 7 + i / 251) as u8).collect();
        send_and_expect(&mut a, &mut b, &long);
        send_and_expect(&mut b, &mut a, b"de vuelta");
    }

    #[test]
    fn closing_delivers_what_was_written_and_then_the_other_end_reads_the_end() {
        let (mut a, mut b, _) = two();
        let long = vec![5u8; 20_000];
        a.write_all(&long).expect("write");
        (a.closer().expect("closer"))();
        assert_eq!(a.read(&mut [0; 8]).expect("closed here"), 0);
        b.limits(Duration::from_secs(10), Duration::from_secs(10))
            .expect("limits");
        let mut got = Vec::new();
        b.read_to_end(&mut got).expect("read");
        assert_eq!(got, long);
        assert!(b.write(b"x").is_err(), "there is nobody to write to");
    }

    #[test]
    fn reading_waits_only_so_long() {
        let (mut a, _b, _) = two();
        a.limits(Duration::from_millis(200), Duration::from_secs(1))
            .expect("limits");
        let started = Instant::now();
        let error = a.read(&mut [0; 8]).expect_err("nothing was sent");
        assert_eq!(error.kind(), ErrorKind::TimedOut);
        assert!(started.elapsed() >= Duration::from_millis(190));
    }

    #[test]
    fn writing_to_an_end_that_does_not_answer_gives_up() {
        let deaf = socket();
        let to = deaf.local_addr().expect("address");
        let mut a = Pipe::start(socket(), to, 9, [1; 32], [2; 32]).expect("pipe");
        a.limits(Duration::from_secs(1), Duration::from_millis(300))
            .expect("limits");
        let chunk = [0u8; 4096];
        let error = (0..1000)
            .find_map(|_| a.write_all(&chunk).err())
            .expect("an end to the room for waiting data");
        assert_eq!(error.kind(), ErrorKind::TimedOut);
    }

    #[test]
    fn the_link_survives_a_port_unreachable() {
        let (mut a, mut b, _) = two();
        let nobody = socket().local_addr().expect("address");
        for _ in 0..5 {
            // On Windows this makes the next read of the socket fail.
            a.socket().send_to(b"hola", nobody).expect("send");
            std::thread::sleep(Duration::from_millis(30));
        }
        send_and_expect(&mut b, &mut a, b"seguimos aca");
        send_and_expect(&mut a, &mut b, b"nosotros tambien");
    }

    #[test]
    fn dropping_every_handle_frees_the_port() {
        let (a, _b, port) = two();
        let other = a.another().expect("clone");
        drop(a);
        std::thread::sleep(Duration::from_millis(100));
        assert!(UdpSocket::bind((Ipv4Addr::LOCALHOST, port)).is_err());
        drop(other);
        eventually(|| UdpSocket::bind((Ipv4Addr::LOCALHOST, port)).ok());
    }
}
