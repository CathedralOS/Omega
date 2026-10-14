//! Native builtin admission: derivation (`proposals`) binds each demanded
//! boundary to its one selected intrinsic row, plan and Terminal boundary
//! once; `retained` carries that binding to admission by plan index; this
//! file admits it through the receiving policy and the local target catalog.
//!
//! Hosted builtins settle as call-shaped rows. Selected float arithmetic,
//! minimum, maximum, negation, square root, NaN/infinity classification,
//! two-rounding multiply-then-add,
//! conversion into a float format and Exact or Saturating conversion to an integer settle as
//! `NearestIeeeFloatArithmetic`, which stage 02 realizes inline as one IEEE
//! semantic operation (not necessarily one machine instruction): admission
//! requires the boundary's exact
//! signature for that operation and format (`(format, format) -> format`,
//! `(format) -> format`, `(other) -> format`, `(integer) -> format` or
//! `(format) -> integer`, `(format) -> bool`, or `(format, format, format) -> format`), so a row
//! selected for another format, carrier or
//! shape cannot stand in.

mod placed_schema;
mod proposals;
mod retained;

pub use proposals::CompilerIntrinsicPlanProvenance;
pub(crate) use proposals::{
    demanded_boundary_identities, derive_selected_intrinsic_settlement_proposals,
};
pub(crate) use retained::prepare_retained_compiler_builtins;
pub use retained::{TerminalCompilerBuiltinProposal, TerminalCompilerBuiltinSettlement};

use crate::provider_admission::AdmittedTerminalMechanism;
use crate::provider_admission::{NativeRealizationInput, ProviderAdmissionRequest};
use crate::{AdmittedBoundaryExecution, AdmittedBoundarySettlement};
use diagnostics::Diagnostic;
use effects::CompilerIntrinsicExecutionIdentity;
use target_operations::{
    BoundarySettlementRealization, CompilerBuiltinExecution, HostedExitProcessI32Realization,
    HostedReadByteRealization, HostedWriteByteI32Realization,
    NearestIeeeFloatArithmeticRealization,
};

