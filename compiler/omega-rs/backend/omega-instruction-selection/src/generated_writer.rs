//! Target lowering for reusable post-handoff writer fragments.
//!
//! This seam consumes only normalized layout-writer geometry. Consumer
//! semantics, symbolic identities, exact placement, resolver authority, and
//! content remain in the separate invocation plan.

use omega_calling_conventions::{MachineRegister, StateFootprintEvidence};
use omega_executable_installation::{InstalledCode, ResolvedPostHandoffEntryWriterContext};
use omega_target::{Architecture, NativeTarget};
use psi_diagnostics::Diagnostic;
use psi_layout_plans::{PostHandoffWriterInvocationPlan, PostHandoffWriterPlan};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoweredPostHandoffWriterFragment {
    target: NativeTarget,
    pointer_register: MachineRegister,
    normalized_plan_fingerprint: u64,
    emitted_bytes_fingerprint: u64,
    bytes: Vec<u8>,
    footprint: StateFootprintEvidence,
}

impl LoweredPostHandoffWriterFragment {
    pub const fn target(&self) -> NativeTarget {
        self.target
    }

    pub const fn pointer_register(&self) -> MachineRegister {
        self.pointer_register
    }

    pub const fn normalized_plan_fingerprint(&self) -> u64 {
        self.normalized_plan_fingerprint
    }

