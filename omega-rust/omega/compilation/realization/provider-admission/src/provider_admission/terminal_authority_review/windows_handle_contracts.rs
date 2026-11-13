//! Reviewed Windows file-object protocols over admitted physical executions.
//!
//! These contracts describe handle creation, aliasing and preservation, not
//! receiver permissions. Complete normalized locators and actual native word
//! placement identify an API; source method names and empty reach do not.

use std::sync::OnceLock;

use calling_conventions::{CallPlan, CallSignature, CallingPolicy, ValueLocation, ValueShape};
use semantic_vocabulary::{BoundaryMachineId, IntegerSign, ScalarType, StructuralTypeId};
use target_operations::NormalizedForeignCallMechanism;
use terminal_psi::{
    BoundaryMachineDeclaration, BoundaryMachineResult, BoundaryParameterKind, StructuralAccess,
    StructuralFieldType, StructuralRecordStorage, StructuralTypeShape,
};

use super::context::ReviewContext;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum WindowsHandleContract {
    /// No kernel-handle creation, alias or lifetime effects. The flow reader
    /// still checks argument storage and rejects handle values escaping through
    /// unproved scalar/buffer roles; empty authority does not imply this fact.
    NoHandleIntervention,
    CreateFile,
    DuplicateHandle,
    CloseHandle,
    CurrentProcess,
    QueryFileMode,
    QueryFinalPath,
    ReadFile,
    WriteFile,
}

fn no_handle_intervention(locator: target::ForeignLocatorIdentityDigest) -> bool {
    static LOCATORS: OnceLock<Vec<target::ForeignLocatorIdentityDigest>> = OnceLock::new();
    LOCATORS
        .get_or_init(|| {
            let apis: &[(&[u8], &[u8])] = &[
                (b"kernel32.dll", b"GetLastError"),
                (b"kernel32.dll", b"SetLastError"),
                (b"ntdll.dll", b"RtlNtStatusToDosError"),
                (b"kernel32.dll", b"WideCharToMultiByte"),
                (b"kernel32.dll", b"CreateDirectoryW"),
                (b"kernel32.dll", b"DeleteFileW"),
                (b"kernel32.dll", b"RemoveDirectoryW"),
            ];
            apis.iter()
                .map(|&(library, export)| {
                    target::normalize_foreign_locator(
                        target::ForeignLocatorCandidate::PeByName {
                            library: library.to_vec(),
                            export: export.to_vec(),
                        },
                        target::TargetProfile::WindowsX64,
                    )
                    .expect("reviewed nonintervening Windows API locator")
                    .identity_digest()
                })
                .collect()
        })
        .contains(&locator)
}

#[derive(Clone, Copy)]
enum Formal {
    Integer(IntegerSign, u16),
    IntegerOutput(IntegerSign, u16),
    WideArray(StructuralAccess),
    ByteView(StructuralAccess),
    IoStatus,
}

struct ReviewedApi {
    locator: target::ForeignLocatorIdentityDigest,
    contract: WindowsHandleContract,
    parameters: &'static [Formal],
    result: Formal,
}

