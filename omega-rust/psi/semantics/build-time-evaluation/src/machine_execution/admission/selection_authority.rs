use semantic_vocabulary::PackageKeyIdentity;
use source::{SourceOrigin, SourceSpan};
use symbols::{SymbolHandle, SymbolKind};
use typed_trees::{
    TypedTrees,
    expression::{BinaryOperator, ExpressionHandle, ExpressionNode},
    machine::Machine,
    state::State,
    statement::{StatementNode, TransitionGuardNode, TransitionTargetNode},
};

use super::{BuildTimeCallEdge, BuildTimeInvocationCustody, BuildTimeSelectionAuthority};

pub(super) fn selection_authority_violation(
    call_edges: &[BuildTimeCallEdge],
    program: &TypedTrees,
    root: &Machine,
    custody: Option<BuildTimeInvocationCustody>,
    authority: Option<&dyn BuildTimeSelectionAuthority>,
    selected: &[crate::SelectedBuildTimeBinaryOperator],
) -> Option<String> {
    if let Some(authority) = authority {
        let Some(custody) = custody else {
            return Some(
                "package-aware build-time evaluation has no authored invocation custody".to_owned(),
            );
        };
        let requester = match custody {
            BuildTimeInvocationCustody::Source(source) => package_for_source(program, source),
            BuildTimeInvocationCustody::Symbol(symbol) => package_for_symbol(program, symbol),
        };
        if let Some(violation) = require_selection(
            program,
            requester,
            package_for_symbol(program, root.symbol),
            authority,
            &format!("build-time invocation of `{}`", root.name),
        ) {
            return Some(violation);
        }
    }

    let mut completed = Vec::new();
    let mut pending = vec![root.symbol];
    while let Some(source_machine) = pending.pop() {
        if completed.contains(&source_machine) {
            continue;
        }
        completed.push(source_machine);
        if let Some(violation) =
            machine_selection_violation(program, source_machine, authority, selected)
        {
            return Some(violation);
        }
        for call in call_edges
            .iter()
            .filter(|call| call.source_machine_symbol == source_machine)
        {
            let Some(target_machine) = target_machine_symbol(program, call) else {
                let Some(authority) = authority else {
                    continue;
                };
                if let Some(operator) = target_operator_symbol(program, call) {
                    let context = format!(
                        "build-time named operator call `{}` -> `{}`",
                        program.symbols.display_path(source_machine, "::"),
                        program.symbols.display_path(operator, "::")
                    );
                    if let Some(violation) = require_selection(
                        program,
                        package_for_symbol(program, source_machine),
                        package_for_symbol(program, operator),
                        authority,
                        &context,
                    ) {
                        return Some(violation);
                    }
                    continue;
                }
                return Some(format!(
                    "build-time call from `{}` has no exact target-machine identity",
                    program.symbols.display_path(source_machine, "::")
                ));
            };
            let context = format!(
                "build-time call `{}` -> `{}`",
                program.symbols.display_path(source_machine, "::"),
                program.symbols.display_path(target_machine, "::")
            );
            if let Some(authority) = authority
                && let Some(violation) = require_selection(
                    program,
                    package_for_symbol(program, source_machine),
                    package_for_symbol(program, target_machine),
                    authority,
                    &context,
                )
            {
                return Some(violation);
            }
            pending.push(target_machine);
        }
    }
    None
}

fn machine_selection_violation(
    program: &TypedTrees,
    machine_symbol: SymbolHandle,
    authority: Option<&dyn BuildTimeSelectionAuthority>,
    selected: &[crate::SelectedBuildTimeBinaryOperator],
) -> Option<String> {
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == machine_symbol)?;
    for state in program.machine_states(machine) {
        let mut expressions = Vec::new();
        for statement in program.statement_table.statements(state.statement_nodes) {
            collect_statement_expressions(program, statement, &mut expressions);
        }

        let mut visited = Vec::new();
        for expression in expressions {
            if let Some(violation) = expression_selection_violation(
                program,
                machine,
                state,
                expression,
                authority,
                selected,
                &mut visited,
            ) {
                return Some(violation);
            }
        }
    }
    None
}

