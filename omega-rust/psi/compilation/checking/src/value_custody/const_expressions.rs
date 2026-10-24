//! Static value category and carrier at a retained expression operand.
//!
//! A type-reference slot may retain a value recipe. Integer arithmetic was
//! historically the only recipe admitted here, although canonical consts also
//! include arrays, records and cases. This judgment follows the typed value
//! tree and exact declaration leaves; it never evaluates an operator token or
//! grants formation, purity, construction predicates or executable custody.

use symbols::SymbolKind;
use typed_trees::data::TypeParameterKind;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};

/// Judge the source recipe before specialization can replace its immutable
/// leaves or erase its operators. Closed canonical replay checks the resulting
/// value again; it cannot replace these supplying-contract obligations.
pub(crate) fn validate_formation(
    program: &dyn crate::CheckingInput,
    caller: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    expression: ExpressionHandle,
    expected: typed_trees::types::TypeReferenceHandle,
    owner: &str,
    diagnostics: &mut Vec<diagnostics::Diagnostic>,
    formation_obligations: &mut Vec<checked_trees::IntegerRangeObligation>,
) {
    let environment = crate::proof_contracts::arithmetic_domains::requires_value_environment(
        program, caller, state,
    );
    let mut decisions = crate::proof_contracts::arithmetic_domains::ArithmeticDecisions::default();
    crate::proof_contracts::arithmetic_domains::validate_value_range_retaining_decisions(
        program,
        caller,
        Some(state),
        expression,
        &environment,
        program.primitive_type_reference(expected),
        program.arithmetic_domain_for_type_reference(expected),
        owner,
        diagnostics,
        &mut decisions,
    );
    formation_obligations.extend(decisions.integer_range_obligations);
    if !crate::expression_result_type_reference(program, caller, state, expression).is_some_and(
        |actual| super::type_references::type_references_match(program, actual, expected),
    ) {
        diagnostics.push(diagnostics::Diagnostic::error(format!(
            "{owner} has no selected result with its declared carrier"
        )));
    }
    if let Err(mut failures) = super::struct_literals::validate_value_construction(
        program,
        caller,
        state,
        expression,
        formation_obligations,
    ) {
        diagnostics.append(&mut failures);
    }
}

pub(crate) fn validate_argument_recipes(
    program: &dyn crate::CheckingInput,
    caller: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    parameters: &[typed_trees::data::TypeParameter],
    arguments: &[typed_trees::expression::StaticMachineArgument],
    diagnostics: &mut Vec<diagnostics::Diagnostic>,
    formation_obligations: &mut Vec<checked_trees::IntegerRangeObligation>,
) {
    // Arity and kind errors retain their normal admission diagnostic. An
    // incomplete pack cannot supply a guessed destination to a source recipe.
    if parameters.len() != arguments.len() {
        return;
    }
    for (parameter, argument) in parameters.iter().zip(arguments) {
        if let (TypeParameterKind::Const {
            type_reference: expected,
        }
        | TypeParameterKind::Value {
            type_reference: expected,
        }) = parameter.kind
            && argument.type_reference.is_valid()
            && let typed_trees::types::TypeReferenceNode::ConstExpression(expression) = program
                .type_reference_table
                .type_reference(argument.type_reference)
        {
            if !has_carrier(program, *expression, &parameter.kind, caller.symbol) {
                if matches!(parameter.kind, TypeParameterKind::Const { .. })
                    && super::type_references::exact_const_carrier_is_eligible(program, expected)
                {
                    diagnostics.push(diagnostics::Diagnostic::error(format!(
                        "const argument `{}` has no static value with its exact declared carrier",
                        parameter.name,
                    )));
                }
                continue;
            }
            validate_formation(
                program,
                caller,
                state,
                *expression,
                expected,
                &format!(
                    "machine `{}` const argument `{}`",
                    caller.name, parameter.name
                ),
                diagnostics,
                formation_obligations,
            );
        }
        if let Some(application) = &argument.application
            && let Some(declaration) = program.conformance_by_symbol(argument.symbol)
        {
            validate_argument_recipes(
                program,
                caller,
                state,
                program.conformance_type_parameters(declaration),
                &application.arguments,
                diagnostics,
                formation_obligations,
            );
        }
    }
}

pub(crate) fn has_carrier(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
    binding_kind: &TypeParameterKind,
    lexical_owner: symbols::SymbolHandle,
) -> bool {
    let (TypeParameterKind::Const {
        type_reference: expected,
    }
    | TypeParameterKind::Value {
        type_reference: expected,
    }) = *binding_kind
    else {
        return false;
    };
    let runtime_value = matches!(binding_kind, TypeParameterKind::Value { .. });
    // Anonymous integer operands land under the destination carrier. Their
    // immutable named leaves retain the independent declaration's carrier.
    if program
        .primitive_type_reference(expected)
        .is_some_and(|primitive| primitive.accepts_integer_literal())
        && typed_trees::const_integer::is_integer_expression(
            &program.expression_table,
            expression,
            &|symbol, _| {
                program.data_type_parameter_by_symbol(symbol).is_some_and(|binder| {
                    matches!(binder.kind, TypeParameterKind::Const { type_reference }
                        if super::type_references::type_references_match(program, type_reference, expected))
                        || runtime_value && matches!(binder.kind, TypeParameterKind::Value { type_reference }
                        if super::type_references::type_references_match(program, type_reference, expected))
                })
            },
        )
    {
        return true;
    }
    if !super::type_references::exact_const_carrier_is_eligible(program, expected)
        || !has_value_category(program, expression, runtime_value)
    {
        return false;
    }
    // Construction retains the selected nominal application, not a type
    // guessed from the required destination or from the first field.
    let actual = match program.expression_table.expression(expression) {
        ExpressionNode::StructLiteral(literal) => {
            let actual = program
                .expression_table
                .constructed_value_type_reference(expression);
            actual.is_valid().then_some(actual).or_else(|| {
                program
                    .type_reference_table
                    .find_named_type_reference(literal.type_symbol)
            })
        }
        _ => crate::parameter_expression_result_type_reference(
            program,
            lexical_owner,
            &[],
            expression,
        ),
    };
    actual.is_some_and(|actual| {
        super::type_references::type_references_match(program, actual, expected)
    })
}

