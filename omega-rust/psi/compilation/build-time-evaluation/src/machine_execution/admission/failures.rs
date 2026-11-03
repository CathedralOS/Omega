//! Refuse a concrete compiler invocation before any possible failure executes.
//! Establish receiving domains before reusing a completed empty body summary.
//! Guarded or unknown summaries still need an ordinary concrete caller.

use super::{
    BuildTimeAdmissionPlan, BuildTimeInvocationCustody, BuildTimeProgram, concrete_invocation,
};
use crate::BuildTimeValue;
use typed_trees::machine::Machine;

impl BuildTimeAdmissionPlan {
    /// After admission has proved authored entry requirements, establish and
    /// project the supplied arguments, then prove failure safety immediately
    /// before interpreting those same values.
    pub(crate) fn require_invocation_failures(
        &self,
        input: BuildTimeProgram<'_>,
        machine: &Machine,
        arguments: &mut [BuildTimeValue],
        custody: Option<BuildTimeInvocationCustody>,
    ) -> Result<(), String> {
        let judgment = (|| {
            // Host values must satisfy their receiving domains before any
            // body result can be used. This also validates opaque contexts.
            super::supplied_values::require_established_arguments(
                input.typed(),
                machine,
                arguments,
            )?;
            let prepared = self.prepare_recursive_checking_input(input, machine.symbol)?;
            let checked = prepared.checked();
            // The body was checked under its entry requirements. Admission has
            // proved authored requires, and the fresh construction above proves
            // receiving domains, so an empty summary needs no other call check.
            if concrete_invocation::require_empty_failure_summary(checked, machine.symbol).is_ok() {
                return Ok(());
            }
            let source_span = custody.and_then(|custody| match custody {
                BuildTimeInvocationCustody::Source(span) => Some(span),
                BuildTimeInvocationCustody::Symbol(symbol) => {
                    input.typed().symbols.symbol_provenance_source_span(symbol)
                }
            });
            let mut matching = checked
                .typed
                .machines()
                .iter()
                .filter(|candidate| candidate.symbol == machine.symbol);
            let selected = matching
                .next()
                .ok_or("constant invocation lost its exact checked machine")?;
            if matching.next().is_some() {
                return Err("constant invocation has ambiguous checked machine identity".into());
            }
            concrete_invocation::value_invocation_discharges(
                &checked.typed,
                selected,
                arguments,
                source_span,
            )
        })();
        semantic_vocabulary::compiler_evaluation::require_compiler_evaluation_failures(
            machine.name.as_str(),
            judgment.as_ref().map(|_| ()).map_err(String::as_str),
        )
    }
}

#[cfg(test)]
mod tests;
