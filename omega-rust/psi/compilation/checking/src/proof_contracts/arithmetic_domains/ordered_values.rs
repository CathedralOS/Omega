//! Exact temporary operand identities for live ordered arithmetic facts.
//!
//! Paths are write-frame metadata only. Equality uses resolved declarations and
//! the complete argument tree of an eligible normal-return call.
//! Observed comparisons also retain consequences of finite integer carriers:
//! a ceiling may disappear at a state edge without losing its bound on a
//! surviving scalar. This is not repeated-call equality. Effects invalidate
//! mutable subjects; only exact immutable copied values survive unknown writes.
use super::{
    ArithmeticDomain, BinaryOperator, ExpressionHandle, ExpressionNode, Interval, Machine,
    PrimitiveType, State, TypeReferenceNode, TypedTrees, ValueEnvironment, declared_place_type_raw,
    place_path,
};
use crate::proof_contracts::arithmetic_domains::expression_analysis::analyze;
use crate::proof_contracts::arithmetic_domains::integer_ranges::{
    literal_interval, primitive_range,
};
use crate::proof_contracts::arithmetic_domains::place_paths::place_paths_overlap;
use crate::value_custody::places::{collection_length_receiver, declared_member_path_type};
use symbols::SymbolHandle;

mod requirements;
pub use requirements::validate_ordered_requirement_call_totality;

#[derive(Clone, Debug, PartialEq)]
pub(super) enum Operand {
    Place {
        root: SymbolHandle,
        fields: Vec<SymbolHandle>,
        path: String,
    },
    Integer(
        numerics::literals::IntegerLiteral,
        Option<numerics::literals::IntegerLanding>,
    ),
    Call {
        target: SymbolHandle,
        arguments: Vec<Operand>,
    },
    CollectionLength(Box<Operand>),
    Binary {
        operator: BinaryOperator,
        primitive: PrimitiveType,
        domain: ArithmeticDomain,
        operands: Vec<Operand>,
    },
}

/// A relation between admitted fixed-integer observations. Every producer,
/// including projected case facts, must establish integer carrier meaning
/// before publication: a strict integer order has a gap of at least one,
/// whereas the same real or floating-point order does not.
#[derive(Clone, Debug)]
pub(super) struct Relation {
    pub(super) left: Operand,
    pub(super) right: Operand,
    pub(super) constraint: RelationConstraint,
}

impl PartialEq for Relation {
    fn eq(&self, other: &Self) -> bool {
        self.constraint == other.constraint
            && ((self.left == other.left && self.right == other.right)
                || (self.constraint == RelationConstraint::Unequal
                    && self.left == other.right
                    && self.right == other.left))
    }
}

/// Inequality is not an ordering: a signed value unequal to zero may be on
/// either side of it. Both facts share operand identity, edge transport and
/// mutation lifetime, but only a minimum difference grants an ordered bound.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum RelationConstraint {
    MinimumDifference(i64),
    Unequal,
}

impl Operand {
    pub(super) fn parameter(parameter: &typed_trees::signature::StateParameter) -> Self {
        Self::Place {
            root: parameter.symbol,
            fields: Vec::new(),
            path: parameter.name.as_str().to_owned(),
        }
    }

    pub(super) fn survives(&self, written: &[String]) -> bool {
        self.survives_preserving_length(written, None)
    }

    pub(super) fn is_immutable_snapshot(&self, program: &TypedTrees, state: &State) -> bool {
        match self {
            Self::Integer(..) => true,
            Self::Place { root, fields, .. } => {
                if !root.is_valid() || !fields.is_empty() {
                    return false;
                }
                // Display paths are invalidation metadata, never declaration
                // authority. Only an exact immutable copied scalar survives an
                // unknown write; an immutable reference does not freeze storage.
                let parameters = program
                    .state_parameters(state)
                    .iter()
                    .filter_map(|parameter| {
                        (parameter.symbol == *root).then_some((
                            !parameter.is_mutable && !parameter.is_self,
                            parameter.type_reference,
                        ))
                    });
                let locals = program
                    .statement_table
                    .statements(state.statement_nodes)
                    .iter()
                    .filter_map(|statement| {
                        let typed_trees::statement::StatementNode::LocalData(local) = statement
                        else {
                            return None;
                        };
                        (local.symbol == *root).then_some((!local.is_mutable, local.type_reference))
                    });
                let mut declarations = parameters.chain(locals);
                declarations.next().is_some_and(|(immutable, reference)| {
                    immutable && program.primitive_type_reference(reference).is_some()
                }) && declarations.next().is_none()
            }
            Self::Call { arguments, .. } => arguments
                .iter()
                .all(|value| value.is_immutable_snapshot(program, state)),
            Self::Binary { operands, .. } => operands
                .iter()
                .all(|value| value.is_immutable_snapshot(program, state)),
            Self::CollectionLength(_) => false,
        }
    }

