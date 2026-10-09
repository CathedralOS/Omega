//! Target lowering for reusable post-handoff writer fragments.
//!
//! This seam consumes only normalized layout-writer geometry. Consumer
//! semantics, symbolic identities, exact placement, resolver authority, and
//! content remain in the separate invocation plan.

use calling_conventions::{MachineRegister, StateFootprintEvidence};
use diagnostics::Diagnostic;
use executable_installation::{
    InstalledCode, ResolvedPostHandoffEntryWriterContext,
    ValidatedPreparedPostHandoffWriterDestination,
};
use post_handoff_writer_plan::{PostHandoffWriterInvocationPlan, PostHandoffWriterPlan};
use target::{Architecture, NativeTarget};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoweredPostHandoffWriterFragment {
    target: NativeTarget,
    pointer_register: MachineRegister,
    normalized_plan_report_fingerprint: u64,
    emitted_bytes_report_fingerprint: u64,
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

    pub const fn normalized_plan_report_fingerprint(&self) -> u64 {
        self.normalized_plan_report_fingerprint
    }

    pub const fn emitted_bytes_report_fingerprint(&self) -> u64 {
        self.emitted_bytes_report_fingerprint
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
#[derive(Debug)]
pub struct PreparedPostHandoffEntryWriterInvocation<'mapping, 'bytes> {
    lowered: LoweredPostHandoffWriter,
    context: ResolvedPostHandoffEntryWriterContext,
    destination: ValidatedPreparedPostHandoffWriterDestination<'mapping, 'bytes>,
}

/// Failed binding returns the exact lowered writer evidence so callers may
/// inspect it or retry against corrected installed/provider facts without
/// regenerating bytes or invocation identity.
#[derive(Debug)]
pub struct PostHandoffEntryWriterBindingError<'mapping, 'bytes> {
    lowered: LoweredPostHandoffWriter,
    destination: ValidatedPreparedPostHandoffWriterDestination<'mapping, 'bytes>,
    diagnostic: Diagnostic,
}

impl<'mapping, 'bytes> PostHandoffEntryWriterBindingError<'mapping, 'bytes> {
    pub const fn diagnostic(&self) -> &Diagnostic {
        &self.diagnostic
    }

    pub fn into_parts(
        self,
    ) -> (
        LoweredPostHandoffWriter,
        ValidatedPreparedPostHandoffWriterDestination<'mapping, 'bytes>,
        Diagnostic,
    ) {
        (self.lowered, self.destination, self.diagnostic)
    }
}

impl<'mapping, 'bytes> PreparedPostHandoffEntryWriterInvocation<'mapping, 'bytes> {
    pub const fn lowered(&self) -> &LoweredPostHandoffWriter {
        &self.lowered
    }

    pub const fn context(&self) -> &ResolvedPostHandoffEntryWriterContext {
        &self.context
    }

    pub const fn destination(
        &self,
    ) -> &ValidatedPreparedPostHandoffWriterDestination<'mapping, 'bytes> {
        &self.destination
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
    let normalized_plan_report_fingerprint = invocation.fragment().report_fingerprint();
    let (bytes, footprint) = match target.architecture {
        Architecture::X86_64 => (
            isa_x86_64::encode_generated_post_handoff_writer_bytes(
                pointer_register,
                invocation.fragment(),
            )?,
            StateFootprintEvidence::new(
                isa_x86_64::generated_post_handoff_writer_clobbers(),
                isa_x86_64::generated_post_handoff_writer_additional_machine_state(),
            ),
        ),
        Architecture::Aarch64 => (
            isa_aarch64::encode_generated_post_handoff_writer_bytes(
                pointer_register,
                invocation.fragment(),
            )?,
            StateFootprintEvidence::new(
                isa_aarch64::generated_post_handoff_writer_clobbers(),
                isa_aarch64::generated_post_handoff_writer_additional_machine_state(),
            ),
        ),
    };
    let emitted_bytes_report_fingerprint = emitted_writer_report_fingerprint(
        target.architecture,
        normalized_plan_report_fingerprint,
        &bytes,
    );
    Ok(LoweredPostHandoffWriter {
        fragment: LoweredPostHandoffWriterFragment {
            target,
            pointer_register,
            normalized_plan_report_fingerprint,
            emitted_bytes_report_fingerprint,
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
pub fn bind_post_handoff_entry_writer_invocation<'mapping, 'bytes>(
    lowered: LoweredPostHandoffWriter,
    installed: &InstalledCode,
    writer: &PostHandoffWriterPlan,
    destination: ValidatedPreparedPostHandoffWriterDestination<'mapping, 'bytes>,
) -> Result<
    PreparedPostHandoffEntryWriterInvocation<'mapping, 'bytes>,
    PostHandoffEntryWriterBindingError<'mapping, 'bytes>,
> {
    let lowered =
        match preflight_post_handoff_entry_writer_binding(lowered, installed.architecture()) {
            Ok(lowered) => lowered,
            Err(error) => {
                let (lowered, diagnostic) = error.into_parts();
                return Err(PostHandoffEntryWriterBindingError {
                    lowered,
                    destination,
                    diagnostic,
                });
            }
        };
    let destination_len = destination.len();
    let destination_site = destination.site();
    let context = match installed.populate_post_handoff_entry_writer_context(
        writer,
        destination_len,
        destination_site,
    ) {
        Ok(context) => context,
        Err(error) => {
            return Err(PostHandoffEntryWriterBindingError {
                lowered,
                destination,
                diagnostic: Diagnostic::error(error.0),
            });
        }
    };
    if !context.binds_invocation(&lowered.invocation)
        || context.normalized_fragment_report_fingerprint()
            != lowered.fragment.normalized_plan_report_fingerprint
    {
        return Err(PostHandoffEntryWriterBindingError {
            lowered,
            destination,
            diagnostic: Diagnostic::error(
                "provider context does not bind the exact lowered post-handoff writer invocation",
            ),
        });
    }
    Ok(PreparedPostHandoffEntryWriterInvocation {
        lowered,
        context,
        destination,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PostHandoffEntryWriterPreflightError {
    lowered: LoweredPostHandoffWriter,
    diagnostic: Diagnostic,
}

impl PostHandoffEntryWriterPreflightError {
    fn into_parts(self) -> (LoweredPostHandoffWriter, Diagnostic) {
        (self.lowered, self.diagnostic)
    }
}

fn preflight_post_handoff_entry_writer_binding(
    lowered: LoweredPostHandoffWriter,
    installed_architecture: Architecture,
) -> Result<LoweredPostHandoffWriter, PostHandoffEntryWriterPreflightError> {
    let rejection = |lowered, diagnostic| PostHandoffEntryWriterPreflightError {
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
    if let Err(diagnostic) = validate_lowered_post_handoff_writer(&lowered) {
        return Err(PostHandoffEntryWriterPreflightError {
            lowered,
            diagnostic,
        });
    }
    Ok(lowered)
}

/// Independently replay one lowered reusable writer without consuming it.
/// This is the final-artifact/orchestration consumer check for the exact
/// invocation, canonical target bytes, footprint, and emitted identity.
pub fn validate_lowered_post_handoff_writer(
    lowered: &LoweredPostHandoffWriter,
) -> Result<(), Diagnostic> {
    lowered
        .invocation
        .validate_structure()
        .map_err(|error| Diagnostic::error(error.0))?;
    if lowered.fragment.normalized_plan_report_fingerprint
        != lowered.invocation.fragment().report_fingerprint()
    {
        return Err(Diagnostic::error(
            "lowered post-handoff writer normalized identity does not match its retained invocation",
        ));
    }

    let (expected_bytes, expected_footprint) = match lowered.fragment.target.architecture {
        Architecture::X86_64 => (
            isa_x86_64::encode_generated_post_handoff_writer_bytes(
                lowered.fragment.pointer_register,
                lowered.invocation.fragment(),
            ),
            StateFootprintEvidence::new(
                isa_x86_64::generated_post_handoff_writer_clobbers(),
                isa_x86_64::generated_post_handoff_writer_additional_machine_state(),
            ),
        ),
        Architecture::Aarch64 => (
            isa_aarch64::encode_generated_post_handoff_writer_bytes(
                lowered.fragment.pointer_register,
                lowered.invocation.fragment(),
            ),
            StateFootprintEvidence::new(
                isa_aarch64::generated_post_handoff_writer_clobbers(),
                isa_aarch64::generated_post_handoff_writer_additional_machine_state(),
            ),
        ),
    };
    let expected_bytes = expected_bytes?;
    let expected_emitted_report_fingerprint = emitted_writer_report_fingerprint(
        lowered.fragment.target.architecture,
        lowered.fragment.normalized_plan_report_fingerprint,
        &lowered.fragment.bytes,
    );
    if lowered.fragment.bytes != expected_bytes
        || lowered.fragment.footprint != expected_footprint
        || lowered.fragment.emitted_bytes_report_fingerprint != expected_emitted_report_fingerprint
    {
        return Err(Diagnostic::error(
            "lowered post-handoff writer bytes, footprint, or emitted report fingerprint fail exact replay",
        ));
    }
    Ok(())
}

fn emitted_writer_report_fingerprint(
    architecture: Architecture,
    normalized_plan_report_fingerprint: u64,
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
    mix_bytes(&normalized_plan_report_fingerprint.to_le_bytes());
    mix_bytes(&(bytes.len() as u64).to_le_bytes());
    mix_bytes(bytes);
    if hash == 0 { 1 } else { hash }
}
