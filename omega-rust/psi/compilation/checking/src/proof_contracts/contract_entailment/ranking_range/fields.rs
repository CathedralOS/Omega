//! An exact field projection is a distinct arithmetic coordinate, never its
//! record. The projection may descend a nested path of exact declared record
//! fields and readable references; every step resolves against its declaration.
//! Resolution supplies a coordinate, not a frozen value. State and call edge
//! owners establish exact arrival equality and complete write-frame preservation
//! of its reference bindings and referent contents before consuming its bounds.
use super::coordinate::{Coordinate, CoordinateKind};
use super::identity_views::{MeasureBodyShape, measure_body_shape};
use super::{
    BTreeMap, BigInt, BinaryOperator, Comparison, Engine, ExpressionHandle, ExpressionNode,
    Machine, Polynomial, PrimitiveType, State, TypeReferenceNode, exact_integer_parameter,
};
use symbols::{SymbolHandle, SymbolKind};
use typed_trees::data::{DataField, DataMember};
use typed_trees::name::Identifier;
use typed_trees::signature::StateParameter;
use typed_trees::types::TypeReferenceHandle;

/// Whether `endpoint` is known to land inside its carrier without reading any
/// live edge hypothesis. An exact immutable u64 field read performs no
/// arithmetic; a bare exact integer formal is already a carrier value; any
/// other authored expression must land on declaration bounds alone, where a
/// mutable input contributes its store-enforced declared bounds at every
/// evaluation. An endpoint that fails this is not malformed -- it only owes
/// the edge judgment a flow-dependent formation proof under that edge's
/// installed hypotheses ([`endpoint_lands_under`]).
pub(super) fn endpoint_statically_formed(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    endpoint: ExpressionHandle,
) -> bool {
    if projected_type(program, state, endpoint).is_some_and(|reference| {
        exact_integer_parameter(program, reference) == Some(PrimitiveType::U64)
    }) {
        return true;
    }
    if parameter(program, state, endpoint).is_some_and(|parameter| {
        exact_integer_parameter(program, parameter.type_reference).is_some()
    }) {
        return true;
    }
    crate::proof_contracts::arithmetic_domains::declared_integer_expression_lands(
        program, machine, state, endpoint,
    )
}

/// Prove `endpoint` lands inside its exact integer carrier under the engine's
/// already-installed hypotheses. `polynomial` is that endpoint normalized in
/// the edge's own namespace, so a requires or constrained-parameter fact that
/// bounds a leaf reaches the result. This is the arithmetic proof the
/// declaration-interval owner cannot see: a computed endpoint such as
/// `record.limit + 1` forms whenever the hypotheses bound `record.limit`, not
/// only when the field's store range does. The normalized polynomial alone
/// cannot carry that proof: algebraic cancellation hides an intermediate
/// operation that already escaped its carrier (`limit + u64::MAX - u64::MAX`
/// normalizes to `limit`), and a typed literal like `1u8` meets a `u64`
/// operand in a polynomial that records no carrier at all. Every operation
/// node therefore owes its own landing under the same hypotheses before the
/// whole endpoint is judged ([`operations_land_under`]).
pub(super) fn endpoint_lands_under(
    engine: &mut Engine<'_>,
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    endpoint: ExpressionHandle,
    polynomial: &Polynomial,
) -> bool {
    if let Some(Some(reference)) =
        super::meanings::admitted_carrier(program, machine, state, endpoint, 0)
        && exact_integer_parameter(program, reference).is_none()
    {
        return false;
    }
    integer_expression_lands_under(engine, program, machine, state, endpoint, polynomial)
}

/// A completed integer value retains its carrier independently of its policy.
/// Strict normalization admits each selected operation before reaching here;
/// this shared formation walk still checks every intermediate. Endpoints add
/// their separate Exact restriction, whereas a shift count may use any policy.
fn integer_expression_lands_under(
    engine: &mut Engine<'_>,
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    endpoint: ExpressionHandle,
    polynomial: &Polynomial,
) -> bool {
    if !operations_land_under(engine, program, machine, state, endpoint, None) {
        return false;
    }
    let carrier = match super::meanings::admitted_carrier(program, machine, state, endpoint, 0) {
        // An anonymous endpoint is a closed value and already landed. A
        // typed-literal operation such as `255u8 + 1u8` reaches here too;
        // `operations_land_under` has already judged its own carrier.
        Some(None) => return true,
        Some(Some(reference)) => reference,
        // A non-builtin endpoint never reaches normalization; the template
        // admission rejected it before this judgment ran.
        None => return false,
    };
    let Some(primitive) = super::integer_parameter(program, carrier) else {
        return false;
    };
    integer_value_lands(engine, primitive, polynomial)
}

