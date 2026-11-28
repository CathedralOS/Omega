//! Answering a parked activation either releases its original ready owner or
//! returns the parked owner, publication and reply to the caller unchanged.
use super::{
    ParkShape, prepare_shape, resume_permit, same_preparation, scalar_identity, stored_reply,
};
use semantic_vocabulary::{
    BoundaryMachineId, StructuralCaseId, StructuralFieldId, StructuralTypeId,
};
use terminal_psi::{
    BoundaryMachineDeclaration, BoundaryMachineResult, BoundaryParameterKind, StructuralAccess,
    StructuralCaseDeclaration, StructuralFieldDeclaration, StructuralFieldType,
    StructuralMultiplicity, StructuralTypeDeclaration, StructuralTypeShape, TerminalModule,
    TerminalPreparedActivationEntry,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResumeShape {
    pub selected: TerminalPreparedActivationEntry,
    pub ready_owner: StructuralTypeId,
    pub suspended_owner: StructuralTypeId,
    pub result: StructuralTypeId,
    pub ready: StructuralCaseId,
    pub ready_field: StructuralFieldId,
    pub mismatch: StructuralCaseId,
    pub mismatch_owner: StructuralFieldId,
    pub mismatch_permit: StructuralFieldId,
    pub mismatch_value: StructuralFieldId,
    pub permit: ParkShape,
}

fn field<'a>(
    module: &TerminalModule,
    case: &'a StructuralCaseDeclaration,
    declaration: &str,
) -> Result<&'a StructuralFieldDeclaration, &'static str> {
    case.fields
        .iter()
        .find(|field| {
            module.identities.text(field.declaration_identity) == Some(declaration)
                && field.storage.is_none()
        })
        .ok_or("resume lost a declared result field")
}

fn operands<'a>(
    module: &'a TerminalModule,
    ty: &StructuralTypeDeclaration,
    family: &str,
) -> Result<Vec<&'a str>, &'static str> {
    let identity = module
        .identities
        .text(ty.identity)
        .ok_or("resume carrier has no identity")?;
    application_identity::type_identity::named_application_operands(
        identity,
        &super::toolchain_name(family),
    )
    .map_err(|_| "resume carrier has malformed generic operands")?
    .ok_or("resume carrier belongs to another generic family")
}

fn reply_input(
    module: &TerminalModule,
    boundary: &BoundaryMachineDeclaration,
    field: &StructuralFieldDeclaration,
    reply: &str,
) -> bool {
    use BoundaryParameterKind::{Scalar, Structural};
    match (
        boundary.parameter_order.as_slice(),
        boundary.scalar_parameters.as_slice(),
        boundary.structural_parameters.as_slice(),
    ) {
        ([Structural, Structural, Scalar], [value], [_, _]) => {
            !field.relevance.is_erased()
                && scalar_identity(*value).as_deref() == Some(reply)
                && field.field_type.canonical_leaf_shape()
                    == Some(StructuralTypeShape::PrimitiveScalar(*value))
        }
        ([Structural, Structural, Structural], [], [_, _, value]) => {
            value.position == 2
                && value.access == StructuralAccess::Owned
                && module
                    .structural_type(value.structural_type)
                    .is_some_and(|ty| {
                        ty.multiplicity == value.multiplicity
                            && module.identities.text(ty.identity)
                                == Some(stored_reply(reply).as_ref())
                            && super::field_matches_carrier(module, field, ty.id)
                    })
        }
        _ => false,
    }
}

