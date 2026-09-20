//! An exact field projection is a distinct arithmetic coordinate, never its
//! record. The projection may descend a nested path of exact declared record
//! fields and readable references; every step resolves against its declaration.
//! Resolution supplies a coordinate, not a frozen value. State and call edge
//! owners establish exact arrival equality and complete write-frame preservation
//! of its reference bindings and referent contents before consuming its bounds.
use super::identity_views::{MeasureBodyShape, measure_body_shape, unwrap_constraint_shells};
use super::{
    BTreeMap, BigInt, BinaryOperator, Comparison, Engine, ExpressionHandle, ExpressionNode,
    Machine, Polynomial, PrimitiveType, RankingRangeState, State, TypeReferenceNode, TypedTrees,
    exact_integer_parameter,
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
    program: &TypedTrees,
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
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    endpoint: ExpressionHandle,
    polynomial: &Polynomial,
) -> bool {
    if !operations_land_under(engine, program, machine, state, endpoint, None) {
        return false;
    }
    let carrier = match super::meanings::builtin(program, machine, state, endpoint, 0) {
        // An anonymous endpoint is a closed value and already landed. A
        // typed-literal operation such as `255u8 + 1u8` reaches here too;
        // `operations_land_under` has already judged its own carrier.
        Some(None) => return true,
        Some(Some(reference)) => reference,
        // A non-builtin endpoint never reaches normalization; the template
        // admission rejected it before this judgment ran.
        None => return false,
    };
    let Some(primitive) = exact_integer_parameter(program, carrier) else {
        return false;
    };
    let Some((minimum, maximum)) = super::integer_carrier_bounds(primitive) else {
        return false;
    };
    let prove = |difference: &Polynomial| {
        engine.prove_at_least(&engine.substituted(difference), &BigInt::zero())
    };
    prove(&polynomial.sub(&Polynomial::constant(minimum)))
        && prove(&Polynomial::constant(maximum).sub(polynomial))
}

/// Arrival equality of a normalized endpoint does not preserve formation:
/// `cap / divisor - cap / divisor` loses both divisions. Replay their
/// independent obligations under the exact simultaneous argument map before
/// accepting a new state, without importing destination requirements.
pub(super) fn endpoint_operations_land_after_arrival(
    engine: &mut Engine<'_>,
    program: &TypedTrees,
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
    program: &TypedTrees,
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
        ExpressionNode::Binary(binary)
            if matches!(
                binary.operator,
                BinaryOperator::Add
                    | BinaryOperator::Subtract
                    | BinaryOperator::Multiply
                    | BinaryOperator::Divide
                    | BinaryOperator::Modulo
            ) =>
        {
            binary
        }
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
    if let (Some(left), Some(right)) = (left, right)
        && left != right
    {
        return false;
    }
    if let Some(primitive) = left.or(right) {
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
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    node: ExpressionHandle,
) -> Option<PrimitiveType> {
    if let Some(value) = program.closed_integer_value_in(node, machine.symbol) {
        return value.primitive;
    }
    match super::meanings::builtin(program, machine, state, node, 0) {
        Some(Some(reference)) => exact_integer_parameter(program, reference),
        _ => None,
    }
}

/// One `u64` field reached from a state formal through an exact chain of
/// declared record fields: `parameter.steps[0]. ... .steps[n].field`.
pub(super) struct FieldCoordinate<'program> {
    pub parameter: &'program StateParameter,
    /// The formal names its record through a reference; an arrival actual is
    /// then a borrow of the rebuilt literal rather than the literal itself.
    pub borrowed: bool,
    /// The record the formal's (referent) type declares.
    pub root: SymbolHandle,
    /// Record or readable-reference fields from `root` to `field`'s owner.
    pub steps: Vec<&'program DataField>,
    pub field: &'program DataField,
    pub identity: String,
}

impl<'program> FieldCoordinate<'program> {
    /// The produced rank of the declared field view `measure` applied to
    /// `subject`: the measure body's projection chain, re-resolved against
    /// the subject formal's own declaration rather than trusted from the view.
    pub(super) fn resolve(
        program: &'program TypedTrees,
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
        Self::for_parameter(program, parameter, &chain, false)
    }

