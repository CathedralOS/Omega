//! Which result places receive the execution owner or the rejected inputs.
//! The interpreter and native emitter must not guess a case tag or offset.
use semantic_vocabulary::{StructuralCaseId, StructuralFieldId, StructuralTypeId};
use terminal_psi::{
    BoundaryMachineDeclaration, BoundaryMachineResult, BoundaryParameterKind,
    StructuralFieldType, StructuralMultiplicity, StructuralTypeShape, TerminalModule,
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
}

/// This checks the source members and their physical carriers, not the size
/// offered by a caller or whether a target can run on that backing.
pub(super) fn result_shape(module: &TerminalModule, boundary: &BoundaryMachineDeclaration,
    selected: terminal_psi::TerminalPreparedActivationEntry)
    -> Result<PreparationShape, &'static str>
{
    let BoundaryMachineResult::Structural(output) = &boundary.result else {
        return Err("prepare must return the ready or rejected outcome");
    };
    let result = module.structural_type(output.structural_type).ok_or("prepare result type is absent")?;
    let StructuralTypeShape::Sum { cases } = &result.shape else {
        return Err("prepare result has no ready and rejected cases");
    };
    let name = |id| module.identities.text(id);
    if name(result.declaration_identity) != Some("toolchain::ActivationPreparation")
        || cases.len() != 2 || result.multiplicity != output.multiplicity {
        return Err("prepare has a foreign or incomplete result");
    }
    let case = |identity| cases.iter().find(|c| name(c.declaration_identity) == Some(identity))
        .ok_or("prepare result lacks a declared case");
    let ready = case("toolchain::ActivationPreparation::Ready")?;
    let rejected = case("toolchain::ActivationPreparation::Rejected")?;
    let [owner] = ready.fields.as_slice() else { return Err("ready must hold one execution owner") };
    let StructuralFieldType::Structural(owner_type) = owner.field_type else {
        return Err("a ready owner must not be scalar");
    };
    let token = module.structural_type(owner_type).ok_or("ready owner is absent")?;
    if name(owner.declaration_identity) != Some("toolchain::ActivationPreparation::Ready::owner")
        || name(token.declaration_identity) != Some("toolchain::ReadyActivation")
        || !token.owns_linear_claim || token.multiplicity != StructuralMultiplicity::Linear
        || !matches!(token.shape, StructuralTypeShape::BorrowingOpaque { lifetime_count } if lifetime_count.get() == 1) {
        return Err("ready must hold the core linear execution owner");
    }
    let application = |id, family| {
        application_identity::type_identity::named_application_arguments(
            name(id).ok_or("prepare result is missing its application identity")?,
            &super::toolchain_name(family),
        ).map_err(|_| "prepare has a malformed result application")?
            .ok_or("prepare has a foreign result application")
    };
    if application(result.identity, "ActivationPreparation")? != application(token.identity, "ReadyActivation")? {
        return Err("prepare returned an owner with different worker or storage arguments");
    }
    if rejected.fields.len() != 2 { return Err("rejection must return both inputs"); }
    let field = |identity| rejected.fields.iter().find(|f| name(f.declaration_identity) == Some(identity))
        .ok_or("rejection must return the original arguments and retained loan");
    let arguments = field("toolchain::ActivationPreparation::Rejected::arguments")?;
    let retained = field("toolchain::ActivationPreparation::Rejected::retained")?;
    let [argument_kind, BoundaryParameterKind::Structural] = boundary.parameter_order.as_slice()
        else { return Err("prepare needs arguments and one retained loan"); };
    let (backing, rest) = boundary.structural_parameters.split_last().ok_or("retained loan is absent")?;
    let backing_type = module.structural_type(backing.structural_type).ok_or("retained loan type is absent")?;
    let byte_slice = match backing_type.shape {
        StructuralTypeShape::ByteSequence(terminal_psi::ByteSequenceCarrier::BorrowedView { .. }) => true,
        StructuralTypeShape::ElementView { element, .. } => module.structural_type(element)
            .is_some_and(|t| matches!(t.shape, StructuralTypeShape::PrimitiveScalar(
                semantic_vocabulary::ScalarType::Integer(i)) if !i.is_address() && i.bits() == Some(8)
                    && i.sign() == semantic_vocabulary::IntegerSign::Unsigned)),
        _ => false,
    };
    if !byte_slice || backing.position != 1
        || matches!(backing.access, terminal_psi::StructuralAccess::SharedBorrow |
            terminal_psi::StructuralAccess::WriteOnlyBorrow)
        || crate::descriptors::descriptor_results::descriptor_value_access(
            std::iter::once(backing_type), backing_type.id, backing.multiplicity
        ) != Some(terminal_psi::StructuralAccess::MutableBorrow) {
        return Err("retained backing must be an exclusive byte slice");
    }
    let matches_type = |field: &StructuralFieldType, ty: StructuralTypeId| {
        if let StructuralFieldType::Structural(id) = field { return *id == ty; }
        module.structural_type(ty).is_some_and(|decl| field.canonical_leaf_shape().as_ref() == Some(&decl.shape))
    };
    let arguments_match = match (argument_kind, boundary.scalar_parameters.as_slice(), rest) {
        (BoundaryParameterKind::Scalar, [ty], []) =>
            arguments.field_type.canonical_leaf_shape() == Some(StructuralTypeShape::PrimitiveScalar(*ty)),
        (BoundaryParameterKind::Structural, [], [arg]) => matches_type(&arguments.field_type, arg.structural_type),
        _ => false,
    };
    if !arguments_match || !matches_type(&retained.field_type, backing.structural_type) {
        return Err("rejection changed an argument or the retained loan's carrier");
    }
    Ok(PreparationShape { selected, result: result.id, ready: ready.id, owner: owner.id, owner_type,
        rejected: rejected.id, arguments: arguments.id, retained: retained.id })
}
