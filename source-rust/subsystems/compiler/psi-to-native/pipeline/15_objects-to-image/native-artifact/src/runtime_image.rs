//! Read root machine layout descriptors from the completed image's initialized
//! data. Only validated addresses and extents enter the interpreter.

use semantic_vocabulary::MachineId;
struct RuntimeImageSections<'image> {
    text_address: u64,
    text: &'image [u8],
    data_address: u64,
    data: &'image [u8],
}

#[derive(Debug)]
pub struct RuntimeCodeLayout {
    pub machine: MachineId,
    pub entry: u64,
    pub regions: Vec<(u64, u64)>,
}

impl crate::RetainedNativeArtifact {
    /// Read linked descriptors from the completed image for machines in this
    /// root. Private object machine indexes belong to their own namespaces.
    pub fn inspect_root_code_layouts(&self) -> Result<Vec<RuntimeCodeLayout>, String> {
        let artifact = self.psi_artifact();
        let descriptions = self
            .inspect_native_code_layouts()
            .map_err(|error| error.to_string())?;
        let output = self.image().output();
        let sections = RuntimeImageSections {
            text_address: output.final_image_layout.text_address,
            text: &output.final_text_bytes,
            data_address: output.final_image_layout.data_address,
            data: &output.final_data_bytes,
        };
        let mut machines = std::collections::BTreeSet::new();
        let mut offsets = std::collections::BTreeSet::new();
        descriptions
            .iter()
            .filter(|row| belongs_to_root(row, artifact.view().identity()))
            .map(|row| {
                if artifact.view().module().machine(row.machine).is_none()
                    || !machines.insert(row.machine)
                    || !offsets.insert(row.data_section_offset)
                {
                    return Err(
                        "native layout descriptor has no unique root machine and placement".into(),
                    );
                }
                sections
                    .read_attributed_descriptor(row)
                    .map_err(str::to_owned)
            })
            .collect()
    }
}

fn belongs_to_root(
    row: &image_emission::LinkedNativeCodeLayout,
    root: terminal_psi::TerminalPsiIdentity,
) -> bool {
    row.private_function.is_none() && row.terminal_psi == root
}

impl RuntimeImageSections<'_> {
    fn read_attributed_descriptor(
        &self,
        row: &image_emission::LinkedNativeCodeLayout,
    ) -> Result<RuntimeCodeLayout, &'static str> {
        let decoded = self.read_descriptor(row.machine, row.data_section_offset)?;
        if decoded.entry != row.entry
            || !decoded
                .regions
                .iter()
                .copied()
                .eq(row.regions.iter().map(|r| (r.base, r.length)))
        {
            return Err(
                "completed descriptor differs from its machine's placed instruction attribution",
            );
        }
        Ok(decoded)
    }

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
    fn another_in_range_descriptor_cannot_replace_the_selected_machine_or_namespace() {
        let data: Vec<_> = [0x104_u64, 0x218, 1, 0x100, 8, 0x10c, 0x240, 1, 0x108, 8]
            .into_iter()
            .flat_map(u64::to_le_bytes)
            .collect();
        let sections = RuntimeImageSections {
            text_address: 0x100,
            text: &[0; 16],
            data_address: 0x200,
            data: &data,
        };
        let root = terminal_psi::TerminalPsiIdentity {
            vocabulary_marker: terminal_psi::VocabularyMarker,
            program_fingerprint: terminal_psi::SemanticFingerprint::from_bytes([1; 32]),
        };
        let other = terminal_psi::TerminalPsiIdentity {
            program_fingerprint: terminal_psi::SemanticFingerprint::from_bytes([2; 32]),
            ..root
        };
        let machine = MachineId::new(1).unwrap();
        let mut row = image_emission::LinkedNativeCodeLayout {
            private_function: None,
            terminal_psi: root,
            machine,
            data_section_offset: 0,
            entry: 0x104,
            regions: vec![image_emission::LinkedNativeCodeRegion {
                base: 0x100,
                length: 8,
            }],
        };
        assert!(belongs_to_root(&row, root));
        assert!(!belongs_to_root(&row, other));
        assert!(sections.read_attributed_descriptor(&row).is_ok());
        row.data_section_offset = 40;
        assert!(
            sections
                .read_descriptor(machine, row.data_section_offset)
                .is_ok()
        );
        assert!(sections.read_attributed_descriptor(&row).is_err());
        row.data_section_offset = 0;
        row.regions[0].length -= 1;
        assert!(sections.read_attributed_descriptor(&row).is_err());
        row.private_function = function_identity::MachineFunctionIdentity::callback_thunk(root, 0);
        assert!(!belongs_to_root(&row, root));
    }

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