    /// The coordinate an authored member chain names: every step an exact
    /// declared field, by resolved symbol and spelling, of the record before
    /// it, rooted at a state formal. The leaf may be any exact unsigned
    /// integer: a scalar rank subject or endpoint reads the projected
    /// natural coordinate, not only the `u64` a declared field view emits.
    pub(super) fn resolve_projection(
        program: &'program TypedTrees,
        state: &State,
        expression: ExpressionHandle,
    ) -> Option<Self> {
        let (parameter, chain) = member_chain(program, state, expression)?;
        let coordinate = Self::for_parameter(
            program,
            parameter,
            &chain.iter().map(|(symbol, _)| *symbol).collect::<Vec<_>>(),
            true,
        )?;
        let spelled_as_declared = chain
            .iter()
            .zip(coordinate.steps.iter().chain([&coordinate.field]))
            .all(|((_, spelled), field)| **spelled == field.name);
        spelled_as_declared.then_some(coordinate)
    }

    fn for_parameter(
        program: &'program TypedTrees,
        parameter: &'program StateParameter,
        chain: &[SymbolHandle],
        natural_leaf: bool,
    ) -> Option<Self> {
        // A mutable record still denotes its arrival value at an edge whose
        // evaluated prefix is proven to preserve its path; mutability is a
        // storage capability, not evidence the field moved.
        if parameter.is_self || parameter.is_const {
            return None;
        }
        let (root, borrowed) = record_referent(program, parameter.type_reference)?;
        let (last, step_symbols) = chain.split_last()?;
        let mut owner = root;
        let mut steps = Vec::with_capacity(step_symbols.len());
        for symbol in step_symbols {
            let (_, field) = declared_field(program, owner, *symbol)?;
            // The coordinate retains the complete declared chain, including
            // stored readable references. Its edge owner separately proves
            // that the carrier, reference bindings and pointee stay unwritten.
            let (next, _) = record_referent(program, field.type_reference)?;
            steps.push(field);
            owner = next;
        }
        let (_, field) = declared_field(program, owner, *last)?;
        // The independently selected field view produces builtin u64.
        // Other carriers need their own view-application proof. An authored
        // member chain read directly as a natural coordinate admits any exact
        // unsigned leaf -- a signed field has no natural rank coordinate.
        let leaf = exact_integer_parameter(program, field.type_reference);
        let leaf_is_natural = if natural_leaf {
            matches!(
                leaf,
                Some(
                    PrimitiveType::U8
                        | PrimitiveType::U16
                        | PrimitiveType::U32
                        | PrimitiveType::U64
                )
            )
        } else {
            leaf == Some(PrimitiveType::U64)
        };
        if !leaf_is_natural {
            return None;
        }
        let mut identity = format!("\0ranking:field:{:?}", parameter.symbol);
        for symbol in chain {
            identity.push_str(&format!(":{symbol:?}"));
        }
        Some(Self {
            parameter,
            borrowed,
            root,
            steps,
            field,
            identity,
        })
    }

    /// The resolved field symbols from the root record down to `field`.
    pub(super) fn chain(&self) -> Vec<SymbolHandle> {
        self.steps
            .iter()
            .chain([&self.field])
            .map(|field| field.symbol)
            .collect()
    }

    /// A telescope names a role, not record compatibility or equality between
    /// copies. Only a unique formal with the exact declaration can carry this
    /// coordinate; duplicated record roles need independent field facts.
    pub(super) fn at_arrival(
        &self,
        program: &'program TypedTrees,
        arrival: RankingRangeState<'_>,
        entry_symbol: SymbolHandle,
    ) -> Option<Self> {
        if !entry_symbol.is_valid() {
            return None;
        }
        let mut parameters = program
            .state_parameters(arrival.state)
            .iter()
            .filter(|parameter| !parameter.is_self)
            .zip(arrival.entry_parameters)
            .filter_map(|(parameter, entry)| (*entry == entry_symbol).then_some(parameter));
        let parameter = parameters.next()?;
        if parameters.next().is_some() {
            return None;
        }
        self.for_carrier(program, parameter)
    }

