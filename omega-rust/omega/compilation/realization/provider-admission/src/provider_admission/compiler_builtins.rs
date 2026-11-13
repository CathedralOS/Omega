/*
Native compiler-builtin admission and placed-scalar schema and backing-origin
verification.

Proposal derivation (`retained`) binds each demanded boundary to its one
selected intrinsic row, plan, and Terminal boundary once and carries that
binding to admission by plan index; `settle_compiler_builtins` admits it
through the receiving policy and the local target catalog.
*/

/*
Hosted builtins settle as call-shaped rows. Selected float arithmetic, minimum,
maximum, negation, square root, NaN/infinity classification, two-rounding
multiply-then-add, conversion into a float format, and Exact, Saturating, or
Trapping conversion to an integer settle as `NearestIeeeFloatArithmetic`, which
Omega 02 realizes inline as one IEEE semantic operation. Admission requires the
boundary's exact signature for that operation and format so a row selected for
another format, carrier, or shape cannot stand in.

For placed caller-RAM scalar access (`AccessorScalarAccess`), we join the
selected scalar schema to its independently admitted Terminal declaration
(`validate_access_schema`) and verify that every reachable occurrence originates
from captured RAM or a checked resident formation
(`require_access_backing_origins`).
*/

mod catalog;
mod retained;

pub use catalog::SettledCompilerBuiltin;
pub use retained::{
    AdmittedTerminalIntrinsics, CompilerIntrinsicPlanProvenance, TerminalCompilerBuiltinProposal,
    TerminalCompilerBuiltinSettlement, admit_terminal_intrinsics,
    derive_selected_intrinsic_settlement_proposals, prepare_retained_compiler_builtins,
    validate_external_provider_owners,
};

use crate::provider_admission::AdmittedTerminalMechanism;
use crate::provider_admission::{NativeRealizationInput, ProviderAdmissionRequest};
use crate::{AdmittedBoundaryExecution, AdmittedBoundarySettlement};
use diagnostics::Diagnostic;
use effects::CompilerAccessorScalarOperation;
use effects::provider_plan::{ProviderPlan, ServiceSchema};
use semantic_vocabulary::{
    BoundaryCompletion, ExternalCompletionKind, IeeeFloatFormat, IntegerSign, ScalarType,
};
use target_operations::{
    BoundarySettlementRealization, CompilerBuiltinExecution, HostedExitProcessI32Realization,
    HostedReadByteRealization, HostedWriteByteI32Realization,
    NearestIeeeFloatArithmeticRealization,
};
use terminal_psi::{
    BoundaryMachineDeclaration, StructuralAccess, TerminalAccessorKind, TerminalModule,
};
use terminal_semantics::AccessorScalarBoundaryDemand;

pub(super) fn settle_compiler_builtins<'request>(
    input: &NativeRealizationInput,
    request: &ProviderAdmissionRequest<'_, 'request>,
) -> Result<
    (
        Vec<AdmittedBoundarySettlement<'request>>,
        Vec<AdmittedTerminalMechanism>,
    ),
    Vec<Diagnostic>,
