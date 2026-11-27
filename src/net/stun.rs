//! Asking a public STUN server which address and port the internet sees one of
//! this computer's sockets at. Only that question is sent; nothing about the
//! game or the player goes with it.

use super::secure;
use std::io::ErrorKind;
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4, ToSocketAddrs, UdpSocket};
use std::time::{Duration, Instant};

const COOKIE: u32 = 0x2112_A442;
const HEADER: usize = 20;
const BINDING_REQUEST: u16 = 0x0001;
const BINDING_SUCCESS: u16 = 0x0101;
const MAPPED_ADDRESS: u16 = 0x0001;
const XOR_MAPPED_ADDRESS: u16 = 0x0020;
const WAIT: Duration = Duration::from_millis(1500);

/// Public servers that answer such questions, tried in order.
pub const SERVERS: [&str; 4] = [
    "stun.l.google.com:19302",
    "stun1.l.google.com:19302",
    "stun.cloudflare.com:3478",
    "stun.services.mozilla.com:3478",
];

fn request(id: &[u8; 12]) -> [u8; HEADER] {
    let mut out = [0u8; HEADER];
    out[..2].copy_from_slice(&BINDING_REQUEST.to_be_bytes());
    // The length of what follows the header: nothing.
    out[4..8].copy_from_slice(&COOKIE.to_be_bytes());
    out[8..].copy_from_slice(id);
    out
}

/// The address in a server's answer, if it is an answer to this question.
fn read_answer(bytes: &[u8], id: &[u8; 12]) -> Option<SocketAddrV4> {
    if bytes.len() < HEADER
        || u16::from_be_bytes([bytes[0], bytes[1]]) != BINDING_SUCCESS
        || bytes[4..8] != COOKIE.to_be_bytes()
        || bytes[8..HEADER] != id[..]
    {
        return None;
    }
    let length = usize::from(u16::from_be_bytes([bytes[2], bytes[3]]));
    let body = bytes.get(HEADER..HEADER.checked_add(length)?)?;
    let mut plain = None;
    let mut at = 0;
    while let Some(head) = body.get(at..at + 4) {
        let kind = u16::from_be_bytes([head[0], head[1]]);
        let size = usize::from(u16::from_be_bytes([head[2], head[3]]));
        let value = body.get(at + 4..at + 4 + size)?;
        // Family 1 is IPv4: reserved byte, family, port, address.
        if size >= 8 && value[1] == 1 {
            let raw = [value[4], value[5], value[6], value[7]];
            let port = u16::from_be_bytes([value[2], value[3]]);
            match kind {
                XOR_MAPPED_ADDRESS => {
                    let ip = Ipv4Addr::from(u32::from_be_bytes(raw) ^ COOKIE);
                    return Some(SocketAddrV4::new(ip, port ^ (COOKIE >> 16) as u16));
                }
                MAPPED_ADDRESS => plain = Some(SocketAddrV4::new(Ipv4Addr::from(raw), port)),
                _ => {}
            }
        }
        // Attributes are padded to four bytes.
        at += 4 + size.div_ceil(4) * 4;
    }
    plain
}

/// Asks one server through `socket`.
fn ask(socket: &UdpSocket, server: SocketAddr, wait: Duration) -> Option<SocketAddrV4> {
    let id: [u8; 12] = secure::random().ok()?;
    socket.send_to(&request(&id), server).ok()?;
    let until = Instant::now() + wait;
    let mut buffer = [0u8; 512];
    // Other datagrams may come first; only the answer to this counts.
    loop {
        let left = until.checked_duration_since(Instant::now())?;
        socket
            .set_read_timeout(Some(left.max(Duration::from_millis(1))))
            .ok()?;
        match socket.recv_from(&mut buffer) {
            Ok((size, _)) => {
                if let Some(found) = read_answer(&buffer[..size], &id) {
                    return Some(found);
                }
            }
            // Windows reports here that something sent earlier found no one.
            Err(e) if e.kind() == ErrorKind::ConnectionReset => {}
            Err(_) => return None,
        }
    }
}

fn resolve<'a>(servers: &'a [&str]) -> impl Iterator<Item = SocketAddr> + 'a {
    servers
        .iter()
        .filter_map(|name| name.to_socket_addrs().ok()?.find(SocketAddr::is_ipv4))
}

/// How the first of `servers` that answers sees `socket`, and which one it was.
pub fn mapped_address(socket: &UdpSocket, servers: &[&str]) -> Option<(SocketAddrV4, SocketAddr)> {
    resolve(servers).find_map(|server| Some((ask(socket, server, WAIT)?, server)))
}

/// Whether two different servers see `socket` at the same port, which is what
/// lets two homes connect directly. `None` if fewer than two answered.
pub fn keeps_port(socket: &UdpSocket, servers: &[&str]) -> Option<bool> {
    let mut first: Option<(SocketAddrV4, SocketAddr)> = None;
    for server in resolve(servers) {
        // Two names of one machine would prove nothing.
        if first.is_some_and(|f| f.1.ip() == server.ip()) {
            continue;
        }
        let Some(seen) = ask(socket, server, WAIT) else {
            continue;
        };
        match first {
            None => first = Some((seen, server)),
            Some((before, _)) => return Some(before == seen),
        }
    }
    None
}