    /// The coordinate this chain names on `carrier`'s own record: the chain
    /// resolved directly against the carrier's declaration, else the unique
    /// nested path that declaration admits for the chain's boundary records.
    /// These are the same two readings `at_arrival` keeps once a telescope has
    /// located the role's carrier -- `pair.left` carries `countdown`, or the
    /// formal is itself a record this chain reaches partway. A caller that
    /// already holds the formal asks this directly; two admissible readings
    /// keep no coordinate rather than guess which record the carriage meant.
    pub(super) fn for_carrier(
        &self,
        program: &'program TypedTrees,
        carrier: &'program StateParameter,
    ) -> Option<Self> {
        // The re-resolved chain lands on the same declared leaf by
        // construction, so the natural-leaf check is the origin's own.
        let chain = self.chain();
        if let Some(coordinate) = Self::for_parameter(program, carrier, &chain, true)
            && coordinate.root == self.root
        {
            return Some(coordinate);
        }
        let chain = self.arrival_chain(program, carrier)?;
        Self::for_parameter(program, carrier, &chain, true)
    }

    /// The chain this coordinate becomes when `parameter`'s record carries
    /// the role at a nested path; see [`nested_arrival_chain`].
    fn arrival_chain(
        &self,
        program: &'program TypedTrees,
        parameter: &'program StateParameter,
    ) -> Option<Vec<SymbolHandle>> {
        nested_arrival_chain(program, parameter, self.root, &self.steps, &self.chain())
    }

    pub(super) fn value(&self) -> Polynomial {
        Polynomial::atom(self.identity.clone())
    }

    pub(super) fn comparisons(&self, program: &TypedTrees) -> Vec<Comparison> {
        let mut comparisons = vec![(
            BinaryOperator::GreaterOrEqual,
            self.value(),
            Polynomial::default(),
        )];
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
        comparisons
    }

    /// The value this coordinate holds after `expression` arrives in its
    /// formal: the formal itself forwarded, a record carrier behind the
    /// destination's `&` or a member-target `p.f` whose projection prefix
    /// lands on this coordinate's root, or the literal chain rebuilt
    /// declaration by declaration down to the field. `arrival_borrowed` is
    /// the destination formal's own reference boundary, so a borrowed arrival
    /// unwraps one `&` no matter how the source slot stored its record. A
    /// forward of the exact prefix projection at any depth keeps the
    /// remaining chain's current value; a literal of another declaration, a
    /// foreign record, or a missing step is not this coordinate.
    pub(super) fn actual(
        &self,
        program: &TypedTrees,
        state: &State,
        engine: &mut Engine<'_>,
        expression: ExpressionHandle,
        arrival_borrowed: bool,
    ) -> Option<Polynomial> {
        if let Some(coordinate) =
            self.actual_coordinate(program, state, expression, arrival_borrowed)
        {
            return Some(coordinate.value());
        }
        let mut current = expression;
        if arrival_borrowed {
            // An owned actual into a borrowed formal has no reference boundary
            // to read through; only a `&` spelling reaches the record the
            // destination denotes.
            let ExpressionNode::Borrow(borrow) = program.expression_table.expression(current)
            else {
                return None;
            };
            current = borrow.target;
        }
        let mut owner = self.root;
        for (depth, field) in self.steps.iter().chain([&self.field]).enumerate() {
            if depth > 0
                && let Some((carrier, prefix)) = rooted_carrier(program, state, current)
            {
                // `current` names the record this step reads through a
                // carrier instead of a literal: a formal forward or a member
                // projection inside the rebuilt record. Its prefix must land
                // on the record the chain expects at this step; the remaining
                // chain then resolves under the carrier's own declaration, so
                // a forward of this coordinate's own prefix keeps this
                // coordinate itself.
                return self
                    .through_carrier_at(program, carrier, &prefix, owner, depth)
                    .map(|coordinate| coordinate.value());
            }
            current = unique_literal_field(program, current, owner, field)?;
            if let Some((next, _)) = record_referent(program, field.type_reference) {
                owner = next;
            }
        }
        engine.normalize(current)
    }

