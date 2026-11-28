//! A step invokes only the worker that issued this ready owner. The result
//! returns its backing on completion or a suspended owner of that same worker.
use application_identity::conformance_identity::{
    MachineOperandSelectionIdentity as Operand, requirement_application_commitment,
};
use language_core::sealed_intrinsics::ActivationOperation;
use semantic_vocabulary::{
    BoundaryMachineId, StructuralCaseId, StructuralFieldId, StructuralTypeId,
};
use terminal_psi::{
    BoundaryMachineDeclaration, BoundaryMachineResult, BoundaryParameterKind, ClosedReachArgument,
    StructuralAccess, StructuralFieldDeclaration, StructuralFieldType, StructuralMultiplicity,
    StructuralTypeShape, TerminalActivationStorage, TerminalMachineResult, TerminalModule,
    TerminalPreparedActivationEntry,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StepShape {
    pub selected: TerminalPreparedActivationEntry,
    pub ready_owner: StructuralTypeId,
    pub result: StructuralTypeId,
    pub returned: StructuralCaseId,
    pub value: StructuralFieldId,
    pub retained: StructuralFieldId,
    pub parked: StructuralCaseId,
    pub owner: StructuralFieldId,
    pub suspended_owner: StructuralTypeId,
}

/// `preparation` comes from the execution owner, not from a search for
/// workers with compatible signatures. This query supplies no storage.
pub fn step_shape(
    module: &TerminalModule,
    boundary: &BoundaryMachineDeclaration,
    preparation: BoundaryMachineId,
) -> Result<StepShape, &'static str> {
    let prepare = module
        .boundary_machine(preparation)
        .ok_or("step's preparation is absent")?;
    let prepared = super::prepare_shape(module, prepare)?;
    let selected = prepared.selected;
    if selected.storage != TerminalActivationStorage::Fixed {
        return Err("step_fixed cannot consume another storage plan");
    }
    let checked = boundary
        .declaration
        .as_ref()
        .ok_or("step needs a checked declaration")?;
    let source = &checked.source;
    let closed = source
        .specialization
        .as_ref()
        .ok_or("step needs a closed worker")?;
    let text = |id| {
        module
            .identities
            .text(id)
            .filter(|s| !s.is_empty())
            .ok_or("step has an absent identity")
    };
    if checked.execution
        != terminal_psi::CheckedBoundaryExecution::OwnedActivation(ActivationOperation::StepFixed)
        || boundary.kind != terminal_psi::BoundaryRequirementKind::Machine
        || boundary.completion != terminal_psi::BoundaryCompletion::NormalReturn
        || boundary.owning_service.is_some()
        || boundary.service_reach_is_installation_bound
        || boundary.attachment.is_some()
        || boundary.numeric_intrinsic.is_some()
        || text(closed.template.identity())? != super::source_owner("step_fixed")
        || text(closed.template_entry.identity())? != super::source_owner("step_fixed::entry")
        || closed.capture.is_some()
        || source.machine != boundary.owner
        || source.callable != boundary.identity
        || checked.service_declaration != source.contract
        || source.published_may_suspend
        || !source.published_may_block
        || boundary.unconditional_termination
    {
        return Err("step differs from its sealed core declaration");
    }
    let [
        ClosedReachArgument::Type(args),
        ClosedReachArgument::Type(out),
        worker @ ClosedReachArgument::Machine { identity, .. },
    ] = checked.method_arguments.as_slice()
    else {
        return Err("step needs the exact argument, result and worker binders");
    };
    let operands = [
        Operand::Type(text(*args)?.into()),
        Operand::Type(text(*out)?.into()),
        Operand::Machine(text(*identity)?.into()),
    ];
    if requirement_application_commitment(text(closed.template_callable)?, &operands)?
        != closed.commitment
        || prepare
            .declaration
            .as_ref()
            .and_then(|d| d.method_arguments.get(..3))
            != Some(checked.method_arguments.as_slice())
    {
        return Err("step and preparation selected different applications");
    }
    let body = worker
        .checked_body(module, selected.entry_machine)
        .ok_or("step selected a different worker body")?;
    if !effects_covered(boundary, &body.contract) {
        return Err("step does not cover the worker's published effects");
    }
    let [input] = boundary.structural_parameters.as_slice() else {
        return Err("step consumes one ready owner");
    };
    if !boundary.scalar_parameters.is_empty()
        || boundary.parameter_order != [BoundaryParameterKind::Structural]
        || input.position != 0
        || input.access != StructuralAccess::Owned
        || input.multiplicity != StructuralMultiplicity::Linear
        || input.structural_type != prepared.owner_type
        || !input.qualifications.is_empty()
        || !input.projected_qualifications.is_empty()
        || !input.additional_input_qualifications.is_empty()
    {
        return Err("step must consume this preparation's exact ready owner");
    }
    let BoundaryMachineResult::Structural(output) = &boundary.result else {
        return Err("step must return its outcome");
    };
    let result = module
        .structural_type(output.structural_type)
        .ok_or("step's result type is absent")?;
    let StructuralTypeShape::Sum { cases } = &result.shape else {
        return Err("step must return its declared cases");
    };
    if text(result.declaration_identity)? != "toolchain::ActivationStep"
        || cases.len() != 2
        || output.multiplicity != result.multiplicity
        || !output.qualifications.is_empty()
        || !output.projected_qualifications.is_empty()
    {
        return Err("step has a foreign or incomplete result");
    }
    let case = |name| {
        cases
            .iter()
            .find(|case| text(case.declaration_identity) == Ok(name))
            .ok_or("step is missing a declared case")
    };
    let returned = case("toolchain::ActivationStep::Returned")?;
    let parked = case("toolchain::ActivationStep::Parked")?;
    if returned.fields.len() != 2 || parked.fields.len() != 1 {
        return Err("step changed a case's fields");
    }
    let field = |fields: &[StructuralFieldDeclaration], name| {
        fields
            .iter()
            .position(|f| text(f.declaration_identity) == Ok(name) && f.storage.is_none())
            .ok_or("step is missing a declared runtime field")
    };
    let value = &returned.fields[field(
        &returned.fields,
        "toolchain::ActivationStep::Returned::value",
    )?];
    let retained = &returned.fields[field(
        &returned.fields,
        "toolchain::ActivationStep::Returned::retained",
    )?];
    let owner = &parked.fields[field(&parked.fields, "toolchain::ActivationStep::Parked::owner")?];
    let original = module
        .structural_type(prepared.result)
        .and_then(|ty| {
            ty.shape.fields_with_case().find(|(case, f)| {
                case.is_some_and(|c| c.id == prepared.rejected) && f.id == prepared.retained
            })
        })
        .map(|(_, f)| f)
        .ok_or("preparation's retained field is absent")?;
    if retained.relevance.is_erased()
        || owner.relevance.is_erased()
        || retained.field_type != original.field_type
        || retained.inline_carrier != original.inline_carrier
    {
        return Err("step changed the retained backing carrier");
    }
    let value_matches = match &body.result {
        TerminalMachineResult::Scalar(s) => {
            !value.relevance.is_erased()
                && value.field_type.canonical_leaf_shape()
                    == Some(StructuralTypeShape::PrimitiveScalar(s.scalar_type))
        }
        TerminalMachineResult::Structural(s) => {
            !value.relevance.is_erased()
                && (value.field_type == StructuralFieldType::Structural(s.structural_type)
                    || module.structural_type(s.structural_type).is_some_and(|t| {
                        value.field_type.canonical_leaf_shape().as_ref() == Some(&t.shape)
                    }))
        }
        TerminalMachineResult::Unit => {
            let unit = application_identity::type_identity::encoding::bare(
                application_identity::type_identity::vocabulary::TypeForm::Unit,
            );
            let id = match value.field_type {
                StructuralFieldType::Erased { type_identity, scalar: None } => Some(type_identity),
                StructuralFieldType::Structural(id) => module.structural_type(id).filter(|t| matches!(&t.shape, StructuralTypeShape::Record { fields, .. } if fields.is_empty())).map(|t| t.identity),
                _ => None,
            };
            id.is_some_and(|id| text(id) == Ok(unit.as_str()))
        }
    };
    if !value_matches {
        return Err("step's returned value differs from the worker result");
    }
    let token = owner
        .field_type
        .as_structural()
        .and_then(|id| module.structural_type(id))
        .ok_or("parked owner is absent")?;
    if text(token.declaration_identity)? != "toolchain::SuspendedActivation"
        || !token.owns_linear_claim
        || token.multiplicity != StructuralMultiplicity::Linear
    {
        return Err("a parked step must retain the core linear suspended owner");
    }
    let ready = module
        .structural_type(input.structural_type)
        .ok_or("ready owner is absent")?;
    if ready.shape != token.shape {
        return Err("parking changed the execution owner's private carrier");
    }
    let arguments = |id, family| {
        application_identity::type_identity::named_application_arguments(
            text(id)?,
            &super::toolchain_name(family),
        )
        .map_err(|_| "step has a malformed owner application")?
        .ok_or("step has a foreign owner application")
    };
    let ready_arguments = arguments(ready.identity, "ReadyActivation")?;
    if ready_arguments != arguments(token.identity, "SuspendedActivation")?
        || ready_arguments != arguments(result.identity, "ActivationStep")?
    {
        return Err("step changed the owner's storage, worker or type arguments");
    }
    Ok(StepShape {
        selected,
        ready_owner: prepared.owner_type,
        result: result.id,
        returned: returned.id,
        value: value.id,
        retained: retained.id,
        parked: parked.id,
        owner: owner.id,
        suspended_owner: token.id,
    })
}

fn effects_covered(
    boundary: &BoundaryMachineDeclaration,
    worker: &terminal_psi::MachineContract,
) -> bool {
    worker.published_service_ceiling.iter().all(|id| {
        boundary.fixed_service_reach.contains(id) && boundary.published_service_ceiling.contains(id)
    }) && worker
        .published_synchronous_entries
        .iter()
        .all(|id| boundary.published_synchronous_entries.contains(id))
        && worker.crash_routes.iter().all(|route| {
            boundary.crash_routes.iter().any(|allowed| {
                route.cause == allowed.cause
                    && allowed
                        .alternatives
                        .contains(&terminal_psi::CrashRouteGuard::Truth)
            })
        })
}
