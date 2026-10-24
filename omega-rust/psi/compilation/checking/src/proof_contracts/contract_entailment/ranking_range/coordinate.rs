//! The member-chain coordinate shared by field ranks and produced lengths:
//! a formal's declared record, its step fields, and the admitted leaf. The
//! two consumers differ only in which record owner a step yields and which
//! leaf the produced value admits — `CoordinateKind` carries that spec so
//! every traversal answers once.

use super::fields::{
    declared_field, declared_spelling_verified, member_chain, member_coordinate_parts,
    record_referent,
};
use super::{
    BTreeMap, Comparison, Engine, ExpressionHandle, Machine, Polynomial, RankingRangeState, State,
    exact_integer_parameter, projections,
};
use symbols::SymbolHandle;
use typed_trees::data::DataField;
use typed_trees::signature::StateParameter;

/// A field view's natural rank, an ordinary stored integer observation,
/// or a collection's length. Each owns its carrier admission, not its custody.
#[derive(Clone, Copy)]
pub(super) enum CoordinateKind {
    Field,
    Integer,
    Length,
}

impl CoordinateKind {
    /// The record owner a chain step's field reaches past its own
    /// declaration — the field view keeps readable-reference hops; the
    /// produced length requires an exact named record.
    fn step_owner<'program>(
        self,
        program: &'program dyn crate::CheckingInput,
        field: &'program DataField,
    ) -> Option<SymbolHandle> {
        match self {
            Self::Field | Self::Integer => {
                record_referent(program, field.type_reference).map(|(next, _)| next)
            }
            Self::Length => {
                if crate::reference_shell(program, field.type_reference).is_some() {
                    return None;
                }
                crate::value_custody::places::data_definition_for_type(
                    program,
                    field.type_reference,
                )
                .map(|declaration| declaration.symbol)
            }
        }
    }

    /// Declared natural field views and ordinary numeric observations share
    /// projection identity, but only the former requires an Exact unsigned
    /// carrier. A count field is a stored value, not a natural ranking view.
    fn leaf_admits(self, program: &dyn crate::CheckingInput, field: &DataField) -> bool {
        match self {
            Self::Field => matches!(
                exact_integer_parameter(program, field.type_reference),
                Some(typed_trees::unsigned_integer_primitive_pattern!())
            ),
            Self::Integer => super::integer_parameter(program, field.type_reference).is_some(),
            Self::Length => super::lengths::is_slice(program, field.type_reference),
        }
    }

    /// The identity prefix a coordinate of this kind mints.
    fn tag(self) -> &'static str {
        match self {
            Self::Field | Self::Integer => "\0ranking:field:",
            Self::Length => "\0ranking:length:",
        }
    }
}

/// A produced-value coordinate inside a formal's declared record: the
/// resolved field chain from `root` down to the leaf `field`.
pub(super) struct Coordinate<'program> {
    /// The formal whose record carries the produced value.
    pub parameter: &'program StateParameter,
    /// The formal names its record through a reference; an arrival actual is
    /// then a borrow of the rebuilt literal rather than the literal itself.
    pub borrowed: bool,
    /// The record the formal's (referent) type declares.
    pub root: SymbolHandle,
    /// The record-typed fields from `root` down to `field`'s owner, in order.
    pub steps: Vec<&'program DataField>,
    /// The leaf field the produced value reads.
    pub field: &'program DataField,
    pub identity: String,
    /// The step-owner and leaf-admission spec this coordinate resolved under.
    pub kind: CoordinateKind,
}

impl<'program> Coordinate<'program> {
    /// The coordinate `chain` names inside `parameter`'s record when the
    /// kind's leaf predicate admits the chain's leaf. A mutable record still
    /// denotes its arrival value at an edge whose evaluated prefix is proven
    /// to preserve its path; mutability is a storage capability, not
    /// evidence the value moved.
    /// Shared member-chain resolution: walk the chain, build the coordinate
    /// under `kind`'s step-owner/leaf spec, then require the spelled hops to
    /// name the resolved fields.
    pub(super) fn resolve_member(
        program: &'program dyn crate::CheckingInput,
        state: &State,
        expression: ExpressionHandle,
        kind: CoordinateKind,
    ) -> Option<Self> {
        let (parameter, chain) = member_chain(program, state, expression)?;
        let coordinate = Self::for_parameter(
            program,
            parameter,
            &chain.iter().map(|(symbol, _)| *symbol).collect::<Vec<_>>(),
            kind,
        )?;
        declared_spelling_verified(&chain, &coordinate.steps, coordinate.field)
            .then_some(coordinate)
    }

