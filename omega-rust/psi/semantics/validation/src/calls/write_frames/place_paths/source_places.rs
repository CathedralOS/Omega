//! Structural source selectors retained independently of coarse write paths.
//! These identify a possible source; declaration, reference-boundary, and
//! access checks remain with the origin consumer.

use facts::{FactPlan, PlaceRoot, PlaceSegment};
use symbols::{SymbolHandle, SymbolKind};
use typed_trees::TypedTrees;
use typed_trees::expression::ExpressionHandle;
use typed_trees::machine::Machine;
use typed_trees::state::State;
use typed_trees::statement::StatementNode;

use crate::calls::write_frames::caller_aliases::{CallerWriteSite, caller_statement_at_site};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(in crate::calls::write_frames) struct FrameSourcePlace {
    pub(in crate::calls::write_frames) root: SymbolHandle,
    pub(in crate::calls::write_frames) segments: Vec<PlaceSegment>,
    /// Permission to interpret candidate selectors as primitive coordinates.
    /// False retains possible-source metadata for the coarse frame consumers.
    pub(in crate::calls::write_frames) builtin_coordinates: bool,
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
        let segments = facts.place_segments.span_or_empty(place.segments);
        let builtin_coordinates = !segments_have_index_geometry(segments)
            || source_has_builtin_coordinates(program, root, expression);
        Self {
            root,
            segments: segments.to_vec(),
            builtin_coordinates,
        }
    }

    /// Append schema-derived selectors. Expression-derived selectors must
    /// also carry their meaning flag via append_source or explicit conjunction.
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

    pub(in crate::calls::write_frames) fn append_source(&self, source: &Self) -> Self {
        let mut projected = self.append_segments(&source.segments);
        projected.builtin_coordinates &= source.builtin_coordinates;
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
    pub(in crate::calls::write_frames) fn append_relative(&self, relative: &Self) -> Self {
        if !self.root.is_valid() || !relative.root.is_valid() {
            return Self::default();
        }
        let mut result = self.clone();
        result.builtin_coordinates &= relative.builtin_coordinates;
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
    let mut owner = None;
    for machine in program.machines() {
        match declaration.kind {
            SymbolKind::Parameter | SymbolKind::Local => {
                for state in program.machine_states(machine) {
                    if state.symbol != declaration.parent {
                        continue;
                    }
                    let retained = match declaration.kind {
                        SymbolKind::Parameter => program
                            .state_parameters(state)
                            .iter()
                            .any(|parameter| parameter.symbol == root),
                        SymbolKind::Local => program
                            .statement_table
                            .statements(state.statement_nodes)
                            .iter()
                            .any(|statement| {
                                matches!(statement, StatementNode::LocalData(local)
                                    if local.symbol == root)
                            }),
                        _ => false,
                    };
                    if retained {
                        if owner.is_some() {
                            return None;
                        }
                        owner = Some((machine, state));
                    }
                }
            }
            SymbolKind::Machine | SymbolKind::Field => {
                let owns_root = match declaration.kind {
                    SymbolKind::Machine => machine.symbol == root,
                    SymbolKind::Field => {
                        declaration.parent == machine.symbol
                            && crate::exact_attached_field(
                                program,
                                machine,
                                root,
                                program.symbols.name(root),
                            )
                            .is_some()
                    }
                    _ => false,
                };
                if owns_root {
                    let (state, _, _) = caller_statement_at_site(
                        program,
                        machine,
                        CallerWriteSite::Expression(expression),
                    )?;
                    if owner.is_some() {
                        return None;
                    }
                    owner = Some((machine, state));
                }
            }
            _ => return None,
        }
    }
    owner
}

#[cfg(test)]
mod tests;
