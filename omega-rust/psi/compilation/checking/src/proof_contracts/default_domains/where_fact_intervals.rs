//! Read-only interval hypotheses derived from standing default-domain facts.
//!
//! This module does not participate in write-frame inference. It projects
//! only facts that the write validator has already established at every legal
//! observation point, and refuses cyclic or unsupported bounds.

use typed_trees::data::{DataDefinition, DataField, DataVariant};
use typed_trees::domain::ProofFact;
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use typed_trees::machine::Machine;
use typed_trees::state::State;

use super::place_queries::data_definition_for_expression;
use super::symbolic_values::integer_literal_value;
use crate::proof_contracts::arithmetic_domains::Interval;

/// R2 rung 3 slice 7 -- READER HYPOTHESES: the standing where facts refine
/// a field READ's interval. Sound because the write net is TOTAL (every
/// write path re-proves the facts) and gated reads are access-gated, so
/// the facts hold at every legal observation. Bounds come from literals or
/// the co-field's DECLARED range (declared ranges always hold), never from
/// flow values.
pub(crate) fn where_fact_interval(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: Option<&State>,
    expression: ExpressionHandle,
) -> Option<Interval> {
    let member = program.expression_table.member_node(expression)?;
    let definition = data_definition_for_expression(program, machine, state, member.receiver)?;
    field_fact_interval(program, definition, member.member.as_str(), 0)
}

/// The per-field core of the reader hypotheses: the interval the
/// definition's standing where facts pin on `field`. `depth` bounds the
/// TRANSITIVE chain (`count <= mid, mid <= capacity[0..=100]` chains
/// through the unranged middle field); a cyclic pair exhausts the cap and
/// resolves to None -- over-refusal only.
fn field_fact_interval(
    program: &dyn crate::CheckingInput,
    definition: &DataDefinition,
    field: &str,
    depth: usize,
) -> Option<Interval> {
    if depth >= 4 || definition.where_facts.is_empty() {
        return None;
    }
    let field_symbol = program
        .data_members(definition)
        .iter()
        .filter_map(|member| member.as_field())
        .find(|candidate| candidate.name.as_str() == field)?
        .symbol;
    if let Some((_, low, high)) = data_where_field_intervals(program, definition)
        .into_iter()
        .flatten()
        .find(|(symbol, _, _)| {
            program.data_members(definition).iter().any(|member| {
                member.as_field().is_some_and(|candidate| {
                    candidate.symbol == *symbol && candidate.name.as_str() == field
                })
            })
        })
    {
        return Some(Interval { low, high });
    }

    let mut interval = Interval {
        low: None,
        high: None,
    };
    let mut refined = false;
    // A fact spelled `a && b` states both conjuncts, exactly as `a, b` does.
    let mut conjuncts = program
        .proof_facts
        .span_or_empty(definition.where_facts)
        .iter()
        .filter_map(|fact| match fact {
            typed_trees::domain::ProofFact::Expression(expression) => Some(*expression),
            _ => None,
        })
        .collect::<Vec<_>>();
    while let Some(fact_expression) = conjuncts.pop() {
        let Some(binary) = program.expression_table.binary_node(fact_expression) else {
            continue;
        };
        if binary.operator == BinaryOperator::And {
            conjuncts.extend([binary.left, binary.right]);
            continue;
        }
        // R2 rung 3 slice 10 -- PRODUCT hypotheses (`count * stride <= len`,
        // ch12's canonical shape): when OUR field is one FACTOR of a
        // product bounded above, the field's upper bound is
        // bound.high / co-factor.low (floor) -- SOUND iff the co-factor's
        // lower bound is >= 1 (from its declared range or a sibling
        // literal fact) and the field's primitive is UNSIGNED (>= 0).
        if matches!(
            binary.operator,
            BinaryOperator::LessOrEqual | BinaryOperator::Less
        ) && let ExpressionNode::Binary(product) =
            program.expression_table.expression(binary.left)
            && matches!(product.operator, BinaryOperator::Multiply)
        {
            let factor = if side_selects_field(program, product.left, field_symbol) {
                Some(product.right)
            } else if side_selects_field(program, product.right, field_symbol) {
                Some(product.left)
            } else {
                None
            };
            if let Some(factor) = factor
                && field_is_unsigned(program, definition, field_symbol)
                && let Some(factor_low) = factor_lower_bound(program, definition, factor)
                && factor_low >= 1
                && let Some(bound) = bound_source_interval(program, definition, binary.right, depth)
                && let Some(mut bound_high) = bound.high
            {
                if matches!(binary.operator, BinaryOperator::Less) {
                    bound_high = bound_high.saturating_sub(1);
                }
                let high = bound_high.div_euclid(factor_low);
                interval.high = Some(interval.high.map_or(high, |current| current.min(high)));
                refined = true;
                continue;
            }
        }
        let (field_side_left, other) = if side_selects_field(program, binary.left, field_symbol) {
            (true, binary.right)
        } else if side_selects_field(program, binary.right, field_symbol) {
            (false, binary.left)
        } else {
            continue;
        };
        let Some(other_interval) = bound_source_interval(program, definition, other, depth) else {
            continue;
        };
        let operator = if field_side_left {
            binary.operator
        } else {
            binary.operator.mirrored()
        };
        match operator {
            BinaryOperator::LessOrEqual => {
                if let Some(high) = other_interval.high {
                    interval.high = Some(interval.high.map_or(high, |current| current.min(high)));
                    refined = true;
                }
            }
            BinaryOperator::Less => {
                if let Some(high) = other_interval.high.and_then(|high| high.checked_sub(1)) {
                    interval.high = Some(interval.high.map_or(high, |current| current.min(high)));
                    refined = true;
                }
            }
            BinaryOperator::GreaterOrEqual => {
                if let Some(low) = other_interval.low {
                    interval.low = Some(interval.low.map_or(low, |current| current.max(low)));
                    refined = true;
                }
            }
            BinaryOperator::Greater => {
                if let Some(low) = other_interval.low.and_then(|low| low.checked_add(1)) {
                    interval.low = Some(interval.low.map_or(low, |current| current.max(low)));
                    refined = true;
                }
            }
            _ => {}
        }
    }
    refined.then_some(interval)
}

