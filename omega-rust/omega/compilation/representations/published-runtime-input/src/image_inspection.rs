//! Read root machine layout descriptors from the completed image's initialized
//! data. Only validated addresses and extents enter the interpreter.

use semantic_vocabulary::MachineId;
use terminal_codec::CanonicalTerminalArtifact;
use terminal_psi::TerminalPsiIdentity;

#[derive(Debug)]
pub struct RuntimeImageInspection {
    manifest: [u8; 32],
    semantic: TerminalPsiIdentity,
    target: target::NativeTarget,
    identity: [u8; 32],
    layouts: Result<Vec<RuntimeCodeLayout>, String>,
}

pub struct RuntimeImageSections<'image> {
    pub text_address: u64,
    pub text: &'image [u8],
    pub data_address: u64,
    pub data: &'image [u8],
}

#[derive(Debug)]
pub struct RuntimeCodeLayout {
    pub machine: MachineId,
    pub entry: u64,
    pub regions: Vec<(u64, u64)>,
}

impl RuntimeImageInspection {
    /// The object inspector locates each descriptor in initialized data. Read
    /// its entry and regions from the same completed image, then discard both
    /// sections. A prior validation error remains an execution refusal.
    pub fn read(
        artifact: &CanonicalTerminalArtifact,
        target: target::NativeTarget,
        identity: [u8; 32],
        sections: RuntimeImageSections<'_>,
        descriptors: Result<impl Iterator<Item = (MachineId, usize)>, String>,
    ) -> Self {
        let layouts = descriptors.and_then(|descriptors| {
            if identity == [0; 32] {
                return Err("native layout inspection has no completed-image identity".into());
            }
            let mut machines = std::collections::BTreeSet::new();
            let mut data_offsets = std::collections::BTreeSet::new();
            descriptors
                .map(|(machine, offset)| {
                    if artifact.view().module().machine(machine).is_none() {
                        return Err("native layout descriptor names another root's machine".into());
                    }
                    if !machines.insert(machine) || !data_offsets.insert(offset) {
                        return Err("native layout reuses a machine or physical descriptor".into());
                    }
                    sections
                        .read_descriptor(machine, offset)
                        .map_err(str::to_owned)
                })
                .collect()
        });
        Self {
            manifest: *artifact.manifest().identity().as_bytes(),
            semantic: artifact.view().identity(),
            target,
            identity,
            layouts,
        }
    }

    pub fn matches(
        &self,
        artifact: &CanonicalTerminalArtifact,
        target: target::NativeTarget,
    ) -> bool {
        self.manifest == *artifact.manifest().identity().as_bytes()
            && self.semantic == artifact.view().identity()
            && self.target == target
    }

    pub fn into_layouts(self) -> Result<([u8; 32], Vec<RuntimeCodeLayout>), String> {
        Ok((self.identity, self.layouts?))
    }
}

impl RuntimeImageSections<'_> {
    fn read_descriptor(
        &self,
        machine: MachineId,
        offset: usize,
    ) -> Result<RuntimeCodeLayout, &'static str> {
        const INVALID: &str = "completed image has a malformed native layout descriptor";
        fn word(bytes: &mut &[u8]) -> Option<u64> {
            let (value, remaining) = bytes.split_first_chunk::<8>()?;
            *bytes = remaining;
            Some(u64::from_le_bytes(*value))
        }
        if !offset.is_multiple_of(8) {
            return Err(INVALID);
        }
        let mut data = self.data.get(offset..).ok_or(INVALID)?;
        let text_end = self
            .text_address
            .checked_add(u64::try_from(self.text.len()).map_err(|_| INVALID)?)
            .ok_or(INVALID)?;
        let entry = word(&mut data).ok_or(INVALID)?;
        let region_address = word(&mut data).ok_or(INVALID)?;
        let count = usize::try_from(word(&mut data).ok_or(INVALID)?).map_err(|_| INVALID)?;
        let expected_address = self
            .data_address
            .checked_add(u64::try_from(offset).map_err(|_| INVALID)?)
            .and_then(|base| base.checked_add(24))
            .ok_or(INVALID)?;
        if region_address != expected_address
            || entry == 0
            || !(self.text_address..text_end).contains(&entry)
        {
            return Err(INVALID);
        }
        let mut data = data
            .get(..count.checked_mul(16).ok_or(INVALID)?)
            .ok_or(INVALID)?;
        let mut regions = Vec::with_capacity(count);
        let mut previous_end = None;
        let mut includes_entry = false;
        while !data.is_empty() {
            let base = word(&mut data).ok_or(INVALID)?;
            let length = word(&mut data).ok_or(INVALID)?;
            let end = base.checked_add(length).ok_or(INVALID)?;
            if length == 0
                || base == 0
                || base < self.text_address
                || end > text_end
                || previous_end.is_some_and(|previous| base <= previous)
            {
                return Err(INVALID);
            }
            previous_end = Some(end);
            includes_entry |= (base..end).contains(&entry);
            regions.push((base, length));
        }
        if !includes_entry {
            return Err(INVALID);
        }
        Ok(RuntimeCodeLayout {
            machine,
            entry,
            regions,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn descriptor_must_resolve_inside_its_actual_text_and_initialized_data() {
        let text = [0; 16];
        let read = |words: &[u64]| {
            let data = words
                .iter()
                .flat_map(|word| word.to_le_bytes())
                .collect::<Vec<_>>();
            let sections = RuntimeImageSections {
                text_address: 0x100,
                text: &text,
                data_address: 0x200,
                data: &data,
            };
            sections.read_descriptor(MachineId::new(1).unwrap(), 0)
        };
        let layout = read(&[0x104, 0x218, 1, 0x100, 16]).unwrap();
        assert_eq!(
            (layout.entry, layout.regions.as_slice()),
            (0x104, [(0x100, 16)].as_slice())
        );
        assert!(read(&[0x104, 0x220, 1, 0x100, 16]).is_err());
        assert!(read(&[0x104, 0x218, 2, 0x100, 16]).is_err());
        assert!(read(&[0x104, 0x218, 1, 0x100, 17]).is_err());
        assert!(read(&[0x104, 0x218, 1, 0x100, 4]).is_err());
    }
}
