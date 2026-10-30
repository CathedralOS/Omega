//! Source checking scopes include every supplied application, not a Native winner.
//!
//! Build and source defaults describe exact target/application contexts. All
//! members and completed finite instances in those contexts owe their contracts,
//! including overridden defaults and uninvoked siblings. The ordinary checking
//! graph propagates those contexts to helpers and declaration/type dependencies.
//! This census grants neither provider precedence nor ABI/install authority.

mod calls;
mod declarations;
mod inventory;
mod schemas;

use super::{ProviderPlanningContext, TargetProviderDefaults};
use symbols::SymbolHandle;
use typed_trees::{TypedTrees, references::DeclarationSelection};

struct CheckingScope<'request> {
    target: &'request str,
    roots: Option<Vec<SymbolHandle>>,
    candidates: Vec<inventory::Candidate>,
    supplied: Vec<bool>,
}

pub(crate) fn source_checking_applications(
    typed: &TypedTrees,
    scopes: &[crate::FixedLayoutTargetScope],
    defaults: &[TargetProviderDefaults],
    topology: &semantic_vocabulary::proof_applications::ProofApplicationPlan<SymbolHandle>,
    operational: &flow_effects::OperationalPlan,
) -> Result<Vec<DeclarationSelection>, String> {
    let context = ProviderPlanningContext::new(typed, source::DependencyScope::Product);
    let prepared = scopes
        .iter()
        .map(|scope| {
            CheckingScope::prepare(
                typed,
                scope,
                defaults,
                inventory::prepare(context, topology, &scope.target),
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let applications =
        semantic_vocabulary::proof_applications::ProofApplications::new(topology, operational);
    let mut work = prepared
        .iter()
        .map(|scope| scope.work(typed, &applications, &[]))
        .collect::<Vec<_>>();
    // An ordinary unmentioned body still owes Source formation. Apply the
    // explicit checking inputs to bodies with no supplying context at all;
    // never widen a body/helper already assigned through a real application.
    // Thus a Mac-only map and its portable helpers retain their Mac scope.
    let unscoped = typed
        .machines()
        .iter()
        .filter(|machine| {
            typed.source_target_context(machine).is_none()
                && !work
                    .iter()
                    .any(|selection| selection.contains(machine.symbol))
        })
        .map(|machine| machine.symbol)
        .collect::<Vec<_>>();
    if !unscoped.is_empty() {
        for (scope, work) in prepared.iter().zip(&mut work) {
            *work = scope.work(typed, &applications, &unscoped);
        }
    }
    Ok(work)
}

impl<'request> CheckingScope<'request> {
    fn prepare(
        typed: &TypedTrees,
        scope: &'request crate::FixedLayoutTargetScope,
        defaults: &[TargetProviderDefaults],
        candidates: Vec<inventory::Candidate>,
    ) -> Result<Self, String> {
        let declarations = scope
            .provider_declarations
            .iter()
            .chain(
                defaults
                    .iter()
                    .filter(|row| {
                        row.target
                            .as_deref()
                            .is_none_or(|target| target == scope.target)
                    })
                    .flat_map(|row| &row.selections),
            )
            .filter(|row| {
                row.target
                    .is_none_or(|target| target.canonical_name() == scope.target)
            })
            .map(|row| declarations::Declaration::new(typed, row))
            .collect::<Result<Vec<_>, _>>()?;
        let mut slots = Vec::new();
        for candidate in &candidates {
            for key in std::iter::once(&candidate.schema.key).chain(&candidate.schema.receivers) {
                if !slots.contains(key) {
                    slots.push(key.clone());
                }
            }
        }
        let declarations = declarations
            .iter()
            .map(|declaration| (declaration, declaration.slots(&slots)))
            .collect::<Vec<_>>();
        let supplied = candidates
            .iter()
            .map(|candidate| {
                declarations.iter().any(|(declaration, slots)| {
                    declaration.provider() == &candidate.provider
                        && slots.iter().any(|slot| {
                            slot == &candidate.schema.key
                                || candidate.schema.receivers.contains(slot)
                        })
                })
            })
            .collect::<Vec<_>>();
        let mut roots = scope.entry_declarations.clone();
        if let Some(roots) = &mut roots {
            for (candidate, supplied) in candidates.iter().zip(&supplied) {
                if *supplied {
                    candidate.extend_checking_roots(typed, roots);
                }
            }
        }
        Ok(Self {
            target: &scope.target,
            roots,
            candidates,
            supplied,
        })
    }

    fn work(
        &self,
        typed: &TypedTrees,
        applications: &semantic_vocabulary::proof_applications::ProofApplications<
            '_,
            SymbolHandle,
            flow_effects::OperationalPlan,
        >,
        unscoped: &[SymbolHandle],
    ) -> DeclarationSelection {
        let mut roots = self.roots.clone();
        if let Some(roots) = &mut roots {
            roots.extend(unscoped.iter().copied());
        }
        let mut dependencies = Vec::new();
        loop {
            // None retains the reusable source surface. Explicit product
            // scopes also include every matching authored target body.
            let reached = applications.reachability(
                self.target,
                self.roots.is_none(),
                &dependencies,
                roots.as_deref(),
            );
            let demands = calls::collect(typed, &reached.machines, &reached.calls);
            let mut changed = false;
            if let Some(roots) = &mut roots {
                // Nested declarations owe their contracts even without an
                // invocation. Use semantic containment, never generated names.
                for machine in typed.machines() {
                    let mut owner = typed.declaration_scope_parent(machine.symbol);
                    while owner.is_valid() {
                        if reached.machines.contains(&owner) {
                            if !roots.contains(&machine.symbol) {
                                roots.push(machine.symbol);
                                changed = true;
                            }
                            break;
                        }
                        owner = typed.declaration_scope_parent(owner);
                    }
                }
                // Completion owns the exact template/instance association.
                // Keep universal templates and all completed local tuples in
                // the same supplying context; captures retain distinct roots.
                for receipt in &typed.machine_specializations {
                    if reached.machines.contains(&receipt.template)
                        || reached.machines.contains(&receipt.instance)
                    {
                        for symbol in [receipt.template, receipt.instance] {
                            if !roots.contains(&symbol) {
                                roots.push(symbol);
                                changed = true;
                            }
                        }
                    }
                }
            }
            for demand in &demands {
                let supplied_for_subject =
                    self.candidates
                        .iter()
                        .zip(&self.supplied)
                        .any(|(candidate, supplied)| {
                            *supplied && candidate.schema.includes(&demand.schema)
                        });
                // Without a supplying declaration, all corresponding Source
                // descriptions remain possible. Native alone decides unique
                // whole-map coverage, precedence, calling plans and admission.
                for (candidate, supplied) in self.candidates.iter().zip(&self.supplied) {
                    if candidate.schema.includes(&demand.schema)
                        && (!supplied_for_subject || *supplied)
                        && let Some(member) =
                            candidate.member_for(typed, demand.called, demand.requirement)
                        && !dependencies.contains(&(demand.called, member))
                    {
                        dependencies.push((demand.called, member));
                        changed = true;
                    }
                }
            }
            if !changed {
                let (machines, declarations) = applications.membership(
                    self.target,
                    self.roots.is_none(),
                    &dependencies,
                    roots.as_deref(),
                );
                // A conformance description retains its row roster, but may
                // not widen an executable body's exact checking context.
                let excluded = DeclarationSelection::from_symbols(
                    typed
                        .machines()
                        .iter()
                        .filter(|machine| !machines.contains(&machine.symbol))
                        .map(|machine| machine.symbol),
                );
                return DeclarationSelection::from_symbols(
                    machines.into_iter().chain(declarations),
                )
                .excluding(&excluded);
            }
        }
    }
}
