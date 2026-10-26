//! Which sealed atomic operation an assignment carrier denotes, and the
//! receiver polarity that operation demands of the target's root.
//!
//! The parser spells `place.store(v, ordering)`, the fetch family, swap, and
//! compare-exchange as an assignment whose value is an
//! [`ExpressionNode::Atomic`] carrier (tokens-to-syntax-trees
//! `atomic_lets.rs`). That spelling is a lowering convenience, not a custody
//! fact: every sealed atomic requirement takes a shared receiver
//! (`wiki/spec/language/concurrency.md`, "All receivers are shared"), so the
//! ordinary "assignment target must be mutable" rule would wrongly reject
//! `self.value.store(..)` through `&self`. This module recovers the operation
//! the carrier denotes and asks the access-plan vocabulary for its receiver
//! polarity, so the custody check demands exactly what the operation requires
//! rather than deciding per type name.
//!
//! Two guards keep this from widening ordinary writes through shared borrows:
//!
//! - only a carrier whose target is atomic storage qualifies, classified by
//!   builtin atom and never by spelling. `.store(..)` on a plain `u32` is not
//!   an atomic operation on an atomic cell, so it keeps the exclusive demand;
//! - a direct `self.value = v` on an atomic field is not a carrier and keeps
//!   the exclusive demand; ordinary assignment never becomes atomic.
//!
//! Placed atomic fields are the sibling case owned by `placed_views`, whose
//! per-operation permission rows already authorize the carrier.

use access_plans::{AccessOperation, AtomicAccessOperation, BorrowPolarity};
use language_core::atomic::AtomicOrderingPlan;
use typed_trees::expression::{
    BinaryOperator, ExpressionHandle, ExpressionNode, TableAtomicExpression,
};
use typed_trees::machine::Machine;
use typed_trees::state::State;
use typed_trees::statement::TableAssignment;
use typed_trees::types::TypeReferenceNode;

/// The borrow polarity an assignment demands of its target's root: exclusive
/// for an ordinary write, or the carried atomic operation's own receiver
/// polarity when the assignment is the parser's carrier for that operation
/// on atomic storage.
pub(crate) fn assignment_receiver_polarity(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: Option<&State>,
    assignment: &TableAssignment,
) -> BorrowPolarity {
    target_atomic_operation(program, machine, state, assignment.target, assignment.value)
        .map_or(BorrowPolarity::Exclusive, |operation| {
            AccessOperation::Atomic(operation).receiver_polarity()
        })
}

/// A normalized atomic operation accesses its cell through the operation's
/// receiver; it never replaces a descriptor used to reach that cell. Share
/// exact source decoding with execution rather than recognizing the synthetic
/// arithmetic update formula independently in borrow and origin checks.
pub(crate) fn target_atomic_operation(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: Option<&State>,
    target: ExpressionHandle,
    value: ExpressionHandle,
) -> Option<AtomicAccessOperation> {
    if !place_is_atomic_storage(program, machine, state, target) {
        return None;
    }
    let ExpressionNode::Atomic(atomic) = program.expression_table.expression(value) else {
        return None;
    };
    atomic_write_operands(program, atomic).map(|(operation, _)| operation)
}

/// Whether a place's declared type is one of the dedicated atomic core types.
/// Classification goes through the compiler-installed builtin atom, so a
/// package type that merely spells `Atomic...` does not qualify and an
/// ordinary integer is never implicitly atomic.
pub fn place_is_atomic_storage(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: Option<&State>,
    place: ExpressionHandle,
) -> bool {
    let Some(declared) =
        crate::value_custody::places::declared_place_type(program, machine, state, place)
    else {
        return false;
    };
    match program.type_reference_table.type_reference(declared) {
        TypeReferenceNode::Named { symbol, .. } => program
            .symbols
            .builtin_type_atom(*symbol)
            .is_some_and(|atom| atom.is_atomic()),
        _ => false,
    }
}

/// One atomic source carrier decoded into its sealed operation, the place it
/// accesses, and its authored operands.
///
/// The parser spells every atomic operation through an arithmetic-shaped
/// carrier (tokens-to-syntax-trees `atomic_lets.rs`): a load is the value
/// `Atomic { value: place, .. }`, and every writing operation is the
/// assignment `place = Atomic { .. }` whose `value` models the new resident in
/// terms of the observed prior. That model exists for the checked interpreter;
/// execution needs the operation it denotes. This decoder is the one owner of
/// that recovery, shared by checked planning and by lowering's independent
/// source correspondence, so the two cannot drift apart on which operand an
/// event reads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AtomicCarrierAccess {
    pub operation: AtomicAccessOperation,
    /// The accessed atomic place: the load's own operand, or the carrier
    /// assignment's target.
    pub place: ExpressionHandle,
    /// Authored operands in event order: the stored or swapped value, the
    /// fetch operand, or the compare-exchange expected and replacement.
    pub operands: Vec<ExpressionHandle>,
    /// The generated result name the observed prior binds; invalid for a
    /// load (whose value is the carrier itself) and a store.
    pub result: ExpressionHandle,
}