fn has_value_category(
    program: &dyn crate::CheckingInput,
    root: ExpressionHandle,
    runtime_value: bool,
) -> bool {
    let mut pending = vec![root];
    let mut visited = Vec::new();
    while let Some(expression) = pending.pop() {
        if !program.expression_table.expression_is_valid(expression) {
            return false;
        }
        if visited.contains(&expression) {
            continue;
        }
        visited.push(expression);
        let node = program.expression_table.expression(expression);
        match node {
            ExpressionNode::Name(path) => {
                let immutable = if path.symbol.is_valid() {
                    match program.symbols.get(path.symbol).kind {
                        SymbolKind::TypeParameter => program
                            .data_type_parameter_by_symbol(path.symbol)
                            .is_some_and(|parameter| {
                                matches!(parameter.kind, TypeParameterKind::Const { .. })
                                    || runtime_value
                                        && matches!(parameter.kind, TypeParameterKind::Value { .. })
                            }),
                        SymbolKind::Const => program
                            .const_declarations()
                            .iter()
                            .find(|declaration| declaration.symbol == path.symbol)
                            .is_some_and(|declaration| {
                                declaration
                                    .evaluated_value
                                    .as_ref()
                                    .and_then(|value| value.encoding())
                                    .is_some_and(|encoding| {
                                        crate::validate_exact_const_value_encoding(
                                            program,
                                            declaration.declared_type,
                                            encoding,
                                        )
                                        .is_ok()
                                    })
                            }),
                        _ => false,
                    }
                } else {
                    // Specialization may replace an integer binder with an
                    // anonymous numeric name. Source names never gain this lane.
                    let members = program.expression_table.name_path_members(path.members);
                    matches!(members, [name] if name.as_str().parse::<i128>().is_ok())
                };
                if !immutable {
                    return false;
                }
            }
            ExpressionNode::Integer(_)
            | ExpressionNode::Boolean(_)
            | ExpressionNode::Binary(_)
            | ExpressionNode::Unary(_)
            | ExpressionNode::StructLiteral(_)
            | ExpressionNode::ArrayLiteral(_)
            | ExpressionNode::Member(_)
            | ExpressionNode::Indexed(_)
            | ExpressionNode::Match(_) => {}
            // A call, reference or opaque observation needs its ordinary
            // checked evaluation before it can supply a canonical value.
            _ => return false,
        }
        crate::expression_traversal::children(program, node, |child| pending.push(child));
    }
    true
}

/// Exact transient coordinates of a timeless Const record observation.
#[derive(Debug, Clone)]
pub struct ImmutableConstRecordProjection {
    pub root: symbols::SymbolHandle,
    pub fields: Vec<symbols::SymbolHandle>,
    pub carrier: typed_trees::types::TypeReferenceHandle,
}

/// A projection of one immutable lexical Const value is timeless. This query
/// supplies only its declaration/field coordinates and scalar carrier, never a
/// field predicate or a fact about runtime/borrowed storage. Runtime Value
/// binders, references, locals, cases and unresolved field names fail closed.
pub fn immutable_record_projection(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
) -> Option<ImmutableConstRecordProjection> {
    let mut current = expression;
    let mut members = Vec::new();
    let mut visited = Vec::new();
    loop {
        if !program.expression_table.expression_is_valid(current) || visited.contains(&current) {
            return None;
        }
        visited.push(current);
        let ExpressionNode::Member(member) = program.expression_table.expression(current) else {
            break;
        };
        if !member.member_symbol.is_valid() || member.case_variant.is_some() {
            return None;
        }
        members.push(member);
        current = member.receiver;
    }
    if members.is_empty() {
        return None;
    }
    let ExpressionNode::Name(path) = program.expression_table.expression(current) else {
        return None;
    };
    if !path.symbol.is_valid()
        || path.head_symbol != path.symbol
        || program
            .expression_table
            .name_path_members(path.members)
            .len()
            != 1
    {
        return None;
    }
    let parameter = program.data_type_parameter_by_symbol(path.symbol)?;
    let TypeParameterKind::Const {
        type_reference: mut carrier,
    } = parameter.kind
    else {
        return None;
    };
    if !super::type_references::exact_const_carrier_is_eligible(program, carrier) {
        return None;
    }
    let mut fields = Vec::with_capacity(members.len());
    for member in members.into_iter().rev() {
        let field = super::places::applied_data_member_field(program, carrier, member)?;
        if field.symbol != member.member_symbol
            || program
                .symbols
                .get(program.symbols.get(field.symbol).parent)
                .kind
                == SymbolKind::Variant
        {
            return None;
        }
        fields.push(field.symbol);
        carrier = field.type_reference;
    }
    Some(ImmutableConstRecordProjection {
        root: parameter.symbol,
        fields,
        carrier,
    })
}
