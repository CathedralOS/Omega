//! Structural source selectors retained independently of coarse write paths.
//! These identify a possible source; declaration, reference-boundary, and
//! access checks remain with the origin consumer.

use facts::{FactPlan, PlaceRoot, PlaceSegment};
use symbols::{SymbolHandle, SymbolKind};
use typed_trees::TypedTrees;
use typed_trees::expression::ExpressionHandle;
use typed_trees::machine::Machine;
use typed_trees::state::State;

use crate::machine_calls::calls::write_frames::caller_aliases::{
    CallerWriteSite, caller_statement_owner,
};

/// Storage category is independent of coordinate precision. Static backing
/// comes only from a literal producer, never from an absent declaration root.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(in crate::machine_calls::calls::write_frames) enum FrameSourceKind {
    #[default]
    Declaration,
    ImageStorage,
}

/// One evaluated recast's source interpretation. This is a flat coordinate
/// snapshot, not a recursively cloned origin graph. Later projections extend
/// only the current view; they cannot rewrite this predecessor's selectors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecastSource {
    pub root: SymbolHandle,
    pub segments: Vec<PlaceSegment>,
    pub representation: Option<(typed_trees::types::TypeReferenceHandle, Vec<PlaceSegment>)>,
    pub target_type: typed_trees::types::TypeReferenceHandle,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(in crate::machine_calls::calls::write_frames) struct FrameSourcePlace {
    pub(in crate::machine_calls::calls::write_frames) kind: FrameSourceKind,
    pub(in crate::machine_calls::calls::write_frames) root: SymbolHandle,
    pub(in crate::machine_calls::calls::write_frames) segments: Vec<PlaceSegment>,
    /// The logical carrier selected by a recast, independent of raw backing.
    pub(in crate::machine_calls::calls::write_frames) representation:
        Option<(typed_trees::types::TypeReferenceHandle, Vec<PlaceSegment>)>,
    pub(in crate::machine_calls::calls::write_frames) recasts: Vec<RecastSource>,
    /// Permission to interpret candidate selectors as primitive coordinates.
    /// False retains possible-source metadata for the coarse frame consumers.
    pub(in crate::machine_calls::calls::write_frames) builtin_coordinates: bool,
}

impl FrameSourcePlace {
    pub(in crate::machine_calls::calls::write_frames) fn image_storage() -> Self {
        Self {
            kind: FrameSourceKind::ImageStorage,
            builtin_coordinates: true,
            ..Self::default()
        }
    }

    pub(in crate::machine_calls::calls::write_frames) fn is_static(&self) -> bool {
        self.kind == FrameSourceKind::ImageStorage
    }

    pub(in crate::machine_calls::calls::write_frames) fn from_expression(
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
        let segments = facts.place_segments.span_or_empty(place.segments);
        let builtin_coordinates = !segments_have_index_geometry(segments)
            || source_has_builtin_coordinates(program, root, expression);
        Self {
            kind: FrameSourceKind::Declaration,
            root,
            segments: segments.to_vec(),
            representation: None,
            recasts: Vec::new(),
            builtin_coordinates,
        }
    }

    pub(in crate::machine_calls::calls::write_frames) fn recast(
        &mut self,
        target_type: typed_trees::types::TypeReferenceHandle,
    ) {
        self.recasts.push(RecastSource {
            root: self.root,
            segments: self.segments.clone(),
            representation: self.representation.clone(),
            target_type,
        });
        self.representation = Some((target_type, Vec::new()));
    }

    /// Append schema-derived selectors. Expression-derived selectors must
    /// also carry their meaning flag via append_source or explicit conjunction.
    pub(in crate::machine_calls::calls::write_frames) fn append_segments(
        &self,
        segments: &[PlaceSegment],
    ) -> Self {
        if !self.root.is_valid() && !self.is_static() {
            return Self::default();
        }
        let mut projected = self.clone();
        projected.segments.extend_from_slice(segments);
        if let Some((_, path)) = &mut projected.representation {
            path.extend_from_slice(segments);
        }
        projected
    }

    pub(in crate::machine_calls::calls::write_frames) fn append_source(
        &self,
        source: &Self,
    ) -> Self {
        let mut projected = self.append_segments(&source.segments);
        projected.builtin_coordinates &= source.builtin_coordinates;
        projected
    }

