//! Build stage 04 fixtures by hand: an abstract plan, the target plan stage 02
//! would store for it, and the prepared execution plan stage 05 selects from.
//!
//! Stage 04's own projection tests and stage 05's selection tests share these
//! builders, so stage 04 never links stage 05 to test its output.

use crate::preparation::BoundaryOriginFacts;
use abstract_operations::{
    AbstractBlock, AbstractFunction, AbstractFunctionResult, AbstractOperation,
    AbstractOperationPlan, AbstractResult,
};
use calling_conventions::{CallSignature, CallingPolicy};
use optimization_unit::reconstruct_psi_optimization_unit_seed;
use semantic_vocabulary::{
    BlockId, EdgeId, IntegerSign, IntegerType, MachineId, OperationId, PlaceId, ScalarType,
    StructuralCaseId, StructuralFieldId, StructuralTypeId, ValueId, WorkScheduleIdentity,
};
use target::NativeTarget;
use target_operations::{
    TargetControlBlock, TargetControlGraph, TargetControlTerminator, TargetFunction,
    TargetOperationPlan, TargetStructuralHomeLayout, TargetStructuralHomeOrigin,
    TargetStructuralHomeRequirement, TargetStructuralParameter, TargetUnitOperation,
    TerminalPsiProvenance,
};
use terminal_psi::{
    BindingRelevance, SemanticFingerprint, StructuralAccess, StructuralCaseDeclaration,
    StructuralFieldDeclaration, StructuralFieldType, StructuralMultiplicity,
    StructuralOperationResult, StructuralParameterDeclaration, StructuralTypeDeclaration,
    StructuralTypeShape, TerminalIdentityIndex, TerminalIdentityTableBuilder, TerminalModule,
    TerminalPsiIdentity, VocabularyMarker,
};

pub const HOST: fn() -> NativeTarget = NativeTarget::linux_x64;

pub fn plan(
    names: TerminalIdentityTableBuilder,
    types: Vec<StructuralTypeDeclaration>,
    function: AbstractFunction,
) -> AbstractOperationPlan {
    AbstractOperationPlan {
        psi: psi(),
        entry: machine(),
        structural_types: types.into(),
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        callables: Default::default(),
        dynamic_dispatch: Default::default(),
        identities: names.build(),
        functions: vec![function],
    }
}

pub fn function_with_body(
    structural_parameters: Vec<StructuralParameterDeclaration>,
    blocks: Vec<AbstractBlock>,
) -> AbstractFunction {
    AbstractFunction {
        machine: machine(),
        attachment: None,
        entry: block_id(1),
        parameters: Vec::new(),
        structural_parameters,
        result: AbstractFunctionResult::Unit,
        entry_claims: Vec::new(),
        block_entry_claims: Vec::new(),
        published_service_ceiling: Vec::new(),
        blocks,
    }
}

