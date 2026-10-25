//! Decode the installed Darwin bridge independently of its writer and patcher.
//! Address pairs, byte stores, argument occurrences, branch destinations, the
//! exact semantic call and the original normal-return observations are replayed.

use super::super::{HostedReceiverPartitions, invalid};
use diagnostics::Diagnostic;

pub(in crate::hosted_receiver) fn validate(
    bytes: &[u8],
    address: u64,
    selected_entry: u64,
    bss: u64,
    partitions: HostedReceiverPartitions,
    recipes: &[terminal_installation::CheckedPrivateEntryRamRecipe],
) -> Result<(), Diagnostic> {
    let fail = || invalid(target::NativeTarget::macos_arm64());
    let (chunks, remainder) = bytes.as_chunks::<4>();
    if !remainder.is_empty() || !address.is_multiple_of(4) {
        return Err(fail());
    }
    let words = chunks
        .iter()
        .map(|bytes| u32::from_le_bytes(*bytes))
        .collect::<Vec<_>>();
    let mut reader = Reader {
        words: &words,
        position: 0,
        address,
    };
    let (continuation, stack_top, allocation) = super::expected_addresses(bss, partitions, &fail)?;
    reader.address(9, continuation)?;
    reader.exact(0x9100_03ea)?;
    reader.exact(0xa900_792a)?;
    reader.address(10, stack_top)?;
    reader.exact(0x9100_015f)?;
    let mut next = allocation;
    let mut originals = Vec::new();
    let mut headers = Vec::new();
    for recipe in recipes {
        let alignment = recipe.alignment();
        if !alignment.is_power_of_two() {
            return Err(fail());
        }
        let original = next
            .checked_add(alignment - 1)
            .map(|value| value & !(alignment - 1))
            .ok_or_else(fail)?;
        let header = original
            .checked_add(recipe.contents().len() as u64)
            .ok_or_else(fail)?;
        next = header
            .checked_add(recipe.observation_header().len() as u64)
            .ok_or_else(fail)?;
        originals.push(original);
        headers.push(header);
        reader.initialize(original, recipe.contents())?;
    }
    for ordinal in 0..recipes.len() {
        if ordinal >= 8 {
            return Err(fail());
        }
        let selected = recipes
            .iter()
            .position(|recipe| recipe.input().runtime_position as usize == ordinal)
            .ok_or_else(fail)?;
        reader.address(ordinal as u32, originals[selected])?;
    }
    let pc = reader.pc()?;
    let call = reader.take()?;
    if call & 0xfc00_0000 != 0x9400_0000
        || pc.checked_add_signed(((i64::from(call & 0x03ff_ffff) << 38) >> 38) * 4)
            != Some(selected_entry)
    {
        return Err(fail());
    }
    for ((recipe, original), header) in recipes.iter().zip(originals).zip(headers) {
        let metadata = recipe.observation_header();
        reader.initialize(header, &metadata)?;
        for (source, count) in [
            (header, metadata.len()),
            (original, recipe.contents().len()),
        ] {
            reader.exact(0xd280_0060)?; // Only the fresh runtime FD.
            reader.address(1, source)?;
            reader.count(2, count)?;
            reader.exact(0xd280_0090)?;
            reader.exact(0xd400_1001)?;
            reader.conditional_branch(2, 5)?; // Darwin carry error, before CMP.
            reader.count(9, count)?;
            reader.exact(0xeb09_001f)?;
            reader.conditional_branch(0, 5)?; // Full write skips failure only.
            for word in [0xd280_1900, 0xd280_0030, 0xd400_1001, 0xd420_0000] {
                reader.exact(word)?;
            }
        }
    }
    reader.address(9, continuation)?;
    for word in [0xa940_792a, 0x9100_015f, 0x5280_0000, 0xd65f_03c0] {
        reader.exact(word)?;
    }
    if reader.position != words.len() {
        return Err(fail());
    }
    Ok(())
}

