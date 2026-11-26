//! Mach-O and PE reach each import through one branch that loads a pointer slot
//! the loader fills. Both formats append the same encoded branch to `.text`,
//! patch it once layout fixes the slot address, and re-check it after
//! relocation, so those three steps live here; each format places its own slot.

use crate::{
    FinalExecutableRegion, FinalExecutableRegionOrigin, FinalImage, FinalImageSection,
    FinalImageSymbolHandle,
};
use calling_conventions::StateFootprintEvidence;
use diagnostics::Diagnostic;
use machine_code::import_code::{
    ImportOperand, ImportOperandKind as Kind, ImportOperandTarget as Target,
};
use target::Architecture;

/// The instruction set of a slot-branch import thunk.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlotThunkIsa {
    X86_64,
    Aarch64,
}

impl SlotThunkIsa {
    pub const fn of(architecture: Architecture) -> Self {
        match architecture {
            Architecture::X86_64 => Self::X86_64,
            Architecture::Aarch64 => Self::Aarch64,
        }
    }

    pub const fn byte_count(self) -> usize {
        match self {
            Self::X86_64 => target_x86_64::import_thunk::X86_64_IMPORT_THUNK_BYTE_COUNT,
            Self::Aarch64 => target_aarch64::import_thunk::AARCH64_IMPORT_THUNK_BYTE_COUNT,
        }
    }

    fn operands(self) -> Vec<ImportOperand> {
        let slot = |offset, kind| ImportOperand {
            offset,
            kind,
            target: Target::BindingSlot,
        };
        match self {
            Self::X86_64 => vec![slot(2, Kind::X86Relative32)],
            Self::Aarch64 => vec![
                slot(0, Kind::Aarch64Page21),
                slot(4, Kind::Aarch64PageOffset12 { scale: 3 }),
            ],
        }
    }

    pub fn footprint(self) -> StateFootprintEvidence {
        match self {
            Self::X86_64 => target_x86_64::import_thunk::x86_64_import_thunk_footprint(),
            Self::Aarch64 => target_aarch64::import_thunk::aarch64_import_thunk_footprint(),
        }
    }

    fn is_canonical(self, bytes: &[u8]) -> bool {
        match self {
            Self::X86_64 => {
                target_x86_64::import_thunk::decode_x86_64_import_thunk(bytes).is_some()
            }
            Self::Aarch64 => target_aarch64::import_thunk::decode_aarch64_import_thunk(bytes),
        }
    }

    fn patch(self, bytes: &mut [u8], thunk_address: u64, slot_address: u64) -> Result<(), &str> {
        match self {
            Self::X86_64 => target_x86_64::import_thunk::patch_x86_64_import_thunk(
                bytes,
                thunk_address,
                slot_address,
            ),
            Self::Aarch64 => target_aarch64::import_thunk::patch_aarch64_import_thunk(
                bytes,
                thunk_address,
                slot_address,
            ),
        }
    }
}

fn thunk_error(format: &str, symbol: &str, detail: &str) -> Diagnostic {
    Diagnostic::error(format!("{format} import thunk `{symbol}` {detail}"))
}

impl FinalImage {
    fn slot_thunk_range(
        &self,
        format: &str,
        symbol: &str,
        text_offset: usize,
    ) -> Result<std::ops::Range<usize>, Diagnostic> {
        let isa = SlotThunkIsa::of(self.target.architecture);
        let end = text_offset
            .checked_add(isa.byte_count())
            .ok_or_else(|| thunk_error(format, symbol, "range overflows"))?;
        if end > self.memory.text.len() {
            return Err(thunk_error(format, symbol, "is out of final .text bounds"));
        }
        Ok(text_offset..end)
    }

    /// Append the encoded thunk of the import `symbol_handle` to `.text`, move
    /// its symbol onto it and record its executable region. Returns its offset.
    pub fn install_slot_thunk(
        &mut self,
        format: &str,
        symbol_handle: FinalImageSymbolHandle,
    ) -> Result<usize, Diagnostic> {
        let isa = SlotThunkIsa::of(self.target.architecture);
        let body = self.encoded_import_body(symbol_handle)?;
        if body.bytes.len() != isa.byte_count() || body.operands != isa.operands() {
            return Err(Diagnostic::error(format!(
                "{format} import body has a different encoded operand contract"
            )));
        }
        let footprint = body.resources.effects.footprint().clone();
        let bytes = body.bytes.clone();
        let text_offset = self.memory.text.len();
        self.memory.text.extend(bytes);

        let symbol = self.symbol_table.symbols.get_mut(symbol_handle);
        symbol.section = FinalImageSection::Text;
        symbol.offset = text_offset;
        symbol.size = isa.byte_count();
        let symbol = symbol.name.clone();

        self.executable_regions.push(FinalExecutableRegion {
            origin: FinalExecutableRegionOrigin::ImportThunk,
            section_offset: text_offset,
            byte_count: isa.byte_count(),
            symbol,
            footprint: Some(footprint),
            entry_effects: None,
        });
        Ok(text_offset)
    }

    /// Point the installed thunk at `text_offset` at the slot at `slot_address`.
    pub fn patch_slot_thunk(
        &mut self,
        format: &str,
        symbol: &str,
        text_offset: usize,
        thunk_address: u64,
        slot_address: u64,
    ) -> Result<(), Diagnostic> {
        let isa = SlotThunkIsa::of(self.target.architecture);
        let range = self.slot_thunk_range(format, symbol, text_offset)?;
        isa.patch(&mut self.memory.text[range], thunk_address, slot_address)
            .map_err(|reason| thunk_error(format, symbol, reason))
    }

    /// Check that the thunk at `text_offset` kept its canonical branch through
    /// relocation and that its region still claims the branch's footprint.
    pub fn validate_slot_thunk(
        &self,
        format: &str,
        symbol: &str,
        text_offset: usize,
    ) -> Result<(), Diagnostic> {
        let isa = SlotThunkIsa::of(self.target.architecture);
        let range = self.slot_thunk_range(format, symbol, text_offset)?;
        if !isa.is_canonical(&self.memory.text[range]) {
            return Err(thunk_error(
                format,
                symbol,
                "does not match the ISA's canonical slot branch",
            ));
        }
        let region = self
            .executable_regions
            .iter()
            .find(|region| {
                region.origin == FinalExecutableRegionOrigin::ImportThunk
                    && region.section_offset == text_offset
                    && region.byte_count == isa.byte_count()
                    && region.symbol == symbol
            })
            .ok_or_else(|| {
                thunk_error(format, symbol, "is missing its executable-region record")
            })?;
        if region.footprint.as_ref() != Some(&isa.footprint()) {
            return Err(thunk_error(
                format,
                symbol,
                "effects differ from the encoded program",
            ));
        }
        Ok(())
    }
}
