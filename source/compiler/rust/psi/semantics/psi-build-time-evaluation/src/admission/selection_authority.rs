use psi_core::PackageKeyIdentity;
use psi_source::{SourceOrigin, SourceSpan};
use psi_symbols::{SymbolHandle, SymbolKind};
use psi_typed_trees::{
    TypedTrees,
    expression::{ExpressionHandle, ExpressionNode},
    machine::Machine,
    statement::{StatementNode, TransitionGuardNode, TransitionTargetNode},
};

use super::{BuildTimeCallEdge, BuildTimeInvocationCustody, BuildTimeSelectionAuthority};

pub(super) fn selection_authority_violation(
    call_edges: &[BuildTimeCallEdge],
    program: &TypedTrees,
    root: &Machine,
    custody: Option<BuildTimeInvocationCustody>,
    authority: &dyn BuildTimeSelectionAuthority,
) -> Option<String> {
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

    let mut completed = Vec::new();
    let mut pending = vec![root.symbol];
    while let Some(source_machine) = pending.pop() {
        if completed.contains(&source_machine) {
            continue;
        }
        completed.push(source_machine);
        if let Some(violation) = machine_selection_violation(program, source_machine, authority) {
            return Some(violation);
        }
        for call in call_edges
            .iter()
            .filter(|call| call.source_machine_symbol == source_machine)
        {
            let Some(target_machine) = target_machine_symbol(program, call) else {
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
            if let Some(violation) = require_selection(
                program,
                package_for_symbol(program, source_machine),
                package_for_symbol(program, target_machine),
                authority,
                &context,
            ) {
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
    authority: &dyn BuildTimeSelectionAuthority,
) -> Option<String> {
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == machine_symbol)?;
    let mut expressions = Vec::new();
    for state in program.machine_states(machine) {
        for statement in program.statement_table.statements(state.statement_nodes) {
            collect_statement_expressions(program, statement, &mut expressions);
        }
    }

    let mut visited = Vec::new();
    for expression in expressions {
        if let Some(violation) =
            expression_selection_violation(program, expression, authority, &mut visited)
        {
            return Some(violation);
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
    target: psi_typed_trees::statement::TransitionTargetHandle,
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
    expression: ExpressionHandle,
    authority: &dyn BuildTimeSelectionAuthority,
    visited: &mut Vec<ExpressionHandle>,
) -> Option<String> {
    if !expression.is_valid() || visited.contains(&expression) {
        return None;
    }
    visited.push(expression);

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
            psi_typed_trees::AuthoredDeclarationSelectionTarget::Intrinsic(_) => continue,
            psi_typed_trees::AuthoredDeclarationSelectionTarget::LateBound(binding) => {
                if binding
                    == psi_typed_trees::AuthoredDeclarationSelectionLateBinding::CheckedOperator
                    && psi_typed_trees_to_checked_trees::typed_operator_has_no_authored_selection(
                        program, expression,
                    )
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
            psi_typed_trees::AuthoredDeclarationSelectionTarget::Resolved(selected) => {
                package_for_symbol(program, selected.selected_symbol())
            }
        };
        let context = format!("build-time authored {:?} selection", selection.kind());
        if let Some(violation) = require_selection(program, requester, owner, authority, &context) {
            return Some(violation);
        }
    }

    let table = &program.expression_table;
    let mut children = Vec::new();
    match table.expression(expression) {
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
        if let Some(violation) = expression_selection_violation(program, child, authority, visited)
        {
            return Some(violation);
        }
    }
    None
}

fn unresolved_spelling_is_confined(
    program: &TypedTrees,
    spelling: &str,
    requester: PackageCustody,
    authority: &dyn BuildTimeSelectionAuthority,
    binding: psi_typed_trees::AuthoredDeclarationSelectionLateBinding,
) -> bool {
    use psi_typed_trees::AuthoredDeclarationSelectionLateBinding as Binding;

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
    binding: psi_typed_trees::AuthoredDeclarationSelectionLateBinding,
) -> bool {
    use psi_typed_trees::AuthoredDeclarationSelectionLateBinding as Binding;

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
    prior_occurrences: &[psi_typed_trees::AuthoredDeclarationSelectionOccurrenceId],
    binding: psi_typed_trees::AuthoredDeclarationSelectionLateBinding,
) -> Option<SymbolHandle> {
    use psi_typed_trees::AuthoredDeclarationSelectionLateBinding as Binding;

    let ordinal = prior_occurrences
        .iter()
        .filter(|occurrence| {
            program
                .authored_declaration_selections()
                .get(**occurrence)
                .is_some_and(|selection| {
                    selection.target()
                        == psi_typed_trees::AuthoredDeclarationSelectionTarget::LateBound(binding)
                })
        })
        .count();
    let table = &program.expression_table;
    let selected = match (binding, table.expression(expression)) {
        (Binding::CheckedCall, ExpressionNode::Call(call)) => call.target_symbol,
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
