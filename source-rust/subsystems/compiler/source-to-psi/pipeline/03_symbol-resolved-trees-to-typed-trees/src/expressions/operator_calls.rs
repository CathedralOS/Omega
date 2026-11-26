//! Close an operator occurrence to its ordinary declaration, after operand
//! qualifications have settled. The selected machine retains its full contract;
//! this pass supplies no checking evidence and changes no argument evaluation.

use crate::declaration_selection::AuthoredDeclarationSelectionKind as Kind;
use crate::declaration_selection::AuthoredDeclarationSelectionLateBinding as Late;
use crate::declaration_selection::AuthoredDeclarationSelectionTarget as Target;
use crate::source_state::TypedTrees;
use crate::source_state::expression::{ExpressionNode, TableCallExpression};
use crate::source_state::operator::ClosedOperatorApplicationArgument;
use diagnostics::Diagnostic;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SelectedOperator {
    pub target: symbols::SymbolHandle,
    pub arguments: Vec<ClosedOperatorApplicationArgument>,
    collection_borrow: Option<language_core::ReferenceAccess>,
}

/// One way an occurrence can apply an operator declaration: the fixed token
/// and the operand expressions the declaration receives, in parameter order
/// (expressions.md, "Operator families"). `indexed` marks indexing, whose
/// collection operand matches under the collection-view rules.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct OperatorForm {
    pub spelling: language_core::operator_spelling::OperatorSpelling,
    operands: [crate::source_state::expression::ExpressionHandle; 3],
    count: usize,
    pub indexed: bool,
}

impl OperatorForm {
    fn new(
        spelling: language_core::operator_spelling::OperatorSpelling,
        written: &[crate::source_state::expression::ExpressionHandle],
        indexed: bool,
    ) -> Self {
        let mut operands = [crate::source_state::expression::ExpressionHandle::invalid(); 3];
        operands[..written.len()].copy_from_slice(written);
        Self {
            spelling,
            operands,
            count: written.len(),
            indexed,
        }
    }

    pub(crate) fn operands(&self) -> &[crate::source_state::expression::ExpressionHandle] {
        &self.operands[..self.count]
    }
}

/// The forms an occurrence may apply, the more specific first. Prefix
/// negation `-x` applies a one-operand `-` before its `0 - x` meaning;
/// `c[i]` applies `[]` to the collection and index, and `c[a..b]` applies
/// `[..]` to the collection and both written endpoints. Suffixed policy
/// operators bind no declaration, and a range with an omitted or inclusive
/// end has no operand tuple a declaration could receive.
pub(crate) fn forms(
    program: &TypedTrees,
    handle: crate::source_state::expression::ExpressionHandle,
) -> [Option<OperatorForm>; 2] {
    use language_core::operator_spelling::OperatorSpelling;
    match program.expression_table.expression(handle) {
        ExpressionNode::Binary(binary)
            if binary.policy == numerics::arithmetic::ArithmeticDomain::Exact =>
        {
            let Some(spelling) =
                crate::source_state::operator::binary_operator_spelling(binary.operator)
            else {
                return [None, None];
            };
            let both = OperatorForm::new(spelling, &[binary.left, binary.right], false);
            if binary.negation {
                [
                    Some(OperatorForm::new(spelling, &[binary.right], false)),
                    Some(both),
                ]
            } else {
                [Some(both), None]
            }
        }
        ExpressionNode::Indexed(indexed) => {
            let table = &program.expression_table;
            let form = match table.range_node(indexed.index) {
                None => OperatorForm::new(
                    OperatorSpelling::Index,
                    &[indexed.collection, indexed.index],
                    true,
                ),
                Some(range)
                    if !range.end_inclusive
                        && table.expression_is_valid(range.start)
                        && table.expression_is_valid(range.end) =>
                {
                    OperatorForm::new(
                        OperatorSpelling::Range,
                        &[indexed.collection, range.start, range.end],
                        true,
                    )
                }
                Some(_) => return [None, None],
            };
            [Some(form), None]
        }
        _ => [None, None],
    }
}