/// A single attempt is one structural local initializer. Its retained call is
/// a source operand roster, not an ordinary invocation or a scalar update
/// formula. Only the local's checked event plan may establish its result.
pub fn atomic_once_carrier(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
) -> Option<AtomicCarrierAccess> {
    let ExpressionNode::Atomic(atomic) = program.expression_table.expression(expression) else {
        return None;
    };
    atomic_once_operands(program, atomic)
}

pub fn atomic_once_operands(
    program: &dyn crate::CheckingInput,
    atomic: &TableAtomicExpression,
) -> Option<AtomicCarrierAccess> {
    let AtomicOrderingPlan::CompareExchangeOnce { success, failure } = atomic.ordering else {
        return None;
    };
    if !atomic.result_custody.is_valid_for(atomic.ordering)
        || atomic.result.is_valid()
        || !failure.valid_compare_exchange_failure(success)
    {
        return None;
    }
    let ExpressionNode::Call(call) = program.expression_table.expression(atomic.value) else {
        return None;
    };
    let [expected, replacement] = program.expression_table.expression_handles(call.arguments)
    else {
        return None;
    };
    if !call.receiver.is_valid()
        || call.target.as_str() != "compare_exchange_once"
        || !call.machine_arguments.is_empty()
        || !call.evidence_arguments.is_empty()
    {
        return None;
    }
    Some(AtomicCarrierAccess {
        operation: AtomicAccessOperation::CompareExchangeOnce { success, failure },
        place: call.receiver,
        operands: vec![*expected, *replacement],
        result: ExpressionHandle::invalid(),
    })
}

/// Executable children of an atomic source carrier. The Once call node is
/// retained data, never a callable declaration; authored operand calls remain
/// ordinary children in receiver/expected/replacement order.
pub fn atomic_execution_children(
    program: &dyn crate::CheckingInput,
    atomic: &TableAtomicExpression,
) -> Vec<ExpressionHandle> {
    atomic_once_operands(program, atomic)
        .map(|carrier| {
            std::iter::once(carrier.place)
                .chain(carrier.operands)
                .collect()
        })
        .unwrap_or_else(|| vec![atomic.value])
}

/// The exact source place a checked single-attempt operation may replace.
/// This query supplies a storage effect, not an exchange-success or publication
/// fact. A same-spelled member on ordinary storage grants no atomic effect.
pub fn atomic_once_write_place(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
) -> Option<ExpressionHandle> {
    let carrier = atomic_once_carrier(program, expression)?;
    place_is_atomic_storage(program, machine, Some(state), carrier.place).then_some(carrier.place)
}

/// The receiving sum must be the actual sealed core application, including
/// its exact payloads. A same-spelled authored type or a custody enum alone
/// cannot establish this join. Restricting the resident to a plain builtin
/// scalar also prevents an event from inventing a bounded-value proof.
pub fn atomic_once_result_type(
    program: &dyn crate::CheckingInput,
    reference: typed_trees::types::TypeReferenceHandle,
    primitive: typed_trees::types::PrimitiveType,
) -> bool {
    fn check(
        program: &dyn crate::CheckingInput,
        reference: typed_trees::types::TypeReferenceHandle,
        primitive: typed_trees::types::PrimitiveType,
    ) -> Option<()> {
        let data = program.closed_data_definition(reference)?;
        let origin = data.generic_instance.unwrap_or(reference);
        let TypeReferenceNode::Generic {
            base_symbol,
            arguments,
            lifetime_arguments,
            ..
        } = program.type_reference_table.type_reference(origin)
        else {
            return None;
        };
        if !lifetime_arguments.is_empty()
            || program
                .normalized_hermetic_symbol_identity(*base_symbol)
                .ok()
                .as_deref()
                != Some("toolchain::AtomicCompareExchangeOnceOutcome")
            || program.symbol_toolchain_relative_path(*base_symbol)
                != Some(std::path::Path::new("layout.omg"))
            || data.properties.multiplicity != language_semantics::Multiplicity::Unrestricted
            || !data.where_facts.is_empty()
            || data.zero_gated
            || data.quotient.is_some()
        {
            return None;
        }
        let [resident] = program
            .type_reference_table
            .type_reference_handles(*arguments)
        else {
            return None;
        };
        let TypeReferenceNode::Named { symbol, .. } =
            program.type_reference_table.type_reference(*resident)
        else {
            return None;
        };
        if program.symbols.builtin_type_atom(*symbol)?.is_atomic()
            || program.primitive_type_reference(*resident)? != primitive
        {
            return None;
        }
        let members = program.data_members(data);
        if members.len() != 3 {
            return None;
        }
        for (index, member) in members.iter().enumerate() {
            let typed_trees::data::DataMember::Variant(case) = member else {
                return None;
            };
            let (name, payload) = [
                ("Mismatched", true),
                ("Exchanged", false),
                ("Uncommitted", true),
            ][index];
            if case.name.as_str() != name || !case.where_facts.is_empty() {
                return None;
            }
            let fields = program.data_payload_fields(case);
            if !payload {
                if !fields.is_empty() {
                    return None;
                }
            } else {
                let [field] = fields else {
                    return None;
                };
                if field.name.as_str() != "observed"
                    || field.relevance.is_erased()
                    || program.normalized_type_identity(field.type_reference)
                        != program.normalized_type_identity(*resident)
                {
                    return None;
                }
            }
        }
        Some(())
    }
    check(program, reference, primitive).is_some()
}

