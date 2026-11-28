//! Storage openings and witness equalities that hold on the current control path.
use super::*;

#[derive(Clone, Default)]
pub(super) struct LiveOpenings {
    pub(super) openings: Vec<PackageOpeningHandle>,
    pub(super) names: Vec<SymbolHandle>,
    pub(super) copies: CopyGroups,
}

impl LiveOpenings {
    pub(super) fn retain_common(&mut self, other: &Self) {
        self.openings
            .retain(|opening| other.openings.contains(opening));
        self.names.retain(|name| other.names.contains(name));
        self.copies.0 = self
            .copies
            .0
            .iter()
            .flat_map(|left| {
                other.copies.0.iter().filter_map(move |right| {
                    let shared: Vec<_> = left
                        .iter()
                        .filter(|place| right.contains(place))
                        .cloned()
                        .collect();
                    (shared.len() > 1).then_some(shared)
                })
            })
            .collect();
    }
}

/// Each group contains distinct places that still carry a copied witness.
/// Removing one replaced place preserves the relation between surviving copies.
#[derive(Clone, Default)]
pub(super) struct CopyGroups(Vec<Vec<PackageSubject>>);

impl CopyGroups {
    pub(super) fn record(&mut self, source: PackageSubject, destination: PackageSubject) {
        if source == destination {
            return;
        }
        self.retain_subjects(|subject| subject != &destination);
        if let Some(group) = self.0.iter_mut().find(|group| group.contains(&source)) {
            group.push(destination);
        } else {
            self.0.push(vec![source, destination]);
        }
    }

    pub(super) fn related(&self, left: &PackageSubject, right: &PackageSubject) -> bool {
        self.0
            .iter()
            .any(|group| group.contains(left) && group.contains(right))
    }

    pub(super) fn retain_subjects(&mut self, survives: impl Fn(&PackageSubject) -> bool) {
        self.0.retain_mut(|group| {
            group.retain(&survives);
            group.len() > 1
        });
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
    let declaration = types.type_head_symbol(types.unconstrained_type_reference(ty))?;
    let data = program.unique_data_definition_by_symbol(declaration)?;
    if data.package_parameters.is_empty()
        || program.stored_value_multiplicity(ty) != language_core::Multiplicity::Unrestricted
    {
        return None;
    }
    let source = stable_subject(program, state, &package_place(program, value)?)?;
    Some((source, destination))
}