/// The per-field intervals a data declaration's `where` facts state when
/// every conjunct either compares an own field with an integer literal or
/// has an independently checked equivalent predicate-call envelope. Such
/// facts describe the complete product of bounded fields, so representation
/// and handback consumers may use these bounds. A necessary consequence
/// alone never retires a declaration predicate. Unsupported conjuncts leave
/// the entire result unknown; a declaration without facts has an empty list.
pub fn data_where_field_intervals(
    program: &dyn crate::CheckingInput,
    definition: &DataDefinition,
) -> Option<Vec<(symbols::SymbolHandle, Option<i64>, Option<i64>)>> {
    let fields = declared_data_fields(program, definition);
    field_intervals(
        program,
        &fields,
        definition.symbol,
        program.proof_facts.span_or_empty(definition.where_facts),
    )
}

/// A case's interval-only facts constrain its payload, never another case
/// with the same field spelling. Constructors establish these bounds and
/// dispatch exposes them only after selecting this case.
pub fn case_where_field_intervals(
    program: &dyn crate::CheckingInput,
    case: &DataVariant,
) -> Option<Vec<(symbols::SymbolHandle, Option<i64>, Option<i64>)>> {
    let fields = program.data_payload_fields(case).iter().collect::<Vec<_>>();
    field_intervals(
        program,
        &fields,
        case.symbol,
        program.proof_facts.span_or_empty(case.where_facts),
    )
}

