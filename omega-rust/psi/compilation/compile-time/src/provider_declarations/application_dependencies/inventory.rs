//! Complete prepared source maps, independent of ABI realization and choice.
use super::super::{
    ProviderPlanningContext, prepared_named_provider_applications, prepared_named_provider_member,
};
use super::schemas::{Key, Schema, provider_key};
use language_semantics::declaration_selection::provider_choices::Candidate as CandidateView;
use symbols::SymbolHandle;
use typed_trees::{TypedTrees, trait_definition::AppliedTraitScope};

pub(super) struct Candidate {
    pub schema: Schema,
    pub provider: Key,
    pub rows: Vec<(SymbolHandle, SymbolHandle)>,
    pub target: String,
}
impl Candidate {
    /// The selected whole map supplies the authored row; the ordinary
    /// requirement/specialization owner supplies its complete local telescope.
    /// An open template is never a substitute for a missing executable tuple.
    pub fn member_for(
        &self,
        program: &TypedTrees,
        called: SymbolHandle,
        requirement: SymbolHandle,
    ) -> Option<SymbolHandle> {
        let mut rows = self.rows.iter().filter(|(row, _)| *row == requirement);
        let &(_, member) = rows.next()?;
        if rows.next().is_some() {
            return None;
        }
        if program.closed_requirement_application(called).is_none() {
            return program
                .machine_by_symbol(member)
                .filter(|machine| program.machine_type_parameters(machine).is_empty())
                .map(|machine| machine.symbol);
        }
        let mut instances = program.machines().iter().filter(|machine| {
            program.machine_type_parameters(machine).is_empty()
                && program.requirement_realization_matches(called, member, machine.symbol)
        });
        let instance = instances.next()?;
        instances.next().is_none().then_some(instance.symbol)
    }
}
impl CandidateView for Candidate {
    type Slot = Key;
    type Provider = Key;
    fn slot(&self) -> &Key {
        &self.schema.key
    }
    fn provider(&self) -> &Key {
        &self.provider
    }
    fn target(&self) -> &str {
        &self.target
    }
    fn has_requirements(&self) -> bool {
        !self.schema.methods.is_empty()
    }
    fn covers_requirements(&self) -> bool {
        self.schema.methods.iter().all(|requirement| {
            self.rows
                .iter()
                .filter(|(row, _)| row == requirement)
                .count()
                == 1
        }) && self
            .rows
            .iter()
            .all(|(row, _)| self.schema.methods.contains(row))
    }
    fn receiver_count(&self) -> usize {
        self.schema.receivers.len()
    }
    fn supplies_receiver(&self, other: &Self) -> Option<bool> {
        self.schema.supplies(&other.schema)
    }
    fn shares_receiver(&self, other: &Self) -> Option<bool> {
        self.schema.overlaps(&other.schema)
    }
}

impl CandidateView for &Candidate {
    type Slot = Key;
    type Provider = Key;
    fn slot(&self) -> &Key {
        &self.schema.key
    }
    fn provider(&self) -> &Key {
        &self.provider
    }
    fn target(&self) -> &str {
        &self.target
    }
    fn has_requirements(&self) -> bool {
        CandidateView::has_requirements(*self)
    }
    fn covers_requirements(&self) -> bool {
        CandidateView::covers_requirements(*self)
    }
    fn receiver_count(&self) -> usize {
        self.schema.receivers.len()
    }
    fn supplies_receiver(&self, other: &Self) -> Option<bool> {
        self.schema.supplies(&other.schema)
    }
    fn shares_receiver(&self, other: &Self) -> Option<bool> {
        self.schema.overlaps(&other.schema)
    }
}

/// The graph already owns exact family membership. No host profile or textual
/// target prefix participates in this declaration projection.
fn selected_member(
    typed: &TypedTrees,
    topology: &semantic_vocabulary::proof_applications::ProofApplicationPlan<SymbolHandle>,
    target: &str,
    member: SymbolHandle,
) -> Option<SymbolHandle> {
    let selected = topology.callable_for_target(member, target)?;
    typed
        .machine_by_symbol(selected)
        .map(|machine| machine.symbol)
}

pub(super) fn prepare(
    context: ProviderPlanningContext<'_>,
    topology: &semantic_vocabulary::proof_applications::ProofApplicationPlan<SymbolHandle>,
    target: &str,
) -> Vec<Candidate> {
    let typed = context.typed;
    let mut candidates = Vec::new();
    for application in prepared_named_provider_applications(context) {
        if !application.scope.declaration.is_boundary {
            continue;
        }
        let Some(schema) = Schema::trait_scope(typed, application.scope.clone()) else {
            continue;
        };
        let mut rows = Vec::new();
        for row in &application.declaration.rows {
            let member = if application.capture.is_valid() {
                prepared_named_provider_member(typed, &application, row.realization_machine)
            } else {
                Some(row.realization_machine)
            };
            if let Some(member) =
                member.and_then(|member| selected_member(typed, topology, target, member))
            {
                rows.push((row.requirement, member));
            }
        }
        candidates.push(Candidate {
            schema,
            provider: provider_key(
                context,
                application.declaration.symbol,
                application.identity.clone(),
            ),
            rows,
            target: target.to_owned(),
        });
    }
    // Attached satisfaction declarations group by their exact data owner and
    // complete applied schema. A partial group stays partial; rows from two
    // providers never form one covering candidate.
    for machine in typed.machines().iter().filter(|machine| {
        machine.attached_data_symbol.is_valid()
            && typed
                .data_definitions()
                .iter()
                .any(|data| data.symbol == machine.attached_data_symbol)
            && !machine.target_family_interface
            && context.contains(machine.symbol)
            && typed
                .source_target_context(machine)
                .is_none_or(|scope| scope == target)
            && !typed
                .machine_specializations
                .iter()
                .any(|receipt| receipt.instance == machine.symbol)
    }) {
        for edge in typed.machine_trait_conformances(machine) {
            let schema = if let Some(scope) =
                AppliedTraitScope::from_machine_conformance(typed, machine, edge)
            {
                if !scope.declaration.is_boundary {
                    continue;
                }
                Schema::trait_scope(typed, scope)
            } else {
                Schema::exact(typed, edge.requirement_symbol)
            };
            let Some(schema) = schema else {
                continue;
            };
            let provider = provider_key(context, machine.attached_data_symbol, None);
            let position = candidates.iter().position(|candidate| {
                candidate.provider == provider && candidate.schema.key == schema.key
            });
            let position = position.unwrap_or_else(|| {
                candidates.push(Candidate {
                    schema,
                    provider,
                    rows: Vec::new(),
                    target: target.to_owned(),
                });
                candidates.len() - 1
            });
            candidates[position]
                .rows
                .push((edge.requirement_symbol, machine.symbol));
        }
    }
    candidates
}
