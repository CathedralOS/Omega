//! Close an operator occurrence to its ordinary declaration, after operand
//! qualifications have settled. The selected machine retains its full contract;
//! this pass supplies no checking evidence and changes no argument evaluation.

use diagnostics::Diagnostic;
use language_semantics::declaration_selection::{
    AuthoredDeclarationSelectionKind as Kind, AuthoredDeclarationSelectionLateBinding as Late,
    AuthoredDeclarationSelectionTarget as Target,
};
use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionNode, TableCallExpression};
use typed_trees::operator::ClosedOperatorApplicationArgument;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SelectedOperator {
    pub target: symbols::SymbolHandle,
    pub arguments: Vec<ClosedOperatorApplicationArgument>,
}

/// One declaration/type query shared by call preparation and domain-cast result
/// accounting. Unknown nonliteral types never act as wildcard operands.
pub(crate) fn selection(
    program: &TypedTrees,
    types: &mut crate::typing_queries::DeclaredExpressionTypes,
    owner: symbols::SymbolHandle,
    handle: typed_trees::expression::ExpressionHandle,
) -> Result<Option<SelectedOperator>, Diagnostic> {
    let ExpressionNode::Binary(binary) = program.expression_table.expression(handle) else {
        return Ok(None);
    };
    if binary.policy != numerics::arithmetic::ArithmeticDomain::Exact {
        return Ok(None);
    }
    let Some(spelling) = typed_trees::operator::binary_operator_spelling(binary.operator) else {
        return Ok(None);
    };
    let mut operands = Vec::with_capacity(2);
    for operand in [binary.left, binary.right] {
        let reference = types.get(program, owner, operand);
        if reference.is_none()
            && !typed_trees::closed_numeric::has_anonymous_numeric_results(program, operand)
        {
            return Ok(None);
        }
        operands.push(reference);
    }
    let span = program.expression_table.source_span(handle);
    selection_for_operands(program, spelling, &operands, span)
}

pub(crate) fn selection_for_operands(
    program: &TypedTrees,
    spelling: language_core::operator_spelling::OperatorSpelling,
    operands: &[Option<typed_trees::types::TypeReferenceHandle>],
    span: source::SourceSpan,
) -> Result<Option<SelectedOperator>, Diagnostic> {
    // An anonymous arithmetic tree has no declared home. Like the existing
    // builtin-meaning query, keep it builtin rather than letting every family
    // match a tuple of wildcard operands.
    if operands.iter().all(Option::is_none) {
        return Ok(None);
    }
    let candidates =
        typed_trees::operator::resolve_spelling_for_operands(program, spelling, operands, span);
    let selected = match candidates.as_slice() {
        [] => return Ok(None),
        [selected] => selected,
        _ => {
            let declarations = candidates
                .iter()
                .map(|candidate| {
                    let parameters = program
                        .operator_parameters(candidate.operator)
                        .iter()
                        .map(|parameter| {
                            program
                                .display_type_reference_with_constraints(parameter.type_reference)
                        })
                        .collect::<Vec<_>>()
                        .join(", ");
                    format!(
                        "{}({parameters})",
                        program
                            .symbols
                            .display_path(candidate.operator.symbol, "::")
                    )
                })
                .collect::<Vec<_>>()
                .join("; ");
            let operands = operands
                .iter()
                .map(|reference| {
                    reference.map_or_else(
                        || "anonymous".to_owned(),
                        |reference| program.display_type_reference_with_constraints(reference),
                    )
                })
                .collect::<Vec<_>>()
                .join(", ");
            return Err(Diagnostic::error(format!(
                "ambiguous operator declarations for operands ({operands}): {declarations}"
            ))
            .with_source_span(span));
        }
    };
    // A body is selected with its complete application, not by discarding its
    // binders. Explicit evidence and open applications remain unavailable here.
    let Some(machine) = program
        .machine_by_symbol(selected.operator.symbol)
        .filter(|machine| {
            machine.supply_mode == language_semantics::MachineSupplyMode::CheckedBody
                && machine.conformance_bounds.is_empty()
        })
    else {
        return Ok(None);
    };
    let Some(arguments) = typed_trees::operator::closed_operator_application_for_operands(
        program,
        selected.operator,
        operands,
    ) else {
        return Ok(None);
    };
    Ok(Some(SelectedOperator {
        target: machine.symbol,
        arguments,
    }))
}