    fn survives_preserving_length(&self, written: &[String], preserved: Option<&Operand>) -> bool {
        match self {
            Self::Place { path, .. } => {
                !written.iter().any(|write| place_paths_overlap(path, write))
            }
            Self::Integer(..) => true,
            Self::Call { arguments, .. } => arguments
                .iter()
                .all(|value| value.survives_preserving_length(written, preserved)),
            Self::CollectionLength(collection) => {
                preserved == Some(collection.as_ref()) || collection.survives(written)
            }
            Self::Binary { operands, .. } => operands
                .iter()
                .all(|value| value.survives_preserving_length(written, preserved)),
        }
    }

    fn rebound(&self, bindings: &[(Operand, Operand)]) -> Vec<Self> {
        let direct = bindings
            .iter()
            .filter(|(source, _)| source == self)
            .map(|(_, target)| target.clone())
            .collect::<Vec<_>>();
        if !direct.is_empty() {
            return direct;
        }
        match self {
            Self::Integer(..) => vec![self.clone()],
            Self::CollectionLength(collection) => collection
                .rebound(bindings)
                .into_iter()
                .map(|collection| Self::CollectionLength(Box::new(collection)))
                .collect(),
            Self::Place { root, fields, path } => bindings
                .iter()
                .filter_map(|(source, target)| {
                    let Self::Place {
                        root: source_root,
                        fields: source_fields,
                        path: source_path,
                    } = source
                    else {
                        return None;
                    };
                    let Self::Place {
                        root: target_root,
                        fields: target_fields,
                        path: target_path,
                    } = target
                    else {
                        return None;
                    };
                    if root != source_root || !fields.starts_with(source_fields) {
                        return None;
                    }
                    let suffix = path.strip_prefix(source_path)?.strip_prefix('.')?;
                    let mut projected = target_fields.clone();
                    projected.extend_from_slice(&fields[source_fields.len()..]);
                    Some(Self::Place {
                        root: *target_root,
                        fields: projected,
                        path: format!("{target_path}.{suffix}"),
                    })
                })
                .collect(),
            Self::Call { target, arguments } => {
                let mut combinations = vec![Vec::new()];
                for argument in arguments {
                    let alternatives = argument.rebound(bindings);
                    combinations = combinations
                        .into_iter()
                        .flat_map(|prefix| {
                            alternatives.iter().map(move |argument| {
                                let mut values = prefix.clone();
                                values.push(argument.clone());
                                values
                            })
                        })
                        .collect();
                }
                combinations
                    .into_iter()
                    .map(|arguments| Self::Call {
                        target: *target,
                        arguments,
                    })
                    .collect()
            }
            Self::Binary {
                operator,
                primitive,
                domain,
                operands,
            } => {
                let [left, right] = operands.as_slice() else {
                    return Vec::new();
                };
                left.rebound(bindings)
                    .into_iter()
                    .flat_map(|left| {
                        right
                            .rebound(bindings)
                            .into_iter()
                            .map(move |right| Self::Binary {
                                operator: *operator,
                                primitive: *primitive,
                                domain: *domain,
                                operands: vec![left.clone(), right],
                            })
                    })
                    .collect()
            }
        }
    }
}

impl Relation {
    pub(super) fn minimum_difference(&self) -> Option<i64> {
        match self.constraint {
            RelationConstraint::MinimumDifference(floor) => Some(floor),
            RelationConstraint::Unequal => None,
        }
    }

