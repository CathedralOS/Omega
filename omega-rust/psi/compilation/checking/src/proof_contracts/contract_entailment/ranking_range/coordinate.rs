//! The member-chain coordinate shared by field ranks and produced lengths:
//! a formal's declared record, its step fields, and the admitted leaf. The
//! two consumers differ only in which record owner a step yields and which
//! leaf the produced value admits — `CoordinateKind` carries that spec so
//! every traversal answers once.

use super::fields::{
    declared_field, member_coordinate_parts, nested_arrival_chain, record_referent,
};
use super::identity_views::unwrap_constraint_shells;
use super::{
    Polynomial, PrimitiveType, RankingRangeState, State, TypeReferenceNode, TypedTrees,
    exact_integer_parameter,
};
use symbols::SymbolHandle;
use typed_trees::data::DataField;
use typed_trees::signature::StateParameter;

/// Which produced value a member chain's leaf carries: a field view's
/// natural rank coordinate, or a collection's length.
#[derive(Clone, Copy)]
pub(super) enum CoordinateKind {
    Field,
    Length,
}

impl CoordinateKind {
    /// The record owner a chain step's field reaches past its own
    /// declaration — the field view keeps readable-reference hops; the
    /// produced length requires an exact named record.
    fn step_owner<'program>(
        self,
        program: &'program TypedTrees,
        field: &'program DataField,
    ) -> Option<SymbolHandle> {
        match self {
            Self::Field => record_referent(program, field.type_reference).map(|(next, _)| next),
            Self::Length => {
                let TypeReferenceNode::Named { symbol: next, .. } = program
                    .type_reference_table
                    .type_reference(unwrap_constraint_shells(program, field.type_reference))
                else {
                    return None;
                };
                Some(*next)
            }
        }
    }

    /// Whether the chain's leaf field admits this produced value.
    /// `natural_leaf` is the origin's rule for an unsigned leaf — a field
    /// view emits `u64` while an authored member chain admits any exact
    /// unsigned integer; the produced length ignores it.
    fn leaf_admits<'program>(
        self,
        program: &'program TypedTrees,
        field: &'program DataField,
        natural_leaf: bool,
    ) -> bool {
        match self {
            Self::Field => {
                let leaf = exact_integer_parameter(program, field.type_reference);
                if natural_leaf {
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
                }
            }
            Self::Length => super::lengths::is_slice(program, field.type_reference),
        }
    }

    /// The identity prefix a coordinate of this kind mints.
    fn tag(self) -> &'static str {
        match self {
            Self::Field => "\0ranking:field:",
            Self::Length => "\0ranking:length:",
        }
    }

    /// The natural-leaf rule a re-resolution on an arrival applies: the
    /// rebuilt chain lands on the same declared leaf by construction, so a
    /// field coordinate keeps the origin's own unsigned-leaf admission.
    /// The length's leaf predicate does not read the flag.
    fn natural_leaf_on_arrival(self) -> bool {
        matches!(self, Self::Field)
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
    pub(super) fn for_parameter(
        program: &'program TypedTrees,
        parameter: &'program StateParameter,
        chain: &[SymbolHandle],
        kind: CoordinateKind,
        natural_leaf: bool,
    ) -> Option<Self> {
        let parts = member_coordinate_parts(
            program,
            parameter,
            chain,
            |program, field| kind.step_owner(program, field),
            |program, field| kind.leaf_admits(program, field, natural_leaf),
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
        program: &'program TypedTrees,
        state: &State,
        entry_parameters: &[SymbolHandle],
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
        program: &'program TypedTrees,
        state: &State,
        entry_parameters: Option<&[SymbolHandle]>,
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

    /// The coordinate the arrival's parameter forwarding keeps: the entry
    /// binding `entry_symbol` must resolve to exactly one of the arrival
    /// state's parameters, which then answers `for_carrier`.
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
    /// These are the same two readings `at_arrival` keeps once a telescope
    /// has located the role's carrier — `pair.left` carries `countdown`, or
    /// the formal is itself a record this chain reaches partway. A caller
    /// that already holds the formal asks this directly; two admissible
    /// readings keep no coordinate rather than guess which record the
    /// carriage meant.
    pub(super) fn for_carrier(
        &self,
        program: &'program TypedTrees,
        carrier: &'program StateParameter,
    ) -> Option<Self> {
        let chain = self.chain();
        if let Some(coordinate) = Self::for_parameter(
            program,
            carrier,
            &chain,
            self.kind,
            self.kind.natural_leaf_on_arrival(),
        ) && coordinate.root == self.root
        {
            return Some(coordinate);
        }
        // The role may instead arrive nested inside the carrier formal's own
        // record — `pair.bag` carries the collection's record — or as a
        // record this chain descends partway. The formal's declaration must
        // admit exactly one such reading; two readings leave the carriage
        // ambiguous and keep no coordinate, exactly as the field owner does.
        let chain = self.arrival_chain(program, carrier)?;
        Self::for_parameter(
            program,
            carrier,
            &chain,
            self.kind,
            self.kind.natural_leaf_on_arrival(),
        )
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
        program: &'program TypedTrees,
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
        program: &'program TypedTrees,
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
        Self::for_parameter(
            program,
            carrier,
            &chain,
            self.kind,
            self.kind.natural_leaf_on_arrival(),
        )
    }
}