fn collect_statement_expressions(
    program: &TypedTrees,
    statement: &StatementNode,
    expressions: &mut Vec<ExpressionHandle>,
) {
    match statement {
        StatementNode::RootBinding(binding) => {
            expressions.push(binding.receiver);
            if binding.implementation_operand.is_valid() {
                expressions.push(binding.implementation_operand);
            }
        }
        StatementNode::AssemblyFact(_) => {}
        StatementNode::Assignment(assignment) => {
            expressions.push(assignment.target);
            expressions.push(assignment.value);
        }
        StatementNode::Call(call) => expressions.extend(
            program
                .statement_table
                .expression_handles(call.arguments)
                .iter()
                .copied(),
        ),
        StatementNode::Expression(expression) => expressions.push(*expression),
        StatementNode::LocalData(local) if local.initial_value.is_valid() => {
            expressions.push(local.initial_value)
        }
        StatementNode::LocalData(_) => {}
        StatementNode::Transition(transition) => {
            if let TransitionGuardNode::When(guard) = transition.guard {
                expressions.push(guard);
            }
            collect_transition_expressions(program, transition.target, expressions);
            if transition.continuation.is_valid() {
                collect_transition_expressions(program, transition.continuation, expressions);
            }
        }
    }
}

fn collect_transition_expressions(
    program: &TypedTrees,
    target: typed_trees::statement::TransitionTargetHandle,
    expressions: &mut Vec<ExpressionHandle>,
) {
    if !target.is_valid() {
        return;
    }
    match program.statement_table.transition_target(target) {
        TransitionTargetNode::Named { arguments, .. } => expressions.extend(
            program
                .statement_table
                .expression_handles(*arguments)
                .iter()
                .copied(),
        ),
        TransitionTargetNode::Value(expression) => expressions.push(*expression),
        TransitionTargetNode::SelfTarget | TransitionTargetNode::Terminal => {}
    }
}

fn expression_selection_violation(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
    authority: Option<&dyn BuildTimeSelectionAuthority>,
    selected: &[crate::SelectedBuildTimeBinaryOperator],
    visited: &mut Vec<ExpressionHandle>,
) -> Option<String> {
    if !expression.is_valid() || visited.contains(&expression) {
        return None;
    }
    visited.push(expression);

    if matches!(
        program.expression_table.expression(expression),
        ExpressionNode::Binary(_)
    ) && !validation::has_builtin_binary_expression_meaning(
        program,
        machine,
        Some(state),
        expression,
    ) && !selected.iter().any(|row| {
        row.expression == expression && row.origin.machine_symbol() == Some(machine.symbol)
    }) {
        return Some(
            "build-time binary operator requires exact authored selection before evaluation; the evaluator cannot execute it as a builtin operator".to_owned(),
        );
    }

    if let Some(violation) = expression_occurrence_violation(program, expression, authority) {
        return Some(violation);
    }

    let table = &program.expression_table;
    let mut children = Vec::new();
    match table.expression(expression) {
        ExpressionNode::Match(dispatch) => {
            children.push(dispatch.subject);
            for arm in table.match_arms(dispatch.arms) {
                if let typed_trees::expression::MatchPattern::Value(pattern) = arm.pattern {
                    children.push(pattern);
                }
                children.push(arm.value);
            }
        }
        ExpressionNode::ArrayLiteral(values) => {
            children.extend(table.expression_handles(*values).iter().copied())
        }
        ExpressionNode::Atomic(atomic) => {
            children.push(atomic.value);
            if atomic.result.is_valid() {
                children.push(atomic.result);
            }
        }
        ExpressionNode::Binary(binary) => {
            children.push(binary.left);
            children.push(binary.right);
        }
        ExpressionNode::Cast(cast) => children.push(cast.value),
        ExpressionNode::Call(call) => {
            if call.receiver.is_valid() {
                children.push(call.receiver);
            }
            children.extend(table.expression_handles(call.arguments).iter().copied());
        }
        ExpressionNode::Indexed(indexed) => {
            children.push(indexed.collection);
            children.push(indexed.index);
        }
        ExpressionNode::Member(member) => children.push(member.receiver),
        ExpressionNode::Borrow(borrow) => children.push(borrow.target),
        ExpressionNode::Range(range) => {
            if range.start.is_valid() {
                children.push(range.start);
            }
            if range.end.is_valid() {
                children.push(range.end);
            }
        }
        ExpressionNode::StructLiteral(literal) => children.extend(
            table
                .struct_fields(literal.fields)
                .iter()
                .map(|field| field.value),
        ),
        ExpressionNode::Unary(unary) => children.push(unary.operand),
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Name(_)
        | ExpressionNode::String(_)
        | ExpressionNode::ZeroValue(_) => {}
    }
    for child in children {
        if let Some(violation) = expression_selection_violation(
            program, machine, state, child, authority, selected, visited,
        ) {
            return Some(violation);
        }
    }
    None
}

