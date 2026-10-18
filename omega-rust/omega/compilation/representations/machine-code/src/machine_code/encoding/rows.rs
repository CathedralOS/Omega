//! Encoded instruction rows and their exact decoded footprints.
//!
//! One encoding owns a single byte image and a single decoded-footprint
//! table. A row names its instruction bytes as an offset span into that
//! image and its decoded footprint as a handle into that table, so a row
//! owns no heap allocation of its own: an encoding of 4000 instructions
//! holds two growable buffers rather than 8000 independent ones, and
//! cloning or comparing an encoding walks contiguous memory instead of
//! chasing one indirection per instruction.

use crate::SelectedFormInternalMachineFixup;
use register_model::{RegisterUnitId, RegisterViewId};
use selected_instructions::{MachineAlternativeKey, MachineEncodedEffects, SelectedInstructionId};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeferredControlEncodingReason {
    RequiresResolvedBranchLayout,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedFormDecodedFootprint {
    pub register_reads: Vec<RegisterViewId>,
    pub register_writes: Vec<RegisterViewId>,
    pub implicit_defs: Vec<RegisterUnitId>,
    pub implicit_clobbers: Vec<RegisterUnitId>,
    pub encoded: MachineEncodedEffects,
}

/// Offset span of one row's instruction bytes inside its encoding's byte
/// image. The zero span is the absence state: a row encodes no bytes exactly
/// when it defers its encoding to resolved layout.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SelectedFormEncodedByteSpan {
    pub offset: u64,
    pub byte_count: u64,
}

/// Handle naming one row's decoded footprint in its encoding's footprint
/// table. The zero handle is the absence state, so a deferred control row
/// states its missing footprint by leaving the handle zero rather than by
/// wrapping it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SelectedFormDecodedFootprintHandle(u64);

/// The byte image and decoded-footprint table every row of one encoding
/// names. Interning is append-only and preserves row order, so two
/// encodings that produced the same rows in the same order hold the same
/// buffers; identity still hashes each row's own bytes through its span, not
/// the image as a whole.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SelectedFormEncodedRowStore {
    bytes: Vec<u8>,
    footprints: Vec<SelectedFormDecodedFootprint>,
}

impl SelectedFormEncodedRowStore {
    pub fn new() -> Self {
        Self::default()
    }

    /// Append one row's bytes and decoded footprint, returning the span and
    /// handle the row retains. Construction scratch stays a `Vec`; the
    /// durable shape is this store plus the row's coordinates into it.
    pub fn intern(
        &mut self,
        bytes: &[u8],
        footprint: SelectedFormDecodedFootprint,
    ) -> (
        SelectedFormEncodedByteSpan,
        SelectedFormDecodedFootprintHandle,
    ) {
        let span = SelectedFormEncodedByteSpan {
            offset: self.bytes.len() as u64,
            byte_count: bytes.len() as u64,
        };
        self.bytes.extend_from_slice(bytes);
        self.footprints.push(footprint);
        let handle = SelectedFormDecodedFootprintHandle(self.footprints.len() as u64);
        (span, handle)
    }

    /// The bytes a span names. An absent or out-of-range span reads as empty
    /// rather than panicking: validation routes consume proposed artifacts.
    pub fn bytes(&self, span: SelectedFormEncodedByteSpan) -> &[u8] {
        let Ok(offset) = usize::try_from(span.offset) else {
            return &[];
        };
        let Ok(byte_count) = usize::try_from(span.byte_count) else {
            return &[];
        };
        offset
            .checked_add(byte_count)
            .and_then(|end| self.bytes.get(offset..end))
            .unwrap_or(&[])
    }

    /// Mutable view of a span, for a route that patches an interned field in
    /// place rather than re-encoding the instruction.
    pub fn bytes_mut(&mut self, span: SelectedFormEncodedByteSpan) -> &mut [u8] {
        let Ok(offset) = usize::try_from(span.offset) else {
            return &mut [];
        };
        let Ok(byte_count) = usize::try_from(span.byte_count) else {
            return &mut [];
        };
        match offset.checked_add(byte_count) {
            Some(end) => self.bytes.get_mut(offset..end).unwrap_or(&mut []),
            None => &mut [],
        }
    }

