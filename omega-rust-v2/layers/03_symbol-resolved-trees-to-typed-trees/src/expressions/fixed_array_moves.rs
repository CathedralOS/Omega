//! `move items[k]` transfers one complete element of an owned fixed array.
//!
//! The selection records only its typed place step beneath the enclosing
//! `Move` obligation. It reads nothing, opens no loan and selects no core
//! collection primitive, so it carries no runtime bound. Terminal lowering
//! must freeze the selector to a literal ordinal below the declared extent
//! and derive the residual schedule; any other selector rejects there.
use crate::expressions::body_typer::BodyTyper;
use crate::type_error::SourceTypeError;
use crate::type_relations::TypeRelations;
use crate::type_system::substitution::TypeInference;
use crate::{TypeError, ValueType};
use omega_syntax::NodeKind;

impl BodyTyper<'_, '_> {
    /// Type the direct operand of an explicit move. Only an index whose
    /// receiver is an owned fixed array of non-copy elements changes meaning;
    /// every other operand keeps its ordinary typing, including copy reads.
    pub(super) fn moved_operand(
        &mut self,
        node: usize,
    ) -> Result<ValueType, TypeError> {
        let source = self.unit.file;
        if source.nodes[source.ungroup(node)].kind != NodeKind::Index {
            return self.type_expression(node);
        }
        self.index_move_place(node, super::indexing::IndexUse::MoveOperand)
    }

    pub(super) fn index_move_place(
        &mut self,
        node: usize,
        use_site: super::indexing::IndexUse,
    ) -> Result<ValueType, TypeError> {
        let source = self.unit.file;
        // Groups nest as deeply as L1 admits; see :ExpressionLayers.
        if source.nodes[node].kind == NodeKind::Paren {
            let (inner, groups) = self.unit.grouped_operand(node)?;
            let value = self.index_move_place(inner, use_site)?;
            for group in groups.into_iter().rev() {
                let inner = source.nodes[group].children[0];
                self.set_origin(group, self.origin(inner));
                self.places.insert(group, self.storage_place(inner));
                self.record_expression(group, value.clone());
            }
            return Ok(value);
        }
        let value = self.index_operand(node, None, use_site)?;
        value
            .validate_closed_indices()
            .map_err(|reason| TypeError::at(source, node, reason))?;
        self.catalog
            .types
            .indices
            .retain_demands_at(source.node_id(node), &value);
        self.record_expression_with_static_argument(node, value.clone(), None);
        Ok(value)
    }

    /// The element place of an owned array. Its receiver was already typed
    /// as an unborrowed place; the selector lands as the ordinary `u64` index.
    pub(super) fn owned_element(
        &mut self,
        node: usize,
        base: usize,
        selector: usize,
        element: ValueType,
    ) -> Result<ValueType, TypeError> {
        let source = self.unit.file;
        let index = ValueType::integer("u64").expect("u64 index carrier");
        let actual = self.type_expression_with_expected_type(selector, Some(&index))?;
        self.check_destination(selector, &actual, &index)?;
        self.record_place(node, base, None, Some(crate::Projection::Index(source.node_id(selector))));
        let origin = self.origin(base).whole();
        self.set_origin(node, origin);
        let place = self.storage_place(base);
        self.places.insert(node, place);
        Ok(element)
    }
}
