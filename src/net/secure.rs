//! Proving that both ends know the room secret without ever sending it, and
//! signing every message afterwards.
//!
//! BLAKE3 in keyed mode is used both as the proof and as the message code.
//! Nothing is encrypted: someone listening on the network could watch a game,
//! but cannot join it, nor alter, replay or reorder what is said.

use super::code::SECRET_BYTES;
use super::stream::{Frame, SEGMENT};
use std::io::{self, Read, Write};
use std::net::SocketAddrV4;

/// Makes every guess at the secret cost this much work, so that a recorded
/// greeting cannot be searched for it.
const STRETCH: u32 = 200_000;
const TAG: usize = 16;
pub const NONCE: usize = 32;
/// The largest datagram of the direct way: one segment and its wrapping.
pub const DATAGRAM: usize = SEGMENT + 64;

/// Unpredictable bytes from the operating system.
pub fn random<const N: usize>() -> io::Result<[u8; N]> {
    let mut out = [0u8; N];
    #[cfg(windows)]
    {
        #[link(name = "bcrypt")]
        unsafe extern "system" {
            fn BCryptGenRandom(algorithm: isize, buffer: *mut u8, size: u32, flags: u32) -> i32;
        }
        const SYSTEM_PREFERRED: u32 = 2;
        // SAFETY: the buffer is writable for exactly the length given.
        let status = unsafe { BCryptGenRandom(0, out.as_mut_ptr(), N as u32, SYSTEM_PREFERRED) };
        if status != 0 {
            return Err(io::Error::other("el sistema no entregó números aleatorios"));
        }
    }
    #[cfg(not(windows))]
    std::fs::File::open("/dev/urandom")?.read_exact(&mut out)?;
    Ok(out)
}

/// A key made from `parts`, slowly on purpose.
fn stretched(purpose: &str, parts: &[&[u8]]) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new_derive_key(purpose);
    for part in parts {
        hasher.update(part);
    }
    let mut base = *hasher.finalize().as_bytes();
    for round in 0..STRETCH {
        base = *blake3::keyed_hash(&base, &round.to_le_bytes()).as_bytes();
    }
    base
}

/// The keys of the direct way, which signs every datagram before any greeting
/// has happened. Whoever records one still has to pay the same price for each
/// guess at the secret.
pub struct PunchKeys {
    pub from_host: [u8; 32],
    pub from_guest: [u8; 32],
    mark: [u8; 32],
}

impl PunchKeys {
    pub fn derive(secret: &[u8; SECRET_BYTES]) -> PunchKeys {
        let base = stretched("hexagonos-de-babel direct 1", &[secret]);
        let purpose = |name: &str| *blake3::keyed_hash(&base, name.as_bytes()).as_bytes();
        PunchKeys {
            from_host: purpose("datagrams from the host"),
            from_guest: purpose("datagrams from the guest"),
            mark: purpose("reply codes"),
        }
    }

    /// What ties a reply code to this room.
    pub fn mark_of(&self, address: SocketAddrV4) -> [u8; 2] {
        let mut bytes = [0u8; 6];
        bytes[..4].copy_from_slice(&address.ip().octets());
        bytes[4..].copy_from_slice(&address.port().to_be_bytes());
        let hash = blake3::keyed_hash(&self.mark, &bytes);
        [hash.as_bytes()[0], hash.as_bytes()[1]]
    }
}

fn datagram_tag(key: &[u8; 32], body: &[u8]) -> [u8; TAG] {
    let mut out = [0; TAG];
    out.copy_from_slice(&blake3::keyed_hash(key, body).as_bytes()[..TAG]);
    out
}

/// One packet of the direct way: the attempt it belongs to, what it says and
/// the signature.
pub fn seal_datagram(key: &[u8; 32], session: u64, frame: &Frame) -> Vec<u8> {
    let mut out = session.to_le_bytes().to_vec();
    // A frame always has a postcard form.
    out.extend(postcard::to_stdvec(frame).unwrap_or_default());
    let tag = datagram_tag(key, &out);
    out.extend(tag);
    out
}

