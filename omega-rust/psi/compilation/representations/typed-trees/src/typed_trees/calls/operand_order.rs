//! Source evaluation order is independent of executable parameter placement.
//! Explicit runtime header Values precede ordinary data operands. Inferred
//! descriptor Values alias an already completed data operand instead.

use crate::static_applications::StaticMachineApplicationHandle;
use crate::{
    TypedTrees, expression::ExpressionNode, static_applications::StaticMachineBindingValue,
};

pub struct CallArgumentEvaluationOrder {
    positions: Vec<usize>,
    position: usize,
    count: usize,
}

impl CallArgumentEvaluationOrder {
    pub fn is_natural(&self) -> bool {
        self.positions.is_empty()
    }
}

impl Iterator for CallArgumentEvaluationOrder {
    type Item = usize;

    fn next(&mut self) -> Option<usize> {
        if self.position == self.count {
            return None;
        }
        let position = self.position;
        self.position += 1;
        Some(self.positions.get(position).copied().unwrap_or(position))
    }
}

impl TypedTrees {
    /// Physical argument positions in their authored evaluation order. The
    /// receiving declaration and retained binding roster authenticate the
    /// runtime suffix; counts alone never invent another header operand.
    pub fn call_argument_evaluation_order(
        &self,
        application: StaticMachineApplicationHandle,
        target: symbols::SymbolHandle,
        count: usize,
    ) -> Result<CallArgumentEvaluationOrder, &'static str> {
        let natural = || CallArgumentEvaluationOrder {
            positions: Vec::new(),
            position: 0,
            count,
        };
        if !application.is_valid() {
            return Ok(natural());
        }
        let table = &self.expression_table;
        let values = table
            .static_machine_application_bindings(application)
            .iter()
            .filter_map(|binding| match binding.value {
                StaticMachineBindingValue::RuntimeValue { expression, .. } => {
                    Some((binding.parameter, expression))
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        if values.is_empty() {
            return Ok(natural());
        }
        let closed = self.closed_requirement_application(target);
        let parameters = if let Some(closed) = &closed {
            Some(self.state_signature_parameters(closed.signature))
        } else if let Some(state) = self.state_by_symbol(target) {
            Some(self.state_parameters(state))
        } else if let Some(machine) = self.machine_by_symbol(target) {
            self.machine_states(machine)
                .first()
                .map(|state| self.state_parameters(state))
        } else if let Some(slot) = self.dynamic_slot_by_symbol(target) {
            Some(slot.parameters.as_slice())
        } else {
            self.trait_machine_signature(target)
                .map(|(_, signature)| self.state_signature_parameters(signature))
                .or_else(|| {
                    self.machine_parameter_signature(target)
                        .map(|(_, signature)| self.state_signature_parameters(signature))
                })
                .or_else(|| {
                    // A logical evidence member has no executable state yet.
                    // Its exact authored bound still owns the ordinary roster;
                    // malformed bounds remain unavailable to this observation.
                    let binder = self.symbols.get(target).parent;
                    let owner = self.symbols.get(binder).parent;
                    self.evidence_bound_requirement(owner, binder, target)
                        .ok()
                        .flatten()
                        .map(|bound| self.state_signature_parameters(bound.signature))
                })
        }
        .ok_or("runtime operand order lost its exact receiving declaration")?;
        if parameters.len() != count {
            return Err("runtime operand order differs from its complete executable roster");
        }
        let mut runtime = Vec::with_capacity(values.len());
        for (binder, expression) in values {
            let mut matches = parameters
                .iter()
                .enumerate()
                .filter(|(_, parameter)| derives_from(self, parameter.symbol, binder));
            let position = matches.next().map(|(position, _)| position);
            if matches.next().is_some() {
                return Err("runtime operand order repeats a receiving Value formal");
            }
            runtime.push((position, expression));
        }
        if runtime.iter().all(|(position, _)| position.is_none())
            && closed.is_none_or(|closed| {
                closed
                    .local_arguments
                    .iter()
                    .all(|argument| argument.static_operand().is_some())
            })
        {
            // Universal contracts and family dispatch headers need no separate
            // executable Value formal. Their ordinary operands keep their order.
            return Ok(natural());
        }
        let runtime_positions = runtime
            .iter()
            .map(|(position, _)| {
                position.ok_or("runtime operand order lost a receiving Value formal")
            })
            .collect::<Result<Vec<_>, _>>()?;
        for (ordinal, position) in runtime_positions.iter().enumerate() {
            if runtime_positions[..ordinal].contains(position) {
                return Err("runtime operand order maps independent binders to one formal");
            }
        }
        let mut explicit = Vec::new();
        let mut inferred = Vec::new();
        for ((_, mut expression), position) in runtime.into_iter().zip(&runtime_positions) {
            while expression.is_valid() {
                match table.expression(expression) {
                    ExpressionNode::Member(member) => expression = member.receiver,
                    _ => break,
                }
            }
            let positions =
                if expression.is_valid() && self.saved_argument_source(expression).is_some() {
                    &mut inferred
                } else {
                    &mut explicit
                };
            positions.push(*position);
        }
        explicit.extend((0..count).filter(|position| !runtime_positions.contains(position)));
        explicit.extend(inferred);
        if explicit.iter().copied().eq(0..count) {
            return Ok(natural());
        }
        Ok(CallArgumentEvaluationOrder {
            positions: explicit,
            position: 0,
            count,
        })
    }
}

/// Generated symbols retain an acyclic, strictly earlier exact derivation.
/// Comparing authored roots would merge independently captured binders.
fn derives_from(
    program: &TypedTrees,
    mut symbol: symbols::SymbolHandle,
    binder: symbols::SymbolHandle,
) -> bool {
    while symbol.is_valid() {
        if symbol == binder {
            return true;
        }
        let preceding = program.symbols.get(symbol).generated_from;
        if !preceding.is_valid() || preceding.arena_index() >= symbol.arena_index() {
            return false;
        }
        symbol = preceding;
    }
    false
}