/// Whether a case can never be established on this definition: one of its
/// `where` facts is a literal false witness (the `0` an instance's refuted
/// `T == i32` leaves behind in generic synthesis, or an authored `where
/// false`). Typing's exhaustiveness count excludes such a case
/// (`exhaustiveness::variant_is_impossible` in stage 03), so a dispatch may
/// omit it without a `_` arm. Every later roster of the instance's cases —
/// the checked structural shape, the closed-sum terminator, the permission
/// schedule's run coverage, stage 05's computed-case bindings — applies this
/// same rule, or one of them would demand an arm, a tag or a successor for
/// a value that cannot exist. Facts that need evaluation are not
/// contradictions and keep the case.
pub fn impossible_variant(program: &dyn crate::CheckingInput, variant: &DataVariant) -> bool {
    use typed_trees::expression::ExpressionNode;
    program
        .proof_facts
        .span_or_empty(variant.where_facts)
        .iter()
        .any(|fact| match fact {
            typed_trees::domain::ProofFact::Expression(expression) => {
                match program.expression_table.expression(*expression) {
                    ExpressionNode::Integer(literal) => literal.decimal_value_i128() == Some(0),
                    ExpressionNode::Boolean(value) => !*value,
                    _ => false,
                }
            }
            typed_trees::domain::ProofFact::Membership(_)
            | typed_trees::domain::ProofFact::Proposition(_) => false,
        })
}

/// The declaration's declared field rows, in order — where-fact readers
/// bind intervals to the same ordinary members.
fn declared_data_fields<'a>(
    program: &'a dyn crate::CheckingInput,
    definition: &'a DataDefinition,
) -> Vec<&'a DataField> {
    program
        .data_members(definition)
        .iter()
        .filter_map(|member| match member {
            typed_trees::data::DataMember::Field(field) => Some(field),
            typed_trees::data::DataMember::Variant(_) => None,
        })
        .collect()
}

fn field_intervals(
    program: &dyn crate::CheckingInput,
    fields: &[&DataField],
    owner: symbols::SymbolHandle,
    facts: &[ProofFact],
) -> Option<Vec<(symbols::SymbolHandle, Option<i64>, Option<i64>)>> {
    let mut intervals: Vec<(symbols::SymbolHandle, Interval)> = Vec::new();
    let mut conjuncts = facts
        .iter()
        .map(|fact| fact.as_expression().copied())
        .collect::<Option<Vec<_>>>()?;
    while let Some(conjunct) = conjuncts.pop() {
        if let Some(binary) = program.expression_table.binary_node(conjunct)
            && binary.operator == BinaryOperator::And
        {
            // Logical conjunction is a sealed compiler connective; authored
            // operator implementations cannot replace its meaning.
            conjuncts.extend([binary.left, binary.right]);
            continue;
        }
        let rows = conjunct_field_intervals(program, fields, owner, conjunct)?;
        for (field, stated) in rows {
            match intervals.iter_mut().find(|(symbol, _)| *symbol == field) {
                Some((_, interval)) => *interval = interval.intersect(stated),
                None => intervals.push((field, stated)),
            }
        }
    }
    Some(
        intervals
            .into_iter()
            .map(|(symbol, interval)| (symbol, interval.low, interval.high))
            .collect(),
    )
}

fn conjunct_field_intervals(
    program: &dyn crate::CheckingInput,
    fields: &[&DataField],
    owner: symbols::SymbolHandle,
    conjunct: ExpressionHandle,
) -> Option<Vec<(symbols::SymbolHandle, Interval)>> {
    if let Some(bounds) = field_literal_bounds(program, fields, &[ProofFact::Expression(conjunct)])
    {
        bounds
            .into_iter()
            .map(|bound| {
                if !literal_comparison_has_builtin_meaning(program, fields, owner, bound) {
                    return None;
                }
                // Floats retain their own literal/carrier judgment; they are
                // never replaced by an integer interval.
                let value = program.expression_table.integer_node(bound.literal)?;
                let bound_value = i64::try_from(value.decimal_value_i128()?).ok()?;
                let (low, high) = match bound.operator {
                    BinaryOperator::LessOrEqual => (None, Some(bound_value)),
                    BinaryOperator::Less => (None, Some(bound_value.checked_sub(1)?)),
                    BinaryOperator::GreaterOrEqual => (Some(bound_value), None),
                    BinaryOperator::Greater => (Some(bound_value.checked_add(1)?), None),
                    BinaryOperator::Equal => (Some(bound_value), Some(bound_value)),
                    _ => return None,
                };
                Some((bound.field, Interval { low, high }))
            })
            .collect()
    } else {
        exact_predicate_field_intervals(program, fields, owner, conjunct)
    }
}