/// Reads a packet, if it is whole and signed with this key.
pub fn open_datagram(key: &[u8; 32], bytes: &[u8]) -> Option<(u64, Frame)> {
    if bytes.len() > DATAGRAM {
        return None;
    }
    let (body, tag) = bytes.split_at_checked(bytes.len().checked_sub(TAG)?)?;
    if !same(&datagram_tag(key, body), tag) {
        return None;
    }
    let (session, rest) = body.split_at_checked(8)?;
    let (frame, left) = postcard::take_from_bytes(rest).ok()?;
    let session = u64::from_le_bytes(session.try_into().ok()?);
    left.is_empty().then_some((session, frame))
}

/// Compared in full whatever differs, so that timing tells nothing.
fn same(expected: &[u8], given: &[u8]) -> bool {
    expected.len() == given.len()
        && expected
            .iter()
            .zip(given)
            .fold(0u8, |sum, (a, b)| sum | (a ^ b))
            == 0
}

/// Everything derived from the secret for one connection.
pub struct Keys {
    pub guest_proof: blake3::Hash,
    pub host_proof: blake3::Hash,
    pub to_guest: [u8; 32],
    pub to_host: [u8; 32],
}

impl Keys {
    pub fn derive(
        secret: &[u8; SECRET_BYTES],
        host_nonce: &[u8; NONCE],
        guest_nonce: &[u8; NONCE],
    ) -> Keys {
        let base = stretched(
            "hexagonos-de-babel room 1",
            &[secret, host_nonce, guest_nonce],
        );
        let purpose = |name: &str| blake3::keyed_hash(&base, name.as_bytes());
        Keys {
            guest_proof: purpose("guest proof"),
            host_proof: purpose("host proof"),
            to_guest: *purpose("messages to the guest").as_bytes(),
            to_host: *purpose("messages to the host").as_bytes(),
        }
    }
}

fn bad(what: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, what.to_string())
}

/// Reads one `u32` length and that many bytes, refusing anything longer than
/// `limit` before making room for it.
pub fn read_block(from: &mut impl Read, limit: usize) -> io::Result<Vec<u8>> {
    let mut length = [0u8; 4];
    from.read_exact(&mut length)?;
    let length = u32::from_le_bytes(length) as usize;
    if length > limit {
        return Err(bad("mensaje demasiado grande"));
    }
    let mut out = vec![0; length];
    from.read_exact(&mut out)?;
    Ok(out)
}

pub fn write_block(to: &mut impl Write, bytes: &[u8]) -> io::Result<()> {
    let length = u32::try_from(bytes.len()).map_err(|_| bad("mensaje demasiado grande"))?;
    let mut out = Vec::with_capacity(4 + bytes.len());
    out.extend(length.to_le_bytes());
    out.extend(bytes);
    to.write_all(&out)
}

/// One direction of the conversation: its key and how many messages went by.
pub struct Seal {
    key: [u8; 32],
    count: u64,
}

impl Seal {
    pub fn new(key: [u8; 32]) -> Seal {
        Seal { key, count: 0 }
    }

    fn tag(&self, payload: &[u8]) -> [u8; TAG] {
        let mut hasher = blake3::Hasher::new_keyed(&self.key);
        hasher.update(&self.count.to_le_bytes());
        hasher.update(payload);
        let mut out = [0; TAG];
        out.copy_from_slice(&hasher.finalize().as_bytes()[..TAG]);
        out
    }

    /// Appends the signature of this message in its place in the sequence.
    pub fn seal(&mut self, mut payload: Vec<u8>) -> Vec<u8> {
        let tag = self.tag(&payload);
        payload.extend(tag);
        self.count += 1;
        payload
    }

