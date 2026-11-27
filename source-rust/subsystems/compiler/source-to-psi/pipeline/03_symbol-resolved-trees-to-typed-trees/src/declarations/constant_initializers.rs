//! A constant position asks for an ordinary zero-argument computation, kept as
//! typed machine input. Only structural discharge and invocation admission may
//! allow running it. The written expression stays separate from the value
//! evaluation later produces.

use crate::source_state::statement::{StatementNode, TableTransition, TransitionTargetNode};
use crate::source_state::{TypedTrees, constant::ConstDeclaration, machine::Machine, state::State};
use arena::Handle;
use diagnostics::Diagnostic;
use symbols::SymbolKind;

pub(crate) fn retain_entry(
    program: &mut TypedTrees,
    declaration: Handle<ConstDeclaration>,
) -> Result<(), Diagnostic> {
    let constant = program.tables.const_declarations.get(declaration).clone();
    if constant.evaluated_value.is_some() || constant.pending_target_observation {
        return Ok(());
    }
    if !program
        .expression_table
        .expression_is_valid(constant.materialized_initializer)
    {
        return Err(Diagnostic::error("constant lost its retained initializer"));
    }
    let name = format!(
        "@constant-initializer<{}>",
        program.symbols.display_path(constant.symbol, "::")
    );
    let symbol =
        program
            .symbols
            .insert_generated_root_from(constant.symbol, SymbolKind::Machine, &name);
    let state_symbol = program
        .symbols
        .insert_generated_children(symbol, [(SymbolKind::State, name.as_str())])
        .start();
    let mut machine = Machine {
        symbol,
        name: crate::source_state::name::Identifier::generated(&name),
        ..Default::default()
    };
    let mut state = State {
        symbol: state_symbol,
        name: machine.name.clone(),
        return_type: constant.declared_type,
        ..Default::default()
    };
    let target = program
        .statement_table
        .insert_transition_target(TransitionTargetNode::Value(
            constant.materialized_initializer,
        ));
    program.statement_table.push_statement(
        &mut state.statement_nodes,
        StatementNode::Transition(TableTransition {
            target,
            source_span: constant.initializer_source_span,
            ..Default::default()
        }),
    );
    program.push_machine_state(&mut machine, state);
    let entry = program
        .tables
        .machines
        .append_to_span(&mut program.roots.machines, machine);
    let constant = program.tables.const_declarations.get_mut(declaration);
    constant.evaluation_entry = entry;
    constant.evaluation_expression = constant.materialized_initializer;
    Ok(())
}

/// Infer the declared type of every unannotated constant in the current cohort
/// from its initializer.
///
/// Constants that depend on other unannotated constants are resolved recursively
/// with cycle detection. When `require_complete` is `false`, initializers whose
/// type is not yet ready (for example a domain cast settled later in cohort
/// closure) are left pending. When `require_complete` is `true`, any remaining
/// unannotated constant whose initializer still has no determined type is
/// refused at its initializer.
pub(crate) fn infer_unannotated_types_in_cohort(
    program: &mut TypedTrees,
    cohort: &crate::lowerer::TypingCohort<'_>,
    require_complete: bool,
) -> Result<(), Diagnostic> {
    let handles = program
        .roots
        .const_declarations
        .handles()
        .skip(cohort.const_declarations)
        .collect::<Vec<_>>();
    for handle in handles {
        let constant = program.tables.const_declarations.get(handle);
        if program
            .type_reference_table
            .contains_type_reference(constant.declared_type)
        {
            continue;
        }
        let symbol = constant.symbol;
        let span = constant.initializer_source_span;
        let mut visiting = Vec::new();
        if let Some(inferred) = infer_constant_handle(program, handle, &mut visiting)? {
            commit_inferred_type(program, handle, inferred);
        } else if require_complete {
            return Err(Diagnostic::error(format!(
                "constant `{}` has no type annotation and its initializer has no determined type: annotate the constant's type",
                program.symbols.display_path(symbol, "::"),
            ))
            .with_source_span(span));
        }
    }
    Ok(())
}

