//! An exact field projection is a distinct arithmetic coordinate, never its
//! record. The projection may descend a nested path of exact declared record
//! fields, and its root record may be reached through a reference; each step
//! is resolved against the declaration the previous step named.
use super::identity_views::{MeasureBodyShape, measure_body_shape, unwrap_constraint_shells};
use super::{
    BigInt, BinaryOperator, Comparison, Engine, ExpressionHandle, ExpressionNode, Machine,
    Polynomial, PrimitiveType, RankingRangeState, State, TypeReferenceNode, TypedTrees,
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
/// only when the field's store range does.
pub(super) fn endpoint_lands_under(
    engine: &Engine<'_>,
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    endpoint: ExpressionHandle,
    polynomial: &Polynomial,
) -> bool {
    let carrier = match super::meanings::builtin(program, machine, state, endpoint, 0) {
        // An anonymous endpoint is a closed value and already landed.
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
    let prove = |difference: Polynomial| {
        engine.prove_at_least(&engine.substituted(&difference), &BigInt::zero())
    };
    prove(polynomial.sub(&Polynomial::constant(minimum)))
        && prove(Polynomial::constant(maximum).sub(polynomial))
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
    /// The record-typed fields from `root` down to `field`'s owner, in order.
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
        Self::for_parameter(program, parameter, &chain)
    }

    /// The coordinate an authored member chain names: every step an exact
    /// declared field, by resolved symbol and spelling, of the record before
    /// it, rooted at a state formal.
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
            // An intermediate step is an owned exact record; a reference
            // boundary inside the chain needs its own load evidence.
            let TypeReferenceNode::Named { symbol: next, .. } = program
                .type_reference_table
                .type_reference(unwrap_constraint_shells(program, field.type_reference))
            else {
                return None;
            };
            steps.push(field);
            owner = *next;
        }
        let (_, field) = declared_field(program, owner, *last)?;
        // The independently selected field view produces builtin u64.
        // Other carriers need their own view-application proof.
        if exact_integer_parameter(program, field.type_reference) != Some(PrimitiveType::U64) {
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
        let coordinate = Self::for_parameter(program, parameter, &self.chain())?;
        (coordinate.root == self.root).then_some(coordinate)
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
            if depth > 0 && self.is_prefix_projection(program, state, current, depth) {
                return Some(self.value());
            }
            current = unique_literal_field(program, current, owner, field)?;
            if let TypeReferenceNode::Named { symbol, .. } = program
                .type_reference_table
                .type_reference(unwrap_constraint_shells(program, field.type_reference))
            {
                owner = *symbol;
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
    /// record. A prefix step through anything but an exact named record -- a
    /// reference, a slice, or a primitive -- has no carrier coordinate.
    fn through_carrier(
        &self,
        program: &'program TypedTrees,
        carrier: &'program StateParameter,
        prefix: &[(SymbolHandle, &Identifier)],
    ) -> Option<Self> {
        let (carrier_root, _) = record_referent(program, carrier.type_reference)?;
        let mut chain = Vec::with_capacity(prefix.len() + self.steps.len() + 1);
        let mut owner = carrier_root;
        for (symbol, _) in prefix {
            let (_, field) = declared_field(program, owner, *symbol)?;
            // A prefix step must land on an exact declared record so this
            // coordinate's chain resumes at one nominal root.
            let TypeReferenceNode::Named { symbol: next, .. } = program
                .type_reference_table
                .type_reference(unwrap_constraint_shells(program, field.type_reference))
            else {
                return None;
            };
            chain.push(*symbol);
            owner = *next;
        }
        if owner != self.root {
            return None;
        }
        chain.extend(self.chain());
        Self::for_parameter(program, carrier, &chain)
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
            return Some(self.through_carrier(program, carrier, &prefix)?.value());
        }
        let mut owner = self.root;
        for field in self.steps.iter().chain([&self.field]) {
            current = unique_literal_field(program, current, owner, field)?;
            if let TypeReferenceNode::Named { symbol, .. } = program
                .type_reference_table
                .type_reference(unwrap_constraint_shells(program, field.type_reference))
            {
                owner = *symbol;
            }
        }
        engine.normalize(current)
    }

    /// `expression` is exactly `parameter.steps[..depth]`: the same formal
    /// projected through the first `depth` steps of this chain.
    fn is_prefix_projection(
        &self,
        program: &TypedTrees,
        state: &State,
        expression: ExpressionHandle,
        depth: usize,
    ) -> bool {
        member_chain(program, state, expression).is_some_and(|(parameter, chain)| {
            parameter.symbol == self.parameter.symbol
                && chain.len() == depth
                && chain
                    .iter()
                    .zip(&self.steps)
                    .all(|((symbol, spelled), step)| {
                        *symbol == step.symbol && *spelled == &step.name
                    })
        })
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
        match program
            .type_reference_table
            .type_reference(unwrap_constraint_shells(program, field.type_reference))
        {
            TypeReferenceNode::Named { symbol, .. } => owner = *symbol,
            _ => owner = SymbolHandle::default(),
        }
    }
    declared
}

/// The member chain of `expression` from its root formal outward, each step
/// as `(resolved field symbol, spelling)`. Case projections are not fields.
fn member_chain<'program>(
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
fn rooted_carrier<'program>(
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
    if let TypeReferenceNode::Reference { referee, .. } =
        program.type_reference_table.type_reference(reference)
    {
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

fn parameter<'program>(
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
fn declared_field(
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
fn unique_literal_field(
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