/// Arrival equality of a normalized endpoint does not preserve formation:
/// `cap / divisor - cap / divisor` loses both divisions. Replay their
/// independent obligations under the exact simultaneous argument map before
/// accepting a new state, without importing destination requirements.
pub(super) fn endpoint_operations_land_after_arrival(
    engine: &mut Engine<'_>,
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    endpoint: ExpressionHandle,
    arrival: &BTreeMap<String, Polynomial>,
) -> bool {
    operations_land_under(engine, program, machine, state, endpoint, Some(arrival))
}

fn operation_value(
    engine: &mut Engine<'_>,
    expression: ExpressionHandle,
    arrival: Option<&BTreeMap<String, Polynomial>>,
) -> Option<Polynomial> {
    let value = engine.normalize(expression)?;
    match arrival {
        // Transport before consuming source equalities; otherwise a current
        // `divisor == 1` could hide the destination's different divisor.
        Some(arrival) => super::inductive_judgment::apply_argument_map(&value, arrival),
        None => Some(value),
    }
}

/// Every runtime arithmetic node inside `node` lands in the carrier its
/// operands select, under the engine's installed hypotheses. A closed subtree
/// is a folded constant whose landing the enclosing operation judges; a leaf
/// or an anonymous-natural node (a builtin collection coordinate, a
/// literal-only operation, proof-integer division) selects no primitive and
/// defers to its typed uses. For a node whose operands do share an exact
/// primitive, each anonymous operand's value must land in that carrier and
/// the node's own normalized result must land in it too -- the result proof
/// sees `x - x` as `0` within one operation, while an overflowed `limit +
/// u64::MAX` cannot launder through a later `- u64::MAX` because that earlier
/// node's own landing was never proved. Two known operand primitives must
/// agree; a `u8` literal next to a `u64` operand selects no shared carrier.
fn operations_land_under(
    engine: &mut Engine<'_>,
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    node: ExpressionHandle,
    arrival: Option<&BTreeMap<String, Polynomial>>,
) -> bool {
    if program
        .closed_integer_value_in(node, machine.symbol)
        .is_some()
    {
        return true;
    }
    let binary = match program.expression_table.expression(node) {
        ExpressionNode::Atomic(atomic) => {
            return operations_land_under(engine, program, machine, state, atomic.value, arrival);
        }
        ExpressionNode::Binary(binary) if super::dependency_binary(binary.operator) => binary,
        _ => return true,
    };
    // Denotation and result representability alone cannot form division:
    // cancellation can erase its result, but cannot erase a zero divisor.
    if matches!(
        binary.operator,
        BinaryOperator::Divide | BinaryOperator::Modulo
    ) {
        let Some(divisor) = operation_value(engine, binary.right, arrival) else {
            return false;
        };
        let divisor = engine.substituted(&divisor);
        if !engine.prove_at_least(&divisor, &BigInt::from_i64(1))
            && !engine.prove_at_least(&Polynomial::default().sub(&divisor), &BigInt::from_i64(1))
        {
            return false;
        }
    }
    let left = operand_primitive(program, machine, state, binary.left);
    let right = operand_primitive(program, machine, state, binary.right);
    // A shift's count is an independent integer operand: it never
    // selects or disputes the result carrier, which is the shifted value's
    // own primitive.
    let shifting = matches!(
        binary.operator,
        BinaryOperator::ShiftLeft | BinaryOperator::ShiftRight
    );
    if !shifting
        && let (Some(left), Some(right)) = (left, right)
        && left != right
    {
        return false;
    }
    let primitive = if shifting { left } else { left.or(right) };
    if shifting {
        // F8's count ruling at an endpoint: an exact `value << count` or
        // `value >> count` owes a provably in-width count under the same
        // hypotheses, like a nonzero divisor. Result membership alone
        // cannot establish count validity, and an anonymous shifted value
        // selects no width at all.
        let Some(width) =
            primitive.and_then(crate::proof_contracts::arithmetic_domains::integer_bit_width)
        else {
            return false;
        };
        let Some(count) = operation_value(engine, binary.right, arrival) else {
            return false;
        };
        let count = engine.substituted(&count);
        if !engine.prove_at_least(&count, &BigInt::zero())
            || !engine.prove_at_least(
                &Polynomial::constant(BigInt::from_i64(width - 1)).sub(&count),
                &BigInt::zero(),
            )
        {
            return false;
        }
    }
    if let Some(primitive) = primitive {
        let Some((minimum, maximum)) = super::integer_carrier_bounds(primitive) else {
            return false;
        };
        // An anonymous operand carries no range of its own; its folded value
        // must land in the operation carrier directly. A typed operand is
        // already a value of the carrier the primitive agreement established.
        for operand in [binary.left, binary.right] {
            if let Some(value) = program.closed_integer_value_in(operand, machine.symbol)
                && value.primitive.is_none()
                && (value.value < minimum || value.value > maximum)
            {
                return false;
            }
        }
        // Exact signed remainder shares division's quotient-formation
        // obligation: MIN % -1 is invalid even though its remainder is zero.
        // A variable divisor must independently exclude -1 or the dividend
        // must exclude MIN; bounding only the remainder would miss overflow.
        if binary.operator == BinaryOperator::Modulo && minimum.is_negative() {
            let Some(divisor) = operation_value(engine, binary.right, arrival) else {
                return false;
            };
            let divisor = engine.substituted(&divisor);
            if !engine.prove_at_least(&divisor, &BigInt::from_i64(1))
                && !engine
                    .prove_at_least(&Polynomial::default().sub(&divisor), &BigInt::from_i64(2))
            {
                let Some(dividend) = operation_value(engine, binary.left, arrival) else {
                    return false;
                };
                let above_minimum =
                    engine.substituted(&dividend.sub(&Polynomial::constant(minimum.clone())));
                if !engine.prove_at_least(&above_minimum, &BigInt::from_i64(1)) {
                    return false;
                }
            }
        }
        let Some(result) = operation_value(engine, node, arrival) else {
            return false;
        };
        let lower = engine.substituted(&result.sub(&Polynomial::constant(minimum)));
        let upper = engine.substituted(&Polynomial::constant(maximum).sub(&result));
        if !engine.prove_at_least(&lower, &BigInt::zero())
            || !engine.prove_at_least(&upper, &BigInt::zero())
        {
            return false;
        }
    }
    operations_land_under(engine, program, machine, state, binary.left, arrival)
        && operations_land_under(engine, program, machine, state, binary.right, arrival)
}