    pub(super) fn survives_byte_store(&self, written: &[String], collection: &Operand) -> bool {
        self.left
            .survives_preserving_length(written, Some(collection))
            && self
                .right
                .survives_preserving_length(written, Some(collection))
    }
    pub(super) fn survives(&self, written: &[String]) -> bool {
        self.left.survives(written) && self.right.survives(written)
    }

    pub(super) fn rebound(&self, bindings: &[(Operand, Operand)]) -> Vec<Self> {
        self.left
            .rebound(bindings)
            .into_iter()
            .flat_map(|left| {
                self.right
                    .rebound(bindings)
                    .into_iter()
                    .map(move |right| Self {
                        left: left.clone(),
                        right,
                        constraint: self.constraint,
                    })
            })
            .collect()
    }
}

/// Only a builtin element store through an exact mutable byte parameter leaves
/// this view's extent unchanged. Callers cross all operand effects first.
pub(super) fn byte_store_collection(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    target: ExpressionHandle,
) -> Option<Operand> {
    if !crate::place_has_builtin_coordinates(program, machine, Some(state), target) {
        return None;
    }
    let ExpressionNode::Indexed(indexed) = program.expression_table.expression(target) else {
        return None;
    };
    let ExpressionNode::Name(name) = program.expression_table.expression(indexed.collection) else {
        return None;
    };
    let parameter = program
        .state_parameters(state)
        .iter()
        .find(|parameter| parameter.symbol == name.symbol)?;
    let TypeReferenceNode::Reference {
        referee,
        access: language_core::ReferenceAccess::Mutable,
        ..
    } = program
        .type_reference_table
        .type_reference(parameter.type_reference)
    else {
        return None;
    };
    let TypeReferenceNode::Slice { element_type } =
        program.type_reference_table.type_reference(*referee)
    else {
        return None;
    };
    if !matches!(
        program.type_reference_table.type_reference(*element_type),
        TypeReferenceNode::Named { .. }
    ) || program.primitive_type_reference(*element_type) != Some(PrimitiveType::U8)
        || matches!(
            program.expression_table.expression(indexed.index),
            ExpressionNode::Range(_)
        )
    {
        return None;
    }
    operand(program, machine, state, indexed.collection)
}

pub(super) fn operand(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
) -> Option<Operand> {
    operand_with_frames(program, machine, state, expression, None)
}

/// `operand` served by the resolver's once-computed whole-program call plans
/// when a live call-frame resolver reaches this site.
pub(super) fn operand_with_frames(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
    frames: Option<&crate::CallFrameResolver<'_>>,
) -> Option<Operand> {
    build_operand(program, machine, state, expression, 0, frames)
}

