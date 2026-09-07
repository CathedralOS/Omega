use super::*;
use crate::flow::{CanonicalPlace, canonical_place_from_symbol};
use facts::{FactContextHandle, FactPlan};
use typed_trees::{TypedTrees, expression::ExpressionNode, signature::StateParameter};

pub(crate) struct PlaceIntegerBounds<'a> {
    pub program: &'a TypedTrees,
    pub semantic: &'a FactPlan,
    pub contexts: &'a [FactContextHandle],
    pub parameters: &'a [StateParameter],
    pub symbols: &'a [SymbolHandle],
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

    fn field(
        &self,
        position: u32,
        path: &[CheckedStructuralPredicatePathSegment],
    ) -> Option<(CanonicalPlace, typed_trees::types::TypeReferenceHandle)> {
        let (symbol, segments, reference) = crate::values::resolve_structural_parameter_path(
            self.program,
            self.parameters,
            position,
            path,
        )?;
        let mut place = canonical_place_from_symbol(symbol)?;
        place.segments = segments;
        Some((place, reference))
    }
}

impl IntegerBoundsSource for PlaceIntegerBounds<'_> {
    fn binding(&mut self, position: usize) -> Option<IntegerRange> {
        self.storage(*self.symbols.get(position)?)
    }

    fn storage(&mut self, symbol: SymbolHandle) -> Option<IntegerRange> {
        self.bounds(&canonical_place_from_symbol(symbol)?)
    }

    fn structural_field(
        &mut self,
        position: u32,
        path: &[CheckedStructuralPredicatePathSegment],
    ) -> Option<IntegerRange> {
        self.bounds(&self.field(position, path)?.0)
    }

    fn indexed_field(
        &mut self,
        position: u32,
        path: &[CheckedStructuralPredicatePathSegment],
        index: Option<&IntegerRange>,
    ) -> Option<IntegerRange> {
        let (place, mut reference) = self.field(position, path)?;
        // Literal byte reads need an element type, not a nominal text domain.
        // Raw fixed arrays and constrained carriers share the same read rule.
        loop {
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
                    break;
                }
                _ => return None,
            }
        }
        let literal = crate::values::literal_at_place(
            self.program,
            self.semantic,
            self.contexts
                .iter()
                .map(|context| self.semantic.contexts.get(*context)),
            &place,
        )?;
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
    }
}