/// The exact primitive `node`'s own value or selected operation carries. A
/// closed operand keeps its landing's carrier -- `1u8` is a `u8` even where a
/// bare literal would stay anonymous -- and an unclosed operand reads the
/// exact parameter of its builtin meaning. Anonymous-natural coordinates and
/// proof-level nodes carry none.
fn operand_primitive(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    node: ExpressionHandle,
) -> Option<PrimitiveType> {
    if let Some(value) = program.closed_integer_value_in(node, machine.symbol) {
        return value.primitive;
    }
    match super::meanings::admitted_carrier(program, machine, state, node, 0) {
        Some(Some(reference)) => exact_integer_parameter(program, reference),
        _ => None,
    }
}

/// One `u64` field reached from a state formal through an exact chain of
/// declared record fields: `parameter.steps[0]. ... .steps[n].field`.
/// The field-view produced coordinate: a [`coordinate::Coordinate`] whose
/// step owners keep readable-reference hops and whose leaf is an exact
/// integer field.
pub(super) type FieldCoordinate<'program> = Coordinate<'program>;

impl<'program> Coordinate<'program> {
    /// The produced rank of the declared field view `measure` applied to
    /// `subject`: the measure body's projection chain, re-resolved against
    /// the subject formal's own declaration rather than trusted from the view.
    pub(super) fn resolve_field(
        program: &'program dyn crate::CheckingInput,
        state: &State,
        subject: ExpressionHandle,
        measure: SymbolHandle,
    ) -> Option<Self> {
        let parameter = parameter(program, state, subject)?;
        let definition = program
            .measures()
            .iter()
            .find(|definition| measure.is_valid() && definition.symbol == measure)?;
        let MeasureBodyShape::FieldProjection {
            path, field_symbol, ..
        } = measure_body_shape(program, definition)?
        else {
            return None;
        };
        let chain = path
            .iter()
            .map(|step| step.field_symbol)
            .chain([field_symbol])
            .collect::<Vec<_>>();
        let coordinate = Self::for_parameter(program, parameter, &chain, CoordinateKind::Field)?;
        (exact_integer_parameter(program, coordinate.field.type_reference)
            == Some(PrimitiveType::U64))
        .then_some(coordinate)
    }

