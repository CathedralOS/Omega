//! Ordinary scalar calls share checked admission and the hermetic interpreter.
//!
//! The scalar evaluator owns expression order and anonymous landing. This owner
//! only crosses the call boundary: validate every authored argument before any
//! selective execution, preserve its declared carrier, then create fresh value
//! snapshots for the exact selected entry. A private checked probe supplies the
//! body/type and crash evidence missing from the pre-resolution syntax forest.
//! Neither a temporary constant placeholder nor an empty service row establishes
//! that evidence. The caller must first close the declaration dependency graph.

use std::sync::Arc;

use diagnostics::Diagnostic;
use language_semantics::const_value::{
    CanonicalConstIdentity, CanonicalConstValue, DecodedCanonicalConstValue,
};
use typed_trees::{
    TypedTrees,
    expression::{ExpressionHandle, ExpressionNode},
    machine::Machine,
    state::State,
    types::PrimitiveType,
};

use crate::const_generic_expressions::value::{self, ConstantCalls};
use crate::{BuildTimeAdmissionPlan, BuildTimeInvocationCustody, BuildTimeValue};

pub(super) struct CheckedInitializers {
    typed: TypedTrees,
    admission: BuildTimeAdmissionPlan,
    authority: Option<Arc<dyn crate::BuildTimeSelectionAuthority>>,
    crash_causes: Vec<(symbols::SymbolHandle, Vec<checked_trees::CrashCause>)>,
}

impl CheckedInitializers {
    pub(super) fn prepare(
        typed: &TypedTrees,
        authority: Option<Arc<dyn crate::BuildTimeSelectionAuthority>>,
        probe_symbols: &[symbols::SymbolHandle],
    ) -> Result<Self, Vec<Diagnostic>> {
        let prepared = crate::PreparedBuildMachineProgram::prepare(typed)?;
        let mut bodies = prepared.typed().clone();
        let (probes, machines): (Vec<_>, Vec<_>) = bodies
            .machines()
            .iter()
            .cloned()
            .partition(|machine| probe_symbols.contains(&machine.symbol));
        if probes.len() != probe_symbols.len() {
            return Err(super::failure(
                source::SourceSpan::default(),
                "initializer preparation lost an exact probe owner",
            ));
        }
        // Probe expressions are checked by the exact scalar evaluator, not as
        // runtime bodies whose call results have only a declared range. Only
        // their compiler-created roots leave this private body check. Every
        // authored helper body remains unchanged and undergoes ordinary checking.
        bodies.roots.machines = arena::HandleSpan::empty();
        for machine in machines {
            bodies.push_machine(machine);
        }
        let checked = typed_trees_to_checked_trees::lower_typed_trees(bodies)?;
        let crash_causes = typed_trees_to_checked_trees::infer_checked_crash_causes(
            &checked.typed,
            &checked.facts,
        );
        let mut typed = checked.typed;
        for probe in probes {
            typed.push_machine(probe);
        }
        let admission =
            BuildTimeAdmissionPlan::infer_with_selection_authority(&typed, authority.clone());
        Ok(Self {
            typed,
            admission,
            authority,
            crash_causes,
        })
    }

    pub(super) fn typed(&self) -> &TypedTrees {
        &self.typed
    }

