//! Explicit host bindings for exact admitted native host operation slots.
//! A slot's realization selects semantics; its complete declaration selects
//! the typed adapter. Every family's offer first passes the shared
//! `admit_exact_foreign_call` premise; the family then owns its own shape
//! rules. No source-name dispatch or stdout-driven exit exists.
//! Output stays buffered; input consumes only the caller's finite replay stream
//! with an instance-private cursor. Checked compound operations remain guest code.

/// Every hosted import adapter reports its declared signature the same
/// way: cloning the `signature` the constructor recorded. One row per
/// adapter expands that impl.
macro_rules! import_signature_via_clone {
    ($name:ty) => {
        impl TerminalImportAdapter for $name {
            fn implemented_signature(&self) -> TerminalImportSignature {
                self.signature.clone()
            }
        }
    };
}

mod absolute_value;
mod allocation;
mod borrowed_storage;
mod buffered_output;
pub(super) mod clock;
mod descriptor_io;
mod descriptor_mode;
mod native_word;
mod network_address;
mod numeric;
mod standard_input;

use native_artifact::TerminalRuntimeHostProvisions;
use semantic_vocabulary::{BoundaryMachineId, IntegerSign, ScalarType};
use target_operations::{BoundaryRealization, BoundarySettlementRealization};
use terminal_interpreter::{TerminalImportSignature, TerminalInstance, TerminalProgram};
use terminal_psi::BoundaryCompletion;

use super::TerminalRuntimeError;

#[derive(Clone, Copy)]
enum OutputKind {
    Line,
    Byte,
}

/// Shared admission premise for every exact foreign call binding before any
/// trusted host adapter is offered for it: an identified provider-plan report,
/// a valid entry mechanism, no callback materializations, no host-supplied
/// residue obligations, normal completion, and an evaluated boundary-entry
/// plan that validates the native signature the adapter will actually run.
/// Each family below keeps its own declaration/ABI shape rules; this one rule
/// gates every offer, so one operation carries it instead of five copies.
pub(super) fn admit_exact_foreign_call(
    signature: &TerminalImportSignature,
    declaration: &terminal_psi::BoundaryMachineDeclaration,
    binding: &target_operations::NormalizedForeignCallBinding,
    native_signature: &calling_conventions::CallSignature,
) -> Result<(), TerminalRuntimeError> {
    if !binding.has_valid_entry_mechanism() {
        return Err(TerminalRuntimeError::InvalidProvision(
            "foreign call binding has no valid entry mechanism",
        ));
    }
    if binding.provider_plan_report_identity() == 0 {
        return Err(TerminalRuntimeError::InvalidProvision(
            "foreign call binding identifies no provider-plan report",
        ));
    }
    if !binding
        .boundary_entry_plan
        .call
        .callback_materializations
        .is_empty()
    {
        return Err(TerminalRuntimeError::InvalidProvision(
            "foreign call binding retains callback materializations",
        ));
    }
    // Structural requirements are caller-carried qualifications. The ordinary
    // boundary argument binder checks them before invoking any host effect;
    // admitting an adapter neither supplies nor re-establishes that evidence.
    // Keep unsupported proof/output residue on the refusal path below.
    if !declaration.scalar_requires.is_empty()
        || !declaration.scalar_ensures.is_empty()
        || !declaration.content_guarantees.is_empty()
        || !declaration.program_local_root_introductions.is_empty()
        || !declaration.crash_routes.is_empty()
    {
        return Err(TerminalRuntimeError::InvalidProvision(
            "foreign declaration retains residue obligations this runtime does not supply",
        ));
    }
    if signature.completion != terminal_psi::BoundaryCompletion::NormalReturn {
        return Err(TerminalRuntimeError::InvalidProvision(
            "foreign call declaration does not complete with a normal return",
        ));
    }
    calling_conventions::validate_boundary_entry_plan(
        binding.boundary_entry_plan.clone(),
        native_signature,
    )
    .map_err(|diagnostic| {
        TerminalRuntimeError::missing(
            "runtime-host-foreign-call-abi",
            format!("foreign boundary-entry plan rejected: {diagnostic}"),
        )
    })?;
    Ok(())
}