/// Decode a load carrier: `place.load(ordering)` in value position.
pub fn atomic_load_carrier(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
) -> Option<AtomicCarrierAccess> {
    let ExpressionNode::Atomic(atomic) = program.expression_table.expression(expression) else {
        return None;
    };
    let AtomicOrderingPlan::Load(ordering) = atomic.ordering else {
        return None;
    };
    (ordering.valid_for_load()
        && atomic.result_custody.is_valid_for(atomic.ordering)
        && !atomic.result_custody.has_structural_result()
        && !atomic.result.is_valid())
    .then_some(AtomicCarrierAccess {
        operation: AtomicAccessOperation::Load(ordering),
        place: atomic.value,
        operands: Vec::new(),
        result: ExpressionHandle::invalid(),
    })
}

/// The parser reserves this binding for the adjacent indivisible event. Its
/// synthetic initializer is never the observed value or an establishment proof.
pub fn atomic_result_placeholder(
    program: &dyn crate::CheckingInput,
    state: &State,
    local: &typed_trees::statement::TableLocalData,
) -> Option<AtomicCarrierAccess> {
    if local.role != language_core::LocalBindingRole::AtomicObservedResult {
        return None;
    }
    let (ordinal, _) = program.state_local_declaration(state, local.symbol)?;
    let typed_trees::statement::StatementNode::Assignment(assignment) = program
        .statement_table
        .statements(state.statement_nodes)
        .get(ordinal + 1)?
    else {
        return None;
    };
    let carrier = atomic_assignment_carrier(program, assignment)?;
    let path = program.expression_table.name_path_node(carrier.result)?;
    let matches = if path.symbol.is_valid() {
        path.symbol == local.symbol
    } else {
        matches!(program.expression_table.name_path_members(path.members), [name] if *name == local.name)
    };
    matches.then_some(carrier)
}

/// Decode a writing carrier: `target = Atomic { .. }` for store, fetch,
/// swap, or decisive compare-exchange. The arithmetic model must be exactly
/// the desugar's shape, with every observed-prior placeholder naming the
/// result; any other value, an illegal ordering, or the single-attempt form
/// (whose `Uncommitted` outcome has no scalar prior) decodes to nothing.
pub fn atomic_assignment_carrier(
    program: &dyn crate::CheckingInput,
    assignment: &TableAssignment,
) -> Option<AtomicCarrierAccess> {
    let ExpressionNode::Atomic(atomic) = program.expression_table.expression(assignment.value)
    else {
        return None;
    };
    let (operation, operands) = atomic_write_operands(program, atomic)?;
    Some(AtomicCarrierAccess {
        operation,
        place: assignment.target,
        operands,
        result: atomic.result,
    })
}

/// Evaluated source operands, excluding the synthetic resident-update model
/// and the result destination. Formation checks must use this same decoding as
/// execution: compare-exchange does not evaluate a subtraction of replacement
/// and prior, and fetch arithmetic has the sealed atomic operation's policy.
pub fn atomic_value_operands(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
) -> Option<Vec<ExpressionHandle>> {
    if let Some(once) = atomic_once_carrier(program, expression) {
        return Some(std::iter::once(once.place).chain(once.operands).collect());
    }
    if let Some(load) = atomic_load_carrier(program, expression) {
        return Some(vec![load.place]);
    }
    let ExpressionNode::Atomic(atomic) = program.expression_table.expression(expression) else {
        return None;
    };
    atomic_write_operands(program, atomic).map(|(_, operands)| operands)
}