    pub(super) fn calls<'program>(
        &'program self,
        machine: &'program Machine,
        state: &'program State,
    ) -> Invocation<'program> {
        Invocation {
            program: self,
            machine,
            state,
        }
    }

    pub(super) fn calls_for_source(
        &self,
        reference: source::SourceSpan,
    ) -> Result<Invocation<'_>, String> {
        let typed = self.typed();
        let mut machines = typed
            .machines()
            .iter()
            .filter(|machine| typed.symbols.symbol_source_span(machine.symbol) == Some(reference));
        let machine = machines
            .next()
            .ok_or("checked initializer probe lost its source owner")?;
        if machines.next().is_some() {
            return Err("checked initializer probe source owner is ambiguous".into());
        }
        let [state] = typed.machine_states(machine) else {
            return Err("checked initializer probe lost its single state".into());
        };
        Ok(self.calls(machine, state))
    }

    pub(super) fn calls_for_symbol(
        &self,
        symbol: symbols::SymbolHandle,
    ) -> Result<Invocation<'_>, String> {
        let typed = self.typed();
        let machine = typed
            .machines()
            .iter()
            .find(|machine| machine.symbol == symbol)
            .ok_or("retained initializer probe lost its exact symbol")?;
        let [state] = typed.machine_states(machine) else {
            return Err("retained initializer probe lost its single state".into());
        };
        Ok(self.calls(machine, state))
    }
}

pub(super) struct Invocation<'program> {
    program: &'program CheckedInitializers,
    machine: &'program Machine,
    state: &'program State,
}

impl Invocation<'_> {
    pub(super) fn validate_custody(
        &self,
        original: ExpressionHandle,
        materialized: ExpressionHandle,
    ) -> Result<Vec<crate::const_generic_expressions::DependencyValue>, String> {
        crate::const_generic_expressions::validate_retained_initializer_call_custody(
            self.program.typed(),
            self.machine,
            self.state,
            original,
            materialized,
        )
    }

    pub(super) fn evaluate(
        &self,
        expression: ExpressionHandle,
        destination: PrimitiveType,
    ) -> Result<(CanonicalConstValue, Vec<Diagnostic>), String> {
        value::evaluate_with_calls(
            self.program.typed(),
            self.machine,
            self.state,
            expression,
            destination,
            self,
        )
    }

    fn selected(&self, expression: ExpressionHandle) -> Result<(&Machine, &State), String> {
        let typed = self.program.typed();
        crate::admission::require_call_expression_selection(
            typed,
            expression,
            self.program.authority.as_deref(),
        )?;
        let ExpressionNode::Call(call) = typed.expression_table.expression(expression) else {
            return Err("initializer lost its selected call".into());
        };
        if !call.machine_arguments.is_empty()
            || !call.evidence_arguments.is_empty()
            || call.static_machine_parameter.is_valid()
            || call.static_requirement_dispatch.is_some()
            || call.quotient_operation.is_some()
            || call.private_layout_operation.is_some()
        {
            return Err(
                "constant invocation needs its complete specialized application context".into(),
            );
        }
        let mut selected = typed.machines().iter().filter(|machine| {
            machine.type_parameters.is_empty()
                && typed
                    .machine_states(machine)
                    .first()
                    .is_some_and(|entry| typed.call_has_no_runtime_receiver(call, machine, entry))
        });
        let machine = selected
            .next()
            .ok_or("constant call has no exact closed ordinary entry")?;
        if selected.next().is_some() {
            return Err("constant call has conflicting exact entries".into());
        }
        let state = typed
            .machine_states(machine)
            .first()
            .ok_or("constant call lost its entry")?;
        self.program.admission.require_common_floor_for_invocation(
            typed,
            machine,
            BuildTimeInvocationCustody::Source(typed.expression_table.source_span(expression)),
        )?;
        Ok((machine, state))
    }
}

