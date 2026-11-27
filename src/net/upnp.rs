//! Asking the home router to let the internet reach a port, with UPnP.
//!
//! Most home routers offer it: a program on the network asks, in plain text,
//! for "TCP port N goes to this computer", and the router does it. Nothing
//! outside the home network takes part.
//!
//! Everything a router says is read as if it came from a stranger: only
//! addresses of the router itself are followed, only short answers are read,
//! and what is copied into a later request is checked first.

use std::io::{Read, Write};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, SocketAddrV4, TcpStream, UdpSocket};
use std::sync::mpsc::channel;
use std::time::{Duration, Instant};

pub const MULTICAST: SocketAddr =
    SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(239, 255, 255, 250), 1900));
/// How long routers get to answer a search.
pub const SEARCH_TIME: Duration = Duration::from_millis(2000);
const CONNECT_TIME: Duration = Duration::from_millis(1500);
const REPLY_TIME: Duration = Duration::from_secs(3);
const MAX_PAGE: u64 = 64 * 1024;
/// A forgotten mapping goes away by itself after this many seconds.
const LEASE: u32 = 7200;
const NAME: &str = "Hexagonos de Babel";
const GATEWAYS: [&str; 2] = [
    "urn:schemas-upnp-org:device:InternetGatewayDevice:1",
    "urn:schemas-upnp-org:device:InternetGatewayDevice:2",
];
/// Faults a router answers with, from the UPnP specification.
const ONLY_PERMANENT: u32 = 725;
const PORT_TAKEN: u32 = 718;

/// Where a router's control service is.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Service {
    host: SocketAddrV4,
    path: String,
    kind: String,
}

/// A forwarded port. Dropping it closes the port again.
pub struct Mapping {
    service: Service,
    pub external_port: u16,
    /// The address the router says it has on the internet.
    pub external_ip: Option<Ipv4Addr>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Why {
    /// Nothing answered: no router, or it does not offer this.
    NoRouter,
    Refused(String),
}

/// Letters, digits and a few signs: safe to copy into a request.
fn plain(text: &str) -> bool {
    !text.is_empty()
        && text.len() <= 200
        && text
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || ":-._/".contains(c))
}

/// An `http://address:port/path` that points at the machine that was asked.
fn parse_url(text: &str, only: Option<IpAddr>) -> Option<(SocketAddrV4, String)> {
    let rest = text.trim().strip_prefix("http://")?;
    let (authority, path) = match rest.find('/') {
        Some(at) => (&rest[..at], &rest[at..]),
        None => (rest, "/"),
    };
    let host = authority.parse::<SocketAddrV4>().ok().or_else(|| {
        authority
            .parse::<Ipv4Addr>()
            .ok()
            .map(|ip| SocketAddrV4::new(ip, 80))
    })?;
    let from_the_router = only.is_none_or(|ip| ip == IpAddr::V4(*host.ip()));
    (from_the_router && plain(path)).then(|| (host, path.to_string()))
}

fn between<'a>(text: &'a str, tag: &str) -> Option<&'a str> {
    let open = format!("<{tag}>");
    let start = text.find(&open)? + open.len();
    let end = text[start..].find(&format!("</{tag}>"))? + start;
    Some(text[start..end].trim())
}

fn blocks<'a>(text: &'a str, tag: &str) -> Vec<&'a str> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(found) = between(rest, tag) {
        out.push(found);
        // Past the end of this one.
        let Some(at) = rest.find(&format!("</{tag}>")) else {
            break;
        };
        rest = &rest[at + tag.len() + 3..];
    }
    out
}

