//! Which result places receive the execution owner or the rejected inputs.
//! The interpreter and native emitter must not guess a case tag or offset.
use semantic_vocabulary::{StructuralCaseId, StructuralFieldId, StructuralTypeId};
use terminal_psi::{
    BoundaryMachineDeclaration, BoundaryMachineResult, BoundaryParameterKind, StructuralFieldType,
    StructuralMultiplicity, StructuralTypeShape, TerminalModule,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PreparationShape {
    pub selected: terminal_psi::TerminalPreparedActivationEntry,
    pub result: StructuralTypeId,
    pub ready: StructuralCaseId,
    pub owner: StructuralFieldId,
    pub owner_type: StructuralTypeId,
    pub rejected: StructuralCaseId,
    pub arguments: StructuralFieldId,
    pub retained: StructuralFieldId,
    pub retained_carrier: StructuralTypeId,
}

/// This checks the source members and their physical carriers, not the size
/// offered by a caller or whether a target can run on that backing.
pub(super) fn result_shape(
    module: &TerminalModule,
    boundary: &BoundaryMachineDeclaration,
    selected: terminal_psi::TerminalPreparedActivationEntry,
) -> Result<PreparationShape, &'static str> {
    let BoundaryMachineResult::Structural(output) = &boundary.result else {
        return Err("prepare must return the ready or rejected outcome");
    };
    let result = module
        .structural_type(output.structural_type)
        .ok_or("prepare result type is absent")?;
    let StructuralTypeShape::Sum { cases } = &result.shape else {
        return Err("prepare result has no ready and rejected cases");
    };
    let name = |id| module.identities.text(id);
    if name(result.declaration_identity) != Some("toolchain::ActivationPreparation")
        || cases.len() != 2
        || result.multiplicity != output.multiplicity
        || !output.qualifications.is_empty()
    {
        return Err("prepare has a foreign or incomplete result");
    }
    let case = |identity| {
        cases
            .iter()
            .find(|c| name(c.declaration_identity) == Some(identity))
            .ok_or("prepare result lacks a declared case")
    };
    let ready = case("toolchain::ActivationPreparation::Ready")?;
    let rejected = case("toolchain::ActivationPreparation::Rejected")?;
    let [owner] = ready.fields.as_slice() else {
        return Err("ready must hold one execution owner");
    };
    let StructuralFieldType::Structural(owner_type) = owner.field_type else {
        return Err("a ready owner must not be scalar");
    };
    let token = module
        .structural_type(owner_type)
        .ok_or("ready owner is absent")?;
    if owner.relevance.is_erased()
        || owner.storage.is_some()
        || name(owner.declaration_identity)
            != Some("toolchain::ActivationPreparation::Ready::owner")
        || name(token.declaration_identity) != Some("toolchain::ReadyActivation")
        || !token.owns_linear_claim
        || token.multiplicity != StructuralMultiplicity::Linear
        || !matches!(token.shape, StructuralTypeShape::BorrowingOpaque { lifetime_count } if lifetime_count.get() == 1)
    {
        return Err("ready must hold the core linear execution owner");
    }
    let application = |id, family| {
        application_identity::type_identity::named_application_arguments(
            name(id).ok_or("prepare result is missing its application identity")?,
            &super::toolchain_name(family),
        )
        .map_err(|_| "prepare has a malformed result application")?
        .ok_or("prepare has a foreign result application")
    };
    if application(result.identity, "ActivationPreparation")?
        != application(token.identity, "ReadyActivation")?
    {
        return Err("prepare returned an owner with different worker or storage arguments");
    }
    if rejected.fields.len() != 2 {
        return Err("rejection must return both inputs");
    }
    let field = |identity| {
        rejected
            .fields
            .iter()
            .find(|f| name(f.declaration_identity) == Some(identity))
            .ok_or("rejection must return the original arguments and retained loan")
    };
    let arguments = field("toolchain::ActivationPreparation::Rejected::arguments")?;
    let retained = field("toolchain::ActivationPreparation::Rejected::retained")?;
    if retained.relevance.is_erased() || retained.storage.is_some() || arguments.storage.is_some() {
        return Err("preparation changed the storage of a rejected input");
    }
    let [argument_kind, BoundaryParameterKind::Structural] = boundary.parameter_order.as_slice()
    else {
        return Err("prepare needs arguments and one retained loan");
    };
    let (backing, rest) = boundary
        .structural_parameters
        .split_last()
        .ok_or("retained loan is absent")?;
    let backing_type = module
        .structural_type(backing.structural_type)
        .ok_or("retained loan type is absent")?;
    if usize::try_from(backing.position).ok() != Some(rest.len()) {
        return Err("prepare lost its structural backing parameter");
    }
    let retained_carrier = retained_backing_carrier(module, backing, backing_type, retained)
        .ok_or("retained backing must return the original exclusive byte slice")?;
    let arguments_match = match (argument_kind, boundary.scalar_parameters.as_slice(), rest) {
        (BoundaryParameterKind::Scalar, [ty], []) => {
            arguments.field_type.canonical_leaf_shape()
                == Some(StructuralTypeShape::PrimitiveScalar(*ty))
        }
        (BoundaryParameterKind::Structural, [], [arg]) => {
            super::field_matches_carrier(module, arguments, arg.structural_type)
        }
        _ => false,
    };
    if !arguments_match {
        return Err("rejection changed an argument or the retained loan's carrier");
    }
    if !super::forwarded_qualifications(
        &output.projected_qualifications,
        rejected.id,
        &[
            (arguments.identity, rest.first()),
            (retained.identity, Some(backing)),
        ],
    ) {
        return Err("preparation asserted an unprovided result qualification");
    }
    Ok(PreparationShape {
        selected,
        result: result.id,
        ready: ready.id,
        owner: owner.id,
        owner_type,
        rejected: rejected.id,
        arguments: arguments.id,
        retained: retained.id,
        retained_carrier,
    })
}

