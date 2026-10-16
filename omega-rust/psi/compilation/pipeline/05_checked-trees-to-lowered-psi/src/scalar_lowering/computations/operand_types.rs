//! Structural operands retain their storage namespace even when no source
//! parameter or local declares their computed result type. All scalar/Unit
//! catalog entrances use these same roots; reservation still grants no custody.
use super::{CheckedTrees, LoweringError};
use checked_trees::{
    CheckedScalarComputationKind as Computation,
    CheckedScalarComputationStructuralArgument as Argument,
};

pub(crate) struct OperandTypeRequirements {
    pub(crate) identities: Vec<typed_trees::type_identity::TypeIdentityId>,
    pub(crate) literal_view: bool,
}

pub(crate) fn type_requirements(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
) -> Result<OperandTypeRequirements, LoweringError> {
    let mut types = super::cases::type_roots(checked, &[machine])?;
    let mut literal_view = false;
    types.extend(super::fields::type_roots(checked, machine)?);
    for value in crate::expression_preparation::computation_graph::reachable_structural_values(
        checked, machine,
    )? {
        crate::unit::attached_unit::catalog::collect_value_roots(checked, value, &mut types)?;
        // A computed aggregate owns its children's formation dependencies just
        // as a named local does. Retain the walk's literal-carrier requirement
        // before emitters borrow this catalog; the receiving call's signature
        // cannot describe intermediate inputs of nested producers.
        literal_view |=
            crate::expression_preparation::structural_value_requirements::collect_value_requirements(
                checked,
                machine,
                value,
                &mut Vec::new(),
            )?;
    }
    let plans = &checked.facts.values.scalar_computations;
    for handle in super::machine_computation_handles(checked, machine)? {
        match &plans.nodes.get(handle).kind {
            Computation::Call {
                source_call,
                structural_arguments,
                ..
            } => {
                for argument in plans
                    .structural_arguments
                    .span(*structural_arguments)
                    .ok_or(LoweringError::Unsupported(
                        "computed operand type roots lost their argument roster",
                    ))?
                {
                    if let Argument::Place(subject) = argument {
                        types.push(subject.type_identity);
                        types.extend(subject.borrowed_root_type_identity());
                        // The actual's type names the selected child, while
                        // reservation allocates the complete producer. Keep
                        // both in the catalog even when no signature or local
                        // names the temporary owner's storage type.
                        if matches!(
                            subject.source,
                            checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralExpression { .. }
                        ) {
                            let owner = crate::expression_preparation::computation_graph::produced_value_root(
                                checked, *source_call, subject,
                            )?;
                            types.push(validation::structural_type_identity(
                                &checked.typed,
                                owner.type_reference,
                                &[],
                                &[],
                            ));
                        }
                        // An owned literal copies immutable image bytes into
                        // its reserved bounded home. Keep that input carrier
                        // in the shared catalog before any emitter borrows it.
                        literal_view |= subject.access
                            == checked_trees::CheckedStructuralAccess::Owned
                            && subject.byte_sequence_literal().is_some();
                    }
                }
            }
            Computation::ByteSequenceLiteralComparison { subject, .. }
            | Computation::ByteSequencePredicate { subject, .. }
            | Computation::CaseMembership {
                subject: Argument::Place(subject),
                ..
            } => {
                types.push(subject.type_identity);
            }
            _ => {}
        }
    }
    types.sort();
    types.dedup();
    Ok(OperandTypeRequirements {
        identities: types,
        literal_view,
    })
}