/// One explicitly supplied host capability root shared by the receiver and
/// real component instances of this invocation. Tokens and stream cursors keep
/// their same owned stores across a checked cross-artifact call.
pub(super) struct HostContext<'host> {
    output: buffered_output::BufferedStreams,
    input: Option<std::sync::Arc<std::sync::Mutex<Box<dyn std::io::Read + Send + 'host>>>>,
    clock: Option<std::sync::Arc<std::sync::Mutex<clock::TerminalClockSnapshot>>>,
    addresses: std::sync::Arc<std::sync::Mutex<network_address::AddressValues>>,
    allocations: std::sync::Arc<std::sync::Mutex<allocation::AllocationValues>>,
}
impl<'host> HostContext<'host> {
    pub(super) fn new(
        input: Option<Box<dyn std::io::Read + Send + 'host>>,
        clock: Option<clock::TerminalClockSnapshot>,
    ) -> Self {
        Self {
            output: buffered_output::BufferedStreams::new(),
            input: input.map(|reader| std::sync::Arc::new(std::sync::Mutex::new(reader))),
            clock: clock.map(|snapshot| std::sync::Arc::new(std::sync::Mutex::new(snapshot))),
            addresses: std::sync::Arc::new(std::sync::Mutex::new(
                network_address::AddressValues::default(),
            )),
            allocations: std::sync::Arc::new(std::sync::Mutex::new(
                allocation::AllocationValues::default(),
            )),
        }
    }
}

pub(super) trait HostBindingTarget<'host> {
    fn import(&self, slot: BoundaryMachineId) -> Option<&terminal_psi::BoundaryMachineDeclaration>;
    fn bind_import(
        &mut self,
        slot: BoundaryMachineId,
        adapter: impl terminal_interpreter::TerminalImportAdapter + 'host,
    ) -> Result<(), TerminalRuntimeError>;
}
impl<'host> HostBindingTarget<'host> for TerminalInstance<'host, '_> {
    fn import(&self, slot: BoundaryMachineId) -> Option<&terminal_psi::BoundaryMachineDeclaration> {
        self.import(slot)
    }
    fn bind_import(
        &mut self,
        slot: BoundaryMachineId,
        adapter: impl terminal_interpreter::TerminalImportAdapter + 'host,
    ) -> Result<(), TerminalRuntimeError> {
        self.bind_import(slot, adapter)
            .map_err(TerminalRuntimeError::Binding)
    }
}
impl<'host> HostBindingTarget<'host> for terminal_interpreter::TerminalComponentBinding<'host, '_> {
    fn import(&self, slot: BoundaryMachineId) -> Option<&terminal_psi::BoundaryMachineDeclaration> {
        self.import(slot)
    }
    fn bind_import(
        &mut self,
        slot: BoundaryMachineId,
        adapter: impl terminal_interpreter::TerminalImportAdapter + 'host,
    ) -> Result<(), TerminalRuntimeError> {
        self.bind_import(slot, adapter)
            .map_err(TerminalRuntimeError::ComponentBinding)
    }
}

