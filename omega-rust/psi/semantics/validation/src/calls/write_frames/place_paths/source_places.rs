//! Structural source selectors retained independently of coarse write paths.
//! These identify a possible source; declaration, reference-boundary, and
//! access checks remain with the origin consumer.

use facts::{FactPlan, PlaceRoot, PlaceSegment};
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::expression::ExpressionHandle;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(in crate::calls::write_frames) struct FrameSourcePlace {
    pub(in crate::calls::write_frames) root: SymbolHandle,
    pub(in crate::calls::write_frames) segments: Vec<PlaceSegment>,
}

impl FrameSourcePlace {
    pub(in crate::calls::write_frames) fn from_expression(
        program: &TypedTrees,
        expression: ExpressionHandle,
    ) -> Self {
        let mut facts = FactPlan::default();
        let place = facts.append_place_from_expression(program, expression);
        let place = facts.places.get(place);
        let PlaceRoot::Symbol(root) = place.root else {
            return Self::default();
        };
        if !root.is_valid() {
            return Self::default();
        }
        Self {
            root,
            segments: facts.place_segments.span_or_empty(place.segments).to_vec(),
        }
    }

    pub(in crate::calls::write_frames) fn append_segments(
        &self,
        segments: &[PlaceSegment],
    ) -> Self {
        if !self.root.is_valid() {
            return Self::default();
        }
        let mut projected = self.clone();
        projected.segments.extend_from_slice(segments);
        projected
    }

    /// The source may already refer to a different binding after alias or
    /// helper substitution. Compare the original expression places, then
    /// append only the projection beyond the original base.
    pub(in crate::calls::write_frames) fn projected(
        &self,
        program: &TypedTrees,
        whole_expression: ExpressionHandle,
        base_expression: ExpressionHandle,
    ) -> Self {
        if !self.root.is_valid() {
            return Self::default();
        }
        let mut facts = FactPlan::default();
        let whole = facts.append_place_from_expression(program, whole_expression);
        let base = facts.append_place_from_expression(program, base_expression);
        let whole = facts.places.get(whole);
        let base = facts.places.get(base);
        let known_base = match base.root {
            PlaceRoot::Symbol(root) => root.is_valid(),
            PlaceRoot::Expression(expression) => expression.is_valid(),
            _ => false,
        };
        if whole.root != base.root || !known_base {
            return Self::default();
        }
        let whole = facts.place_segments.span_or_empty(whole.segments);
        let base = facts.place_segments.span_or_empty(base.segments);
        let Some(suffix) = whole.strip_prefix(base) else {
            return Self::default();
        };
        self.append_segments(suffix)
    }