fn commit_inferred_type(
    program: &mut TypedTrees,
    handle: Handle<ConstDeclaration>,
    inferred: crate::source_state::types::TypeReferenceHandle,
) {
    let entry = {
        let constant = program.tables.const_declarations.get_mut(handle);
        constant.declared_type = inferred;
        constant.evaluation_entry
    };
    if program.tables.machines.is_valid(entry) {
        let states = program.tables.machines.get(entry).states;
        if let Some(state_handle) = states.handles().next() {
            program
                .tables
                .machine_states
                .get_mut(state_handle)
                .return_type = inferred;
        }
    }
}

fn infer_constant_handle(
    program: &mut TypedTrees,
    handle: Handle<ConstDeclaration>,
    visiting: &mut Vec<symbols::SymbolHandle>,
) -> Result<Option<crate::source_state::types::TypeReferenceHandle>, Diagnostic> {
    let constant = program.tables.const_declarations.get(handle).clone();
    if program
        .type_reference_table
        .contains_type_reference(constant.declared_type)
    {
        return Ok(Some(constant.declared_type));
    }
    if visiting.contains(&constant.symbol) {
        return Err(Diagnostic::error(format!(
            "constant `{}` has a cyclic initializer dependency and no type annotation",
            program.symbols.display_path(constant.symbol, "::"),
        ))
        .with_source_span(constant.initializer_source_span));
    }
    let initializer = if program
        .expression_table
        .expression_is_valid(constant.materialized_initializer)
    {
        constant.materialized_initializer
    } else if program
        .expression_table
        .expression_is_valid(constant.authored_initializer)
    {
        constant.authored_initializer
    } else {
        return Ok(None);
    };
    let owner = if program.tables.machines.is_valid(constant.evaluation_entry) {
        program
            .tables
            .machines
            .get(constant.evaluation_entry)
            .symbol
    } else {
        constant.symbol
    };
    visiting.push(constant.symbol);
    let result = infer_expression_type(program, owner, initializer, visiting);
    visiting.pop();
    let inferred = result?;
    if let Some(inferred) = inferred {
        commit_inferred_type(program, handle, inferred);
    }
    Ok(inferred)
}

fn ensure_builtin_type_reference(
    program: &mut TypedTrees,
    atom: symbols::BuiltinTypeAtom,
    domain: numerics::arithmetic::ArithmeticDomain,
) -> Option<crate::source_state::types::TypeReferenceHandle> {
    let symbol = program.symbols.builtin_atom_symbol(atom)?;
    if let Some(existing) = program
        .type_reference_table
        .find_arithmetic_result_type_reference(symbol, domain)
    {
        return Some(existing);
    }
    let base_type = ensure_named_type_reference(program, symbol);
    if domain == numerics::arithmetic::ArithmeticDomain::Exact {
        return Some(base_type);
    }
    let constraints = program.type_reference_table.insert_constraints([
        crate::source_state::types::TypeConstraintNode::ArithmeticDomain(domain),
    ]);
    Some(program.type_reference_table.insert(
        crate::source_state::types::TypeReferenceNode::Constrained {
            base_type,
            constraints,
        },
    ))
}

fn ensure_named_type_reference(
    program: &mut TypedTrees,
    symbol: symbols::SymbolHandle,
) -> crate::source_state::types::TypeReferenceHandle {
    if let Some(existing) = program
        .type_reference_table
        .find_named_type_reference(symbol)
    {
        return existing;
    }
    let name = crate::source_state::name::Identifier::generated(program.symbols.name(symbol));
    program
        .type_reference_table
        .insert(crate::source_state::types::TypeReferenceNode::Named { symbol, name })
}

