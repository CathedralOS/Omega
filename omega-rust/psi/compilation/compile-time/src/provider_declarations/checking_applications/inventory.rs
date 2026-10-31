//! Complete prepared source maps, independent of ABI realization and choice.
use super::super::{
    ProviderPlanningContext, prepared_named_provider_applications, prepared_named_provider_member,
};
use super::schemas::{Key, Schema, provider_key};
use symbols::SymbolHandle;
use typed_trees::{TypedTrees, trait_definition::AppliedTraitScope};

pub(super) struct Candidate {
    pub schema: Schema,
    pub provider: Key,
    pub rows: Vec<(SymbolHandle, SymbolHandle)>,
}
impl Candidate {
    /// Every supplied member owes its body contract, including all instances
    /// already produced by the canonical finite/application completion owner.
    pub fn extend_checking_roots(&self, program: &TypedTrees, roots: &mut Vec<SymbolHandle>) {
        for &(_, member) in &self.rows {
            if !roots.contains(&member) {
                roots.push(member);
            }
            for machine in program.machines() {
                if program
                    .machine_instantiation_path(machine.symbol, member)
                    .is_some()
                    && !roots.contains(&machine.symbol)
                {
                    roots.push(machine.symbol);
                }
            }
        }
    }

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
pub(super) fn prepare(context: ProviderPlanningContext<'_>) -> Vec<Candidate> {
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
            if let Some(member) = member.filter(|member| typed.machine_by_symbol(*member).is_some())
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
            && context.contains(machine.symbol)
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