struct Reader<'a> {
    words: &'a [u32],
    position: usize,
    address: u64,
}
impl Reader<'_> {
    fn take(&mut self) -> Result<u32, Diagnostic> {
        let word = self
            .words
            .get(self.position)
            .copied()
            .ok_or_else(|| invalid(target::NativeTarget::macos_arm64()))?;
        self.position += 1;
        Ok(word)
    }
    fn pc(&self) -> Result<u64, Diagnostic> {
        self.address
            .checked_add(
                (self.position as u64)
                    .checked_mul(4)
                    .ok_or_else(|| invalid(target::NativeTarget::macos_arm64()))?,
            )
            .ok_or_else(|| invalid(target::NativeTarget::macos_arm64()))
    }
    fn exact(&mut self, expected: u32) -> Result<(), Diagnostic> {
        if self.take()? != expected {
            return Err(invalid(target::NativeTarget::macos_arm64()));
        }
        Ok(())
    }
    fn address(&mut self, register: u32, expected: u64) -> Result<(), Diagnostic> {
        let fail = || invalid(target::NativeTarget::macos_arm64());
        let pc = self.pc()?;
        let page = self.take()?;
        let add = self.take()?;
        if page & 0x9f00_001f != 0x9000_0000 | register
            || add & 0xffc0_03ff != 0x9100_0000 | (register << 5) | register
        {
            return Err(fail());
        }
        let immediate = ((page >> 5) & 0x7ffff) << 2 | ((page >> 29) & 3);
        let signed_pages = (i64::from(immediate) << 43) >> 43;
        let actual = (pc & !0xfff)
            .checked_add_signed(signed_pages * 4096)
            .and_then(|value| value.checked_add(u64::from((add >> 10) & 0xfff)));
        if actual != Some(expected) {
            return Err(fail());
        }
        Ok(())
    }
    fn initialize(&mut self, original: u64, bytes: &[u8]) -> Result<(), Diagnostic> {
        self.address(9, original)?;
        for byte in bytes {
            let immediate = self.take()?;
            if immediate & 0xffe0_001f != 0x5280_000a
                || (immediate >> 5) & 0xffff != u32::from(*byte)
            {
                return Err(invalid(target::NativeTarget::macos_arm64()));
            }
            self.exact(0x3900_012a)?; // Store W10 to current original byte.
            self.exact(0x9100_0529)?; // Advance exactly one byte, no gap or alias.
        }
        Ok(())
    }
    fn count(&mut self, register: u32, expected: usize) -> Result<(), Diagnostic> {
        let low = self.take()?;
        let high = self.take()?;
        if low & 0xffe0_001f != 0xd280_0000 | register
            || high & 0xffe0_001f != 0xf2a0_0000 | register
            || u64::from(((low >> 5) & 0xffff) | (((high >> 5) & 0xffff) << 16)) != expected as u64
        {
            return Err(invalid(target::NativeTarget::macos_arm64()));
        }
        Ok(())
    }
    fn conditional_branch(&mut self, condition: u32, distance: i64) -> Result<(), Diagnostic> {
        let word = self.take()?;
        if word & 0xff00_001f != 0x5400_0000 | condition
            || ((i64::from((word >> 5) & 0x7ffff) << 45) >> 45) != distance
        {
            return Err(invalid(target::NativeTarget::macos_arm64()));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::Reader;

    #[test]
    fn original_byte_initialization_refuses_other_residence_or_missing_store() {
        let words = [
            0x9000_0009,
            0x9108_0129,
            0x5280_016a,
            0x3900_012a,
            0x9100_0529,
        ];
        let mut reader = Reader {
            words: &words,
            position: 0,
            address: 0x1000,
        };
        assert!(reader.initialize(0x1200, &[11]).is_ok());
        let mut other = Reader {
            words: &words,
            position: 0,
            address: 0x1000,
        };
        assert!(other.initialize(0x1204, &[11]).is_err());
        for (position, replacement) in [(2, 0x5280_018a), (3, 0xd503_201f), (4, 0x9100_0929)] {
            let mut corrupted = words;
            corrupted[position] = replacement;
            let mut reader = Reader {
                words: &corrupted,
                position: 0,
                address: 0x1000,
            };
            assert!(reader.initialize(0x1200, &[11]).is_err());
        }
    }

    #[test]
    fn original_argument_register_and_full_width_observation_count_are_exact() {
        let addresses = [0x9000_0000, 0x9108_0000];
        let mut reader = Reader {
            words: &addresses,
            position: 0,
            address: 0x1000,
        };
        assert!(reader.address(0, 0x1200).is_ok());
        let mut substituted = Reader {
            words: &addresses,
            position: 0,
            address: 0x1000,
        };
        assert!(substituted.address(1, 0x1200).is_err());
        let count = [0xd280_0002, 0xf2a0_0022];
        let mut reader = Reader {
            words: &count,
            position: 0,
            address: 0,
        };
        assert!(reader.count(2, 65536).is_ok());
        let mut truncated = Reader {
            words: &count,
            position: 0,
            address: 0,
        };
        assert!(truncated.count(2, 0).is_err());
    }

    #[test]
    fn observation_error_path_cannot_skip_the_fail_closed_exit() {
        for word in [0x5400_0082, 0x5400_00a3, 0x1400_0005] {
            let words = [word];
            let mut reader = Reader {
                words: &words,
                position: 0,
                address: 0,
            };
            assert!(reader.conditional_branch(2, 5).is_err());
        }
        let words = [0x5400_00a2];
        let mut reader = Reader {
            words: &words,
            position: 0,
            address: 0,
        };
        assert!(reader.conditional_branch(2, 5).is_ok());
    }
}