    /// The coordinate an authored member chain names: every step an exact
    /// declared field, by resolved symbol and spelling, of the record before
    /// it, rooted at a state formal. The leaf is an ordinary integer value;
    /// selected rank views and endpoint operations independently restrict
    /// arithmetic policy and prove their required nonnegative bounds.
    pub(super) fn resolve_field_projection(
        program: &'program dyn crate::CheckingInput,
        state: &State,
        expression: ExpressionHandle,
    ) -> Option<Self> {
        Self::resolve_member(program, state, expression, CoordinateKind::Integer)
    }

    pub(super) fn field_comparisons(&self, program: &dyn crate::CheckingInput) -> Vec<Comparison> {
        let mut comparisons = Vec::new();
        // Carrier membership is independent of an authored rank range. The
        // signed-window interval owner below retains narrower storage facts,
        // but cannot represent an unrestricted u64 ceiling. Keep the exact
        // declared carrier here so reconstructed field actuals can prove
        // every operation forms without assuming a smaller arrival value.
        if let Some(primitive) = super::integer_parameter(program, self.field.type_reference)
            && let Some((minimum, maximum)) = super::integer_carrier_bounds(primitive)
        {
            comparisons.extend([
                (
                    BinaryOperator::GreaterOrEqual,
                    self.value(),
                    Polynomial::constant(minimum),
                ),
                (
                    BinaryOperator::LessOrEqual,
                    self.value(),
                    Polynomial::constant(maximum),
                ),
            ]);
        }
        if let Some((minimum, maximum)) =
            crate::enforced_integer_type_bounds(program, self.field.type_reference)
        {
            comparisons.extend([
                (
                    BinaryOperator::GreaterOrEqual,
                    self.value(),
                    Polynomial::constant(BigInt::from_i64(minimum)),
                ),
                (
                    BinaryOperator::LessOrEqual,
                    self.value(),
                    Polynomial::constant(BigInt::from_i64(maximum)),
                ),
            ]);
        }
        // The leaf's owning declaration states `where` facts every write
        // re-proves: they bound each legal read the way a declared field
        // range does, so the coordinate carries them as hypotheses too.
        let owner = program.symbols.get(self.field.symbol).parent;
        if let Some(definition) =
            crate::declarations::data_queries::data_definition_by_symbol(program, owner)
            && let Some(intervals) =
                crate::proof_contracts::default_domains::data_where_field_intervals(
                    program, definition,
                )
            && let Some((_, low, high)) = intervals
                .iter()
                .find(|(symbol, ..)| *symbol == self.field.symbol)
        {
            if let Some(low) = low {
                comparisons.push((
                    BinaryOperator::GreaterOrEqual,
                    self.value(),
                    Polynomial::constant(BigInt::from_i64(*low)),
                ));
            }
            if let Some(high) = high {
                comparisons.push((
                    BinaryOperator::LessOrEqual,
                    self.value(),
                    Polynomial::constant(BigInt::from_i64(*high)),
                ));
            }
        }
        comparisons
    }

