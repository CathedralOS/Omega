//! Values established in reserved storage, in establishment order.
//!
//! Reserving gated storage is not whole-value establishment. Keep only the
//! established subtrees; finishing an aggregate folds those children into one
//! value, after which the ordinary recursive declaration-order cleanup applies.
//! Arrays remain sparse until complete: untouched gated elements are never
//! enumerated. This is source permission-production scratch, forked at edges,
//! not a runtime liveness map or a second ownership replay.

use facts::PlaceSegment;
use language_semantics::Multiplicity;
use typed_trees::data::DataMember;
use typed_trees::types::{FixedArrayLength, TypeReferenceHandle, TypeReferenceNode};

/// The first case mismatch in a zero value's exact closed structural path.
/// Both claim production and residual cleanup use this absence: a nonselected
/// payload is not a moved child and cannot suppress its owner's nominal hook.
/// Unresolved projections prove no absence, and an active linear payload is
/// never established by the zero representation.
pub(super) fn zero_image_inactive_case<'path>(
    program: &dyn ::validation::CheckingInput,
    reference: TypeReferenceHandle,
    path: &'path [PlaceSegment],
) -> Option<&'path [PlaceSegment]> {
    for (segment_index, segment) in path.iter().enumerate() {
        let PlaceSegment::Case { variant } = segment else {
            continue;
        };
        let projected = validation::project_type_reference_from_segments(
            program,
            reference,
            &path[..segment_index],
        )?;
        let definition = program
            .closed_data_definition(validation::peel_declared_constraints(program, projected))?;
        let zero_case = program
            .data_members(definition)
            .iter()
            .find_map(|member| member.as_variant())?;
        if zero_case.symbol != *variant {
            return Some(&path[..=segment_index]);
        }
    }
    None
}

#[derive(Clone, PartialEq, Eq)]
pub(super) struct ConstructionFrontier {
    reference: TypeReferenceHandle,
    pub(super) established: Vec<Vec<PlaceSegment>>,
}

impl ConstructionFrontier {
    pub(super) fn new(
        program: &dyn ::validation::CheckingInput,
        reference: TypeReferenceHandle,
    ) -> Self {
        let mut frontier = Self {
            reference,
            established: Vec::new(),
        };
        frontier.seed_zero_values(program, reference, &mut Vec::new());
        frontier
    }

    fn seed_zero_values(
        &mut self,
        program: &dyn ::validation::CheckingInput,
        reference: TypeReferenceHandle,
        path: &mut Vec<PlaceSegment>,
    ) {
        // ownership.md, Establishment and erasure: implicit zero-filling
        // establishes no linear obligation. A nominal linear owner must be
        // explicitly established, even when all of its bytes admit zero.
        if nominal_linear_owner(program, reference) {
            return;
        }
        if !requires_owned_establishment(program, reference) {
            self.established.push(path.clone());
            return;
        }
        // A gated or claim-bearing array has no established zero element. Do
        // not expand its literal dimension to discover an empty live set.
        for child_path in record_children(program, reference).unwrap_or_default() {
            let parent_length = path.len();
            path.extend(child_path);
            if let Some(child) =
                validation::project_type_reference_from_segments(program, self.reference, path)
            {
                self.seed_zero_values(program, child, path);
            }
            path.truncate(parent_length);
        }
        // Zero selects the first case, not every possible payload. Once its
        // actual children are established, a valid-zero transparent record
        // is whole even when that selected case has no children at all.
        if !validation::type_requires_establishment(program, reference)
            && (record_children(program, reference).is_some_and(|children| children.is_empty())
                || self.complete(program, reference, path))
        {
            self.replace_subtrees(path);
        }
    }