impl ConstantCalls for Invocation<'_> {
    fn validate_call(
        &self,
        expression: ExpressionHandle,
    ) -> Result<(PrimitiveType, Vec<Diagnostic>), String> {
        let typed = self.program.typed();
        let (_, entry) = self.selected(expression)?;
        let destination =
            crate::const_generic_expressions::exact_probe_destination(typed, entry.return_type)
                .ok_or("constant call result needs an exact builtin integer or Boolean carrier")?;
        let ExpressionNode::Call(call) = typed.expression_table.expression(expression) else {
            return Err("constant validation lost its selected call".into());
        };
        let arguments = typed.expression_table.expression_handles(call.arguments);
        let parameters = typed.state_parameters(entry);
        if arguments.len() != call.arguments.len() || arguments.len() != parameters.len() {
            return Err("constant call argument count differs from its exact entry".into());
        }
        let mut warnings = Vec::new();
        for (argument, parameter) in arguments.iter().zip(parameters) {
            let carrier = crate::const_generic_expressions::exact_probe_destination(
                typed,
                parameter.type_reference,
            )
            .ok_or("constant call argument needs an exact builtin integer or Boolean carrier")?;
            for warning in value::validate_with_calls(
                typed,
                self.machine,
                self.state,
                *argument,
                carrier,
                self,
            )? {
                if !warnings.contains(&warning) {
                    warnings.push(warning);
                }
            }
        }
        Ok((destination, warnings))
    }

    fn evaluate_call(
        &self,
        expression: ExpressionHandle,
    ) -> Result<(CanonicalConstValue, Vec<Diagnostic>), String> {
        let typed = self.program.typed();
        let (machine, entry) = self.selected(expression)?;
        let causes = self
            .program
            .crash_causes
            .iter()
            .find(|(symbol, _)| *symbol == machine.symbol)
            .ok_or("constant invocation has no complete checked failure summary")?;
        if !causes.1.is_empty() {
            return Err(format!(
                "constant invocation of `{}` retains unhandled {:?} routes; concrete invocation discharge is required",
                machine.name, causes.1
            ));
        }
        let ExpressionNode::Call(call) = typed.expression_table.expression(expression) else {
            return Err("constant execution lost its selected call".into());
        };
        let mut arguments = Vec::new();
        let mut warnings = Vec::new();
        for (argument, parameter) in typed
            .expression_table
            .expression_handles(call.arguments)
            .iter()
            .zip(typed.state_parameters(entry))
        {
            let destination = crate::const_generic_expressions::exact_probe_destination(
                typed,
                parameter.type_reference,
            )
            .ok_or("constant call argument lost its exact carrier")?;
            let (value, argument_warnings) = value::evaluate_with_calls(
                typed,
                self.machine,
                self.state,
                *argument,
                destination,
                self,
            )?;
            arguments.push(match value.decode_encoding() {
                Some(DecodedCanonicalConstValue::Integer { value, .. }) => {
                    let bits = if destination.is_signed_integer() {
                        i64::try_from(value)
                            .map_err(|_| "constant argument exceeds signed interpreter storage")?
                    } else {
                        u64::try_from(value)
                            .map_err(|_| "constant argument exceeds unsigned interpreter storage")?
                            as i64
                    };
                    BuildTimeValue::Int(bits)
                }
                Some(DecodedCanonicalConstValue::Boolean(value)) => BuildTimeValue::Bool(value),
                _ => return Err("constant argument is not a canonical scalar snapshot".into()),
            });
            for warning in argument_warnings {
                if !warnings.contains(&warning) {
                    warnings.push(warning);
                }
            }
        }
        let result = self
            .program
            .admission
            .evaluate_const_evaluable_machine_symbol_for_invocation(
                typed,
                machine.symbol,
                arguments,
                BuildTimeInvocationCustody::Source(typed.expression_table.source_span(expression)),
            )?;
        let destination =
            crate::const_generic_expressions::exact_probe_destination(typed, entry.return_type)
                .ok_or("constant result lost its exact carrier")?;
        let value = match result {
            BuildTimeValue::Bool(value) if destination == PrimitiveType::Bool => {
                CanonicalConstValue::boolean(value)
            }
            BuildTimeValue::Int(value) if destination.accepts_integer_literal() => {
                let value = if destination.is_signed_integer() {
                    i128::from(value)
                } else {
                    i128::from(value as u64)
                };
                let identity = CanonicalConstIdentity::integer(destination.name(), value);
                CanonicalConstValue::new(identity.type_name, identity.encoding, value.to_string())
            }
            _ => return Err("constant result does not match its declared scalar carrier".into()),
        };
        Ok((value, warnings))
    }
}