// Numeric type positions have no implicit machine activation. Check the same
// source-owned selections as machine expressions, then establish context-free
// operator meaning through the shared exact query. Never substitute the
// callee's lexical context for the caller's argument expressions.
pub(crate) fn require_call_expression_selection(
    program: &TypedTrees,
    expression: ExpressionHandle,
    authority: Option<&dyn BuildTimeSelectionAuthority>,
) -> Result<(), String> {
    let ExpressionNode::Call(call) = program.expression_table.expression(expression) else {
        return Err("range endpoint lost its original call selection".to_owned());
    };
    if let Some(violation) = expression_occurrence_violation(program, expression, authority) {
        return Err(violation);
    }
    if call.receiver.is_valid()
        && let Some(violation) = expression_occurrence_violation(program, call.receiver, authority)
    {
        return Err(violation);
    }
    Ok(())
}

pub(crate) fn require_closed_integer_argument(
    program: &TypedTrees,
    evaluated: &TypedTrees,
    expression: ExpressionHandle,
    authority: Option<&dyn BuildTimeSelectionAuthority>,
) -> Result<(), String> {
    require_closed_scalar_custody(program, expression, authority)?;
    evaluated.closed_integer_value_in(expression, SymbolHandle::invalid())
        .map(|_| ())
        .ok_or_else(|| "range endpoint argument requires a closed integer expression with context-independent operator meaning".to_owned())
}

/// A Boolean argument position admits Boolean literals, Boolean logic and
/// folded helper calls. Comparisons stay outside: their integer operands
/// would need the same owner-independent meaning the integer gate proves,
/// and the shared scalar evaluator is the only place that can land them.
/// Custody is resolved on the original tree exactly as for integers; the
/// caller lands the value on the working tree through the shared evaluator.
pub(crate) fn require_closed_boolean_argument(
    program: &TypedTrees,
    expression: ExpressionHandle,
    authority: Option<&dyn BuildTimeSelectionAuthority>,
) -> Result<(), String> {
    let mut pending = vec![expression];
    let mut visited = Vec::new();
    while let Some(expression) = pending.pop() {
        if visited.contains(&expression) {
            continue;
        }
        visited.push(expression);
        match program.expression_table.expression(expression) {
            ExpressionNode::Boolean(_) => {
                if let Some(violation) =
                    expression_occurrence_violation(program, expression, authority)
                {
                    return Err(violation);
                }
            }
            ExpressionNode::Binary(binary)
                if matches!(binary.operator, BinaryOperator::And | BinaryOperator::Or) =>
            {
                if let Some(violation) =
                    expression_occurrence_violation(program, expression, authority)
                {
                    return Err(violation);
                }
                pending.push(binary.right);
                pending.push(binary.left);
            }
            ExpressionNode::Call(_) => {
                require_closed_scalar_custody(program, expression, authority)?
            }
            _ => {
                return Err(
                    "range endpoint Boolean argument admits only Boolean literals, Boolean logic and folded helper calls"
                        .to_owned(),
                );
            }
        }
    }
    Ok(())
}

/// Selection custody over one closed scalar argument tree in the original
/// program, where every call still has its target. Integer, decimal and
/// Boolean leaves are all admitted here because a nested helper call may
/// take either kind; the caller's final query on the working tree decides
/// the carrier. Keep traversal complete if the shared evaluators grow new
/// expression forms: each new form needs its own selection walk.
fn require_closed_scalar_custody(
    program: &TypedTrees,
    expression: ExpressionHandle,
    authority: Option<&dyn BuildTimeSelectionAuthority>,
) -> Result<(), String> {
    let mut pending = vec![expression];
    let mut visited = Vec::new();
    while let Some(expression) = pending.pop() {
        if visited.contains(&expression) {
            continue;
        }
        visited.push(expression);
        if matches!(
            program.expression_table.expression(expression),
            ExpressionNode::Call(_)
        ) {
            require_call_expression_selection(program, expression, authority)?;
        } else if let Some(violation) =
            expression_occurrence_violation(program, expression, authority)
        {
            return Err(violation);
        }
        match program.expression_table.expression(expression) {
            ExpressionNode::Call(call) => {
                // The range evaluator admits the exact call before replacing
                // it in its working tree. Resolve occurrence custody here in
                // the original tree, where CheckedCall still has its target.
                pending.extend(
                    program.expression_table.expression_handles(call.arguments).iter().rev().copied(),
                );
            }
            ExpressionNode::Binary(binary) => {
                pending.push(binary.right);
                pending.push(binary.left);
            }
            ExpressionNode::Integer(_) | ExpressionNode::Float(_) | ExpressionNode::Boolean(_) => {}
            _ => return Err("range endpoint argument requires a closed integer expression with context-independent operator meaning".to_owned()),
        }
    }
    Ok(())
}

