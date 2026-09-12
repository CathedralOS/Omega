//! Guard facts and build-time execution require the current node's builtin
//! meaning. Authored case membership retains its selections on an equality
//! node; structural synthesis instead retains an explicit CaseMembership
//! operation. Both must rejoin the exact nominal subject and case classifier,
//! rather than treating a payload-bearing classifier as a constructor value.

use crate::places::declared_place_type_raw;
use language_core::OperatorSpelling;
use typed_trees::data::{DataDefinition, DataMember};
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use typed_trees::machine::Machine;
use typed_trees::state::State;
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};
use typed_trees::{
    AuthoredDeclarationSelectionKind, AuthoredDeclarationSelectionTarget, TypedTrees,
};

#[cfg(test)]
mod tests;

/// Preserve selected operator meaning before interpreting an expression as a
/// primitive bound. This grants no value, effect, or lifetime proof: callers
/// must still establish their own range and evaluation-snapshot obligations.
pub fn has_builtin_bound_expression_meaning(
    program: &TypedTrees,
    machine: &Machine,
    state: Option<&State>,
    expression: ExpressionHandle,
) -> bool {
    bound_subtree_meaning(program, machine, state, expression, 0)
}

/// Check a guard node whose Boolean children the caller decomposes and checks
/// separately. A true conjunction (or false disjunction) can contribute an
/// independent builtin fact even when another child selects an authored
/// operator. Boolean wrappers still owe their own exact equality meaning.
pub fn has_builtin_decomposed_guard_meaning(
    program: &TypedTrees,
    machine: &Machine,
    state: Option<&State>,
    expression: ExpressionHandle,
) -> bool {
    if let ExpressionNode::Binary(binary) = program.expression_table.expression(expression) {
        match binary.operator {
            BinaryOperator::And | BinaryOperator::Or => return true,
            BinaryOperator::Equal | BinaryOperator::NotEqual
                if [binary.left, binary.right].into_iter().any(|operand| {
                    matches!(
                        program.expression_table.expression(operand),
                        ExpressionNode::Boolean(_)
                    )
                }) =>
            {
                return builtin_boolean_equality(program, machine, state, expression, binary);
            }
            _ => {}
        }
    }
    has_builtin_bound_expression_meaning(program, machine, state, expression)
}

/// Check one binary node's selected meaning; callers own operand traversal.
/// This grants no value, totality, effect, or lifetime evidence.
pub fn has_builtin_binary_expression_meaning(
    program: &TypedTrees,
    machine: &Machine,
    state: Option<&State>,
    expression: ExpressionHandle,
) -> bool {
    if !program.expression_table.expression_is_valid(expression) {
        return false;
    }
    let ExpressionNode::Binary(binary) = program.expression_table.expression(expression) else {
        return false;
    };
    match binary.operator {
        BinaryOperator::CaseMembership => {
            has_exact_case_membership_meaning(program, machine, state, expression, binary)
        }
        BinaryOperator::Equal | BinaryOperator::NotEqual => {
            builtin_boolean_equality(program, machine, state, expression, binary)
        }
        BinaryOperator::Less
        | BinaryOperator::LessOrEqual
        | BinaryOperator::Greater
        | BinaryOperator::GreaterOrEqual => {
            builtin_ordering(program, machine, state, expression, binary)
        }
        BinaryOperator::Add
        | BinaryOperator::Subtract
        | BinaryOperator::Multiply
        | BinaryOperator::Divide
        | BinaryOperator::Modulo => {
            builtin_arithmetic_node(program, machine, state, expression, binary)
        }
        // These operations have no overloadable operator spelling.
        BinaryOperator::And
        | BinaryOperator::Or
        | BinaryOperator::BitwiseAnd
        | BinaryOperator::BitwiseOr
        | BinaryOperator::BitwiseXor
        | BinaryOperator::ShiftLeft
        | BinaryOperator::ShiftRight => true,
    }
}

fn bound_subtree_meaning(
    program: &TypedTrees,
    machine: &Machine,
    state: Option<&State>,
    expression: ExpressionHandle,
    depth: usize,
) -> bool {
    if !expression.is_valid() || depth >= 128 {
        return false;
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Binary(binary) => {
            let meaning =
                has_builtin_binary_expression_meaning(program, machine, state, expression);
            meaning
                && bound_subtree_meaning(program, machine, state, binary.left, depth + 1)
                && bound_subtree_meaning(program, machine, state, binary.right, depth + 1)
        }
        ExpressionNode::Borrow(borrow) => {
            bound_subtree_meaning(program, machine, state, borrow.target, depth + 1)
        }
        ExpressionNode::Atomic(atomic) => {
            bound_subtree_meaning(program, machine, state, atomic.value, depth + 1)
        }
        ExpressionNode::Unary(unary) => {
            bound_subtree_meaning(program, machine, state, unary.operand, depth + 1)
        }
        ExpressionNode::Cast(cast) => {
            bound_subtree_meaning(program, machine, state, cast.value, depth + 1)
        }
        // Places and calls are symbolic leaves, not interpreted arithmetic.
        _ => true,
    }
}