/// Admit each derived builtin proposal through the receiving policy and the
/// local target catalog. Derivation already bound every proposal to its one
/// selected intrinsic row, exact plan and Terminal boundary in this compile,
/// so admission reads those bindings instead of rejoining them
/// (pipeline.md, "Verification placement"). The boundary contract, receiving
/// classification and target support are decided here and nowhere else.
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
    // Every executed row derivation could not close is reported together:
    // the roster was scoped to this entry's executed boundaries before it
    // arrived, so each unsettled proposal here names a boundary this entry
    // actually calls.
    let unsettled = request
        .compiler_builtins
        .iter()
        .filter_map(|proposal| {
            let requirement = proposal.requirement_identity;
            let plan = &proposal.provider_plan.name;
            match proposal.settlement {
                TerminalCompilerBuiltinSettlement::Execution(_) => None,
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
    let mut admitted = Vec::with_capacity(request.compiler_builtins.len());
    let mut mechanisms = Vec::with_capacity(request.compiler_builtins.len());
    for proposal in request.compiler_builtins {
        let requirement = proposal.requirement_identity;
        let TerminalCompilerBuiltinSettlement::Execution(execution) = proposal.settlement else {
            unreachable!("unsettled compiler-builtin proposals were refused above");
        };
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
            || boundary.completion != compiler_builtin_completion(execution)
        {
            return Err(vec![Diagnostic::error(format!(
                "compiler-builtin completion for `{requirement}` disagrees with its exact requirement contract"
            ))]);
        }
        let mechanism = compiler_intrinsic_execution_identity(execution);
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
            CompilerBuiltinExecution::PlacedScalarAccess {
                contract,
                placement_commitment,
                access_slot,
                operation,
            } => {
                let module = input.context().module();
                let demand = terminal_semantics::placed_scalar_boundary_demand(
                    &module.structural_types,
                    &module.identities,
                    &module.placed_view_plans,
                    &module.boundary_machines,
                    boundary.id,
                )
                .map_err(|reason| vec![Diagnostic::error(reason)])?;
                placed_schema::validate(module, boundary, proposal.provider_plan, &demand)
                    .map_err(|reason| vec![Diagnostic::error(reason)])?;
                require_captured_ram_origins(input, boundary.id)
                    .map_err(|reason| vec![Diagnostic::error(reason)])?;
                let compiler_operation = placed_operation(operation);
                if demand.placement_commitment != placement_commitment
                    || demand.access_slot != access_slot
                    || demand.operation != operation
                    || operation == terminal_psi::TerminalPlacedAccessorKind::Take
                    || effects::compiler_placed_scalar_contract_identity(
                        proposal.provider_plan.schema.identity_digest(),
                        placement_commitment,
                        access_slot,
                        compiler_operation,
                    ) != contract
                {
                    return Err(vec![Diagnostic::error(
                        "placed scalar execution substituted its sealed operation or requested unsupported destructive take",
                    )]);
                }
                target_operations::PlacedScalarAccessRealization { contract, demand }.into()
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
                // Only an x86 FMA carries the admitted feature custody; the
                // catalog refuses it anywhere else.
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
            execution: AdmittedBoundaryExecution::CompilerBuiltin(execution),
            realization: BoundarySettlementRealization::Builtin(realization),
        });
        mechanisms.push(AdmittedTerminalMechanism {
            boundary: boundary.id,
            mechanism: mechanism.into(),
        });
    }
    Ok((admitted, mechanisms))
}

/// Compare the retained requirement premise with the exact selected schema row.
/// Opaque foreign code is admitted under this premise; a display name or service
/// ceiling cannot supply it. Closed intrinsic executions additionally check their
/// independently known behavior below.
pub(super) fn boundary_contract_matches_selected_schema(
    boundary: &terminal_psi::BoundaryMachineDeclaration,
    plan: &effects::provider_plan::ProviderPlan,
    requirement: &str,
) -> bool {
    let mut matching = plan.schema.methods.iter().filter(|method| {
        plan.schema.identities.id_at(requirement.as_ref()) == Some(method.requirement_identity)
    });
    matching.next().is_some_and(|method| {
        method.completion == boundary.completion
            && boundary.unconditional_termination == method.terminates_guarantee
            && method.termination_premises.is_empty()
    }) && matching.next().is_none()
}

fn compiler_builtin_completion(
    execution: CompilerBuiltinExecution,
) -> semantic_vocabulary::BoundaryCompletion {
    match execution {
        CompilerBuiltinExecution::HostedExitProcessI32 => {
            semantic_vocabulary::BoundaryCompletion::External(
                semantic_vocabulary::ExternalCompletionKind::ProcessExit,
            )
        }
        CompilerBuiltinExecution::HostedWriteByteI32
        | CompilerBuiltinExecution::HostedReadByte
        | CompilerBuiltinExecution::NearestIeeeFloatArithmetic { .. }
        | CompilerBuiltinExecution::PlacedScalarAccess { .. } => {
            semantic_vocabulary::BoundaryCompletion::NormalReturn
        }
    }
}

/// Admit one selected float execution through the receiving target catalog.
/// The realization must denote exactly the Terminal boundary's signature:
/// the operation's operand count and operand type and one scalar result of
/// the execution's own format, with no structural lane. A row selected for
/// another format, integer carrier or operand shape cannot stand in for this
/// boundary.
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
    if !realization.signature_matches(&boundary.scalar_parameters, result)
        || !boundary.structural_parameters.is_empty()
    {
        return Err("does not match its Terminal boundary signature".to_owned());
    }
    Ok(realization)
}

