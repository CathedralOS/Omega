//! The live place facts a selected scalar plan reads during flow and at exits.
//!
//! This is the interval source behind `validation::selected_scalar_interval`:
//! it answers each operand position from the live snapshot at its place, the
//! standing invariants of its storage (declared ranges, an immutable formal's
//! arrival contract) and completed byte literals. It computes no arithmetic;
//! the interval domain is validation's.

use crate::flow::canonical_place_from_symbol;
use crate::values::exclusive_reference;
use checked_trees::CheckedStructuralPredicatePathSegment;
use facts::{FactContextHandle, FactPlan, IntegerRange};
use numerics::bignum::BigInt;
use symbols::SymbolHandle;
use typed_trees::{
    signature::StateParameter,
    types::{PrimitiveType, TypeReferenceHandle},
};
use validation::{CanonicalPlace, SelectedScalarIntervalSource};

pub(crate) struct LivePlaceIntervals<'a> {
    pub program: &'a dyn ::validation::CheckingInput,
    pub semantic: &'a FactPlan,
    pub contexts: &'a [FactContextHandle],
    pub parameters: &'a [StateParameter],
    pub symbols: &'a [SymbolHandle],
    /// The state whose declarations own `symbols`; used to recover the declared
    /// type of nonlocal storage reads (owned formals and mutable locals).
    pub state: SymbolHandle,
}

impl LivePlaceIntervals<'_> {
    fn snapshot(&self, place: &CanonicalPlace) -> Option<IntegerRange> {
        crate::values::snapshots::integer_bounds_at_place(
            self.program,
            self.semantic,
            self.contexts
                .iter()
                .map(|context| self.semantic.contexts.get(*context)),
            place,
        )
    }

    /// Bounds for one resolved operand place. Live snapshots and standing
    /// invariants both hold at this read; neither replays an earlier initializer.
    pub(crate) fn interval_at_place(
        &self,
        place: &CanonicalPlace,
        primitive_type: PrimitiveType,
    ) -> Option<IntegerRange> {
        let standing = match place.root {
            facts::PlaceRoot::Symbol(symbol) if place.segments.is_empty() => {
                self.standing_interval(symbol, primitive_type)
            }
            _ => validation::carrier_integer_range(primitive_type),
        };
        match (self.snapshot(place), standing) {
            (Some(snapshot), Some(standing)) => {
                let minimum = snapshot.minimum.max(standing.minimum);
                let maximum = snapshot.maximum.min(standing.maximum);
                (minimum <= maximum).then_some(IntegerRange { minimum, maximum })
            }
            (snapshot, standing) => snapshot.or(standing),
        }
    }

    fn standing_interval(
        &self,
        symbol: SymbolHandle,
        primitive_type: PrimitiveType,
    ) -> Option<IntegerRange> {
        let mut bounds = self
            .declared_type(symbol)
            .and_then(|reference| {
                validation::frozen_declared_integer_range(self.program, reference, primitive_type)
            })
            .or_else(|| validation::carrier_integer_range(primitive_type))?;
        // An immutable owned formal retains its arrival contract throughout
        // this state. Mutable or borrowed storage must use live evidence, not
        // an assumption about the value that arrived before intervening writes.
        if self.parameters.iter().any(|parameter| {
            parameter.symbol == symbol
                && !parameter.is_mutable
                && !exclusive_reference(self.program, parameter.type_reference)
                && self
                    .program
                    .primitive_type_reference(parameter.type_reference)
                    == Some(primitive_type)
        }) && let Some((machine, state)) = validation::find_state(self.program, self.state)
            && let Some((minimum, maximum)) =
                validation::state_parameter_integer_interval(self.program, machine, state, symbol)
        {
            if let Some(minimum) = minimum {
                bounds.minimum = bounds.minimum.max(BigInt::from_i64(minimum));
            }
            if let Some(maximum) = maximum {
                bounds.maximum = bounds.maximum.min(BigInt::from_i64(maximum));
            }
        }
        (bounds.minimum <= bounds.maximum).then_some(bounds)
    }

    /// The resolved place, the field's declared type, and whether the whole
    /// path is frozen (immutable end to end). Declared constraints only hold
    /// as read invariants on frozen storage: a mutable formal or exclusive
    /// reference can be reborrowed through a pointee type that drops them.
    fn field(
        &self,
        position: u32,
        path: &[CheckedStructuralPredicatePathSegment],
    ) -> Option<(CanonicalPlace, TypeReferenceHandle, bool)> {
        let (symbol, segments, reference, frozen) =
            crate::values::resolve_structural_parameter_path(
                self.program,
                self.parameters,
                position,
                path,
            )?;
        let mut place = canonical_place_from_symbol(symbol)?;
        place.segments = segments;
        Some((place, reference, frozen))
    }

    /// The declared type behind a storage symbol: a frozen formal or immutable
    /// local declared in this state. Mutable and exclusive-reference storage
    /// can be reborrowed into a call whose own pointee type drops declared
    /// constraints, so they keep the raw carrier instead.
    fn declared_type(&self, symbol: SymbolHandle) -> Option<TypeReferenceHandle> {
        if let Some(parameter) = self
            .parameters
            .iter()
            .find(|parameter| parameter.symbol == symbol)
        {
            return (!parameter.is_mutable
                && !exclusive_reference(self.program, parameter.type_reference))
            .then_some(parameter.type_reference);
        }
        let state = self.program.state_by_symbol(self.state)?;
        let (_, local) = self.program.state_local_declaration(state, symbol)?;
        (!local.is_mutable && !exclusive_reference(self.program, local.type_reference))
            .then_some(local.type_reference)
    }
}