    /// Substitute a proven relative source beneath this caller source.
    /// Runtime indexes belong to the callee's expression namespace, so retain
    /// their possible-element meaning without retaining executable handles.
    pub(in crate::calls::write_frames) fn append_relative(&self, relative: &Self) -> Self {
        if !self.root.is_valid() || !relative.root.is_valid() {
            return Self::default();
        }
        let mut result = self.clone();
        result
            .segments
            .extend(relative.segments.iter().map(|segment| match segment {
                PlaceSegment::Index { .. } => PlaceSegment::Index {
                    expression: ExpressionHandle::invalid(),
                },
                _ => *segment,
            }));
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arena::HandleSpan;
    use symbols::{SymbolKind, SymbolNameRef, SymbolTableBuilder};
    use typed_trees::expression::{
        ExpressionNode, TableIndexedExpression, TableMemberExpression, TableNamePath,
    };
    use typed_trees::name::Identifier;

    fn name(program: &mut TypedTrees, spelling: &str, symbol: SymbolHandle) -> ExpressionHandle {
        let mut members = HandleSpan::empty();
        program
            .expression_table
            .push_name_path_member(&mut members, Identifier::generated(spelling));
        let mut member_symbols = HandleSpan::empty();
        program
            .expression_table
            .push_name_path_member_symbol(&mut member_symbols, symbol);
        program
            .expression_table
            .insert(ExpressionNode::Name(TableNamePath {
                members,
                member_symbols,
                head_symbol: symbol,
                symbol,
            }))
    }

    fn field_after_index(
        program: &mut TypedTrees,
        collection: ExpressionHandle,
        index: ExpressionHandle,
        field: SymbolHandle,
    ) -> (ExpressionHandle, ExpressionHandle) {
        let indexed =
            program
                .expression_table
                .insert(ExpressionNode::Indexed(TableIndexedExpression {
                    collection,
                    index,
                }));
        let whole =
            program
                .expression_table
                .insert(ExpressionNode::Member(TableMemberExpression {
                    receiver: indexed,
                    member_symbol: field,
                    member: Identifier::generated("view"),
                    case_variant: None,
                }));
        (indexed, whole)
    }

    fn fixture() -> (TypedTrees, Vec<SymbolHandle>) {
        let mut symbols = SymbolTableBuilder::default();
        let root = symbols.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
        let children = symbols.insert_children(
            root,
            [
                (SymbolKind::Local, SymbolNameRef::Static("values")),
                (SymbolKind::Local, SymbolNameRef::Static("index")),
                (SymbolKind::Field, SymbolNameRef::Static("view")),
                (SymbolKind::Parameter, SymbolNameRef::Static("caller")),
            ],
        );
        let handles = SymbolTableBuilder::child_handles(children).collect();
        (
            TypedTrees {
                symbols: symbols.finish(),
                ..TypedTrees::default()
            },
            handles,
        )
    }

    #[test]
    fn coarse_write_path_retains_structural_field_after_index() {
        for runtime in [false, true] {
            let (mut program, symbols) = fixture();
            let collection = name(&mut program, "values", symbols[0]);
            let index = if runtime {
                name(&mut program, "index", symbols[1])
            } else {
                program.expression_table.insert(ExpressionNode::Integer(
                    numerics::literals::IntegerLiteral::from_value(1),
                ))
            };
            let (_, expression) = field_after_index(&mut program, collection, index, symbols[2]);
            let origin = super::super::frame_place_path(&program, expression).expect("place");
            assert_eq!(origin.path, "values");
            assert_eq!(
                origin.precision,
                super::super::FramePathPrecision::CollectionCoarse
            );
            assert_eq!(origin.source.root, symbols[0]);
            assert_eq!(
                origin.source.segments,
                vec![
                    if runtime {
                        PlaceSegment::Index { expression: index }
                    } else {
                        PlaceSegment::FixedIndex { index: 1 }
                    },
                    PlaceSegment::Field { symbol: symbols[2] },
                ]
            );
            assert_eq!(
                super::super::coarse_place_path(&program, expression),
                Some(origin.path)
            );
        }
    }

    #[test]
    fn structural_projection_appends_only_the_normalized_suffix() {
        let (mut program, symbols) = fixture();
        let collection = name(&mut program, "values", symbols[0]);
        let index = name(&mut program, "index", symbols[1]);
        let (indexed, whole) = field_after_index(&mut program, collection, index, symbols[2]);
        let source = FrameSourcePlace {
            root: symbols[3],
            segments: vec![PlaceSegment::FixedIndex { index: 5 }],
        };
        assert_eq!(
            source.projected(&program, whole, indexed),
            FrameSourcePlace {
                root: symbols[3],
                segments: vec![
                    PlaceSegment::FixedIndex { index: 5 },
                    PlaceSegment::Field { symbol: symbols[2] },
                ],
            }
        );
        assert_eq!(
            source.projected(&program, indexed, whole),
            FrameSourcePlace::default()
        );
        let foreign = name(&mut program, "caller", symbols[3]);
        assert_eq!(
            source.projected(&program, whole, foreign),
            FrameSourcePlace::default()
        );
        assert_eq!(
            FrameSourcePlace::default().projected(&program, whole, indexed),
            FrameSourcePlace::default()
        );
    }

    #[test]
    fn relative_source_erases_only_callee_runtime_index_handles() {
        let (mut program, symbols) = fixture();
        let index = name(&mut program, "index", symbols[1]);
        let caller = FrameSourcePlace {
            root: symbols[3],
            segments: vec![PlaceSegment::Index { expression: index }],
        };
        let relative = FrameSourcePlace {
            root: symbols[0],
            segments: vec![
                PlaceSegment::Index { expression: index },
                PlaceSegment::FixedIndex { index: 2 },
                PlaceSegment::Field { symbol: symbols[2] },
            ],
        };
        assert_eq!(
            caller.append_relative(&relative),
            FrameSourcePlace {
                root: symbols[3],
                segments: vec![
                    PlaceSegment::Index { expression: index },
                    PlaceSegment::Index {
                        expression: ExpressionHandle::invalid()
                    },
                    PlaceSegment::FixedIndex { index: 2 },
                    PlaceSegment::Field { symbol: symbols[2] },
                ],
            }
        );
        assert_eq!(
            caller.append_relative(&FrameSourcePlace::default()),
            FrameSourcePlace::default()
        );
        assert_eq!(
            FrameSourcePlace::default().append_relative(&relative),
            FrameSourcePlace::default()
        );
        assert_eq!(
            FrameSourcePlace::from_expression(&program, ExpressionHandle::invalid()),
            FrameSourcePlace::default()
        );
    }
}