pub(super) fn builtin_ordering(
    program: &TypedTrees,
    machine: &Machine,
    state: Option<&State>,
    expression: ExpressionHandle,
    comparison: &typed_trees::expression::TableBinaryExpression,
) -> bool {
    let spelling = match comparison.operator {
        BinaryOperator::Less => OperatorSpelling::Less,
        BinaryOperator::LessOrEqual => OperatorSpelling::LessEqual,
        BinaryOperator::Greater => OperatorSpelling::Greater,
        BinaryOperator::GreaterOrEqual => OperatorSpelling::GreaterEqual,
        _ => return false,
    };
    typed_trees::operator::has_builtin_spelled_expression_meaning(
        program,
        machine.symbol,
        expression,
        spelling,
        &[
            operand_type(program, machine, state, comparison.left),
            operand_type(program, machine, state, comparison.right),
        ],
    )
}

pub(super) fn builtin_boolean_equality(
    program: &TypedTrees,
    machine: &Machine,
    state: Option<&State>,
    expression: ExpressionHandle,
    comparison: &typed_trees::expression::TableBinaryExpression,
) -> bool {
    // Typed lowering encodes `value in Type::Case` as equality, but retains
    // its case selections. Membership tests the tag, not authored value
    // equality. Rejoin those selections before applying operator rules; this
    // does not admit the declarations or waive callers' operand checks.
    if has_exact_case_membership_meaning(program, machine, state, expression, comparison) {
        return true;
    }
    let spelling = match comparison.operator {
        BinaryOperator::Equal => OperatorSpelling::Equal,
        BinaryOperator::NotEqual => OperatorSpelling::NotEqual,
        _ => return false,
    };
    // Parser-generated arm comparisons are ordinary equality expressions too.
    // Neither their missing authored occurrence nor their Boolean literal is
    // evidence that a visible equality declaration has builtin meaning.
    typed_trees::operator::has_builtin_spelled_expression_meaning(
        program,
        machine.symbol,
        expression,
        spelling,
        &[
            operand_type(program, machine, state, comparison.left),
            operand_type(program, machine, state, comparison.right),
        ],
    )
}

/// Rejoin authored or explicitly generated case membership with its exact
/// subject and carrier. This distinguishes tag observation from ordinary value
/// equality; it does not admit the operands or their declarations.
pub fn has_exact_case_membership_meaning(
    program: &TypedTrees,
    machine: &Machine,
    state: Option<&State>,
    expression: ExpressionHandle,
    comparison: &typed_trees::expression::TableBinaryExpression,
) -> bool {
    if !program.expression_table.expression_is_valid(expression)
        || !program
            .expression_table
            .expression_is_valid(comparison.left)
        || !program
            .expression_table
            .expression_is_valid(comparison.right)
        || !matches!(
            comparison.operator,
            BinaryOperator::Equal | BinaryOperator::CaseMembership
        )
    {
        return false;
    }
    let Some(owner) = exact_case_reference_owner(program, comparison.right) else {
        return false;
    };
    if !membership_subject_matches_owner(program, machine, state, comparison.left, owner) {
        return false;
    }
    // Structural synthesis has an explicit tag operation, not an authored
    // membership occurrence. Its meaning follows from that operation and the
    // independently checked subject/carrier/case relationship above. Plain
    // equality still needs the complete authored membership roster below.
    if comparison.operator == BinaryOperator::CaseMembership {
        return true;
    }
    let ExpressionNode::Name(case) = program.expression_table.expression(comparison.right) else {
        return false;
    };
    let mut has_owner = false;
    let mut has_case = false;
    for occurrence in program
        .expression_table
        .authored_selection_occurrences(expression)
    {
        let Some(selection) = program.authored_declaration_selections().get(occurrence) else {
            return false;
        };
        let AuthoredDeclarationSelectionTarget::Resolved(selected) = selection.target() else {
            return false;
        };
        match selection.kind() {
            AuthoredDeclarationSelectionKind::CaseReference
                if selected.selected_symbol() == owner.symbol && !has_owner =>
            {
                has_owner = true
            }
            AuthoredDeclarationSelectionKind::CaseMembership
                if selected.selected_symbol() == case.symbol && !has_case =>
            {
                has_case = true
            }
            _ => return false,
        }
    }
    has_owner && has_case
}