    /// The exact coordinate `expression` installs for this chain when the
    /// actual itself denotes a record carrier: this formal forwarded, a
    /// same-record sibling behind the destination's `&`, or a member-target
    /// `p.f` (possibly `&p.f`) whose projection prefix lands on this
    /// coordinate's root record so the chain resumes there. A literal or
    /// computed actual carries no coordinate; `actual` walks those field by
    /// field. Callers that keep a coordinate set may `include` the result so
    /// the carrier's declared field bounds become hypotheses.
    pub(super) fn field_actual_coordinate(
        &self,
        program: &'program dyn crate::CheckingInput,
        state: &State,
        expression: ExpressionHandle,
        arrival_borrowed: bool,
    ) -> Option<Self> {
        let mut current = expression;
        if arrival_borrowed
            && let ExpressionNode::Borrow(borrow) = program.expression_table.expression(current)
        {
            // `&x` and `&x.f` both arrive with the record their target
            // denotes; the carrier resolution below names the same coordinate
            // either way. An actual already behind a reference arrives as its
            // own carrier without a borrow node.
            current = borrow.target;
        }
        let (carrier, prefix) = rooted_carrier(program, state, current)?;
        self.through_carrier(program, carrier, &prefix)
    }

    /// Transport this exact declared field through the completed actual.
    /// Owned projections carry captured values; a reference boundary carries
    /// only its binding identity. The edge owner independently preserves the
    /// referent path against writes before using its current coordinate.
    /// The normal-return owner never projects a captured referent from a
    /// descriptor, and destination requirements supply no facts here.
    pub(super) fn field_actual(
        &self,
        program: &'program dyn crate::CheckingInput,
        machine: &Machine,
        state: &State,
        engine: &mut Engine<'_>,
        expression: ExpressionHandle,
    ) -> Option<Polynomial> {
        completed_scalar_actual(
            program,
            machine,
            state,
            engine,
            expression,
            &self.location(program).path,
        )
    }
}

/// A scalar observation uses the same completed-value identity whether it
/// occupies a whole actual or a declared field. The prefix/normal-return owner
/// supplies public equations; arithmetic still proves each observed operand's
/// formation under this edge's preserved hypotheses.
pub(super) fn completed_scalar_actual(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    engine: &mut Engine<'_>,
    expression: ExpressionHandle,
    path: &[facts::PlaceSegment],
) -> Option<Polynomial> {
    use super::super::completed_values::{
        completed_value_observations, expression_statement_index,
    };
    let statement_index = expression_statement_index(program, state, expression)?;
    let reference = crate::expression_result_type_reference(program, machine, state, expression)?;
    let mut projected_type = reference;
    let mut completed_path_length = path.len();
    for (index, segment) in path.iter().enumerate() {
        if crate::reference_shell(program, projected_type).is_some() {
            completed_path_length = index;
            break;
        }
        projected_type = crate::value_custody::places::project_type_reference_from_segments(
            program,
            projected_type,
            std::slice::from_ref(segment),
        )?;
    }
    let (completed_path, referent_path) = path.split_at(completed_path_length);
    // A completed borrow may also carry an observed referent value. Ask
    // for both evidence classes in one normal-return observation; never
    // turn descriptor identity into permission to extract that value.
    let mut demands = vec![path.to_vec()];
    if !referent_path.is_empty() {
        demands.push(completed_path.to_vec());
    }
    let projections = completed_value_observations(
        program,
        machine,
        state,
        statement_index,
        expression,
        reference,
        &demands,
    )?;
    for projection in projections {
        let suffix = if projection.path.as_slice() == path {
            &[][..]
        } else if projection.path == completed_path {
            referent_path
        } else {
            continue;
        };
        for source in projection.sources {
            if let Some(value) =
                completed_source_value(program, machine, state, engine, &source, suffix)
            {
                return Some(value);
            }
        }
    }
    None
}

