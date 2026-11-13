/*
We bind admitted native host operation slots (`bind`, `bind_admitted_intrinsics`,
`HostContext`) to typed interpreter adapters.

A slot's realization selects its semantics while its complete declaration
selects the typed adapter. Modeled services pass `admit_exact_foreign_call`
(valid entry mechanism, identified provider-plan report, no callback
materializations, no unsupported proof/output residue, normal completion, and a
validated `CallSignature`) plus their family-specific declaration/ABI shape
rules. Separately sponsored native providers (`native_provider`) join the
complete produced ABI and original homes, with semantic completion still owned
by Psi. Output stays buffered in `BufferedStreams`, and standard input consumes
only the caller's finite replay stream through an instance-private cursor.
*/

macro_rules! import_signature_via_clone {
    ($name:ty) => {
        impl TerminalImportAdapter for $name {
            fn implemented_signature(&self) -> TerminalImportSignature {
                self.signature.clone()
            }
        }
    };
}

macro_rules! handle_effect_requires_dispatch {
    ($message:literal) => {
        handle_effect_requires_dispatch!(TerminalEffectRejection::new($message));
    };
    ($rejection:expr) => {
        fn handle_effect(&mut self, _: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
            Err($rejection)
        }
    };
}

mod absolute_value;
mod affine_transform;
mod allocation;
mod borrowed_storage;
mod buffered_output;
pub(super) mod clock;
mod descriptor_io;
mod descriptor_mode;
mod filesystem;
mod integer_absolute;
mod integer_division;
mod integer_parsing;
mod native_provider;
mod native_word;
mod network_address;
mod numeric;
mod reference_state;
mod scalar_codec;
mod scalar_record;
mod standard_input;

use native_artifact::TerminalRuntimeHostProvisions;
use semantic_vocabulary::{BoundaryMachineId, IntegerSign, ScalarType};
use target::TargetProfile;
use target_operations::{BoundaryRealization, BoundarySettlementRealization};
use terminal_interpreter::{
    TerminalEffect, TerminalEffectRejection, TerminalImportSignature, TerminalInstance,
    TerminalProgram, TerminalScalarValue, TerminalStructuralValue,
};
use terminal_psi::{BoundaryCompletion, BoundaryMachineResult};

use super::TerminalRuntimeError;

pub(super) fn executes_internally(
    program: &TerminalProgram,
    provision: &native_artifact::TerminalRuntimeHostProvision,
) -> bool {
    program.is_compiler_primitive(provision.boundary)
        || matches!(
            provision.realization,
            BoundarySettlementRealization::Builtin(BoundaryRealization::AccessorScalarAccess(_))
        )
}

#[derive(Clone, Copy)]
enum OutputKind {
    Line,
    Byte,
}

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
    if !declaration.normal_qualifications.is_empty()
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

pub(super) struct CallOperands<'effect> {
    pub(super) boundary: &'effect BoundaryMachineId,
    pub(super) arguments: &'effect [TerminalScalarValue],
    pub(super) structural_arguments: &'effect [TerminalStructuralValue],
    pub(super) byte_sequence_arguments: &'effect [Option<Vec<u8>>],
    pub(super) result: &'effect BoundaryMachineResult,
}

pub(super) fn call_operands<'effect, E>(
    effect: &'effect TerminalEffect,
    refuse: impl FnOnce() -> E,
) -> Result<CallOperands<'effect>, E> {
    let TerminalEffect::BoundaryCall {
        site: terminal_interpreter::TerminalEffectWait { boundary, .. },
        arguments,
        structural_arguments,
        byte_sequence_arguments,
        result,
        ..
    } = effect
    else {
        return Err(refuse());
    };
    Ok(CallOperands {
        boundary,
        arguments,
        structural_arguments,
        byte_sequence_arguments,
        result,
    })
}

pub(super) fn refusal(message: &'static str) -> impl Fn() -> TerminalEffectRejection + Copy {
    move || TerminalEffectRejection::new(message)
}

pub(super) fn unprovisioned(message: &'static str) -> impl Fn() -> TerminalRuntimeError + Copy {
    move || TerminalRuntimeError::InvalidProvision(message)
}

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