fn build_operand(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
    depth: usize,
    frames: Option<&crate::CallFrameResolver<'_>>,
) -> Option<Operand> {
    if !expression.is_valid() || depth >= 128 {
        return None;
    }
    if let Some(collection) = collection_length_receiver(program, machine, Some(state), expression)
    {
        return Some(Operand::CollectionLength(Box::new(build_operand(
            program,
            machine,
            state,
            collection,
            depth + 1,
            frames,
        )?)));
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Integer(literal) => {
            Some(Operand::Integer(literal.clone(), literal.landing()))
        }
        ExpressionNode::Name(path) => {
            if !path.symbol.is_valid() || path.symbol != path.head_symbol {
                return None;
            }
            let parameters = program.state_parameters(state);
            let local = program
                .state_local_declaration(state, path.symbol)
                .is_some();
            let parameter = parameters
                .iter()
                .any(|parameter| parameter.symbol == path.symbol);
            let attached = path.symbol == machine.symbol
                && parameters.iter().any(|parameter| parameter.is_self);
            let value_binder = program
                .machine_type_parameters(machine)
                .iter()
                .any(|parameter| {
                    parameter.symbol == path.symbol
                        && matches!(
                            parameter.kind,
                            typed_trees::data::TypeParameterKind::Value { .. }
                                | typed_trees::data::TypeParameterKind::Const { .. }
                        )
                });
            if !local && !parameter && !attached && !value_binder {
                return None;
            }
            Some(Operand::Place {
                root: path.symbol,
                fields: Vec::new(),
                path: place_path(program, expression)?,
            })
        }
        ExpressionNode::Member(member) => {
            let Operand::Place {
                root, mut fields, ..
            } = build_operand(program, machine, state, member.receiver, depth + 1, frames)?
            else {
                return None;
            };
            // A case-tagged projection resolves its payload field under that
            // exact variant on the receiver's declared data (destructure-arm
            // reads). Untagged members keep the ordinary field lookup.
            let field = if member.case_variant.is_some() {
                crate::value_custody::places::declared_case_projection_field(
                    program,
                    machine,
                    Some(state),
                    expression,
                )?
                .symbol
            } else if root == machine.symbol {
                crate::exact_self_field(program, machine, expression)?.symbol
            } else {
                crate::value_custody::places::declared_member_field_symbol(
                    program,
                    machine,
                    Some(state),
                    expression,
                )?
            };
            if !field.is_valid()
                || declared_place_type_raw(program, machine, Some(state), expression).is_none()
            {
                return None;
            }
            fields.push(field);
            Some(Operand::Place {
                root,
                fields,
                path: place_path(program, expression)?,
            })
        }
        ExpressionNode::Call(call) => {
            // Reject non-value calls before deriving whole-program summaries.
            // Shape eligibility grants no purity: the complete candidate check
            // below still consumes both summaries for every admitted call.
            crate::machine_calls::denotational_calls::plain_value_call_target(program, call)?;
            let plans = crate::operand_call_plans(program, frames);
            let (_, entry) =
                crate::machine_calls::denotational_calls::normal_return_call_candidate(
                    program, call, &plans.0, &plans.1,
                )
                .ok()?;
            let arguments = program.expression_table.expression_handles(call.arguments);
            if arguments.len() != program.state_parameters(entry).len() {
                return None;
            }
            Some(Operand::Call {
                target: call.target_symbol,
                arguments: arguments
                    .iter()
                    .map(|argument| {
                        build_operand(program, machine, state, *argument, depth + 1, frames)
                    })
                    .collect::<Option<Vec<_>>>()?,
            })
        }
        ExpressionNode::Binary(binary)
            if crate::has_builtin_bound_expression_meaning(
                program,
                machine,
                Some(state),
                expression,
            ) =>
        {
            let (primitive, domain) = integer_meaning(program, machine, state, expression)?;
            Some(Operand::Binary {
                operator: binary.operator,
                primitive,
                domain,
                operands: vec![
                    build_operand(program, machine, state, binary.left, depth + 1, frames)?,
                    build_operand(program, machine, state, binary.right, depth + 1, frames)?,
                ],
            })
        }
        _ => None,
    }
}

fn integer_meaning(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
) -> Option<(PrimitiveType, ArithmeticDomain)> {
    if collection_length_receiver(program, machine, Some(state), expression).is_some() {
        return Some((PrimitiveType::U64, ArithmeticDomain::Exact));
    }
    // Reuse the arithmetic owner's operand-driven carrier/policy selection.
    // This query supplies no range proof; ordinary validation still owes all
    // formation diagnostics, and the empty environment contains no relations.
    let analysis = analyze(
        program,
        machine,
        Some(state),
        expression,
        &ValueEnvironment::new(),
        None,
        ArithmeticDomain::Exact,
        "ordered operand type",
        &mut Vec::new(),
    );
    // Ordinary call arithmetic deliberately leaves an unconstrained result
    // neutral. Ordering still knows the exact selected declaration's carrier;
    // querying it here does not grant repeat-call equality or a body-derived
    // range contract.
    let declared_call = match program.expression_table.expression(expression) {
        ExpressionNode::Call(call) => super::return_ranges::call_return_type(program, call),
        _ => None,
    };
    let primitive = analysis.primitive.or_else(|| {
        declared_call.and_then(|reference| program.primitive_type_reference(reference))
    })?;
    (primitive != PrimitiveType::Addr && primitive_range(primitive).is_some()).then_some((
        primitive,
        analysis
            .domain
            .or_else(|| {
                declared_call
                    .map(|reference| program.arithmetic_domain_for_type_reference(reference))
            })
            .unwrap_or(ArithmeticDomain::Exact),
    ))
}

