//! Storage openings and witness equalities that hold on the current control path.
use super::*;

mod history;
pub(super) use history::CopyHistory;
use history::CopyPoint;

#[derive(Clone, Default)]
pub(super) struct LiveOpenings {
    pub(super) openings: Vec<PackageOpeningHandle>,
    pub(super) names: Vec<SymbolHandle>,
    pub(super) copies: CopyPoint,
}

impl LiveOpenings {
    pub(super) fn retain_common(&mut self, other: &Self, history: &mut CopyHistory) {
        self.openings
            .retain(|opening| other.openings.contains(opening));
        self.names.retain(|name| other.names.contains(name));
        self.copies = history.join(self.copies, other.copies);
    }
}

pub(super) fn stable_subject(
    program: &TypedTrees,
    state: &State,
    subject: &PackageSubject,
) -> Option<PackageSubject> {
    let backed = backing(program, state, subject)?;
    backed
        .steps
        .iter()
        .all(|step| !matches!(step, PackageSubjectStep::IndexBinding(_)))
        .then_some(backed)
}

/// Assignments and local declarations transfer the same witness relation.
pub(super) fn value_copy(
    program: &TypedTrees,
    state: &State,
    statement: &StatementNode,
) -> Option<(PackageSubject, PackageSubject)> {
    let (ty, value, destination) = match statement {
        StatementNode::LocalData(local) => (
            local.type_reference,
            local.ordinary_value_initializer()?,
            bound_subject(local.symbol),
        ),
        StatementNode::Assignment(assignment) => (
            crate::types::typing_queries::declared_place_type(program, state, assignment.target)
                .ok()?,
            assignment.value,
            stable_subject(program, state, &package_place(program, assignment.target)?)?,
        ),
        _ => return None,
    };
    let types = &program.type_reference_table;
    if !types.contains_type_reference(ty) {
        return None;
    }
    // An enclosing array or record can transfer a package at one of its
    // member paths even when the outer declaration has no hidden parameter.
    let base = types.unconstrained_type_reference(ty);
    let aggregate = match types.type_reference(base) {
        TypeReferenceNode::FixedArray { .. } => true,
        _ => types
            .type_head_symbol(base)
            .and_then(|head| program.unique_data_definition_by_symbol(head))
            .is_some_and(|data| {
                !data.package_parameters.is_empty()
                    || program.stored_data_fields(data).next().is_some()
            }),
    };
    if !aggregate
        || program.stored_value_multiplicity(ty) != language_core::Multiplicity::Unrestricted
    {
        return None;
    }
    let source = stable_subject(program, state, &package_place(program, value)?)?;
    Some((source, destination))
}

#[cfg(test)]
mod tests;
