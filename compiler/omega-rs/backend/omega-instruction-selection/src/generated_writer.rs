//! Target lowering for reusable post-handoff writer helpers.
//!
//! This seam consumes only normalized layout-writer geometry. Consumer
//! semantics, symbolic identities, exact placement, resolver authority, and
//! content remain in the separate invocation plan.

use omega_calling_conventions::{MachineRegister, StateFootprintEvidence};
use omega_core::diagnostics::Diagnostic;
use omega_executable_installation::{InstalledCode, ResolvedPostHandoffEntryWriterContext};
use omega_layout_plans::{PostHandoffWriterInvocationPlan, PostHandoffWriterPlan};
use omega_target::{Architecture, NativeTarget};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoweredPostHandoffWriterHelper {
    pub target: NativeTarget,
    pub pointer_register: MachineRegister,
    pub normalized_plan_fingerprint: u64,
    pub emitted_bytes_fingerprint: u64,
    pub bytes: Vec<u8>,
    pub footprint: StateFootprintEvidence,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoweredPostHandoffWriter {
    pub helper: LoweredPostHandoffWriterHelper,
    pub invocation: PostHandoffWriterInvocationPlan,
}

/// One exact provider invocation paired with the reusable helper bytes that
/// consume it. The installed-code resolver owns the sealed packed words;
/// helper code, target identity, and exact footprint remain inspectable final
/// artifact inputs without returning destination or entry addresses.
#[derive(Debug, PartialEq, Eq)]
pub struct PreparedPostHandoffEntryWriterInvocation {
    lowered: LoweredPostHandoffWriter,
    context: ResolvedPostHandoffEntryWriterContext,
}

impl PreparedPostHandoffEntryWriterInvocation {
    pub const fn lowered(&self) -> &LoweredPostHandoffWriter {
        &self.lowered
    }

    pub const fn context(&self) -> &ResolvedPostHandoffEntryWriterContext {
        &self.context
    }
}

/// Lower one validated post-handoff writer into architecture-specific code.
/// The reusable helper identity and emitted bytes exclude invocation evidence;
/// the returned sibling retains exact targets and placement for provider
/// preparation.
pub fn lower_post_handoff_writer_helper(
    target: NativeTarget,
    pointer_register: MachineRegister,
    writer: &PostHandoffWriterPlan,
) -> Result<LoweredPostHandoffWriter, Diagnostic> {
    let invocation = writer
        .lower_reusable_helper()
        .map_err(|error| Diagnostic::error(error.0))?;
    let normalized_plan_fingerprint = invocation.helper.fingerprint();
    let (bytes, footprint) = match target.architecture {
        Architecture::X86_64 => (
            omega_isa_x86_64::encode_generated_post_handoff_writer_bytes(
                pointer_register,
                &invocation.helper,
            )?,
            StateFootprintEvidence::new(
                omega_isa_x86_64::generated_post_handoff_writer_clobbers(),
                omega_isa_x86_64::generated_post_handoff_writer_additional_machine_state(),
            ),
        ),
        Architecture::Aarch64 => (
            omega_isa_aarch64::encode_generated_post_handoff_writer_bytes(
                pointer_register,
                &invocation.helper,
            )?,
            StateFootprintEvidence::new(
                omega_isa_aarch64::generated_post_handoff_writer_clobbers(),
                omega_isa_aarch64::generated_post_handoff_writer_additional_machine_state(),
            ),
        ),
    };
    let emitted_bytes_fingerprint =
        emitted_writer_fingerprint(target.architecture, normalized_plan_fingerprint, &bytes);
    Ok(LoweredPostHandoffWriter {
        helper: LoweredPostHandoffWriterHelper {
            target,
            pointer_register,
            normalized_plan_fingerprint,
            emitted_bytes_fingerprint,
            bytes,
            footprint,
        },
        invocation,
    })
}

/// Lower and populate one writer as a single checked provider preparation
/// step. This rejects target/installed-artifact drift and proves that the
/// opaque packed context is the invocation sibling of the exact normalized
/// helper whose bytes and footprint will enter the final artifact.
pub fn prepare_post_handoff_entry_writer_invocation(
    target: NativeTarget,
    pointer_register: MachineRegister,
    installed: &InstalledCode,
    writer: &PostHandoffWriterPlan,
    destination_len: usize,
    destination_site: omega_layout_plans::PlacementSite,
) -> Result<PreparedPostHandoffEntryWriterInvocation, Diagnostic> {
    if installed.architecture() != target.architecture {
        return Err(Diagnostic::error(format!(
            "post-handoff writer target architecture {:?} does not match installed artifact architecture {:?}",
            target.architecture,
            installed.architecture()
        )));
    }
    let lowered = lower_post_handoff_writer_helper(target, pointer_register, writer)?;
    let context = installed
        .populate_post_handoff_entry_writer_context(writer, destination_len, destination_site)
        .map_err(|error| Diagnostic::error(error.0))?;
    if !context.binds_invocation(&lowered.invocation)
        || context.normalized_helper_fingerprint() != lowered.helper.normalized_plan_fingerprint
    {
        return Err(Diagnostic::error(
            "provider context does not bind the exact lowered post-handoff writer invocation",
        ));
    }
    Ok(PreparedPostHandoffEntryWriterInvocation { lowered, context })
}

fn emitted_writer_fingerprint(
    architecture: Architecture,
    normalized_plan_fingerprint: u64,
    bytes: &[u8],
) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    let mut mix_bytes = |values: &[u8]| {
        for value in values {
            hash ^= u64::from(*value);
            hash = hash.wrapping_mul(0x100000001b3);
        }
    };
    mix_bytes(b"omega.post-handoff-writer-emission.v1");
    mix_bytes(&[match architecture {
        Architecture::X86_64 => 0,
        Architecture::Aarch64 => 1,
    }]);
    mix_bytes(&normalized_plan_fingerprint.to_le_bytes());
    mix_bytes(&(bytes.len() as u64).to_le_bytes());
    mix_bytes(bytes);
    if hash == 0 { 1 } else { hash }
}

