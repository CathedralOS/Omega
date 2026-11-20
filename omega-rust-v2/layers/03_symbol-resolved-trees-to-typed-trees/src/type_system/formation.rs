//! Type formation records each open index and each applied declaration at the site that uses it.
//! Replay those edges only after all declaration bodies and field types are available.
use super::type_catalog::TypeCatalog;
use crate::declarations::resolved_source_file::ResolvedSourceFile;
use crate::type_error::SourceTypeError;
use crate::type_system::substitution::TypeInference;
use crate::{ObligationKind, Site, TypeError, TypedStage, TypingObligation, ValueType};
use omega_resolved::{DeclarationId, TreeNodeId};
use omega_typed::site_at;
use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap, HashSet};

#[derive(PartialEq)]
pub(super) enum TypeRequirement {
    Index(ValueType),
    Universe(ValueType, ValueType),
    RuntimeParameter(DeclarationId, usize),
    Application(DeclarationId, Vec<ValueType>),
}
pub(crate) struct SiteRequirements {
    owner: Option<DeclarationId>,
    rows: Vec<TypeRequirement>,
}
pub(crate) type FormationSites = BTreeMap<(Site, TreeNodeId), SiteRequirements>;

#[derive(Default)]
pub(crate) struct Formations {
    local: RefCell<FormationSites>,
    pub(crate) imported: Option<&'static FormationSites>,
}

impl Formations {
    pub(crate) fn into_sites(self) -> FormationSites {
        self.local.into_inner()
    }

    pub(crate) fn index(
        &self,
        unit: &ResolvedSourceFile<'_>,
        node: usize,
        value: ValueType,
    ) {
        self.retain(unit, node, vec![TypeRequirement::Index(value)]);
    }

    pub(super) fn runtime_parameters(
        &self,
        unit: &ResolvedSourceFile<'_>,
        node: usize,
        ty: &ValueType,
    ) {
        let parameters = ty
            .machine_type_parameters()
            .map(|(owner, index)| TypeRequirement::RuntimeParameter(owner.clone(), index));
        self.retain(unit, node, parameters.collect());
    }

    pub(super) fn retain(
        &self,
        unit: &ResolvedSourceFile<'_>,
        node: usize,
        requirements: Vec<TypeRequirement>,
    ) {
        if requirements.is_empty() {
            return;
        }
        let mut sites = self.local.borrow_mut();
        let checked = sites
            .entry((site_at(unit.file, node), unit.file.node_id(node)))
            .or_insert_with(|| SiteRequirements {
                owner: unit.specialization_owner(node).cloned(),
                rows: Vec::new(),
            });
        for row in requirements {
            if !checked.rows.contains(&row) {
                checked.rows.push(row);
            }
        }
    }

    pub(crate) fn check(
        &self,
        types: &TypeCatalog<'_>,
        output: &mut TypedStage,
    ) -> Result<(), TypeError> {
        let sites = self.local.borrow();
        let all_sites = sites
            .values()
            .chain(self.imported.into_iter().flat_map(|m| m.values()));
        check_runtime_arguments(all_sites.clone(), output)?;
        let graph = DeclarationRequirements::from_sites(types, all_sites);
        let mut proofs = Vec::new();
        for call in &output.calls {
            let applications = graph
                .specialize(types, &call.machine, call.node, |ty| call.instantiate(ty))
                .map_err(|m| TypeError::for_site(&call.site, m))?;
            graph.check_applications(types, applications, &call.site, call.node, &mut proofs)?;
        }
        for obligation in &output.obligations {
            if let ObligationKind::MathematicalApplication {
                definition: Some(id),
                generic_arguments,
                ..
            } = &obligation.kind
            {
                let applications = graph
                    .specialize(types, id, obligation.node, |ty| ty.substitute(id, generic_arguments))
                    .map_err(|m| TypeError::for_site(&obligation.site, m))?;
                graph.check_applications(types, applications, &obligation.site, obligation.node, &mut proofs)?;
            }
        }
        for ((site, node), checked) in sites.iter() {
            for row in &checked.rows {
                let kind = match row {
                    TypeRequirement::Application(id, args) => {
                        graph.check_applications(types, vec![(id.clone(), args.clone())], site, *node, &mut proofs)?;
                        continue;
                    }
                    TypeRequirement::Index(ValueType::IndexedInteger { width, .. }) => ObligationKind::PositiveWidth(*width.clone()),
                    TypeRequirement::Index(_) | TypeRequirement::RuntimeParameter(..) => continue,
                    TypeRequirement::Universe(subject, sort) => ObligationKind::UniverseMembership {
                        subject: subject.clone(),
                        sort: sort.clone(),
                    },
                };
                proofs.push(TypingObligation {
                    node: *node,
                    site: site.clone(),
                    kind,
                });
            }
        }
        output.obligations.extend(proofs);
        Ok(())
    }
}

