//! Recursive proof results remain available when using the induction hypothesis.
//! Register only the exact recursive call after its entry requirements and
//! decrease have been checked; ordinary circular specification imports stay refused.

use super::{StructuralJudge, StructuralTerm};
use std::sync::Arc;
use typed_trees::machine::Machine;

impl StructuralJudge<'_> {
    pub(in crate::proof_contracts::contract_entailment) fn retain_induction_value(
        &mut self,
        machine: &Machine,
        value: &StructuralTerm,
    ) {
        if self.has_induction_value(value) {
            return;
        }
        let resolved = match self.prepare_induction_value(machine, value) {
            Ok(resolved) => resolved,
            Err(reason) => {
                if std::env::var_os("OMEGA_STRUCT_TRACE").is_some() {
                    eprintln!("induction-value owner={:?}: {reason}", self.machine_symbol);
                }
                return;
            }
        };
        // The original application enters the IH before any conclusion is
        // installed. Save its current operand observations too: later result
        // judgment resolves those operands, without authorizing a new call.
        let values = Arc::make_mut(&mut self.induction_values);
        values.push(value.clone());
        if !resolved.same_resolution_payload(value)
            && !values
                .iter()
                .any(|known| known.same_resolution_payload(&resolved))
        {
            values.push(resolved);
        }
    }

    fn prepare_induction_value(
        &self,
        machine: &Machine,
        value: &StructuralTerm,
    ) -> Result<StructuralTerm, &'static str> {
        let StructuralTerm::Application {
            result_type,
            target,
            selections,
            machine: spelling,
            arguments,
        } = value
        else {
            return Err("the induction result is not an exact application");
        };
        let entry = self
            .program
            .machine_states(machine)
            .first()
            .ok_or("the induction owner has no entry state")?;
        if *target != entry.symbol || arguments.len() != self.program.state_parameters(entry).len()
        {
            return Err(
                "the induction application does not preserve its entry state and argument count",
            );
        }
        if !result_type.is_valid()
            || !crate::value_custody::type_references::type_references_match(
                self.program,
                *result_type,
                entry.return_type,
            )
        {
            return Err(
                "the induction result does not preserve its declared result type and access policy",
            );
        }
        if !arguments
            .iter()
            .all(|value| self.identity_value_is_available(value))
        {
            return Err("an induction argument has no admitted stable value observation");
        }
        let plans =
            crate::machine_calls::calls::operand_call_plans(self.program, self.frame_resolver());
        crate::machine_calls::denotational_calls::checked_value_call_candidate(
            self.program,
            machine,
            entry,
            &plans.0,
            &plans.1,
        )?;
        Ok(StructuralTerm::Application {
            result_type: *result_type,
            target: *target,
            selections: selections.clone(),
            machine: spelling.clone(),
            arguments: arguments
                .iter()
                .map(|value| self.resolve(value.clone()))
                .collect(),
        })
    }

    pub(super) fn has_induction_value(&self, value: &StructuralTerm) -> bool {
        self.induction_values
            .iter()
            .any(|known| known.same_resolution_payload(value))
    }

    pub(super) fn retain_induction_values_from(&mut self, completed: &Self) {
        for value in completed.induction_values.iter() {
            if !self.has_induction_value(value) {
                Arc::make_mut(&mut self.induction_values).push(value.clone());
            }
        }
    }
}

pub(super) fn common_induction_values(
    outcomes: &[&StructuralJudge<'_>],
) -> Arc<Vec<StructuralTerm>> {
    let Some(first) = outcomes.first() else {
        return Arc::new(Vec::new());
    };
    if outcomes
        .iter()
        .all(|other| Arc::ptr_eq(&first.induction_values, &other.induction_values))
    {
        return Arc::clone(&first.induction_values);
    }
    Arc::new(
        first
            .induction_values
            .iter()
            .filter(|value| {
                outcomes
                    .iter()
                    .all(|other| other.has_induction_value(value))
            })
            .cloned()
            .collect(),
    )
}