/// Routers that answered a search sent to `target`, at most a few.
fn routers(target: SocketAddr, from: Ipv4Addr, wait: Duration) -> Vec<(SocketAddrV4, String)> {
    let Ok(socket) = UdpSocket::bind((from, 0)) else {
        return Vec::new();
    };
    for kind in GATEWAYS {
        let question = format!(
            "M-SEARCH * HTTP/1.1\r\nHOST: 239.255.255.250:1900\r\nMAN: \"ssdp:discover\"\r\nMX: 2\r\nST: {kind}\r\n\r\n"
        );
        let _ = socket.send_to(question.as_bytes(), target);
    }
    let until = Instant::now() + wait;
    let mut found: Vec<(SocketAddrV4, String)> = Vec::new();
    let mut buffer = [0u8; 1500];
    while found.len() < 4 {
        // Once one has answered, the others only get a moment more.
        let end = if found.is_empty() {
            until
        } else {
            until.min(Instant::now() + Duration::from_millis(250))
        };
        let Some(left) = end.checked_duration_since(Instant::now()) else {
            break;
        };
        if socket
            .set_read_timeout(Some(left.max(Duration::from_millis(1))))
            .is_err()
        {
            break;
        }
        let Ok((size, sender)) = socket.recv_from(&mut buffer) else {
            break;
        };
        let text = String::from_utf8_lossy(&buffer[..size]);
        let location = text.lines().find_map(|line| {
            let (name, value) = line.split_once(':')?;
            name.trim()
                .eq_ignore_ascii_case("location")
                .then_some(value)
        });
        if let Some(place) = location.and_then(|l| parse_url(l, Some(sender.ip())))
            && !found.contains(&place)
        {
            found.push(place);
        }
    }
    found
}

fn find(bytes: &[u8], what: &[u8]) -> Option<usize> {
    bytes.windows(what.len()).position(|w| w == what)
}

/// The body of a response sent in pieces.
fn dechunk(mut raw: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    while let Some(end) = find(raw, b"\r\n") {
        let size = std::str::from_utf8(&raw[..end])
            .ok()
            .and_then(|line| usize::from_str_radix(line.split(';').next()?.trim(), 16).ok());
        raw = &raw[end + 2..];
        match size {
            Some(size) if size > 0 => {
                out.extend(&raw[..size.min(raw.len())]);
                raw = raw.get(size + 2..).unwrap_or(&[]);
            }
            _ => break,
        }
    }
    out
}

/// One request and its answer: the status and the text.
fn http(host: SocketAddrV4, request: &str) -> Option<(u16, String)> {
    let mut stream = TcpStream::connect_timeout(&SocketAddr::V4(host), CONNECT_TIME).ok()?;
    stream.set_read_timeout(Some(REPLY_TIME)).ok()?;
    stream.set_write_timeout(Some(REPLY_TIME)).ok()?;
    stream.write_all(request.as_bytes()).ok()?;
    let mut raw = Vec::new();
    // A router that keeps the line open is read up to what it had said.
    let _ = stream.take(MAX_PAGE).read_to_end(&mut raw);
    let split = find(&raw, b"\r\n\r\n")?;
    let head = String::from_utf8_lossy(&raw[..split]).to_ascii_lowercase();
    let status = head.split_whitespace().nth(1)?.parse().ok()?;
    let body = &raw[split + 4..];
    let body = if head.contains("transfer-encoding: chunked") {
        dechunk(body)
    } else {
        body.to_vec()
    };
    Some((status, String::from_utf8_lossy(&body).into_owned()))
}

/// What a router's description says about where to send orders.
fn control_of(host: SocketAddrV4, location: &str, description: &str) -> Option<Service> {
    let mut chosen: Option<(String, String)> = None;
    for block in blocks(description, "service") {
        let (Some(kind), Some(control)) =
            (between(block, "serviceType"), between(block, "controlURL"))
        else {
            continue;
        };
        let wanted = kind.contains("WANIPConnection") || kind.contains("WANPPPConnection");
        // The newer kind is better when there are two.
        let better = chosen.as_ref().is_none_or(|(old, _)| {
            !old.contains("WANIPConnection") && kind.contains("WANIPConnection")
        });
        if wanted && better && plain(kind) {
            chosen = Some((kind.to_string(), control.to_string()));
        }
    }
    let (kind, control) = chosen?;
    let (host, path) = if control.starts_with("http://") {
        // Only the router itself.
        parse_url(&control, Some(IpAddr::V4(*host.ip())))?
    } else if control.starts_with('/') {
        (host, control)
    } else {
        let folder = &location[..=location.rfind('/')?];
        (host, format!("{folder}{control}"))
    };
    plain(&path).then_some(Service { host, path, kind })
}