impl SelectedScalarIntervalSource for LivePlaceIntervals<'_> {
    fn binding(&mut self, position: usize, primitive_type: PrimitiveType) -> Option<IntegerRange> {
        self.storage(*self.symbols.get(position)?, primitive_type)
    }

    fn storage(
        &mut self,
        symbol: SymbolHandle,
        primitive_type: PrimitiveType,
    ) -> Option<IntegerRange> {
        let place = canonical_place_from_symbol(symbol)?;
        self.interval_at_place(&place, primitive_type)
    }

    fn structural_field(
        &mut self,
        position: u32,
        path: &[CheckedStructuralPredicatePathSegment],
    ) -> Option<IntegerRange> {
        let (place, reference, frozen) = self.field(position, path)?;
        self.snapshot(&place).or_else(|| {
            // A constant index below a byte carrier is the same read the
            // dynamic indexed plan serves: when no snapshot sits at the
            // indexed place itself, a live whole-carrier literal supplies the
            // element. Static capacity and later writes never substitute.
            if let Some((CheckedStructuralPredicatePathSegment::FixedIndex(index), parent)) =
                path.split_last()
            {
                let index = IntegerRange {
                    minimum: BigInt::from_u64(*index),
                    maximum: BigInt::from_u64(*index),
                };
                if let Some(bounds) = self.indexed_field(position, parent, Some(&index)) {
                    return Some(bounds);
                }
            }
            // An exact typed structural field keeps its declared storage
            // invariant when no narrower value snapshot is live, then its
            // complete carrier. Mutable storage can be reborrowed through a
            // pointee type that drops declared constraints, so only its
            // carrier remains. This is not a declaration-derived domain
            // membership or initialization proof.
            let primitive_type = self.program.primitive_type_reference(reference)?;
            if frozen {
                validation::frozen_declared_integer_range(self.program, reference, primitive_type)
            } else {
                None
            }
            .or_else(|| validation::carrier_integer_range(primitive_type))
        })
    }

    fn byte_length(
        &mut self,
        position: u32,
        path: &[CheckedStructuralPredicatePathSegment],
    ) -> Option<IntegerRange> {
        let length = self.field(position, path).and_then(|(place, _, _)| {
            let literal = crate::values::literal_at_place(
                self.program,
                self.semantic,
                self.contexts
                    .iter()
                    .map(|context| self.semantic.contexts.get(*context)),
                &place,
            )?;
            let bytes = typed_trees::byte_predicates::literal_bytes(self.program, literal)?;
            u64::try_from(bytes.len()).ok().map(BigInt::from_u64)
        });
        // A completed whole-carrier literal is the live length snapshot: it
        // names neither static capacity nor a declared bound, and storage
        // invalidation retires it under the same writes as the carrier's
        // value. Any weaker observation keeps the `u64` carrier bounds.
        length
            .map(|length| IntegerRange {
                minimum: length.clone(),
                maximum: length,
            })
            .or_else(|| validation::carrier_integer_range(PrimitiveType::U64))
    }

    fn indexed_field(
        &mut self,
        position: u32,
        path: &[CheckedStructuralPredicatePathSegment],
        index: Option<&IntegerRange>,
    ) -> Option<IntegerRange> {
        let (place, mut reference, frozen) = self.field(position, path)?;
        // Literal byte reads need an element type, not a nominal text domain.
        // Raw fixed arrays and constrained carriers share the same read rule.
        let element_reference = loop {
            use typed_trees::types::TypeReferenceNode;
            match self.program.type_reference_table.type_reference(reference) {
                TypeReferenceNode::Reference { referee, .. }
                | TypeReferenceNode::Constrained {
                    base_type: referee, ..
                } => reference = *referee,
                TypeReferenceNode::FixedArray { element_type, .. }
                | TypeReferenceNode::Slice { element_type }
                    if self.program.primitive_type_reference(*element_type)
                        == Some(PrimitiveType::U8) =>
                {
                    break *element_type;
                }
                _ => return None,
            }
        };
        let literal_bounds = crate::values::literal_at_place(
            self.program,
            self.semantic,
            self.contexts
                .iter()
                .map(|context| self.semantic.contexts.get(*context)),
            &place,
        )
        .and_then(|literal| {
            let bytes = typed_trees::byte_predicates::literal_bytes(self.program, literal)?;
            // A singleton selects one byte. Otherwise every byte in this live
            // snapshot bounds any successful read; this does not prove the index
            // valid or turn an out-of-bounds access into a normal return.
            let bytes = if let Some(index) = index.filter(|index| index.minimum == index.maximum) {
                let position = usize::try_from(index.minimum.to_u64()?).ok()?;
                std::slice::from_ref(bytes.get(position)?)
            } else {
                &bytes[..]
            };
            Some(IntegerRange {
                minimum: BigInt::from_u64(u64::from(*bytes.iter().min()?)),
                maximum: BigInt::from_u64(u64::from(*bytes.iter().max()?)),
            })
        });
        // Without a live literal, a declared element range is the storage
        // invariant every checked write enforced on every element — but only
        // while the carrier is frozen; mutable storage can be reborrowed
        // through a pointee type that drops declared constraints. Past that,
        // the element's own primitive carrier still bounds any successful
        // read; this proves nothing about the index or the access returning.
        literal_bounds
            .or_else(|| {
                frozen.then(|| {
                    validation::frozen_declared_integer_range(
                        self.program,
                        element_reference,
                        PrimitiveType::U8,
                    )
                })?
            })
            .or_else(|| validation::carrier_integer_range(PrimitiveType::U8))
    }
}
