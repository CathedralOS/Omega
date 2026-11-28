//! Send one bounded datagram to the peer installed for this Build activation.

use package_compilation::build_execution::BuildNetworkPeer;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, UdpSocket};
use std::time::Duration;
use terminal_interpreter::TerminalEffectRejection;

pub(super) fn send(peer: &BuildNetworkPeer, bytes: &[u8]) -> Result<bool, TerminalEffectRejection> {
    if bytes.len() > usize::from(peer.max_datagram_bytes()) {
        return Err(TerminalEffectRejection::new(
            "Build datagram exceeds the admitted peer's byte limit",
        ));
    }
    Ok(send_datagram(peer.address(), bytes).unwrap_or(false))
}

fn send_datagram(peer: SocketAddr, bytes: &[u8]) -> std::io::Result<bool> {
    let local = match peer {
        SocketAddr::V4(_) => IpAddr::V4(Ipv4Addr::UNSPECIFIED),
        SocketAddr::V6(_) => IpAddr::V6(Ipv6Addr::UNSPECIFIED),
    };
    let socket = UdpSocket::bind(SocketAddr::new(local, 0))?;
    socket.set_write_timeout(Some(Duration::from_secs(1)))?;
    Ok(socket.send_to(bytes, peer)? == bytes.len())
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
        let peer = BuildNetworkPeer::new(receiver.local_addr().unwrap(), 0).unwrap();

        assert!(send(&peer, b"").unwrap());
        assert_eq!(receiver.recv(&mut [0; 1]).unwrap(), 0);
        assert!(send(&peer, b"x").is_err());
        receiver.set_nonblocking(true).unwrap();
        assert_eq!(
            receiver.recv(&mut [0; 1]).unwrap_err().kind(),
            std::io::ErrorKind::WouldBlock
        );
    }
}