/// The sealed ordinary value carrier the atomic cell receives. Anonymous
/// literals have no standalone result type; their checked operation supplies
/// this receiving type just as an ordinary call supplies each formal type.
/// Cell and resident identities remain distinct even when layout is equal.
pub fn atomic_resident_type_reference(
    program: &dyn crate::CheckingInput,
    cell: typed_trees::types::TypeReferenceHandle,
) -> Option<typed_trees::types::TypeReferenceHandle> {
    let cell = program.type_reference_table.peeled_type_reference(cell);
    if let Some((_, field)) = program.placed_view_field_plan_for_type_reference(cell)
        && matches!(field.access, access_plans::FieldAccess::Atomic { .. })
    {
        return Some(field.value_type);
    }
    let TypeReferenceNode::Named { symbol, .. } = program.type_reference_table.type_reference(cell)
    else {
        return None;
    };
    if !program.symbols.builtin_type_atom(*symbol)?.is_atomic() {
        return None;
    }
    let primitive = program.primitive_type_reference(cell)?;
    let mut residents = program
        .symbols
        .child_handles(program.symbols.root())?
        .filter_map(|symbol| {
            let atom = program.symbols.builtin_type_atom(symbol)?;
            if atom.is_atomic() {
                return None;
            }
            let reference = program
                .type_reference_table
                .find_named_type_reference(symbol)?;
            (program.primitive_type_reference(reference) == Some(primitive)).then_some(reference)
        });
    let resident = residents.next()?;
    residents.next().is_none().then_some(resident)
}

fn atomic_write_operands(
    program: &dyn crate::CheckingInput,
    atomic: &TableAtomicExpression,
) -> Option<(AtomicAccessOperation, Vec<ExpressionHandle>)> {
    if !atomic.result_custody.is_valid_for(atomic.ordering)
        || atomic.result_custody.has_structural_result()
    {
        return None;
    }
    let binary = |expression| match program.expression_table.expression(expression) {
        ExpressionNode::Binary(binary) => Some(*binary),
        _ => None,
    };
    let prior = |expression| same_result_name(program, expression, atomic.result);
    Some(match atomic.ordering {
        AtomicOrderingPlan::Load(_) | AtomicOrderingPlan::CompareExchangeOnce { .. } => {
            return None;
        }
        AtomicOrderingPlan::Store(ordering) => {
            if !ordering.valid_for_store() || atomic.result.is_valid() {
                return None;
            }
            (AtomicAccessOperation::Store(ordering), vec![atomic.value])
        }
        AtomicOrderingPlan::Swap(ordering) => {
            if !atomic.result.is_valid() {
                return None;
            }
            (AtomicAccessOperation::Swap(ordering), vec![atomic.value])
        }
        AtomicOrderingPlan::ReadModifyWrite(ordering) => {
            let model = binary(atomic.value)?;
            if !prior(model.left) {
                return None;
            }
            let operation = match model.operator {
                BinaryOperator::Add => AtomicAccessOperation::FetchAdd(ordering),
                BinaryOperator::Subtract => AtomicAccessOperation::FetchSub(ordering),
                BinaryOperator::BitwiseXor => AtomicAccessOperation::FetchXor(ordering),
                BinaryOperator::BitwiseOr => AtomicAccessOperation::FetchOr(ordering),
                BinaryOperator::BitwiseAnd => AtomicAccessOperation::FetchAnd(ordering),
                _ => return None,
            };
            (operation, vec![model.right])
        }
        AtomicOrderingPlan::CompareExchange { success, failure } => {
            if !failure.valid_compare_exchange_failure(success) {
                return None;
            }
            // prior + (prior == expected) * (replacement - prior)
            let sum = binary(atomic.value)?;
            let product = binary(sum.right)?;
            let equal = binary(product.left)?;
            let difference = binary(product.right)?;
            if sum.operator != BinaryOperator::Add
                || product.operator != BinaryOperator::Multiply
                || equal.operator != BinaryOperator::Equal
                || difference.operator != BinaryOperator::Subtract
                || !prior(sum.left)
                || !prior(equal.left)
                || !prior(difference.right)
            {
                return None;
            }
            (
                AtomicAccessOperation::CompareExchange { success, failure },
                vec![equal.right, difference.left],
            )
        }
    })
}

/// Whether `expression` is a one-member name spelling the same member as
/// the carrier's result name. The desugar generates every prior placeholder
/// with the result binding's own name; binding that name to the actual local
/// is the consumer's separate check.
fn same_result_name(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
    result: ExpressionHandle,
) -> bool {
    let member = |expression: ExpressionHandle| {
        if !program.expression_table.expression_is_valid(expression) {
            return None;
        }
        let path = program.expression_table.name_path_node(expression)?;
        match program.expression_table.name_path_members(path.members) {
            [member] => Some(member.clone()),
            _ => None,
        }
    };
    matches!((member(expression), member(result)), (Some(left), Some(right)) if left == right)
}