/// Sends an order. `Err` holds the router's fault code, if it gave one.
fn call(service: &Service, action: &str, args: &[(&str, String)]) -> Result<String, Option<u32>> {
    let inner: String = args
        .iter()
        .map(|(name, value)| format!("<{name}>{value}</{name}>"))
        .collect();
    let kind = &service.kind;
    let body = format!(
        "<?xml version=\"1.0\"?><s:Envelope xmlns:s=\"http://schemas.xmlsoap.org/soap/envelope/\" s:encodingStyle=\"http://schemas.xmlsoap.org/soap/encoding/\"><s:Body><u:{action} xmlns:u=\"{kind}\">{inner}</u:{action}></s:Body></s:Envelope>"
    );
    let request = format!(
        "POST {} HTTP/1.1\r\nHost: {}\r\nContent-Type: text/xml; charset=\"utf-8\"\r\nSOAPAction: \"{kind}#{action}\"\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        service.path,
        service.host,
        body.len()
    );
    match http(service.host, &request) {
        Some((200, text)) => Ok(text),
        Some((_, text)) => Err(between(&text, "errorCode").and_then(|c| c.parse().ok())),
        None => Err(None),
    }
}

/// The address of this computer on the network that leads to `router`.
fn local_toward(router: SocketAddrV4) -> Option<Ipv4Addr> {
    let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)).ok()?;
    socket.connect(router).ok()?;
    match socket.local_addr().ok()?.ip() {
        IpAddr::V4(ip) if !ip.is_unspecified() => Some(ip),
        _ => None,
    }
}

fn forward(service: &Service, port: u16) -> Result<Mapping, String> {
    let client = local_toward(service.host).ok_or("no se encontró la dirección local")?;
    let mut last = "el router no aceptó el pedido".to_string();
    for step in 0..4u16 {
        // The code names whatever port came out, so it need not be `port`.
        let external = if port <= 65_000 {
            port + step
        } else {
            port - step
        };
        for lease in [LEASE, 0] {
            let args = [
                ("NewRemoteHost", String::new()),
                ("NewExternalPort", external.to_string()),
                ("NewProtocol", "TCP".into()),
                ("NewInternalPort", port.to_string()),
                ("NewInternalClient", client.to_string()),
                ("NewEnabled", "1".into()),
                ("NewPortMappingDescription", NAME.into()),
                ("NewLeaseDuration", lease.to_string()),
            ];
            match call(service, "AddPortMapping", &args) {
                Ok(_) => {
                    let external_ip = call(service, "GetExternalIPAddress", &[])
                        .ok()
                        .and_then(|text| between(&text, "NewExternalIPAddress")?.parse().ok());
                    return Ok(Mapping {
                        service: service.clone(),
                        external_port: external,
                        external_ip,
                    });
                }
                // Only permanent mappings: ask for one.
                Err(Some(ONLY_PERMANENT)) if lease != 0 => {}
                // The port is used by something else: try the next.
                Err(Some(PORT_TAKEN)) => break,
                Err(code) => {
                    last = match code {
                        Some(code) => format!("el router respondió con el error {code}"),
                        None => "el router no respondió".into(),
                    };
                    return Err(last);
                }
            }
        }
    }
    Err(last)
}

