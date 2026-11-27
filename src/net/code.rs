//! The two codes that two players pass each other, in characters that are easy
//! to read out loud: the room code (where the host is and the secret that lets
//! a friend in) and, when the front door is closed, the guest's reply code
//! (where the guest is).

use super::secure::PunchKeys;
use std::net::{Ipv4Addr, SocketAddrV4};

/// Crockford's alphabet: no I, L, O or U to confuse with digits.
const ALPHABET: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";
pub const SECRET_BYTES: usize = 6;
/// Characters and group size of each kind. The lengths tell them apart.
const ROOM: (usize, usize) = (24, 6);
const REPLY: (usize, usize) = (15, 5);

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct RoomCode {
    /// Where the room listens for a direct TCP connection.
    pub address: SocketAddrV4,
    /// The port for the direct way through both routers, at the same address.
    /// Zero when there is none.
    pub udp: u16,
    pub secret: [u8; SECRET_BYTES],
}

/// Never shows the secret, so that a log or a panic cannot leak it.
impl std::fmt::Debug for RoomCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "RoomCode({}, udp {}, secret hidden)",
            self.address, self.udp
        )
    }
}

/// Detects typing mistakes. It protects nothing.
fn checksum(payload: &[u8], bits: usize) -> u128 {
    let hash = blake3::hash(payload);
    let two = u16::from_be_bytes([hash.as_bytes()[0], hash.as_bytes()[1]]);
    u128::from(two) & ((1 << bits) - 1)
}

/// The payload and its checksum as grouped characters.
fn write(payload: &[u8], (length, group): (usize, usize)) -> String {
    let spare = length * 5 - payload.len() * 8;
    let mut bits: u128 = payload.iter().fold(0, |bits, &b| bits << 8 | u128::from(b));
    bits = bits << spare | checksum(payload, spare);
    let mut out = String::with_capacity(length + length / group);
    for i in 0..length {
        if i > 0 && i % group == 0 {
            out.push('-');
        }
        let five = (bits >> (5 * (length - 1 - i))) & 31;
        out.push(ALPHABET[five as usize] as char);
    }
    out
}

/// Reads a typed code of `N` bytes. Case, spaces and dashes do not matter, and
/// letters that look like digits are read as those digits.
fn read<const N: usize>(text: &str, (length, _): (usize, usize)) -> Option<[u8; N]> {
    let mut bits: u128 = 0;
    let mut count = 0;
    for c in text.chars() {
        if c == '-' || c.is_whitespace() {
            continue;
        }
        let c = match c.to_ascii_uppercase() {
            'I' | 'L' => '1',
            'O' => '0',
            other => other,
        };
        let value = ALPHABET.iter().position(|&a| a as char == c)?;
        count += 1;
        if count > length {
            return None;
        }
        bits = bits << 5 | value as u128;
    }
    if count != length {
        return None;
    }
    let spare = length * 5 - N * 8;
    let body = bits >> spare;
    let mut payload = [0u8; N];
    for (i, byte) in payload.iter_mut().enumerate() {
        *byte = (body >> (8 * (N - 1 - i))) as u8;
    }
    (checksum(&payload, spare) == bits & ((1 << spare) - 1)).then_some(payload)
}

fn address_of(bytes: &[u8]) -> SocketAddrV4 {
    let ip = Ipv4Addr::new(bytes[0], bytes[1], bytes[2], bytes[3]);
    SocketAddrV4::new(ip, u16::from_be_bytes([bytes[4], bytes[5]]))
}

impl RoomCode {
    /// `XXXXXX-XXXXXX-XXXXXX-XXXXXX`.
    pub fn text(&self) -> String {
        let mut payload = [0u8; 14];
        payload[..4].copy_from_slice(&self.address.ip().octets());
        payload[4..6].copy_from_slice(&self.address.port().to_be_bytes());
        payload[6..8].copy_from_slice(&self.udp.to_be_bytes());
        payload[8..].copy_from_slice(&self.secret);
        write(&payload, ROOM)
    }

    pub fn parse(text: &str) -> Option<RoomCode> {
        let payload: [u8; 14] = read(text, ROOM)?;
        let mut secret = [0; SECRET_BYTES];
        secret.copy_from_slice(&payload[8..]);
        Some(RoomCode {
            address: address_of(&payload),
            udp: u16::from_be_bytes([payload[6], payload[7]]),
            secret,
        })
    }

    /// What the player typed: a code, optionally followed by `@address` to
    /// reach the host through another network, such as a virtual one.
    pub fn parse_invitation(text: &str) -> Option<RoomCode> {
        let (code, other) = match text.split_once('@') {
            Some((code, other)) => (code, Some(other.trim())),
            None => (text, None),
        };
        let mut room = RoomCode::parse(code)?;
        if let Some(other) = other {
            room.address.set_ip(other.parse().ok()?);
        }
        Some(room)
    }
}

