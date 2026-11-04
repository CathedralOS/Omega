//! Complete default-domain interval contracts.
//!
//! Occurrence-independent reads and stores use an envelope only when it
//! accounts for every declared conjunct. Coupled parent restoration uses
//! captured current storage and the complete predicate instead.

use crate::frozen_program::expression_table;
use typed_trees::data::DataMember;
use typed_trees::data::{DataDefinition, DataField, DataVariant};
use typed_trees::domain::ProofFact;
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use typed_trees::machine::Machine;
use typed_trees::state::State;

use super::place_queries::data_definition_for_expression;
use crate::proof_contracts::arithmetic_domains::{BigIntInterval, Interval};

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
/// (`exhaustiveness::variant_is_impossible` in Psi 03), so a dispatch may
/// omit it without a `_` arm. Every later roster of the instance's cases —
/// the checked structural shape, the closed-sum terminator, the permission
/// schedule's run coverage, Psi 06's computed-case bindings — applies this
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
                match expression_table(program).expression(*expression) {
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
            DataMember::Field(field) => Some(field),
            DataMember::Variant(_) => None,
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
        if let Some(binary) = expression_table(program).binary_node(conjunct)
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
                let value = expression_table(program).integer_node(bound.literal)?;
                let bound_value = i64::try_from(value.decimal_value_i128()?).ok()?;
                let interval = BigIntInterval::from_comparison(
                    bound.operator,
                    true,
                    numerics::bignum::BigInt::from_i64(bound_value),
                )?;
                Some((bound.field, interval.to_interval()?))
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
    let Some(binary) = expression_table(program).binary_node(bound.conjunct) else {
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
            let path = expression_table(program).name_path_node(*actual)?;
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
        interval: Interval,
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
        if let Some(binary) = expression_table(program).binary_node(conjunct)
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
            normalized.push(FieldRepresentationBound::Integer { field, interval });
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
        let binary = expression_table(program).binary_node(conjunct)?;
        if binary.operator == BinaryOperator::And {
            conjuncts.extend([binary.left, binary.right]);
            continue;
        }
        let is_literal = |expression| {
            matches!(
                expression_table(program).expression(expression),
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

/// Another observer can read an atomic cell between updates, so every atomic
/// update must preserve its resident bound. Ordinary scalar fields retain their
/// own type restrictions; enclosing record predicates are checked separately.
pub(crate) fn atomic_field_interval(
    program: &dyn crate::CheckingInput,
    definition: &DataDefinition,
    field: &DataField,
) -> Option<Interval> {
    if crate::reference_shell(program, field.type_reference).is_some()
        || !crate::is_atomic_type(program, field.type_reference)
    {
        return None;
    }
    data_where_field_intervals(program, definition)?
        .into_iter()
        .find(|(symbol, _, _)| *symbol == field.symbol)
        .map(|(_, low, high)| Interval { low, high })
}

/// Read the declaration's interval for atomic or policy-governed write checks.
/// Ordinary stores may leave requirements unproved until restoration, so this
/// query supplies no proof about current contents.
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
        let ExpressionNode::Name(path) = expression_table(program).expression(target) else {
            break;
        };
        let [name] = expression_table(program).name_path_members(path.members) else {
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
        if let Some(borrow) = expression_table(program).borrow_expression(local.initial_value) {
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
    let member = expression_table(program).member_node(target)?;
    if crate::value_custody::recasts::pending_views::withholds_default_predicate(
        program,
        machine,
        state,
        member.receiver,
    ) {
        return None;
    }
    let definition =
        data_definition_for_expression(program, machine, Some(state), member.receiver)?;
    let field = program
        .data_members(definition)
        .iter()
        .find_map(|candidate| match candidate {
            DataMember::Field(field) if field.name == member.member => Some(field),
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
        && expression_table(program)
            .name_path_node(expression)
            .is_some_and(|path| path.symbol == field)
}