/// The model is deliberately bounded to native calls in this customer's
/// closure. Unsupported handle operations remain unknown, never preserving by
/// default. The ordinary foreign classifier owns their separate class union.
fn reviewed_apis() -> &'static [ReviewedApi] {
    use Formal::{ByteView, Integer, IntegerOutput, IoStatus, WideArray};
    use IntegerSign::{Signed as I, Unsigned as U};
    use StructuralAccess::{MutableBorrow, SharedBorrow};
    use WindowsHandleContract as C;

    static APIS: OnceLock<Vec<ReviewedApi>> = OnceLock::new();
    APIS.get_or_init(|| {
        let rows: &[(&[u8], &[u8], C, &[Formal], Formal)] = &[
            (
                b"kernel32.dll",
                b"CreateFileW",
                C::CreateFile,
                &[
                    WideArray(SharedBorrow),
                    Integer(U, 32),
                    Integer(U, 32),
                    Integer(U, 64),
                    Integer(U, 32),
                    Integer(U, 32),
                    Integer(U, 64),
                ],
                Integer(I, 64),
            ),
            (
                b"kernel32.dll",
                b"DuplicateHandle",
                C::DuplicateHandle,
                &[
                    Integer(I, 64),
                    Integer(I, 64),
                    Integer(I, 64),
                    IntegerOutput(I, 64),
                    Integer(U, 32),
                    Integer(I, 32),
                    Integer(U, 32),
                ],
                Integer(I, 32),
            ),
            (
                b"kernel32.dll",
                b"CloseHandle",
                C::CloseHandle,
                &[Integer(I, 64)],
                Integer(I, 32),
            ),
            (
                b"kernel32.dll",
                b"GetCurrentProcess",
                C::CurrentProcess,
                &[],
                Integer(I, 64),
            ),
            (
                b"ntdll.dll",
                b"NtQueryInformationFile",
                C::QueryFileMode,
                &[
                    Integer(I, 64),
                    IoStatus,
                    IntegerOutput(U, 32),
                    Integer(U, 32),
                    Integer(U, 32),
                ],
                Integer(I, 32),
            ),
            (
                b"kernel32.dll",
                b"GetFinalPathNameByHandleW",
                C::QueryFinalPath,
                &[
                    Integer(I, 64),
                    WideArray(MutableBorrow),
                    Integer(U, 32),
                    Integer(U, 32),
                ],
                Integer(U, 32),
            ),
            (
                b"kernel32.dll",
                b"ReadFile",
                C::ReadFile,
                &[
                    Integer(I, 64),
                    ByteView(MutableBorrow),
                    IntegerOutput(U, 32),
                    Integer(U, 64),
                ],
                Integer(I, 32),
            ),
            (
                b"kernel32.dll",
                b"WriteFile",
                C::WriteFile,
                &[
                    Integer(I, 64),
                    ByteView(SharedBorrow),
                    IntegerOutput(U, 32),
                    Integer(U, 64),
                ],
                Integer(I, 32),
            ),
        ];
        rows.iter()
            .map(|&(library, export, contract, parameters, result)| {
                let locator = target::normalize_foreign_locator(
                    target::ForeignLocatorCandidate::PeByName {
                        library: library.to_vec(),
                        export: export.to_vec(),
                    },
                    target::TargetProfile::WindowsX64,
                )
                .expect("reviewed Windows handle API locator");
                ReviewedApi {
                    locator: locator.identity_digest(),
                    contract,
                    parameters,
                    result,
                }
            })
            .collect()
    })
}

pub(super) struct ReviewedHandleCall<'a> {
    pub contract: WindowsHandleContract,
    pub declaration: &'a BoundaryMachineDeclaration,
}

/// Check the actual admitted execution, including its full entry plan. Replay
/// retains the original operand declaration and reviewed API protocol; a
/// commitment alone cannot say which source operand reaches an API slot.
pub(super) fn reviewed_call<'a>(
    context: &'a ReviewContext<'_>,
    boundary: BoundaryMachineId,
) -> Result<Option<ReviewedHandleCall<'a>>, String> {
    let Some(ReviewedWindowsImport {
        locator,
        declaration,
        binding,
    }) = original_windows_import(context, boundary)?
    else {
        return Ok(None);
    };
    if no_handle_intervention(locator.identity_digest()) {
        return Ok(Some(ReviewedHandleCall {
            contract: WindowsHandleContract::NoHandleIntervention,
            declaration,
        }));
    }
    let Some(api) = reviewed_apis()
        .iter()
        .find(|api| api.locator == locator.identity_digest())
    else {
        return Ok(None);
    };
    if !matches_declaration(context, declaration, api) {
        return Err(format!(
            "Windows {:?} at {boundary:?} does not match its reviewed complete operand contract",
            api.contract,
        ));
    }
    let expected = expected_call(api)?;
    if binding.boundary_entry_plan.call != expected {
        return Err(format!(
            "Windows {:?} at {boundary:?} changes the reviewed physical argument placement",
            api.contract,
        ));
    }
    Ok(Some(ReviewedHandleCall {
        contract: api.contract,
        declaration,
    }))
}

