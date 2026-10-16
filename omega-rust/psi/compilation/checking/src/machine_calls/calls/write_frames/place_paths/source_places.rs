//! Structural source selectors retained independently of coarse write paths.
//! These identify a possible source; declaration, reference-boundary, and
//! access checks remain with the origin consumer.

use facts::{PlaceRoot, PlaceSegment};
use symbols::{SymbolHandle, SymbolKind};
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
    /// Exact formation occurrence in its original body. Call/state substitution
    /// rebases storage coordinates, never this issuer into the caller's syntax.
    pub formation_expression: ExpressionHandle,
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
        program: &dyn crate::CheckingInput,
        expression: ExpressionHandle,
    ) -> Self {
        // Use the address algebra, not the raw evidence-place builder:
        // fixed windows must retain their bounds rather than masquerading
        // as runtime element selectors in every later origin consumer.
        let Some(place) = source_coordinates(program, expression) else {
            return Self::default();
        };
        let Some(root) = place.root.as_symbol() else {
            return Self::default();
        };
        if !root.is_valid() {
            return Self::default();
        }
        let segments = place.segments.as_slice();
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
        formation_expression: ExpressionHandle,
    ) {
        self.recasts.push(RecastSource {
            formation_expression,
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
        // Alias substitution remains in this body's expression namespace.
        // Preserve both evaluated selectors and any newly formed views.
        self.substitute_source(source, |segment| segment)
    }

    /// The source may already refer to a different binding after alias or
    /// helper substitution. Compare the original expression places, then
    /// append only the projection beyond the original base.
    pub(in crate::machine_calls::calls::write_frames) fn projected(
        &self,
        program: &dyn crate::CheckingInput,
        whole_expression: ExpressionHandle,
        base_expression: ExpressionHandle,
    ) -> Self {
        if !self.root.is_valid() && !self.is_static() {
            return Self::default();
        }
        let Some(whole) = source_coordinates(program, whole_expression) else {
            return Self::default();
        };
        let Some(base) = source_coordinates(program, base_expression) else {
            return Self::default();
        };
        let known_base = match base.root {
            PlaceRoot::Symbol(root) => root.is_valid(),
            PlaceRoot::Expression(expression) => expression.is_valid(),
            _ => false,
        };
        if whole.root != base.root || !known_base {
            return Self::default();
        }
        let whole_segments = whole.segments.as_slice();
        let base_segments = base.segments.as_slice();
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
    /// their element/window meaning without retaining executable handles.
    pub(in crate::machine_calls::calls::write_frames) fn append_relative(
        &self,
        program: &dyn crate::CheckingInput,
        relative: &Self,
    ) -> Self {
        self.substitute_source(relative, |segment| {
            segment.erase_runtime_selector(|expression| {
                program.expression_table.range_node(expression).is_some()
            })
        })
    }

    /// Rebase the complete presentation chain as well as its final coordinate.
    /// The caller owns whether selectors cross an expression namespace.
    fn substitute_source(
        &self,
        relative: &Self,
        map_selector: impl Fn(PlaceSegment) -> PlaceSegment + Copy,
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
            let rebased = base_coordinate.substitute_source(&predecessor, map_selector);
            result.recasts.push(RecastSource {
                formation_expression: recast.formation_expression,
                root: rebased.root,
                segments: rebased.segments,
                representation: rebased.representation,
                target_type: recast.target_type,
            });
        }
        result
            .segments
            .extend(relative.segments.iter().copied().map(map_selector));
        if let Some((_, path)) = &mut result.representation {
            path.extend(relative.segments.iter().copied().map(map_selector));
        }
        // A substituted view selects a new logical carrier over the same
        // backing. Its selectors belong to that presentation, not to the
        // preceding interpretation of the captured source.
        if let Some((carrier, path)) = &relative.representation {
            result.representation =
                Some((*carrier, path.iter().copied().map(map_selector).collect()));
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
                | PlaceSegment::OpaqueRange
                | PlaceSegment::Index { .. }
        )
    })
}

/// Flow facts and captured frame origins use the same address projection.
/// Whole builtin windows must not acquire an extra range dimension merely
/// because the origin is captured for a call instead of a local binding.
/// Recover the exact expression prefix from its authenticated declaration
/// owner; absent scope retains the raw coordinates and grants no new meaning.
fn source_coordinates(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
) -> Option<crate::CanonicalPlace> {
    let place = crate::canonical_place_from_expression(program, expression)?;
    // Fields and element selectors already have exact raw coordinates. Only
    // a window can collapse a collection dimension, so only that geometry
    // needs operation meaning and the expression's lexical owner. Ordinary
    // sources keep their established coordinates without rescanning the body.
    if !place.segments.iter().any(|segment| match segment {
        PlaceSegment::FixedRange { .. } => true,
        PlaceSegment::Index { expression } => {
            expression.is_valid() && program.expression_table.range_node(*expression).is_some()
        }
        _ => false,
    }) {
        return Some(place);
    }
    let Some(root) = place.root.as_symbol() else {
        return Some(place);
    };
    let Some((machine, state)) = source_owner(program, root, expression) else {
        return Some(place);
    };
    let Some((owner, _, statement_index)) =
        caller_statement_owner(program, machine, CallerWriteSite::Expression(expression))
    else {
        return Some(place);
    };
    if owner.symbol != state.symbol {
        return Some(place);
    }
    crate::canonical_place_from_expression_in_state(
        program,
        state.symbol,
        statement_index,
        expression,
    )
}

fn source_has_builtin_coordinates(
    program: &dyn crate::CheckingInput,
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
    program: &dyn crate::CheckingInput,
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
            let (machine, state) = program.machine_state(machine_symbol, state_symbol)?;
            let retained = match declaration.kind {
                SymbolKind::Parameter => program.state_parameter_by_symbol(state, root).is_some(),
                _ => program.state_local_declaration(state, root).is_some(),
            };
            retained.then_some((machine, state))
        }
        SymbolKind::Machine | SymbolKind::Field => {
            let machine_symbol = match declaration.kind {
                SymbolKind::Machine => root,
                _ => declaration.parent,
            };
            let machine = program.machine(machine_symbol)?;
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