/// Normalize the value described by completion, preserving each selected
/// operation's formation before algebra can erase an overflowing intermediate.
/// Descriptor suffixes belong only to direct parameter identity transport.
fn completed_source_value(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    engine: &mut Engine<'_>,
    source: &super::super::completed_values::CompletedValueSource,
    suffix: &[facts::PlaceSegment],
) -> Option<Polynomial> {
    use super::super::completed_values::CompletedValueSource;
    match source {
        CompletedValueSource::Parameter { symbol, path } => {
            let mut path = path.clone();
            path.extend_from_slice(suffix);
            if path.is_empty() {
                return engine.strict_symbol_value(*symbol);
            }
            let location = super::RankSubjectCoordinate {
                parameter: *symbol,
                path,
            };
            let coordinate =
                FieldCoordinate::at_coordinate(program, state, &location, CoordinateKind::Integer)?;
            engine
                .install_hypotheses(coordinate.field_comparisons(program))
                .then(|| coordinate.value())
        }
        CompletedValueSource::Expression(source) if suffix.is_empty() => {
            super::meanings::install_nonpolynomial_terms(
                program, machine, state, engine, *source, 0,
            )?;
            super::meanings::admitted_carrier(program, machine, state, *source, 0)?;
            let value = engine.normalize(*source)?;
            integer_expression_lands_under(engine, program, machine, state, *source, &value)
                .then_some(value)
        }
        CompletedValueSource::Integer(integer) if suffix.is_empty() => {
            Some(Polynomial::constant(integer.clone()))
        }
        CompletedValueSource::ScalarBinary {
            operator,
            meaning,
            operands,
        } if suffix.is_empty() => {
            if !Engine::integer_operation_meaning(*operator, *meaning) {
                return None;
            }
            let [left, right] = operands.as_ref();
            let left = completed_source_value(program, machine, state, engine, left, &[])?;
            let right = completed_source_value(program, machine, state, engine, right, &[])?;
            let value = engine.selected_integer_binary_value(
                *operator,
                *meaning,
                left.clone(),
                right.clone(),
            )?;
            if !engine.integer_operation_lands(
                *operator,
                meaning.map(|(primitive, _)| primitive),
                &left,
                &right,
                &value,
            ) {
                return None;
            }
            engine.install_integer_operation_bounds(*operator, &left, &right, &value);
            Some(value)
        }
        _ => None,
    }
}

fn integer_value_lands(engine: &Engine<'_>, primitive: PrimitiveType, value: &Polynomial) -> bool {
    engine.integer_value_lands(primitive, value)
}

/// The declared type of an exact member chain rooted at a state formal:
/// `parameter.a.b.field`, every step a resolved field of the record before it.
pub(super) fn projected_type(
    program: &dyn crate::CheckingInput,
    state: &State,
    expression: ExpressionHandle,
) -> Option<TypeReferenceHandle> {
    let (machine, _) = crate::find_state(program, state.symbol)?;
    let place = crate::canonical_place_from_expression_in_state(
        program,
        state.symbol,
        usize::MAX,
        expression,
    )?;
    let root = place.root.as_symbol()?;
    if let Some(parameter) = program.state_parameter_by_symbol(state, root) {
        if parameter.is_const {
            return None;
        }
    } else {
        // Local projection typing shares saved-value custody. This admits
        // captured record fields without replaying a later mutable carrier.
        super::saved_arguments::saved(program, state, root)?;
    }
    if place.segments.is_empty()
        || !place.segments.iter().all(|segment| match segment {
            facts::PlaceSegment::Field { symbol } => {
                symbol.is_valid() && program.any_field_owner(*symbol).is_some()
            }
            facts::PlaceSegment::Case { variant } => {
                variant.is_valid() && program.variant_owner(*variant).is_some()
            }
            _ => false,
        })
    {
        return None;
    }
    crate::value_custody::places::declared_place_type_raw(program, machine, Some(state), expression)
}

/// The member chain of `expression` from its root formal outward, each step
/// as `(resolved field symbol, spelling)`. Case projections are not fields.

/// The parts both produced coordinates resolve identically: the formal's
/// non-const record carrier, each intermediate chain step walked through
/// `step_owner`, the leaf admitted by `leaf_admits`, and the atom identity
/// `tag` + parameter symbol + chain. The coordinates differ only in what a
/// step traverses, which leaf produces the rank, and the identity prefix.
pub(super) struct MemberCoordinateParts<'program> {
    pub borrowed: bool,
    pub root: SymbolHandle,
    pub steps: Vec<&'program DataField>,
    pub field: &'program DataField,
    pub identity: String,
}