fn expression_occurrence_violation(
    program: &TypedTrees,
    expression: ExpressionHandle,
    authority: Option<&dyn BuildTimeSelectionAuthority>,
) -> Option<String> {
    if let Some(authority) = authority {
        let occurrences = program
            .expression_table
            .authored_selection_occurrences(expression)
            .collect::<Vec<_>>();
        for (occurrence_offset, occurrence) in occurrences.iter().copied().enumerate() {
            let Some(selection) = program.authored_declaration_selections().get(occurrence) else {
                return Some(format!(
                    "build-time expression retains unknown authored declaration selection occurrence {}",
                    occurrence.ordinal()
                ));
            };
            let requester = package_for_source(program, selection.source_span());
            let owner = match selection.target() {
                typed_trees::AuthoredDeclarationSelectionTarget::Intrinsic(_) => continue,
                typed_trees::AuthoredDeclarationSelectionTarget::LateBound(binding) => {
                    if binding
                        == typed_trees::AuthoredDeclarationSelectionLateBinding::CheckedOperator
                        && (typed_trees_to_checked_trees::typed_operator_has_no_authored_selection(
                            program, expression,
                        ) || unresolved_operator_candidates_are_confined(
                            program, expression, requester, authority,
                        ))
                    {
                        continue;
                    } else {
                        match late_bound_selection_symbol(
                            program,
                            expression,
                            &occurrences[..occurrence_offset],
                            binding,
                        ) {
                            Some(selected) => package_for_symbol(program, selected),
                            None => {
                                if unresolved_spelling_is_confined(
                                    program,
                                    program.symbols.source_text(selection.source_span()),
                                    requester,
                                    authority,
                                    binding,
                                ) {
                                    continue;
                                }
                                return Some(format!(
                                    "build-time expression has unresolved authored {:?} selection `{}` ({binding:?}); package authority must be known before compiler execution",
                                    selection.kind(),
                                    program.symbols.source_text(selection.source_span()),
                                ));
                            }
                        }
                    }
                }
                typed_trees::AuthoredDeclarationSelectionTarget::Resolved(selected) => {
                    package_for_symbol(program, selected.selected_symbol())
                }
            };
            let context = format!("build-time authored {:?} selection", selection.kind());
            if let Some(violation) =
                require_selection(program, requester, owner, authority, &context)
            {
                return Some(violation);
            }
        }
    }

    None
}

/// Confinement of a late-bound `CheckedOperator` occurrence is vacuous when
/// the operand-filtered candidate set is empty: `resolve_spelling_for_operands`
/// is the single use-site resolution authority, so no authored operator
/// declaration can be selected for this expression. The checked stage then
/// resolves the occurrence to builtin meaning (the enclosing admission walk
/// already proved builtin meaning for binary nodes) or rejects the program on
/// its own; either way no foreign package authority is consumed.
fn unresolved_operator_candidates_are_confined(
    program: &TypedTrees,
    expression: ExpressionHandle,
    requester: PackageCustody,
    authority: &dyn BuildTimeSelectionAuthority,
) -> bool {
    let candidates = typed_trees_to_checked_trees::typed_operator_authored_selection_candidates(
        program, expression,
    );
    candidates.into_iter().all(|candidate| {
        require_selection(
            program,
            requester,
            package_for_symbol(program, candidate),
            authority,
            "candidate for an unresolved build-time operator selection",
        )
        .is_none()
    })
}