    /// The exact coordinate `expression` installs for this chain when the
    /// actual itself denotes a record carrier: this formal forwarded, a
    /// same-record sibling behind the destination's `&`, or a member-target
    /// `p.f` (possibly `&p.f`) whose projection prefix lands on this
    /// coordinate's root record so the chain resumes there. A literal or
    /// computed actual carries no coordinate; `actual` walks those field by
    /// field. Callers that keep a coordinate set may `include` the result so
    /// the carrier's declared field bounds become hypotheses.
    pub(super) fn actual_coordinate(
        &self,
        program: &'program TypedTrees,
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

    /// The coordinate this chain reads through `carrier`'s member `prefix`:
    /// `p.f` (or a bare formal) denotes a record, and this coordinate's chain
    /// resumes there once the prefix lands on this coordinate's own root
    /// record. Each prefix step must resolve to an exact record, directly
    /// or through a readable reference; slices and primitives cannot carry it.
    fn through_carrier(
        &self,
        program: &'program TypedTrees,
        carrier: &'program StateParameter,
        prefix: &[(SymbolHandle, &Identifier)],
    ) -> Option<Self> {
        self.through_carrier_at(program, carrier, prefix, self.root, 0)
    }

    /// The coordinate this chain reads when `carrier.prefix` supplies the
    /// record `expected` at chain position `depth`: the prefix walks the
    /// carrier's own declaration, must land on `expected`, and the remaining
    /// chain steps resolve there. A prefix landing on any other record names
    /// a different carriage, not this coordinate.
    fn through_carrier_at(
        &self,
        program: &'program TypedTrees,
        carrier: &'program StateParameter,
        prefix: &[(SymbolHandle, &Identifier)],
        expected: SymbolHandle,
        depth: usize,
    ) -> Option<Self> {
        let (carrier_root, _) = record_referent(program, carrier.type_reference)?;
        let mut chain = Vec::with_capacity(prefix.len() + self.steps.len() + 1 - depth);
        let mut owner = carrier_root;
        for (symbol, _) in prefix {
            let (_, field) = declared_field(program, owner, *symbol)?;
            // Resume only at the exact declared record reached by this
            // readable projection, preserving the reference-bearing prefix.
            let (next, _) = record_referent(program, field.type_reference)?;
            chain.push(*symbol);
            owner = next;
        }
        if owner != expected {
            return None;
        }
        chain.extend(self.chain().into_iter().skip(depth));
        Self::for_parameter(program, carrier, &chain, true)
    }

    /// The value `expression` installs for this coordinate's chain when it
    /// arrives at ANOTHER formal of the same record -- a cross-machine call
    /// actual, where `self.parameter` is the callee's formal rather than the
    /// carrier named here. A bare forward or a member actual re-resolves the
    /// chain behind the actual's own resolved prefix; a borrow unwraps the
    /// destination formal's `&`; a literal is rebuilt declaration by
    /// declaration. Every carrier route still requires the exact record this
    /// coordinate's chain starts at: a same-shaped record elsewhere in the
    /// actual's projection is not its carrier.
    /// Resolving a source coordinate also installs its store-enforced field
    /// bounds. A caller may forward a whole record without spelling this leaf;
    /// the callee's demanded projection must still read the source's bounded
    /// value. These are caller declaration facts, never callee requirements.
    pub(super) fn arrived(
        &self,
        program: &'program TypedTrees,
        state: &State,
        engine: &mut Engine<'_>,
        expression: ExpressionHandle,
        arrival_borrowed: bool,
    ) -> Option<Polynomial> {
        let mut current = expression;
        if arrival_borrowed
            && let ExpressionNode::Borrow(borrow) = program.expression_table.expression(current)
        {
            // `&x` supplies the record `x` denotes; an actual already behind a
            // reference arrives as its own carrier without a borrow node.
            current = borrow.target;
        }
        if let Some((carrier, prefix)) = rooted_carrier(program, state, current) {
            let coordinate = self.through_carrier(program, carrier, &prefix)?;
            return engine
                .install_hypotheses(coordinate.comparisons(program))
                .then(|| coordinate.value());
        }
        let mut owner = self.root;
        for (depth, field) in self.steps.iter().chain([&self.field]).enumerate() {
            if depth > 0
                && let Some((carrier, prefix)) = rooted_carrier(program, state, current)
            {
                // As in `actual`: a literal field value may itself name a
                // record through a carrier's projection prefix; the prefix
                // must land on the record this chain expects at this step
                // before the remaining steps resume under its declaration.
                let coordinate =
                    self.through_carrier_at(program, carrier, &prefix, owner, depth)?;
                return engine
                    .install_hypotheses(coordinate.comparisons(program))
                    .then(|| coordinate.value());
            }
            current = unique_literal_field(program, current, owner, field)?;
            if let Some((next, _)) = record_referent(program, field.type_reference) {
                owner = next;
            }
        }
        engine.normalize(current)
    }
}

/// The declared type of an exact member chain rooted at a state formal:
/// `parameter.a.b.field`, every step a resolved field of the record before it.
pub(super) fn projected_type(
    program: &TypedTrees,
    state: &State,
    expression: ExpressionHandle,
) -> Option<TypeReferenceHandle> {
    let (parameter, chain) = member_chain(program, state, expression)?;
    let (mut owner, _) = record_referent(program, parameter.type_reference)?;
    let mut declared = None;
    for (symbol, spelled) in &chain {
        let (_, field) = declared_field(program, owner, *symbol)?;
        if field.name != **spelled {
            return None;
        }
        declared = Some(field.type_reference);
        owner = record_referent(program, field.type_reference)
            .map_or(SymbolHandle::default(), |(next, _)| next);
    }
    declared
}

/// The member chain of `expression` from its root formal outward, each step
/// as `(resolved field symbol, spelling)`. Case projections are not fields.
pub(super) fn member_chain<'program>(
    program: &'program TypedTrees,
    state: &State,
    expression: ExpressionHandle,
) -> Option<(
    &'program StateParameter,
    Vec<(SymbolHandle, &'program Identifier)>,
)> {
    let mut chain = Vec::new();
    let mut cursor = expression;
    while let ExpressionNode::Member(member) = program.expression_table.expression(cursor) {
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
    Some((parameter(program, state, cursor)?, chain))
}

/// The formal `expression` is rooted at plus its resolved member chain: `x`
/// is `(x, [])`, `x.a.b` is `(x, [a, b])`. Any other shape is a computed
/// value with no carrier root.
pub(super) fn rooted_carrier<'program>(
    program: &'program TypedTrees,
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
}

/// The record a formal's type declares, directly or as the referent of one
/// reference, and whether that reference boundary is present.
pub(super) fn record_referent(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
) -> Option<(SymbolHandle, bool)> {
    let mut reference = unwrap_constraint_shells(program, type_reference);
    let mut borrowed = false;
    if let TypeReferenceNode::Reference {
        referee, access, ..
    } = program.type_reference_table.type_reference(reference)
    {
        if !access.is_readable() {
            return None;
        }
        reference = unwrap_constraint_shells(program, *referee);
        borrowed = true;
    }
    let TypeReferenceNode::Named { symbol, .. } =
        program.type_reference_table.type_reference(reference)
    else {
        return None;
    };
    (symbol.is_valid()
        && program
            .data_definitions()
            .iter()
            .any(|data| data.symbol == *symbol))
    .then_some((*symbol, borrowed))
}

pub(super) fn parameter<'program>(
    program: &'program TypedTrees,
    state: &State,
    expression: ExpressionHandle,
) -> Option<&'program StateParameter> {
    let ExpressionNode::Name(path) = program.expression_table.expression(expression) else {
        return None;
    };
    let [name] = program.expression_table.name_path_members(path.members) else {
        return None;
    };
    // The edge owners evaluate every expression through this resolver over a
    // parameter-preserving prefix, so a mutable parameter's name still denotes
    // the live record the edge observes.
    program.state_parameters(state).iter().find(|parameter| {
        path.symbol.is_valid()
            && path.symbol == path.head_symbol
            && parameter.symbol == path.symbol
            && parameter.name == *name
            && !parameter.is_self
            && !parameter.is_const
    })
}