/// One declaration/type query shared by call preparation and domain-cast result
/// accounting. Unknown nonliteral types never act as wildcard operands. The
/// first form with a matching declaration is the occurrence's selection.
pub(crate) fn selection(
    program: &TypedTrees,
    types: &mut crate::types::typing_queries::DeclaredExpressionTypes,
    owner: symbols::SymbolHandle,
    handle: crate::source_state::expression::ExpressionHandle,
) -> Result<Option<(SelectedOperator, OperatorForm)>, Diagnostic> {
    let span = program.expression_table.source_span(handle);
    'forms: for form in forms(program, handle).into_iter().flatten() {
        let mut operands = [None; 3];
        for (slot, operand) in operands.iter_mut().zip(form.operands()) {
            let reference = types.get(program, owner, *operand);
            if reference.is_none()
                && !crate::source_state::closed_numeric::has_anonymous_numeric_results(
                    program, *operand,
                )
            {
                continue 'forms;
            }
            *slot = reference;
        }
        if let Some(selected) =
            selection_for_operands(program, form, &operands[..form.count], span)?
        {
            return Ok(Some((selected, form)));
        }
    }
    Ok(None)
}

pub(crate) fn selection_for_operands(
    program: &TypedTrees,
    form: OperatorForm,
    operands: &[Option<crate::source_state::types::TypeReferenceHandle>],
    span: source_text::SourceSpan,
) -> Result<Option<SelectedOperator>, Diagnostic> {
    let spelling = form.spelling;
    // An anonymous arithmetic tree has no declared home. Like the existing
    // builtin-meaning query, keep it builtin rather than letting every family
    // match a tuple of wildcard operands.
    if operands.iter().all(Option::is_none) {
        return Ok(None);
    }
    let candidates = if form.indexed {
        crate::source_state::operator::resolve_indexed_spelling_for_operands(
            program, spelling, operands, span,
        )
    } else {
        crate::source_state::operator::resolve_spelling_for_operands(
            program, spelling, operands, span,
        )
    };
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
            matches!(
                machine.supply_mode,
                source_declarations::MachineSupplyMode::CheckedBody
                    | source_declarations::MachineSupplyMode::TopLevelRequirement
            ) && machine.conformance_bounds.is_empty()
        })
    else {
        return Ok(None);
    };
    let Some(arguments) =
        crate::source_state::operator::operator_call_arguments(program, selected.operator, operands)
    else {
        return Ok(None);
    };
    let collection_borrow = if form.indexed {
        collection_borrow(program, selected.operator, operands)
    } else {
        None
    };
    if collection_borrow == Some(language_core::ReferenceAccess::WriteOnly) {
        return Ok(None);
    }
    Ok(Some(SelectedOperator {
        target: machine.symbol,
        arguments,
        collection_borrow,
    }))
}