    /// The source may already refer to a different binding after alias or
    /// helper substitution. Compare the original expression places, then
    /// append only the projection beyond the original base.
    pub(in crate::machine_calls::calls::write_frames) fn projected(
        &self,
        program: &TypedTrees,
        whole_expression: ExpressionHandle,
        base_expression: ExpressionHandle,
    ) -> Self {
        if !self.root.is_valid() && !self.is_static() {
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
        let whole_segments = facts.place_segments.span_or_empty(whole.segments);
        let base_segments = facts.place_segments.span_or_empty(base.segments);
        let Some(suffix) = whole_segments.strip_prefix(base_segments) else {
            return Self::default();
        };
        // The base origin is already proven, including helper-result bases.
        // Only newly appended index geometry requires operation-meaning custody.
        let mut projected = self.append_segments(suffix);
        if segments_have_index_geometry(suffix) {
            projected.builtin_coordinates &= match whole.root {
                PlaceRoot::Symbol(root) => {
                    source_has_builtin_coordinates(program, root, whole_expression)
                }
                _ => false,
            };
        }
        projected
    }

    /// Substitute a proven relative source beneath this caller source.
    /// Runtime indexes belong to the callee's expression namespace, so retain
    /// their possible-element meaning without retaining executable handles.
    pub(in crate::machine_calls::calls::write_frames) fn append_relative(
        &self,
        relative: &Self,
    ) -> Self {
        if (!self.root.is_valid() && !self.is_static()) || !relative.root.is_valid() {
            return Self::default();
        }
        let mut result = self.clone();
        result.builtin_coordinates &= relative.builtin_coordinates;
        let base_coordinate = Self {
            kind: self.kind,
            root: self.root,
            segments: self.segments.clone(),
            representation: self.representation.clone(),
            builtin_coordinates: self.builtin_coordinates,
            recasts: Vec::new(),
        };
        for recast in &relative.recasts {
            let predecessor = Self {
                root: recast.root,
                segments: recast.segments.clone(),
                representation: recast.representation.clone(),
                builtin_coordinates: relative.builtin_coordinates,
                ..Self::default()
            };
            let rebased = base_coordinate.append_relative(&predecessor);
            result.recasts.push(RecastSource {
                root: rebased.root,
                segments: rebased.segments,
                representation: rebased.representation,
                target_type: recast.target_type,
            });
        }
        result
            .segments
            .extend(relative.segments.iter().map(|segment| match segment {
                PlaceSegment::Index { .. } => PlaceSegment::Index {
                    expression: ExpressionHandle::invalid(),
                },
                _ => *segment,
            }));
        if let Some((_, path)) = &mut result.representation {
            path.extend(relative.segments.iter().map(|segment| match segment {
                PlaceSegment::Index { .. } => PlaceSegment::Index {
                    expression: ExpressionHandle::invalid(),
                },
                _ => *segment,
            }));
        }
        // A view introduced inside the callee selects a new logical carrier
        // over the same substituted backing. Its path is relative to that
        // carrier, not to the caller's previous interpretation.
        if let Some((carrier, path)) = &relative.representation {
            result.representation = Some((
                *carrier,
                path.iter()
                    .map(|segment| match segment {
                        PlaceSegment::Index { .. } => PlaceSegment::Index {
                            expression: ExpressionHandle::invalid(),
                        },
                        _ => *segment,
                    })
                    .collect(),
            ));
        }
        result
    }
}

fn segments_have_index_geometry(segments: &[PlaceSegment]) -> bool {
    segments.iter().any(|segment| {
        matches!(
            segment,
            PlaceSegment::FixedIndex { .. }
                | PlaceSegment::FixedRange { .. }
                | PlaceSegment::Index { .. }
        )
    })
}

fn source_has_builtin_coordinates(
    program: &TypedTrees,
    root: SymbolHandle,
    expression: ExpressionHandle,
) -> bool {
    source_owner(program, root, expression).is_some_and(|(machine, state)| {
        crate::place_has_builtin_coordinates(program, machine, Some(state), expression)
    })
}

/// Recover context from declaration ownership, never from a matching name or
/// the first state of an attached machine. A data field's declaration alone
/// cannot identify which attached machine is evaluating the expression.
fn source_owner(
    program: &TypedTrees,
    root: SymbolHandle,
    expression: ExpressionHandle,
) -> Option<(&Machine, &State)> {
    let declaration = program.symbols.get(root);
    // The retained parent names the declaring container, so at most one
    // machine can own `root`: a parameter/local lives under exactly one state
    // of exactly one machine, and a machine field lives under its machine.
    // The whole-program ownership scan collapses to that single candidate.
    match declaration.kind {
        SymbolKind::Parameter | SymbolKind::Local => {
            let state_symbol = declaration.parent;
            let machine_symbol = program.symbols.get(state_symbol).parent;
            let machine = program
                .machines()
                .iter()
                .find(|machine| machine.symbol == machine_symbol)?;
            let state = program
                .machine_states(machine)
                .iter()
                .find(|state| state.symbol == state_symbol)?;
            let retained = match declaration.kind {
                SymbolKind::Parameter => program
                    .state_parameters(state)
                    .iter()
                    .any(|parameter| parameter.symbol == root),
                _ => program.state_local_declaration(state, root).is_some(),
            };
            retained.then_some((machine, state))
        }
        SymbolKind::Machine | SymbolKind::Field => {
            let machine_symbol = match declaration.kind {
                SymbolKind::Machine => root,
                _ => declaration.parent,
            };
            let machine = program
                .machines()
                .iter()
                .find(|machine| machine.symbol == machine_symbol)?;
            let owns_root = match declaration.kind {
                SymbolKind::Machine => true,
                _ => {
                    crate::exact_attached_field(program, machine, root, program.symbols.name(root))
                        .is_some()
                }
            };
            if !owns_root {
                return None;
            }
            let (state, _, _) =
                caller_statement_owner(program, machine, CallerWriteSite::Expression(expression))?;
            Some((machine, state))
        }
        _ => None,
    }
}