> {
    let unsettled = request
        .compiler_builtins
        .iter()
        .filter_map(|proposal| {
            let requirement = proposal.requirement_identity;
            let plan = &proposal.provider_plan.name;
            match proposal.settlement {
                TerminalCompilerBuiltinSettlement::Execution { .. } => None,
                TerminalCompilerBuiltinSettlement::NoClosedCatalogIdentity => {
                    Some(Diagnostic::error(format!(
                        "selected compiler intrinsic `{plan}` for Terminal boundary `{requirement}` has no closed native catalog identity",
                    )))
                }
                TerminalCompilerBuiltinSettlement::NoNativeBoundaryRealization => {
                    Some(Diagnostic::error(format!(
                        "selected compiler intrinsic `{plan}` for Terminal boundary `{requirement}` has no native boundary realization",
                    )))
                }
            }
        })
        .collect::<Vec<_>>();
    if !unsettled.is_empty() {
        return Err(unsettled);
    }
    admit_x86_fma_features(request)?;
    let mut admitted = Vec::with_capacity(request.compiler_builtins.len());
    let mut mechanisms = Vec::with_capacity(request.compiler_builtins.len());
    for proposal in request.compiler_builtins {
        let requirement = proposal.requirement_identity;
        let TerminalCompilerBuiltinSettlement::Execution {
            identity: mechanism,
            execution: settled,
        } = proposal.settlement
        else {
            unreachable!("unsettled compiler-builtin proposals were refused above");
        };
        let execution = settled.execution();
        let boundary = terminal_psi::find_boundary_machine(
            &input.plan().boundary_machines,
            proposal.boundary,
        )
        .ok_or_else(|| {
            vec![Diagnostic::error(format!(
                "compiler-builtin proposal for `{requirement}` names an absent Terminal boundary"
            ))]
        })?;
        if !boundary_contract_matches_selected_schema(boundary, proposal.provider_plan, requirement)
        {
            return Err(vec![Diagnostic::error(format!(
                "compiler-builtin boundary {:?} for `{requirement}` disagrees with its exact selected requirement contract (completion {:?}, unconditional termination {})",
                boundary.id, boundary.completion, boundary.unconditional_termination,
            ))]);
        }
        let completion = compiler_builtin_completion(execution);
        if boundary.completion != completion {
            return Err(vec![Diagnostic::error(format!(
                "compiler-builtin boundary {:?} for `{requirement}` declares completion {:?}, but its exact native mechanism requires {:?}",
                boundary.id, boundary.completion, completion,
            ))]);
        }
        request
            .terminal_authority_policy
            .classify(mechanism)
            .map_err(|unclassified| {
                vec![Diagnostic::error(format!(
                    "receiving terminal-authority policy version {} does not classify compiler intrinsic {:?} required by `{requirement}`",
                    request.terminal_authority_policy.identity().version(),
                    unclassified.mechanism(),
                ))]
            })?;
        let realization = match execution {
            CompilerBuiltinExecution::StaticInterrupt { .. } => {
                return Err(vec![Diagnostic::error(
                    "static interrupt execution requires its exact installed occurrence; ordinary provider proposals cannot supply it",
                )]);
            }
            CompilerBuiltinExecution::AccessLifecycle { .. }
            | CompilerBuiltinExecution::EmptyRunRelease => {
                return Err(vec![Diagnostic::error(
                    "private access lifecycle cannot be selected through a provider proposal",
                )]);
            }
            CompilerBuiltinExecution::ProgramStorageEntry => {
                let binding = input
                    .selected_program_entry_call()
                    .filter(|binding| {
                        binding.boundary() == boundary.id
                            && binding.provider_plan() == proposal.provider_plan.identity_digest()
                    })
                    .ok_or_else(|| {
                        vec![Diagnostic::error(
                            "semantic-entry transport has no exact Build-selected continuation binding",
                        )]
                    })?;
                if input.plan().function(binding.entry()).is_none() {
                    return Err(vec![Diagnostic::error(
                        "semantic-entry continuation left its executable closure",
                    )]);
                }
                mechanisms.push(AdmittedTerminalMechanism {
                    boundary: boundary.id,
                    mechanism: mechanism.into(),
                });
                continue;
            }
            CompilerBuiltinExecution::ProgramImageRetainedSuffixStart => {
                let scalar = semantic_vocabulary::IntegerType::address(64)
                    .map(semantic_vocabulary::ScalarType::Integer)
                    .map_err(|_| vec![Diagnostic::error("invalid target address carrier")])?;
                if request.target() != target::NativeTarget::uefi_x64()
                    || !boundary.scalar_parameters.is_empty()
                    || !boundary.structural_parameters.is_empty()
                    || boundary.result != terminal_psi::BoundaryMachineResult::Scalar(scalar.into())
                {
                    return Err(vec![Diagnostic::error(
                        "program image observation requires the exact UEFI address-result boundary",
                    )]);
                }
                target_operations::BoundaryRealization::ProgramImageRetainedSuffixStart
            }
            CompilerBuiltinExecution::AccessorScalarAccess {
                contract,
                placement_commitment,
                access_slot,
                operation,
            } => {
                let module = input.module();
                let demand = terminal_semantics::accessor_scalar_boundary_demand(
                    &module.structural_types,
                    &module.identities,
                    &module.access_view_plans,
                    &module.boundary_machines,
                    boundary.id,
                )
                .map_err(|reason| vec![Diagnostic::error(reason)])?;
                validate_access_schema(
                    module,
                    boundary,
                    proposal.provider_plan,
                    proposal.semantic_schema,
                    &demand,
                )
                .map_err(|reason| vec![Diagnostic::error(reason)])?;
                require_access_backing_origins(input, boundary.id)
                    .map_err(|reason| vec![Diagnostic::error(reason)])?;
                let compiler_operation = access_operation(operation);
                if demand.placement_commitment != placement_commitment
                    || demand.access_slot != access_slot
                    || demand.operation != operation
                    || operation == TerminalAccessorKind::Take
                    || effects::compiler_accessor_scalar_contract_identity(
                        proposal.semantic_schema.identity_digest(),
                        placement_commitment,
                        access_slot,
                        compiler_operation,
                    ) != contract
                {
                    return Err(vec![Diagnostic::error(
                        "placed scalar execution substituted its sealed operation or requested unsupported destructive take",
                    )]);
                }
                target_operations::AccessorScalarAccessRealization { contract, demand }.into()
            }
            CompilerBuiltinExecution::HostedExitProcessI32
                if HostedExitProcessI32Realization::supports_target(request.target()) =>
            {
                HostedExitProcessI32Realization.into()
            }
            CompilerBuiltinExecution::HostedExitProcessI32 => {
                return Err(vec![Diagnostic::error(format!(
                    "local target catalog cannot realize hosted process exit for `{requirement}` on {:?}",
                    request.target()
                ))]);
            }
            CompilerBuiltinExecution::HostedWriteByteI32
                if HostedWriteByteI32Realization::supports_target(request.target()) =>
            {
                HostedWriteByteI32Realization.into()
            }
            CompilerBuiltinExecution::HostedWriteByteI32 => {
                return Err(vec![Diagnostic::error(format!(
                    "local target catalog cannot realize hosted write-byte for `{requirement}` on {:?}",
                    request.target()
                ))]);
            }
            CompilerBuiltinExecution::HostedReadByte
                if HostedReadByteRealization::supports_target(request.target()) =>
            {
                HostedReadByteRealization.into()
            }
            CompilerBuiltinExecution::HostedReadByte => {
                return Err(vec![Diagnostic::error(format!(
                    "local target catalog cannot realize hosted read-byte for `{requirement}` on {:?}",
                    request.target()
                ))]);
            }
            CompilerBuiltinExecution::NearestIeeeFloatArithmetic { operation, format } => {
                let x86_scalar_fma = request.x86_scalar_fma_provider.filter(|_| {
                    operation == target_operations::IeeeFloatArithmeticOperation::FusedMultiplyAdd
                        && request.target().architecture == target::Architecture::X86_64
                });
                float_arithmetic_realization(
                    NearestIeeeFloatArithmeticRealization {
                        operation,
                        format,
                        x86_scalar_fma,
                    },
                    request.target(),
                    boundary,
                    &input.plan().structural_types,
                )
                .map_err(|reason| {
                    vec![Diagnostic::error(format!(
                        "compiler float arithmetic for `{requirement}` {reason}"
                    ))]
                })?
                .into()
            }
        };
        admitted.push(AdmittedBoundarySettlement {
            boundary: boundary.id,
            execution: AdmittedBoundaryExecution::CompilerBuiltin(settled),
            realization: BoundarySettlementRealization::Builtin(realization),
        });
        mechanisms.push(AdmittedTerminalMechanism {
            boundary: boundary.id,
            mechanism: mechanism.into(),
        });
    }
    Ok((admitted, mechanisms))
}