fn literal_comparison_has_builtin_meaning(
    program: &dyn crate::CheckingInput,
    fields: &[&DataField],
    owner: symbols::SymbolHandle,
    bound: FieldLiteralBound,
) -> bool {
    use language_core::OperatorSpelling;

    let Some(field) = fields.iter().find(|field| field.symbol == bound.field) else {
        return false;
    };
    let Some(binary) = program.expression_table.binary_node(bound.conjunct) else {
        return false;
    };
    let spelling = match binary.operator {
        BinaryOperator::Less => OperatorSpelling::Less,
        BinaryOperator::LessOrEqual => OperatorSpelling::LessEqual,
        BinaryOperator::Greater => OperatorSpelling::Greater,
        BinaryOperator::GreaterOrEqual => OperatorSpelling::GreaterEqual,
        BinaryOperator::Equal => OperatorSpelling::Equal,
        _ => return false,
    };
    // Keep declared qualifications in overload lookup. An unstamped anonymous
    // literal is unknown, never an assumed copy of the opposite field's type.
    let field_type = Some(field.type_reference);
    let literal_type = crate::landed_integer_literal_type_reference(program, bound.literal);
    let operands = if side_selects_field(program, binary.left, field.symbol) {
        [field_type, literal_type]
    } else {
        [literal_type, field_type]
    };
    typed_trees::operator::has_builtin_spelled_expression_meaning(
        program,
        owner,
        bound.conjunct,
        spelling,
        &operands,
    )
}

/// Retiring a declaration predicate requires equivalence, not merely a useful
/// consequence. The shared checked predicate explorer proves both directions
/// over complete formal carriers before these exact own-field actuals can
/// describe the declaration's full Cartesian envelope. Unsupported clauses
/// leave the entire declaration outside interval-only representation.
fn exact_predicate_field_intervals(
    program: &dyn crate::CheckingInput,
    fields: &[&DataField],
    owner: symbols::SymbolHandle,
    expression: ExpressionHandle,
) -> Option<Vec<(symbols::SymbolHandle, Interval)>> {
    let application = crate::proof_contracts::collection_predicates::predicate_application(
        program, owner, expression,
    )?;
    let rows = crate::proof_contracts::collection_predicates::exact_predicate_intervals(
        program, owner, expression,
    )?;
    let entry = program
        .machine_states(&program.machines()[application.machine])
        .first()?;
    let parameters = program.state_parameters(entry);
    if rows.len() != parameters.len() || rows.len() != application.arguments.len() {
        return None;
    }
    rows.into_iter()
        .zip(parameters)
        .zip(application.arguments)
        .map(|((row, parameter), actual)| {
            if row.extent || row.argument != *actual {
                return None;
            }
            let path = program.expression_table.name_path_node(*actual)?;
            let field = fields
                .iter()
                .find(|field| field.symbol.is_valid() && field.symbol == path.symbol)?;
            let primitive = program.primitive_type_reference(field.type_reference)?;
            if crate::proof_contracts::collection_predicates::bare_integer_parameter_type(
                program,
                parameter.type_reference,
            ) != Some(primitive)
            {
                return None;
            }
            Some((
                field.symbol,
                Interval {
                    low: Some(i64::try_from(row.low).ok()?),
                    high: Some(i64::try_from(row.high).ok()?),
                },
            ))
        })
        .collect()
}