    pub const fn emitted_bytes_fingerprint(&self) -> u64 {
        self.emitted_bytes_fingerprint
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub const fn footprint(&self) -> &StateFootprintEvidence {
        &self.footprint
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoweredPostHandoffWriter {
    fragment: LoweredPostHandoffWriterFragment,
    invocation: PostHandoffWriterInvocationPlan,
}

impl LoweredPostHandoffWriter {
    pub const fn fragment(&self) -> &LoweredPostHandoffWriterFragment {
        &self.fragment
    }

    pub const fn invocation(&self) -> &PostHandoffWriterInvocationPlan {
        &self.invocation
    }
}

/// One exact provider invocation paired with the reusable fragment bytes that
/// consume it. The installed-code resolver owns the sealed packed words;
/// fragment code, target identity, and exact footprint remain inspectable final
/// artifact inputs without returning destination or entry addresses.
#[derive(Debug, PartialEq, Eq)]
pub struct PreparedPostHandoffEntryWriterInvocation {
    lowered: LoweredPostHandoffWriter,
    context: ResolvedPostHandoffEntryWriterContext,
}

/// Failed binding returns the exact lowered writer evidence so callers may
/// inspect it or retry against corrected installed/provider facts without
/// regenerating bytes or invocation identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PostHandoffEntryWriterBindingError {
    lowered: LoweredPostHandoffWriter,
    diagnostic: Diagnostic,
}

impl PostHandoffEntryWriterBindingError {
    pub const fn diagnostic(&self) -> &Diagnostic {
        &self.diagnostic
    }

    pub fn into_parts(self) -> (LoweredPostHandoffWriter, Diagnostic) {
        (self.lowered, self.diagnostic)
    }
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
/// The reusable fragment identity and emitted bytes exclude invocation evidence;
/// the returned sibling retains exact targets and placement for provider
/// preparation.
pub fn lower_post_handoff_writer_fragment(
    target: NativeTarget,
    pointer_register: MachineRegister,
    writer: &PostHandoffWriterPlan,
) -> Result<LoweredPostHandoffWriter, Diagnostic> {
    let invocation = writer
        .lower_reusable_fragment()
        .map_err(|error| Diagnostic::error(error.0))?;
    invocation
        .validate_structure()
        .map_err(|error| Diagnostic::error(error.0))?;
    let normalized_plan_fingerprint = invocation.fragment().fingerprint();
    let (bytes, footprint) = match target.architecture {
        Architecture::X86_64 => (
            omega_isa_x86_64::encode_generated_post_handoff_writer_bytes(
                pointer_register,
                invocation.fragment(),
            )?,
            StateFootprintEvidence::new(
                omega_isa_x86_64::generated_post_handoff_writer_clobbers(),
                omega_isa_x86_64::generated_post_handoff_writer_additional_machine_state(),
            ),
        ),
        Architecture::Aarch64 => (
            omega_isa_aarch64::encode_generated_post_handoff_writer_bytes(
                pointer_register,
                invocation.fragment(),
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
        fragment: LoweredPostHandoffWriterFragment {
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

/// Bind one already-lowered AOT writer fragment to its provider-populated
/// invocation context. Provider preparation never generates host code. This
/// rejects target/installed-artifact drift and proves that the opaque packed
/// context is the invocation sibling of the exact normalized fragment whose
/// bytes and footprint entered the final artifact.
pub fn bind_post_handoff_entry_writer_invocation(
    lowered: LoweredPostHandoffWriter,
    installed: &InstalledCode,
    writer: &PostHandoffWriterPlan,
    destination_len: usize,
    destination_site: psi_layout_plans::PlacementSite,
) -> Result<PreparedPostHandoffEntryWriterInvocation, PostHandoffEntryWriterBindingError> {
    let lowered = preflight_post_handoff_entry_writer_binding(lowered, installed.architecture())?;
    let context = match installed.populate_post_handoff_entry_writer_context(
        writer,
        destination_len,
        destination_site,
    ) {
        Ok(context) => context,
        Err(error) => {
            return Err(PostHandoffEntryWriterBindingError {
                lowered,
                diagnostic: Diagnostic::error(error.0),
            });
        }
    };
    if !context.binds_invocation(&lowered.invocation)
        || context.normalized_fragment_fingerprint() != lowered.fragment.normalized_plan_fingerprint
    {
        return Err(PostHandoffEntryWriterBindingError {
            lowered,
            diagnostic: Diagnostic::error(
                "provider context does not bind the exact lowered post-handoff writer invocation",
            ),
        });
    }
    Ok(PreparedPostHandoffEntryWriterInvocation { lowered, context })
}

fn preflight_post_handoff_entry_writer_binding(
    lowered: LoweredPostHandoffWriter,
    installed_architecture: Architecture,
) -> Result<LoweredPostHandoffWriter, PostHandoffEntryWriterBindingError> {
    let rejection = |lowered, diagnostic| PostHandoffEntryWriterBindingError {
        lowered,
        diagnostic: Diagnostic::error(diagnostic),
    };
    let target_architecture = lowered.fragment.target.architecture;
    if installed_architecture != target_architecture {
        return Err(rejection(
            lowered,
            format!(
                "post-handoff writer target architecture {:?} does not match installed artifact architecture {installed_architecture:?}",
                target_architecture,
            ),
        ));
    }
    if let Err(error) = lowered.invocation.validate_structure() {
        return Err(rejection(lowered, error.0));
    }
    if lowered.fragment.normalized_plan_fingerprint != lowered.invocation.fragment().fingerprint() {
        return Err(rejection(
            lowered,
            "lowered post-handoff writer normalized identity does not match its retained invocation"
                .into(),
        ));
    }

    let (expected_bytes, expected_footprint) = match lowered.fragment.target.architecture {
        Architecture::X86_64 => (
            omega_isa_x86_64::encode_generated_post_handoff_writer_bytes(
                lowered.fragment.pointer_register,
                lowered.invocation.fragment(),
            ),
            StateFootprintEvidence::new(
                omega_isa_x86_64::generated_post_handoff_writer_clobbers(),
                omega_isa_x86_64::generated_post_handoff_writer_additional_machine_state(),
            ),
        ),
        Architecture::Aarch64 => (
            omega_isa_aarch64::encode_generated_post_handoff_writer_bytes(
                lowered.fragment.pointer_register,
                lowered.invocation.fragment(),
            ),
            StateFootprintEvidence::new(
                omega_isa_aarch64::generated_post_handoff_writer_clobbers(),
                omega_isa_aarch64::generated_post_handoff_writer_additional_machine_state(),
            ),
        ),
    };
    let expected_bytes = match expected_bytes {
        Ok(bytes) => bytes,
        Err(diagnostic) => {
            return Err(PostHandoffEntryWriterBindingError {
                lowered,
                diagnostic,
            });
        }
    };
    let expected_emitted_fingerprint = emitted_writer_fingerprint(
        lowered.fragment.target.architecture,
        lowered.fragment.normalized_plan_fingerprint,
        &lowered.fragment.bytes,
    );
    if lowered.fragment.bytes != expected_bytes
        || lowered.fragment.footprint != expected_footprint
        || lowered.fragment.emitted_bytes_fingerprint != expected_emitted_fingerprint
    {
        return Err(rejection(
            lowered,
            "lowered post-handoff writer bytes, footprint, or emitted fingerprint fail exact replay"
                .into(),
        ));
    }
    Ok(lowered)
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
    use psi_layout_plans::{
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
                        stored_integer_fit: None,
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
                        stored_integer_fit: None,
                    },
                    source: PostHandoffWriterSource::Resolve(target),
                },
            ],
        }
    }

    #[test]
    fn one_generic_fragment_shape_lowers_entry_and_data_sources_on_both_targets() {
        let entry =
            RelocationTarget::Entry(EntryStubId::from_normalized_identity(7).expect("entry"));
        let data = RelocationTarget::Data(DataSymbolId::from_normalized_identity(9).expect("data"));

        for (target, pointer) in [
            (NativeTarget::linux_x64(), MachineRegister::X86Rdi),
            (NativeTarget::linux_arm64(), MachineRegister::Aarch64X(0)),
        ] {
            let entry_lowered = lower_post_handoff_writer_fragment(target, pointer, &writer(entry))
                .expect("entry fragment");
            let data_lowered = lower_post_handoff_writer_fragment(target, pointer, &writer(data))
                .expect("data fragment");

            assert!(!entry_lowered.fragment().bytes().is_empty());
            assert_eq!(
                entry_lowered.fragment().bytes(),
                data_lowered.fragment().bytes(),
                "source kind and identity are invocation evidence, not emitted geometry"
            );
            assert_eq!(
                entry_lowered.fragment().normalized_plan_fingerprint(),
                data_lowered.fragment().normalized_plan_fingerprint()
            );
            assert_eq!(
                entry_lowered.fragment().emitted_bytes_fingerprint(),
                data_lowered.fragment().emitted_bytes_fingerprint()
            );
            assert_ne!(
                entry_lowered.invocation().sources(),
                data_lowered.invocation().sources()
            );
        }
    }

    #[test]
    fn architecture_specific_bytes_retain_one_normalized_fragment_identity() {
        let target =
            RelocationTarget::Entry(EntryStubId::from_normalized_identity(7).expect("entry"));
        let x86 = lower_post_handoff_writer_fragment(
            NativeTarget::linux_x64(),
            MachineRegister::X86Rdi,
            &writer(target),
        )
        .expect("x86 fragment");
        let arm = lower_post_handoff_writer_fragment(
            NativeTarget::linux_arm64(),
            MachineRegister::Aarch64X(0),
            &writer(target),
        )
        .expect("AArch64 fragment");

        assert_eq!(
            x86.fragment().normalized_plan_fingerprint(),
            arm.fragment().normalized_plan_fingerprint()
        );
        assert_ne!(x86.fragment().bytes(), arm.fragment().bytes());
        assert_ne!(
            x86.fragment().emitted_bytes_fingerprint(),
            arm.fragment().emitted_bytes_fingerprint()
        );
    }

    #[test]
    fn binding_preflight_returns_exact_lowered_evidence_for_retry() {
        let target =
            RelocationTarget::Entry(EntryStubId::from_normalized_identity(7).expect("entry"));
        let lowered = lower_post_handoff_writer_fragment(
            NativeTarget::linux_x64(),
            MachineRegister::X86Rdi,
            &writer(target),
        )
        .expect("x86 fragment");
        let expected = lowered.clone();

        let rejection = preflight_post_handoff_entry_writer_binding(lowered, Architecture::Aarch64)
            .expect_err("installed architecture drift must reject");
        assert!(rejection.diagnostic().message.contains("architecture"));
        let (lowered, diagnostic) = rejection.into_parts();
        assert!(diagnostic.message.contains("architecture"));
        assert_eq!(lowered, expected);

        let lowered = preflight_post_handoff_entry_writer_binding(lowered, Architecture::X86_64)
            .expect("the exact returned evidence remains valid for corrected retry");
        assert_eq!(lowered, expected);

        let mut corrupted = lowered;
        corrupted.fragment.bytes[0] ^= 1;
        let corrupted_snapshot = corrupted.clone();
        let rejection =
            preflight_post_handoff_entry_writer_binding(corrupted, Architecture::X86_64)
                .expect_err("emitted byte corruption must reject");
        assert!(rejection.diagnostic().message.contains("exact replay"));
        let (returned, _) = rejection.into_parts();
        assert_eq!(returned, corrupted_snapshot);
    }
}