pub fn target_plan(
    plan: &AbstractOperationPlan,
    mut blocks: Vec<TargetControlBlock>,
) -> TargetOperationPlan {
    let layouts =
        crate::preparation::native_geometry::NativeGeometry::new(&plan.structural_types, HOST())
            .unwrap();
    let function = &plan.functions[0];
    let unit = reconstruct_psi_optimization_unit_seed(plan, WorkScheduleIdentity::new(1).unwrap())
        .unwrap();
    for block in &mut blocks {
        let source = unit.functions[0].block(block.block).expect("source block");
        for (position, instruction) in block.operations.iter_mut().enumerate() {
            if matches!(&instruction.kind, TargetUnitOperation::CleanupCall { .. }) {
                continue;
            }
            let node = &source.nodes[position];
            instruction.source = Some(target_operations::TargetInstructionSource {
                operation: node.operation.psi_operation().expect("source instruction"),
                result: node.definitions.first().copied(),
                metadata: target_operations::TargetOperationMetadata {
                    work: node.work.clone(),
                    effect: node.effect,
                    ownership: node.ownership.clone(),
                },
            });
        }
        let terminal = unit.functions[0]
            .block(block.block)
            .unwrap()
            .nodes
            .last()
            .unwrap();
        block.terminator_metadata = target_operations::TargetOperationMetadata {
            work: terminal.work.clone(),
            effect: terminal.effect,
            ownership: terminal.ownership.clone(),
        };
        let edge_work = |edge| {
            terminal
                .successors
                .iter()
                .find(|candidate| candidate.psi_edge == edge)
                .unwrap()
                .work
                .clone()
        };
        match &mut block.terminator {
            TargetControlTerminator::Jump { successor } => {
                successor.work = edge_work(successor.psi_edge)
            }
            TargetControlTerminator::Conditional {
                when_true,
                when_false,
                ..
            } => {
                when_true.work = edge_work(when_true.psi_edge);
                when_false.work = edge_work(when_false.psi_edge);
            }
            TargetControlTerminator::StructuralCase { cases, .. } => {
                for case in cases {
                    case.work = edge_work(case.psi_edge);
                }
            }
            _ => {}
        }
    }
    let shapes = function
        .structural_parameters
        .iter()
        .map(|parameter| {
            crate::preparation::structural_inputs::parameter_shape(parameter, &layouts)
                .expect("parameter shape")
        })
        .collect();
    let call_plan = calling_conventions::evaluate_call_plan(
        CallingPolicy::native_for_target(HOST()),
        &CallSignature {
            parameters: shapes,
            result: None,
        },
    )
    .expect("call plan");
    let parameters = function
        .structural_parameters
        .iter()
        .zip(&call_plan.parameters)
        .map(|(parameter, placement)| TargetStructuralParameter {
            place: parameter.place,
            structural_type: parameter.structural_type,
            multiplicity: parameter.multiplicity,
            access: parameter.access,
            projected_qualifications: parameter.projected_qualifications.clone(),
            shape: placement.shape,
            placement: placement.clone(),
        })
        .collect();
    TargetOperationPlan {
        publication: crate::preparation::capture_source_publication(plan).unwrap(),
        psi: plan.psi,
        target: HOST(),
        profile: None,
        entry: plan.entry,
        functions: vec![TargetFunction {
            machine: function.machine,
            attachment: None,
            scalar_abi: None,
            mixed_structural_scalar_abi: None,
            structural_contract: (!function.structural_parameters.is_empty()
                || !unit.functions[0].declared_places.is_empty())
            .then(|| target_operations::TargetStructuralContract {
                parameters: function.structural_parameters.clone(),
                places: unit.functions[0].structural_places.clone(),
                entry_claims: function.entry_claims.clone(),
                published_service_ceiling: function.published_service_ceiling.clone(),
                result: function.result.structural().cloned(),
            }),
            provenance: TerminalPsiProvenance {
                operations: Vec::new(),
                edges: Vec::new(),
            },
            graph: TargetControlGraph {
                structural_types: plan.structural_types.clone(),
                call_plan,
                scalar_parameters: Vec::new(),
                parameters,
                entry: function.entry,
                blocks,
            },
        }],
        native_callback_arguments: Vec::new(),
        borrowed_storage_layouts: Default::default(),
    }
}

pub fn abi_parameter(
    plan: &AbstractOperationPlan,
    parameter: &StructuralParameterDeclaration,
) -> TargetStructuralParameter {
    let layouts =
        crate::preparation::native_geometry::NativeGeometry::new(&plan.structural_types, HOST())
            .unwrap();
    let shape = crate::preparation::structural_inputs::parameter_shape(parameter, &layouts)
        .expect("parameter shape");
    let call_plan = calling_conventions::evaluate_call_plan(
        CallingPolicy::native_for_target(HOST()),
        &CallSignature {
            parameters: vec![shape],
            result: None,
        },
    )
    .expect("parameter call plan");
    let placement = call_plan
        .parameters
        .into_iter()
        .next()
        .expect("parameter placement");
    TargetStructuralParameter {
        place: parameter.place,
        structural_type: parameter.structural_type,
        multiplicity: parameter.multiplicity,
        access: parameter.access,
        projected_qualifications: parameter.projected_qualifications.clone(),
        shape: placement.shape,
        placement,
    }
}

pub fn block(
    id: u64,
    operations: Vec<TargetUnitOperation>,
    terminator: TargetControlTerminator,
) -> TargetControlBlock {
    TargetControlBlock {
        block: block_id(id),
        parameters: Vec::new(),
        structural_parameters: Vec::new(),
        operations: operations
            .into_iter()
            .map(|kind| target_operations::TargetInstruction { kind, source: None })
            .collect(),
        terminator,
        terminator_metadata: target_operations::TargetOperationMetadata {
            work: Vec::new(),
            effect: optimization_unit::EffectLink {
                input: 0,
                output: 0,
            },
            ownership: Vec::new(),
        },
    }
}

pub fn return_unit(id: u64) -> TargetControlTerminator {
    TargetControlTerminator::Return {
        psi_edge: edge(id),
        cleanup_actions: Vec::new(),
    }
}