/// This is a process-ending outcome, not a preserving handle operation. The
/// retained declaration and selected schema must already promise external
/// completion; an ExitProcess locator alone cannot abandon caller obligations.
/// Runtime still owes authority for this exact containing process domain.
pub(super) fn reviewed_process_exit<'a>(
    context: &'a ReviewContext<'_>,
    boundary: BoundaryMachineId,
) -> Result<Option<&'a BoundaryMachineDeclaration>, String> {
    let Some(ReviewedWindowsImport {
        locator,
        declaration,
        binding,
    }) = original_windows_import(context, boundary)?
    else {
        return Ok(None);
    };
    if !matches!(
        declaration.completion,
        terminal_psi::BoundaryCompletion::External(
            semantic_vocabulary::ExternalCompletionKind::ProcessExit
        )
    ) {
        return Ok(None);
    }
    let requirement = binding
        .requirement_identity(context.selected.identities())
        .ok_or("process exit lost its original selected requirement identity")?;
    let (plan, _, _) = context.selected_requirement(requirement)?;
    let receiving = context
        .input
        .module()
        .boundary_machine(boundary)
        .ok_or("process exit lost its original receiving declaration")?;
    let receiving_identity = context
        .input
        .module()
        .identities
        .text(receiving.identity)
        .ok_or("process exit lost its original receiving identity")?;
    if !context.selected.selected_row_matches_requirement(
        receiving_identity,
        requirement,
        binding.provider_plan_report_identity(),
    ) || !super::super::compiler_builtins::boundary_contract_matches_selected_schema(
        declaration,
        plan,
        requirement,
    ) {
        return Err(format!(
            "process exit {boundary:?} substitutes its exact admitted requirement completion"
        ));
    }
    if !process_exit_physical_matches(locator, declaration, &binding.boundary_entry_plan.call)? {
        return Err(format!(
            "process exit {boundary:?} changes its admitted declaration/Windows ABI: identity={receiving_identity}, declaration={declaration:?}, call={:?}",
            binding.boundary_entry_plan.call,
        ));
    }
    Ok(Some(declaration))
}

fn process_exit_physical_matches(
    locator: &target::NormalizedForeignLocator,
    declaration: &BoundaryMachineDeclaration,
    call: &CallPlan,
) -> Result<bool, String> {
    let exit = target::normalize_foreign_locator(
        target::ForeignLocatorCandidate::PeByName {
            library: b"kernel32.dll".to_vec(),
            export: b"ExitProcess".to_vec(),
        },
        target::TargetProfile::WindowsX64,
    )
    .map_err(|reason| format!("process exit locator is unsupported: {reason:?}"))?;
    let expected = calling_conventions::evaluate_call_plan(
        CallingPolicy::MicrosoftX64,
        &CallSignature {
            parameters: vec![ValueShape::integer(4, 4)],
            result: None,
        },
    )
    .map_err(|reason| reason.to_string())?;
    Ok(locator.identity_digest() == exit.identity_digest()
        && process_exit_signature(declaration)
        && *call == expected)
}

fn process_exit_signature(declaration: &BoundaryMachineDeclaration) -> bool {
    declaration.role == terminal_psi::BoundaryDeclarationRole::Callable
        && declaration.has_valid_parameter_order()
        && declaration.parameter_order == [BoundaryParameterKind::Scalar]
        && declaration.scalar_parameters.len() == 1
        && integer_matches(declaration.scalar_parameters[0], IntegerSign::Signed, 32)
        && declaration.structural_parameters.is_empty()
        && declaration.result.is_unit()
        && matches!(
            declaration.completion,
            terminal_psi::BoundaryCompletion::External(
                semantic_vocabulary::ExternalCompletionKind::ProcessExit
            )
        )
        && declaration.crash_routes.is_empty()
        && declaration.scalar_ensures.is_empty()
        && declaration.normal_qualifications.is_empty()
        && declaration.content_guarantees.is_empty()
        && declaration.program_local_root_introductions.is_empty()
}

/// Reuse the actual settled import and complete normalized key. In particular,
/// a checked/pre-narrowed mechanism cannot stand in for its original execution.
struct ReviewedWindowsImport<'a> {
    locator: &'a target::NormalizedForeignLocator,
    declaration: &'a BoundaryMachineDeclaration,
    binding: &'a target_operations::NormalizedForeignCallBinding,
}

fn original_windows_import<'a>(
    context: &'a ReviewContext<'_>,
    boundary: BoundaryMachineId,
) -> Result<Option<ReviewedWindowsImport<'a>>, String> {
    let Some(binding) = context.foreign_binding(boundary) else {
        return Ok(None);
    };
    let NormalizedForeignCallMechanism::Import { locator, .. } = &binding.mechanism else {
        return Ok(None);
    };
    if locator.target() != target::TargetProfile::WindowsX64 {
        return Ok(None);
    }
    let actual = crate::provider_admission::terminal_authority_policy::normalized_foreign_terminal_mechanism(
        locator, &binding.boundary_entry_plan,
    )?;
    if context.mechanism(boundary) != Some(&actual) {
        return Err(format!(
            "Windows handle call {boundary:?} substitutes its complete admitted foreign key"
        ));
    }
    let calling_boundary = crate::selected_calling_boundary(
        context.input.module(),
        context.input.target_machine_selections(),
        boundary,
    )?;
    let declaration = context
        .input
        .module()
        .boundary_machine(calling_boundary)
        .ok_or_else(|| format!("Windows handle call {boundary:?} lost its admitted declaration"))?;
    Ok(Some(ReviewedWindowsImport {
        locator,
        declaration,
        binding,
    }))
}