pub(super) struct HostContext<'host> {
    output: buffered_output::BufferedStreams,
    input: Option<std::sync::Arc<std::sync::Mutex<Box<dyn std::io::Read + Send + 'host>>>>,
    clock: Option<std::sync::Arc<std::sync::Mutex<clock::TerminalClockEnvironment>>>,
    addresses: std::sync::Arc<std::sync::Mutex<network_address::AddressValues>>,
    native: native_provider::NativeContext,
    filesystem: filesystem::FilesystemContext,
    allocations: std::sync::Arc<std::sync::Mutex<allocation::AllocationValues>>,
}
impl<'host> HostContext<'host> {
    pub(super) fn finish_native_provider(&self) -> Result<(), TerminalRuntimeError> {
        self.native.finish().map_err(|error| {
            TerminalRuntimeError::missing(super::NATIVE_PROVIDER_RETIREMENT, error.reason)
        })
    }
    pub(super) fn new(
        input: Option<Box<dyn std::io::Read + Send + 'host>>,
        clock: Option<clock::TerminalClockEnvironment>,
        native_provider: Option<std::process::Command>,
        native_services: super::TerminalNativeServiceDelegation,
        filesystem: super::TerminalFilesystemDelegation,
    ) -> Self {
        // The sponsor resolves its own relative paths (libc, Windows), so a
        // rooted namespace starts it in that root as well.
        let native_provider = native_provider.map(|mut command| {
            if let super::TerminalFilesystemDelegation::HostDirectory(root) = &filesystem {
                command.current_dir(root);
            }
            command
        });
        let allocations = std::sync::Arc::new(std::sync::Mutex::new(
            allocation::AllocationValues::default(),
        ));
        Self {
            native: native_provider::NativeContext::new(
                native_provider,
                native_services,
                std::sync::Arc::clone(&allocations),
            ),
            filesystem: filesystem::FilesystemContext::new(filesystem),
            output: buffered_output::BufferedStreams::new(),
            input: input.map(|reader| std::sync::Arc::new(std::sync::Mutex::new(reader))),
            clock: clock.map(|snapshot| std::sync::Arc::new(std::sync::Mutex::new(snapshot))),
            addresses: std::sync::Arc::new(std::sync::Mutex::new(
                network_address::AddressValues::default(),
            )),
            allocations,
        }
    }
}

pub(super) trait HostBindingTarget<'host> {
    fn bind_storage_resolver(
        &mut self,
        resolver: std::rc::Rc<dyn terminal_interpreter::TerminalProviderStorageResolver + 'host>,
    ) -> Result<(), TerminalRuntimeError>;
    fn import(&self, slot: BoundaryMachineId) -> Option<&terminal_psi::BoundaryMachineDeclaration>;
    fn bind_import(
        &mut self,
        slot: BoundaryMachineId,
        adapter: impl terminal_interpreter::TerminalImportAdapter + 'host,
    ) -> Result<(), TerminalRuntimeError>;
}
impl<'host> HostBindingTarget<'host> for TerminalInstance<'host, '_> {
    fn bind_storage_resolver(
        &mut self,
        resolver: std::rc::Rc<dyn terminal_interpreter::TerminalProviderStorageResolver + 'host>,
    ) -> Result<(), TerminalRuntimeError> {
        self.bind_storage_resolver(resolver)
            .map_err(|error| TerminalRuntimeError::missing("runtime-provider-storage", error))
    }
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
    fn bind_storage_resolver(
        &mut self,
        resolver: std::rc::Rc<dyn terminal_interpreter::TerminalProviderStorageResolver + 'host>,
    ) -> Result<(), TerminalRuntimeError> {
        self.bind_storage_resolver(resolver)
            .map_err(|error| TerminalRuntimeError::missing("runtime-provider-storage", error))
    }
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

fn bind_numeric_import<'host>(
    program: &TerminalProgram,
    instance: &mut impl HostBindingTarget<'host>,
    slot: BoundaryMachineId,
    realization: target_operations::NearestIeeeFloatArithmeticRealization,
) -> Result<(), TerminalRuntimeError> {
    let declaration = instance
        .import(slot)
        .ok_or(TerminalRuntimeError::InvalidProvision(
            "numeric host provision names no demanded import",
        ))?;
    let adapter = numeric::NumericImport::for_import(program, declaration, realization)?;
    instance.bind_import(slot, adapter)
}