pub(super) fn record(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    environment: &mut ValueEnvironment,
    comparison: &typed_trees::expression::TableBinaryExpression,
    positive: bool,
) {
    let unequal = matches!(
        (comparison.operator, positive),
        (BinaryOperator::NotEqual, true) | (BinaryOperator::Equal, false)
    );
    let (left, right, strict, equal) = match (comparison.operator, positive) {
        (BinaryOperator::GreaterOrEqual, true) | (BinaryOperator::Less, false) => {
            (comparison.left, comparison.right, false, false)
        }
        (BinaryOperator::Greater, true) | (BinaryOperator::LessOrEqual, false) => {
            (comparison.left, comparison.right, true, false)
        }
        (BinaryOperator::LessOrEqual, true) | (BinaryOperator::Greater, false) => {
            (comparison.right, comparison.left, false, false)
        }
        (BinaryOperator::Less, true) | (BinaryOperator::GreaterOrEqual, false) => {
            (comparison.right, comparison.left, true, false)
        }
        (BinaryOperator::Equal, true) | (BinaryOperator::NotEqual, false) => {
            (comparison.left, comparison.right, false, true)
        }
        (BinaryOperator::NotEqual, true) | (BinaryOperator::Equal, false) => {
            (comparison.left, comparison.right, false, false)
        }
        _ => return,
    };
    let left_meaning = integer_meaning(program, machine, state, left);
    let right_meaning = integer_meaning(program, machine, state, right);
    let bounded_integer = |expression, meaning: Option<(PrimitiveType, ArithmeticDomain)>| {
        meaning.is_some()
            || matches!(program.expression_table.expression(expression),
                ExpressionNode::Integer(literal)
                    if literal.landing().is_none()
                        && (literal.value_i64().is_some() || literal.value_bignum().and_then(|value| value.to_u64()).is_some()))
    };
    // An anonymous comparison literal has no carrier yet, but its exact value
    // can still be an ordered operand if it lies in the fixed-integer window.
    // Retain its anonymous identity: this does not stamp a guessed width.
    if !bounded_integer(left, left_meaning) || !bounded_integer(right, right_meaning) {
        return;
    }
    if unequal {
        let (Some(left), Some(right)) = (
            operand(program, machine, state, left),
            operand(program, machine, state, right),
        ) else {
            return;
        };
        let relation = Relation {
            left,
            right,
            constraint: RelationConstraint::Unequal,
        };
        if !environment.ordered_values.contains(&relation) {
            environment.ordered_values.push(relation);
        }
        return;
    }
    // Builtin integer equality contributes both non-strict orders. Keep them
    // in the same relation store as guards, rather than unifying symbol atoms:
    // a write to either operand must retire the equality before a later use.
    // The guard owner checks selected operator meaning before calling here;
    // bounded_integer above excludes float equality and its NaN semantics.
    for (ceiling_expression, ceiling_meaning, value_expression) in std::iter::once((
        left,
        left_meaning,
        right,
    ))
    .chain(equal.then_some((right, right_meaning, left)))
    {
        let Some(value) = operand(program, machine, state, value_expression) else {
            continue;
        };
        // An observed ceiling need not have a repeatable identity. Its finite
        // carrier still bounds this observation, so retain that consequence
        // when a call temporary disappears at a state edge. This says nothing
        // about the value returned by any subsequent invocation. The subject
        // retains its exact identity and ordinary write invalidation.
        let carrier_ceiling =
            ceiling_meaning.and_then(|(primitive, _)| integer_carrier_ceiling(primitive));
        for ceiling in operand(program, machine, state, ceiling_expression)
            .into_iter()
            .chain(carrier_ceiling)
        {
            let relation = Relation {
                left: ceiling,
                right: value.clone(),
                constraint: RelationConstraint::MinimumDifference(i64::from(strict)),
            };
            if !environment.ordered_values.contains(&relation) {
                environment.ordered_values.push(relation);
            }
        }
    }
}

