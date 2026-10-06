//! Element bounds a cited helper theorem delivers to the caller's flow.
//!
//! A statement call cites a checked theorem: once its `requires` hold under
//! the exact argument substitution, its `ensures` conclusions enter the
//! caller's facts ([citation](../../../../../../../wiki/spec/proofs/contracts.md#citation-and-induction)).
//! This adapter imports the conclusions arithmetic formation can use: a
//! builtin comparison between a literal and an element `cells[index]` of an
//! array parameter, where the actual array is a place and the actual index a
//! literal. `cell_is_bit(&context.g.board, 1)` with
//! `ensures cells[index] >= 0 && cells[index] <= 1` bounds the place
//! `context.g.board[1]` to `[0, 1]`.
//!
//! Soundness rests on the ordinary obligations, not on this adapter: the
//! callee's `ensures` is proved at each of its exits and the call's
//! `requires` at this call site, each in the same compilation, so a program
//! that compiles cites only proved facts. The facts live in the caller's
//! value environment and lapse with it: any write overlapping the array
//! place, and any unknown write, drops them.

use super::guard_narrowing::comparison_interval;
use super::{
    BinaryOperator, ExpressionHandle, ExpressionNode, Interval, Machine, ProofFact,
    SignatureContractKind, State, TypedTrees, ValueEnvironment, place_path,
};
use crate::proof_contracts::bound_expression_meaning::has_builtin_binary_expression_meaning;

/// `collection[index]` with a literal, non-negative `index` over a place
/// collection, spelled as the value environment keys it.
pub(crate) fn element_place_path(
    program: &TypedTrees,
    expression: ExpressionHandle,
) -> Option<String> {
    let ExpressionNode::Indexed(indexed) = program.expression_table.expression(expression) else {
        return None;
    };
    let index = program
        .expression_table
        .constant_integer_value(indexed.index)?;
    let index = u64::try_from(index).ok()?;
    Some(format!(
        "{}[{index}]",
        place_path(program, indexed.collection)?
    ))
}

/// `collection[index]` whose value cannot change during the activation, keyed
/// as `element_place_path` keys a literal index: the collection is a shared
/// loan the state never reseats (the loan freezes its bytes), and the index
/// is a literal or an immutable scalar parameter. Every read of that spelling
/// is then the same value, so a guard's facts about one read bound the others
/// (`text[index] < 48 || text[index] > 57 -> ..` refuted gives the later
/// `text[index] - 48` its range).
pub(crate) fn stable_element_place_path(
    program: &TypedTrees,
    machine: &Machine,
    state: Option<&State>,
    expression: ExpressionHandle,
) -> Option<String> {
    if let Some(path) = element_place_path(program, expression) {
        return Some(path);
    }
    let state = state?;
    let ExpressionNode::Indexed(indexed) = program.expression_table.expression(expression) else {
        return None;
    };
    let parameter = |operand: ExpressionHandle| {
        let ExpressionNode::Name(name) = program.expression_table.expression(operand) else {
            return None;
        };
        if !name.symbol.is_valid() || name.symbol != name.head_symbol {
            return None;
        }
        program
            .state_parameters(state)
            .iter()
            .find(|parameter| parameter.symbol == name.symbol)
    };
    let collection = parameter(indexed.collection)?;
    let index = parameter(indexed.index)?;
    let frozen = matches!(
        program
            .type_reference_table
            .type_reference(collection.type_reference),
        typed_trees::types::TypeReferenceNode::Reference {
            access: language_core::ReferenceAccess::Shared,
            ..
        }
    ) && crate::state_reference_parameter_binding_is_stable(
        program,
        machine,
        state,
        collection.symbol,
    );
    let fixed_index = !index.is_mutable
        && program
            .primitive_type_reference(index.type_reference)
            .is_some_and(|primitive| primitive.accepts_integer_literal());
    (frozen && fixed_index).then(|| {
        Some(format!(
            "{}[{}]",
            place_path(program, indexed.collection)?,
            place_path(program, indexed.index)?
        ))
    })?
}