/// A machine may store T only in its body. Follow the uses of that parameter
/// through forwarding calls so that each concrete selection is checked once.
fn check_runtime_arguments<'a>(
    sites: impl Iterator<Item = &'a SiteRequirements>,
    output: &'a TypedStage,
) -> Result<(), TypeError> {
    let calls = output
        .calls
        .iter()
        .chain(output.library.into_iter().flat_map(|library| &library.calls));
    let mut selections = HashMap::<_, Vec<_>>::new();
    for call in calls {
        for (owner, args) in std::iter::once((&call.machine, &call.type_arguments)).chain(call.enclosing_arguments.iter().map(|(id, args)| (id, args))) {
            selections.entry(owner).or_default().push((&call.site, args));
        }
    }
    let mut pending: Vec<_> = sites
        .flat_map(|site| &site.rows)
        .filter_map(|row| match row {
            TypeRequirement::RuntimeParameter(owner, index) => Some((owner, *index)),
            _ => None,
        })
        .collect();
    let mut seen = HashSet::new();
    while let Some((owner, index)) = pending.pop() {
        if seen.insert((owner, index)) {
            for &(site, args) in selections.get(owner).into_iter().flatten() {
                let argument = &args[index];
                if let Some((_, reason)) = argument.open_runtime_contract() {
                    return Err(TypeError::for_site(site, reason));
                }
                pending.extend(argument.machine_type_parameters());
            }
        }
    }
    Ok(())
}

type Application = (DeclarationId, Vec<ValueType>);
struct DeclarationRequirements<'a>(HashMap<&'a DeclarationId, Vec<&'a TypeRequirement>>);
impl<'a> DeclarationRequirements<'a> {
    fn from_sites(
        types: &'a TypeCatalog<'_>,
        sites: impl Iterator<Item = &'a SiteRequirements>,
    ) -> Self {
        let mut by_owner = HashMap::<_, Vec<_>>::new();
        let mut callers = HashMap::<_, Vec<_>>::new();
        let mut pending = Vec::new();
        for site in sites {
            let Some(owner) = &site.owner else { continue };
            for row in &site.rows {
                if matches!(row, TypeRequirement::RuntimeParameter(..)) {
                    continue;
                }
                by_owner.entry(owner).or_default().push(row);
                if let TypeRequirement::Application(target, args) = row {
                    for (scope, _) in types.scoped_arguments(target, args) {
                        callers.entry(scope).or_default().push(owner);
                    }
                } else {
                    pending.push(owner);
                }
            }
        }
        // Plain recursive records need no specialization work. Keep only declarations
        // that can reach an authored index or universe requirement.
        let mut needed = HashSet::new();
        while let Some(owner) = pending.pop() {
            if needed.insert(owner) {
                pending.extend(callers.remove(owner).into_iter().flatten());
            }
        }
        by_owner.retain(|id, _| needed.contains(id));
        Self(by_owner)
    }

    fn specialize(
        &self,
        types: &TypeCatalog<'_>,
        owner: &DeclarationId,
        node: TreeNodeId,
        apply: impl Fn(&ValueType) -> ValueType,
    ) -> Result<Vec<Application>, String> {
        let mut nested = Vec::new();
        for row in self.0.get(owner).into_iter().flatten() {
            match row {
                TypeRequirement::RuntimeParameter(..) => {}
                TypeRequirement::Index(value) => {
                    let applied = apply(value);
                    applied.validate_closed_indices()?;
                    types.indices.retain_demands_at(node, &applied);
                }
                TypeRequirement::Universe(subject, sort) => TypeCatalog::check_universe_member(&apply(subject), &apply(sort))?,
                TypeRequirement::Application(id, args) => nested.push((id.clone(), args.iter().map(&apply).collect())),
            }
        }
        Ok(nested)
    }

    fn check_applications(
        &self,
        types: &TypeCatalog<'_>,
        applications: Vec<Application>,
        site: &Site,
        node: TreeNodeId,
        proofs: &mut Vec<TypingObligation>,
    ) -> Result<(), TypeError> {
        enum Work {
            Visit(Application),
            Leave(DeclarationId),
        }
        let mut work: Vec<_> = applications.into_iter().map(Work::Visit).collect();
        let mut active = HashSet::new();
        let mut checked = HashSet::new();
        while let Some(step) = work.pop() {
            let (id, args) = match step {
                Work::Leave(id) => {
                    active.remove(&id);
                    continue;
                }
                Work::Visit(application) => {
                    if checked.contains(&application) {
                        continue;
                    }
                    if !active.contains(&application.0) {
                        checked.insert(application.clone());
                    }
                    application
                }
            };
            let mut nested = Vec::new();
            for (scope, _) in types.scoped_arguments(&id, &args) {
                nested.extend(
                    self.specialize(types, scope, node, |value| types.apply_type_arguments(value, &id, &args))
                        .map_err(|m| TypeError::for_site(site, m))?,
                );
            }
            if nested.is_empty() {
                continue;
            }
            if !active.insert(id.clone()) {
                // The same declaration with different arguments requires a recursive
                // formation judgment. Preserve the exact application for the verifier.
                let proof = TypingObligation {
                    node,
                    site: site.clone(),
                    kind: ObligationKind::FormRecursiveApplication {
                        declaration: id,
                        arguments: args,
                    },
                };
                if !proofs.contains(&proof) {
                    proofs.push(proof);
                }
                continue;
            }
            work.push(Work::Leave(id));
            work.extend(nested.into_iter().rev().map(Work::Visit));
        }
        Ok(())
    }
}