/// One conjunct of interval-only declaration facts, normalized so the field
/// reads on the left: `field <operator> literal`. `literal` is the authored
/// integer or float literal; each consumer reads it at the field's own carrier.
/// `conjunct` is the authored comparison, kept so a float consumer can confirm
/// the comparison selects the builtin or sealed IEEE meaning.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct FieldLiteralBound {
    pub(crate) field: symbols::SymbolHandle,
    pub(crate) operator: BinaryOperator,
    pub(crate) literal: ExpressionHandle,
    pub(crate) conjunct: ExpressionHandle,
}

/// Complete field bounds for representation validity. Integer rows carry the
/// shared, independently equivalent predicate envelope; IEEE rows retain the
/// authored comparison for its exact carrier/selected-meaning judgment.
#[derive(Debug, Clone, Copy)]
pub(crate) enum FieldRepresentationBound {
    Integer {
        field: symbols::SymbolHandle,
        minimum: Option<i64>,
        maximum: Option<i64>,
        conjunct: ExpressionHandle,
    },
    FloatLiteral(FieldLiteralBound),
}

impl FieldRepresentationBound {
    pub(crate) fn field(self) -> symbols::SymbolHandle {
        match self {
            Self::Integer { field, .. } => field,
            Self::FloatLiteral(bound) => bound.field,
        }
    }

    pub(crate) fn conjunct(self) -> ExpressionHandle {
        match self {
            Self::Integer { conjunct, .. } => conjunct,
            Self::FloatLiteral(bound) => bound.conjunct,
        }
    }
}

/// Normalize the whole default domain before a representation consumer can
/// retire its predicates. Every conjunct must describe independent own-field
/// sets; a useful consequence of an unsupported stronger clause is insufficient.
pub(crate) fn data_where_field_representation_bounds(
    program: &dyn crate::CheckingInput,
    definition: &DataDefinition,
) -> Option<Vec<FieldRepresentationBound>> {
    use typed_trees::types::PrimitiveType;

    let fields = declared_data_fields(program, definition);
    let mut conjuncts = program
        .proof_facts
        .span_or_empty(definition.where_facts)
        .iter()
        .map(|fact| fact.as_expression().copied())
        .collect::<Option<Vec<_>>>()?;
    let mut normalized = Vec::new();
    while let Some(conjunct) = conjuncts.pop() {
        if let Some(binary) = program.expression_table.binary_node(conjunct)
            && binary.operator == BinaryOperator::And
        {
            // Conjunction is sealed; no authored implementation can replace it.
            conjuncts.extend([binary.left, binary.right]);
            continue;
        }
        if let Some(bounds) =
            field_literal_bounds(program, &fields, &[ProofFact::Expression(conjunct)])
            && bounds.iter().all(|bound| {
                fields
                    .iter()
                    .find(|field| field.symbol == bound.field)
                    .and_then(|field| program.primitive_type_reference(field.type_reference))
                    .is_some_and(|primitive| {
                        matches!(primitive, PrimitiveType::F32 | PrimitiveType::F64)
                    })
            })
        {
            normalized.extend(
                bounds
                    .into_iter()
                    .map(FieldRepresentationBound::FloatLiteral),
            );
            continue;
        }
        for (field, interval) in
            conjunct_field_intervals(program, &fields, definition.symbol, conjunct)?
        {
            let declaration = fields.iter().find(|candidate| candidate.symbol == field)?;
            if !program
                .primitive_type_reference(declaration.type_reference)?
                .accepts_integer_literal()
            {
                return None;
            }
            normalized.push(FieldRepresentationBound::Integer {
                field,
                minimum: interval.low,
                maximum: interval.high,
                conjunct,
            });
        }
    }
    Some(normalized)
}