    pub(super) fn for_parameter(
        program: &'program dyn crate::CheckingInput,
        parameter: &'program StateParameter,
        chain: &[SymbolHandle],
        kind: CoordinateKind,
    ) -> Option<Self> {
        let parts = member_coordinate_parts(
            program,
            parameter,
            chain,
            |program, field| kind.step_owner(program, field),
            |program, field| kind.leaf_admits(program, field),
            kind.tag(),
        )?;
        Some(Self {
            parameter,
            borrowed: parts.borrowed,
            root: parts.root,
            steps: parts.steps,
            field: parts.field,
            identity: parts.identity,
            kind,
        })
    }

    /// The resolved symbol chain `steps` + `field` spell, in declaration
    /// order.
    pub(super) fn chain(&self) -> Vec<SymbolHandle> {
        self.steps
            .iter()
            .chain([&self.field])
            .map(|field| field.symbol)
            .collect()
    }

    /// The coordinate `self` holds at the callee's arrival block when the
    /// caller side is still at entry: a non-arrival keeps the entry
    /// coordinate unchanged.
    pub(super) fn arrive_unless_at_entry(
        self,
        program: &'program dyn crate::CheckingInput,
        state: &State,
        entry_parameters: &super::RankStateBindings,
        at_entry: bool,
    ) -> Option<Self> {
        if at_entry {
            return Some(self);
        }
        self.at_arrival(
            program,
            RankingRangeState {
                state,
                entry_parameters,
            },
            self.parameter.symbol,
        )
    }

    /// The coordinate `self` holds at the callee's arrival block when the
    /// caller passes explicit entry bindings; a caller without them keeps
    /// the coordinate unchanged.
    pub(super) fn at_entry_arrival(
        self,
        program: &'program dyn crate::CheckingInput,
        state: &State,
        entry_parameters: Option<&super::RankStateBindings>,
    ) -> Option<Self> {
        match entry_parameters {
            Some(entries) => self.at_arrival(
                program,
                RankingRangeState {
                    state,
                    entry_parameters: entries,
                },
                self.parameter.symbol,
            ),
            None => Some(self),
        }
    }

    /// Resolve the selected arrival path against its exact formal. The
    /// binding owns correspondence; declaration traversal only checks that
    /// this selected path admits the same kind of produced value.
    pub(super) fn at_arrival(
        &self,
        program: &'program dyn crate::CheckingInput,
        arrival: RankingRangeState<'_>,
        entry_symbol: SymbolHandle,
    ) -> Option<Self> {
        let mut entry = self.location(program);
        entry.parameter = entry_symbol;
        let destination = arrival.entry_parameters.destination(&entry)?;
        Self::at_coordinate(program, arrival.state, &destination, self.kind)
    }

    pub(super) fn location(
        &self,
        program: &dyn crate::CheckingInput,
    ) -> super::RankSubjectCoordinate {
        let mut path = Vec::new();
        for field in self.steps.iter().chain([&self.field]) {
            crate::value_custody::places::push_field_place_segments(
                program,
                &mut path,
                field.symbol,
            );
        }
        super::RankSubjectCoordinate {
            parameter: self.parameter.symbol,
            path,
        }
    }

    pub(super) fn at_coordinate(
        program: &'program dyn crate::CheckingInput,
        state: &State,
        coordinate: &super::RankSubjectCoordinate,
        kind: CoordinateKind,
    ) -> Option<Self> {
        let parameter = program.state_parameter_by_symbol(state, coordinate.parameter)?;
        let mut chain = Vec::new();
        for segment in &coordinate.path {
            match segment {
                facts::PlaceSegment::Field { symbol } if symbol.is_valid() => chain.push(*symbol),
                facts::PlaceSegment::Case { variant } if variant.is_valid() => {}
                _ => return None,
            }
        }
        let result = Self::for_parameter(program, parameter, &chain, kind)?;
        (result.location(program) == *coordinate).then_some(result)
    }

    /// The polynomial atom this coordinate's identity denotes.
    pub(super) fn value(&self) -> Polynomial {
        Polynomial::atom(self.identity.clone())
    }

    /// The coordinate this chain reads through `carrier`'s member `prefix`:
    /// `p.f` (or a bare formal) denotes a record, and this coordinate's
    /// chain resumes there once the prefix lands on this coordinate's own
    /// root record. A prefix step through anything but an exact declared
    /// record — a reference, a slice, or a primitive — has no carrier
    /// coordinate.
    pub(super) fn through_carrier(
        &self,
        program: &'program dyn crate::CheckingInput,
        carrier: &'program StateParameter,
        prefix: &[(SymbolHandle, &typed_trees::name::Identifier)],
    ) -> Option<Self> {
        self.through_carrier_at(program, carrier, prefix, self.root, 0)
    }