fn compiler_intrinsic_execution_identity(
    execution: CompilerBuiltinExecution,
) -> CompilerIntrinsicExecutionIdentity {
    match execution {
        CompilerBuiltinExecution::PlacedScalarAccess {
            contract,
            placement_commitment,
            access_slot,
            operation,
        } => CompilerIntrinsicExecutionIdentity::PlacedScalarAccess {
            contract,
            placement_commitment,
            access_slot,
            operation: placed_operation(operation),
        },

        CompilerBuiltinExecution::HostedExitProcessI32 => {
            CompilerIntrinsicExecutionIdentity::HostedExitProcessI32
        }
        CompilerBuiltinExecution::HostedWriteByteI32 => {
            CompilerIntrinsicExecutionIdentity::HostedWriteByteI32
        }
        CompilerBuiltinExecution::HostedReadByte => {
            CompilerIntrinsicExecutionIdentity::HostedReadByte
        }
        CompilerBuiltinExecution::NearestIeeeFloatArithmetic { operation, format } => {
            use effects::CompilerNumericType as Numeric;
            use effects::CompilerPrimitiveFloatBinaryOperation as Primitive;
            use target_operations::IeeeFloatArithmeticOperation as Arithmetic;
            use target_operations::IeeeFloatIntegerCarrier as Integer;
            let (float_format, numeric, other) = match format {
                semantic_vocabulary::IeeeFloatFormat::Binary32 => (
                    numerics::literals::FloatFormat::F32,
                    Numeric::F32,
                    Numeric::F64,
                ),
                semantic_vocabulary::IeeeFloatFormat::Binary64 => (
                    numerics::literals::FloatFormat::F64,
                    Numeric::F64,
                    Numeric::F32,
                ),
            };
            let binary = |operation| CompilerIntrinsicExecutionIdentity::PrimitiveFloatBinary {
                operation,
                format: float_format,
            };
            let conversion = |source| CompilerIntrinsicExecutionIdentity::NamedFloatConversion {
                source,
                target: numeric,
                domain: numerics::arithmetic::ArithmeticDomain::Exact,
            };
            let carrier = |carrier| match carrier {
                Integer::I8 => Numeric::I8,
                Integer::I16 => Numeric::I16,
                Integer::I32 => Numeric::I32,
                Integer::I64 => Numeric::I64,
                Integer::U8 => Numeric::U8,
                Integer::U16 => Numeric::U16,
                Integer::U64 => Numeric::U64,
                Integer::U32 => Numeric::U32,
            };
            let builtin = CompilerIntrinsicExecutionIdentity::BuiltinFunction;
            match operation {
                Arithmetic::DirectedBinary {
                    operation,
                    rounding,
                } => builtin(match (operation, rounding, format) {
                    (
                        target_operations::IeeeFloatBinaryOperation::Add,
                        target_operations::IeeeFloatRoundingDirection::TowardZero,
                        semantic_vocabulary::IeeeFloatFormat::Binary32,
                    ) => symbols::BuiltinFunction::FloatAddTowardZeroF32,
                    (
                        target_operations::IeeeFloatBinaryOperation::Add,
                        target_operations::IeeeFloatRoundingDirection::TowardZero,
                        semantic_vocabulary::IeeeFloatFormat::Binary64,
                    ) => symbols::BuiltinFunction::FloatAddTowardZeroF64,
                    (
                        target_operations::IeeeFloatBinaryOperation::Add,
                        target_operations::IeeeFloatRoundingDirection::TowardPositive,
                        semantic_vocabulary::IeeeFloatFormat::Binary32,
                    ) => symbols::BuiltinFunction::FloatAddTowardPositiveF32,
                    (
                        target_operations::IeeeFloatBinaryOperation::Add,
                        target_operations::IeeeFloatRoundingDirection::TowardPositive,
                        semantic_vocabulary::IeeeFloatFormat::Binary64,
                    ) => symbols::BuiltinFunction::FloatAddTowardPositiveF64,
                    (
                        target_operations::IeeeFloatBinaryOperation::Add,
                        target_operations::IeeeFloatRoundingDirection::TowardNegative,
                        semantic_vocabulary::IeeeFloatFormat::Binary32,
                    ) => symbols::BuiltinFunction::FloatAddTowardNegativeF32,
                    (
                        target_operations::IeeeFloatBinaryOperation::Add,
                        target_operations::IeeeFloatRoundingDirection::TowardNegative,
                        semantic_vocabulary::IeeeFloatFormat::Binary64,
                    ) => symbols::BuiltinFunction::FloatAddTowardNegativeF64,
                    (
                        target_operations::IeeeFloatBinaryOperation::Subtract,
                        target_operations::IeeeFloatRoundingDirection::TowardZero,
                        semantic_vocabulary::IeeeFloatFormat::Binary32,
                    ) => symbols::BuiltinFunction::FloatSubtractTowardZeroF32,
                    (
                        target_operations::IeeeFloatBinaryOperation::Subtract,
                        target_operations::IeeeFloatRoundingDirection::TowardZero,
                        semantic_vocabulary::IeeeFloatFormat::Binary64,
                    ) => symbols::BuiltinFunction::FloatSubtractTowardZeroF64,
                    (
                        target_operations::IeeeFloatBinaryOperation::Subtract,
                        target_operations::IeeeFloatRoundingDirection::TowardPositive,
                        semantic_vocabulary::IeeeFloatFormat::Binary32,
                    ) => symbols::BuiltinFunction::FloatSubtractTowardPositiveF32,
                    (
                        target_operations::IeeeFloatBinaryOperation::Subtract,
                        target_operations::IeeeFloatRoundingDirection::TowardPositive,
                        semantic_vocabulary::IeeeFloatFormat::Binary64,
                    ) => symbols::BuiltinFunction::FloatSubtractTowardPositiveF64,
                    (
                        target_operations::IeeeFloatBinaryOperation::Subtract,
                        target_operations::IeeeFloatRoundingDirection::TowardNegative,
                        semantic_vocabulary::IeeeFloatFormat::Binary32,
                    ) => symbols::BuiltinFunction::FloatSubtractTowardNegativeF32,
                    (
                        target_operations::IeeeFloatBinaryOperation::Subtract,
                        target_operations::IeeeFloatRoundingDirection::TowardNegative,
                        semantic_vocabulary::IeeeFloatFormat::Binary64,
                    ) => symbols::BuiltinFunction::FloatSubtractTowardNegativeF64,
                    (
                        target_operations::IeeeFloatBinaryOperation::Multiply,
                        target_operations::IeeeFloatRoundingDirection::TowardZero,
                        semantic_vocabulary::IeeeFloatFormat::Binary32,
                    ) => symbols::BuiltinFunction::FloatMultiplyTowardZeroF32,
                    (
                        target_operations::IeeeFloatBinaryOperation::Multiply,
                        target_operations::IeeeFloatRoundingDirection::TowardZero,
                        semantic_vocabulary::IeeeFloatFormat::Binary64,
                    ) => symbols::BuiltinFunction::FloatMultiplyTowardZeroF64,
                    (
                        target_operations::IeeeFloatBinaryOperation::Multiply,
                        target_operations::IeeeFloatRoundingDirection::TowardPositive,
                        semantic_vocabulary::IeeeFloatFormat::Binary32,
                    ) => symbols::BuiltinFunction::FloatMultiplyTowardPositiveF32,
                    (
                        target_operations::IeeeFloatBinaryOperation::Multiply,
                        target_operations::IeeeFloatRoundingDirection::TowardPositive,
                        semantic_vocabulary::IeeeFloatFormat::Binary64,
                    ) => symbols::BuiltinFunction::FloatMultiplyTowardPositiveF64,
                    (
                        target_operations::IeeeFloatBinaryOperation::Multiply,
                        target_operations::IeeeFloatRoundingDirection::TowardNegative,
                        semantic_vocabulary::IeeeFloatFormat::Binary32,
                    ) => symbols::BuiltinFunction::FloatMultiplyTowardNegativeF32,
                    (
                        target_operations::IeeeFloatBinaryOperation::Multiply,
                        target_operations::IeeeFloatRoundingDirection::TowardNegative,
                        semantic_vocabulary::IeeeFloatFormat::Binary64,
                    ) => symbols::BuiltinFunction::FloatMultiplyTowardNegativeF64,
                    (
                        target_operations::IeeeFloatBinaryOperation::Divide,
                        target_operations::IeeeFloatRoundingDirection::TowardZero,
                        semantic_vocabulary::IeeeFloatFormat::Binary32,
                    ) => symbols::BuiltinFunction::FloatDivideTowardZeroF32,
                    (
                        target_operations::IeeeFloatBinaryOperation::Divide,
                        target_operations::IeeeFloatRoundingDirection::TowardZero,
                        semantic_vocabulary::IeeeFloatFormat::Binary64,
                    ) => symbols::BuiltinFunction::FloatDivideTowardZeroF64,
                    (
                        target_operations::IeeeFloatBinaryOperation::Divide,
                        target_operations::IeeeFloatRoundingDirection::TowardPositive,
                        semantic_vocabulary::IeeeFloatFormat::Binary32,
                    ) => symbols::BuiltinFunction::FloatDivideTowardPositiveF32,
                    (
                        target_operations::IeeeFloatBinaryOperation::Divide,
                        target_operations::IeeeFloatRoundingDirection::TowardPositive,
                        semantic_vocabulary::IeeeFloatFormat::Binary64,
                    ) => symbols::BuiltinFunction::FloatDivideTowardPositiveF64,
                    (
                        target_operations::IeeeFloatBinaryOperation::Divide,
                        target_operations::IeeeFloatRoundingDirection::TowardNegative,
                        semantic_vocabulary::IeeeFloatFormat::Binary32,
                    ) => symbols::BuiltinFunction::FloatDivideTowardNegativeF32,
                    (
                        target_operations::IeeeFloatBinaryOperation::Divide,
                        target_operations::IeeeFloatRoundingDirection::TowardNegative,
                        semantic_vocabulary::IeeeFloatFormat::Binary64,
                    ) => symbols::BuiltinFunction::FloatDivideTowardNegativeF64,
                }),
                Arithmetic::Add => binary(Primitive::Add),
                Arithmetic::Subtract => binary(Primitive::Subtract),
                Arithmetic::Multiply => binary(Primitive::Multiply),
                Arithmetic::Divide => binary(Primitive::Divide),
                Arithmetic::Compare(comparison) => {
                    use semantic_vocabulary::IeeeFloatComparisonOperation as Relation;
                    binary(match comparison {
                        Relation::Equal => Primitive::Equal,
                        Relation::NotEqual => Primitive::NotEqual,
                        Relation::Less => Primitive::Less,
                        Relation::LessOrEqual => Primitive::LessOrEqual,
                        Relation::Greater => Primitive::Greater,
                        Relation::GreaterOrEqual => Primitive::GreaterOrEqual,
                    })
                }
                Arithmetic::Minimum => builtin(symbols::BuiltinFunction::FloatMinimum),
                Arithmetic::Maximum => builtin(symbols::BuiltinFunction::FloatMaximum),
                Arithmetic::SquareRoot => builtin(symbols::BuiltinFunction::FloatSquareRoot),
                Arithmetic::IsNan => builtin(symbols::BuiltinFunction::FloatIsNan),
                Arithmetic::IsInfinite => builtin(symbols::BuiltinFunction::FloatIsInfinite),
                Arithmetic::IsFinite => builtin(symbols::BuiltinFunction::FloatIsFinite),
                Arithmetic::IsNormal => builtin(symbols::BuiltinFunction::FloatIsNormal),
                Arithmetic::IsSubnormal => builtin(symbols::BuiltinFunction::FloatIsSubnormal),
                Arithmetic::Classify => builtin(match format {
                    semantic_vocabulary::IeeeFloatFormat::Binary32 => {
                        symbols::BuiltinFunction::FloatClassifyF32
                    }
                    semantic_vocabulary::IeeeFloatFormat::Binary64 => {
                        symbols::BuiltinFunction::FloatClassifyF64
                    }
                }),
                Arithmetic::MultiplyThenAdd => builtin(match format {
                    semantic_vocabulary::IeeeFloatFormat::Binary32 => {
                        symbols::BuiltinFunction::FloatMultiplyThenAddF32
                    }
                    semantic_vocabulary::IeeeFloatFormat::Binary64 => {
                        symbols::BuiltinFunction::FloatMultiplyThenAddF64
                    }
                }),
                Arithmetic::FusedMultiplyAdd => builtin(match format {
                    semantic_vocabulary::IeeeFloatFormat::Binary32 => {
                        symbols::BuiltinFunction::FloatFusedMultiplyAddF32
                    }
                    semantic_vocabulary::IeeeFloatFormat::Binary64 => {
                        symbols::BuiltinFunction::FloatFusedMultiplyAddF64
                    }
                }),
                Arithmetic::Negate => {
                    CompilerIntrinsicExecutionIdentity::NamedFloatNegation(float_format)
                }
                Arithmetic::ConvertFormat => conversion(other),
                Arithmetic::FromInteger(source) => conversion(carrier(source)),
                Arithmetic::TruncateToInteger { target, domain } => {
                    CompilerIntrinsicExecutionIdentity::NamedFloatConversion {
                        source: numeric,
                        target: carrier(target),
                        domain: match domain {
                            target_operations::IeeeFloatToIntegerDomain::Exact => {
                                numerics::arithmetic::ArithmeticDomain::Exact
                            }
                            target_operations::IeeeFloatToIntegerDomain::Saturating => {
                                numerics::arithmetic::ArithmeticDomain::Saturating
                            }
                            target_operations::IeeeFloatToIntegerDomain::Trapping => {
                                numerics::arithmetic::ArithmeticDomain::Trapping
                            }
                        },
                    }
                }
            }
        }
    }
}