    /// Checks and removes the signature.
    pub fn open(&mut self, mut message: Vec<u8>) -> io::Result<Vec<u8>> {
        let at = message
            .len()
            .checked_sub(TAG)
            .ok_or_else(|| bad("mensaje incompleto"))?;
        let expected = self.tag(&message[..at]);
        if !same(&expected, &message[at..]) {
            return Err(bad("firma incorrecta"));
        }
        message.truncate(at);
        self.count += 1;
        Ok(message)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn random_bytes_differ_every_time() {
        let a: [u8; 32] = random().expect("random");
        let b: [u8; 32] = random().expect("random");
        assert_ne!(a, b);
        assert_ne!(a, [0; 32]);
    }

    #[test]
    fn keys_depend_on_the_secret_and_on_both_nonces() {
        let key = |s: u8, h: u8, g: u8| Keys::derive(&[s; 6], &[h; 32], &[g; 32]).to_guest;
        let base = key(1, 2, 3);
        assert_eq!(base, key(1, 2, 3));
        assert!(
            [key(9, 2, 3), key(1, 9, 3), key(1, 2, 9)]
                .iter()
                .all(|k| *k != base)
        );
        let keys = Keys::derive(&[1; 6], &[2; 32], &[3; 32]);
        assert_ne!(keys.guest_proof, keys.host_proof);
        assert_ne!(keys.to_guest, keys.to_host);
    }

    #[test]
    fn altered_replayed_and_reordered_messages_are_refused() {
        let key = [7; 32];
        let (mut sender, mut receiver) = (Seal::new(key), Seal::new(key));
        let first = sender.seal(b"uno".to_vec());
        let second = sender.seal(b"dos".to_vec());
        assert!(Seal::new(key).open(second.clone()).is_err(), "out of order");
        let mut altered = first.clone();
        altered[0] ^= 1;
        assert!(Seal::new(key).open(altered).is_err());
        assert_eq!(receiver.open(first.clone()).expect("first"), b"uno");
        assert!(receiver.open(first).is_err(), "replayed");
        assert_eq!(receiver.open(second).expect("second"), b"dos");
        assert!(Seal::new([8; 32]).open(sender.seal(b"x".to_vec())).is_err());
        assert!(Seal::new(key).open(vec![0; 3]).is_err(), "too short");
    }

    #[test]
    fn punch_keys_differ_by_direction_and_secret() {
        let a = PunchKeys::derive(&[1; SECRET_BYTES]);
        let b = PunchKeys::derive(&[2; SECRET_BYTES]);
        assert_ne!(a.from_host, a.from_guest);
        assert_ne!(a.from_host, b.from_host);
        assert_ne!(a.from_guest, b.from_guest);
        let here: SocketAddrV4 = "93.184.216.34:5000".parse().expect("address");
        let there: SocketAddrV4 = "93.184.216.34:5001".parse().expect("address");
        assert_eq!(a.mark_of(here), a.mark_of(here));
        assert_ne!(a.mark_of(here), a.mark_of(there));
        assert_ne!(a.mark_of(here), b.mark_of(here));
    }

    #[test]
    fn datagrams_with_another_key_or_altered_are_refused() {
        let frame = Frame::Data {
            seq: 7,
            bytes: vec![9; SEGMENT],
        };
        let sealed = seal_datagram(&[3; 32], 42, &frame);
        assert!(sealed.len() <= DATAGRAM, "{} bytes", sealed.len());
        assert_eq!(open_datagram(&[3; 32], &sealed), Some((42, frame)));
        assert_eq!(open_datagram(&[4; 32], &sealed), None);
        let short = seal_datagram(&[3; 32], 42, &Frame::Punch { seen: false });
        for at in 0..short.len() {
            let mut altered = short.clone();
            altered[at] ^= 1;
            assert_eq!(open_datagram(&[3; 32], &altered), None, "byte {at}");
            assert_eq!(open_datagram(&[3; 32], &short[..at]), None, "cut at {at}");
        }
        assert_eq!(open_datagram(&[3; 32], &[0; 4000]), None);
        // Signed, but with more than a packet says.
        let mut padded = short[..short.len() - TAG].to_vec();
        padded.push(0);
        let tag = datagram_tag(&[3; 32], &padded);
        padded.extend(tag);
        assert_eq!(open_datagram(&[3; 32], &padded), None);
    }

    #[test]
    fn a_huge_length_is_refused_before_reading() {
        let mut bytes: &[u8] = &u32::MAX.to_le_bytes();
        assert!(read_block(&mut bytes, 1024).is_err());
        let mut out = Vec::new();
        write_block(&mut out, b"hola").expect("write");
        assert_eq!(read_block(&mut out.as_slice(), 4).expect("read"), b"hola");
        assert!(read_block(&mut out.as_slice(), 3).is_err());
    }
}