fn integer_carrier_ceiling(primitive: PrimitiveType) -> Option<Operand> {
    use numerics::literals::{IntegerLiteral, IntegerRadix};

    let literal = if primitive == PrimitiveType::U64 {
        // The interval owner's i64 endpoint cannot represent this finite bound.
        IntegerLiteral::from_parts(false, IntegerRadix::Decimal, "18446744073709551615").ok()?
    } else {
        IntegerLiteral::from_value(primitive_range(primitive)?.high?)
    };
    Some(Operand::Integer(literal, None))
}

/// Two reads of the same resolved place at one subtraction have equal values.
/// This local identity needs no entry premise or arithmetic re-analysis.
pub(super) fn same_place(
    program: &TypedTrees,
    machine: &Machine,
    state: Option<&State>,
    left: ExpressionHandle,
    right: ExpressionHandle,
) -> bool {
    if ![left, right].into_iter().all(|expression| {
        matches!(
            program.expression_table.expression(expression),
            ExpressionNode::Name(_) | ExpressionNode::Member(_)
        )
    }) {
        return false;
    }
    let Some(state) = state else {
        return false;
    };
    let Some(left) = operand(program, machine, state, left) else {
        return false;
    };
    matches!(left, Operand::Place { .. })
        && operand(program, machine, state, right).is_some_and(|right| left == right)
}

pub(super) fn subtract_floor(
    program: &TypedTrees,
    machine: &Machine,
    state: Option<&State>,
    environment: &ValueEnvironment,
    left: ExpressionHandle,
    right: ExpressionHandle,
) -> Option<i64> {
    if environment.ordered_values.is_empty() {
        return None;
    }
    let state = state?;
    let left = operand(program, machine, state, left)?;
    let right = operand(program, machine, state, right)?;
    environment
        .ordered_values
        .iter()
        .filter(|relation| relation.left == left && relation.right == right)
        .filter_map(Relation::minimum_difference)
        .max()
}

/// Query a live comparison without widening an unequal value into an interval
/// that includes the excluded point. No source guard is replayed here: joins,
/// edge substitution and writes already determined which relations survive.
pub(super) fn excludes_integer_value(
    program: &TypedTrees,
    machine: &Machine,
    state: Option<&State>,
    environment: &ValueEnvironment,
    expression: ExpressionHandle,
    value: i64,
) -> bool {
    if environment.ordered_values.is_empty() {
        return false;
    }
    let Some(subject) = state.and_then(|state| operand(program, machine, state, expression)) else {
        return false;
    };
    environment.ordered_values.iter().any(|relation| {
        for (candidate, opposite, subject_is_left) in [
            (&relation.left, &relation.right, true),
            (&relation.right, &relation.left, false),
        ] {
            if *candidate != subject {
                continue;
            }
            let Operand::Integer(literal, _) = opposite else {
                continue;
            };
            let Some(constant) = literal.value_i64() else {
                continue;
            };
            let excludes = match relation.constraint {
                RelationConstraint::Unequal => constant == value,
                RelationConstraint::MinimumDifference(floor) if subject_is_left => {
                    i128::from(constant) + i128::from(floor) > i128::from(value)
                }
                RelationConstraint::MinimumDifference(floor) => {
                    i128::from(constant) - i128::from(floor) < i128::from(value)
                }
            };
            if excludes {
                return true;
            }
        }
        false
    })
}

/// A live bound `ceiling - value >= distance` proves that an unsigned
/// increment no larger than distance stays below that integer ceiling.
/// `ceiling` is the result carrier's representable high (`None` at u64 scale,
/// where every admitted fixed-width ceiling already qualifies). A narrower
/// result admits a composed ceiling operand only when that operand's own
/// carrier is provably bounded inside it: `count < cap` records
/// `cap >= count + 1`, and `cap`'s declared u32 carrier bounds the result by
/// u32::MAX -- the same transitivity an anonymous literal ceiling always had.
/// The relation's exact operand identity and ordinary write invalidation
/// remain authoritative.
pub(super) fn unsigned_increase_fits(
    program: &TypedTrees,
    machine: &Machine,
    state: Option<&State>,
    environment: &ValueEnvironment,
    value: ExpressionHandle,
    increase: Interval,
    ceiling: Option<i64>,
) -> bool {
    let (Some(state), Some(low), Some(high)) = (state, increase.low, increase.high) else {
        return false;
    };
    if low < 0 || high < low {
        return false;
    }
    let Some(value) = operand(program, machine, state, value) else {
        return false;
    };
    composed_ceiling_gap(&environment.ordered_values, &value, high, |operand| {
        operand_carrier_within_ceiling(program, machine, state, operand, ceiling)
    })
}