pub(super) fn bind<'host>(
    program: &TerminalProgram,
    instance: &mut impl HostBindingTarget<'host>,
    provisions: Option<&TerminalRuntimeHostProvisions>,
    imports: &[BoundaryMachineId],
    context: &HostContext<'host>,
) -> Result<buffered_output::BufferedStreams, TerminalRuntimeError> {
    let output = context.output.clone();
    let input = &context.input;
    let clock = &context.clock;
    let addresses = &context.addresses;
    let allocations = &context.allocations;
    for slot in imports {
        let provision = provisions
            .and_then(|provisions| {
                provisions
                    .host()
                    .iter()
                    .find(|provision| provision.boundary == *slot)
            })
            .ok_or_else(|| {
                TerminalRuntimeError::missing(
                    "runtime-host-import-bindings",
                    format!("slot {slot:?} has no admitted host realization"),
                )
            })?;
        let kind = match provision.realization {
            BoundarySettlementRealization::Builtin(BoundaryRealization::HostedReadByte(_)) => {
                let declaration =
                    instance
                        .import(*slot)
                        .ok_or(TerminalRuntimeError::InvalidProvision(
                            "stdin host provision names no demanded import",
                        ))?;
                let reader = input.as_ref().ok_or_else(|| {
                    TerminalRuntimeError::missing(
                        "runtime-standard-input",
                        "no finite replay stream was supplied",
                    )
                })?;
                let adapter = standard_input::ReadByteImport::for_import(
                    program,
                    declaration,
                    std::sync::Arc::clone(reader),
                )?;
                instance.bind_import(*slot, adapter)?;
                continue;
            }
            BoundarySettlementRealization::Builtin(BoundaryRealization::LinuxWriteLine(_)) => {
                OutputKind::Line
            }
            BoundarySettlementRealization::Builtin(BoundaryRealization::HostedWriteByteI32(_)) => {
                OutputKind::Byte
            }
            BoundarySettlementRealization::Builtin(
                BoundaryRealization::NearestIeeeFloatArithmetic(realization),
            ) => {
                let declaration =
                    instance
                        .import(*slot)
                        .ok_or(TerminalRuntimeError::InvalidProvision(
                            "numeric host provision names no demanded import",
                        ))?;
                let adapter =
                    numeric::NumericImport::for_import(program, declaration, realization)?;
                instance.bind_import(*slot, adapter)?;
                continue;
            }
            BoundarySettlementRealization::NormalizedForeignCall(ref binding) => {
                let declaration =
                    instance
                        .import(*slot)
                        .ok_or(TerminalRuntimeError::InvalidProvision(
                            "foreign host provision names no demanded import",
                        ))?;
                // The occurrence keeps its portable contract while the foreign
                // binding names the selected row. Rejoin their retained exact
                // association once before either host implementation is offered.
                if !provisions.is_some_and(|provisions| {
                    provisions.matches_host_requirement(
                        declaration,
                        binding
                            .requirement_identity(provisions.selection_identities())
                            .unwrap_or_default(),
                        binding.provider_plan_report_identity(),
                        &program.module().identities,
                    )
                }) {
                    return Err(TerminalRuntimeError::InvalidProvision(
                        "foreign host binding does not match its admitted requirement row",
                    ));
                }
                if let Some(adapter) = absolute_value::AbsoluteValueImport::admit(
                    program,
                    declaration,
                    binding,
                    provisions.and_then(|provisions| provisions.host_calling_application(*slot)),
                )? {
                    instance.bind_import(*slot, adapter)?;
                    continue;
                }
                if let Some(adapter) = allocation::AllocationImport::admit(
                    program,
                    declaration,
                    binding,
                    provisions.and_then(|provisions| provisions.host_calling_application(*slot)),
                    std::sync::Arc::clone(allocations),
                )? {
                    instance.bind_import(*slot, adapter)?;
                    continue;
                }
                if let Some(adapter) = clock::ClockImport::admit(
                    declaration,
                    binding,
                    provisions.map_or(&[], TerminalRuntimeHostProvisions::borrowed_storage_layouts),
                    clock.as_ref().cloned(),
                )? {
                    instance.bind_import(*slot, adapter)?;
                    continue;
                }
                if let Some(adapter) = descriptor_io::DescriptorIoImport::admit(
                    program,
                    declaration,
                    binding,
                    provisions.map_or(&[], TerminalRuntimeHostProvisions::borrowed_storage_layouts),
                    input.as_ref().cloned(),
                    output.clone(),
                )? {
                    instance.bind_import(*slot, adapter)?;
                    continue;
                }
                if let Some(adapter) = network_address::NetworkAddressImport::admit(
                    declaration,
                    binding,
                    provisions.and_then(|provisions| provisions.host_calling_application(*slot)),
                    std::sync::Arc::clone(&addresses),
                )? {
                    instance.bind_import(*slot, adapter)?;
                    continue;
                }
                let Some(adapter) =
                    descriptor_mode::DescriptorModeImport::admit(declaration, binding)?
                else {
                    return Err(TerminalRuntimeError::missing(
                        "runtime-host-import-bindings",
                        "no trusted host implementation matches this exact foreign binding",
                    ));
                };
                instance.bind_import(*slot, adapter)?;
                continue;
            }
            _ => {
                return Err(TerminalRuntimeError::missing(
                    "runtime-host-import-bindings",
                    format!(
                        "slot {slot:?} needs a runtime adapter for {:?}",
                        provision.realization
                    ),
                ));
            }
        };
        let declaration = instance
            .import(*slot)
            .ok_or(TerminalRuntimeError::InvalidProvision(
                "host provision names no demanded import declaration",
            ))?;
        let signature = TerminalImportSignature::from(declaration);
        let valid = signature.completion == BoundaryCompletion::NormalReturn
            && signature.result.is_unit()
            && match kind {
                OutputKind::Line => {
                    signature.scalar_parameters.is_empty()
                        && signature.structural_parameters.len() == 1
                }
                OutputKind::Byte => {
                    signature.structural_parameters.is_empty()
                        && matches!(signature.scalar_parameters.as_slice(),
                [ScalarType::Integer(integer)] if integer.sign() == IntegerSign::Signed && integer.bits() == 32)
                }
            };
        if !valid {
            return Err(TerminalRuntimeError::InvalidProvision(
                "buffered host operation differs from its admitted declaration",
            ));
        }
        instance.bind_import(
            *slot,
            buffered_output::BufferedOutput::new(signature, kind, output.stdout.clone()),
        )?;
    }
    Ok(output)
}