/// The guest's answer to a room code. It carries a mark that only someone who
/// knows that room's secret could have made.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReplyCode {
    address: SocketAddrV4,
    mark: [u8; 2],
}

impl ReplyCode {
    pub fn new(address: SocketAddrV4, keys: &PunchKeys) -> ReplyCode {
        ReplyCode {
            address,
            mark: keys.mark_of(address),
        }
    }

    /// `XXXXX-XXXXX-XXXXX`.
    pub fn text(&self) -> String {
        let mut payload = [0u8; 8];
        payload[..4].copy_from_slice(&self.address.ip().octets());
        payload[4..6].copy_from_slice(&self.address.port().to_be_bytes());
        payload[6..].copy_from_slice(&self.mark);
        write(&payload, REPLY)
    }

    pub fn parse(text: &str) -> Option<ReplyCode> {
        let payload: [u8; 8] = read(text, REPLY)?;
        Some(ReplyCode {
            address: address_of(&payload),
            mark: [payload[6], payload[7]],
        })
    }

    pub fn address(&self) -> SocketAddrV4 {
        self.address
    }

    pub fn belongs_to(&self, keys: &PunchKeys) -> bool {
        self.mark == keys.mark_of(self.address)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> RoomCode {
        RoomCode {
            address: "192.168.1.37:47612".parse().expect("address"),
            udp: 50311,
            secret: [9, 200, 0, 255, 17, 4],
        }
    }

    fn sample_reply() -> ReplyCode {
        ReplyCode::new(
            "93.184.216.34:61000".parse().expect("address"),
            &PunchKeys::derive(&sample().secret),
        )
    }

    /// Every way someone might type or paste it.
    fn typed(text: &str) -> [String; 5] {
        [
            text.to_string(),
            text.to_lowercase(),
            text.replace('-', ""),
            format!("  {}\r\n", text.replace('-', " ")),
            text.replace('1', "l").replace('0', "o"),
        ]
    }

    #[test]
    fn codes_of_both_kinds_read_back_however_they_are_typed() {
        let room = sample().text();
        assert_eq!(room.len(), 27);
        for text in typed(&room) {
            assert!(RoomCode::parse(&text) == Some(sample()), "{text}");
        }
        let reply = sample_reply().text();
        assert_eq!(reply.len(), 17);
        for text in typed(&reply) {
            assert_eq!(ReplyCode::parse(&text), Some(sample_reply()), "{text}");
        }
        let no_direct = RoomCode { udp: 0, ..sample() };
        assert!(RoomCode::parse(&no_direct.text()) == Some(no_direct));
    }

    #[test]
    fn a_room_code_is_not_a_reply_and_a_reply_is_not_a_room() {
        assert_eq!(ReplyCode::parse(&sample().text()), None);
        assert!(RoomCode::parse(&sample_reply().text()).is_none());
    }

    #[test]
    fn a_reply_from_another_room_is_told_apart() {
        let reply = ReplyCode::parse(&sample_reply().text()).expect("reply");
        assert!(reply.belongs_to(&PunchKeys::derive(&sample().secret)));
        assert!(!reply.belongs_to(&PunchKeys::derive(&[1; SECRET_BYTES])));
        assert_eq!(reply.address().to_string(), "93.184.216.34:61000");
    }

    #[test]
    fn most_typing_mistakes_are_noticed() {
        type Reads = fn(&str) -> bool;
        let kinds: [(String, Reads); 2] = [
            (sample().text(), |t| RoomCode::parse(t).is_some()),
            (sample_reply().text(), |t| ReplyCode::parse(t).is_some()),
        ];
        for (text, reads) in kinds {
            let (mut noticed, mut tried) = (0, 0);
            for (i, original) in text.char_indices().filter(|c| c.1 != '-') {
                for &replacement in ALPHABET {
                    if replacement as char == original {
                        continue;
                    }
                    let mut wrong = text.clone().into_bytes();
                    wrong[i] = replacement;
                    let wrong = String::from_utf8(wrong).expect("ascii");
                    tried += 1;
                    noticed += usize::from(!reads(&wrong));
                }
            }
            assert!(noticed * 10 >= tried * 9, "{noticed} of {tried}");
        }
    }

    #[test]
    fn nonsense_is_not_a_code() {
        for text in [
            "",
            "hola",
            "ABCDE-ABCDE",
            &"A".repeat(500),
            "ñ".repeat(20).as_str(),
        ] {
            assert!(RoomCode::parse(text).is_none(), "{text}");
            assert!(ReplyCode::parse(text).is_none(), "{text}");
        }
    }

    #[test]
    fn an_address_after_the_code_replaces_the_one_inside() {
        let typed = format!("{}@100.64.0.5", sample().text());
        let room = RoomCode::parse_invitation(&typed).expect("invitation");
        assert_eq!(room.address.to_string(), "100.64.0.5:47612");
        assert!(room.secret == sample().secret);
        assert!(RoomCode::parse_invitation(&format!("{}@casa", sample().text())).is_none());
    }
}
