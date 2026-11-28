//! One fixed-peer UDP socket per Build activation. A reply can use only the
//! socket that this activation has already used to send to its admitted peer.
use package_compilation::build_execution::{BuildNetworkPeer, DatagramAccess};
use semantic_vocabulary::{
    BoundaryMachineId, CanonicalStructuralPathSegment as Path, IntegerSign, IntegerType,
    IntegerValue, StructuralCaseId, StructuralFieldId, StructuralTypeId,
};
use std::{
    io,
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, UdpSocket},
    sync::Mutex,
    time::Duration,
};
use terminal_interpreter::{
    TerminalAggregateValue, TerminalEffectRejection, TerminalEffectResult, TerminalScalarValue,
};

pub(crate) struct Datagrams {
    peer: BuildNetworkPeer,
    send_admitted: bool,
    reply: Option<ReplyShape>,
    socket: Mutex<Option<UdpSocket>>,
}

impl Datagrams {
    pub(crate) fn install(
        module: &terminal_psi::TerminalModule,
        calls: impl IntoIterator<Item = (DatagramAccess, BuildNetworkPeer, BoundaryMachineId)>,
    ) -> Result<Option<Self>, String> {
        let mut channel: Option<Self> = None;
        for (access, peer, boundary) in calls {
            let channel = channel.get_or_insert_with(|| Self {
                peer,
                send_admitted: false,
                reply: None,
                socket: Mutex::new(None),
            });
            if channel.peer != peer {
                return Err("one Build activation cannot redirect its admitted UDP peer".into());
            }
            match access {
                DatagramAccess::Send => channel.send_admitted = true,
                DatagramAccess::ReceiveReply => {
                    let result = ReplyShape::from_boundary(module, boundary)?;
                    if channel.reply.as_ref().is_some_and(|old| old != &result) {
                        return Err(
                            "datagram receives disagree about the installed result type".into()
                        );
                    }
                    channel.reply = Some(result);
                }
            }
        }
        Ok(channel)
    }

    pub(super) fn send(&self, bytes: &[u8]) -> Result<bool, TerminalEffectRejection> {
        if !self.send_admitted {
            return Err(TerminalEffectRejection::new(
                "Build send has no admitted network operation",
            ));
        }
        if bytes.len() > usize::from(self.peer.max_datagram_bytes()) {
            return Err(TerminalEffectRejection::new(
                "Build datagram exceeds the admitted peer's byte limit",
            ));
        }
        let mut socket = self
            .socket
            .lock()
            .map_err(|_| TerminalEffectRejection::new("Build UDP socket lock failed"))?;
        if let Some(socket) = socket.as_ref() {
            return Ok(socket.send(bytes).is_ok_and(|len| len == bytes.len()));
        }
        let Ok(opened) = open_peer(self.peer.address()) else {
            return Ok(false);
        };
        let sent = opened.send(bytes).is_ok_and(|len| len == bytes.len());
        if sent {
            *socket = Some(opened);
        }
        Ok(sent)
    }

    pub(super) fn receive(
        &self,
        destination: &mut [u8],
    ) -> Result<TerminalEffectResult, TerminalEffectRejection> {
        let result = self.reply.as_ref().ok_or_else(|| {
            TerminalEffectRejection::new("Build receive has no admitted network operation")
        })?;
        let socket = self
            .socket
            .lock()
            .map_err(|_| TerminalEffectRejection::new("Build UDP socket lock failed"))?;
        let outcome = socket.as_ref().map_or(Reply::Unavailable, |socket| {
            receive_reply(socket, destination, self.peer.max_datagram_bytes())
        });
        Ok(result.finish(outcome))
    }
}

fn open_peer(peer: SocketAddr) -> io::Result<UdpSocket> {
    let local = match peer {
        SocketAddr::V4(_) => IpAddr::V4(Ipv4Addr::UNSPECIFIED),
        SocketAddr::V6(_) => IpAddr::V6(Ipv6Addr::UNSPECIFIED),
    };
    let socket = UdpSocket::bind(SocketAddr::new(local, 0))?;
    socket.set_write_timeout(Some(Duration::from_secs(1)))?;
    socket.set_read_timeout(Some(Duration::from_secs(1)))?;
    socket.connect(peer)?;
    Ok(socket)
}

#[derive(Debug, PartialEq, Eq)]
enum Reply {
    Received(usize),
    Unavailable,
    TooLarge,
}

fn receive_reply(socket: &UdpSocket, destination: &mut [u8], maximum: u16) -> Reply {
    let limit = destination.len().min(usize::from(maximum));
    // A single extra byte distinguishes a full view from a truncated datagram.
    let mut packet = vec![0; limit + 1];
    match socket.recv(&mut packet) {
        Ok(n) if n <= limit => {
            destination[..n].copy_from_slice(&packet[..n]);
            Reply::Received(n)
        }
        Ok(_) => Reply::TooLarge,
        // Winsock reports WSAEMSGSIZE instead of the truncated byte count.
        #[cfg(windows)]
        Err(error) if error.raw_os_error() == Some(10040) => Reply::TooLarge,
        Err(_) => Reply::Unavailable,
    }
}