pub fn bind_admitted_intrinsics(
    instance: &mut TerminalInstance<'_, '_>,
    admitted: &provider_admission::provider_admission::AdmittedTerminalIntrinsics,
) -> Result<(), TerminalRuntimeError> {
    let program = instance.program().clone();
    if program.execution_identity() != admitted.terminal() {
        return Err(TerminalRuntimeError::InvalidProvision(
            "semantic intrinsic admission belongs to another Terminal artifact",
        ));
    }
    for (slot, realization) in admitted.operations() {
        bind_numeric_import(&program, instance, *slot, *realization)?;
    }
    Ok(())
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
    instance.bind_storage_resolver(std::rc::Rc::new(allocation::AllocationStorage {
        values: std::sync::Arc::clone(allocations),
    }))?;
    let mut emulates_allocation = false;
    for provision in provisions.map_or(&[][..], TerminalRuntimeHostProvisions::host) {
        if imports.contains(&provision.boundary)
            && let BoundarySettlementRealization::NormalizedForeignCall(binding) =
                &provision.realization
        {
            emulates_allocation |= allocation::emulates_allocation(binding)?;
        }
    }
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
                bind_numeric_import(program, instance, *slot, realization)?;
                continue;
            }
            BoundarySettlementRealization::NormalizedForeignCall(ref binding) => {
                let declaration =
                    instance
                        .import(*slot)
                        .ok_or(TerminalRuntimeError::InvalidProvision(
                            "foreign host provision names no demanded import",
                        ))?;
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
                if let Some(adapter) = filesystem::linux::LinuxFilesystemImport::admit(
                    &context.filesystem,
                    program,
                    declaration,
                    binding,
                    provisions.map_or(&[], TerminalRuntimeHostProvisions::borrowed_storage_layouts),
                )? {
                    instance.bind_import(*slot, adapter)?;
                    continue;
                }
                let filesystem = context.filesystem.admit(
                    program,
                    declaration,
                    binding,
                    provisions
                        .map(|provisions| provisions.borrowed_storage_layouts())
                        .unwrap_or(&[]),
                )?;
                if filesystem.as_ref().is_some_and(|call| call.descriptor_io()) {
                    let adapter = native_provider::NativeImport::admit(
                        program,
                        declaration,
                        binding,
                        provisions,
                        &context.native,
                        filesystem,
                        input.as_ref().cloned(),
                        output.clone(),
                    )?
                    .ok_or_else(|| {
                        TerminalRuntimeError::missing(
                            "runtime-filesystem",
                            "filesystem descriptor needs an explicit matching-host native sponsor",
                        )
                    })?;
                    instance.bind_import(*slot, adapter)?;
                    continue;
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
                    emulates_allocation,
                )? {
                    instance.bind_import(*slot, adapter)?;
                    continue;
                }
                if let Some(adapter) = reference_state::ReferenceStateImport::admit(
                    program,
                    declaration,
                    binding,
                    provisions.and_then(|provisions| provisions.host_calling_application(*slot)),
                )? {
                    instance.bind_import(*slot, adapter)?;
                    continue;
                }
                if let Some(adapter) = clock::ClockImport::admit(
                    program,
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
                    &context.filesystem,
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
                if let Some(adapter) = integer_parsing::IntegerParsingImport::admit(
                    program,
                    declaration,
                    binding,
                    provisions.map_or(&[], TerminalRuntimeHostProvisions::borrowed_storage_layouts),
                )? {
                    instance.bind_import(*slot, adapter)?;
                    continue;
                }
                if let Some(adapter) = network_address::NetworkAddressImport::admit(
                    declaration,
                    binding,
                    provisions.and_then(|provisions| provisions.host_calling_application(*slot)),
                    std::sync::Arc::clone(addresses),
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
                    filesystem,
                    input.as_ref().cloned(),
                    output.clone(),
                )? {
                    instance.bind_import(*slot, adapter)?;
                    continue;
                }
                return Err(TerminalRuntimeError::missing(
                    "runtime-host-import-bindings",
                    format!(
                        "no trusted host implementation matches boundary {:?} `{}` with mechanism {:?}",
                        declaration.id,
                        program
                            .module()
                            .identities
                            .text(declaration.identity)
                            .unwrap_or("<missing identity>"),
                        binding.mechanism
                    ),
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
