//! Prepare the selected implementation of a generic boundary method for each
//! argument tuple used by that method's callers.

use super::{GeneratedMachines, Stop, specialize_application};
use crate::source_state::TypedTrees;
use crate::source_state::trait_definition::AppliedTraitScope;
use diagnostics::Diagnostic;
use std::collections::{HashMap, HashSet};
use symbols::SymbolHandle;

pub(super) struct SelectedMethods {
    providers: HashMap<SymbolHandle, Vec<(SymbolHandle, SymbolHandle)>>,
    prepared: HashSet<(SymbolHandle, SymbolHandle)>,
    generated: GeneratedMachines,
}

impl SelectedMethods {
    pub(super) fn new(program: &TypedTrees, selections: &[SymbolHandle]) -> Self {
        let mut providers = HashMap::<_, Vec<_>>::new();
        let mut seen = HashSet::new();
        for &selection in selections {
            let symbol = program.implementation_value_head(selection);
            if !seen.insert(symbol) {
                continue;
            }
            let Some(provider) = program.conformance_by_symbol(symbol) else {
                continue;
            };
            for row in &provider.rows {
                if program.machine_by_symbol(row.realization_machine)
                    .is_some_and(|machine| !machine.type_parameters.is_empty()) {
                    providers.entry(row.requirement).or_default()
                        .push((symbol, row.realization_machine));
                }
            }
        }
        Self { providers, prepared: HashSet::new(), generated: GeneratedMachines::default() }
    }

    pub(super) fn prepare_applications(&mut self, program: &mut TypedTrees) -> Result<(), Vec<Diagnostic>> {
        if self.providers.is_empty() {
            return Ok(());
        }
        let mut pending = Vec::new();
        for closed in program.closed_requirement_applications() {
            let Some(members) = self.providers.get(&closed.template) else { continue };
            for &(provider, member) in members {
                let scope = program.conformance_by_symbol(provider)
                    .and_then(|record| AppliedTraitScope::from_conformance(program, record))
                    .and_then(|scope| scope.declaring_scope(program, closed.declaring_trait));
                if scope.is_some_and(|scope| program.requirement_application_matches_contract_scope(closed.signature.symbol, &scope))
                    && self.prepared.insert((member, closed.signature.symbol)) {
                    pending.push((closed.signature.symbol, closed.application, member));
                }
            }
        }
        for (requirement, call, member) in pending {
            let declaration = program.machine_by_symbol(member).expect("the selection retained its member");
            let parameters = program.machine_type_parameters(declaration).to_vec();
            let name = declaration.name.clone();
            let mut arguments = program.expression_table.static_machine_application_arguments(call).to_vec();
            if arguments.len() != parameters.len() {
                return Err(vec![Diagnostic::error("the selected method declares a different number of static parameters from its requirement")]);
            }
            let owner = program.expression_table.static_machine_application(call).lexical_owner;
            let application = program.expression_table.retain_static_machine_application(requirement, name, &arguments);
            program.expression_table.static_machine_application_mut(application).lexical_owner = owner;
            specialize_application(program, member, &parameters, application, &mut arguments, &mut self.generated)
                .map_err(|reason| {
                    let detail = match reason {
                        Stop::Unserved(reason) => format!("cannot prepare the selected method: {reason}"),
                        Stop::Rejected(reason) => reason,
                        Stop::Defect(reason) => reason.to_owned(),
                    };
                    vec![Diagnostic::error(detail)]
                })?
                .ok_or_else(|| vec![Diagnostic::error("the selected method still has an open argument in a closed requirement")])?;
        }
        Ok(())
    }
}