fn field_literal_bounds(
    program: &dyn crate::CheckingInput,
    fields: &[&DataField],
    facts: &[ProofFact],
) -> Option<Vec<FieldLiteralBound>> {
    let mut bounds = Vec::new();
    let mut conjuncts = Vec::new();
    for fact in facts {
        let expression = fact.as_expression()?;
        conjuncts.push(*expression);
    }
    while let Some(conjunct) = conjuncts.pop() {
        let Some(binary) = program.expression_table.binary_node(conjunct) else {
            return None;
        };
        if binary.operator == BinaryOperator::And {
            conjuncts.extend([binary.left, binary.right]);
            continue;
        }
        let is_literal = |expression| {
            matches!(
                program.expression_table.expression(expression),
                ExpressionNode::Integer(_) | ExpressionNode::Float(_)
            )
        };
        let field_named = |expression| {
            fields
                .iter()
                .find(|field| side_selects_field(program, expression, field.symbol))
        };
        let (field, literal, field_on_left) = match (
            field_named(binary.left),
            is_literal(binary.right),
            field_named(binary.right),
            is_literal(binary.left),
        ) {
            (Some(field), true, _, _) => (field, binary.right, true),
            (_, _, Some(field), true) => (field, binary.left, false),
            _ => return None,
        };
        let operator = if field_on_left {
            binary.operator
        } else {
            binary.operator.mirrored()
        };
        if !matches!(
            operator,
            BinaryOperator::LessOrEqual
                | BinaryOperator::Less
                | BinaryOperator::GreaterOrEqual
                | BinaryOperator::Greater
                | BinaryOperator::Equal
        ) {
            return None;
        }
        bounds.push(FieldLiteralBound {
            field: field.symbol,
            operator,
            literal,
            conjunct,
        });
    }
    Some(bounds)
}

/// The interval a data declaration's literal `where` conjuncts state on `field`,
/// independent of whether the declaration also carries relational conjuncts.
pub(crate) fn field_literal_where_interval(
    program: &dyn crate::CheckingInput,
    definition: &DataDefinition,
    field: &str,
) -> Option<Interval> {
    field_fact_interval(program, definition, field, 3)
}

/// The interval a store to `target` must land in when the target is a field
/// (or an immutable local exclusive loan of a field) of data whose facts are
/// all interval-only: every store re-proves its own field's interval, which is
/// then the whole default domain for that field.
pub(crate) fn stored_field_where_interval(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    mut target: ExpressionHandle,
) -> Option<Interval> {
    for _ in 0..8 {
        if program.expression_table.member_node(target).is_some() {
            break;
        }
        let ExpressionNode::Name(path) = program.expression_table.expression(target) else {
            break;
        };
        let [name] = program.expression_table.name_path_members(path.members) else {
            break;
        };
        let Some(local) = program
            .statement_table
            .statements(state.statement_nodes)
            .iter()
            .filter_map(|statement| statement.as_local_data())
            .find(|local| !local.is_mutable && local.name == *name)
        else {
            break;
        };
        if let Some(borrow) = program
            .expression_table
            .borrow_expression(local.initial_value)
        {
            if !matches!(
                borrow.access,
                language_core::ReferenceAccess::Mutable | language_core::ReferenceAccess::WriteOnly
            ) {
                break;
            }
            target = borrow.target;
        } else {
            target = local.initial_value;
        }
    }
    let member = program.expression_table.member_node(target)?;
    let definition =
        data_definition_for_expression(program, machine, Some(state), member.receiver)?;
    let field = program
        .data_members(definition)
        .iter()
        .find_map(|candidate| match candidate {
            typed_trees::data::DataMember::Field(field) if field.name == member.member => {
                Some(field)
            }
            _ => None,
        })?;
    data_where_field_intervals(program, definition)?
        .into_iter()
        .find(|(symbol, _, _)| *symbol == field.symbol)
        .map(|(_, low, high)| Interval { low, high })
}