pub(crate) fn sites(
    program: &TypedTrees,
    frontier: usize,
) -> Vec<(
    typed_trees::expression::ExpressionHandle,
    typed_trees::expression::TableBinaryExpression,
)> {
    program
        .expression_table
        .iter_expressions()
        .filter_map(|(handle, node)| match node {
            ExpressionNode::Binary(binary)
                if handle.arena_index() as usize > frontier
                    && binary.policy == numerics::arithmetic::ArithmeticDomain::Exact =>
            {
                typed_trees::operator::binary_operator_spelling(binary.operator)
                    .map(|_| (handle, *binary))
            }
            _ => None,
        })
        .collect()
}

/// A builtin operator takes unqualified operands only, in executable code and
/// in `requires` and `ensures` alike (domains.md, "Operands"). When typing
/// knows every operand's type, one of them carries a declared domain, and no
/// operator declaration accepts those types, the program breaks that rule:
/// report it and show the cast that removes the domain. An operand whose type
/// typing cannot read here, or a declaration that matches but cannot be
/// selected yet, is not decided here; the lowered-form reader refuses those.
fn qualified_builtin_operand(
    program: &TypedTrees,
    types: &mut crate::typing_queries::DeclaredExpressionTypes,
    owner: symbols::SymbolHandle,
    handle: typed_trees::expression::ExpressionHandle,
    binary: typed_trees::expression::TableBinaryExpression,
) -> Option<Diagnostic> {
    let spelling = typed_trees::operator::binary_operator_spelling(binary.operator)?;
    let mut operands = [None; 2];
    let mut qualified = None;
    for (position, operand) in [binary.left, binary.right].into_iter().enumerate() {
        let reference = types.get(program, owner, operand);
        if reference.is_none()
            && !typed_trees::closed_numeric::has_anonymous_numeric_results(program, operand)
        {
            return None;
        }
        if qualified.is_none()
            && let Some(reference) = reference
            && let Some(written) = declared_domains(program, reference)
        {
            qualified = Some((operand, written));
        }
        operands[position] = reference;
    }
    let (operand, (carrier, domains)) = qualified?;
    let span = program.expression_table.source_span(handle);
    if !typed_trees::operator::resolve_spelling_for_operands(program, spelling, &operands, span)
        .is_empty()
    {
        return None;
    }
    let operator = binary.operator.display_name();
    let value = program.expression_table.display_name(operand);
    // A compound operand keeps its own grouping inside the suggested cast.
    let cast_operand = if value.contains(' ') {
        format!("({value})")
    } else {
        value.clone()
    };
    let removal = domains
        .iter()
        .map(|domain| format!("-{domain}"))
        .collect::<Vec<_>>()
        .join(" ");
    Some(
        Diagnostic::error(format!(
            "builtin operator `{operator}` takes unqualified operands only, but `{value}` has \
             type `{carrier} in {}` and no operator `{operator}` is declared for it; remove the \
             domain with a cast first, such as `({cast_operand} as {removal})`",
            domains.join(" & "),
        ))
        .with_source_span(span),
    )
}