fn infer_expression_type(
    program: &mut TypedTrees,
    owner: symbols::SymbolHandle,
    expression: crate::source_state::expression::ExpressionHandle,
    visiting: &mut Vec<symbols::SymbolHandle>,
) -> Result<Option<crate::source_state::types::TypeReferenceHandle>, Diagnostic> {
    use crate::source_state::expression::{BinaryOperator, ExpressionNode, UnaryOperator};
    use crate::source_state::types::{FixedArrayLength, TypeReferenceNode};
    use numerics::arithmetic::ArithmeticDomain;
    use symbols::BuiltinTypeAtom;

    if !program.expression_table.expression_is_valid(expression) {
        return Ok(None);
    }
    let retained = program
        .expression_table
        .constructed_value_type_reference(expression);
    if program
        .type_reference_table
        .contains_type_reference(retained)
    {
        return Ok(Some(retained));
    }
    if let Some(reference) = program
        .declared_callable_expression_type_reference(expression)
        .map_err(Diagnostic::error)?
        && program
            .type_reference_table
            .contains_type_reference(reference)
    {
        return Ok(Some(reference));
    }
    let node = program.expression_table.expression(expression).clone();
    match node {
        ExpressionNode::Integer(literal) => Ok(literal.landing().and_then(|landing| {
            ensure_builtin_type_reference(
                program,
                crate::source_state::types::builtin_type_atom(landing.landed_type),
                landing.domain,
            )
        })),
        ExpressionNode::Float(literal) => Ok(literal.landing().and_then(|format| {
            ensure_builtin_type_reference(
                program,
                crate::types::typing_queries::float_literal_atom(format),
                ArithmeticDomain::Exact,
            )
        })),
        ExpressionNode::UndeterminedNaN(format) => Ok(ensure_builtin_type_reference(
            program,
            crate::types::typing_queries::float_literal_atom(format),
            ArithmeticDomain::Exact,
        )),
        ExpressionNode::Boolean(_) => Ok(ensure_builtin_type_reference(
            program,
            BuiltinTypeAtom::Bool,
            ArithmeticDomain::Exact,
        )),
        ExpressionNode::Name(path) => {
            if let Some(target_handle) =
                program.roots.const_declarations.handles().find(|&handle| {
                    let decl = program.tables.const_declarations.get(handle);
                    (path.symbol.is_valid() && decl.symbol == path.symbol)
                        || (path.head_symbol.is_valid() && decl.symbol == path.head_symbol)
                })
            {
                return infer_constant_handle(program, target_handle, visiting);
            }
            if let Some(symbol) = path.resolved_symbol()
                && program.symbols.symbols().nodes().is_valid(symbol)
                && program.symbols.get(symbol).kind == SymbolKind::Variant
            {
                let parent = program.symbols.get(symbol).parent;
                if let Some(data) = program.unique_data_definition_by_symbol(parent)
                    && program.data_type_parameters(data).is_empty()
                    && data.generic_instance.is_none()
                {
                    return Ok(Some(ensure_named_type_reference(program, parent)));
                }
            }
            Ok(crate::types::typing_queries::declared_expression_type(
                program, owner, expression,
            ))
        }
        ExpressionNode::Call(call) => {
            if language_core::data_layout_queries::is_data_layout_query_spelling(
                call.target.as_str(),
                call.target_symbol.is_valid(),
                call.receiver.is_valid(),
            ) {
                return Ok(ensure_builtin_type_reference(
                    program,
                    BuiltinTypeAtom::U64,
                    ArithmeticDomain::Exact,
                ));
            }
            if let Some(result) =
                crate::types::typing_queries::declared_expression_type(program, owner, expression)
            {
                return Ok(Some(result));
            }
            if call.target_symbol.is_valid()
                && let Some(result) = program
                    .state_by_symbol(call.target_symbol)
                    .map(|state| state.return_type)
                    .or_else(|| {
                        program
                            .machine_by_symbol(call.target_symbol)
                            .and_then(|machine| {
                                program
                                    .machine_states(machine)
                                    .first()
                                    .map(|state| state.return_type)
                            })
                    })
                    .filter(|reference| {
                        program
                            .type_reference_table
                            .contains_type_reference(*reference)
                    })
            {
                return Ok(Some(result));
            }
            Ok(None)
        }
        ExpressionNode::Unary(unary) => {
            let Some(operand) = infer_expression_type(program, owner, unary.operand, visiting)?
            else {
                return Ok(None);
            };
            match unary.operator {
                UnaryOperator::LogicalNot => Ok(ensure_builtin_type_reference(
                    program,
                    BuiltinTypeAtom::Bool,
                    ArithmeticDomain::Exact,
                )),
                UnaryOperator::BitwiseNot => Ok(
                    crate::source_state::operator::core_integer_operand(program, operand)
                        .map(|_| operand),
                ),
                UnaryOperator::Dereference => {
                    let bare = program
                        .type_reference_table
                        .unconstrained_type_reference(operand);
                    match program.type_reference_table.type_reference(bare) {
                        TypeReferenceNode::Reference { referee, .. } => Ok(Some(*referee)),
                        _ => Ok(None),
                    }
                }
            }
        }
        ExpressionNode::Binary(binary) => {
            let left = if crate::source_state::closed_numeric::has_anonymous_numeric_results(
                program,
                binary.left,
            ) {
                None
            } else {
                infer_expression_type(program, owner, binary.left, visiting)?
            };
            let right = if crate::source_state::closed_numeric::has_anonymous_numeric_results(
                program,
                binary.right,
            ) {
                None
            } else {
                infer_expression_type(program, owner, binary.right, visiting)?
            };
            if binary.operator.is_arithmetic() {
                if left.is_none() && right.is_none() {
                    return Ok(None);
                }
                if let Some(result) = crate::types::typing_queries::declared_expression_type(
                    program, owner, expression,
                ) {
                    return Ok(Some(result));
                }
                if let Some(result) = crate::source_state::operator::core_integer_result(
                    program,
                    binary.operator,
                    binary.policy,
                    left,
                    right,
                ) {
                    return Ok(Some(result));
                }
                let mut chosen: Option<symbols::SymbolHandle> = None;
                let mut chosen_ref = None;
                for reference in [left, right].into_iter().flatten() {
                    let TypeReferenceNode::Named { symbol, .. } =
                        program.type_reference_table.type_reference(reference)
                    else {
                        return Ok(None);
                    };
                    if program.symbols.get(*symbol).kind != SymbolKind::BuiltinType
                        || chosen.is_some_and(|earlier| earlier != *symbol)
                    {
                        return Ok(None);
                    }
                    chosen = Some(*symbol);
                    chosen_ref = Some(reference);
                }
                return Ok(chosen_ref);
            }
            match binary.operator {
                BinaryOperator::Equal
                | BinaryOperator::NotEqual
                | BinaryOperator::Less
                | BinaryOperator::LessOrEqual
                | BinaryOperator::Greater
                | BinaryOperator::GreaterOrEqual
                | BinaryOperator::And
                | BinaryOperator::Or
                | BinaryOperator::CaseMembership => {
                    if left.is_none() && right.is_none() {
                        return Ok(None);
                    }
                    Ok(ensure_builtin_type_reference(
                        program,
                        BuiltinTypeAtom::Bool,
                        ArithmeticDomain::Exact,
                    ))
                }
                _ => Ok(None),
            }
        }
        ExpressionNode::Cast(cast) => {
            if program
                .type_reference_table
                .contains_type_reference(cast.result_type)
            {
                return Ok(Some(cast.result_type));
            }
            if cast.form == language_core::CastForm::Value
                && cast.semantic_domain.is_empty()
                && program
                    .type_reference_table
                    .contains_type_reference(cast.target_type)
            {
                if cast.domain == ArithmeticDomain::Exact {
                    return Ok(Some(cast.target_type));
                }
                if let TypeReferenceNode::Named { symbol, .. } = program
                    .type_reference_table
                    .type_reference(cast.target_type)
                {
                    let symbol = *symbol;
                    if let Some(atom) = program.symbols.builtin_type_atom(symbol) {
                        return Ok(ensure_builtin_type_reference(program, atom, cast.domain));
                    }
                }
            }
            Ok(None)
        }
        ExpressionNode::StructLiteral(literal) => {
            if program
                .type_reference_table
                .contains_type_reference(literal.application_type)
            {
                return Ok(Some(literal.application_type));
            }
            if literal.type_symbol.is_valid()
                && let Some(data) = program.unique_data_definition_by_symbol(literal.type_symbol)
                && program.data_type_parameters(data).is_empty()
                && data.generic_instance.is_none()
            {
                return Ok(Some(ensure_named_type_reference(
                    program,
                    literal.type_symbol,
                )));
            }
            Ok(None)
        }
        ExpressionNode::ArrayLiteral(elements) => {
            let handles = program
                .expression_table
                .expression_handles(elements)
                .to_vec();
            if handles.is_empty() {
                return Ok(None);
            }
            let mut element_type = None;
            for element in &handles {
                let candidate =
                    if crate::source_state::closed_numeric::has_anonymous_numeric_results(
                        program, *element,
                    ) {
                        None
                    } else {
                        infer_expression_type(program, owner, *element, visiting)?
                    };
                if let Some(candidate) = candidate {
                    if let Some(previous) = element_type
                        && program.normalized_type_identity(previous)
                            != program.normalized_type_identity(candidate)
                    {
                        return Ok(None);
                    }
                    element_type = Some(candidate);
                }
            }
            let Some(element_type) = element_type else {
                return Ok(None);
            };
            let array_type = program
                .type_reference_table
                .insert(TypeReferenceNode::FixedArray {
                    element_type,
                    length: FixedArrayLength::Literal(handles.len()),
                });
            Ok(Some(array_type))
        }
        ExpressionNode::Member(member) => {
            let Some(receiver) = infer_expression_type(program, owner, member.receiver, visiting)?
            else {
                return Ok(None);
            };
            Ok(crate::types::typing_queries::selected_member_value_type(
                program,
                receiver,
                member.selection,
            ))
        }
        ExpressionNode::Indexed(indexed) => {
            if matches!(
                program.expression_table.expression(indexed.index),
                ExpressionNode::Range(_)
            ) {
                return Ok(None);
            }
            let Some(collection) =
                infer_expression_type(program, owner, indexed.collection, visiting)?
            else {
                return Ok(None);
            };
            Ok(crate::types::typing_queries::collection_element_type(program, collection).ok())
        }
        ExpressionNode::Match(dispatch) => {
            let arm_values = program
                .expression_table
                .match_arms(dispatch.arms)
                .iter()
                .map(|arm| arm.value)
                .collect::<Vec<_>>();
            let mut joined = None;
            for value in arm_values {
                let Some(candidate) = infer_expression_type(program, owner, value, visiting)?
                else {
                    return Ok(None);
                };
                if joined.is_some_and(|previous| {
                    program.normalized_type_identity(previous)
                        != program.normalized_type_identity(candidate)
                }) {
                    return Ok(None);
                }
                joined.get_or_insert(candidate);
            }
            Ok(joined)
        }
        _ => Ok(crate::types::typing_queries::declared_expression_type(
            program, owner, expression,
        )),
    }
}