/// The unique field `symbol` declared by the exact record `owner`.
pub(super) fn declared_field(
    program: &TypedTrees,
    owner: SymbolHandle,
    symbol: SymbolHandle,
) -> Option<(SymbolHandle, &DataField)> {
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
    fields.next().is_none().then_some((owner, field))
}

/// The value of the unique `field` entry of a plain literal of exactly `owner`.
pub(super) fn unique_literal_field(
    program: &TypedTrees,
    literal: ExpressionHandle,
    owner: SymbolHandle,
    field: &DataField,
) -> Option<ExpressionHandle> {
    let ExpressionNode::StructLiteral(literal) = program.expression_table.expression(literal)
    else {
        return None;
    };
    if literal.type_symbol != owner || literal.case_symbol.is_some() || literal.case_name.is_some()
    {
        return None;
    }
    let mut fields = program
        .expression_table
        .struct_fields(literal.fields)
        .iter()
        .filter(|candidate| candidate.field_symbol == field.symbol && candidate.name == field.name);
    let value = fields.next()?.value;
    fields.next().is_none().then_some(value)
}

/// The chain a `root`-anchored coordinate becomes when `parameter`'s record
/// carries the role at a nested path: the formal's declaration contains one
/// of this chain's boundary records at a unique field path, or the formal is
/// itself a record this chain reaches partway. `steps` are the chain's
/// record-typed intermediate fields and `chain` the full resolved field
/// symbols including the leaf. Each candidate is a distinct reading of the
/// same role, so zero or two stay role-less rather than guess which record
/// the arrival meant.
pub(super) fn nested_arrival_chain(
    program: &TypedTrees,
    parameter: &StateParameter,
    root: SymbolHandle,
    steps: &[&DataField],
    chain: &[SymbolHandle],
) -> Option<Vec<SymbolHandle>> {
    let (formal_record, _) = record_referent(program, parameter.type_reference)?;
    // `boundaries[i]` is the record this chain reaches after `chain[..i]`
    // under its own root -- the owner `chain[i]` resolves against. A carrier
    // formal can denote any of them through its own fields.
    let mut boundaries = Vec::with_capacity(steps.len() + 1);
    boundaries.push(root);
    for step in steps {
        let TypeReferenceNode::Named { symbol: next, .. } = program
            .type_reference_table
            .type_reference(unwrap_constraint_shells(program, step.type_reference))
        else {
            return None;
        };
        boundaries.push(*next);
    }
    let mut candidates: Vec<Vec<SymbolHandle>> = Vec::new();
    for (boundary, target) in boundaries.iter().enumerate() {
        let mut prefixes = Vec::new();
        collect_record_paths(program, formal_record, *target, &mut prefixes);
        for mut prefix in prefixes {
            if boundary == 0 && prefix.is_empty() {
                // The direct reading was already judged by the caller.
                continue;
            }
            prefix.extend(&chain[boundary..]);
            if !candidates.contains(&prefix) {
                candidates.push(prefix);
            }
            if candidates.len() > 1 {
                return None;
            }
        }
    }
    let [resolved] = candidates.as_slice() else {
        return None;
    };
    Some(resolved.clone())
}