fn unresolved_spelling_is_confined(
    program: &TypedTrees,
    spelling: &str,
    requester: PackageCustody,
    authority: &dyn BuildTimeSelectionAuthority,
    binding: typed_trees::AuthoredDeclarationSelectionLateBinding,
) -> bool {
    use typed_trees::AuthoredDeclarationSelectionLateBinding as Binding;

    if !matches!(
        binding,
        Binding::CheckedCall
            | Binding::CheckedMember
            | Binding::CheckedStaticPathSegment
            | Binding::CheckedStructLiteralType
            | Binding::CheckedStructLiteralCase
            | Binding::CheckedStructLiteralField
    ) {
        return false;
    }
    let candidates = program
        .symbols
        .symbols()
        .nodes()
        .iter()
        .filter_map(|(symbol, data)| {
            (program.symbols.name(symbol) == spelling
                && candidate_kind_matches_binding(data.kind, binding))
            .then_some(symbol)
        })
        .collect::<Vec<_>>();
    !candidates.is_empty()
        && candidates.into_iter().all(|candidate| {
            require_selection(
                program,
                requester,
                package_for_symbol(program, candidate),
                authority,
                "candidate for an unresolved build-time declaration selection",
            )
            .is_none()
        })
}

fn candidate_kind_matches_binding(
    kind: SymbolKind,
    binding: typed_trees::AuthoredDeclarationSelectionLateBinding,
) -> bool {
    use typed_trees::AuthoredDeclarationSelectionLateBinding as Binding;

    match binding {
        Binding::CheckedCall => matches!(
            kind,
            SymbolKind::BuiltinFunction
                | SymbolKind::Function
                | SymbolKind::Machine
                | SymbolKind::MachineParameter
                | SymbolKind::State
                | SymbolKind::Trait
                | SymbolKind::ConformanceParameter
        ),
        Binding::CheckedMember | Binding::CheckedStructLiteralField => {
            matches!(kind, SymbolKind::Field | SymbolKind::State)
        }
        Binding::CheckedStaticPathSegment => !matches!(
            kind,
            SymbolKind::Root
                | SymbolKind::Local
                | SymbolKind::Parameter
                | SymbolKind::TypeParameter
                | SymbolKind::ConformanceParameter
                | SymbolKind::MachineParameter
                | SymbolKind::PropositionParameter
                | SymbolKind::PropositionMachineParameter
        ),
        Binding::CheckedStructLiteralType => {
            matches!(kind, SymbolKind::BuiltinType | SymbolKind::Data)
        }
        Binding::CheckedStructLiteralCase => matches!(kind, SymbolKind::Variant),
        Binding::CheckedCaseMembership
        | Binding::CheckedDomainMembership
        | Binding::CheckedStaticArgument
        | Binding::CheckedOperator
        | Binding::CheckedConformance => false,
    }
}

fn late_bound_selection_symbol(
    program: &TypedTrees,
    expression: ExpressionHandle,
    prior_occurrences: &[typed_trees::AuthoredDeclarationSelectionOccurrenceId],
    binding: typed_trees::AuthoredDeclarationSelectionLateBinding,
) -> Option<SymbolHandle> {
    use typed_trees::AuthoredDeclarationSelectionLateBinding as Binding;

    let ordinal = prior_occurrences
        .iter()
        .filter(|occurrence| {
            program
                .authored_declaration_selections()
                .get(**occurrence)
                .is_some_and(|selection| {
                    selection.target()
                        == typed_trees::AuthoredDeclarationSelectionTarget::LateBound(binding)
                })
        })
        .count();
    let table = &program.expression_table;
    let selected = match (binding, table.expression(expression)) {
        (Binding::CheckedCall, ExpressionNode::Call(call)) => {
            if call.target_symbol.is_valid() {
                call.target_symbol
            } else {
                typed_trees::operator::resolve_named_expression_call(program, call)
                    .map(|operator| operator.symbol)
                    .unwrap_or_else(SymbolHandle::invalid)
            }
        }
        (Binding::CheckedMember, ExpressionNode::Member(member)) => member.member_symbol,
        (Binding::CheckedStaticPathSegment, ExpressionNode::Name(path)) => table
            .name_path_member_symbols(path.member_symbols)
            .get(ordinal)
            .copied()
            .unwrap_or_else(SymbolHandle::invalid),
        (Binding::CheckedStructLiteralType, ExpressionNode::StructLiteral(literal)) => {
            literal.type_symbol
        }
        (Binding::CheckedStructLiteralCase, ExpressionNode::StructLiteral(literal)) => {
            literal.case_symbol.unwrap_or_else(SymbolHandle::invalid)
        }
        (Binding::CheckedStructLiteralField, ExpressionNode::StructLiteral(literal)) => table
            .struct_fields(literal.fields)
            .get(ordinal)
            .map(|field| field.field_symbol)
            .unwrap_or_else(SymbolHandle::invalid),
        // Operator selection genuinely depends on checked arithmetic facts.
        // Do not guess whether a source operator is intrinsic or overloaded.
        (Binding::CheckedOperator, ExpressionNode::Binary(_) | ExpressionNode::Unary(_)) => {
            SymbolHandle::invalid()
        }
        _ => SymbolHandle::invalid(),
    };
    selected.is_valid().then_some(selected)
}