    /// The footprint a handle names, absent for the zero handle.
    pub fn footprint(
        &self,
        handle: SelectedFormDecodedFootprintHandle,
    ) -> Option<&SelectedFormDecodedFootprint> {
        self.footprints.get(Self::index(handle)?)
    }

    pub fn footprint_mut(
        &mut self,
        handle: SelectedFormDecodedFootprintHandle,
    ) -> Option<&mut SelectedFormDecodedFootprint> {
        let index = Self::index(handle)?;
        self.footprints.get_mut(index)
    }

    /// One row's bytes and decoded footprint together, absent when the row
    /// defers its encoding and therefore has neither.
    pub fn encoded(
        &self,
        row: &SelectedFormEncodingRow,
    ) -> Option<(&[u8], &SelectedFormDecodedFootprint)> {
        Some((self.bytes(row.code), self.footprint(row.footprint)?))
    }

    fn index(handle: SelectedFormDecodedFootprintHandle) -> Option<usize> {
        usize::try_from(handle.0.checked_sub(1)?).ok()
    }
}

/// What one row still leaves unresolved. The row's bytes and footprint are
/// properties of the row itself; this names the field no layout-independent
/// encoding can bind yet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SelectedFormEncodingState {
    Encoded,
    DeferredControl {
        reason: DeferredControlEncodingReason,
    },
    UnresolvedInternalMachineCall {
        fixup: SelectedFormInternalMachineFixup,
    },
    /// Normalized foreign call whose import field stays unresolved until
    /// object construction binds it to the declared import symbol. The fixup
    /// names the selected roster row, never a raw foreign coordinate.
    UnresolvedNormalizedForeignCall {
        fixup: crate::SelectedFormNormalizedForeignCallFixup,
    },
    /// Image-relative address whose field stays unresolved until its exact
    /// referent is placed. The reference describes identity, not RAM authority.
    UnresolvedImageAddress {
        fixup: crate::SelectedFormImageAddressFixup,
    },
}

/// Closed rule-neutral disposition consumed by generic encoding and layout.
/// Rule-local plans remain the authority; this value is only their exact row
/// projection under authenticated optimization custody.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SelectedFormMachineDisposition {
    RetainedV1,
    Aarch64ElidedCompareI64ZeroV1 {
        consumer: SelectedInstructionId,
    },
    Aarch64FusedBranchNonZeroToCbnzV1 {
        compare: SelectedInstructionId,
        source_read: physical_instructions::QualifiedPhysicalRead,
    },
    Aarch64ElidedSameViewCopyI64V1 {
        consumer: SelectedInstructionId,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedFormEncodingRow {
    pub instruction: SelectedInstructionId,
    pub alternative: MachineAlternativeKey,
    pub machine_disposition: SelectedFormMachineDisposition,
    pub state: SelectedFormEncodingState,
    pub address: Option<ResolvedPhysicalAddress>,
    /// Instruction bytes in the encoding's byte image. Empty exactly when
    /// the state defers this row's encoding to resolved layout.
    pub code: SelectedFormEncodedByteSpan,
    /// Decoded footprint in the encoding's footprint table. Absent exactly
    /// when the state defers this row's encoding to resolved layout.
    pub footprint: SelectedFormDecodedFootprintHandle,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SelectedFormEncodingCounts {
    pub ordinary_encoded: u64,
    pub ordinary_deferred_control: u64,
    pub ordinary_encoded_call_templates: u64,
    pub ordinary_deferred_internal_control: u64,
    pub ordinary_internal_fixups: u64,
    /// Encoded normalized-foreign-call templates whose import fields stay
    /// unresolved until object construction binds them to declared import
    /// symbols. These are not internal control fixups.
    pub ordinary_normalized_foreign_import_fixups: u64,
    /// Encoded byte-literal image addresses whose fields stay unresolved
    /// until object construction binds them to read-only image symbols.
    pub ordinary_image_address_fixups: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResolvedPhysicalAddress {
    pub symbolic: physical_instructions::PhysicalAddressOperation,
    /// Signed byte displacement from the operand base. Negative values are
    /// admitted only for frame slots resident below the unadjusted entry
    /// stack pointer inside the ABI red zone.
    pub displacement: i64,
    /// One for an ordinary displacement. Otherwise round the absolute frame
    /// base upward to this alignment, then add the symbolic member offset.
    /// The retained frame owns the padded interval containing that address.
    pub frame_alignment_bytes: u16,
}