/// The carrier and the declared domains a type reference carries directly,
/// by name, or `None` when it carries none. An arithmetic policy is not a
/// declared domain: core declares the policy operators.
fn declared_domains(
    program: &TypedTrees,
    reference: typed_trees::types::TypeReferenceHandle,
) -> Option<(String, Vec<String>)> {
    use typed_trees::types::{DomainConstraintSubject, TypeConstraintNode, TypeReferenceNode};
    let table = &program.type_reference_table;
    let TypeReferenceNode::Constrained {
        base_type,
        constraints,
    } = table.type_reference(reference)
    else {
        return None;
    };
    let names = table
        .constraint_span(*constraints)?
        .iter()
        .filter_map(|constraint| match constraint {
            TypeConstraintNode::Domain(domain)
                if domain.subject == DomainConstraintSubject::Declared
                    && domain.symbol.is_valid() =>
            {
                Some(domain.name.as_str().to_owned())
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    (!names.is_empty()).then(|| (table.display_name(*base_type), names))
}

pub(crate) fn prepare_site(
    program: &mut TypedTrees,
    owners: &typed_trees::references::ProgramOperandOwners,
    types: &mut crate::typing_queries::DeclaredExpressionTypes,
    handle: typed_trees::expression::ExpressionHandle,
    binary: typed_trees::expression::TableBinaryExpression,
) -> Result<(), Diagnostic> {
    let span = program.expression_table.source_span(handle);
    let mut answer = None;
    let mut caller = symbols::SymbolHandle::invalid();
    for owner in owners.expression_owners(handle) {
        let selected = selection(program, types, owner, handle)?;
        if answer
            .as_ref()
            .is_some_and(|previous| previous != &selected)
        {
            return Err(Diagnostic::error(
                "operator occurrence has conflicting declaration contexts",
            )
            .with_source_span(span));
        }
        if !caller.is_valid() {
            caller = owner;
        }
        answer = Some(selected);
    }
    let Some(Some(selected)) = answer else {
        if answer.is_some()
            && let Some(error) = qualified_builtin_operand(program, types, caller, handle, binary)
        {
            return Err(error);
        }
        return Ok(());
    };
    let target = selected.target;
    let Some(machine) = program.machine_by_symbol(target) else {
        return Err(
            Diagnostic::error("selected operator lost its declaration").with_source_span(span)
        );
    };
    let label = machine.name.clone();
    let result_type = program
        .machine_states(machine)
        .first()
        .map(|state| state.return_type);
    let selections = program
        .expression_table
        .authored_selection_occurrences(handle)
        .filter(|occurrence| {
            program
                .authored_declaration_selections()
                .get(*occurrence)
                .is_some_and(|selection| {
                    selection.kind() == Kind::Operator && selection.source_span() == span
                })
        })
        .collect::<Vec<_>>();
    let [occurrence] = selections.as_slice() else {
        return Err(
            Diagnostic::error("operator call lost its unique authored selection")
                .with_source_span(span),
        );
    };
    let selection = program
        .authored_declaration_selections()
        .get(*occurrence)
        .unwrap();
    match selection.target() {
        Target::LateBound(Late::CheckedOperator) => {
            program
                .finalize_late_bound_authored_declaration_selection(
                    *occurrence,
                    Late::CheckedOperator,
                    target,
                )
                .map_err(|error| {
                    Diagnostic::error(format!("operator choice cannot be retained: {error:?}"))
                        .with_source_span(span)
                })?;
        }
        Target::Resolved(selected) if selected.selected_symbol() == target => {}
        _ => {
            return Err(
                Diagnostic::error("operator choice contradicts its authored selection")
                    .with_source_span(span),
            );
        }
    }
    let arguments = program
        .expression_table
        .insert_expression_handles([binary.left, binary.right]);
    let (machine_arguments, application, result_type) =
        retain_application(program, &selected, caller, &label, result_type, span)?;
    *program.expression_table.expression_mut(handle) = ExpressionNode::Call(TableCallExpression {
        authored_call_selection: Some(*occurrence),
        authored_call_binding: None,
        authored_machine_application: application,
        build_selection: Default::default(),
        exact_coercion_target: Default::default(),
        requirement_application: Default::default(),
        receiver: Default::default(),
        target_symbol: target,
        static_machine_parameter: Default::default(),
        target: label,
        static_requirement_dispatch: None,
        machine_arguments: machine_arguments.into(),
        quotient_operation: None,
        private_layout_operation: None,
        installed_interrupt_operation: None,
        layout_query: None,
        native_code_layout: None,
        arguments,
        evidence_arguments: Box::default(),
        operational_acknowledgement: Default::default(),
    });
    if let Some(result_type) = result_type {
        program
            .expression_table
            .set_constructed_value_type_reference(handle, result_type);
    }

    Ok(())
}
/// Retain the same static argument/binding packet as an ordinary named call.
/// Inferred constants use their canonical value, never a re-read source token.
fn retain_application(
    program: &mut TypedTrees,
    selected: &SelectedOperator,
    caller: symbols::SymbolHandle,
    label: &typed_trees::name::Identifier,
    result_type: Option<typed_trees::types::TypeReferenceHandle>,
    span: source::SourceSpan,
) -> Result<
    (
        Vec<typed_trees::expression::StaticMachineArgument>,
        typed_trees::static_applications::StaticMachineApplicationHandle,
        Option<typed_trees::types::TypeReferenceHandle>,
    ),
    Diagnostic,
> {
    use typed_trees::expression::StaticMachineArgument;
    use typed_trees::static_applications::{StaticMachineBinding, StaticMachineBindingValue};
    if selected.arguments.is_empty() {
        return Ok((Vec::new(), Default::default(), None));
    }
    let failure = |reason| Diagnostic::error(reason).with_source_span(span);
    let mut arguments = Vec::with_capacity(selected.arguments.len());
    let mut bindings = Vec::with_capacity(selected.arguments.len());
    let mut substitutions = Vec::with_capacity(selected.arguments.len());
    for argument in &selected.arguments {
        let (parameter, reference, binding) = match argument {
            ClosedOperatorApplicationArgument::Type {
                binder_symbol,
                type_reference,
            } => (
                *binder_symbol,
                *type_reference,
                StaticMachineBindingValue::Type(*type_reference),
            ),
            ClosedOperatorApplicationArgument::Const {
                binder_symbol,
                value,
                ..
            } => {
                value
                    .decode_encoding()
                    .ok_or_else(|| failure("operator application lost its canonical constant"))?;
                let value = language_semantics::const_value::CanonicalConstValue::new(
                    value.type_name.clone(),
                    value.encoding.clone(),
                    "",
                );
                let reference = program.type_reference_table.insert(
                    typed_trees::types::TypeReferenceNode::Named {
                        symbol: symbols::SymbolHandle::invalid(),
                        name: typed_trees::name::Identifier::generated(value.atom()),
                    },
                );
                (
                    *binder_symbol,
                    reference,
                    StaticMachineBindingValue::Const(reference),
                )
            }
        };
        arguments.push(StaticMachineArgument {
            type_reference: reference,
            ..Default::default()
        });
        bindings.push(StaticMachineBinding {
            parameter,
            value: binding,
        });
        substitutions.push((parameter, reference));
    }
    let result_type = result_type
        .filter(|reference| reference.is_valid())
        .map(|reference| {
            program
                .apply_callable_type(reference, &substitutions, &[], selected.target)
                .map_err(|reason| {
                    Diagnostic::error(format!("operator result application: {reason}"))
                        .with_source_span(span)
                })
        })
        .transpose()?;
    let application = program.expression_table.retain_static_machine_application(
        selected.target,
        label.clone(),
        &arguments,
    );
    let row = program
        .expression_table
        .static_machine_application_mut(application);
    row.lexical_owner = caller;
    row.binding_declaration = selected.target;
    program
        .expression_table
        .set_static_machine_application_bindings(application, &bindings);
    Ok((arguments, application, result_type))
}
