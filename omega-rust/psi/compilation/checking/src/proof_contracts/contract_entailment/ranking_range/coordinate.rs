//! The member-chain coordinate shared by field ranks and produced lengths:
//! a formal's declared record, its step fields, and the admitted leaf. The
//! two consumers differ only in which record owner a step yields and which
//! leaf the produced value admits — `CoordinateKind` carries that spec so
//! every traversal answers once.

use super::fields::{
    declared_field, declared_spelling_verified, member_chain, member_coordinate_parts,
    record_referent,
};
use super::{ExpressionHandle, Polynomial, RankingRangeState, State, exact_integer_parameter};
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
                let Some(next) = program
                    .type_reference_table
                    .unconstrained_named_symbol(field.type_reference)
                else {
                    return None;
                };
                Some(next)
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
