//! Natural call ranks belong to a complete invocation component. Private
//! participants need an exact carried coordinate, not a fabricated declaration
//! witness. Search declaration identities and judge every actual edge under its
//! own completed prefix; only the full weak-edge DAG establishes termination.

use validation::ValidatedRuntimeNaturalComponent;

mod candidate_selection;
mod coordinate_assignment;
mod coordinates;
mod edge_observations;
mod observations;
mod ranges;
pub(super) use observations::retain_observation;
mod views;

/// Infer missing private observations, then prove all actual edges and cycles.
/// Fully authored views keep the richer ordinary view-admission owner.
pub(super) fn select(
    program: &dyn validation::CheckingInput,
    component: &[usize],
    frames: Option<&validation::CallFrameResolver<'_>>,
) -> Option<Result<ValidatedRuntimeNaturalComponent, &'static str>> {
    let machines = component
        .iter()
        .map(|position| &program.machines()[*position])
        .collect::<Vec<_>>();
    if machines
        .iter()
        .all(|machine| machine.termination_plan.implementation_witness.is_some())
    {
        return None;
    }
    let candidates = match candidate_selection::prepare(program, &machines, frames)? {
        Ok(candidates) => candidates,
        Err(reason) => return Some(Err(reason)),
    };
    let edges =
        match edge_observations::collect(program, &machines, frames, &candidates.field_paths) {
            Ok(edges) => edges,
            Err(reason) => return Some(Err(reason)),
        };
    Some(
        coordinate_assignment::select(
            program,
            &machines,
            &candidates.ranks,
            &edges,
            candidates.order,
        )
        .ok_or("no exact private-coordinate assignment proves every edge and complete cycle"),
    )
}