/// Rejoin a case classifier's complete retained namespace/carrier/case chain.
/// A namespace root names a declaration, not runtime storage. Authored selection
/// and import visibility remain separate source-authority checks; payload and
/// default obligations apply only when the expression is used as a value.
pub fn exact_case_reference_owner(
    program: &TypedTrees,
    expression: ExpressionHandle,
) -> Option<&DataDefinition> {
    if !program.expression_table.expression_is_valid(expression) {
        return None;
    }
    let ExpressionNode::Name(case) = program.expression_table.expression(expression) else {
        return None;
    };
    let table = &program.expression_table;
    let members = table.name_path_members(case.members);
    let selected = table.name_path_member_symbols(case.member_symbols);
    if members.len() < 2
        || selected.len() != members.len()
        || selected.first() != Some(&case.head_symbol)
        || selected.last() != Some(&case.symbol)
        || selected.iter().any(|symbol| !symbol.is_valid())
        || selected[..selected.len() - 2]
            .iter()
            .any(|symbol| program.symbols.get(*symbol).kind != symbols::SymbolKind::Module)
        || selected.windows(2).any(|pair| {
            !program
                .symbols
                .child_handles(pair[0])
                .is_some_and(|mut children| children.any(|child| child == pair[1]))
        })
    {
        return None;
    }
    let carrier = selected[selected.len() - 2];
    program.data_definitions().iter().find(|owner| {
        owner.symbol == carrier
            && program.symbols.get(case.symbol).parent == carrier
            && program.data_members(owner).iter().any(|member| {
                matches!(member, DataMember::Variant(variant) if variant.symbol == case.symbol)
            })
    })
}

fn membership_subject_matches_owner(
    program: &TypedTrees,
    machine: &Machine,
    state: Option<&State>,
    subject: ExpressionHandle,
    owner: &DataDefinition,
) -> bool {
    // Membership observes the borrowed value's tag. Peel explicit borrowing
    // only for this nominal-owner check: ordinary operator lookup must retain
    // reference carriers so reference-typed operator candidates remain visible.
    // Access mode and loan legality are still checked at the authored borrow.
    let mut subject = subject;
    let mut borrow_depth = 0;
    while let ExpressionNode::Borrow(borrow) = program.expression_table.expression(subject) {
        if borrow_depth >= 128 {
            return false;
        }
        borrow_depth += 1;
        subject = borrow.target;
    }
    // Fresh values retain their own nominal identity without a declared place
    // type. Do not require an artificial local binding or borrow the tested
    // case's type as evidence for an otherwise unknown subject. Field
    // construction and default-domain obligations remain separate checks.
    match program.expression_table.expression(subject) {
        ExpressionNode::Name(path)
            if path.symbol == machine.symbol && path.head_symbol == machine.symbol =>
        {
            // Attached `self` is rooted at the machine symbol, not the entry
            // parameter symbol. Rejoin that exact owner; its diagnostic name
            // alone cannot authorize a foreign receiver's tag observation.
            return machine.symbol.is_valid()
                && machine.attached_data_symbol == owner.symbol
                && matches!(program.expression_table.name_path_members(path.members),
                    [name] if name.as_str() == "self")
                && program
                    .expression_table
                    .name_path_member_symbols(path.member_symbols)
                    == [machine.symbol]
                && state.map_or_else(
                    || {
                        program.machine_states(machine).iter().any(|state| {
                            program
                                .state_parameters(state)
                                .iter()
                                .any(|parameter| parameter.is_self)
                        })
                    },
                    |state| {
                        // A receiver on another callable state cannot establish
                        // this machine's `self`, even for the same nominal type.
                        program
                            .machine_states(machine)
                            .iter()
                            .any(|candidate| candidate.symbol == state.symbol)
                            && program
                                .state_parameters(state)
                                .iter()
                                .any(|parameter| parameter.is_self)
                    },
                );
        }
        ExpressionNode::StructLiteral(literal) => {
            return literal.type_symbol == owner.symbol
                && program.data_members(owner).iter().any(|member| {
                    matches!(member, DataMember::Variant(variant)
                        if literal.case_symbol == Some(variant.symbol))
                });
        }
        ExpressionNode::Name(path) if path.head_symbol != path.symbol => {
            return exact_case_reference_owner(program, subject)
                .is_some_and(|subject_owner| subject_owner.symbol == owner.symbol)
                // A bare case is a value only without a payload. RHS case
                // domains intentionally do not have this restriction.
                && program.data_members(owner).iter().any(|member| {
                    matches!(member, DataMember::Variant(variant)
                        if variant.symbol == path.symbol && variant.payload.is_empty())
                });
        }
        _ => {}
    }
    let Some(subject_type) = operand_type(program, machine, state, subject)
        .and_then(|reference| crate::places::unwrapped_type_reference(program, reference))
    else {
        return false;
    };
    // Only nominal subjects match. In particular, type_symbol() also walks
    // array/slice element types, which cannot establish a collection's tag.
    match program.type_reference_table.type_reference(subject_type) {
        TypeReferenceNode::Named { symbol, .. } => *symbol == owner.symbol,
        TypeReferenceNode::Generic { base_symbol, .. } => *base_symbol == owner.symbol,
        _ => false,
    }
}