/// Index and range syntax lend an owned collection when operand zero
/// of the selected declaration requires a reference.
fn collection_borrow(
    program: &TypedTrees,
    operator: &crate::source_state::operator::OperatorDefinition,
    operands: &[Option<crate::source_state::types::TypeReferenceHandle>],
) -> Option<language_core::ReferenceAccess> {
    use crate::source_state::types::TypeReferenceNode;
    let table = &program.type_reference_table;
    let reference_access = |reference| {
        let reference =
            crate::source_state::operator::unconstrained_type_reference(program, reference)?;
        match table.type_reference(reference) {
            TypeReferenceNode::Reference { access, .. } => Some(*access),
            _ => None,
        }
    };
    let written = operands.first().copied().flatten()?;
    let parameter = program.operator_parameters(operator).first()?;
    if reference_access(written).is_some() {
        return None;
    }
    reference_access(parameter.type_reference)
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
    types: &mut crate::types::typing_queries::DeclaredExpressionTypes,
    owner: symbols::SymbolHandle,
    handle: crate::source_state::expression::ExpressionHandle,
    binary: crate::source_state::expression::TableBinaryExpression,
) -> Option<Diagnostic> {
    let spelling = crate::source_state::operator::binary_operator_spelling(binary.operator)?;
    let mut operands = [None; 2];
    let mut qualified = None;
    for (position, operand) in [binary.left, binary.right].into_iter().enumerate() {
        let reference = types.get(program, owner, operand);
        if reference.is_none()
            && !crate::source_state::closed_numeric::has_anonymous_numeric_results(program, operand)
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
    if !crate::source_state::operator::resolve_spelling_for_operands(
        program, spelling, &operands, span,
    )
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
    reference: crate::source_state::types::TypeReferenceHandle,
) -> Option<(String, Vec<String>)> {
    use crate::source_state::types::{
        DomainConstraintSubject, TypeConstraintNode, TypeReferenceNode,
    };
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

#[derive(Clone, Debug, PartialEq, Eq)]
enum SelectedMeaning {
    Declaration((SelectedOperator, OperatorForm)),
    Core(numerics::intrinsics::CoreIntegerOperatorSignature),
}

fn core_application(
    program: &TypedTrees,
    types: &mut crate::types::typing_queries::DeclaredExpressionTypes,
    owner: symbols::SymbolHandle,
    binary: crate::source_state::expression::TableBinaryExpression,
) -> Option<numerics::intrinsics::CoreIntegerOperatorSignature> {
    use numerics::arithmetic::ArithmeticDomain;
    use numerics::intrinsics::{CoreIntegerOperand, CoreIntegerOperatorSignature};
    if binary.policy != ArithmeticDomain::Exact {
        return None;
    }
    let operation = binary.operator.scalar_operation()?;
    let references = [
        types.get(program, owner, binary.left),
        types.get(program, owner, binary.right),
    ];
    let read = |reference, expression| match reference {
        Some(reference) => {
            let (_, symbols::BuiltinTypeAtom::Integer(carrier), policy) =
                crate::source_state::operator::core_integer_operand(program, reference)?
            else {
                return None;
            };
            Some(Some(CoreIntegerOperand { carrier, policy }))
        }
        None if crate::source_state::closed_numeric::has_anonymous_numeric_results(
            program, expression,
        ) =>
        {
            Some(None)
        }
        None => None,
    };
    let left = read(references[0], binary.left)?;
    let right = read(references[1], binary.right)?;
    let carrier = left.or(right)?.carrier;
    let plain = CoreIntegerOperand {
        carrier,
        policy: ArithmeticDomain::Exact,
    };
    let application = CoreIntegerOperatorSignature::select(
        operation,
        [left.unwrap_or(plain), right.unwrap_or(plain)],
    )?;
    let spelling = crate::source_state::operator::binary_operator_spelling(binary.operator)?;
    (!crate::source_state::operator::has_scoped_operator_meaning(
        program,
        owner,
        spelling,
        &references,
    ))
    .then_some(application)
}

fn retain_core_application(
    program: &mut TypedTrees,
    handle: crate::source_state::expression::ExpressionHandle,
    application: numerics::intrinsics::CoreIntegerOperatorSignature,
) -> Result<(), Diagnostic> {
    use crate::declaration_selection::AuthoredDeclarationSelectionIntrinsic as Intrinsic;
    let span = program.expression_table.source_span(handle);
    let occurrences = program
        .expression_table
        .authored_selection_occurrences(handle)
        .filter(|occurrence| {
            program
                .authored_declaration_selections()
                .get(*occurrence)
                .is_some_and(|row| row.kind() == Kind::Operator && row.source_span() == span)
        })
        .collect::<Vec<_>>();
    let [occurrence] = occurrences.as_slice() else {
        return Err(
            Diagnostic::error("core operator lost its unique source selection")
                .with_source_span(span),
        );
    };
    let target = program
        .authored_declaration_selections()
        .get(*occurrence)
        .unwrap()
        .target();
    match target {
        Target::LateBound(Late::CheckedOperator) => program
            .finalize_authored_intrinsic_selection(
                *occurrence,
                Late::CheckedOperator,
                Intrinsic::CoreIntegerOperator(application),
            )
            .map_err(|error| {
                Diagnostic::error(format!(
                    "core operator selection cannot be retained: {error:?}"
                ))
                .with_source_span(span)
            }),
        Target::Intrinsic(Intrinsic::CoreIntegerOperator(previous)) if previous == application => {
            Ok(())
        }
        _ => Err(
            Diagnostic::error("core operator conflicts with its selected declaration")
                .with_source_span(span),
        ),
    }
}

pub(crate) fn prepare_site(
    program: &mut TypedTrees,
    owners: &crate::expressions::operand_owners::CohortOperandOwners,
    types: &mut crate::types::typing_queries::DeclaredExpressionTypes,
    handle: crate::source_state::expression::ExpressionHandle,
) -> Result<(), Diagnostic> {
    let span = program.expression_table.source_span(handle);
    let binary = match program.expression_table.expression(handle) {
        ExpressionNode::Binary(binary) => Some(*binary),
        _ => None,
    };
    let mut answer = None;
    let mut caller = symbols::SymbolHandle::invalid();
    for owner in owners.expression_owners(handle) {
        let selected = match selection(program, types, owner, handle)? {
            Some(selected) => Some(SelectedMeaning::Declaration(selected)),
            None => binary
                .and_then(|binary| core_application(program, types, owner, binary))
                .map(SelectedMeaning::Core),
        };
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
            && let Some(binary) = binary
            && let Some(error) = qualified_builtin_operand(program, types, caller, handle, binary)
        {
            return Err(error);
        }
        return Ok(());
    };
    let selected = match selected {
        SelectedMeaning::Core(application) => {
            return retain_core_application(program, handle, application);
        }
        SelectedMeaning::Declaration(selected) => selected,
    };
    let (selected, form) = selected;
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
    let mut operands = form.operands;
    if let Some(access) = selected.collection_borrow {
        let table = &mut program.expression_table;
        let target = operands[0];
        let span = table.source_span(target);
        let borrow = table.insert(ExpressionNode::Borrow(
            crate::source_state::expression::TableBorrowExpression { target, access },
        ));
        table.set_source_span(borrow, span);
        operands[0] = borrow;
    }
    let arguments = program
        .expression_table
        .insert_expression_handles(operands[..form.count].iter().copied());
    let (machine_arguments, application, result_type) =
        retain_application(program, &selected, caller, &label, result_type, span)?;
    *program.expression_table.expression_mut(handle) = ExpressionNode::Call(TableCallExpression {
        runtime_call: None,
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
    label: &crate::source_state::name::Identifier,
    result_type: Option<crate::source_state::types::TypeReferenceHandle>,
    span: source_text::SourceSpan,
) -> Result<
    (
        Vec<crate::source_state::expression::StaticMachineArgument>,
        crate::source_state::static_applications::StaticMachineApplicationHandle,
        Option<crate::source_state::types::TypeReferenceHandle>,
    ),
    Diagnostic,
> {
    use crate::source_state::expression::StaticMachineArgument;
    use crate::source_state::static_applications::{
        StaticMachineBinding, StaticMachineBindingValue,
    };
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
                let value = application_identity::const_value::CanonicalConstValue::new(
                    value.type_name.clone(),
                    value.encoding.clone(),
                    "",
                );
                let reference = program.type_reference_table.insert(
                    crate::source_state::types::TypeReferenceNode::Named {
                        symbol: symbols::SymbolHandle::invalid(),
                        name: crate::source_state::name::Identifier::generated(value.atom()),
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