fn target_machine_symbol(program: &TypedTrees, call: &BuildTimeCallEdge) -> Option<SymbolHandle> {
    if call.target_machine_symbol.is_valid() {
        return Some(call.target_machine_symbol);
    }
    (call.target_state_symbol.is_valid()
        && program.symbols.get(call.target_state_symbol).kind == SymbolKind::Machine)
        .then_some(call.target_state_symbol)
}

fn target_operator_symbol(program: &TypedTrees, call: &BuildTimeCallEdge) -> Option<SymbolHandle> {
    (call.target_operator_symbol.is_valid()
        && typed_trees::operator::declaration_by_symbol(program, call.target_operator_symbol)
            .is_some())
    .then_some(call.target_operator_symbol)
}

#[derive(Debug, Clone, Copy)]
enum PackageCustody {
    Toolchain,
    Package(PackageKeyIdentity),
    UnownedUser,
    Missing,
}

fn package_for_source(program: &TypedTrees, source: SourceSpan) -> PackageCustody {
    match program.symbols.source_file(source) {
        Some(file) if file.origin == SourceOrigin::Toolchain => PackageCustody::Toolchain,
        Some(file) => file
            .package_identity
            .map(PackageCustody::Package)
            .unwrap_or(PackageCustody::UnownedUser),
        None => PackageCustody::Missing,
    }
}

fn package_for_symbol(program: &TypedTrees, symbol: SymbolHandle) -> PackageCustody {
    if let Some(identity) = program.symbols.symbol_package_identity(symbol) {
        return PackageCustody::Package(identity);
    }
    match program.symbols.symbol_source_origin(symbol) {
        Some(SourceOrigin::Toolchain) => PackageCustody::Toolchain,
        Some(SourceOrigin::User) => PackageCustody::UnownedUser,
        None => PackageCustody::Missing,
    }
}