    pub(super) fn cleanup_free(
        &self,
        program: &dyn ::validation::CheckingInput,
        path: &[PlaceSegment],
    ) -> bool {
        validation::project_type_reference_from_segments(program, self.reference, path).is_some_and(
            |reference| program.type_multiplicity(reference) == Multiplicity::Unrestricted,
        )
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

    pub(super) fn establish(
        &mut self,
        program: &dyn ::validation::CheckingInput,
        path: &[PlaceSegment],
    ) {
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
                validation::project_type_reference_from_segments(program, self.reference, prefix)
            else {
                break;
            };
            if !self.complete(program, reference, prefix) {
                break;
            }
            self.replace_subtrees(prefix);
        }
    }

    /// Join already established values without replaying their zero-filled
    /// children as validated writes. Only complete direct children can finish
    /// the containing value; a gated child's raw fields do not establish it.
    pub(super) fn completes_with(
        &self,
        program: &dyn ::validation::CheckingInput,
        later: &[Vec<PlaceSegment>],
        receiving_writes: &[Vec<PlaceSegment>],
    ) -> bool {
        let mut combined = self.clone();
        for path in later {
            if !combined.contains(path) {
                combined.replace_subtrees(path);
            }
        }
        // A validated receiving write may complete nested parents against
        // retained sibling values. Seeded zero leaves carry no such write:
        // replaying those would invent a valid gated containing value.
        for path in receiving_writes {
            combined.establish(program, path);
        }
        combined.contains(&[])
    }

    fn replace_subtrees(&mut self, path: &[PlaceSegment]) {
        self.established.retain(|prior| !prior.starts_with(path));
        self.established.push(path.to_vec());
    }

    fn complete(
        &self,
        program: &dyn ::validation::CheckingInput,
        reference: TypeReferenceHandle,
        path: &[PlaceSegment],
    ) -> bool {
        // Filling a nominal linear owner's fields cannot mint its separate
        // whole-value obligation. An explicit store to that owner already
        // establishes the endpoint before this ancestor-completion walk.
        if nominal_linear_owner(program, reference) {
            return false;
        }
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
        record_children(program, reference).is_some_and(|children| {
            !children.is_empty()
                && children.into_iter().all(|segments| {
                    let mut child = path.to_vec();
                    child.extend(segments);
                    self.contains(&child)
                })
        })
    }
}

/// Arrays inherit element multiplicity but own only element claims. Only
/// a nominal type (including a bounded type parameter) can own a root claim.
fn nominal_linear_owner(
    program: &dyn ::validation::CheckingInput,
    reference: TypeReferenceHandle,
) -> bool {
    program
        .unwrapped_type_reference(reference)
        .is_some_and(|reference| {
            matches!(
                program.type_reference_table.type_reference(reference),
                TypeReferenceNode::Named { .. } | TypeReferenceNode::Generic { .. }
            ) && program.type_multiplicity(reference) == Multiplicity::Linear
        })
}

pub(super) fn transparent_storage(
    program: &dyn ::validation::CheckingInput,
    reference: TypeReferenceHandle,
) -> bool {
    program.type_multiplicity(reference) == Multiplicity::Affine
        || program
            .unwrapped_type_reference(reference)
            .is_some_and(|reference| {
                matches!(
                    program.type_reference_table.type_reference(reference),
                    TypeReferenceNode::FixedArray { .. }
                )
            })
}

/// Default-domain validity and owned establishment are separate judgments.
/// Transparent storage containing claims is reserved even if its zero bits
/// satisfy every value-domain predicate.
pub(super) fn requires_owned_establishment(
    program: &dyn ::validation::CheckingInput,
    reference: TypeReferenceHandle,
) -> bool {
    validation::type_requires_establishment(program, reference)
        || super::type_carries_linear_obligation(program, reference)
}

fn record_children(
    program: &dyn ::validation::CheckingInput,
    reference: TypeReferenceHandle,
) -> Option<Vec<Vec<PlaceSegment>>> {
    let reference = program.unwrapped_type_reference(reference)?;
    let symbol = program.type_reference_table.type_head_symbol(reference)?;
    let definition = program.data_definition_by_symbol(symbol)?;
    let mut children = Vec::new();
    for member in program.data_members(definition) {
        if let Some(field) = member.as_field() {
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
    Some(children)
}
