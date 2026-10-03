//! Seed bound cleanup prerequisites into the ordinary entry-flow namespace.
//! The caller rebuilds from declaration facts when the bound inputs grow, so
//! prior flow observations never become assumptions of the next iteration.
use checked_trees::ContextualCleanupEntryRequirements;
use diagnostics::Diagnostic;
use facts::{Fact, FactOrigin, FactPayload, FactPlace, FactPlan, Place, PlaceRoot, ProgramPoint};
use typed_trees::TypedTrees;

pub(crate) fn seed_entry_requirements(
    program: &TypedTrees,
    semantic: &mut FactPlan,
    requirements: &ContextualCleanupEntryRequirements,
) -> Result<(), Vec<Diagnostic>> {
    for (_, requirement) in requirements.rows.iter() {
        let invalid = || {
            vec![Diagnostic::error(
                "bound cleanup prerequisite has no exact Boolean entry place",
            )]
        };
        let machine = program
            .machine_by_symbol(requirement.machine)
            .ok_or_else(invalid)?;
        let entry = crate::checks::termination::named_transition_target_state_index(
            program,
            machine,
            machine.symbol,
        )
        .ok_or_else(invalid)?;
        let state = program
            .machine_states(machine)
            .get(entry)
            .ok_or_else(invalid)?;
        let (root, segments, reference, _) = crate::values::resolve_structural_parameter_path(
            program,
            program.state_parameters(state),
            requirement.parameter_position,
            requirements.paths.span_or_empty(requirement.path),
        )
        .ok_or_else(invalid)?;
        if program.primitive_type_reference(reference)
            != Some(typed_trees::types::PrimitiveType::Bool)
        {
            return Err(invalid());
        }
        let segments = semantic.place_segments.insert_many(segments);
        let place = semantic.append_place(Place {
            root: PlaceRoot::Symbol(root),
            segments,
        });
        let value = semantic
            .scalar_values
            .append(facts::ScalarValue::Boolean(requirement.expected));
        semantic.append_fact_context(Fact {
            place: FactPlace::Place(place),
            point: ProgramPoint::State {
                machine_symbol: machine.symbol,
                state_symbol: state.symbol,
            },
            origin: FactOrigin::ContextualCleanup {
                machine_symbol: machine.symbol,
                state_symbol: state.symbol,
            },
            evidence: Default::default(),
            payload: FactPayload::AssignedScalarValue { value },
        });
    }
    Ok(())
}