#[cfg(test)]
mod tests {
    use crate::source_state::types::{FixedArrayLength, PrimitiveType, TypeReferenceNode};

    fn typed_constant_type(
        source: &str,
        name: &str,
    ) -> (
        crate::source_state::TypedTrees,
        crate::source_state::types::TypeReferenceHandle,
    ) {
        let resolved = crate::resolve_fixture(source);
        let program = crate::lower_symbol_resolved_trees(&resolved).unwrap();
        let declaration = program
            .const_declarations()
            .iter()
            .find(|decl| program.symbols.name(decl.symbol) == name)
            .unwrap_or_else(|| panic!("constant `{name}` not found"));
        let declared_type = declaration.declared_type;
        (program, declared_type)
    }

    #[test]
    fn infers_constant_types_from_calls_other_constants_and_typed_operands() {
        let source = "
            machine flag() -> bool crashes none { true }
            const first = second;
            const second = flag();
            const mask = 2u8 + 3;
            const flipped = 3 + 2u8;
            const ordered = 2u8 < 3;
            machine main() -> u8 {
                let ready: bool = first;
                let ok: bool = ordered;
                match ready && ok {
                    true -> mask + flipped,
                    false -> 0u8
                }
            }
        ";
        let resolved = crate::resolve_fixture(source);
        let program = crate::lower_symbol_resolved_trees(&resolved).unwrap();
        for (name, expected) in [
            ("first", PrimitiveType::Bool),
            ("second", PrimitiveType::Bool),
            ("mask", PrimitiveType::U8),
            ("flipped", PrimitiveType::U8),
            ("ordered", PrimitiveType::Bool),
        ] {
            let declaration = program
                .const_declarations()
                .iter()
                .find(|decl| program.symbols.name(decl.symbol) == name)
                .unwrap_or_else(|| panic!("constant `{name}` not found"));
            assert_eq!(
                program.primitive_type_reference(declaration.declared_type),
                Some(expected),
                "{name}"
            );
            assert!(
                program
                    .tables
                    .machines
                    .is_valid(declaration.evaluation_entry),
                "{name} keeps its evaluation entry"
            );
            let entry = program.tables.machines.get(declaration.evaluation_entry);
            let state = program.machine_states(entry).first().unwrap();
            assert_eq!(state.return_type, declaration.declared_type, "{name}");
        }
    }

