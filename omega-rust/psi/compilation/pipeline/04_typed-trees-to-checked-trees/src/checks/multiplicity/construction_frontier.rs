//! Values established in reserved storage, in establishment order.
//!
//! Reserving gated storage is not whole-value establishment. Keep only the
//! established subtrees; finishing an aggregate folds those children into one
//! value, after which the ordinary recursive declaration-order cleanup applies.
//! Arrays remain sparse until complete: untouched gated elements are never
//! enumerated. This is source permission-production scratch, forked at edges,
//! not a runtime liveness map or a second ownership replay.

use facts::PlaceSegment;
use typed_trees::TypedTrees;
use typed_trees::data::DataMember;
use typed_trees::types::{FixedArrayLength, TypeReferenceHandle, TypeReferenceNode};

#[derive(Clone)]
pub(super) struct ConstructionFrontier {
    reference: TypeReferenceHandle,
    pub(super) established: Vec<Vec<PlaceSegment>>,
}

impl ConstructionFrontier {
    pub(super) fn new(program: &TypedTrees, reference: TypeReferenceHandle) -> Self {
        let mut frontier = Self {
            reference,
            established: Vec::new(),
        };
        frontier.seed_zero_values(program, reference, &mut Vec::new());
        frontier
    }

    fn seed_zero_values(
        &mut self,
        program: &TypedTrees,
        reference: TypeReferenceHandle,
        path: &mut Vec<PlaceSegment>,
    ) {
        if !validation::type_requires_establishment(program, reference) {
            self.established.push(path.clone());
            return;
        }
        // A gated array has no established zero element. In particular, do
        // not expand its literal dimension to discover an empty live set.
        for child_path in record_children(program, reference) {
            let parent_length = path.len();
            path.extend(child_path);
            if let Some(child) =
                crate::flow::project_type_reference_from_segments(program, self.reference, path)
            {
                self.seed_zero_values(program, child, path);
            }
            path.truncate(parent_length);
        }
    }

    pub(super) fn contains(&self, path: &[PlaceSegment]) -> bool {
        self.established
            .iter()
            .any(|established| path.starts_with(established))
    }

    pub(super) fn retire(&mut self, path: &[PlaceSegment]) {
        // A complete ancestor keeps its declaration-ordered residual model;
        // the separate retirement set records the extracted child. An exact
        // unfinished-construction child leaves the establishment roster, so a
        // later replacement occupies its new establishment position.
        self.established
            .retain(|established| !established.starts_with(path));
    }

    pub(super) fn establish(&mut self, program: &TypedTrees, path: &[PlaceSegment]) {
        if self
            .established
            .iter()
            .any(|established| established.len() < path.len() && path.starts_with(established))
        {
            return;
        }
        self.replace_subtrees(path);
        // A valid store establishes its endpoint. Ancestors finish only when
        // every contained value is established. Default-domain write checking
        // still owns validity and invariant-window closure; this walk grants
        // no mathematical facts and cannot waive that check.
        for length in (0..path.len()).rev() {
            let prefix = &path[..length];
            let Some(reference) =
                crate::flow::project_type_reference_from_segments(program, self.reference, prefix)
            else {
                break;
            };
            if !self.complete(program, reference, prefix) {
                break;
            }
            self.replace_subtrees(prefix);
        }
    }

    fn replace_subtrees(&mut self, path: &[PlaceSegment]) {
        self.established.retain(|prior| !prior.starts_with(path));
        self.established.push(path.to_vec());
    }

    fn complete(
        &self,
        program: &TypedTrees,
        reference: TypeReferenceHandle,
        path: &[PlaceSegment],
    ) -> bool {
        let Some(reference) = program.unwrapped_type_reference(reference) else {
            return false;
        };
        if let TypeReferenceNode::FixedArray {
            length: FixedArrayLength::Literal(length),
            ..
        } = program.type_reference_table.type_reference(reference)
        {
            // The antichain contains disjoint, canonical in-bounds indexes.
            // Count established direct elements rather than scanning length.
            return self.established.iter().filter(|child| {
                child.len() == path.len() + 1
                    && child.starts_with(path)
                    && matches!(child.last(), Some(PlaceSegment::FixedIndex { index }) if *index < *length)
            }).count() == *length;
        }
        let children = record_children(program, reference);
        !children.is_empty()
            && children.into_iter().all(|segments| {
                let mut child = path.to_vec();
                child.extend(segments);
                self.contains(&child)
            })
    }
}

fn record_children(program: &TypedTrees, reference: TypeReferenceHandle) -> Vec<Vec<PlaceSegment>> {
    let Some(reference) = program.unwrapped_type_reference(reference) else {
        return Vec::new();
    };
    let symbol = match program.type_reference_table.type_reference(reference) {
        TypeReferenceNode::Named { symbol, .. } => *symbol,
        TypeReferenceNode::Generic { base_symbol, .. } => *base_symbol,
        _ => return Vec::new(),
    };
    let Some(definition) = crate::lookup::data_definition_by_symbol(program, symbol) else {
        return Vec::new();
    };
    let mut children = Vec::new();
    for member in program.data_members(definition) {
        if let DataMember::Field(field) = member {
            children.push(vec![PlaceSegment::Field {
                symbol: field.symbol,
            }]);
        }
    }
    // Zero storage selects only the first case. Its payload is another
    // subtree, not the union of all possible cases.
    if let Some(variant) = program
        .data_members(definition)
        .iter()
        .find_map(|member| match member {
            DataMember::Variant(variant) => Some(variant),
            DataMember::Field(_) => None,
        })
    {
        let case = PlaceSegment::Case {
            variant: variant.symbol,
        };
        for field in program.data_payload_fields(variant) {
            children.push(vec![
                case,
                PlaceSegment::Field {
                    symbol: field.symbol,
                },
            ]);
        }
    }
    children
}