/// Asks without waiting for the answer: it keeps the router remembering
/// `socket` while nothing else is being said.
pub fn poke(socket: &UdpSocket, server: SocketAddr) {
    if let Ok(id) = secure::random::<12>() {
        let _ = socket.send_to(&request(&id), server);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// What a server sends back.
    fn answer(id: &[u8; 12], attributes: &[(u16, Vec<u8>)]) -> Vec<u8> {
        let mut body = Vec::new();
        for (kind, value) in attributes {
            body.extend(kind.to_be_bytes());
            body.extend((value.len() as u16).to_be_bytes());
            body.extend(value);
            body.resize(body.len().next_multiple_of(4), 0);
        }
        let mut out = Vec::new();
        out.extend(BINDING_SUCCESS.to_be_bytes());
        out.extend((body.len() as u16).to_be_bytes());
        out.extend(COOKIE.to_be_bytes());
        out.extend(id);
        out.extend(body);
        out
    }

    fn xor(ip: Ipv4Addr) -> Vec<u8> {
        let mut value = vec![0, 1, 0x12, 0x34];
        value.extend((u32::from(ip) ^ COOKIE).to_be_bytes());
        value
    }

    fn plain(ip: Ipv4Addr) -> Vec<u8> {
        let mut value = vec![0, 1, 0x12, 0x34];
        value.extend(ip.octets());
        value
    }

    const ID: [u8; 12] = [7; 12];
    const SEEN: Ipv4Addr = Ipv4Addr::new(93, 184, 216, 34);
    /// What the helpers put in the port field.
    const FIELD: u16 = 0x1234;

    #[test]
    fn the_address_is_read_in_either_form() {
        let odd = (0x8022, b"abc".to_vec());
        for attributes in [
            vec![(XOR_MAPPED_ADDRESS, xor(SEEN))],
            vec![(MAPPED_ADDRESS, plain(SEEN))],
            // The padded unknown attribute in front is skipped.
            vec![odd.clone(), (XOR_MAPPED_ADDRESS, xor(SEEN))],
        ] {
            let found = read_answer(&answer(&ID, &attributes), &ID).expect("address");
            assert_eq!(*found.ip(), SEEN);
        }
    }

    #[test]
    fn stun_reports_the_port_too() {
        let hidden = answer(&ID, &[(XOR_MAPPED_ADDRESS, xor(SEEN))]);
        let found = read_answer(&hidden, &ID).expect("address");
        assert_eq!(found.port(), FIELD ^ 0x2112);
        let bare = answer(&ID, &[(MAPPED_ADDRESS, plain(SEEN))]);
        assert_eq!(read_answer(&bare, &ID).expect("address").port(), FIELD);
    }

    #[test]
    fn answers_that_are_not_for_us_or_are_broken_say_nothing() {
        let good = answer(&ID, &[(XOR_MAPPED_ADDRESS, xor(SEEN))]);
        assert_eq!(read_answer(&good, &[8; 12]), None, "another question");
        for cut in 0..good.len() {
            assert_eq!(read_answer(&good[..cut], &ID), None, "cut at {cut}");
        }
        let mut lying = good.clone();
        lying[2] = 0xFF;
        assert_eq!(read_answer(&lying, &ID), None, "a length beyond the data");
        let mut wrong_cookie = good.clone();
        wrong_cookie[4] ^= 1;
        assert_eq!(read_answer(&wrong_cookie, &ID), None);
        let mut failure = good;
        failure[0] = 0x01;
        failure[1] = 0x11;
        assert_eq!(read_answer(&failure, &ID), None, "an error answer");
        assert_eq!(read_answer(&[], &ID), None);
        let huge = answer(&ID, &[(0x8022, vec![1; 3])]);
        assert_eq!(read_answer(&huge[..HEADER + 4], &ID), None);
    }

    #[test]
    fn the_answer_is_read_on_the_socket_that_asked() {
        let server = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).expect("server");
        let at = server.local_addr().expect("address");
        let running = std::thread::spawn(move || {
            let mut buffer = [0u8; 64];
            let (size, from) = server.recv_from(&mut buffer).expect("question");
            assert_eq!(size, HEADER);
            let mut id = [0u8; 12];
            id.copy_from_slice(&buffer[8..HEADER]);
            // A stray datagram first, then the real answer.
            server.send_to(b"noise", from).expect("noise");
            let reply = answer(&id, &[(XOR_MAPPED_ADDRESS, xor(SEEN))]);
            server.send_to(&reply, from).expect("answer");
        });
        let socket = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).expect("asking");
        let found = ask(&socket, at, Duration::from_secs(5)).expect("answer");
        assert_eq!(*found.ip(), SEEN);
        running.join().expect("server");

        let silent = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).expect("silent");
        let at = silent.local_addr().expect("address");
        assert_eq!(ask(&socket, at, Duration::from_millis(150)), None);
    }
}
