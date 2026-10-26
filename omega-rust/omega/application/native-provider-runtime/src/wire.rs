//! Bounded private-worker messages, not a language artifact or public protocol.
//! One in-flight request on the owned stream correlates its exact response;
//! transport success supplies no semantic completion or ownership receipt.

use std::io::{self, Read, Write};
pub(crate) const MAXIMUM_PACKET_BYTES: usize = 16 * 1024 * 1024;
pub(crate) trait Duplex: Read + Write {}
impl<T: Read + Write> Duplex for T {}

pub(crate) fn send(stream: &mut dyn Duplex, bytes: &[u8]) -> io::Result<()> {
    if bytes.len() > MAXIMUM_PACKET_BYTES {
        return Err(invalid("worker packet exceeds sponsor's byte budget"));
    }
    stream.write_all(&(bytes.len() as u32).to_le_bytes())?;
    stream.write_all(bytes)?;
    stream.flush()
}
pub(crate) fn receive(stream: &mut dyn Duplex) -> io::Result<Vec<u8>> {
    let mut length = [0; 4];
    stream.read_exact(&mut length)?;
    let length = u32::from_le_bytes(length) as usize;
    if length == 0 || length > MAXIMUM_PACKET_BYTES {
        return Err(invalid("worker packet has empty or excessive extent"));
    }
    let mut bytes = vec![0; length];
    stream.read_exact(&mut bytes)?;
    Ok(bytes)
}
pub(crate) fn invalid(reason: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, reason)
}

pub(crate) struct Packet(pub(crate) Vec<u8>);
impl Packet {
    pub(crate) fn new(tag: u8) -> Self {
        Self(vec![tag])
    }
    pub(crate) fn byte(&mut self, value: u8) {
        self.0.push(value);
    }
    pub(crate) fn number(&mut self, value: u64) {
        self.0.extend_from_slice(&value.to_le_bytes());
    }
    pub(crate) fn bytes(&mut self, value: &[u8]) -> io::Result<()> {
        let count =
            u64::try_from(value.len()).map_err(|_| invalid("worker byte extent overflows"))?;
        if self
            .0
            .len()
            .checked_add(value.len())
            .and_then(|n| n.checked_add(8))
            .is_none_or(|n| n > MAXIMUM_PACKET_BYTES)
        {
            return Err(invalid("worker message exceeds sponsor's byte budget"));
        }
        self.number(count);
        self.0.extend_from_slice(value);
        Ok(())
    }
    pub(crate) fn text(&mut self, value: &str) -> io::Result<()> {
        self.bytes(value.as_bytes())
    }
}

pub(crate) struct Cursor<'a> {
    bytes: &'a [u8],
    offset: usize,
}
impl<'a> Cursor<'a> {
    pub(crate) fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }
    pub(crate) fn take(&mut self, extent: usize) -> io::Result<&'a [u8]> {
        let end = self
            .offset
            .checked_add(extent)
            .ok_or_else(|| invalid("worker byte coordinate overflows"))?;
        let result = self
            .bytes
            .get(self.offset..end)
            .ok_or_else(|| invalid("worker message is truncated"))?;
        self.offset = end;
        Ok(result)
    }
    pub(crate) fn byte(&mut self) -> io::Result<u8> {
        Ok(self.take(1)?[0])
    }
    pub(crate) fn number(&mut self) -> io::Result<u64> {
        Ok(u64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }
    pub(crate) fn usize(&mut self) -> io::Result<usize> {
        usize::try_from(self.number()?)
            .map_err(|_| invalid("worker number exceeds host address model"))
    }
    pub(crate) fn bytes(&mut self) -> io::Result<&'a [u8]> {
        let count = self.usize()?;
        self.take(count)
    }
    pub(crate) fn text(&mut self) -> io::Result<&'a str> {
        std::str::from_utf8(self.bytes()?).map_err(|_| invalid("worker text is not UTF-8"))
    }
    pub(crate) fn remaining(&self) -> usize {
        self.bytes.len() - self.offset
    }
    pub(crate) fn finish(self) -> io::Result<()> {
        if self.offset == self.bytes.len() {
            Ok(())
        } else {
            Err(invalid("worker message has trailing bytes"))
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) struct Geometry {
    pub(crate) extent: usize,
    pub(crate) alignment: usize,
    pub(crate) referent: Option<(usize, usize)>,
}
impl Geometry {
    pub(crate) fn write(self, packet: &mut Packet) {
        packet.number(self.extent as u64);
        packet.number(self.alignment as u64);
        packet.byte(u8::from(self.referent.is_some()));
        if let Some((extent, alignment)) = self.referent {
            packet.number(extent as u64);
            packet.number(alignment as u64);
        }
    }
    pub(crate) fn read(cursor: &mut Cursor<'_>) -> io::Result<Self> {
        let extent = cursor.usize()?;
        let alignment = cursor.usize()?;
        let referent = match cursor.byte()? {
            0 => None,
            1 => Some((cursor.usize()?, cursor.usize()?)),
            _ => return Err(invalid("worker borrowed marker is not Boolean")),
        };
        if extent > MAXIMUM_PACKET_BYTES
            || !alignment.is_power_of_two()
            || alignment > MAXIMUM_PACKET_BYTES
        {
            return Err(invalid(
                "worker value geometry exceeds the sponsor's native allocation budget",
            ));
        }
        if let Some((size, align)) = referent {
            if extent != 8
                || alignment != 8
                || size > MAXIMUM_PACKET_BYTES
                || !align.is_power_of_two()
                || align > MAXIMUM_PACKET_BYTES
            {
                return Err(invalid(
                    "worker borrow geometry is outside the native pointer/home budget",
                ));
            }
        }
        Ok(Self {
            extent,
            alignment,
            referent,
        })
    }
}
