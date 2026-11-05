//! Static value category and carrier at a retained expression operand.
//!
//! A type-reference slot may retain a value recipe. Integer arithmetic was
//! historically the only recipe admitted here, although canonical consts also
//! include arrays, records and cases. This judgment follows the typed value
//! tree and exact declaration leaves; it never evaluates an operator token or
//! grants formation, purity, construction predicates or executable custody.

use crate::frozen_program::expression_table;
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
    // Each element is its own recipe under the array's element carrier; the
    // literal itself has no nominal result type to compare.
    if let Some((elements, element_type)) = array_literal_elements(program, expression, expected) {
        for element in elements {
            validate_formation(
                program,
                caller,
                state,
                element,
                element_type,
                owner,
                diagnostics,
                formation_obligations,
            );
        }
        return;
    }
    let environment = crate::proof_contracts::arithmetic_domains::requires_value_environment(
        program, caller, state,
    );
    crate::proof_contracts::arithmetic_domains::validate_value_range_retaining_obligations(
        program,
        caller,
        Some(state),
        expression,
        &environment,
        program.primitive_type_reference(expected),
        program.arithmetic_domain_for_type_reference(expected),
        owner,
        diagnostics,
        formation_obligations,
    );
    let anonymous_integer = program
        .primitive_type_reference(expected)
        .is_some_and(|primitive| primitive.accepts_integer_literal())
        && typed_trees::const_integer::is_integer_expression(
            expression_table(program),
            expression,
            &|symbol, _| {
                program.data_type_parameter_by_symbol(symbol).is_some_and(|binder| {
                    matches!(binder.kind, TypeParameterKind::Const { type_reference } | TypeParameterKind::Value { type_reference }
                        if super::type_references::type_references_match(program, type_reference, expected))
                })
            },
        );
    if !anonymous_integer
        && !crate::expression_result_type_reference(program, caller, state, expression).is_some_and(
            |actual| super::type_references::type_references_match(program, actual, expected),
        )
    {
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
        if let TypeParameterKind::Const {
            type_reference: expected,
        }
        | TypeParameterKind::Value {
            type_reference: expected,
        } = parameter.kind
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
            expression_table(program),
            expression,
            &|symbol, _| {
                program.data_type_parameter_by_symbol(symbol).is_some_and(|binder| {
                    matches!(binder.kind, TypeParameterKind::Const { type_reference }
                        if super::type_references::type_references_match(program, type_reference, expected))
                        || (runtime_value
                            || program.typed_program().finite_value_binder_in_scope(lexical_owner, symbol))
                            && matches!(binder.kind, TypeParameterKind::Value { type_reference }
                        if super::type_references::type_references_match(program, type_reference, expected))
                })
            },
        )
    {
        return true;
    }
    if !super::type_references::exact_const_carrier_is_eligible(program, expected)
        || !has_value_category(program, expression, runtime_value, lexical_owner)
    {
        return false;
    }
    // An array literal has no nominal carrier to retain. Its fixed extent and
    // every element land under the expected array's own element carrier, as
    // a record literal's fields land under its declared field carriers.
    if matches!(
        expression_table(program).expression(expression),
        ExpressionNode::ArrayLiteral(_)
    ) {
        let Some((elements, element_type)) = array_literal_elements(program, expression, expected)
        else {
            return false;
        };
        let element_kind = match binding_kind {
            TypeParameterKind::Value { .. } => TypeParameterKind::Value {
                type_reference: element_type,
            },
            _ => TypeParameterKind::Const {
                type_reference: element_type,
            },
        };
        return elements
            .iter()
            .all(|element| has_carrier(program, *element, &element_kind, lexical_owner));
    }
    // Construction retains the selected nominal application, not a type
    // guessed from the required destination or from the first field.
    let actual = match expression_table(program).expression(expression) {
        ExpressionNode::StructLiteral(literal) => {
            let actual = expression_table(program).constructed_value_type_reference(expression);
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

/// The elements of an array literal whose count equals the expected fixed
/// array's closed extent, paired with that array's element carrier.
fn array_literal_elements(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
    expected: typed_trees::types::TypeReferenceHandle,
) -> Option<(
    Vec<ExpressionHandle>,
    typed_trees::types::TypeReferenceHandle,
)> {
    let ExpressionNode::ArrayLiteral(elements) = expression_table(program).expression(expression)
    else {
        return None;
    };
    let (element_type, length) = fixed_array_geometry(program, expected)?;
    let elements = expression_table(program).expression_handles(*elements);
    (elements.len() == length).then(|| (elements.to_vec(), element_type))
}

fn has_value_category(
    program: &dyn crate::CheckingInput,
    root: ExpressionHandle,
    runtime_value: bool,
    lexical_owner: symbols::SymbolHandle,
) -> bool {
    let mut pending = vec![root];
    let mut visited = Vec::new();
    while let Some(expression) = pending.pop() {
        if !expression_table(program).expression_is_valid(expression) {
            return false;
        }
        if visited.contains(&expression) {
            continue;
        }
        visited.push(expression);
        let node = expression_table(program).expression(expression);
        match node {
            ExpressionNode::Name(path) => {
                let immutable = if path.symbol.is_valid() {
                    match program.symbols.get(path.symbol).kind {
                        SymbolKind::TypeParameter => program
                            .data_type_parameter_by_symbol(path.symbol)
                            .is_some_and(|parameter| {
                                matches!(parameter.kind, TypeParameterKind::Const { .. })
                                    || (runtime_value
                                        || program.typed_program().finite_value_binder_in_scope(
                                            lexical_owner,
                                            path.symbol,
                                        ))
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
                    let members = expression_table(program).name_path_members(path.members);
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
        crate::queries::expression_traversal::children(program, node, |child| pending.push(child));
    }
    true
}

/// One step from an immutable Const aggregate to a contained value: a record
/// field by its declaration symbol, or a fixed-array element by a literal
/// position inside the array's declared extent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConstProjectionStep {
    Field(symbols::SymbolHandle),
    Element(usize),
}

/// Exact transient coordinates of a timeless Const aggregate observation.
#[derive(Debug, Clone)]
pub struct ImmutableConstProjection {
    pub root: symbols::SymbolHandle,
    pub path: Vec<ConstProjectionStep>,
    pub carrier: typed_trees::types::TypeReferenceHandle,
}

/// A projection of one immutable lexical Const value is timeless. This query
/// supplies only its declaration/field/element coordinates and scalar carrier,
/// never a field predicate or a fact about runtime/borrowed storage. Runtime
/// Value binders, references, locals, cases, unresolved field names and
/// non-literal or out-of-extent element positions fail closed.
pub fn immutable_const_projection(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
) -> Option<ImmutableConstProjection> {
    enum Step<'node> {
        Member(&'node typed_trees::expression::TableMemberExpression),
        Element(ExpressionHandle),
    }
    let mut current = expression;
    let mut steps = Vec::new();
    let mut visited = Vec::new();
    loop {
        if !program.expression_table.expression_is_valid(current) || visited.contains(&current) {
            return None;
        }
        visited.push(current);
        match expression_table(program).expression(current) {
            ExpressionNode::Member(member) => {
                if !member.member_symbol.is_valid() || member.case_variant.is_some() {
                    return None;
                }
                steps.push(Step::Member(member));
                current = member.receiver;
            }
            ExpressionNode::Indexed(indexed) => {
                steps.push(Step::Element(indexed.index));
                current = indexed.collection;
            }
            _ => break,
        }
    }
    if steps.is_empty() {
        return None;
    }
    let ExpressionNode::Name(path) = expression_table(program).expression(current) else {
        return None;
    };
    if !path.symbol.is_valid()
        || path.head_symbol != path.symbol
        || expression_table(program)
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
    let mut projection = Vec::with_capacity(steps.len());
    for step in steps.into_iter().rev() {
        match step {
            Step::Member(member) => {
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
                projection.push(ConstProjectionStep::Field(field.symbol));
                carrier = field.type_reference;
            }
            Step::Element(index) => {
                let ExpressionNode::Integer(index) = expression_table(program).expression(index)
                else {
                    return None;
                };
                let index = usize::try_from(index.value_i64()?).ok()?;
                let (element, length) = fixed_array_geometry(program, carrier)?;
                if index >= length {
                    return None;
                }
                projection.push(ConstProjectionStep::Element(index));
                carrier = element;
            }
        }
    }
    Some(ImmutableConstProjection {
        root: parameter.symbol,
        path: projection,
        carrier,
    })
}

/// The element carrier and closed extent of a declared fixed-array carrier.
pub fn fixed_array_geometry(
    program: &dyn crate::CheckingInput,
    carrier: typed_trees::types::TypeReferenceHandle,
) -> Option<(typed_trees::types::TypeReferenceHandle, usize)> {
    match program
        .type_reference_table
        .type_reference(crate::peel_declared_constraints(program, carrier))
    {
        typed_trees::types::TypeReferenceNode::FixedArray {
            element_type,
            length: typed_trees::types::FixedArrayLength::Literal(length),
        } => Some((*element_type, *length)),
        _ => None,
    }
}
