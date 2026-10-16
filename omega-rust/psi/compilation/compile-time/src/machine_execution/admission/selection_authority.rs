use language_semantics::declaration_selection::BuildOperation;
use semantic_vocabulary::PackageKeyIdentity;
use source::{SourceOrigin, SourceSpan};
use symbols::{SymbolHandle, SymbolKind};
use typed_trees::{
    TypedTrees,
    expression::{ExpressionHandle, ExpressionNode},
    machine::Machine,
    state::State,
    statement::{StatementNode, TransitionGuardNode, TransitionTargetNode},
};

use super::{BuildTimeCallEdge, BuildTimeInvocationCustody, BuildTimeSelectionAuthority};

/// One declaration-selection rejection. An authored occurrence carries its
/// own source span so the reporting owner can point at the selection rather
/// than at the invocation that reached it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SelectionViolation {
    pub(crate) message: String,
    pub(crate) source_span: Option<SourceSpan>,
}

impl SelectionViolation {
    fn unlocated(message: String) -> Self {
        Self {
            message,
            source_span: None,
        }
    }
}

impl From<SelectionViolation> for String {
    fn from(violation: SelectionViolation) -> Self {
        violation.message
    }
}

pub(super) fn selection_authority_violation(
    call_edges: &[BuildTimeCallEdge],
    program: &TypedTrees,
    root: &Machine,
    custody: Option<BuildTimeInvocationCustody>,
    authority: Option<&dyn BuildTimeSelectionAuthority>,
    selected: &[crate::SelectedBuildTimeBinaryOperator],
) -> Option<SelectionViolation> {
    if let Some(authority) = authority {
        let Some(custody) = custody else {
            return Some(SelectionViolation::unlocated(
                "package-aware build-time evaluation has no authored invocation custody".to_owned(),
            ));
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
            return Some(SelectionViolation::unlocated(violation));
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
                        return Some(SelectionViolation::unlocated(violation));
                    }
                    continue;
                }
                return Some(SelectionViolation::unlocated(format!(
                    "build-time call from `{}` has no exact target-machine identity",
                    program.symbols.display_path(source_machine, "::")
                )));
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
                return Some(SelectionViolation::unlocated(violation));
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
) -> Option<SelectionViolation> {
    let machine = program.machine(machine_symbol)?;
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
        StatementNode::Assembly(_) => {}
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
        TransitionTargetNode::Terminal => {}
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
) -> Option<SelectionViolation> {
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
        return Some(SelectionViolation {
            message: "build-time binary operator requires exact authored selection before evaluation; the evaluator cannot execute it as a builtin operator".to_owned(),
            source_span: Some(program.expression_table.source_span(expression)),
        });
    }

    if let Some(violation) = expression_occurrence_violation(program, expression, authority) {
        return Some(violation);
    }

    let children = expression_children(program, expression);
    for child in children {
        if let Some(violation) = expression_selection_violation(
            program, machine, state, child, authority, selected, visited,
        ) {
            return Some(violation);
        }
    }
    None
}

pub(crate) fn expression_children(
    program: &TypedTrees,
    expression: ExpressionHandle,
) -> Vec<ExpressionHandle> {
    let mut children = Vec::new();
    ::validation::expression_traversal::children(
        program,
        program.expression_table.expression(expression),
        |child| {
            if child.is_valid() {
                children.push(child);
            }
        },
    );
    children
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
    let Some(call) = program.expression_table.call_node(expression) else {
        return Err("range endpoint lost its original call selection".to_owned());
    };
    if let Some(violation) = expression_occurrence_violation(program, expression, authority) {
        return Err(violation.message);
    }
    if call.receiver.is_valid()
        && let Some(violation) = expression_occurrence_violation(program, call.receiver, authority)
    {
        return Err(violation.message);
    }
    Ok(())
}

pub(crate) fn require_closed_integer_argument(
    program: &TypedTrees,
    evaluated: &TypedTrees,
    expression: ExpressionHandle,
    authority: Option<&dyn BuildTimeSelectionAuthority>,
) -> Result<(), String> {
    require_closed_expression_custody(program, expression, authority)?;
    evaluated.closed_integer_value_in(expression, SymbolHandle::invalid())
        .map(|_| ())
        .ok_or_else(|| "range endpoint argument requires a closed integer expression with context-independent operator meaning".to_owned())
}

/// Visit every authored selection, including unexecuted branches, before the
/// shared scalar evaluator validates shapes and executes demanded expressions.
pub(crate) fn require_closed_expression_custody(
    program: &TypedTrees,
    expression: ExpressionHandle,
    authority: Option<&dyn BuildTimeSelectionAuthority>,
) -> Result<(), String> {
    let mut pending = vec![expression];
    let mut visited = Vec::new();
    while let Some(expression) = pending.pop() {
        if !program.expression_table.expression_is_valid(expression) {
            return Err("invalid constant selection expression".into());
        }
        if visited.contains(&expression) {
            continue;
        }
        visited.push(expression);
        if let Some(violation) = expression_occurrence_violation(program, expression, authority) {
            return Err(violation.message);
        }
        pending.extend(expression_children(program, expression).into_iter().rev());
    }
    Ok(())
}

