use std::io::{self, Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use super::model::{ResolverExecutionRequestedEndpoint, invalid_input};

pub(super) const CONNECT_REQUEST_BYTE_LIMIT: usize = 4 * 1024;
const CONNECT_HEADER_LIMIT: usize = 32;
const CONNECT_RESPONSE_BYTE_LIMIT: usize = 4 * 1024;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(5);

pub(super) enum ConnectRequestError {
    Malformed,
    Oversized,
}

pub(super) fn read_connect_request(
    client: &mut TcpStream,
    stop: &AtomicBool,
) -> Result<ResolverExecutionRequestedEndpoint, ConnectRequestError> {
    client
        .set_read_timeout(Some(Duration::from_millis(100)))
        .map_err(|_| ConnectRequestError::Malformed)?;
    let deadline = Instant::now() + REQUEST_TIMEOUT;
    let mut request = Vec::with_capacity(512);
    let mut buffer = [0_u8; 512];
    loop {
        if stop.load(Ordering::Acquire) || Instant::now() >= deadline {
            return Err(ConnectRequestError::Malformed);
        }
        match client.read(&mut buffer) {
            Ok(0) => return Err(ConnectRequestError::Malformed),
            Ok(count) => {
                if request.len().saturating_add(count) > CONNECT_REQUEST_BYTE_LIMIT {
                    return Err(ConnectRequestError::Oversized);
                }
                request.extend_from_slice(&buffer[..count]);
                if request.windows(4).any(|window| window == b"\r\n\r\n") {
                    return parse_connect_request(&request);
                }
            }
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                ) => {}
            Err(_) => return Err(ConnectRequestError::Malformed),
        }
    }
}

fn parse_connect_request(
    request: &[u8],
) -> Result<ResolverExecutionRequestedEndpoint, ConnectRequestError> {
    let end = request
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .ok_or(ConnectRequestError::Malformed)?
        + 4;
    if end != request.len() {
        return Err(ConnectRequestError::Malformed);
    }
    let request = std::str::from_utf8(request).map_err(|_| ConnectRequestError::Malformed)?;
    let lines = request[..request.len() - 4]
        .split("\r\n")
        .collect::<Vec<_>>();
    if lines.is_empty() || lines.len() > CONNECT_HEADER_LIMIT + 1 {
        return Err(if lines.len() > CONNECT_HEADER_LIMIT + 1 {
            ConnectRequestError::Oversized
        } else {
            ConnectRequestError::Malformed
        });
    }
    let mut request_line = lines[0].split(' ');
    if request_line.next() != Some("CONNECT") {
        return Err(ConnectRequestError::Malformed);
    }
    let authority = request_line.next().ok_or(ConnectRequestError::Malformed)?;
    if request_line.next() != Some("HTTP/1.1") || request_line.next().is_some() {
        return Err(ConnectRequestError::Malformed);
    }
    for header in &lines[1..] {
        let Some((name, value)) = header.split_once(':') else {
            return Err(ConnectRequestError::Malformed);
        };
        if name.is_empty()
            || !name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
            || value.bytes().any(|byte| byte < b' ' && byte != b'\t')
        {
            return Err(ConnectRequestError::Malformed);
        }
        if name.eq_ignore_ascii_case("transfer-encoding")
            || name.eq_ignore_ascii_case("content-length") && value.trim() != "0"
        {
            return Err(ConnectRequestError::Malformed);
        }
    }
    parse_authority(authority).map_err(|_| ConnectRequestError::Malformed)
}

pub(super) fn parse_authority(authority: &str) -> io::Result<ResolverExecutionRequestedEndpoint> {
    if authority.starts_with('[') {
        let close = authority
            .find(']')
            .ok_or_else(|| invalid_input("CONNECT authority has an unterminated IPv6 address"))?;
        if authority.as_bytes().get(close + 1) != Some(&b':') {
            return Err(invalid_input("CONNECT authority has no port"));
        }
        let host = &authority[1..close];
        let port = parse_port(&authority[close + 2..])?;
        return ResolverExecutionRequestedEndpoint::new(host, port);
    }
    let (host, port) = authority
        .rsplit_once(':')
        .ok_or_else(|| invalid_input("CONNECT authority has no port"))?;
    if host.contains(':') {
        return Err(invalid_input("CONNECT IPv6 authority is not bracketed"));
    }
    ResolverExecutionRequestedEndpoint::new(host, parse_port(port)?)
}

fn parse_port(port: &str) -> io::Result<u16> {
    if port.is_empty() || !port.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(invalid_input("resolver endpoint port is malformed"));
    }
    let port = port
        .parse::<u16>()
        .map_err(|_| invalid_input("resolver endpoint port is out of range"))?;
    if port == 0 {
        return Err(invalid_input("resolver endpoint port is zero"));
    }
    Ok(port)
}

pub(super) fn write_response(stream: &mut TcpStream, response: &[u8]) -> io::Result<()> {
    stream.set_write_timeout(Some(Duration::from_secs(1)))?;
    stream.write_all(response)
}

pub(super) fn open_connect_tunnel(
    broker: SocketAddr,
    target: &ResolverExecutionRequestedEndpoint,
) -> io::Result<(TcpStream, Vec<u8>)> {
    let mut stream = TcpStream::connect_timeout(&broker, REQUEST_TIMEOUT)?;
    stream.set_read_timeout(Some(REQUEST_TIMEOUT))?;
    stream.set_write_timeout(Some(REQUEST_TIMEOUT))?;
    let authority = target.authority();
    write!(
        stream,
        "CONNECT {authority} HTTP/1.1\r\nHost: {authority}\r\nContent-Length: 0\r\n\r\n"
    )?;
    stream.flush()?;

    let trailing = read_connect_response(&mut stream)?;
    stream.set_read_timeout(None)?;
    stream.set_write_timeout(None)?;
    Ok((stream, trailing))
}

fn read_connect_response(stream: &mut TcpStream) -> io::Result<Vec<u8>> {
    let mut response = Vec::with_capacity(512);
    let mut buffer = [0_u8; 512];
    loop {
        let count = stream.read(&mut buffer)?;
        if count == 0 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "the resolver broker closed before its CONNECT response",
            ));
        }
        if response.len().saturating_add(count) > CONNECT_RESPONSE_BYTE_LIMIT {
            return Err(io::Error::other(
                "the resolver broker CONNECT response exceeded its fixed limit",
            ));
        }
        response.extend_from_slice(&buffer[..count]);
        let Some(end) = response.windows(4).position(|window| window == b"\r\n\r\n") else {
            continue;
        };
        let header_end = end + 4;
        let header = std::str::from_utf8(&response[..header_end])
            .map_err(|_| invalid_input("the resolver broker response is not ASCII"))?;
        let mut lines = header[..header.len() - 4].split("\r\n");
        if lines.next() != Some("HTTP/1.1 200 Connection Established")
            || lines.any(|line| {
                line.split_once(':').is_none_or(|(name, value)| {
                    name.is_empty()
                        || !name
                            .bytes()
                            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
                        || value.bytes().any(|byte| byte < b' ' && byte != b'\t')
                })
            })
        {
            return Err(io::Error::other(
                "the resolver broker rejected the CONNECT request",
            ));
        }
        return Ok(response[header_end..].to_vec());
    }
}