pub fn home(
    operation: OperationId,
    result: &StructuralOperationResult,
    layout: TargetStructuralHomeLayout,
) -> TargetStructuralHomeRequirement {
    TargetStructuralHomeRequirement {
        origin: TargetStructuralHomeOrigin::OperationResult {
            operation,
            result: result.clone(),
        },
        layout,
    }
}

pub fn entry(id: u64, operations: Vec<AbstractOperation>) -> AbstractBlock {
    AbstractBlock {
        block: block_id(id),
        structural_parameters: Vec::new(),
        parameters: Vec::new(),
        operations,
    }
}

pub fn parameter(structural_type: StructuralTypeId) -> StructuralParameterDeclaration {
    parameter_at(1, structural_type)
}

pub fn parameter_at(id: u64, structural_type: StructuralTypeId) -> StructuralParameterDeclaration {
    StructuralParameterDeclaration {
        place: place(id),
        position: 0,
        structural_type,
        multiplicity: StructuralMultiplicity::Unrestricted,
        access: StructuralAccess::SharedBorrow,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
        additional_input_qualifications: Vec::new(),
    }
}

pub fn structural_result(
    place: PlaceId,
    structural_type: StructuralTypeId,
) -> StructuralOperationResult {
    StructuralOperationResult {
        place,
        structural_type,
        multiplicity: StructuralMultiplicity::Unrestricted,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
        additional_input_qualifications: Vec::new(),
        qualification_establishments: Vec::new(),
        claims: Vec::new(),
    }
}

pub fn sum_type(
    id: u64,
    identity: TerminalIdentityIndex,
    names: &mut TerminalIdentityTableBuilder,
    payload: &[StructuralFieldDeclaration],
) -> StructuralTypeDeclaration {
    StructuralTypeDeclaration {
        declaration_identity: Default::default(),
        quotient_carrier: None,
        id: ty(id),
        identity,
        owns_linear_claim: false,
        multiplicity: terminal_psi::StructuralMultiplicity::Unrestricted,
        no_code_affine_cleanup: true,
        descriptor_cleanup: false,
        nominal_cleanup: None,
        shape: StructuralTypeShape::Sum {
            cases: vec![
                StructuralCaseDeclaration {
                    declaration_identity: Default::default(),
                    id: case_id(1),
                    identity: names.intern("Left"),
                    fields: payload.to_vec(),
                },
                StructuralCaseDeclaration {
                    declaration_identity: Default::default(),
                    id: case_id(2),
                    identity: names.intern("Right"),
                    fields: Vec::new(),
                },
            ],
        },
        scalar_restriction: None,
        requires: Vec::new(),
    }
}

pub fn reference_type(
    id: u64,
    identity: TerminalIdentityIndex,
    referent: StructuralTypeId,
) -> StructuralTypeDeclaration {
    StructuralTypeDeclaration {
        declaration_identity: Default::default(),
        quotient_carrier: None,
        id: ty(id),
        identity,
        owns_linear_claim: false,
        multiplicity: terminal_psi::StructuralMultiplicity::Unrestricted,
        no_code_affine_cleanup: true,
        descriptor_cleanup: false,
        nominal_cleanup: None,
        shape: StructuralTypeShape::Reference {
            referent,
            access: StructuralAccess::SharedBorrow,
        },
        scalar_restriction: None,
        requires: Vec::new(),
    }
}

pub fn scalar_field(
    id: u64,
    identity: TerminalIdentityIndex,
    integer: IntegerType,
) -> StructuralFieldDeclaration {
    StructuralFieldDeclaration {
        declaration_identity: Default::default(),
        inline_carrier: None,
        id: StructuralFieldId::new(id).unwrap(),
        identity,
        relevance: BindingRelevance::Relevant,
        field_type: StructuralFieldType::Scalar(ScalarType::Integer(integer)),
        storage: None,
    }
}

pub fn u32_type() -> IntegerType {
    IntegerType::new(IntegerSign::Unsigned, 32).unwrap()
}

pub fn u64_type() -> IntegerType {
    IntegerType::new(IntegerSign::Unsigned, 64).unwrap()
}

pub fn psi() -> TerminalPsiIdentity {
    TerminalPsiIdentity {
        vocabulary_marker: VocabularyMarker::CURRENT,
        program_fingerprint: SemanticFingerprint::from_bytes([9; 32]),
    }
}