    #[test]
    fn infers_constant_types_from_domain_casts_records_and_arrays() {
        let source = "
            domain u32::Metres requires self > 0;
            data Pair [copy] { left: u8; right: u8; }
            const span = 5u32 as +Metres;
            const copy_span = span;
            const pair = Pair { left: 1, right: 2 };
            const bytes = [1u8, 2, 3];
            machine main() -> u8 { pair.left + bytes[0] }
        ";
        let (program, span_type) = typed_constant_type(source, "span");
        assert!(
            matches!(
                program.type_reference_table.type_reference(span_type),
                TypeReferenceNode::Constrained { .. }
            ),
            "expected domain-qualified type for `span`"
        );
        let copy_span = program
            .const_declarations()
            .iter()
            .find(|decl| program.symbols.name(decl.symbol) == "copy_span")
            .unwrap();
        assert_eq!(
            program.normalized_type_identity(copy_span.declared_type),
            program.normalized_type_identity(span_type)
        );
        let pair = program
            .const_declarations()
            .iter()
            .find(|decl| program.symbols.name(decl.symbol) == "pair")
            .unwrap();
        assert!(matches!(
            program.type_reference_table.type_reference(pair.declared_type),
            TypeReferenceNode::Named { name, .. } if name.as_str() == "Pair"
        ));
        let bytes = program
            .const_declarations()
            .iter()
            .find(|decl| program.symbols.name(decl.symbol) == "bytes")
            .unwrap();
        match program
            .type_reference_table
            .type_reference(bytes.declared_type)
        {
            TypeReferenceNode::FixedArray {
                element_type,
                length: FixedArrayLength::Literal(3),
            } => {
                assert_eq!(
                    program.primitive_type_reference(*element_type),
                    Some(PrimitiveType::U8)
                );
            }
            other => panic!("expected [u8; 3], found {other:?}"),
        }
    }

    #[test]
    fn refuses_cyclic_and_undetermined_unannotated_constants() {
        for (source, expected) in [
            (
                "const first = second; const second = first; machine main() -> u8 { 0u8 }",
                "cyclic initializer dependency",
            ),
            (
                "const bad = 2u8 + 3u16; machine main() -> u8 { 0u8 }",
                "no determined type",
            ),
            (
                "const bad = [1, 2]; machine main() -> u8 { 0u8 }",
                "no determined type",
            ),
        ] {
            let resolved = crate::resolve_fixture(source);
            let error = crate::lower_symbol_resolved_trees(&resolved)
                .expect_err(&format!("expected refusal for: {source}"));
            assert!(
                error.message.contains(expected),
                "{source}: got `{}`",
                error.message
            );
        }
    }
}