#[cfg(test)]
mod tests {
    use super::*;
    use omega_layout_plans::{
        ByteOrder, DataSymbolId, EntryStubId, MaterializationWrite, PlacementConstraints,
        PlacementPhase, PostHandoffWriterSource, PostHandoffWriterStep, RelocationTarget,
    };

    fn writer(target: RelocationTarget) -> PostHandoffWriterPlan {
        PostHandoffWriterPlan {
            byte_len: 16,
            byte_order: ByteOrder::LittleEndian,
            placement: PlacementConstraints::unconstrained(PlacementPhase::PostHandoff),
            steps: vec![
                PostHandoffWriterStep {
                    write: MaterializationWrite {
                        field: "target".into(),
                        target,
                        container_byte_offset: 0,
                        container_width_bits: 32,
                        destination_lsb: 0,
                        source_lsb: 0,
                        width: 16,
                    },
                    source: PostHandoffWriterSource::Resolve(target),
                },
                PostHandoffWriterStep {
                    write: MaterializationWrite {
                        field: "target".into(),
                        target,
                        container_byte_offset: 8,
                        container_width_bits: 64,
                        destination_lsb: 16,
                        source_lsb: 16,
                        width: 48,
                    },
                    source: PostHandoffWriterSource::Resolve(target),
                },
            ],
        }
    }

    #[test]
    fn one_generic_helper_shape_lowers_entry_and_data_sources_on_both_targets() {
        let entry =
            RelocationTarget::Entry(EntryStubId::from_normalized_identity(7).expect("entry"));
        let data = RelocationTarget::Data(DataSymbolId::from_normalized_identity(9).expect("data"));

        for (target, pointer) in [
            (NativeTarget::linux_x64(), MachineRegister::X86Rdi),
            (NativeTarget::linux_arm64(), MachineRegister::Aarch64X(0)),
        ] {
            let entry_lowered = lower_post_handoff_writer_helper(target, pointer, &writer(entry))
                .expect("entry helper");
            let data_lowered = lower_post_handoff_writer_helper(target, pointer, &writer(data))
                .expect("data helper");

            assert!(!entry_lowered.helper.bytes.is_empty());
            assert_eq!(
                entry_lowered.helper.bytes, data_lowered.helper.bytes,
                "source kind and identity are invocation evidence, not emitted geometry"
            );
            assert_eq!(
                entry_lowered.helper.normalized_plan_fingerprint,
                data_lowered.helper.normalized_plan_fingerprint
            );
            assert_eq!(
                entry_lowered.helper.emitted_bytes_fingerprint,
                data_lowered.helper.emitted_bytes_fingerprint
            );
            assert_ne!(
                entry_lowered.invocation.sources,
                data_lowered.invocation.sources
            );
        }
    }

    #[test]
    fn architecture_specific_bytes_retain_one_normalized_helper_identity() {
        let target =
            RelocationTarget::Entry(EntryStubId::from_normalized_identity(7).expect("entry"));
        let x86 = lower_post_handoff_writer_helper(
            NativeTarget::linux_x64(),
            MachineRegister::X86Rdi,
            &writer(target),
        )
        .expect("x86 helper");
        let arm = lower_post_handoff_writer_helper(
            NativeTarget::linux_arm64(),
            MachineRegister::Aarch64X(0),
            &writer(target),
        )
        .expect("AArch64 helper");

        assert_eq!(
            x86.helper.normalized_plan_fingerprint,
            arm.helper.normalized_plan_fingerprint
        );
        assert_ne!(x86.helper.bytes, arm.helper.bytes);
        assert_ne!(
            x86.helper.emitted_bytes_fingerprint,
            arm.helper.emitted_bytes_fingerprint
        );
    }
}