/// Import the element bounds of a checked callee's `ensures` after a
/// statement call returns.
pub(crate) fn seed_cited_element_bounds(
    program: &TypedTrees,
    call: &typed_trees::statement::TableCall,
    environment: &mut ValueEnvironment,
) {
    if !call.receiver.is_empty() || call.static_requirement_dispatch.is_some() {
        return;
    }
    let Some(callee) = checked_callee(program, call.target_symbol) else {
        return;
    };
    let Some(entry) = program.machine_states(callee).first() else {
        return;
    };
    let parameters = program.state_parameters(entry);
    let arguments = program.statement_table.expression_handles(call.arguments);
    if parameters.len() != arguments.len() {
        return;
    }
    let substitution = Substitution {
        program,
        parameters,
        arguments,
    };
    for contract in program.machine_contracts(callee) {
        if contract.kind != SignatureContractKind::Ensures {
            continue;
        }
        for fact in program.proof_facts.span_or_empty(contract.facts) {
            let ProofFact::Expression(expression) = fact else {
                continue;
            };
            seed_conjunct(
                program,
                callee,
                entry,
                &substitution,
                *expression,
                environment,
            );
        }
    }
}

/// A free machine whose checked body proves its own `ensures`.
fn checked_callee(program: &TypedTrees, symbol: symbols::SymbolHandle) -> Option<&Machine> {
    if !symbol.is_valid() {
        return None;
    }
    program.machines().iter().find(|machine| {
        machine.attached_data.is_none()
            && machine.supply_mode == language_semantics::MachineSupplyMode::CheckedBody
            && machine.body_is_present
            && (machine.symbol == symbol
                || program
                    .machine_states(machine)
                    .first()
                    .is_some_and(|entry| entry.symbol == symbol))
    })
}

struct Substitution<'program> {
    program: &'program TypedTrees,
    parameters: &'program [typed_trees::signature::StateParameter],
    arguments: &'program [ExpressionHandle],
}

impl Substitution<'_> {
    /// The actual argument bound to the formal a callee-side name reads.
    fn actual(&self, formal: ExpressionHandle) -> Option<ExpressionHandle> {
        let ExpressionNode::Name(path) = self.program.expression_table.expression(formal) else {
            return None;
        };
        if !path.symbol.is_valid()
            || self
                .program
                .expression_table
                .name_path_members(path.members)
                .len()
                != 1
        {
            return None;
        }
        let position = self
            .parameters
            .iter()
            .position(|parameter| parameter.symbol == path.symbol)?;
        // A formal the callee may write through is not a stable subject.
        if self.parameters[position].is_mutable {
            return None;
        }
        self.arguments.get(position).copied()
    }

    /// The caller-side element path a callee-side `array[index]` denotes.
    fn element_path(&self, formal: ExpressionHandle) -> Option<String> {
        let ExpressionNode::Indexed(indexed) = self.program.expression_table.expression(formal)
        else {
            return None;
        };
        let array = self.actual(indexed.collection)?;
        let array = match self.program.expression_table.expression(array) {
            ExpressionNode::Borrow(borrow)
                if borrow.access == language_core::ReferenceAccess::Shared =>
            {
                borrow.target
            }
            _ => array,
        };
        let index = match self
            .program
            .expression_table
            .constant_integer_value(indexed.index)
        {
            Some(index) => index,
            None => self
                .program
                .expression_table
                .constant_integer_value(self.actual(indexed.index)?)?,
        };
        let index = u64::try_from(index).ok()?;
        Some(format!("{}[{index}]", place_path(self.program, array)?))
    }
}

fn seed_conjunct(
    program: &TypedTrees,
    callee: &Machine,
    entry: &State,
    substitution: &Substitution<'_>,
    conjunct: ExpressionHandle,
    environment: &mut ValueEnvironment,
) {
    let ExpressionNode::Binary(comparison) = program.expression_table.expression(conjunct) else {
        return;
    };
    if comparison.operator == BinaryOperator::And {
        seed_conjunct(
            program,
            callee,
            entry,
            substitution,
            comparison.left,
            environment,
        );
        seed_conjunct(
            program,
            callee,
            entry,
            substitution,
            comparison.right,
            environment,
        );
        return;
    }
    if !matches!(
        comparison.operator,
        BinaryOperator::Less
            | BinaryOperator::LessOrEqual
            | BinaryOperator::Greater
            | BinaryOperator::GreaterOrEqual
            | BinaryOperator::Equal
    ) || !has_builtin_binary_expression_meaning(program, callee, Some(entry), conjunct)
    {
        return;
    }
    let (element, literal, element_on_left) = match program
        .expression_table
        .constant_integer_value(comparison.right)
    {
        Some(literal) => (comparison.left, literal, true),
        None => match program
            .expression_table
            .constant_integer_value(comparison.left)
        {
            Some(literal) => (comparison.right, literal, false),
            None => return,
        },
    };
    let Some(path) = substitution.element_path(element) else {
        return;
    };
    let interval = comparison_interval(
        comparison.operator,
        Interval {
            low: Some(literal),
            high: Some(literal),
        },
        element_on_left,
    );
    environment.narrow(path, interval);
}