pub fn machine() -> MachineId {
    MachineId::new(1).unwrap()
}

pub fn block_id(value: u64) -> BlockId {
    BlockId::new(value).unwrap()
}

pub fn place(value: u64) -> PlaceId {
    PlaceId::new(value).unwrap()
}

pub fn op(value: u64) -> OperationId {
    OperationId::new(value).unwrap()
}

pub fn edge(value: u64) -> EdgeId {
    EdgeId::new(value).unwrap()
}

pub fn val(value: u64) -> ValueId {
    ValueId::new(value).unwrap()
}

pub fn ty(value: u64) -> StructuralTypeId {
    StructuralTypeId::new(value).unwrap()
}

pub fn case_id(value: u64) -> StructuralCaseId {
    StructuralCaseId::new(value).unwrap()
}

pub fn boolean_result() -> AbstractResult {
    AbstractResult {
        value: val(1),
        scalar_type: ScalarType::Boolean,
    }
}

/// The types the fixtures' module retains: every integer width these
/// hand-built plans use, closed the way preparation closes an accepted
/// module's types.
pub fn fixture_types() -> &'static crate::preparation::native_geometry::ModuleTypes {
    static TYPES: std::sync::LazyLock<crate::preparation::native_geometry::ModuleTypes> =
        std::sync::LazyLock::new(|| {
            let integer =
                |bits| ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, bits).unwrap());
            let closed = module_retaining(&[integer(8), integer(32), integer(64)]);
            crate::preparation::accepted_types(&closed, closed.identity(), closed.module())
                .expect("a closed module supplies its own types")
        });
    &TYPES
}

/// Prepare the execution plan stage 05 selects from for a hand-built target
/// plan, with the callable invocations and evidence the unit records.
pub fn prepare_fixture_execution_plan(
    target: &TargetOperationPlan,
    unit: &optimization_unit::PsiOptimizationUnit,
    origins: BoundaryOriginFacts<'_>,
    callable_bindings: &verified_optimization_input::NativeCallableBindings,
    installed_interrupts: &provider_admission::AdmittedInstalledInterruptCohort,
    types: &crate::preparation::native_geometry::ModuleTypes,
) -> Result<
    legalized_operations::PreparedExecutionPlan,
    crate::preparation::TargetOperationAuditError,
> {
    let callable_invocations =
        crate::preparation::PreparedCallableInvocations::new(unit, callable_bindings).map_err(
            |reason| crate::preparation::LegalizationError::CallableValueProvenance { reason },
        )?;
    crate::preparation::prepare_execution_plan(
        target,
        crate::preparation::PreparationEvidence {
            optimization_unit: unit.identity,
            work_schedule: unit.work_schedule,
            accepted_obligation_facts: &unit.accepted_obligation_facts,
        },
        origins,
        &callable_invocations,
        installed_interrupts,
        types,
        // These raw-plan unit controls do not model an optimization run.
        optimization_core::PrePhysicalOptimizationManifestIdentity::from_canonical_bytes(
            b"standalone native-preparation fixture",
        ),
    )
}

/// A closed module that retains exactly these scalar types, each declared
/// as a primitive scalar, so a fixture closes types the way preparation
/// closes them against an accepted module.
fn module_retaining(scalars: &[ScalarType]) -> terminal_codec::ClosedTerminalModule {
    let mut module = TerminalModule::declarations();
    let mut identities = TerminalIdentityTableBuilder::default();
    let rows = scalars
        .iter()
        .enumerate()
        .map(|(position, scalar)| {
            let identity = identities.intern(&format!("retained_scalar_{position}"));
            (identity, *scalar)
        })
        .collect::<Vec<_>>();
    module.identities = identities.build();
    for (position, (identity, scalar)) in rows.into_iter().enumerate() {
        module
            .types
            .structural_types
            .push(StructuralTypeDeclaration {
                declaration_identity: Default::default(),
                id: StructuralTypeId::new(u64::try_from(position).unwrap() + 1).unwrap(),
                identity,
                shape: StructuralTypeShape::PrimitiveScalar(scalar),
                requires: Vec::new(),
                quotient_carrier: None,
                owns_linear_claim: false,
                no_code_affine_cleanup: false,
                multiplicity: terminal_psi::StructuralMultiplicity::Unrestricted,
                descriptor_cleanup: false,
                nominal_cleanup: None,
                scalar_restriction: None,
            });
    }
    terminal_codec::ClosedTerminalModule::new(module).unwrap()
}
