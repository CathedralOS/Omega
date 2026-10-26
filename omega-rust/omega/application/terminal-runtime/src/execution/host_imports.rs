//! Explicit host bindings for exact admitted native host operation slots.
//! A slot's realization selects semantics; its complete declaration selects
//! the typed adapter. Modeled services pass the shared
//! `admit_exact_foreign_call` premise and their exact shape rules. A separately
//! sponsored native provider joins the complete produced ABI and original
//! homes, with semantic completion still owned by Psi. No source-name
//! dispatch or stdout-driven exit exists.
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

mod affine_transform;
mod allocation;
mod borrowed_storage;
mod buffered_output;
pub(super) mod clock;
mod descriptor_io;
mod descriptor_mode;
mod integer_absolute;
mod integer_division;
mod native_provider;
mod native_word;
mod network_address;
mod numeric;
mod scalar_codec;
mod scalar_record;
mod standard_input;

use native_artifact::TerminalRuntimeHostProvisions;
use semantic_vocabulary::{BoundaryMachineId, IntegerSign, ScalarType};
use target::TargetProfile;
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
/// a valid entry mechanism, no callback materializations, no declaration
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
    if !declaration.requires.is_empty()
        || !declaration.scalar_requires.is_empty()
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

/// Whether this host actually supplies the selected target's runtime: another
/// host's libc/CRT supply is never borrowed for an offer.
fn host_supplies_target(target: TargetProfile) -> bool {
    match target {
        TargetProfile::LinuxX64 => cfg!(all(target_os = "linux", target_arch = "x86_64")),
        TargetProfile::LinuxArm64 => cfg!(all(target_os = "linux", target_arch = "aarch64")),
        TargetProfile::MacosArm64 => cfg!(all(target_os = "macos", target_arch = "aarch64")),
        TargetProfile::MacosX64 => cfg!(all(target_os = "macos", target_arch = "x86_64")),
        TargetProfile::WindowsX64 => cfg!(all(target_os = "windows", target_arch = "x86_64")),
        _ => false,
    }
}

/// Match one hosted C-runtime symbol against a binding's normalized locator.
///
/// How the platform spells an ordinary C symbol is target data, not operation
/// data: `libc.so.6` at the profile's GLIBC version, the Mach-O load identity,
/// or a provider DLL export. `darwin_install_names` carries the accepted
/// load-identity spellings; `windows_library` names the DLL that exports
/// `symbol` on Windows, or `None` for an operation this runtime does not
/// supply there. One table owns this identity so no operation family
/// restates it. The offer is refused — not errored — when this host supplies
/// no runtime for the selected target.
pub(super) fn matches_platform_c_symbol(
    binding: &target_operations::NormalizedForeignCallBinding,
    symbol: &[u8],
    darwin_install_names: &[&[u8]],
    windows_library: Option<&[u8]>,
) -> Result<bool, TerminalRuntimeError> {
    let Some((locator, _)) = binding.import() else {
        return Ok(false);
    };
    let target = locator.target();
    if !host_supplies_target(target) {
        return Ok(false);
    }
    let matches = |candidate| {
        target::normalize_foreign_locator(candidate, target)
            .map(|offered| offered == *locator)
            .map_err(|_| {
                TerminalRuntimeError::InvalidProvision(
                    "a hosted C symbol spelling is not a valid locator for its target",
                )
            })
    };
    match target {
        TargetProfile::LinuxX64 | TargetProfile::LinuxArm64 => {
            matches(target::ForeignLocatorCandidate::ElfVersioned {
                object: b"libc.so.6".to_vec(),
                symbol: symbol.to_vec(),
                version: if target == TargetProfile::LinuxX64 {
                    b"GLIBC_2.2.5".to_vec()
                } else {
                    b"GLIBC_2.17".to_vec()
                },
            })
        }
        TargetProfile::MacosArm64 | TargetProfile::MacosX64 => {
            let darwin_symbol = [b"_".as_slice(), symbol].concat();
            for install_name in darwin_install_names {
                if matches(target::ForeignLocatorCandidate::MachODylibSymbol {
                    install_name: install_name.to_vec(),
                    symbol: darwin_symbol.clone(),
                })? {
                    return Ok(true);
                }
            }
            Ok(false)
        }
        TargetProfile::WindowsX64 => match windows_library {
            Some(library) => matches(target::ForeignLocatorCandidate::PeByName {
                library: library.to_vec(),
                export: symbol.to_vec(),
            }),
            None => Ok(false),
        },
        _ => Ok(false),
    }
}

/// One explicitly supplied host capability root shared by the receiver and
/// real component instances of this invocation. Tokens and stream cursors keep
/// their same owned stores across a checked cross-artifact call.
pub(super) struct HostContext<'host> {
    output: buffered_output::BufferedStreams,
    input: Option<std::sync::Arc<std::sync::Mutex<Box<dyn std::io::Read + Send + 'host>>>>,
    clock: Option<std::sync::Arc<std::sync::Mutex<clock::TerminalClockSnapshot>>>,
    addresses: std::sync::Arc<std::sync::Mutex<network_address::AddressValues>>,
    native: native_provider::NativeContext,
    allocations: std::sync::Arc<std::sync::Mutex<allocation::AllocationValues>>,
}
impl<'host> HostContext<'host> {
    pub(super) fn finish_native_provider(&self) -> Result<(), TerminalRuntimeError> {
        self.native.finish().map_err(|error| {
            TerminalRuntimeError::missing("native-provider-retirement", error.reason)
        })
    }
    pub(super) fn new(
        input: Option<Box<dyn std::io::Read + Send + 'host>>,
        clock: Option<clock::TerminalClockSnapshot>,
        native_provider: Option<std::process::Command>,
    ) -> Self {
        Self {
            native: native_provider::NativeContext::new(native_provider),
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
                if let Some(adapter) = affine_transform::AffineTransformImport::admit(
                    program,
                    declaration,
                    binding,
                    provisions.and_then(|provisions| provisions.host_calling_application(*slot)),
                )? {
                    instance.bind_import(*slot, adapter)?;
                    continue;
                }
                if let Some(adapter) = integer_absolute::IntegerAbsoluteImport::admit(
                    program,
                    declaration,
                    binding,
                    provisions.and_then(|provisions| provisions.host_calling_application(*slot)),
                )? {
                    instance.bind_import(*slot, adapter)?;
                    continue;
                }
                if let Some(adapter) = integer_division::IntegerDivisionImport::admit(
                    program,
                    declaration,
                    binding,
                    provisions.and_then(|provisions| provisions.host_calling_application(*slot)),
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
                if let Some(adapter) =
                    descriptor_mode::DescriptorModeImport::admit(declaration, binding)?
                {
                    instance.bind_import(*slot, adapter)?;
                    continue;
                }
                if let Some(adapter) = native_provider::NativeImport::admit(
                    program,
                    declaration,
                    binding,
                    provisions,
                    &context.native,
                )? {
                    instance.bind_import(*slot, adapter)?;
                    continue;
                }
                return Err(TerminalRuntimeError::missing(
                    "runtime-host-import-bindings",
                    "no trusted host implementation matches this exact foreign binding",
                ));
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