pub fn resume_shape(
    module: &TerminalModule,
    boundary: &BoundaryMachineDeclaration,
    preparation: BoundaryMachineId,
) -> Result<ResumeShape, &'static str> {
    let permit = resume_permit(module, boundary)?;
    let declaration = module
        .boundary_machine(preparation)
        .ok_or("resume has no originating preparation")?;
    let prepared = prepare_shape(module, declaration)?;
    if !same_preparation(module, boundary, declaration) {
        return Err("resume selects a different worker or storage plan from the owner");
    }
    let checked = boundary
        .declaration
        .as_ref()
        .ok_or("resume has no checked signature")?;
    let reply = module
        .identities
        .text(permit.reply)
        .ok_or("resume has no declared reply")?;
    let [input, publication, ..] = boundary.structural_parameters.as_slice() else {
        return Err("resume must take the suspended owner and its answer publication");
    };
    let ready_type = module
        .structural_type(prepared.owner_type)
        .ok_or("preparation lost its ready owner")?;
    let parked_type = module
        .structural_type(input.structural_type)
        .ok_or("resume's suspended owner is absent")?;
    if input.position != 0
        || input.access != StructuralAccess::Owned
        || input.multiplicity != StructuralMultiplicity::Linear
        || !input.qualifications.is_empty()
        || !input.projected_qualifications.is_empty()
        || !input.additional_input_qualifications.is_empty()
        || !parked_type.owns_linear_claim
        || parked_type.multiplicity != input.multiplicity
        || module.identities.text(parked_type.declaration_identity)
            != Some("toolchain::SuspendedActivation")
        || parked_type.shape != ready_type.shape
    {
        return Err("resume must consume the exact linear suspended execution owner");
    }
    let BoundaryMachineResult::Structural(output) = &boundary.result else {
        return Err("resume must return its ready or mismatch outcome");
    };
    let result = module
        .structural_type(output.structural_type)
        .ok_or("resume result is absent")?;
    let StructuralTypeShape::Sum { cases } = &result.shape else {
        return Err("resume requires a sum result");
    };
    if module.identities.text(result.declaration_identity) != Some("toolchain::ResumeAttempt")
        || result.multiplicity != output.multiplicity
        || !output.qualifications.is_empty()
        || cases.len() != 2
    {
        return Err("resume has a foreign or incomplete result");
    }
    let owner_arguments = operands(module, ready_type, "ReadyActivation")?;
    if operands(module, parked_type, "SuspendedActivation")? != owner_arguments {
        return Err("resume changes the closed arguments of the execution owner");
    }
    let result_arguments = operands(module, result, "ResumeAttempt")?;
    // The result adds the permit lifetime (if retained in these type
    // identities) and the Reply type to the original owner's telescope.
    let (arguments, result_reply, worker, plan) =
        match (owner_arguments.as_slice(), result_arguments.as_slice()) {
            ([a, r, _, _], [b, s, reply, worker, plan]) if (a, r) == (b, s) => {
                (&owner_arguments[2..], *reply, *worker, *plan)
            }
            ([storage, a, r, _, _], [backing, _, b, s, reply, worker, plan])
                if (storage, a, r) == (backing, b, s) =>
            {
                (&owner_arguments[3..], *reply, *worker, *plan)
            }
            _ => return Err("resume returned a different backing scope or worker signature"),
        };
    if arguments != [worker, plan] || (result_reply != reply && result_reply != stored_reply(reply))
    {
        return Err("resume returned different worker, storage or reply operands");
    }
    let case = |declaration| {
        cases
            .iter()
            .find(|row| module.identities.text(row.declaration_identity) == Some(declaration))
            .ok_or("resume is missing a declared case")
    };
    let ready = case("toolchain::ResumeAttempt::Ready")?;
    let mismatch = case("toolchain::ResumeAttempt::Mismatch")?;
    if ready.fields.len() != 1 || mismatch.fields.len() != 3 {
        return Err("resume changed a case's field count");
    }
    let ready_field = field(module, ready, "toolchain::ResumeAttempt::Ready::owner")?;
    let owner_field = field(
        module,
        mismatch,
        "toolchain::ResumeAttempt::Mismatch::owner",
    )?;
    let permit_field = field(
        module,
        mismatch,
        "toolchain::ResumeAttempt::Mismatch::permit",
    )?;
    let value_field = field(
        module,
        mismatch,
        "toolchain::ResumeAttempt::Mismatch::value",
    )?;
    if ready_field.relevance.is_erased()
        || owner_field.relevance.is_erased()
        || permit_field.relevance.is_erased()
        || ready_field.field_type != StructuralFieldType::Structural(prepared.owner_type)
        || owner_field.field_type != StructuralFieldType::Structural(input.structural_type)
        || permit_field.field_type != StructuralFieldType::Structural(publication.structural_type)
        || !reply_input(module, boundary, value_field, reply)
    {
        return Err("resume must return the original owner, permit and reply carriers");
    }
    if !super::forwarded_qualifications(
        &output.projected_qualifications,
        mismatch.id,
        &[
            (permit_field.identity, Some(publication)),
            (value_field.identity, boundary.structural_parameters.get(2)),
        ],
    ) {
        return Err("resume asserted an unprovided result qualification");
    }
    let source = &checked.source;
    let [declared_owner, _, _] = source.parameter_types.as_slice() else {
        return Err("resume needs the declared owner, publication and reply");
    };
    let original = super::declared_operands(
        module,
        declaration
            .declaration
            .as_ref()
            .ok_or("resume lost its preparation")?
            .source
            .result_type,
        "ActivationPreparation",
    )?;
    let returned = super::declared_operands(module, source.result_type, "ResumeAttempt")?;
    if super::declared_operands(module, *declared_owner, "SuspendedActivation")? != original
        || !matches!((original.as_slice(), returned.as_slice()),
            ([a, r, w, p], [b, s, value, worker, plan])
            if (a, r, w, p) == (b, s, worker, plan) && *value == reply)
    {
        return Err("resume changed the declared owner, reply, worker or storage in its outcome");
    }
    Ok(ResumeShape {
        selected: prepared.selected,
        ready_owner: prepared.owner_type,
        suspended_owner: input.structural_type,
        result: result.id,
        ready: ready.id,
        ready_field: ready_field.id,
        mismatch: mismatch.id,
        mismatch_owner: owner_field.id,
        mismatch_permit: permit_field.id,
        mismatch_value: value_field.id,
        permit,
    })
}
