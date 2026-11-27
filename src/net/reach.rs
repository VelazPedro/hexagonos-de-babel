//! Making a room reachable from another network, and being honest about when
//! that cannot be done from here.
//!
//! Two things are tried at the same time: asking the router to open the port
//! (UPnP) and asking a public server how the internet sees the socket of the
//! direct way (STUN). If the router opens the port, a friend walks in; if not,
//! the two computers can still meet by knocking from both sides.

use super::code::RoomCode;
use super::stun;
use super::upnp::{self, Mapping};
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4, UdpSocket};
use std::time::Duration;

/// How a room can be reached from outside the home network.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reach {
    /// Not looked at yet.
    Searching,
    /// The router opened the port by itself: the code works from anywhere.
    Open(RoomCode),
    /// The router did not. The guest may need to send a reply code back.
    Direct(RoomCode),
    /// No internet was found.
    Offline,
}

/// What looking around found out. The mapping, if the router made one, must be
/// kept for as long as the port should stay open.
pub struct Found {
    pub reach: Reach,
    pub mapping: Option<Mapping>,
    /// The server that answered, to keep asking so the router remembers.
    pub keepalive: Option<SocketAddr>,
}

/// Where the questions go: the real internet, or stand-ins for a test.
#[derive(Clone)]
pub struct Probe {
    pub search: SocketAddr,
    pub wait: Duration,
    pub stun: Vec<String>,
    /// This computer's address on the home network.
    pub lan: Ipv4Addr,
    /// No network at all: both players are on this computer and the front door
    /// is closed on purpose, to try the direct way.
    pub loopback: bool,
}

impl Probe {
    pub fn real(lan: Ipv4Addr) -> Probe {
        Probe {
            search: upnp::MULTICAST,
            wait: upnp::SEARCH_TIME,
            stun: stun::SERVERS.iter().map(ToString::to_string).collect(),
            lan,
            loopback: false,
        }
    }

    pub fn loopback() -> Probe {
        Probe {
            loopback: true,
            stun: Vec::new(),
            ..Probe::real(Ipv4Addr::LOCALHOST)
        }
    }

    /// How the outside sees `socket`, and who said so.
    pub fn mapped_address(&self, socket: &UdpSocket) -> Option<(SocketAddrV4, Option<SocketAddr>)> {
        if self.loopback {
            let port = socket.local_addr().ok()?.port();
            return Some((SocketAddrV4::new(Ipv4Addr::LOCALHOST, port), None));
        }
        let names: Vec<&str> = self.stun.iter().map(String::as_str).collect();
        stun::mapped_address(socket, &names).map(|(seen, server)| (seen, Some(server)))
    }
}

/// An address somebody else on the internet could send to.
pub fn is_public(ip: Ipv4Addr) -> bool {
    let [a, b, c, _] = ip.octets();
    !(ip.is_private()
        || ip.is_loopback()
        || ip.is_link_local()
        || ip.is_unspecified()
        || ip.is_broadcast()
        || ip.is_multicast()
        || ip.is_documentation()
        // Shared address space, used by providers between homes.
        || (a == 100 && (64..128).contains(&b))
        // Reserved for protocols and for benchmarks.
        || (a == 192 && b == 0 && c == 0)
        || (a == 198 && (18..20).contains(&b))
        || a >= 240)
}

fn with(room: &RoomCode, ip: Ipv4Addr, port: u16, udp: u16) -> RoomCode {
    RoomCode {
        address: SocketAddrV4::new(ip, port),
        udp,
        secret: room.secret,
    }
}

/// The port nothing listens on: the closed front door of [`Probe::loopback`].
const CLOSED: u16 = 1;

/// Finds out how `room` can be reached; `socket` is the one of the direct way.
pub fn find(probe: &Probe, room: &RoomCode, socket: &UdpSocket) -> Found {
    let found = |reach, mapping, keepalive| Found {
        reach,
        mapping,
        keepalive,
    };
    if probe.loopback {
        let seen = probe.mapped_address(socket);
        let reach = seen.map_or(Reach::Offline, |(at, _)| {
            Reach::Direct(with(room, *at.ip(), CLOSED, at.port()))
        });
        return found(reach, None, None);
    }
    let port = room.address.port();
    let (mapped, seen) = std::thread::scope(|scope| {
        let asking = scope.spawn(|| probe.mapped_address(socket));
        let mapped = upnp::open(probe.search, probe.lan, port, probe.wait);
        (mapped, asking.join().ok().flatten())
    });
    let keepalive = seen.and_then(|s| s.1);
    let seen = seen.map(|s| s.0);
    let udp = seen.map_or(0, |at| at.port());

    // The router opened the port, and the address it has is the one the
    // internet sees. Anything else means somebody stands in between.
    if let Ok(mapping) = mapped {
        let ip = match (mapping.external_ip, seen) {
            (Some(own), Some(outside)) => (own == *outside.ip() && is_public(own)).then_some(own),
            (Some(own), None) => is_public(own).then_some(own),
            (None, outside) => outside.map(|at| *at.ip()),
        };
        if let Some(ip) = ip {
            let code = with(room, ip, mapping.external_port, udp);
            return found(Reach::Open(code), Some(mapping), keepalive);
        }
    }
    let reach = seen.map_or(Reach::Offline, |at| {
        Reach::Direct(with(room, *at.ip(), port, udp))
    });
    found(reach, None, keepalive)
}

#[cfg(test)]
mod tests {
    use super::super::upnp::fake::router;
    use super::*;
    use std::net::UdpSocket;

