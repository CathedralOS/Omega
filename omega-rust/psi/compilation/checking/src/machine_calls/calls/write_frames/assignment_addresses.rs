//! Whether a captured store can publish facts under its current place spelling.
//!
//! A complete store footprint authorizes invalidation, not address equality.
//! RHS execution may replace an intermediate reference slot after the target
//! address was captured. Check those address inputs separately from the contents
//! being stored. This grants neither access nor facts about the old contents.

use super::caller_aliases::{CallerWriteSite, caller_statement_owner};
use super::{Machine, StatementNode};

pub(super) fn destination_remains_current(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    statement: &StatementNode,
    writes: Option<&[String]>,
    captured: Option<&crate::CanonicalPlace>,
) -> Option<()> {
    let assignment = statement.as_assignment()?;
    let (state, _, ordinal) =
        caller_statement_owner(program, machine, CallerWriteSite::Statement(statement))?;
    if captured.is_none() {
        crate::place_has_builtin_coordinates(program, machine, Some(state), assignment.target)
            .then_some(())?;
    }
    // A consumer holding the completed address may know a more precise old
    // backing than the authored path now names. Judge that captured coordinate
    // directly; changing an alias does not redirect an already owned place.
    let authored = if captured.is_none() {
        crate::canonical_place_from_expression_in_state(
            program,
            state.symbol,
            ordinal,
            assignment.target,
        )
    } else {
        None
    };
    let place = captured.or(authored.as_ref())?;
    let root = place.root.as_symbol()?;
    // A selector must retain the value evaluated before the RHS. Reuse the
    // ordinary observation judgment, including exact operator/call identity;
    // neither a same spelling nor an empty write summary proves repeatability.
    let selectors = captured.map_or_else(
        || crate::assignment_target_selectors(program, assignment.target),
        |captured| {
            captured
                .segments
                .iter()
                .filter_map(|segment| {
                    if let facts::PlaceSegment::Index { expression } = segment {
                        Some(*expression)
                    } else {
                        None
                    }
                })
                .collect()
        },
    );
    for selector in selectors {
        crate::proof_contracts::arithmetic_domains::observation_survives_writes(
            program, machine, state, selector, writes,
        )
        .then_some(())?;
    }
    place
        .segments
        .iter()
        .all(|segment| {
            matches!(
                segment,
                facts::PlaceSegment::Field { .. }
                    | facts::PlaceSegment::Case { .. }
                    | facts::PlaceSegment::FixedIndex { .. }
                    | facts::PlaceSegment::Index { .. }
            )
        })
        .then_some(())?;
    let declared = crate::value_custody::places::canonical_place_type_reference(
        program,
        state.symbol,
        ordinal,
        &crate::CanonicalPlace {
            root: place.root,
            segments: Vec::new(),
        },
    )?;
    let receiving = super::reference_subjects::assignment_stored_value_type(
        program, machine, state, assignment,
    )?;
    for depth in 0..=place.segments.len() {
        let mut reference = crate::value_custody::places::project_type_reference_from_segments(
            program,
            declared,
            &place.segments[..depth],
        )?;
        let mut incoming_address =
            depth == 0 && program.state_parameter_by_symbol(state, root).is_some();
        while let Some(shell) = crate::reference_shell(program, reference) {
            if depth == place.segments.len() && reference == receiving {
                break;
            }
            // An exclusive parameter's outer address is fixed for this
            // invocation. Its inner reference slots remain replaceable.
            if !incoming_address || shell.access == language_core::ReferenceAccess::Shared {
                let (path, precision) = super::place_paths::coordinate_storage_path(
                    program,
                    program.symbols.name(root),
                    &place.segments[..depth],
                );
                writes
                    .is_some_and(|writes| {
                        writes.iter().all(|written| {
                            // A strict descendant changes the referent, not
                            // this reference slot. Collection-coarse paths
                            // cannot make that distinction and stay coarse.
                            if precision == super::FramePathPrecision::Exact {
                                super::stored_origins::place_suffix(written, &path).is_none()
                            } else {
                                !super::demand::frame_paths_overlap(&path, written)
                            }
                        })
                    })
                    .then_some(())?;
            }
            incoming_address = false;
            reference = shell.referee;
        }
    }
    Some(())
}