#[derive(PartialEq, Eq)]
struct ReplyShape {
    ty: StructuralTypeId,
    received: StructuralCaseId,
    count: StructuralFieldId,
    unavailable: StructuralCaseId,
    too_large: StructuralCaseId,
}

impl ReplyShape {
    fn from_boundary(
        module: &terminal_psi::TerminalModule,
        boundary: BoundaryMachineId,
    ) -> Result<Self, String> {
        let Some(terminal_psi::BoundaryMachineResult::Structural(result)) =
            module.boundary_machine(boundary).map(|decl| &decl.result)
        else {
            return Err("datagram receive must return its installed result type".into());
        };
        let decl = module
            .structural_type(result.structural_type)
            .ok_or("missing datagram result declaration")?;
        let terminal_psi::StructuralTypeShape::Sum { cases } = &decl.shape else {
            return Err("datagram receive result must be a sum".into());
        };
        let case = |name: &str| {
            cases
                .iter()
                .find(|case| module.identities.text(case.identity) == Some(name))
                .ok_or_else(|| format!("missing datagram result case {name}"))
        };
        let received = case("Received")?;
        let [count] = received.fields.as_slice() else {
            return Err("datagram result must contain the received byte count".into());
        };
        let unavailable = case("Unavailable")?;
        let too_large = case("TooLarge")?;
        let count_type = semantic_vocabulary::ScalarType::Integer(
            IntegerType::new(IntegerSign::Unsigned, 64).expect("u64"),
        );
        if cases.len() != 3
            || !unavailable.fields.is_empty()
            || !too_large.fields.is_empty()
            || count.field_type != terminal_psi::StructuralFieldType::Scalar(count_type)
            || module.identities.text(count.identity) != Some("count")
        {
            return Err("datagram receive has an incompatible result declaration".into());
        }
        Ok(Self {
            ty: decl.id,
            received: received.id,
            count: count.id,
            unavailable: unavailable.id,
            too_large: too_large.id,
        })
    }

    fn finish(&self, reply: Reply) -> TerminalEffectResult {
        let (case, fields) = match reply {
            Reply::Unavailable => (self.unavailable, vec![]),
            Reply::TooLarge => (self.too_large, vec![]),
            Reply::Received(count) => (
                self.received,
                vec![(
                    vec![Path::Field(self.count)],
                    TerminalScalarValue::Integer {
                        scalar_type: IntegerType::new(IntegerSign::Unsigned, 64).expect("u64"),
                        value: IntegerValue::Unsigned(count as u128),
                    },
                )],
            ),
        };
        TerminalEffectResult::Aggregate(TerminalAggregateValue::from_scalars(
            self.ty,
            Some(case),
            fields,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_byte_grant_sends_empty_datagrams_and_refuses_any_payload() {
        let receiver = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        receiver
            .set_read_timeout(Some(Duration::from_secs(1)))
            .unwrap();
        let channel = Datagrams {
            peer: BuildNetworkPeer::new(receiver.local_addr().unwrap(), 0).unwrap(),
            send_admitted: true,
            reply: None,
            socket: Mutex::default(),
        };
        assert!(channel.send(b"").unwrap());
        assert_eq!(receiver.recv(&mut [0; 1]).unwrap(), 0);
        assert!(channel.send(b"x").is_err());
        receiver.set_nonblocking(true).unwrap();
        assert_eq!(
            receiver.recv(&mut [0; 1]).unwrap_err().kind(),
            io::ErrorKind::WouldBlock
        );
    }
    #[test]
    fn receive_is_bounded_by_the_peer_and_the_view_and_preserves_unwritten_bytes() {
        let peer = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let channel = open_peer(peer.local_addr().unwrap()).unwrap();
        channel
            .set_read_timeout(Some(Duration::from_millis(30)))
            .unwrap();
        let client = channel.local_addr().unwrap();
        let mut buffer = [9; 4];
        for (packet, max, expected) in [
            (b"abc".as_slice(), 2, Reply::TooLarge),
            (b"abc".as_slice(), 8, Reply::TooLarge),
            (b"".as_slice(), 0, Reply::Received(0)),
        ] {
            peer.send_to(packet, client).unwrap();
            assert_eq!(receive_reply(&channel, &mut buffer[1..3], max), expected);
            assert_eq!(buffer, [9; 4]);
        }
        assert_eq!(receive_reply(&channel, &mut buffer, 4), Reply::Unavailable);
        assert_eq!(buffer, [9; 4]);
        peer.send_to(b"ok", client).unwrap();
        assert_eq!(
            receive_reply(&channel, &mut buffer[1..3], 2),
            Reply::Received(2)
        );
        assert_eq!(buffer, [9, b'o', b'k', 9]);
    }
}
