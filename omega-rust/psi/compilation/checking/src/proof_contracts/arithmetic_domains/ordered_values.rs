//! Exact temporary operand identities for live ordered arithmetic facts.
//!
//! Field and index projections compose over the same operand tree; a field
//! does not require a name-only receiver. Paths are write-frame metadata only.
//! Equality uses resolved declarations and
//! the complete argument tree of an eligible normal-return call.
//! A match whose every arm denotes one ordered operand denotes that operand.
//! The numeric interval join at the match does not replace that equality:
//! a later guard must still reach the joined value, as it does for a copy.
//! Observed comparisons also retain consequences of finite integer carriers:
//! a ceiling may disappear at a state edge without losing its bound on a
//! surviving scalar. This is not repeated-call equality. Effects invalidate
//! mutable subjects; only exact immutable copied values survive unknown writes.
//! At a state edge, forgetting an undelivered operand must not forget the
//! consequences it connects between delivered operands. The same bounded
//! difference walk serves arithmetic queries and this scope projection; only
//! ordering facts compose, while nonconvex exclusions keep their own meaning.
use super::{
    ArithmeticDomain, BinaryOperator, ExpressionHandle, ExpressionNode, Interval, Machine,
    PrimitiveType, State, ValueEnvironment, declared_place_type_raw, place_path,
};
use crate::proof_contracts::arithmetic_domains::expression_analysis::analyze_meaning;
use crate::proof_contracts::arithmetic_domains::integer_ranges::{
    literal_interval, primitive_range,
};
use crate::proof_contracts::arithmetic_domains::place_paths::place_paths_overlap;
use crate::value_custody::places::collection_length_receiver;
use symbols::SymbolHandle;

mod congruence;
mod delivered;
mod requirements;
mod storage;
mod transport;
pub(crate) use delivered::delivered_value_orderings;
use delivered::{call_result_orderings, ensured_call_operand, match_subtract_floor};
pub use requirements::validate_ordered_requirement_call_totality;
pub(super) use transport::rebind_relations;