pub(super) fn member_coordinate_parts<'program>(
    program: &'program dyn crate::CheckingInput,
    parameter: &'program StateParameter,
    chain: &[SymbolHandle],
    step_owner: impl Fn(&'program dyn crate::CheckingInput, &'program DataField) -> Option<SymbolHandle>,
    leaf_admits: impl Fn(&'program dyn crate::CheckingInput, &'program DataField) -> bool,
    tag: &str,
) -> Option<MemberCoordinateParts<'program>> {
    if parameter.is_const {
        return None;
    }
    let (root, borrowed) = record_referent(program, parameter.type_reference)?;
    let (last, step_symbols) = chain.split_last()?;
    let mut owner = root;
    let mut steps = Vec::with_capacity(step_symbols.len());
    for symbol in step_symbols {
        let field = declared_field(program, owner, *symbol)?;
        steps.push(field);
        owner = step_owner(program, field)?;
    }
    let field = declared_field(program, owner, *last)?;
    if !leaf_admits(program, field) {
        return None;
    }
    let mut identity = format!("{tag}{:?}", parameter.symbol);
    for symbol in chain {
        identity.push_str(&format!(":{symbol:?}"));
    }
    Some(MemberCoordinateParts {
        borrowed,
        root,
        steps,
        field,
        identity,
    })
}

