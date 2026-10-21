//! Exact covered intervals for one foreign invocation.
//!
//! ABI marshaling, preservation and result capture can be separated by
//! compiler-only allocation instructions. The semantic intervals remain an
//! ordered union, never a hull or a call-only substitution. Mutable import and
//! callback offsets stay object-relative so each patch belongs to its exact
//! covered segment.

use crate::NativeByteSpan;
use crate::physical::derivation::evidence::{ranges_overlap, span};
use crate::physical::native_byte_span;
use image_emission::{ObjectCodeAttribution, ObjectForeignCall, ObjectFunction};
use sha2::{Digest, Sha256};

pub(super) struct CoveredInvocationIntervals {
    pub(super) machine: Vec<NativeByteSpan>,
    pub(super) object: Vec<NativeByteSpan>,
    pub(super) final_image: Vec<NativeByteSpan>,
}

pub(super) struct CoveredByteDigests {
    pub(super) machine: [u8; 32],
    pub(super) object: [u8; 32],
    pub(super) final_image: [u8; 32],
}

impl CoveredInvocationIntervals {
    pub(super) fn derive(
        function: &ObjectFunction,
        attributions: &[&ObjectCodeAttribution],
    ) -> Result<Self, &'static str> {
        if attributions.is_empty() {
            return Err("normalized foreign call has no emitted attribution intervals");
        }
        let mut machine = Vec::with_capacity(attributions.len());
        let mut object = Vec::with_capacity(attributions.len());
        for attribution in attributions {
            let offset = attribution.attribution.code_offset;
            let count = attribution.attribution.byte_count;
            let object_offset = attribution.text_offset;
            if count == 0
                || function.text_offset.checked_add(offset) != Some(object_offset)
                || offset.checked_add(count).is_none()
                || object_offset.checked_add(count).is_none()
                || machine.last().is_some_and(|previous: &NativeByteSpan| {
                    previous
                        .offset()
                        .checked_add(previous.byte_count())
                        .is_none_or(|end| end >= offset)
                })
            {
                return Err(
                    "normalized foreign call has unordered, overlapping or detached intervals",
                );
            }
            machine.push(native_byte_span(offset, count));
            object.push(native_byte_span(object_offset, count));
        }
        Ok(Self {
            machine,
            final_image: object.clone(),
            object,
        })
    }

    pub(super) fn contains_object_span(&self, offset: usize, count: usize) -> bool {
        count != 0
            && offset.checked_add(count).is_some_and(|end| {
                self.object.iter().any(|span| {
                    span.offset() <= offset
                        && span
                            .offset()
                            .checked_add(span.byte_count())
                            .is_some_and(|span_end| end <= span_end)
                })
            })
    }

    pub(super) fn overlaps_object_span(&self, offset: usize, count: usize) -> bool {
        offset.checked_add(count).is_some_and(|end| {
            self.object.iter().any(|span| {
                span.offset()
                    .checked_add(span.byte_count())
                    .is_some_and(|span_end| ranges_overlap(span.offset(), span_end, offset, end))
            })
        })
    }

    pub(super) fn require_invocation(
        &self,
        placed: Option<&machine_code::PlacedInstructionSpan>,
        architecture: target::Architecture,
        foreign: &ObjectForeignCall,
        object_bytes: &[u8],
    ) -> Result<(), &'static str> {
        let (offset, count) = if let Some(placed) = placed {
            (
                usize::try_from(placed.section_offset)
                    .map_err(|_| "foreign call offset overflow")?,
                usize::try_from(placed.byte_count).map_err(|_| "foreign call size overflow")?,
            )
        } else {
            // The object producer's import-site contract is E8+rel32 or BL,
            // not a four-byte operand mistaken for the complete x86 call.
            if foreign.binding.symbolic_import().is_none() {
                return Err("kernel entry lacks retained instruction custody");
            }
            match architecture {
                target::Architecture::X86_64 => {
                    let offset = foreign
                        .text_offset
                        .checked_sub(1)
                        .ok_or("foreign x86 call opcode offset underflow")?;
                    let end = offset
                        .checked_add(5)
                        .ok_or("foreign x86 call end overflow")?;
                    if object_bytes.get(offset..end) != Some(&[0xe8, 0, 0, 0, 0]) {
                        return Err(
                            "foreign object call differs from its exact x86 import instruction",
                        );
                    }
                    (offset, 5)
                }
                target::Architecture::Aarch64 => {
                    let end = foreign
                        .text_offset
                        .checked_add(4)
                        .ok_or("foreign aarch64 call end overflow")?;
                    if !foreign.text_offset.is_multiple_of(4)
                        || object_bytes.get(foreign.text_offset..end)
                            != Some(&0x9400_0000_u32.to_le_bytes())
                    {
                        return Err(
                            "foreign object call differs from its exact aarch64 import instruction",
                        );
                    }
                    (foreign.text_offset, 4)
                }
            }
        };
        if !self.contains_object_span(offset, count) {
            return Err("normalized foreign invocation instruction is outside its owned intervals");
        }
        Ok(())
    }

    pub(super) fn replay_bytes(
        &self,
        machine_bytes: &[u8],
        object_bytes: &[u8],
        final_image_bytes: &[u8],
        mutable_offsets: &[(usize, usize)],
    ) -> Result<CoveredByteDigests, &'static str> {
        for &(start, end) in mutable_offsets {
            let count = end
                .checked_sub(start)
                .ok_or("foreign mutable interval is reversed")?;
            if !self.contains_object_span(start, count) {
                return Err("foreign mutable interval crosses uncovered bytes");
            }
        }
        let mut machine_digest = Sha256::new();
        let mut object_digest = Sha256::new();
        let mut final_image_digest = Sha256::new();
        for ((machine_span, object_span), image_span) in
            self.machine.iter().zip(&self.object).zip(&self.final_image)
        {
            let before_machine = span(machine_bytes, *machine_span)?;
            let before_object = span(object_bytes, *object_span)?;
            let after_image = span(final_image_bytes, *image_span)?;
            if before_machine != before_object {
                return Err("normalized foreign call changed before object custody");
            }
            for (byte_index, (before, after)) in before_object.iter().zip(after_image).enumerate() {
                let offset = object_span
                    .offset()
                    .checked_add(byte_index)
                    .ok_or("normalized foreign covered byte offset overflow")?;
                if before != after
                    && !mutable_offsets
                        .iter()
                        .any(|(start, end)| offset >= *start && offset < *end)
                {
                    return Err(
                        "normalized foreign bytes changed outside their exact relocation set",
                    );
                }
            }
            machine_digest.update(before_machine);
            object_digest.update(before_object);
            final_image_digest.update(after_image);
        }
        Ok(CoveredByteDigests {
            machine: machine_digest.finalize().into(),
            object: object_digest.finalize().into(),
            final_image: final_image_digest.finalize().into(),
        })
    }
}