fn require_selection(
    _program: &TypedTrees,
    requester: PackageCustody,
    owner: PackageCustody,
    authority: &dyn BuildTimeSelectionAuthority,
    context: &str,
) -> Option<String> {
    match (requester, owner) {
        (PackageCustody::Toolchain, _) | (_, PackageCustody::Toolchain) => None,
        (PackageCustody::Package(requester), PackageCustody::Package(owner))
            if authority.allows_declaration_selection(requester, owner) =>
        {
            None
        }
        (PackageCustody::Package(requester), PackageCustody::Package(owner)) => Some(format!(
            "{context} selects package {} from package {} without direct dependency authority",
            authority.package_label(owner),
            authority.package_label(requester),
        )),
        (PackageCustody::UnownedUser, _) | (_, PackageCustody::UnownedUser) => Some(format!(
            "{context} has user source without reconciled package custody"
        )),
        (PackageCustody::Missing, _) | (_, PackageCustody::Missing) => Some(format!(
            "{context} lacks compiler-owned source/package provenance"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        BuildTimeSelectionAuthority, ExpressionHandle, ExpressionNode, PackageCustody,
        PackageKeyIdentity, TypedTrees, expression_occurrence_violation,
        late_bound_selection_symbol, unresolved_operator_candidates_are_confined,
    };
    use source_files_to_tokens::Lexer;
    use tokens_to_syntax_trees::parse_syntax_trees;

    fn typed_from_source(source: &str) -> TypedTrees {
        let tokens = Lexer::new(source).tokenize().expect("tokenize");
        let syntax = parse_syntax_trees(&tokens).expect("parse");
        let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
            syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
        )
        .expect("resolve");
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).expect("type")
    }

    fn named_call(program: &TypedTrees, name: &str) -> ExpressionHandle {
        program
            .expression_table
            .iter_expressions()
            .find_map(|(expression, node)| {
                matches!(node, ExpressionNode::Call(call) if call.target.as_str() == name)
                    .then_some(expression)
            })
            .expect("named operator call")
    }

    #[test]
    fn unresolved_named_operator_call_rejoins_its_exact_typed_declaration() {
        let source = r#"
            data Math {}

            operator Math::same(value: u64) -> u64;

            machine selected() -> u64 {
                let result: u64 = Math::same(7);
                transition { _ -> result }
            }
        "#;
        let typed = typed_from_source(source);
        let expression = named_call(&typed, "same");
        let operator = typed
            .operators()
            .iter()
            .find(|operator| {
                typed
                    .operator_path_members(operator.name)
                    .last()
                    .is_some_and(|name| name.as_str() == "same")
            })
            .expect("named operator declaration");
        let operational = validation::infer_operational_may(&typed);
        let selected_call = operational
            .machines()
            .iter()
            .flat_map(|machine| operational.states.span_or_empty(machine.states))
            .flat_map(|state| operational.calls.span_or_empty(state.calls))
            .find(|call| call.target_name == "same")
            .expect("named operator operational call");

        assert_eq!(selected_call.target_operator_symbol, operator.symbol);
        assert!(!selected_call.target_machine_symbol.is_valid());

        assert_eq!(
            late_bound_selection_symbol(
                &typed,
                expression,
                &[],
                typed_trees::AuthoredDeclarationSelectionLateBinding::CheckedCall,
            ),
            Some(operator.symbol)
        );
    }

    #[test]
    fn ambiguous_named_operator_call_does_not_manufacture_early_identity() {
        let typed = typed_from_source(
            r#"
                data Math {}

                operator Math::same(value: i32) -> i32;
                operator Math::same(value: u32) -> u32;

                machine selected(value: i32) -> i32 {
                    let result: i32 = Math::same(value);
                    transition { _ -> result }
                }
            "#,
        );
        let expression = named_call(&typed, "same");
        let operational = validation::infer_operational_may(&typed);
        let selected_call = operational
            .machines()
            .iter()
            .flat_map(|machine| operational.states.span_or_empty(machine.states))
            .flat_map(|state| operational.calls.span_or_empty(state.calls))
            .find(|call| call.target_name == "same")
            .expect("ambiguous named operator operational call");

        assert!(!selected_call.target_operator_symbol.is_valid());
        assert_eq!(
            late_bound_selection_symbol(
                &typed,
                expression,
                &[],
                typed_trees::AuthoredDeclarationSelectionLateBinding::CheckedCall,
            ),
            None
        );
    }

    struct UnconsultedAuthority;

    impl BuildTimeSelectionAuthority for UnconsultedAuthority {
        fn allows_declaration_selection(
            &self,
            _requester: PackageKeyIdentity,
            _owner: PackageKeyIdentity,
        ) -> bool {
            panic!("an empty candidate set cannot consume declaration-selection authority");
        }

        fn package_label(&self, identity: PackageKeyIdentity) -> String {
            format!("package-{identity:?}")
        }
    }

    // A spelled operator whose authored declarations cannot match the actual
    // operands resolves to builtin meaning: the operand-filtered candidate set
    // is empty, so the late-bound `CheckedOperator` occurrence is vacuously
    // confined no matter who requests it and the authority is never consulted.
    #[test]
    fn builtin_only_operator_occurrence_is_vacuously_confined() {
        let typed = typed_from_source(
            r#"
                data Math {}

                boundary operator < Math::less(left: f32, right: f32) -> bool;

                machine probe(value: u64) -> bool {
                    transition { _ -> (value < 256) }
                }
            "#,
        );
        let (expression, _) = typed
            .expression_table
            .iter_expressions()
            .find(|(_, node)| {
                matches!(
                    node,
                    ExpressionNode::Binary(binary)
                        if binary.operator == typed_trees::expression::BinaryOperator::Less
                )
            })
            .expect("builtin integer comparison expression");
        assert!(
            typed_trees_to_checked_trees::typed_operator_authored_selection_candidates(
                &typed, expression,
            )
            .is_empty(),
            "no authored `<` declaration accepts u64 operands"
        );
        assert!(unresolved_operator_candidates_are_confined(
            &typed,
            expression,
            PackageCustody::UnownedUser,
            &UnconsultedAuthority,
        ));
        assert_eq!(
            expression_occurrence_violation(&typed, expression, Some(&UnconsultedAuthority)),
            None,
            "builtin-only occurrence consumes no declaration-selection authority"
        );
    }
}