fn operand_type(
    program: &TypedTrees,
    machine: &Machine,
    state: Option<&State>,
    expression: ExpressionHandle,
) -> Option<TypeReferenceHandle> {
    operand_type_at_depth(program, machine, state, expression, 0)
}

fn operand_type_at_depth(
    program: &TypedTrees,
    machine: &Machine,
    state: Option<&State>,
    expression: ExpressionHandle,
    depth: usize,
) -> Option<TypeReferenceHandle> {
    if depth >= 128 || !program.expression_table.expression_is_valid(expression) {
        return None;
    }
    // This is type lookup, not an immutable-value proof: mutable parameters
    // and locals keep ordinary evaluation-snapshot narrowing. Anonymous literal and
    // unresolved computed types remain wildcard candidates, never an assumed
    // copy of the other operand's carrier that could hide an overload.
    let reference = match program.expression_table.expression(expression) {
        ExpressionNode::Boolean(_) => {
            builtin_type_reference(program, symbols::BuiltinTypeAtom::Bool)
        }
        ExpressionNode::Integer(_) => {
            crate::operators::landed_integer_literal_type_reference(program, expression)
        }
        ExpressionNode::Name(path) if path.symbol.is_valid() && path.head_symbol == path.symbol => {
            crate::expression_types::named_value_type_reference(program, path)
        }
        ExpressionNode::Member(_)
            if crate::places::collection_length_receiver(program, machine, state, expression)
                .is_some() =>
        {
            builtin_type_reference(program, symbols::BuiltinTypeAtom::U64)
        }
        ExpressionNode::Member(_) | ExpressionNode::Indexed(_) | ExpressionNode::Call(_) => {
            declared_place_type_raw(program, machine, state, expression)
        }
        ExpressionNode::Cast(cast) => Some(cast.target_type),
        ExpressionNode::ZeroValue(type_reference) => Some(*type_reference),
        ExpressionNode::Binary(binary) => {
            let spelling = match binary.operator {
                BinaryOperator::Add => OperatorSpelling::Add,
                BinaryOperator::Subtract => OperatorSpelling::Subtract,
                BinaryOperator::Multiply => OperatorSpelling::Multiply,
                BinaryOperator::Divide => OperatorSpelling::Divide,
                BinaryOperator::Modulo => OperatorSpelling::Modulo,
                _ => return None,
            };
            let operands = [binary.left, binary.right];
            let types = operands
                .map(|operand| operand_type_at_depth(program, machine, state, operand, depth + 1));
            let carrier = types.into_iter().flatten().next()?;
            // Primitive lookup permits constraint shells but not references.
            // Check that before the structural helper unwraps either shell.
            program.primitive_type_reference(carrier)?;
            let unwrapped = crate::places::unwrapped_type_reference(program, carrier)?;
            let typed_trees::types::TypeReferenceNode::Named { symbol, .. } =
                program.type_reference_table.type_reference(unwrapped)
            else {
                return None;
            };
            if !matches!(
                program.symbols.builtin_type_atom(*symbol),
                Some(
                    symbols::BuiltinTypeAtom::I8
                        | symbols::BuiltinTypeAtom::I16
                        | symbols::BuiltinTypeAtom::I32
                        | symbols::BuiltinTypeAtom::I64
                        | symbols::BuiltinTypeAtom::U8
                        | symbols::BuiltinTypeAtom::U16
                        | symbols::BuiltinTypeAtom::U32
                        | symbols::BuiltinTypeAtom::U64
                )
            ) || !operands
                .into_iter()
                .zip(types)
                .all(|(operand, reference)| match reference {
                    Some(reference) => {
                        program.normalized_type_identity(reference)
                            == program.normalized_type_identity(carrier)
                    }
                    None => matches!(program.expression_table.expression(operand),
                            ExpressionNode::Integer(literal) if literal.landing().is_none()),
                })
                || !typed_trees::operator::has_builtin_spelled_expression_meaning(
                    program,
                    machine.symbol,
                    expression,
                    spelling,
                    &types,
                )
            {
                return None;
            }
            // Recover a carrier only after checking this operation's meaning.
            // Keep the known binding's semantic shell conservatively; no range
            // or qualification proof is inferred for the computed value here.
            Some(carrier)
        }
        // In particular, do not let declared_place_type_raw erase a Borrow
        // shell and incorrectly rule out a reference-typed operator candidate.
        _ => None,
    }?;
    program
        .type_reference_table
        .contains_type_reference(reference)
        .then_some(reference)
}

