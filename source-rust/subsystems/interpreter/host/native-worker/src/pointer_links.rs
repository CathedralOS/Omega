/*
We link and compare pointers inside the worker's address space (`link_pointer`,
`pointer_matches`, `returned_pointer_matches`).

Because shared mappings reside at different virtual addresses in the host and
worker processes, the sponsor supplies checked `NativeHomeId` and range
identities rather than raw addresses. When comparing a returned 8-byte word with
an admitted home range, the word acts purely as an address selector and is never
dereferenced.
*/

use crate::{
    native_provider_runtime::{NativeHomeId, NativeProviderError, NativeProviderProcess},
    wire::{Cursor, Packet, invalid},
    worker::Worker,
};
use std::io;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NativePointerSlot {
    pub home: NativeHomeId,
    pub offset: usize,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NativePointerTarget {
    pub home: NativeHomeId,
    pub offset: usize,
    pub byte_size: usize,
    pub alignment: usize,
}

impl NativeProviderProcess {
    pub fn link_pointer(
        &mut self,
        slot: NativePointerSlot,
        target: NativePointerTarget,
    ) -> Result<(), NativeProviderError> {
        let packet = self.pointer_packet(5, slot, target)?;
        if !self.exchange(packet)?.is_empty() {
            return Err(invalid("native pointer link reply has a payload").into());
        }
        Ok(())
    }
    pub fn pointer_matches(
        &mut self,
        slot: NativePointerSlot,
        target: NativePointerTarget,
    ) -> Result<bool, NativeProviderError> {
        let packet = self.pointer_packet(6, slot, target)?;
        match self.exchange(packet)?.as_slice() {
            [0] => Ok(false),
            [1] => Ok(true),
            _ => Err(invalid("native pointer comparison reply is malformed").into()),
        }
    }

    pub fn returned_pointer_matches(
        &mut self,
        value: &[u8],
        target: NativePointerTarget,
    ) -> Result<bool, NativeProviderError> {
        if value.len() != 8 || self.faulted || target.home.era != self.era {
            return Err(invalid("returned pointer has no current native incarnation").into());
        }
        self.homes
            .get(&target.home.ordinal)
            .ok_or_else(|| invalid("returned pointer target has no home"))?
            .pointer(target.offset, target.byte_size, target.alignment)?;
        let mut packet = Packet::new(7);
        packet.bytes(value)?;
        for number in [
            target.home.ordinal,
            target.offset as u64,
            target.byte_size as u64,
            target.alignment as u64,
        ] {
            packet.number(number);
        }
        match self.exchange(packet)?.as_slice() {
            [0] => Ok(false),
            [1] => Ok(true),
            _ => Err(invalid("returned pointer comparison reply is malformed").into()),
        }
    }
    fn pointer_packet(
        &self,
        tag: u8,
        slot: NativePointerSlot,
        target: NativePointerTarget,
    ) -> Result<Packet, NativeProviderError> {
        if self.faulted || slot.home.era != self.era || target.home.era != self.era {
            return Err(
                invalid("native pointer link belongs to another or faulted incarnation").into(),
            );
        }
        self.homes
            .get(&slot.home.ordinal)
            .ok_or_else(|| invalid("native pointer slot has no home"))?
            .pointer(slot.offset, 8, 8)?;
        self.homes
            .get(&target.home.ordinal)
            .ok_or_else(|| invalid("native pointer target has no home"))?
            .pointer(target.offset, target.byte_size, target.alignment)?;
        let mut packet = Packet::new(tag);
        for number in [
            slot.home.ordinal,
            slot.offset as u64,
            target.home.ordinal,
            target.offset as u64,
            target.byte_size as u64,
            target.alignment as u64,
        ] {
            packet.number(number);
        }
        Ok(packet)
    }
}

impl Worker {
    pub(super) fn returned_pointer_matches(&self, mut cursor: Cursor<'_>) -> io::Result<Vec<u8>> {
        let actual = cursor.bytes()?;
        if actual.len() != 8 {
            return Err(invalid("returned pointer word has invalid width"));
        }
        let home = cursor.number()?;
        let offset = cursor.usize()?;
        let extent = cursor.usize()?;
        let alignment = cursor.usize()?;
        cursor.finish()?;
        let target = self
            .homes
            .get(&home)
            .ok_or_else(|| invalid("returned pointer target has no mapped home"))?
            .pointer(offset, extent, alignment)?;
        Ok(vec![u8::from(actual == (target as u64).to_le_bytes())])
    }

    pub(super) fn pointer_link(
        &mut self,
        mut cursor: Cursor<'_>,
        write: bool,
    ) -> io::Result<Vec<u8>> {
        let slot_id = cursor.number()?;
        let slot_offset = cursor.usize()?;
        let target_id = cursor.number()?;
        let target_offset = cursor.usize()?;
        let extent = cursor.usize()?;
        let alignment = cursor.usize()?;
        cursor.finish()?;
        let target = self
            .homes
            .get(&target_id)
            .ok_or_else(|| invalid("native pointer target has no mapped home"))?
            .pointer(target_offset, extent, alignment)?;
        let slot = self
            .homes
            .get_mut(&slot_id)
            .ok_or_else(|| invalid("native pointer slot has no mapped home"))?;
        slot.pointer(slot_offset, 8, 8)?;
        let bytes = (target as u64).to_le_bytes();
        if write {
            slot.write(slot_offset, &bytes)?;
            Ok(Vec::new())
        } else {
            let mut actual = [0; 8];
            slot.read(slot_offset, &mut actual)?;
            Ok(vec![u8::from(actual == bytes)])
        }
    }
}