#[derive(Clone, Debug, PartialEq)]
pub(super) enum Operand {
    Place {
        root: SymbolHandle,
        path: String,
    },
    Field {
        receiver: Box<Operand>,
        field: SymbolHandle,
        name: String,
        primitive: Option<PrimitiveType>,
    },
    Integer(
        numerics::literals::IntegerLiteral,
        Option<numerics::literals::IntegerLanding>,
    ),
    Call {
        target: SymbolHandle,
        /// A valid handle names one operation in this evaluation. Only the
        /// repeatable denotation route leaves this invalid and uses argument
        /// congruence across distinct source calls.
        occurrence: ExpressionHandle,
        arguments: Vec<Operand>,
    },
    CollectionLength(Box<Operand>),
    Indexed {
        collection: Box<Operand>,
        index: Box<Operand>,
        primitive: Option<PrimitiveType>,
    },
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
            path: parameter.name.as_str().to_owned(),
        }
    }

    /// Only fixed field chains have a precise legacy write-frame label.
    /// Indexed observations keep their resolved selector but conservatively
    /// depend on the collection footprint until that owner supplies a leaf.
    pub(super) fn static_path(&self) -> Option<String> {
        match self {
            Self::Place { path, .. } => Some(path.clone()),
            Self::Field { receiver, name, .. } => {
                Some(format!("{}.{}", receiver.static_path()?, name))
            }
            _ => None,
        }
    }

    pub(super) fn survives(&self, written: &[String]) -> bool {
        self.survives_paths(written)
    }

    pub(super) fn is_immutable_snapshot(
        &self,
        program: &dyn crate::CheckingInput,
        state: &State,
    ) -> bool {
        match self {
            Self::Integer(..) => true,
            Self::Place { root, .. } => {
                if !root.is_valid() {
                    return false;
                }
                // Display paths are invalidation metadata, never declaration
                // authority. Only an exact immutable copied scalar survives an
                // unknown write; an immutable reference does not freeze storage.
                let parameters = program
                    .state_parameters(state)
                    .iter()
                    .filter_map(|parameter| {
                        (parameter.symbol == *root)
                            .then_some((!parameter.is_mutable, parameter.type_reference))
                    });
                let locals = program
                    .statement_table
                    .statements(state.statement_nodes)
                    .iter()
                    .filter_map(|statement| {
                        let local = statement.as_local_data()?;
                        (local.symbol == *root).then_some((!local.is_mutable, local.type_reference))
                    });
                let mut declarations = parameters.chain(locals);
                declarations.next().is_some_and(|(immutable, reference)| {
                    immutable && program.primitive_type_reference(reference).is_some()
                }) && declarations.next().is_none()
            }
            Self::Call {
                occurrence,
                arguments,
                ..
            } => {
                !occurrence.is_valid()
                    && arguments
                        .iter()
                        .all(|value| value.is_immutable_snapshot(program, state))
            }
            Self::Binary { operands, .. } => operands
                .iter()
                .all(|value| value.is_immutable_snapshot(program, state)),
            Self::CollectionLength(_) | Self::Indexed { .. } | Self::Field { .. } => false,
        }
    }

    fn survives_paths(&self, written: &[String]) -> bool {
        match self {
            Self::Place { path, .. } => {
                !written.iter().any(|write| place_paths_overlap(path, write))
            }
            Self::Field { receiver, .. } => self.static_path().map_or_else(
                || receiver.survives_paths(written),
                |path| {
                    !written
                        .iter()
                        .any(|write| place_paths_overlap(&path, write))
                },
            ),
            Self::Integer(..) => true,
            Self::Call {
                occurrence,
                arguments,
                ..
            } => {
                (!occurrence.is_valid() || written.is_empty())
                    && arguments.iter().all(|value| value.survives_paths(written))
            }
            Self::CollectionLength(collection) => collection.survives(written),
            Self::Indexed {
                collection, index, ..
            } => collection.survives(written) && index.survives(written),
            Self::Binary { operands, .. } => {
                operands.iter().all(|value| value.survives_paths(written))
            }
        }
    }

    pub(super) fn rebound(&self, bindings: &[(Operand, Operand)]) -> Vec<Self> {
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
            Self::Indexed {
                collection,
                index,
                primitive,
            } => {
                let collections = collection.rebound(bindings);
                let indices = index.rebound(bindings);
                collections
                    .into_iter()
                    .flat_map(|collection| {
                        indices.iter().map(move |index| Self::Indexed {
                            collection: Box::new(collection.clone()),
                            index: Box::new(index.clone()),
                            primitive: *primitive,
                        })
                    })
                    .collect()
            }
            Self::Place { .. } => Vec::new(),
            Self::Field {
                receiver,
                field,
                name,
                primitive,
            } => receiver
                .rebound(bindings)
                .into_iter()
                .map(|receiver| Self::Field {
                    receiver: Box::new(receiver),
                    field: *field,
                    name: name.clone(),
                    primitive: *primitive,
                })
                .collect(),
            // A raw runtime event is local to this evaluation. Only an exact
            // direct binding above can carry its completed value across a state
            // edge; argument rebinding cannot recreate another invocation.
            Self::Call { occurrence, .. } if occurrence.is_valid() => Vec::new(),
            Self::Call {
                target, arguments, ..
            } => {
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
                        occurrence: ExpressionHandle::invalid(),
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
    /// A constant comparison on the exact observed operand. Display paths are
    /// invalidation metadata, never enough to match a relation to a read.
    fn literal_comparisons<'relation>(
        &'relation self,
        subject: &'relation Operand,
        relations: &'relation [Relation],
    ) -> impl Iterator<Item = (i64, RelationConstraint, bool)> + 'relation {
        [
            (&self.left, &self.right, true),
            (&self.right, &self.left, false),
        ]
        .into_iter()
        .filter_map(move |(candidate, opposite, subject_on_left)| {
            if !congruence::same_value(relations, candidate, subject) {
                return None;
            }
            let Operand::Integer(literal, _) = opposite else {
                return None;
            };
            Some((literal.value_i64()?, self.constraint, subject_on_left))
        })
    }

    pub(super) fn minimum_difference(&self) -> Option<i64> {
        match self.constraint {
            RelationConstraint::MinimumDifference(floor) => Some(floor),
            RelationConstraint::Unequal => None,
        }
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

/// `operand` served by the resolver's once-computed whole-program call plans
/// when a live call-frame resolver reaches this site.
pub(super) fn operand(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
    frames: Option<&crate::CallFrameResolver<'_>>,
) -> Option<Operand> {
    build_operand(program, machine, state, expression, 0, frames)
}

fn build_operand(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
    depth: usize,
    frames: Option<&crate::CallFrameResolver<'_>>,
) -> Option<Operand> {
    if !expression.is_valid() || depth >= 128 {
        return None;
    }
    // A builtin whole view preserves coordinates. Resolve that geometry at
    // observation time, never by replaying a local reference initializer.
    if let Some(source) = crate::value_custody::places::whole_collection_view_source(
        program,
        machine,
        Some(state),
        expression,
    ) {
        return build_operand(program, machine, state, source, depth + 1, frames);
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
        ExpressionNode::Borrow(borrow) => {
            build_operand(program, machine, state, borrow.target, depth + 1, frames)
        }
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
            if !local && !parameter && !value_binder {
                return None;
            }
            Some(Operand::Place {
                root: path.symbol,
                path: place_path(program, expression)?,
            })
        }
        ExpressionNode::Member(member) => {
            let receiver =
                build_operand(program, machine, state, member.receiver, depth + 1, frames)?;
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
            } else if matches!(&receiver, Operand::Place { root, .. } if *root == machine.symbol) {
                crate::exact_self_field(program, machine, expression)?.symbol
            } else {
                crate::value_custody::places::declared_member_field_symbol(
                    program,
                    machine,
                    Some(state),
                    expression,
                )?
            };
            let reference = declared_place_type_raw(program, machine, Some(state), expression)?;
            if !field.is_valid() {
                return None;
            }
            Some(Operand::Field {
                receiver: Box::new(receiver),
                field,
                name: member.member.as_str().to_owned(),
                primitive: program.primitive_type_reference(reference),
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
                occurrence: ExpressionHandle::invalid(),
                arguments: arguments
                    .iter()
                    .map(|argument| {
                        build_operand(program, machine, state, *argument, depth + 1, frames)
                    })
                    .collect::<Option<Vec<_>>>()?,
            })
        }
        ExpressionNode::Indexed(indexed)
            if crate::place_has_builtin_coordinates(program, machine, Some(state), expression)
                && !matches!(
                    program.expression_table.expression(indexed.index),
                    ExpressionNode::Range(_)
                ) =>
        {
            // A collection and its selector jointly identify one observation.
            // Neither source spelling nor a remembered guard is its identity:
            // ordinary effect invalidation retires a write to either operand,
            // and state transport rebinds both resolved trees simultaneously.
            let index = build_operand(program, machine, state, indexed.index, depth + 1, frames)?;
            let primitive = declared_place_type_raw(program, machine, Some(state), expression)
                .and_then(|reference| program.primitive_type_reference(reference));
            indexed_operand(
                program,
                machine,
                state,
                indexed.collection,
                index,
                primitive,
                frames,
                depth + 1,
            )
        }
        ExpressionNode::Cast(cast)
            if !cast.form.is_recast()
                && cast.semantic_domain.is_empty()
                && cast.domain == ArithmeticDomain::Exact =>
        {
            // An admitted Exact integer conversion preserves its observed
            // value. Formation still owes source-to-destination fit; treating
            // it as an identity supplies no target-range premise. Qualified,
            // policy-changing, address and reference casts stay opaque.
            let source = integer_meaning(program, machine, state, cast.value)?.0;
            let target = program.primitive_type_reference(cast.target_type)?;
            if source == PrimitiveType::Addr
                || target == PrimitiveType::Addr
                || primitive_range(target).is_none()
            {
                return None;
            }
            build_operand(program, machine, state, cast.value, depth + 1, frames)
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
        ExpressionNode::Match(dispatch) => {
            // The subject is evaluated once and exactly one arm's value is the
            // result. Arms that denote one operand therefore denote the match,
            // including when a guard on that operand arrives after the join.
            // Distinct arm operands stay opaque: union of their ranges is a
            // different fact and lives in the arithmetic walk.
            let mut shared_operand = None;
            for arm in program.expression_table.match_arms(dispatch.arms) {
                let arm_operand =
                    build_operand(program, machine, state, arm.value, depth + 1, frames)?;
                match &shared_operand {
                    None => shared_operand = Some(arm_operand),
                    Some(current) if current == &arm_operand => {}
                    Some(_) => return None,
                }
            }
            shared_operand
        }
        _ => None,
    }
}