fn placed_operation(
    operation: terminal_psi::TerminalPlacedAccessorKind,
) -> effects::CompilerPlacedScalarOperation {
    match operation {
        terminal_psi::TerminalPlacedAccessorKind::Read => {
            effects::CompilerPlacedScalarOperation::Read
        }
        terminal_psi::TerminalPlacedAccessorKind::Take => {
            effects::CompilerPlacedScalarOperation::Take
        }
        terminal_psi::TerminalPlacedAccessorKind::Write => {
            effects::CompilerPlacedScalarOperation::Write
        }
    }
}

fn require_captured_ram_origins(
    input: &NativeRealizationInput,
    boundary: semantic_vocabulary::BoundaryMachineId,
) -> Result<(), String> {
    use std::collections::BTreeSet;
    let module = input.context().module();
    let graph = terminal_semantics::descriptor_results::terminal_placed_origin_graph(module)
        .ok_or_else(|| "placed RAM origin graph is incomplete".to_owned())?;
    let reachable = input
        .plan()
        .functions
        .iter()
        .map(|function| function.machine)
        .collect::<BTreeSet<_>>();
    let mut origins = terminal_semantics::PlacedRamOriginAnalysis::new(
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
                        "placed scalar native access requires initialized captured RAM on every entry-relative origin; a formal or host pointer is not backing authority (machine {:?}, operation {:?}, boundary {:?}, arguments {:?})",
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