    /// The coordinate this chain reads when `carrier.prefix` supplies the
    /// record `expected` at chain position `depth`: the prefix walks the
    /// carrier's own declaration, must land on `expected`, and the remaining
    /// chain steps resolve there. A prefix landing on any other record names
    /// a different carriage, not this coordinate.
    pub(super) fn through_carrier_at(
        &self,
        program: &'program dyn crate::CheckingInput,
        carrier: &'program StateParameter,
        prefix: &[(SymbolHandle, &typed_trees::name::Identifier)],
        expected: SymbolHandle,
        depth: usize,
    ) -> Option<Self> {
        let (carrier_root, _) = record_referent(program, carrier.type_reference)?;
        let mut chain = Vec::with_capacity(prefix.len() + self.steps.len() + 1 - depth);
        let mut owner = carrier_root;
        for (symbol, _) in prefix {
            let (_, field) = declared_field(program, owner, *symbol)?;
            let Some(next) = self.kind.step_owner(program, field) else {
                return None;
            };
            chain.push(*symbol);
            owner = next;
        }
        if owner != expected {
            return None;
        }
        chain.extend(self.chain().into_iter().skip(depth));
        Self::for_parameter(program, carrier, &chain, self.kind)
    }
}

/// Recover the exact entry coordinate represented by this selected reading.
/// A current-state projection absent from the chosen witness serves its own
/// guards only and contributes no template substitution.
pub(super) fn arrived_entry_coordinate(
    program: &dyn crate::CheckingInput,
    entries: &super::RankStateBindings,
    coordinate: &Coordinate<'_>,
) -> Option<super::RankSubjectCoordinate> {
    entries.entry(&coordinate.location(program))
}

/// The projected coordinates one range judgment reads: the produced rank
/// coordinate plus every auxiliary coordinate discovered while installing
/// authored expressions. Binding spellings, declared facts, and arrival
/// substitution all iterate the same set, so a coordinate named in a guard and
/// the measure's own subject resolve to the same canonical atom. Field ranks
/// and produced lengths carry this same set over the same member-chain
/// coordinate; a [`CoordinateFamily`] supplies the four answers that differ.
pub(super) struct CoordinateSet<'program, F: CoordinateFamily<'program>> {
    rank: Option<Coordinate<'program>>,
    auxiliary: Vec<Coordinate<'program>>,
    family: std::marker::PhantomData<F>,
}

/// What one coordinate family answers differently from the other. A field rank
/// names its coordinate through an authored member projection, substitutes
/// only the entry roles its judgment demands, carries the scalar actual the
/// arrival evaluates, and contributes the leaf's declared carrier facts. A
/// produced length names its coordinate through the receiver of an authored
/// `x.chain.len` spelling, installs every coordinate that spelling names,
/// carries the length actual the same spelling computes, and contributes the
/// natural `>= 0` bound. The set owner, its installation walk, and its
/// substitution loop read no other difference.
pub(super) trait CoordinateFamily<'program> {
    /// The coordinate one authored projection expression names inside `scope`.
    fn resolve(
        program: &'program dyn crate::CheckingInput,
        machine: &Machine,
        scope: &State,
        expression: ExpressionHandle,
    ) -> Option<Coordinate<'program>>;

    /// Whether the judgment demands the installed coordinate at `entry`: a
    /// field rank substitutes only observed scalar roles, while an authored
    /// `.len` spelling is itself the demand for the length it computes.
    fn demanded(
        demanded: &[super::RankSubjectCoordinate],
        entry: &super::RankSubjectCoordinate,
    ) -> bool;

    /// The value this family's coordinate holds on arrival.
    #[allow(clippy::too_many_arguments)]
    fn arrived_actual(
        program: &'program dyn crate::CheckingInput,
        machine: &Machine,
        state: &State,
        destination_state: &State,
        destination: &super::RankSubjectCoordinate,
        engine: &mut Engine<'_>,
        argument: ExpressionHandle,
        bindings: &[(SymbolHandle, String)],
    ) -> Option<Polynomial>;

    /// The hypotheses one installed coordinate contributes.
    fn comparisons(
        program: &dyn crate::CheckingInput,
        coordinate: &Coordinate<'program>,
    ) -> Vec<Comparison>;
}