    const SEEN: Ipv4Addr = Ipv4Addr::new(93, 184, 216, 34);

    /// The port the stand-in server says it sees.
    const SEEN_PORT: u16 = 0x1234 ^ 0x2112;

    fn room() -> RoomCode {
        RoomCode {
            address: "192.168.1.37:47612".parse().expect("address"),
            udp: 0,
            secret: [1, 2, 3, 4, 5, 6],
        }
    }

    fn look(probe: &Probe) -> Found {
        let socket = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).expect("socket");
        find(probe, &room(), &socket)
    }

    /// A STUN server that always says the same address.
    fn stun_saying(ip: Option<Ipv4Addr>) -> String {
        let socket = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).expect("stun");
        let at = socket.local_addr().expect("address");
        socket
            .set_read_timeout(Some(Duration::from_millis(50)))
            .expect("timeout");
        std::thread::spawn(move || {
            let mut buffer = [0u8; 64];
            // Long enough for the test, short enough to end by itself.
            for _ in 0..200 {
                let Ok((size, from)) = socket.recv_from(&mut buffer) else {
                    continue;
                };
                let Some(ip) = ip.filter(|_| size == 20) else {
                    continue;
                };
                let mut out = Vec::new();
                out.extend([0x01, 0x01, 0, 12]);
                out.extend(0x2112_A442u32.to_be_bytes());
                out.extend(&buffer[8..20]);
                out.extend([0x00, 0x20, 0, 8, 0, 1, 0x12, 0x34]);
                out.extend((u32::from(ip) ^ 0x2112_A442).to_be_bytes());
                let _ = socket.send_to(&out, from);
            }
        });
        at.to_string()
    }

    fn silent_search() -> SocketAddr {
        let socket = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).expect("silent");
        let at = socket.local_addr().expect("address");
        std::mem::forget(socket);
        at
    }

    fn probe(search: SocketAddr, stun: String) -> Probe {
        Probe {
            search,
            wait: Duration::from_millis(300),
            stun: vec![stun],
            lan: Ipv4Addr::LOCALHOST,
            loopback: false,
        }
    }

    #[test]
    fn addresses_of_other_homes_are_told_from_those_of_this_one() {
        for ip in [SEEN, Ipv4Addr::new(8, 8, 4, 4), Ipv4Addr::new(1, 1, 1, 1)] {
            assert!(is_public(ip), "{ip}");
        }
        for ip in [
            [192, 168, 1, 5],
            [10, 0, 0, 1],
            [172, 16, 0, 1],
            [127, 0, 0, 1],
            [169, 254, 1, 1],
            [100, 64, 0, 1],
            [100, 127, 255, 254],
            [0, 0, 0, 0],
            [224, 0, 0, 1],
            [255, 255, 255, 255],
            [203, 0, 113, 7],
        ] {
            assert!(!is_public(Ipv4Addr::from(ip)), "{ip:?}");
        }
        assert!(is_public(Ipv4Addr::new(100, 128, 0, 1)));
    }

    #[test]
    fn a_router_that_opens_the_port_gives_a_code_for_the_internet() {
        let router = router("93.184.216.34");
        let found = look(&probe(router.search, stun_saying(Some(SEEN))));
        let Reach::Open(code) = found.reach else {
            panic!("{:?}", found.reach);
        };
        assert_eq!(code.address.to_string(), "93.184.216.34:47612");
        assert_eq!(code.udp, SEEN_PORT);
        assert_eq!(code.secret, room().secret, "the same secret");
        assert!(found.mapping.is_some());
        drop(found);
        assert!(router.state.lock().expect("router").mappings.is_empty());
    }

    #[test]
    fn a_shared_address_goes_the_direct_way_and_leaves_nothing_open() {
        // The router's own side is a provider address; or there are two routers
        // in a row and the outside sees another address.
        for own in ["100.72.1.9", "93.184.216.99"] {
            let router = router(own);
            let found = look(&probe(router.search, stun_saying(Some(SEEN))));
            let Reach::Direct(code) = found.reach else {
                panic!("{own}: {:?}", found.reach);
            };
            assert_eq!(*code.address.ip(), SEEN);
            assert_eq!(code.udp, SEEN_PORT);
            assert!(found.mapping.is_none());
            assert!(router.state.lock().expect("router").mappings.is_empty());
        }
    }

    #[test]
    fn without_upnp_the_code_carries_the_direct_way() {
        let found = look(&probe(silent_search(), stun_saying(Some(SEEN))));
        assert!(found.mapping.is_none());
        assert!(found.keepalive.is_some());
        let Reach::Direct(code) = found.reach else {
            panic!("{:?}", found.reach);
        };
        // The TCP port stays, in case it was opened by hand.
        assert_eq!(code.address.to_string(), "93.184.216.34:47612");
        assert_eq!(code.udp, SEEN_PORT);

        let found = look(&probe(silent_search(), stun_saying(None)));
        assert_eq!(found.reach, Reach::Offline);
    }

    #[test]
    fn the_loopback_stand_in_closes_the_front_door_and_needs_no_network() {
        let socket = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).expect("socket");
        let port = socket.local_addr().expect("address").port();
        let found = find(&Probe::loopback(), &room(), &socket);
        let Reach::Direct(code) = found.reach else {
            panic!("{:?}", found.reach);
        };
        assert_eq!(code.address.to_string(), "127.0.0.1:1");
        assert_eq!(code.udp, port);
    }
}