/// Forwards `port` of this computer to the internet. `target` is where the
/// search goes, `from` this computer's address on the home network.
pub fn open(target: SocketAddr, from: Ipv4Addr, port: u16, wait: Duration) -> Result<Mapping, Why> {
    // Sent from the home network's address first, so that the question does
    // not leave through a virtual adapter.
    let mut found = routers(target, from, wait);
    if found.is_empty() && !from.is_unspecified() {
        found = routers(target, Ipv4Addr::UNSPECIFIED, wait);
    }
    if found.is_empty() {
        return Err(Why::NoRouter);
    }
    let mut last = String::new();
    for (host, location) in found {
        let service = http(
            host,
            &format!("GET {location} HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\n\r\n"),
        )
        .filter(|(status, _)| *status == 200)
        .and_then(|(_, text)| control_of(host, &location, &text));
        let Some(service) = service else {
            last = "no se entendió la descripción del router".into();
            continue;
        };
        match forward(&service, port) {
            Ok(mapping) => return Ok(mapping),
            Err(why) => last = why,
        }
    }
    Err(Why::Refused(last))
}

impl Drop for Mapping {
    fn drop(&mut self) {
        let (service, port) = (self.service.clone(), self.external_port);
        let (done, finished) = channel();
        let started = std::thread::Builder::new()
            .name("upnp-close".into())
            .spawn(move || {
                let args = [
                    ("NewRemoteHost", String::new()),
                    ("NewExternalPort", port.to_string()),
                    ("NewProtocol", "TCP".into()),
                ];
                let _ = call(&service, "DeletePortMapping", &args);
                let _ = done.send(());
            });
        // A router that does not answer must not hold the game up for long.
        if started.is_ok() {
            let _ = finished.recv_timeout(Duration::from_millis(1500));
        }
    }
}