pub(super) fn member_chain<'program>(
    program: &'program dyn crate::CheckingInput,
    state: &State,
    expression: ExpressionHandle,
) -> Option<(
    &'program StateParameter,
    Vec<(SymbolHandle, &'program Identifier)>,
)> {
    if let Some(path) = program.expression_table.name_path_node(expression) {
        let members = program.expression_table.name_path_members(path.members);
        if members.len() > 1 {
            let place = crate::canonical_place_from_expression_in_state(
                program,
                state.symbol,
                usize::MAX,
                expression,
            )?;
            let parameter = program.state_parameter_by_symbol(state, place.root.as_symbol()?)?;
            if place.segments.len() != members.len() - 1 {
                return None;
            }
            let chain = place
                .segments
                .iter()
                .zip(&members[1..])
                .map(|(segment, name)| {
                    let facts::PlaceSegment::Field { symbol } = segment else {
                        return None;
                    };
                    symbol.is_valid().then_some((*symbol, name))
                })
                .collect::<Option<Vec<_>>>()?;
            return Some((parameter, chain));
        }
    }
    let mut chain = Vec::new();
    let mut cursor = expression;
    while let Some(member) = program.expression_table.member_node(cursor) {
        if !member.member_symbol.is_valid() || member.case_variant.is_some() || chain.len() >= 128 {
            return None;
        }
        chain.push((member.member_symbol, &member.member));
        cursor = member.receiver;
    }
    if chain.is_empty() {
        return None;
    }
    chain.reverse();
    if let Some(parameter) = parameter(program, state, cursor) {
        Some((parameter, chain))
    } else if cursor != expression
        && let Some((parameter, mut prefix)) = member_chain(program, state, cursor)
    {
        prefix.extend(chain);
        Some((parameter, prefix))
    } else {
        captured_carrier(program, state, expression)
    }
}

/// A coordinate survives only when the member chain's spelled hops name the
/// resolved fields in order — `steps` plus the leaf `field`.
pub(super) fn declared_spelling_verified<'program>(
    chain: &[(SymbolHandle, &'program Identifier)],
    steps: &[&'program DataField],
    field: &'program DataField,
) -> bool {
    chain
        .iter()
        .zip(steps.iter().chain([&field]))
        .all(|((_, spelled), field)| **spelled == field.name)
}

/// The formal `expression` is rooted at plus its resolved member chain: `x`
/// is `(x, [])`, `x.a.b` is `(x, [a, b])`. Any other shape is a computed
/// value with no carrier root.
pub(super) fn rooted_carrier<'program>(
    program: &'program dyn crate::CheckingInput,
    state: &State,
    expression: ExpressionHandle,
) -> Option<(
    &'program StateParameter,
    Vec<(SymbolHandle, &'program Identifier)>,
)> {
    if let Some(parameter) = parameter(program, state, expression) {
        return Some((parameter, Vec::new()));
    }
    member_chain(program, state, expression)
        .or_else(|| captured_carrier(program, state, expression))
}

/// A completed saved record or selected leaf retains the exact source
/// coordinate. Reconstruct its member chain from those declarations rather
/// than assigning a local's destination field spelling to a source sibling.
fn captured_carrier<'program>(
    program: &'program dyn crate::CheckingInput,
    state: &State,
    expression: ExpressionHandle,
) -> Option<(
    &'program StateParameter,
    Vec<(SymbolHandle, &'program Identifier)>,
)> {
    let coordinate = super::saved_arguments::captured_coordinate(program, state, expression)?;
    let parameter = program.state_parameter_by_symbol(state, coordinate.parameter)?;
    let mut chain = Vec::new();
    let mut reference = parameter.type_reference;
    for segment in coordinate.path {
        let facts::PlaceSegment::Field { symbol } = segment else {
            return None;
        };
        let (owner, _) = record_referent(program, reference)?;
        let field = declared_field(program, owner, symbol)?;
        chain.push((field.symbol, &field.name));
        reference = field.type_reference;
    }
    Some((parameter, chain))
}

/// The record a formal's type declares, directly or as the referent of one
/// reference, and whether that reference boundary is present.
pub(super) fn record_referent(
    program: &dyn crate::CheckingInput,
    type_reference: TypeReferenceHandle,
) -> Option<(SymbolHandle, bool)> {
    let mut reference = program
        .type_reference_table
        .unconstrained_type_reference(type_reference);
    let mut borrowed = false;
    if let TypeReferenceNode::Reference {
        referee, access, ..
    } = program.type_reference_table.type_reference(reference)
    {
        if !access.is_readable() {
            return None;
        }
        reference = program
            .type_reference_table
            .unconstrained_type_reference(*referee);
        borrowed = true;
    }
    // Lifetime/type applications still name one exact record declaration.
    // Reuse its retained closed instance or open template; leaf admission
    // independently rejects any carrier whose scalar/collection shape is open.
    let declaration = crate::value_custody::places::data_definition_for_type(program, reference)?;
    declaration
        .symbol
        .is_valid()
        .then_some((declaration.symbol, borrowed))
}

pub(super) fn parameter<'program>(
    program: &'program dyn crate::CheckingInput,
    state: &State,
    expression: ExpressionHandle,
) -> Option<&'program StateParameter> {
    let path = program.expression_table.name_path_node(expression)?;
    // One member: the subject is a bare binding occurrence, not a chain.
    // Which binding it is comes from the resolved head symbol below.
    let [_] = program.expression_table.name_path_members(path.members) else {
        return None;
    };
    // The edge owners evaluate every expression through this resolver over a
    // parameter-preserving prefix, so a mutable parameter's symbol still
    // denotes the live record the edge observes.
    program.state_parameters(state).iter().find(|parameter| {
        path.symbol.is_valid()
            && path.symbol == path.head_symbol
            && parameter.symbol == path.symbol
            && !parameter.is_const
    })
}

/// The unique field `symbol` declared by the exact record `owner` -- one
/// nominal member of one declaration, whichever module asked.
pub(super) fn declared_field<'program>(
    program: &'program dyn crate::CheckingInput,
    owner: SymbolHandle,
    symbol: SymbolHandle,
) -> Option<&'program DataField> {
    let declaration = program
        .data_definitions()
        .iter()
        .find(|data| owner.is_valid() && data.symbol == owner)?;
    let selected = program.symbols.get(symbol);
    if !symbol.is_valid() || selected.kind != SymbolKind::Field || selected.parent != owner {
        return None;
    }
    let mut fields = program
        .data_members(declaration)
        .iter()
        .filter_map(|member| match member {
            DataMember::Field(field) if field.symbol == symbol => Some(field),
            _ => None,
        });
    let field = fields.next()?;
    fields.next().is_none().then_some(field)
}

/// The value of the unique `field` entry of a plain literal of exactly `owner`.
pub fn unique_literal_field(
    program: &dyn crate::CheckingInput,
    literal: ExpressionHandle,
    owner: SymbolHandle,
    field_symbol: SymbolHandle,
    field_name: &typed_trees::name::Identifier,
) -> Option<ExpressionHandle> {
    let literal = program.expression_table.struct_literal_node(literal)?;
    if literal.type_symbol != owner || literal.case_symbol.is_some() || literal.case_name.is_some()
    {
        return None;
    }
    let mut fields = program
        .expression_table
        .struct_fields(literal.fields)
        .iter()
        .filter(|candidate| {
            candidate.field_symbol == field_symbol && candidate.name == *field_name
        });
    let value = fields.next()?.value;
    fields.next().is_none().then_some(value)
}
