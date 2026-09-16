use super::{
    BigInt, CheckedStructuralPredicatePathSegment, IntegerBoundsSource, IntegerRange,
    PrimitiveType, SymbolHandle,
};
use crate::flow::{CanonicalPlace, canonical_place_from_symbol};
use crate::values::bounds::declared_bounds;
use crate::values::bounds::primitive_range;
use crate::values::exclusive_reference;
use facts::{FactContextHandle, FactPlan};
use typed_trees::{
    TypedTrees, expression::ExpressionNode, signature::StateParameter, statement::StatementNode,
    types::TypeReferenceHandle,
};

pub(crate) struct PlaceIntegerBounds<'a> {
    pub program: &'a TypedTrees,
    pub semantic: &'a FactPlan,
    pub contexts: &'a [FactContextHandle],
    pub parameters: &'a [StateParameter],
    pub symbols: &'a [SymbolHandle],
    /// The state whose declarations own `symbols`; used to recover the declared
    /// type of nonlocal storage reads (owned formals and mutable locals).
    pub state: SymbolHandle,
}

impl PlaceIntegerBounds<'_> {
    fn bounds(&self, place: &CanonicalPlace) -> Option<IntegerRange> {
        crate::values::snapshots::integer_bounds_at_place(
            self.program,
            self.semantic,
            self.contexts
                .iter()
                .map(|context| self.semantic.contexts.get(*context)),
            place,
        )
    }

    /// Bounds for one resolved operand place: a live snapshot first, then the
    /// declared storage invariant a bare frozen root still carries, and
    /// finally the raw carrier. Segment paths and nonlocal storage keep only
    /// the levels that survive reborrowing.
    pub(crate) fn bounds_at_place(
        &self,
        place: &CanonicalPlace,
        primitive_type: PrimitiveType,
    ) -> Option<IntegerRange> {
        self.bounds(place).or_else(|| {
            let declared = match place.root {
                facts::PlaceRoot::Symbol(symbol) if place.segments.is_empty() => {
                    self.declared_type(symbol)
                }
                _ => None,
            };
            declared
                .and_then(|reference| declared_bounds(self.program, reference, primitive_type))
                .or_else(|| primitive_range(primitive_type))
        })
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
        let state = crate::semantic_calls::find_state(self.program, self.state)?;
        program_statements(self.program, state)
            .iter()
            .find_map(|statement| match statement {
                StatementNode::LocalData(local)
                    if local.symbol == symbol
                        && !local.is_mutable
                        && !exclusive_reference(self.program, local.type_reference) =>
                {
                    Some(local.type_reference)
                }
                _ => None,
            })
    }
}

fn program_statements<'a>(
    program: &'a TypedTrees,
    state: &'a typed_trees::state::State,
) -> &'a [StatementNode] {
    program.statement_table.statements(state.statement_nodes)
}

impl IntegerBoundsSource for PlaceIntegerBounds<'_> {
    fn binding(&mut self, position: usize, primitive_type: PrimitiveType) -> Option<IntegerRange> {
        self.storage(*self.symbols.get(position)?, primitive_type)
    }

    fn storage(
        &mut self,
        symbol: SymbolHandle,
        primitive_type: PrimitiveType,
    ) -> Option<IntegerRange> {
        let place = canonical_place_from_symbol(symbol)?;
        self.bounds(&place).or_else(|| {
            // A binding still has its complete carrier range when no narrower
            // value snapshot is live. A declared range constraint is the
            // storage invariant every checked write enforced, so it tightens
            // that fallback; neither is a domain-membership or initialization
            // proof. A live snapshot takes precedence because it is tighter.
            self.declared_type(symbol)
                .and_then(|reference| declared_bounds(self.program, reference, primitive_type))
                .or_else(|| primitive_range(primitive_type))
        })
    }

    fn structural_field(
        &mut self,
        position: u32,
        path: &[CheckedStructuralPredicatePathSegment],
    ) -> Option<IntegerRange> {
        let (place, reference, frozen) = self.field(position, path)?;
        self.bounds(&place).or_else(|| {
            // An exact typed structural field keeps its declared storage
            // invariant when no narrower value snapshot is live, then its
            // complete carrier. Mutable storage can be reborrowed through a
            // pointee type that drops declared constraints, so only its
            // carrier remains. This is not a declaration-derived domain
            // membership or initialization proof.
            let primitive_type = self.program.primitive_type_reference(reference)?;
            if frozen {
                declared_bounds(self.program, reference, primitive_type)
            } else {
                None
            }
            .or_else(|| primitive_range(primitive_type))
        })
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
            let ExpressionNode::String(bytes) = self.program.expression_table.expression(literal)
            else {
                return None;
            };
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
                frozen
                    .then(|| declared_bounds(self.program, element_reference, PrimitiveType::U8))?
            })
            .or_else(|| primitive_range(PrimitiveType::U8))
    }
}
