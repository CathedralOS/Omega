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
    pub(super) fn record(&mut self, source: PackageSubject, new_local: PackageSubject) {
        if source == new_local {
            return;
        }
        self.retain_subjects(|subject| subject != &new_local);
        if let Some(group) = self.0.iter_mut().find(|group| group.contains(&source)) {
            group.push(new_local);
        } else {
            self.0.push(vec![source, new_local]);
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

pub(super) fn value_copy_source(
    program: &TypedTrees,
    state: &State,
    ty: typed::types::TypeReferenceHandle,
    initializer: Option<ExpressionHandle>,
) -> Option<PackageSubject> {
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
    stable_subject(program, state, &package_place(program, initializer?)?)
}