fn builtin_type_reference(
    program: &TypedTrees,
    atom: symbols::BuiltinTypeAtom,
) -> Option<TypeReferenceHandle> {
    // Builtin result types are independent of the sibling expression. Reuse an
    // actual reference to the exact compiler builtin atom;
    // do not manufacture a handle or identify a same-spelled user declaration.
    let symbol = program
        .symbols
        .child_handles(program.symbols.root())?
        .find(|symbol| program.symbols.builtin_type_atom(*symbol) == Some(atom))?;
    program
        .type_reference_table
        .find_named_type_reference(symbol)
}

/// A recognized numeric bound must retain the meaning of the arithmetic
/// expression it consumes, independently of the body's eventual operator.
pub(super) fn builtin_arithmetic(
    program: &TypedTrees,
    machine: &Machine,
    state: Option<&State>,
    expression: ExpressionHandle,
) -> bool {
    let ExpressionNode::Binary(binary) = program.expression_table.expression(expression) else {
        return false;
    };
    builtin_arithmetic_node(program, machine, state, expression, binary)
        && folded_constant_is_builtin(program, machine, state, binary.left)
        && folded_constant_is_builtin(program, machine, state, binary.right)
}

fn builtin_arithmetic_node(
    program: &TypedTrees,
    machine: &Machine,
    state: Option<&State>,
    expression: ExpressionHandle,
    binary: &typed_trees::expression::TableBinaryExpression,
) -> bool {
    let spelling = match binary.operator {
        BinaryOperator::Add => OperatorSpelling::Add,
        BinaryOperator::Subtract => OperatorSpelling::Subtract,
        BinaryOperator::Multiply => OperatorSpelling::Multiply,
        BinaryOperator::Divide => OperatorSpelling::Divide,
        BinaryOperator::Modulo => OperatorSpelling::Modulo,
        _ => return false,
    };
    typed_trees::operator::has_builtin_spelled_expression_meaning(
        program,
        machine.symbol,
        expression,
        spelling,
        &[
            operand_type(program, machine, state, binary.left),
            operand_type(program, machine, state, binary.right),
        ],
    )
}

/// `constant_integer_value` evaluates syntax, not selected declarations.
/// Check only the constant-shaped subtree it could fold; a nonconstant place
/// remains the responsibility of the calling relation recognizer.
pub(super) fn folded_constant_is_builtin(
    program: &TypedTrees,
    machine: &Machine,
    state: Option<&State>,
    expression: ExpressionHandle,
) -> bool {
    constant_subtree_meaning(program, machine, state, expression, 0).unwrap_or(true)
}

fn constant_subtree_meaning(
    program: &TypedTrees,
    machine: &Machine,
    state: Option<&State>,
    expression: ExpressionHandle,
    depth: usize,
) -> Option<bool> {
    if depth >= 128 {
        return Some(false);
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Integer(_) => Some(true),
        ExpressionNode::Borrow(borrow) => {
            constant_subtree_meaning(program, machine, state, borrow.target, depth + 1)
        }
        ExpressionNode::Binary(binary) => {
            let left = constant_subtree_meaning(program, machine, state, binary.left, depth + 1)?;
            let right = constant_subtree_meaning(program, machine, state, binary.right, depth + 1)?;
            Some(
                left && right
                    && builtin_arithmetic_node(program, machine, state, expression, binary),
            )
        }
        _ => None,
    }
}
