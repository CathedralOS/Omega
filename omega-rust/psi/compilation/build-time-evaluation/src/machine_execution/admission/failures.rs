//! Refuse a concrete compiler invocation before any possible failure executes.
//! Establish supplied value construction before admitting the exact call.
//! Entry requirements and guarded or unknown failure summaries need a caller check.

use super::{
    BuildTimeAdmissionPlan, BuildTimeAdmissionRejection, BuildTimeInvocationCustody,
    BuildTimeProgram, closure_validation::has_authored_requires, concrete_invocation,
};
use crate::BuildTimeValue;
use typed_trees::machine::Machine;

impl BuildTimeAdmissionPlan {
    /// Admit the exact host values received by a public evaluation request.
    /// Checking the body proves its internal calls, not the entry requirements
    /// on these supplied values. Establish those before using the body proof.
    pub(super) fn require_supplied_invocation(
        &self,
        input: BuildTimeProgram<'_>,
        machine: &Machine,
        arguments: &mut [BuildTimeValue],
        custody: Option<BuildTimeInvocationCustody>,
    ) -> Result<(), BuildTimeAdmissionRejection> {
        self.require_invocation_failures(input, machine, arguments, custody)
            .map_err(BuildTimeAdmissionRejection::from)?;
        // The exact call proves every recorded entry requirement, including
        // generated domain memberships. Ordinary checked flow proves internal
        // calls. Only that established judgment discharges the precondition
        // fence; every other admission requirement still applies.
        self.require_floor(input, machine, custody, true)
    }

    /// Establish supplied constructions, prove every recorded entry requirement,
    /// and refuse surviving failures for those exact arguments. The other
    /// admission requirements remain independently mandatory before execution.
    pub(crate) fn require_invocation_failures(
        &self,
        input: BuildTimeProgram<'_>,
        machine: &Machine,
        arguments: &mut [BuildTimeValue],
        custody: Option<BuildTimeInvocationCustody>,
    ) -> Result<(), String> {
        let judgment = (|| {
            // Check supplied constructions and authenticate opaque contexts
            // before proving the exact call's recorded entry requirements.
            super::supplied_values::require_established_arguments(
                input.typed(),
                machine,
                arguments,
            )?;
            let prepared = self.prepare_recursive_checking_input(input, machine.symbol)?;
            let checked = prepared.checked();
            let selected = exact_checked_machine(checked, machine.symbol)?;
            require_checked_invocation_failures(checked, selected, arguments, input, custody)
        })();
        require_failure_judgment(machine, judgment)
    }
}

fn exact_checked_machine(
    checked: &checked_trees::CheckedTrees,
    symbol: symbols::SymbolHandle,
) -> Result<&Machine, String> {
    let mut matching = checked
        .typed
        .machines()
        .iter()
        .filter(|candidate| candidate.symbol == symbol);
    let selected = matching
        .next()
        .ok_or("constant invocation lost its exact checked machine")?;
    if matching.next().is_some() {
        return Err("constant invocation has ambiguous checked machine identity".into());
    }
    Ok(selected)
}

fn invocation_source_span(
    input: BuildTimeProgram<'_>,
    custody: Option<BuildTimeInvocationCustody>,
) -> Option<source::SourceSpan> {
    custody.and_then(|custody| match custody {
        BuildTimeInvocationCustody::Source(span) => Some(span),
        BuildTimeInvocationCustody::Symbol(symbol) => {
            input.typed().symbols.symbol_provenance_source_span(symbol)
        }
    })
}

fn require_checked_invocation_failures(
    checked: &checked_trees::CheckedTrees,
    machine: &Machine,
    arguments: &[BuildTimeValue],
    input: BuildTimeProgram<'_>,
    custody: Option<BuildTimeInvocationCustody>,
) -> Result<(), String> {
    let entry = concrete_invocation::entry_state(&checked.typed, machine)
        .ok_or("constant invocation has no exact entry state")?;
    let entry_has_requirements = has_authored_requires(checked.typed.machine_contracts(machine))
        || has_authored_requires(checked.typed.state_contracts(entry));
    // A constructed snapshot is not proof of the entry contract. In particular,
    // parameter-domain memberships are recorded separately as requires. Check
    // those exact requirements at the call, even when the body is crash-free.
    // With no entry requirements, the checked body's empty summary needs no
    // other call check; its internal calls were already checked in that body.
    if !entry_has_requirements
        && concrete_invocation::require_empty_failure_summary(checked, machine.symbol).is_ok()
    {
        return Ok(());
    }
    concrete_invocation::value_invocation_discharges(
        &checked.typed,
        machine,
        arguments,
        invocation_source_span(input, custody),
    )
}

fn require_failure_judgment(machine: &Machine, judgment: Result<(), String>) -> Result<(), String> {
    semantic_vocabulary::compiler_evaluation::require_compiler_evaluation_failures(
        machine.name.as_str(),
        judgment.as_ref().map(|_| ()).map_err(String::as_str),
    )
}

#[cfg(test)]
mod tests;