fn integer_matches(scalar: ScalarType, sign: IntegerSign, bits: u16) -> bool {
    matches!(scalar, ScalarType::Integer(integer)
        if !integer.is_address() && integer.sign() == sign && integer.bits() == bits)
}

fn matches_declaration(
    context: &ReviewContext<'_>,
    declaration: &BoundaryMachineDeclaration,
    api: &ReviewedApi,
) -> bool {
    if !declaration.has_valid_parameter_order()
        || declaration.parameter_order.len() != api.parameters.len()
        || declaration.completion != terminal_psi::BoundaryCompletion::NormalReturn
        || !declaration.scalar_ensures.is_empty()
        || !declaration.normal_qualifications.is_empty()
        || !declaration.content_guarantees.is_empty()
        || !declaration.program_local_root_introductions.is_empty()
        || !declaration.crash_routes.is_empty()
    {
        return false;
    }
    let Formal::Integer(sign, bits) = api.result else {
        return false;
    };
    if !matches!(&declaration.result, BoundaryMachineResult::Scalar(result)
        if integer_matches(result.scalar_type, sign, bits))
    {
        return false;
    }
    let (mut scalar, mut structural) = (0, 0);
    for (kind, expected) in declaration.parameter_order.iter().zip(api.parameters) {
        let valid = match kind {
            BoundaryParameterKind::Scalar => {
                let actual = declaration.scalar_parameters[scalar];
                scalar += 1;
                matches!(expected, Formal::Integer(sign, bits) if integer_matches(actual, *sign, *bits))
            }
            BoundaryParameterKind::Structural => {
                let actual = &declaration.structural_parameters[structural];
                structural += 1;
                structural_matches(context, actual.structural_type, actual.access, *expected)
            }
        };
        if !valid {
            return false;
        }
    }
    true
}

fn structural_matches(
    context: &ReviewContext<'_>,
    identity: StructuralTypeId,
    access: StructuralAccess,
    expected: Formal,
) -> bool {
    let Some(declaration) = context.input.module().structural_type(identity) else {
        return false;
    };
    match expected {
        Formal::IntegerOutput(sign, bits) => {
            access == StructuralAccess::MutableBorrow
                && matches!(declaration.shape, StructuralTypeShape::PrimitiveScalar(scalar)
                if integer_matches(scalar, sign, bits))
        }
        Formal::WideArray(expected_access) => {
            access == expected_access
                && matches!(&declaration.shape, StructuralTypeShape::FixedArray {
                element, ..
            } if context.input.module().structural_type(*element).is_some_and(|element|
                matches!(element.shape, StructuralTypeShape::PrimitiveScalar(scalar)
                    if integer_matches(scalar, IntegerSign::Unsigned, 16))))
        }
        Formal::ByteView(expected_access) => {
            access == expected_access
                && matches!(declaration.shape, StructuralTypeShape::ByteSequence(
                terminal_psi::ByteSequenceCarrier::BorrowedView { access: ceiling }
            ) if ceiling.is_none_or(|ceiling| ceiling == expected_access))
        }
        Formal::IoStatus => {
            access == StructuralAccess::MutableBorrow
                && matches!(&declaration.shape, StructuralTypeShape::Record {
                fields, storage: StructuralRecordStorage::Packed,
            } if fields.len() == 2 && fields.iter().zip([IntegerSign::Signed, IntegerSign::Unsigned])
                .all(|(field, sign)| !field.relevance.is_erased()
                    && field.storage.is_none()
                    && matches!(field.field_type, StructuralFieldType::Scalar(scalar)
                        if integer_matches(scalar, sign, 64))))
        }
        Formal::Integer(_, _) => false,
    }
}

