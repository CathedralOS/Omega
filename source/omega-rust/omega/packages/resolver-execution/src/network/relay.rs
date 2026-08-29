use std::io::{self, Read, Write};
use std::net::{Shutdown, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use super::budget::ResolverExecutionTransferBudget;

const RELAY_TIMEOUT: Duration = Duration::from_secs(120);
const IO_POLL_INTERVAL: Duration = Duration::from_millis(2);

pub(super) fn relay(
    client: &mut TcpStream,
    upstream: &mut TcpStream,
    stop: &AtomicBool,
    transfer_budget: &ResolverExecutionTransferBudget,
) -> RelayObservation {
    if client.set_nonblocking(true).is_err() || upstream.set_nonblocking(true).is_err() {
        let _ = client.shutdown(Shutdown::Both);
        let _ = upstream.shutdown(Shutdown::Both);
        return RelayObservation::default();
    }
    let deadline = Instant::now() + RELAY_TIMEOUT;
    let mut client_to_upstream = RelayDirection::default();
    let mut upstream_to_client = RelayDirection::default();
    while Instant::now() < deadline && !stop.load(Ordering::Acquire) {
        let mut progressed = false;
        match client_to_upstream.pump(client, upstream, transfer_budget) {
            Ok(direction_progressed) => progressed |= direction_progressed,
            Err(RelayError::TransferLimit) => {
                return relay_observation(&client_to_upstream, &upstream_to_client, true);
            }
            Err(RelayError::Io) => break,
        }
        match upstream_to_client.pump(upstream, client, transfer_budget) {
            Ok(direction_progressed) => progressed |= direction_progressed,
            Err(RelayError::TransferLimit) => {
                return relay_observation(&client_to_upstream, &upstream_to_client, true);
            }
            Err(RelayError::Io) => break,
        }
        if client_to_upstream.complete() && upstream_to_client.complete() {
            return relay_observation(&client_to_upstream, &upstream_to_client, false);
        }
        if !progressed {
            thread::sleep(IO_POLL_INTERVAL);
        }
    }
    let _ = client.shutdown(Shutdown::Both);
    let _ = upstream.shutdown(Shutdown::Both);
    relay_observation(&client_to_upstream, &upstream_to_client, false)
}

#[derive(Default)]
pub(super) struct RelayObservation {
    pub(super) uploaded_bytes: u64,
    pub(super) downloaded_bytes: u64,
    pub(super) transfer_ceiling_reached: bool,
}

fn relay_observation(
    upload: &RelayDirection,
    download: &RelayDirection,
    transfer_ceiling_reached: bool,
) -> RelayObservation {
    RelayObservation {
        uploaded_bytes: upload.transferred_bytes,
        downloaded_bytes: download.transferred_bytes,
        transfer_ceiling_reached,
    }
}

enum RelayError {
    Io,
    TransferLimit,
}

#[derive(Default)]
struct RelayDirection {
    buffer: Vec<u8>,
    offset: usize,
    source_closed: bool,
    destination_closed: bool,
    transferred_bytes: u64,
}

impl RelayDirection {
    fn pump(
        &mut self,
        source: &mut TcpStream,
        destination: &mut TcpStream,
        transfer_budget: &ResolverExecutionTransferBudget,
    ) -> Result<bool, RelayError> {
        let mut progressed = false;
        if self.offset < self.buffer.len() {
            match destination.write(&self.buffer[self.offset..]) {
                Ok(0) => self.destination_closed = true,
                Ok(count) => {
                    self.offset += count;
                    progressed = true;
                }
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => {}
                Err(_error) => return Err(RelayError::Io),
            }
            if self.offset == self.buffer.len() {
                self.buffer.clear();
                self.offset = 0;
            }
        }
        if self.buffer.is_empty() && !self.source_closed && !self.destination_closed {
            let mut buffer = [0_u8; 16 * 1024];
            match source.read(&mut buffer) {
                Ok(0) => {
                    self.source_closed = true;
                    let _ = destination.shutdown(Shutdown::Write);
                }
                Ok(count) => {
                    transfer_budget
                        .charge(count)
                        .map_err(|()| RelayError::TransferLimit)?;
                    self.transferred_bytes = self
                        .transferred_bytes
                        .checked_add(u64::try_from(count).map_err(|_| RelayError::Io)?)
                        .ok_or(RelayError::Io)?;
                    self.buffer.extend_from_slice(&buffer[..count]);
                    progressed = true;
                }
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => {}
                Err(_error) => return Err(RelayError::Io),
            }
        }
        Ok(progressed)
    }

    fn complete(&self) -> bool {
        (self.source_closed || self.destination_closed) && self.buffer.is_empty()
    }
}