#[cfg(test)]
pub(crate) mod fake {
    //! A router that speaks just enough UPnP.

    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Mutex};

    #[derive(Default)]
    pub struct Router {
        pub external: String,
        pub mappings: Vec<(u16, u16)>,
        /// Ports someone else uses.
        pub taken: Vec<u16>,
        pub permanent_only: bool,
        pub refuse_everything: bool,
        pub seen: Vec<String>,
    }

    pub struct Fake {
        pub search: SocketAddr,
        pub state: Arc<Mutex<Router>>,
        stop: Arc<AtomicBool>,
    }

    impl Drop for Fake {
        fn drop(&mut self) {
            self.stop.store(true, Ordering::Relaxed);
        }
    }

    const DESCRIPTION: &str = "<root><device><deviceList><device><deviceList><device><serviceList>\
        <service><serviceType>urn:schemas-upnp-org:service:WANCommonInterfaceConfig:1</serviceType><controlURL>/ctl/Common</controlURL></service>\
        <service><serviceType>urn:schemas-upnp-org:service:WANIPConnection:1</serviceType><controlURL>ctl/IPConn</controlURL></service>\
        </serviceList></device></deviceList></device></deviceList></device></root>";

    fn fault(code: u32) -> String {
        let text = format!("<UPnPError><errorCode>{code}</errorCode></UPnPError>");
        format!(
            "HTTP/1.1 500 Internal Server Error\r\nContent-Length: {}\r\n\r\n{text}",
            text.len()
        )
    }

    fn ok(text: &str) -> String {
        format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n{text}",
            text.len()
        )
    }

    fn value(body: &str, tag: &str) -> u16 {
        between(body, tag).and_then(|v| v.parse().ok()).unwrap_or(0)
    }

    fn serve(mut stream: TcpStream, state: &Mutex<Router>) {
        let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
        let mut raw = Vec::new();
        let mut chunk = [0u8; 2048];
        let head_end = loop {
            if let Some(at) = find(&raw, b"\r\n\r\n") {
                break at;
            }
            match stream.read(&mut chunk) {
                Ok(0) | Err(_) => return,
                Ok(n) => raw.extend(&chunk[..n]),
            }
        };
        let head = String::from_utf8_lossy(&raw[..head_end]).into_owned();
        let length = head
            .lines()
            .find_map(|l| {
                l.to_ascii_lowercase()
                    .strip_prefix("content-length:")?
                    .trim()
                    .parse()
                    .ok()
            })
            .unwrap_or(0usize);
        while raw.len() < head_end + 4 + length {
            match stream.read(&mut chunk) {
                Ok(0) | Err(_) => return,
                Ok(n) => raw.extend(&chunk[..n]),
            }
        }
        let body = String::from_utf8_lossy(&raw[head_end + 4..]).into_owned();
        let first = head.lines().next().unwrap_or_default().to_string();
        let mut router = state.lock().expect("router");
        let reply = if first.starts_with("GET /desc.xml") {
            // Sent in two pieces, as some routers do.
            let (a, b) = DESCRIPTION.split_at(60);
            format!(
                "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n{:x}\r\n{a}\r\n{:x}\r\n{b}\r\n0\r\n\r\n",
                a.len(),
                b.len()
            )
        } else if first.starts_with("POST /ctl/IPConn") {
            let action = head
                .lines()
                .find_map(|l| l.split_once('#'))
                .map(|(_, a)| a.trim_end_matches('"').to_string())
                .unwrap_or_default();
            router.seen.push(action.clone());
            match action.as_str() {
                _ if router.refuse_everything => fault(501),
                "AddPortMapping" => {
                    let external = value(&body, "NewExternalPort");
                    let internal = value(&body, "NewInternalPort");
                    let lease = value(&body, "NewLeaseDuration");
                    if router.taken.contains(&external) {
                        fault(PORT_TAKEN)
                    } else if router.permanent_only && lease != 0 {
                        fault(ONLY_PERMANENT)
                    } else {
                        router.mappings.push((external, internal));
                        ok("<u:AddPortMappingResponse/>")
                    }
                }
                "GetExternalIPAddress" => ok(&format!(
                    "<NewExternalIPAddress>{}</NewExternalIPAddress>",
                    router.external
                )),
                "DeletePortMapping" => {
                    let external = value(&body, "NewExternalPort");
                    router.mappings.retain(|m| m.0 != external);
                    ok("<u:DeletePortMappingResponse/>")
                }
                _ => fault(401),
            }
        } else {
            "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n".to_string()
        };
        let _ = stream.write_all(reply.as_bytes());
    }

    /// A router on this computer, with the external address it claims.
    pub fn router(external: &str) -> Fake {
        let state = Arc::new(Mutex::new(Router {
            external: external.into(),
            ..Router::default()
        }));
        let stop = Arc::new(AtomicBool::new(false));

        let http = std::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).expect("http");
        http.set_nonblocking(true).expect("nonblocking");
        let web = http.local_addr().expect("address").port();
        let (s, flag) = (state.clone(), stop.clone());
        std::thread::spawn(move || {
            while !flag.load(Ordering::Relaxed) {
                match http.accept() {
                    Ok((stream, _)) => {
                        let _ = stream.set_nonblocking(false);
                        serve(stream, &s);
                    }
                    Err(_) => std::thread::sleep(Duration::from_millis(5)),
                }
            }
        });

        let udp = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).expect("udp");
        udp.set_read_timeout(Some(Duration::from_millis(50)))
            .expect("timeout");
        let search = udp.local_addr().expect("address");
        let flag = stop.clone();
        std::thread::spawn(move || {
            let mut buffer = [0u8; 1500];
            while !flag.load(Ordering::Relaxed) {
                if let Ok((_, from)) = udp.recv_from(&mut buffer) {
                    let answer = format!(
                        "HTTP/1.1 200 OK\r\nCACHE-CONTROL: max-age=120\r\nLocation: http://127.0.0.1:{web}/desc.xml\r\nST: {}\r\n\r\n",
                        GATEWAYS[0]
                    );
                    let _ = udp.send_to(answer.as_bytes(), from);
                }
            }
        });
        Fake {
            search,
            state,
            stop,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::fake::router;
    use super::*;

    const QUICK: Duration = Duration::from_millis(400);
    const LAN: Ipv4Addr = Ipv4Addr::LOCALHOST;

    #[test]
    fn the_router_opens_the_port_and_closes_it_again() {
        let fake = router("93.184.216.34");
        let mapping = open(fake.search, LAN, 47612, QUICK).expect("mapping");
        assert_eq!(mapping.external_port, 47612);
        assert_eq!(mapping.external_ip, Some(Ipv4Addr::new(93, 184, 216, 34)));
        assert_eq!(
            fake.state.lock().expect("router").mappings,
            [(47612, 47612)]
        );
        drop(mapping);
        assert!(fake.state.lock().expect("router").mappings.is_empty());
    }

    #[test]
    fn a_port_in_use_moves_on_and_permanent_only_routers_are_asked_for_that() {
        let fake = router("93.184.216.34");
        {
            let mut router = fake.state.lock().expect("router");
            router.taken = vec![47612, 47613];
            router.permanent_only = true;
        }
        let mapping = open(fake.search, LAN, 47612, QUICK).expect("mapping");
        assert_eq!(mapping.external_port, 47614);
        // Forwarded to where the game listens, whatever the outside number.
        assert_eq!(
            fake.state.lock().expect("router").mappings,
            [(47614, 47612)]
        );
    }

    #[test]
    fn no_router_and_a_refusing_router_are_told_apart() {
        let silent = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).expect("silent");
        let at = silent.local_addr().expect("address");
        assert_eq!(open(at, LAN, 47612, QUICK).err(), Some(Why::NoRouter));

        let fake = router("93.184.216.34");
        fake.state.lock().expect("router").refuse_everything = true;
        assert!(matches!(
            open(fake.search, LAN, 47612, QUICK),
            Err(Why::Refused(text)) if text.contains("501")
        ));
    }

    #[test]
    fn only_the_router_itself_is_ever_followed() {
        let router = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1));
        assert!(parse_url("http://192.168.1.1:5000/desc.xml", Some(router)).is_some());
        assert_eq!(
            parse_url("http://192.168.1.1/x", Some(router)).map(|u| u.0.port()),
            Some(80)
        );
        for hostile in [
            "http://203.0.113.9:80/desc.xml",
            "https://192.168.1.1/desc.xml",
            "http://192.168.1.1:80/a b",
            "http://192.168.1.1:80/a\r\nHost: x",
            "http://router.local/desc.xml",
            "",
        ] {
            assert!(parse_url(hostile, Some(router)).is_none(), "{hostile}");
        }
        let host: SocketAddrV4 = "192.168.1.1:5000".parse().expect("host");
        let page = |control: &str| {
            format!(
                "<service><serviceType>urn:schemas-upnp-org:service:WANIPConnection:1</serviceType><controlURL>{control}</controlURL></service>"
            )
        };
        let at = |control: &str| control_of(host, "/rootDesc.xml", &page(control));
        assert_eq!(at("/ctl").map(|s| s.path), Some("/ctl".into()));
        assert_eq!(at("ctl").map(|s| s.path), Some("/ctl".into()));
        assert_eq!(
            at("http://192.168.1.1:5001/ctl").map(|s| s.host.port()),
            Some(5001)
        );
        assert!(at("http://203.0.113.9/ctl").is_none(), "another machine");
        assert!(at("/ctl\r\nX: y").is_none(), "no line breaks");
    }

    #[test]
    fn chunked_answers_are_put_back_together() {
        assert_eq!(dechunk(b"3\r\nabc\r\n2\r\nde\r\n0\r\n\r\n"), b"abcde");
        assert_eq!(dechunk(b"zz\r\nabc"), b"");
        // A size that promises more than there is.
        assert_eq!(dechunk(b"ff\r\nabc"), b"abc");
        assert_eq!(dechunk(b""), b"");
    }
}