/// Every field path under `from`'s declaration that lands on record `to`,
/// including the empty path when `from` is already `to`. Only owned exact
/// record steps carry the descent -- a reference boundary or a leaf ends it,
/// matching `for_parameter`'s chain resolution -- and a path that would
/// revisit a record it already crossed reads a slot it left, not a new
/// arrival. Collection stops at two: a second path already makes the
/// carriage ambiguous, so no caller needs the rest.
pub(super) fn collect_record_paths(
    program: &TypedTrees,
    from: SymbolHandle,
    to: SymbolHandle,
    paths: &mut Vec<Vec<SymbolHandle>>,
) {
    fn visit(
        program: &TypedTrees,
        owner: SymbolHandle,
        target: SymbolHandle,
        visiting: &mut Vec<SymbolHandle>,
        path: &mut Vec<SymbolHandle>,
        paths: &mut Vec<Vec<SymbolHandle>>,
        depth: usize,
    ) {
        if paths.len() >= 2 || depth >= 16 {
            return;
        }
        let Some(declaration) = program
            .data_definitions()
            .iter()
            .find(|data| data.symbol == owner)
        else {
            return;
        };
        visiting.push(owner);
        for member in program.data_members(declaration) {
            let DataMember::Field(field) = member else {
                continue;
            };
            let TypeReferenceNode::Named { symbol: next, .. } = program
                .type_reference_table
                .type_reference(unwrap_constraint_shells(program, field.type_reference))
            else {
                continue;
            };
            path.push(field.symbol);
            // A step landing on the target records its own path even when
            // the record is already crossed -- `x` and `x.y` both landing on
            // the target are genuinely two readings. Deeper paths through an
            // already-crossed record keep reading slots this path left, so
            // recursion continues only into fresh records.
            if *next == target {
                paths.push(path.clone());
            }
            if !visiting.contains(next) {
                visit(program, *next, target, visiting, path, paths, depth + 1);
            }
            path.pop();
            if paths.len() >= 2 {
                break;
            }
        }
        visiting.pop();
    }
    if from == to {
        paths.push(Vec::new());
    }
    visit(
        program,
        from,
        to,
        &mut Vec::new(),
        &mut Vec::new(),
        paths,
        0,
    );
}

