//! A Boolean value retains conditional relations from its evaluation, not an
//! initializer to replay later. Each relation already owns resolved operands,
//! selected comparison meaning and write invalidation. Copies and state edges
//! transport those same observations; writing a subject withdraws its relation
//! without changing the Boolean's saved value.

use super::super::ordered_values::{Operand, Relation, rebind_relations};
use super::{ExpressionHandle, Machine, State, ValueEnvironment};
use typed_trees::types::PrimitiveType;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct BooleanValue {
    pub(in crate::proof_contracts::arithmetic_domains) binding: Operand,
    pub(in crate::proof_contracts::arithmetic_domains) alternatives: [Vec<Relation>; 2],
}

impl BooleanValue {
    pub(in crate::proof_contracts::arithmetic_domains) fn rebound(
        &self,
        bindings: &[(Operand, Operand)],
    ) -> Vec<Self> {
        let targets = self.binding.rebound(bindings);
        if targets.is_empty() {
            return Vec::new();
        }
        let alternatives = self
            .alternatives
            .each_ref()
            .map(|relations| rebind_relations(relations, bindings));
        targets
            .into_iter()
            .map(|binding| Self {
                binding,
                alternatives: alternatives.clone(),
            })
            .collect()
    }
}

/// Capture the evaluated value only when its expression has no writes. An
/// effectful initializer cannot describe current operands by replaying its
/// pre-call comparison. Unknown expressions simply retain no conditional facts.
pub(crate) fn record_boolean_local(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: Option<&State>,
    environment: &mut ValueEnvironment,
    local: &typed_trees::statement::TableLocalData,
) {
    if !local.symbol.is_valid() {
        return;
    }
    record_binding(
        program,
        machine,
        state,
        environment,
        Operand::Place {
            root: local.symbol,
            path: local.name.as_str().to_owned(),
        },
        program.primitive_type_reference(local.type_reference),
        local.initial_value,
    );
}

pub(crate) fn record_boolean_assignment(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: Option<&State>,
    environment: &mut ValueEnvironment,
    target: ExpressionHandle,
    expression: ExpressionHandle,
) {
    let Some(state) = state else {
        return;
    };
    let primitive = super::declared_place_type_raw(program, machine, Some(state), target)
        .and_then(|reference| program.primitive_type_reference(reference));
    if primitive != Some(PrimitiveType::Bool) {
        return;
    }
    let Some(binding) = super::ordered_values::operand(program, machine, state, target, None)
    else {
        return;
    };
    record_binding(
        program,
        machine,
        Some(state),
        environment,
        binding,
        primitive,
        expression,
    );
}

fn record_binding(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: Option<&State>,
    environment: &mut ValueEnvironment,
    binding: Operand,
    primitive: Option<PrimitiveType>,
    expression: ExpressionHandle,
) {
    if primitive != Some(PrimitiveType::Bool) || !expression.is_valid() {
        return;
    }
    if crate::machine_calls::calls::expression_has_calls(program, expression)
        && !crate::CallFrameResolver::new(program).is_some_and(|frames| {
            frames
                .expression_may_write_paths(machine, expression)
                .is_some_and(|paths| paths.is_empty())
        })
    {
        return;
    }
    let alternatives = [false, true].map(|positive| {
        let mut selected = environment.clone();
        super::narrow_environment_by_condition(
            program,
            machine,
            state,
            &mut selected,
            expression,
            positive,
        );
        selected.ordered_values
    });
    if alternatives.iter().all(Vec::is_empty) {
        return;
    }
    environment
        .boolean_values
        .retain(|value| value.binding != binding);
    environment.boolean_values.push(BooleanValue {
        binding,
        alternatives,
    });
}

pub(super) fn select(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: Option<&State>,
    environment: &mut ValueEnvironment,
    expression: ExpressionHandle,
    positive: bool,
) -> bool {
    if environment.boolean_values.is_empty()
        || super::declared_place_type_raw(program, machine, state, expression)
            .and_then(|reference| program.primitive_type_reference(reference))
            != Some(PrimitiveType::Bool)
    {
        return false;
    }
    let Some(binding) = state.and_then(|state| {
        super::ordered_values::operand(program, machine, state, expression, None)
    }) else {
        return false;
    };
    let Some(value) = environment
        .boolean_values
        .iter()
        .find(|value| value.binding == binding)
    else {
        return false;
    };
    for relation in &value.alternatives[usize::from(positive)] {
        if !environment.ordered_values.contains(relation) {
            environment.ordered_values.push(relation.clone());
        }
    }
    true
}