impl<'program, F: CoordinateFamily<'program>> CoordinateSet<'program, F> {
    pub(super) fn new(rank: Coordinate<'program>) -> Self {
        Self {
            rank: Some(rank),
            auxiliary: Vec::new(),
            family: std::marker::PhantomData,
        }
    }

    /// No produced rank coordinate: the measure is not this family's ranked
    /// shape, but authored spellings inside endpoints, requires facts, guards,
    /// or actuals still name auxiliary coordinates the same install discovers.
    pub(super) fn empty() -> Self {
        Self {
            rank: None,
            auxiliary: Vec::new(),
            family: std::marker::PhantomData,
        }
    }

    pub(super) fn value(&self) -> Option<Polynomial> {
        self.rank.as_ref().map(Coordinate::value)
    }

    pub(super) fn coordinates(&self) -> impl Iterator<Item = &Coordinate<'program>> {
        self.rank.iter().chain(&self.auxiliary)
    }

    pub(super) fn include(&mut self, coordinate: Coordinate<'program>) {
        // The identity spells the formal and the whole resolved chain, so two
        // same-named leaves under different steps stay distinct coordinates.
        if !self
            .coordinates()
            .any(|existing| existing.identity == coordinate.identity)
        {
            self.auxiliary.push(coordinate);
        }
    }

    pub(super) fn comparisons(&self, program: &dyn crate::CheckingInput) -> Vec<Comparison> {
        self.coordinates()
            .flat_map(|coordinate| F::comparisons(program, coordinate))
            .collect()
    }

    /// Bind only authored projections, never every field or slice in a record.
    /// Two same-named leaves remain independent by formal and resolved chain.
    /// Each authored spelling resolves in this state's own scope first, then
    /// through the other scope's declared reading transported onto this
    /// state's arrival; the engine then binds the exact atom it names.
    pub(super) fn install(
        &mut self,
        program: &'program dyn crate::CheckingInput,
        machine: &Machine,
        state: &State,
        root: &State,
        entry_parameters: Option<&super::RankStateBindings>,
        engine: &mut Engine<'_>,
        expressions: &[ExpressionHandle],
    ) -> Option<()> {
        projections::for_each_projection_expression(
            program,
            expressions,
            |member, expression, visit| {
                if let Some(coordinate) =
                    F::resolve(program, machine, state, expression).or_else(|| {
                        let coordinate = F::resolve(program, machine, root, expression)?;
                        coordinate.at_arrival(
                            program,
                            RankingRangeState {
                                state,
                                entry_parameters: entry_parameters?,
                            },
                            coordinate.parameter.symbol,
                        )
                    })
                {
                    if !engine.bind_strict_projection(expression, coordinate.value()) {
                        return None;
                    }
                    self.include(coordinate);
                }
                visit(member.receiver);
                Some(())
            },
        )
    }

    /// Every demanded coordinate of this formal receives the same evaluated
    /// actual. The caller applies this map simultaneously, never sequentially
    /// rewriting one coordinate's actual through another's new value. A
    /// duplicated destination claim produces no substitution rather than a
    /// first/last-wins guess.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn substitute(
        &self,
        program: &'program dyn crate::CheckingInput,
        machine: &Machine,
        state: &State,
        entry_parameters: Option<&super::RankStateBindings>,
        destination: Option<RankingRangeState<'_>>,
        demanded: &[super::RankSubjectCoordinate],
        engine: &mut Engine<'_>,
        formal: SymbolHandle,
        argument: ExpressionHandle,
        bindings: &[(SymbolHandle, String)],
        substitutions: &mut BTreeMap<String, Polynomial>,
    ) -> Option<bool> {
        let mut matched = false;
        for coordinate in self.coordinates() {
            let entry = match entry_parameters {
                None => coordinate.location(program),
                Some(entries) => match arrived_entry_coordinate(program, entries, coordinate) {
                    Some(entry) => entry,
                    None => continue,
                },
            };
            if !F::demanded(demanded, &entry) {
                continue;
            }
            let (arrival_state, target) = match destination {
                Some(destination) => (
                    destination.state,
                    destination.entry_parameters.destination(&entry)?,
                ),
                None => (state, entry),
            };
            if target.parameter != formal {
                continue;
            }
            let actual = F::arrived_actual(
                program,
                machine,
                state,
                arrival_state,
                &target,
                engine,
                argument,
                bindings,
            )?;
            if substitutions
                .insert(coordinate.identity.clone(), actual)
                .is_some()
            {
                return None;
            }
            matched = true;
        }
        Some(matched)
    }
}