/// Every field path under `from`'s declaration that lands on a leaf `accept`
/// names, reached through owned exact record steps exactly as
/// `collect_record_paths` descends. A bare formal's role packed inside a
/// record literal is legible only while that leaf path is unique: two
/// candidate leaves make the carriage ambiguous, so collection stops at two
/// and callers keep no coordinate rather than guess which leaf holds the
/// role.
pub(super) fn collect_leaf_paths(
    program: &TypedTrees,
    from: SymbolHandle,
    accept: impl Fn(&TypedTrees, &DataField) -> bool,
    paths: &mut Vec<Vec<SymbolHandle>>,
) {
    fn visit(
        program: &TypedTrees,
        owner: SymbolHandle,
        accept: &dyn Fn(&TypedTrees, &DataField) -> bool,
        visiting: &mut Vec<SymbolHandle>,
        path: &mut Vec<SymbolHandle>,
        paths: &mut Vec<Vec<SymbolHandle>>,
        depth: usize,
    ) {
        if paths.len() >= 2 || depth >= 16 {
            return;
        }
        let Some(declaration) = program
            .data_definitions()
            .iter()
            .find(|data| data.symbol == owner)
        else {
            return;
        };
        visiting.push(owner);
        for member in program.data_members(declaration) {
            let DataMember::Field(field) = member else {
                continue;
            };
            path.push(field.symbol);
            if accept(program, field) {
                paths.push(path.clone());
            } else if let TypeReferenceNode::Named { symbol: next, .. } = program
                .type_reference_table
                .type_reference(unwrap_constraint_shells(program, field.type_reference))
                && program
                    .data_definitions()
                    .iter()
                    .any(|data| data.symbol == *next)
                && !visiting.contains(next)
            {
                // A reference boundary or a scalar leaf ends the descent,
                // matching `for_parameter`'s chain resolution; a path through
                // an already-crossed record reads a slot this path left.
                visit(program, *next, accept, visiting, path, paths, depth + 1);
            }
            path.pop();
            if paths.len() >= 2 {
                break;
            }
        }
        visiting.pop();
    }
    visit(
        program,
        from,
        &accept,
        &mut Vec::new(),
        &mut Vec::new(),
        paths,
        0,
    );
}

/// The natural coordinate a bare integer entry role takes inside
/// `parameter`'s record: the declaration must carry exactly one exact
/// unsigned leaf reachable through owned record steps. The claim only locates
/// the leaf -- membership, endpoints, and descent still prove independently,
/// and a second admissible leaf keeps no coordinate rather than guessing.
pub(super) fn integer_leaf_coordinate<'program>(
    program: &'program TypedTrees,
    parameter: &'program StateParameter,
) -> Option<FieldCoordinate<'program>> {
    let (root, _) = record_referent(program, parameter.type_reference)?;
    let mut paths = Vec::new();
    collect_leaf_paths(
        program,
        root,
        |program, field| {
            matches!(
                exact_integer_parameter(program, field.type_reference),
                Some(
                    PrimitiveType::U8
                        | PrimitiveType::U16
                        | PrimitiveType::U32
                        | PrimitiveType::U64
                )
            )
        },
        &mut paths,
    );
    let [path] = paths.as_slice() else {
        return None;
    };
    FieldCoordinate::for_parameter(program, parameter, path, true)
}

/// The unique non-self formal of `state` whose telescope slot claims `entry`,
/// or none when the role is unclaimed or contested by a second carrier.
pub(super) fn unique_entry_carrier<'program>(
    program: &'program TypedTrees,
    state: &'program State,
    entry_parameters: &[SymbolHandle],
    entry: SymbolHandle,
) -> Option<&'program StateParameter> {
    let mut carriers = program
        .state_parameters(state)
        .iter()
        .filter(|parameter| !parameter.is_self)
        .zip(entry_parameters)
        .filter_map(|(parameter, claimed)| (*claimed == entry).then_some(parameter));
    let carrier = carriers.next()?;
    carriers.next().is_none().then_some(carrier)
}