fn expression_occurrence_violation(
    program: &TypedTrees,
    expression: ExpressionHandle,
    authority: Option<&dyn BuildTimeSelectionAuthority>,
) -> Option<SelectionViolation> {
    if let Some(authority) = authority {
        let occurrences = program
            .expression_table
            .authored_selection_occurrences(expression)
            .collect::<Vec<_>>();
        for (occurrence_offset, occurrence) in occurrences.iter().copied().enumerate() {
            let Some(selection) = program.authored_declaration_selections().get(occurrence) else {
                return Some(SelectionViolation {
                    message: format!(
                        "build-time expression retains unknown authored declaration selection occurrence {}",
                        occurrence.ordinal()
                    ),
                    source_span: Some(program.expression_table.source_span(expression)),
                });
            };
            let located = |message: String| SelectionViolation {
                message,
                source_span: Some(selection.source_span()),
            };
            // Product references are operands of one exact Build operation,
            // not host generic arguments. Keep every authored occurrence, but
            // establish its lexical product authority without consulting the
            // caller's Build-import namespace or executing the declaration.
            if validation::compiler_operations::is_build_declaration_selection(program, expression)
            {
                use typed_trees::AuthoredDeclarationSelectionKind as Kind;
                if selection.kind() == Kind::Call {
                    continue;
                }
                if selection.kind() == Kind::StaticArgument {
                    let argument_index = occurrences[..occurrence_offset]
                        .iter()
                        .filter(|occurrence| {
                            program
                                .authored_declaration_selections()
                                .get(**occurrence)
                                .is_some_and(|earlier| earlier.kind() == Kind::StaticArgument)
                        })
                        .count();
                    let selected =
                        validation::compiler_operations::resolve_build_declaration_operand(
                            program,
                            expression,
                            argument_index,
                            selection.source_span(),
                        );
                    if selected.is_none() {
                        return Some(located("Build declaration operand does not resolve to one visible product declaration in this occurrence's package scope".to_owned()));
                    }
                    if let Some(retained) = selection.target().as_resolved()
                        && Some(retained.selected_symbol()) != selected
                    {
                        return Some(located("Build declaration occurrence does not retain its exact product declaration".to_owned()));
                    }
                    continue;
                }
            }
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
                                // No exact owner is derivable for this
                                // occurrence, so its authority can only be
                                // established by spelling: every declaration
                                // the spelling could select must already be
                                // admitted for the requesting package.
                                match unresolved_spelling_confinement(
                                    program,
                                    program.symbols.source_text(selection.source_span()),
                                    requester,
                                    authority,
                                    binding,
                                ) {
                                    Ok(()) => continue,
                                    Err(unconfined) => {
                                        return Some(located(format!(
                                            "build-time expression has unresolved authored {:?} selection `{}` ({binding:?}) with no derivable exact owner, so its package authority is confined by spelling across the program and {unconfined}; package authority must be known before compiler execution",
                                            selection.kind(),
                                            program.symbols.source_text(selection.source_span()),
                                        )));
                                    }
                                }
                            }
                        }
                    }
                }
                typed_trees::AuthoredDeclarationSelectionTarget::Resolved(selected) => {
                    // Builtin type slots are compiler-owned identities, not
                    // declarations with a fictional source/package owner.
                    // Same-spelled or generated symbols do not classify here.
                    if program
                        .symbols
                        .builtin_type_atom(selected.selected_symbol())
                        .is_some()
                    {
                        continue;
                    }
                    package_for_symbol(program, selected.selected_symbol())
                }
            };
            let context = format!("build-time authored {:?} selection", selection.kind());
            if let Some(violation) =
                require_selection(program, requester, owner, authority, &context)
            {
                return Some(located(violation));
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

/// Spelling-wide confinement for a late-bound occurrence with no derivable
/// exact owner: `Ok` when every declaration the spelling could select is
/// already admitted for the requester, otherwise the first unadmitted
/// candidate's rejection. This is the fallback route only; an occurrence
/// whose owner type is known is confined on that declaration alone.
fn unresolved_spelling_confinement(
    program: &TypedTrees,
    spelling: &str,
    requester: PackageCustody,
    authority: &dyn BuildTimeSelectionAuthority,
    binding: typed_trees::AuthoredDeclarationSelectionLateBinding,
) -> Result<(), String> {
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
        return Err(format!(
            "a {binding:?} occurrence has no spelling-confinement route"
        ));
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
    if candidates.is_empty() {
        return Err(format!(
            "no declaration spelled `{spelling}` exists in the program"
        ));
    }
    for candidate in candidates {
        if let Some(violation) = require_selection(
            program,
            requester,
            package_for_symbol(program, candidate),
            authority,
            "candidate for an unresolved build-time declaration selection",
        ) {
            return Err(format!(
                "`{}` is not: {violation}",
                program.symbols.display_path(candidate, "::")
            ));
        }
    }
    Ok(())
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
        (Binding::CheckedStaticArgument, ExpressionNode::Call(call))
            if BuildOperation::from_call_target(call.target.as_str())
                == Some(BuildOperation::ProviderSelection) =>
        {
            call.machine_arguments
                .get(ordinal)
                .map(|argument| argument.symbol)
                .unwrap_or_default()
        }
        (Binding::CheckedCall, ExpressionNode::Call(call)) => {
            if call.target_symbol.is_valid() {
                call.target_symbol
            } else {
                typed_trees::operator::resolve_named_expression_call(program, call)
                    .map(|operator| operator.symbol)
                    .unwrap_or_else(SymbolHandle::invalid)
            }
        }
        // Destructure-pattern payload projections keep an invalid typed
        // member symbol; derive the declaration from the receiver's exact
        // owner type the way checked binding does before consulting spelling.
        (Binding::CheckedMember, ExpressionNode::Member(member)) => {
            if member.member_symbol.is_valid() {
                member.member_symbol
            } else {
                typed_trees_to_checked_trees::late_bound_member_declaration_from_exact_owner(
                    program, expression,
                )
                .unwrap_or_else(SymbolHandle::invalid)
            }
        }
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