/// Passing a mutable slice directly borrows its element storage. The result
/// holds that same borrow as a first-class mutable view of the same elements.
fn retained_backing_carrier(
    module: &TerminalModule,
    backing: &terminal_psi::StructuralParameterDeclaration,
    input: &terminal_psi::StructuralTypeDeclaration,
    field: &terminal_psi::StructuralFieldDeclaration,
) -> Option<StructuralTypeId> {
    use terminal_psi::{ByteSequenceCarrier, StructuralAccess};
    let output = if field.inline_carrier.is_some() {
        crate::inline_carrier_declaration(module.types.structural_types.iter(), field)?
    } else if let StructuralFieldType::Structural(id) = field.field_type {
        module.structural_type(id)?
    } else {
        super::field_matches_carrier(module, field, input.id).then_some(input)?
    };
    let access = |ty: &terminal_psi::StructuralTypeDeclaration| {
        crate::descriptors::descriptor_results::descriptor_value_access(
            std::iter::once(ty),
            ty.id,
            ty.multiplicity,
        )
    };
    if field.relevance.is_erased() || access(output) != Some(StructuralAccess::MutableBorrow) {
        return None;
    }
    let declared_access = match (&input.shape, &output.shape) {
        (
            StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView { access }),
            StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView { .. }),
        ) => *access,
        (
            StructuralTypeShape::ElementView { element, access },
            StructuralTypeShape::ElementView { element: returned, .. },
        ) if element == returned && module.structural_type(*element).is_some_and(|ty| matches!(
            ty.shape, StructuralTypeShape::PrimitiveScalar(semantic_vocabulary::ScalarType::Integer(i))
                if !i.is_address() && i.bits() == Some(8) && i.sign() == semantic_vocabulary::IntegerSign::Unsigned
        )) => *access,
        _ => return None,
    };
    match backing.access {
        StructuralAccess::Owned
            if input.id == output.id && access(input) == Some(StructuralAccess::MutableBorrow) =>
        {
            Some(output.id)
        }
        StructuralAccess::MutableBorrow
            if declared_access.is_none_or(|a| a == StructuralAccess::MutableBorrow) =>
        {
            Some(output.id)
        }
        _ => None,
    }
}