fn side_selects_field(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
    field: symbols::SymbolHandle,
) -> bool {
    field.is_valid()
        && program
            .expression_table
            .name_path_node(expression)
            .is_some_and(|path| path.symbol == field)
}

/// The bound-supplying side's SOUND interval: a literal is itself; a
/// co-field name reads its DECLARED range (or full type width) from the
/// data definition's own members.
fn bound_source_interval(
    program: &dyn crate::CheckingInput,
    definition: &DataDefinition,
    expression: ExpressionHandle,
    depth: usize,
) -> Option<Interval> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Integer(value) => {
            let literal = value
                .decimal_value_i128()
                .and_then(|value| i64::try_from(value).ok())?;
            Some(Interval {
                low: Some(literal),
                high: Some(literal),
            })
        }
        ExpressionNode::Name(path) => {
            let field = program
                .data_members(definition)
                .iter()
                .filter_map(|member| member.as_field())
                .find(|field| field.symbol.is_valid() && field.symbol == path.symbol)?;
            crate::proof_contracts::arithmetic_domains::range_constraint_interval(
                program,
                field.type_reference,
            )
            .or_else(|| field_fact_interval(program, definition, field.name.as_str(), depth + 1))
        }
        _ => None,
    }
}

/// Slice 10: is the definition's named field an UNSIGNED primitive (its
/// values are >= 0 -- the product-division soundness guard)?
fn field_is_unsigned(
    program: &dyn crate::CheckingInput,
    definition: &DataDefinition,
    field: symbols::SymbolHandle,
) -> bool {
    program
        .data_members(definition)
        .iter()
        .find_map(|member| match member {
            typed_trees::data::DataMember::Field(data_field)
                if data_field.symbol.is_valid() && data_field.symbol == field =>
            {
                program.primitive_type_reference(data_field.type_reference)
            }
            _ => None,
        })
        .is_some_and(|primitive| {
            matches!(
                primitive,
                typed_trees::unsigned_integer_primitive_pattern!()
            )
        })
}

/// Slice 10: a co-factor's LOWER bound -- its declared range, or a sibling
/// literal fact (`stride >= 40` / `40 <= stride`), single level.
fn factor_lower_bound(
    program: &dyn crate::CheckingInput,
    definition: &DataDefinition,
    factor: ExpressionHandle,
) -> Option<i64> {
    let path = program.expression_table.name_path_node(factor)?;
    let factor_symbol = program
        .data_members(definition)
        .iter()
        .filter_map(|member| member.as_field())
        .find(|field| field.symbol.is_valid() && field.symbol == path.symbol)?
        .symbol;
    if let Some(interval) = bound_source_interval(program, definition, factor, 4)
        && let Some(low) = interval.low
    {
        return Some(low);
    }
    for fact in program.proof_facts.span_or_empty(definition.where_facts) {
        let Some(expression) = fact.as_expression() else {
            continue;
        };
        let Some(binary) = program.expression_table.binary_node(*expression) else {
            continue;
        };
        let bound = match binary.operator {
            BinaryOperator::GreaterOrEqual
                if side_selects_field(program, binary.left, factor_symbol) =>
            {
                integer_literal_value(program, binary.right).map(|value| value as i64)
            }
            BinaryOperator::Greater if side_selects_field(program, binary.left, factor_symbol) => {
                integer_literal_value(program, binary.right)
                    .map(|value| (value as i64).saturating_add(1))
            }
            BinaryOperator::LessOrEqual
                if side_selects_field(program, binary.right, factor_symbol) =>
            {
                integer_literal_value(program, binary.left).map(|value| value as i64)
            }
            BinaryOperator::Less if side_selects_field(program, binary.right, factor_symbol) => {
                integer_literal_value(program, binary.left)
                    .map(|value| (value as i64).saturating_add(1))
            }
            _ => None,
        };
        if let Some(bound) = bound {
            return Some(bound);
        }
    }
    None
}