fn integer_meaning(
    program: &dyn crate::CheckingInput,
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
    let analysis = analyze_meaning(
        program,
        machine,
        Some(state),
        expression,
        "ordered operand type",
    )
    .value;
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

/// One element observation, whether read in source or instantiated from a
/// public contract. Builtin windows translate the ordinal in the same operand
/// namespace; they do not add a dimension or confer a fixed array extent.
pub(super) fn indexed_operand(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    collection: ExpressionHandle,
    index: Operand,
    primitive: Option<PrimitiveType>,
    frames: Option<&crate::CallFrameResolver<'_>>,
    depth: usize,
) -> Option<Operand> {
    if depth >= 128
        || !crate::place_has_builtin_coordinates(program, machine, Some(state), collection)
    {
        return None;
    }
    if let ExpressionNode::Borrow(borrow) = program.expression_table.expression(collection) {
        return indexed_operand(
            program,
            machine,
            state,
            borrow.target,
            index,
            primitive,
            frames,
            depth + 1,
        );
    }
    if let Some(window) = crate::value_custody::places::subslice_expression(program, collection) {
        let window = program.expression_table.indexed_node(window)?;
        let range = program.expression_table.range_node(window.index)?;
        let start = if range.start.is_valid() {
            build_operand(program, machine, state, range.start, depth + 1, frames)?
        } else {
            Operand::Integer(numerics::literals::IntegerLiteral::from_value(0), None)
        };
        let index = match &start {
            Operand::Integer(literal, _) if literal.value_u64() == Some(0) => index,
            _ => Operand::Binary {
                operator: BinaryOperator::Add,
                primitive: PrimitiveType::U64,
                domain: ArithmeticDomain::Exact,
                operands: vec![start, index],
            },
        };
        return indexed_operand(
            program,
            machine,
            state,
            window.collection,
            index,
            primitive,
            frames,
            depth + 1,
        );
    }
    Some(Operand::Indexed {
        collection: Box::new(build_operand(
            program,
            machine,
            state,
            collection,
            depth + 1,
            frames,
        )?),
        index: Box::new(index),
        primitive,
    })
}

pub(super) fn record(
    program: &dyn crate::CheckingInput,
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
            operand(program, machine, state, left, None),
            operand(program, machine, state, right, None),
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
        let Some(value) = operand(program, machine, state, value_expression, None) else {
            continue;
        };
        // An observed ceiling need not have a repeatable identity. Its finite
        // carrier still bounds this observation, so retain that consequence
        // when a call temporary disappears at a state edge. This says nothing
        // about the value returned by any subsequent invocation. The subject
        // retains its exact identity and ordinary write invalidation.
        let carrier_ceiling =
            ceiling_meaning.and_then(|(primitive, _)| integer_carrier_ceiling(primitive));
        for ceiling in operand(program, machine, state, ceiling_expression, None)
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
    program: &dyn crate::CheckingInput,
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
    let Some(left) = operand(program, machine, state, left, None) else {
        return false;
    };
    left.static_path().is_some()
        && operand(program, machine, state, right, None).is_some_and(|right| left == right)
}

pub(super) fn subtract_floor(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: Option<&State>,
    environment: &ValueEnvironment,
    left: ExpressionHandle,
    right: ExpressionHandle,
) -> Option<i64> {
    subtract_floor_limited(program, machine, state, environment, left, right, 0)
}

/// `left - right` is at least this floor when a live ordering says so.
///
/// A match on either side keeps the floor every arm proves. A call operand
/// contributes the orderings its proved `ensures` states about that result.
/// Both are the same ordering query, not a separate subtraction recognizer.
pub(super) fn subtract_floor_limited(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: Option<&State>,
    environment: &ValueEnvironment,
    left: ExpressionHandle,
    right: ExpressionHandle,
    depth: usize,
) -> Option<i64> {
    if depth > 8 {
        return None;
    }
    let state = state?;
    if matches!(
        program.expression_table.expression(right),
        ExpressionNode::Match(_)
    ) {
        return match_subtract_floor(program, machine, state, environment, left, right, depth);
    }
    if matches!(
        program.expression_table.expression(left),
        ExpressionNode::Match(_)
    ) {
        // `match - right` is not the ordered floor of `left - match`. Ask
        // each left arm against the same right; the join takes the minimum.
        let ExpressionNode::Match(dispatch) = program.expression_table.expression(left) else {
            return None;
        };
        let arms = program.expression_table.match_arms(dispatch.arms);
        let arm_environments = super::dispatch_arm_environments(
            program,
            machine,
            Some(state),
            left,
            dispatch,
            environment,
        );
        if arms.is_empty() || arm_environments.len() != arms.len() {
            return None;
        }
        let mut floor = None;
        for (arm, arm_environment) in arms.iter().zip(&arm_environments) {
            let arm_floor = subtract_floor_limited(
                program,
                machine,
                Some(state),
                arm_environment,
                arm.value,
                right,
                depth + 1,
            )?;
            floor = Some(floor.map_or(arm_floor, |existing: i64| existing.min(arm_floor)));
        }
        return floor;
    }
    let left_named = operand(program, machine, state, left, None);
    let right_named = operand(program, machine, state, right, None);
    // Identical observations cancel. The ensures-call identity is not that
    // observation: it names one occurrence and must not equate two calls.
    if let (Some(left_named), Some(right_named)) = (&left_named, &right_named)
        && left_named == right_named
    {
        return Some(0);
    }
    let left_operand = left_named.or_else(|| ensured_call_operand(program, machine, state, left));
    let right_operand =
        right_named.or_else(|| ensured_call_operand(program, machine, state, right));
    let (Some(left_operand), Some(right_operand)) = (left_operand, right_operand) else {
        return None;
    };
    let call_orderings = call_result_orderings(program, machine, state, left)
        .into_iter()
        .chain(call_result_orderings(program, machine, state, right))
        .collect::<Vec<_>>();
    if environment.ordered_values.is_empty() && call_orderings.is_empty() {
        return None;
    }
    let relation_floor = |relations: &[Relation]| {
        relations
            .iter()
            .filter(|relation| {
                congruence::same_value(relations, &relation.left, &left_operand)
                    && congruence::same_value(relations, &relation.right, &right_operand)
            })
            .filter_map(Relation::minimum_difference)
            .max()
    };
    if call_orderings.is_empty() {
        return relation_floor(&environment.ordered_values);
    }
    let mut relations = environment.ordered_values.clone();
    relations.extend(call_orderings);
    relation_floor(&relations)
}

/// Query a live comparison without widening an unequal value into an interval
/// that includes the excluded point. No source guard is replayed here: joins,
/// edge substitution and writes already determined which relations survive.
pub(super) fn excludes_integer_value(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: Option<&State>,
    environment: &ValueEnvironment,
    expression: ExpressionHandle,
    value: i64,
) -> bool {
    if environment.ordered_values.is_empty() {
        return false;
    }
    let Some(subject) = state.and_then(|state| operand(program, machine, state, expression, None))
    else {
        return false;
    };
    environment.ordered_values.iter().any(|relation| {
        for (constant, constraint, subject_is_left) in
            relation.literal_comparisons(&subject, &environment.ordered_values)
        {
            let excludes = match constraint {
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

/// Project retained ordering facts onto a current place read. Boolean values
/// and state edges carry resolved relations, not the original guard's interval
/// cache. Arithmetic must consume those surviving observations too, otherwise
/// a count proof can succeed while the same operand's product still appears
/// unbounded. Only representable interval endpoints are projected; inequality
/// exclusions retain their nonconvex meaning with their existing query owner.
pub(super) fn refine_place_interval(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: Option<&State>,
    environment: &ValueEnvironment,
    expression: ExpressionHandle,
    mut interval: Interval,
) -> Interval {
    if environment.ordered_values.is_empty() {
        return interval;
    }
    let Some(subject) = state.and_then(|state| operand(program, machine, state, expression, None))
    else {
        return interval;
    };
    for relation in &environment.ordered_values {
        for (constant, constraint, subject_on_left) in
            relation.literal_comparisons(&subject, &environment.ordered_values)
        {
            let RelationConstraint::MinimumDifference(distance) = constraint else {
                continue;
            };
            // Widen before computing an endpoint. Overflow of the interval's
            // i64 window loses precision; it never fabricates a clamped bound.
            let endpoint = if subject_on_left {
                i128::from(constant) + i128::from(distance)
            } else {
                i128::from(constant) - i128::from(distance)
            };
            let Ok(endpoint) = i64::try_from(endpoint) else {
                continue;
            };
            interval = interval.intersect(if subject_on_left {
                Interval {
                    low: Some(endpoint),
                    high: None,
                }
            } else {
                Interval {
                    low: None,
                    high: Some(endpoint),
                }
            });
        }
    }
    interval
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
    program: &dyn crate::CheckingInput,
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
    // A positive gap requires a retained relation. Carrier-only analysis has
    // an empty relation environment; rebuilding its operand identity here
    // recursively repeats the same unavailable proof search. Zero increases
    // retain the ordinary operand-eligibility check below.
    if high > 0 && environment.ordered_values.is_empty() {
        return false;
    }
    let Some(value) = operand(program, machine, state, value, None) else {
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
    program: &dyn crate::CheckingInput,
    operand: &Operand,
) -> Option<Option<i64>> {
    match operand {
        Operand::Integer(literal, _) => Some(literal_interval(literal).high),
        Operand::Place { root, .. } => {
            let type_reference =
                crate::value_custody::places::declaration_type_reference(program, *root)?;
            let primitive = program.primitive_type_reference(type_reference)?;
            Some(primitive_range(primitive).and_then(|range| range.high))
        }
        Operand::Binary { primitive, .. } => {
            Some(primitive_range(*primitive).and_then(|range| range.high))
        }
        Operand::CollectionLength(_) => Some(None),
        Operand::Indexed { primitive, .. } | Operand::Field { primitive, .. } => {
            primitive.and_then(primitive_range).map(|range| range.high)
        }
        Operand::Call { .. } => None,
    }
}

/// Whether a composed ceiling operand bounds the value inside `ceiling`, the
/// result carrier's representable high. A u64-scale ceiling (`None`) accepts
/// every carrier because no admitted integer carrier exceeds it; a narrower
/// ceiling admits only an operand whose carrier bound fits inside it.
fn operand_carrier_within_ceiling(
    program: &dyn crate::CheckingInput,
    _machine: &Machine,
    _state: &State,
    operand: &Operand,
    ceiling: Option<i64>,
) -> bool {
    let Some(ceiling) = ceiling else {
        return true;
    };
    matches!(
        operand_carrier_bound(program, operand),
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
/// count: in a consistent environment the strongest path needs no repeated
/// operand. Contradictory guards can contain positive cycles, but the bounded
/// walk still terminates and only publishes consequences of its actual links.
fn composed_ceiling_gap(
    relations: &[Relation],
    value: &Operand,
    needed: i64,
    admits: impl Fn(&Operand) -> bool,
) -> bool {
    if needed <= 0 {
        return true;
    }
    ordered_gaps(relations, value)
        .into_iter()
        .any(|(operand, gap)| gap >= needed && admits(operand))
}

/// Largest representable minimum differences reachable from one observation.
/// Edge projection and arithmetic queries share this bounded walk. A sum
/// outside i64 loses precision rather than clamping to a stronger premise.
fn ordered_gaps<'relations>(
    relations: &'relations [Relation],
    value: &'relations Operand,
) -> Vec<(&'relations Operand, i64)> {
    ordered_gaps_by(relations, value, |left, right| {
        congruence::same_value(relations, left, right)
    })
}

fn ordered_gaps_by<'relations>(
    relations: &'relations [Relation],
    value: &'relations Operand,
    same_value: impl Fn(&Operand, &Operand) -> bool,
) -> Vec<(&'relations Operand, i64)> {
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
            .find(|(known, _)| same_value(known, operand))
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
            let Some(gap) = gap.checked_add(floor) else {
                continue;
            };
            match dist
                .iter_mut()
                .find(|(known, _)| same_value(known, &relation.left))
            {
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
        }
        if !improved {
            break;
        }
    }
    dist
}