fn admit_x86_fma_features(
    request: &ProviderAdmissionRequest<'_, '_>,
) -> Result<(), Vec<Diagnostic>> {
    let Some(requirement) = target::X86FeatureRequirement::scalar_fma(request.profile) else {
        return Ok(());
    };
    let mut diagnostics = Vec::new();
    for proposal in request.compiler_builtins {
        let TerminalCompilerBuiltinSettlement::Execution { execution, .. } = proposal.settlement
        else {
            continue;
        };
        let CompilerBuiltinExecution::NearestIeeeFloatArithmetic {
            operation: target_operations::IeeeFloatArithmeticOperation::FusedMultiplyAdd,
            format,
        } = execution.execution()
        else {
            continue;
        };
        let slot = match format {
            semantic_vocabulary::IeeeFloatFormat::Binary32 => target::X86ScalarFmaSlot::Binary32,
            semantic_vocabulary::IeeeFloatFormat::Binary64 => target::X86ScalarFmaSlot::Binary64,
        };
        let Some(provider) = request.x86_scalar_fma_provider else {
            diagnostics.push(Diagnostic::error(format!(
                "selected x86 scalar FMA ProviderPlan `{}` requires explicit AVX+FMA3 admission for slot `{}`",
                proposal.provider_plan.name,
                slot.requirement_identity(),
            )));
            continue;
        };
        if provider.profile() != request.profile || !provider.admits(requirement, slot) {
            diagnostics.push(Diagnostic::error(format!(
                "x86 scalar FMA admission does not match exact selected profile `{}` for slot `{}`",
                request.profile.target_name(),
                slot.requirement_identity(),
            )));
        }
    }
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

pub(super) fn boundary_contract_matches_selected_schema(
    boundary: &terminal_psi::BoundaryMachineDeclaration,
    plan: &effects::provider_plan::ProviderPlan,
    requirement: &str,
) -> bool {
    let Some(requirement_id) = plan
        .schema
        .declaration
        .identities
        .id_at(requirement.as_ref())
    else {
        return false;
    };
    let mut matching = plan
        .schema
        .declaration
        .methods
        .iter()
        .filter(|method| method.requirement_identity == requirement_id);
    matching.next().is_some_and(|method| {
        method.completion == boundary.completion
            && boundary.unconditional_termination
                == (method.terminates_guarantee && method.termination_premises.is_empty())
    }) && matching.next().is_none()
}

fn compiler_builtin_completion(execution: CompilerBuiltinExecution) -> BoundaryCompletion {
    match execution {
        CompilerBuiltinExecution::HostedExitProcessI32 => {
            BoundaryCompletion::External(ExternalCompletionKind::ProcessExit)
        }
        CompilerBuiltinExecution::HostedWriteByteI32
        | CompilerBuiltinExecution::HostedReadByte
        | CompilerBuiltinExecution::ProgramImageRetainedSuffixStart
        | CompilerBuiltinExecution::ProgramStorageEntry
        | CompilerBuiltinExecution::StaticInterrupt { .. }
        | CompilerBuiltinExecution::AccessLifecycle { .. }
        | CompilerBuiltinExecution::EmptyRunRelease
        | CompilerBuiltinExecution::NearestIeeeFloatArithmetic { .. }
        | CompilerBuiltinExecution::AccessorScalarAccess { .. } => BoundaryCompletion::NormalReturn,
    }
}

fn float_arithmetic_realization(
    realization: NearestIeeeFloatArithmeticRealization,
    target: target::NativeTarget,
    boundary: &terminal_psi::BoundaryMachineDeclaration,
    structural_types: &[terminal_psi::StructuralTypeDeclaration],
) -> Result<NearestIeeeFloatArithmeticRealization, String> {
    if !realization.supports_target(target) {
        return Err(format!(
            "has no local target catalog realization on {target:?}"
        ));
    }
    float_arithmetic_signature(realization, boundary, structural_types)
}

fn float_arithmetic_signature(
    realization: NearestIeeeFloatArithmeticRealization,
    boundary: &terminal_psi::BoundaryMachineDeclaration,
    structural_types: &[terminal_psi::StructuralTypeDeclaration],
) -> Result<NearestIeeeFloatArithmeticRealization, String> {
    if realization.operation == target_operations::IeeeFloatArithmeticOperation::Classify {
        let terminal_psi::BoundaryMachineResult::Structural(result) = &boundary.result else {
            return Err("does not match its structural classification result".to_owned());
        };
        let mut declarations = structural_types
            .iter()
            .filter(|declaration| declaration.id == result.structural_type);
        if declarations
            .next()
            .and_then(target_operations::conventional_float_class_layout)
            .is_none()
            || declarations.next().is_some()
            || boundary.scalar_parameters
                != [semantic_vocabulary::ScalarType::IeeeFloat(
                    realization.format,
                )]
            || !boundary.structural_parameters.is_empty()
            || result.multiplicity != terminal_psi::StructuralMultiplicity::Unrestricted
            || !result.qualifications.is_empty()
            || !result.projected_qualifications.is_empty()
        {
            return Err("does not match its structural classification result".to_owned());
        }
        return Ok(realization);
    }
    let terminal_psi::BoundaryMachineResult::Scalar(result) = boundary.result else {
        return Err("does not match its Terminal boundary signature".to_owned());
    };
    if !result.qualifications.is_empty()
        || !realization.signature_matches(&boundary.scalar_parameters, result.scalar_type)
        || !boundary.structural_parameters.is_empty()
    {
        return Err("does not match its Terminal boundary signature".to_owned());
    }
    Ok(realization)
}

fn access_operation(operation: TerminalAccessorKind) -> CompilerAccessorScalarOperation {
    match operation {
        TerminalAccessorKind::Read => CompilerAccessorScalarOperation::Read,
        TerminalAccessorKind::Take => CompilerAccessorScalarOperation::Take,
        TerminalAccessorKind::Write => CompilerAccessorScalarOperation::Write,
    }
}

fn validate_access_schema(
    module: &TerminalModule,
    boundary: &BoundaryMachineDeclaration,
    plan: &ProviderPlan,
    schema: &ServiceSchema,
    demand: &AccessorScalarBoundaryDemand,
) -> Result<(), String> {
    let invalid = || {
        "placed scalar selected schema does not match its Terminal signature and effects".to_owned()
    };
    let [method] = schema.declaration.methods.as_slice() else {
        return Err(invalid());
    };
    let requirement = module
        .identities
        .text(boundary.identity)
        .ok_or_else(invalid)?;
    let accessor = module
        .structural_type(demand.accessor)
        .and_then(|declaration| module.identities.text(declaration.identity))
        .filter(|identity| !identity.is_empty())
        .ok_or_else(invalid)?;
    let borrow = match demand.access {
        StructuralAccess::SharedBorrow => "ref",
        StructuralAccess::MutableBorrow => "ref-mut",
        _ => return Err(invalid()),
    };
    let (scalar, logical_bits) = primitive_access_carrier(demand.scalar)?;
    let mut parameters = vec![format!("{borrow}({accessor})")];
    let name = match demand.operation {
        TerminalAccessorKind::Read => "read",
        TerminalAccessorKind::Take => "take",
        TerminalAccessorKind::Write => {
            parameters.push(scalar.clone());
            "write"
        }
    };
    let has_result = demand.operation != TerminalAccessorKind::Write;
    if schema
        .declaration
        .identities
        .text(method.requirement_identity)
        != requirement
        || method.name != name
        || schema
            .declaration
            .identities
            .text(schema.declaration.trait_name)
            != format!("{}::{name}", method.requirement_owner)
        || schema.declaration.trait_package_identity != plan.origin_package_identity
        || method.requirement_owner_package_identity != schema.declaration.trait_package_identity
        || !schema.declaration.receiver_applications.is_empty()
        || method.parameter_count != parameters.len()
        || method.parameter_type_identities.len() != parameters.len()
        || !method
            .parameter_type_identities
            .iter()
            .zip(&parameters)
            .all(|(actual, expected)| schema.declaration.identities.text(*actual) == expected)
        || method.has_result != has_result
        || if has_result {
            schema
                .declaration
                .identities
                .text(method.result_type_identity)
                != scalar
        } else {
            method.result_type_identity != Default::default()
        }
        || method.completion != boundary.completion
        || method.terminates_guarantee != boundary.unconditional_termination
        || !method.termination_premises.is_empty()
        || !method.entry_claims.is_empty()
        || !method.result_claims.is_empty()
        || !method.service_reach.is_empty()
        || !method.synchronous_invocations.is_empty()
        || !boundary.requires.is_empty()
        || !boundary.scalar_requires.is_empty()
        || !boundary.scalar_ensures.is_empty()
        || !boundary.normal_qualifications.is_empty()
        || !boundary.program_local_root_introductions.is_empty()
        || !boundary.content_guarantees.is_empty()
        || !boundary.crash_routes.is_empty()
        || !boundary.fixed_service_reach.is_empty()
        || !boundary.published_service_ceiling.is_empty()
        || boundary.owning_service.is_some()
        || method.may_suspend
        || method.may_block
        || !schema
            .calling_for_requirement(method.requirement_identity)
            .is_empty()
    {
        return Err(invalid());
    }
    let placement = module
        .access_view_plans
        .iter()
        .find(|placement| placement.carrier == demand.carrier)
        .ok_or_else(invalid)?;
    let entry = placement
        .placement
        .access()
        .plan()
        .entries()
        .get(demand.access_slot as usize)
        .ok_or_else(invalid)?;
    let descriptor = placement.placement.access().field_descriptor(entry.key());
    let complete = if let Some(descriptor) = descriptor {
        matches!(descriptor.logical_extent().fragments(), [fragment]
        if fragment.source_bit_offset() == 0
            && fragment.layout_bit_offset() == descriptor.container_byte_offset() * 8
            && fragment.width_bits() == u64::from(descriptor.transfer_width_bits())
            && fragment.width_bits() == logical_bits)
    } else {
        placement
            .placement
            .access()
            .field_region(entry.key())
            .is_some()
            && u64::from(demand.transfer_width_bits) == logical_bits
    };
    if !complete {
        return Err("native placed scalar access requires a complete scalar container; fragmented or narrowed codecs require an explicit native realization".to_owned());
    }
    Ok(())
}

fn primitive_access_carrier(scalar: ScalarType) -> Result<(String, u64), String> {
    let (name, bits) = match scalar {
        ScalarType::Boolean => ("bool".to_owned(), 8),
        ScalarType::Integer(integer) if integer.is_address() => {
            ("addr".to_owned(), u64::from(integer.bits()))
        }
        ScalarType::Integer(integer) => (
            format!(
                "{}{}",
                if integer.sign() == IntegerSign::Signed {
                    "i"
                } else {
                    "u"
                },
                integer.bits()
            ),
            u64::from(integer.bits()),
        ),
        ScalarType::IeeeFloat(IeeeFloatFormat::Binary32) => ("f32".to_owned(), 32),
        ScalarType::IeeeFloat(IeeeFloatFormat::Binary64) => ("f64".to_owned(), 64),
        ScalarType::Callable(_) => {
            return Err(
                "native placed scalar access cannot carry local callable descriptors".to_owned(),
            );
        }
    };
    Ok((format!("named(name({name}))"), bits))
}

fn require_access_backing_origins(
    input: &NativeRealizationInput,
    boundary: semantic_vocabulary::BoundaryMachineId,
) -> Result<(), String> {
    use std::collections::BTreeSet;
    let module = input.module();
    let reachable = input
        .plan()
        .functions
        .iter()
        .map(|function| function.machine)
        .collect::<BTreeSet<_>>();
    let mut selected_calls = Vec::new();
    for operation in module
        .machines
        .iter()
        .filter(|machine| reachable.contains(&machine.id))
        .flat_map(|machine| &machine.blocks)
        .flat_map(|block| &block.operations)
    {
        let terminal_psi::OperationKind::BoundaryCall { boundary: head, .. } = &operation.kind
        else {
            continue;
        };
        if selected_calls.iter().any(|(previous, _)| previous == head) {
            continue;
        }
        if let Some(terminal_psi::TargetMachineCallable::Machine(member)) =
            input.target_family_member(*head)?
        {
            selected_calls.push((*head, member));
        }
    }
    let graph = terminal_semantics::descriptor_results::terminal_access_origin_graph(
        module,
        &selected_calls,
    )
    .ok_or_else(|| "placed RAM origin graph is incomplete".to_owned())?;
    let mut origins = terminal_semantics::AccessRamOriginAnalysis::new(
        module,
        &graph,
        &reachable,
        input.plan().entry,
    )
    .with_private_entry_roots(
        input
            .private_entry_ram()
            .iter()
            .map(|recipe| recipe.input().clone()),
    );
    let mut occurrences = 0;
    for machine in module
        .machines
        .iter()
        .filter(|machine| reachable.contains(&machine.id))
    {
        for block in &machine.blocks {
            for (position, operation) in block.operations.iter().enumerate() {
                let terminal_psi::OperationKind::BoundaryCall {
                    boundary: actual,
                    structural_arguments,
                    ..
                } = &operation.kind
                else {
                    continue;
                };
                if *actual != boundary {
                    continue;
                }
                occurrences += 1;
                let position = u32::try_from(position)
                    .map_err(|_| "placed RAM occurrence exceeds representation".to_owned())?;
                if origins
                    .boundary_formations(
                        machine.id,
                        (block.id, position),
                        boundary,
                        structural_arguments,
                    )
                    .is_none()
                {
                    return Err(format!(
                        "placed scalar native access requires captured RAM or a checked resident formation on every entry-relative origin; a formal or host pointer is not backing authority (machine {:?}, operation {:?}, boundary {:?}, arguments {:?})",
                        machine.id, operation.id, boundary, structural_arguments
                    ));
                }
            }
        }
    }
    if occurrences == 0 {
        return Err("placed scalar native admission has no actual boundary occurrence".to_owned());
    }
    Ok(())
}