/// The upper bound an operand's own carrier already enforces. A literal is
/// its own bound; a place is bounded by its declared carrier's primitive
/// range; a binary operand carries its declared primitive; a collection
/// length is usize-bounded (unbounded at i64 resolution). `None` leaves a
/// call result's bound unknown -- its contract evidence lives elsewhere.
fn operand_carrier_bound(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    operand: &Operand,
) -> Option<Option<i64>> {
    match operand {
        Operand::Integer(literal, _) => Some(literal_interval(literal).high),
        Operand::Place { path, .. } => {
            let members = path.split('.').map(str::to_owned).collect::<Vec<_>>();
            let type_reference =
                declared_member_path_type(program, machine, Some(state), &members)?;
            let primitive = program.primitive_type_reference(type_reference)?;
            Some(primitive_range(primitive).and_then(|range| range.high))
        }
        Operand::Binary { primitive, .. } => {
            Some(primitive_range(*primitive).and_then(|range| range.high))
        }
        Operand::CollectionLength(_) => Some(None),
        Operand::Call { .. } => None,
    }
}

/// Whether a composed ceiling operand bounds the value inside `ceiling`, the
/// result carrier's representable high. A u64-scale ceiling (`None`) accepts
/// every carrier because no admitted integer carrier exceeds it; a narrower
/// ceiling admits only an operand whose carrier bound fits inside it.
fn operand_carrier_within_ceiling(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    operand: &Operand,
    ceiling: Option<i64>,
) -> bool {
    let Some(ceiling) = ceiling else {
        return true;
    };
    matches!(
        operand_carrier_bound(program, machine, state, operand),
        Some(Some(bound)) if bound <= ceiling
    )
}

/// Whether recorded `left >= right + floor` relations compose a ceiling
/// `c >= value + gap` with `gap >= needed`. Distances add along a chain, so
/// strict and non-strict links mix exactly: `i <= outer <= limit < len`
/// yields `len >= i + 1`, while an all-non-strict chain stays at gap 0 and
/// cannot prove a positive increase.
///
/// `dist` tracks the largest derived gap per operand and each round relaxes
/// every relation once — `left >= right + floor` with `right >= value + gap`
/// derives `left >= value + gap + floor`. Rounds are bounded by the operand
/// count: a chain that needs more hops revisits an operand, and a cycle that
/// still adds distance (`x >= x + k`, `k > 0`) is a contradiction no live
/// environment can contain, so cyclic chases cannot form and the walk
/// always terminates.
fn composed_ceiling_gap(
    relations: &[Relation],
    value: &Operand,
    needed: i64,
    admits: impl Fn(&Operand) -> bool,
) -> bool {
    if needed <= 0 {
        return true;
    }
    let mut operands: Vec<&Operand> = vec![value];
    for relation in relations {
        for operand in [&relation.left, &relation.right] {
            if !operands.contains(&operand) {
                operands.push(operand);
            }
        }
    }
    let mut dist: Vec<(&Operand, i64)> = vec![(value, 0)];
    let known_gap = |dist: &[(&Operand, i64)], operand: &Operand| {
        dist.iter()
            .find(|(known, _)| **known == *operand)
            .map(|(_, gap)| *gap)
    };
    for _ in 1..operands.len() {
        let mut improved = false;
        for relation in relations {
            let Some(floor) = relation.minimum_difference() else {
                continue;
            };
            let Some(gap) = known_gap(&dist, &relation.right) else {
                continue;
            };
            let gap = gap.saturating_add(floor);
            match dist.iter_mut().find(|(known, _)| **known == relation.left) {
                Some((_, known)) if *known >= gap => {}
                Some((_, known)) => {
                    *known = gap;
                    improved = true;
                }
                None => {
                    dist.push((&relation.left, gap));
                    improved = true;
                }
            }
            if gap >= needed && admits(&relation.left) {
                return true;
            }
        }
        if !improved {
            break;
        }
    }
    false
}