fn expected_call(api: &ReviewedApi) -> Result<CallPlan, String> {
    let shape = |formal| match formal {
        Formal::Integer(_, bits) => ValueShape::integer(u32::from(bits / 8), bits / 8),
        Formal::ByteView(_) => ValueShape::integer(16, 8),
        _ => ValueShape::integer(8, 8),
    };
    let signature = CallSignature {
        parameters: api.parameters.iter().copied().map(shape).collect(),
        result: Some(shape(api.result)),
    };
    if !matches!(
        api.contract,
        WindowsHandleContract::ReadFile | WindowsHandleContract::WriteFile
    ) {
        return calling_conventions::evaluate_call_plan(CallingPolicy::MicrosoftX64, &signature)
            .map_err(|error| error.to_string());
    }
    // The checked source's one complete byte view supplies the API's pointer
    // and count words. Retain the whole u64 count; the caller separately proves
    // it fits DWORD. The count-output pointer stays the fourth native word.
    let native = CallSignature {
        parameters: vec![ValueShape::integer(8, 8); 5],
        result: signature.result,
    };
    let mut plan = calling_conventions::evaluate_call_plan(CallingPolicy::MicrosoftX64, &native)
        .map_err(|error| error.to_string())?;
    let count = plan.parameters.remove(2);
    plan.parameters[1].shape = ValueShape::integer(16, 8);
    for mut location in count.locations {
        let ValueLocation::Register {
            value_byte_offset, ..
        } = &mut location
        else {
            return Err("Microsoft byte-view count has no reviewed register placement".into());
        };
        *value_byte_offset += 8;
        plan.parameters[1].locations.push(location);
    }
    Ok(plan)
}

#[cfg(test)]
mod tests {
    use super::*;
    use semantic_vocabulary::{BoundaryCompletion, ExternalCompletionKind, IntegerType};
    use terminal_psi::{
        BoundaryDeclarationRole, NominalDeclarationIdentity, TerminalIdentityIndex,
    };

    fn declaration() -> BoundaryMachineDeclaration {
        BoundaryMachineDeclaration {
            id: BoundaryMachineId::new(1).unwrap(),
            declaration: None,
            numeric_intrinsic: None,
            role: BoundaryDeclarationRole::Callable,
            kind: semantic_vocabulary::BoundaryRequirementKind::Trait,
            completion: BoundaryCompletion::External(ExternalCompletionKind::ProcessExit),
            unconditional_termination: false,
            identity: TerminalIdentityIndex::ABSENT,
            owner: NominalDeclarationIdentity::new(TerminalIdentityIndex::ABSENT),
            attachment: None,
            parameter_order: vec![BoundaryParameterKind::Scalar],
            scalar_parameters: vec![ScalarType::Integer(
                IntegerType::new(IntegerSign::Signed, 32).unwrap(),
            )],
            crash_routes: Vec::new(),
            structural_parameters: Vec::new(),
            result: BoundaryMachineResult::Unit,
            result_reference_regions: Vec::new(),
            requires: Vec::new(),
            normal_qualifications: Vec::new(),
            scalar_domain_requirements: Vec::new(),
            scalar_requires: Vec::new(),
            scalar_ensures: Vec::new(),
            reference_outputs: Vec::new(),
            program_local_root_introductions: Vec::new(),
            content_guarantees: Vec::new(),
            fixed_service_reach: Vec::new(),
            owning_service: None,
            published_synchronous_entries: Vec::new(),
            published_service_ceiling: Vec::new(),
        }
    }

    #[test]
    fn windows_handle_process_exit_requires_external_declaration_and_exact_windows_abi() {
        // This control tests representation/ABI only. The production caller
        // separately rejoins admitted source completion and selected schema;
        // these deliberately absent identities cannot supply that custody.
        let mut declaration = declaration();
        let locator = |export: &[u8]| {
            target::normalize_foreign_locator(
                target::ForeignLocatorCandidate::PeByName {
                    library: b"kernel32.dll".to_vec(),
                    export: export.to_vec(),
                },
                target::TargetProfile::WindowsX64,
            )
            .unwrap()
        };
        let signature = CallSignature {
            parameters: vec![ValueShape::integer(4, 4)],
            result: None,
        };
        let call = calling_conventions::evaluate_call_plan(CallingPolicy::MicrosoftX64, &signature)
            .unwrap();
        let exit = locator(b"ExitProcess");
        assert!(process_exit_physical_matches(&exit, &declaration, &call).unwrap());
        assert!(
            !process_exit_physical_matches(&locator(b"CloseHandle"), &declaration, &call).unwrap()
        );
        let mut returns = signature.clone();
        returns.result = Some(ValueShape::integer(4, 4));
        let returns =
            calling_conventions::evaluate_call_plan(CallingPolicy::MicrosoftX64, &returns).unwrap();
        assert!(!process_exit_physical_matches(&exit, &declaration, &returns).unwrap());
        declaration.completion = BoundaryCompletion::NormalReturn;
        assert!(!process_exit_physical_matches(&exit, &declaration, &call).unwrap());
        declaration.completion = BoundaryCompletion::External(ExternalCompletionKind::ProcessExit);
        declaration.scalar_parameters[0] =
            ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 32).unwrap());
        assert!(!process_exit_physical_matches(&exit, &declaration, &call).unwrap());
    }
}
