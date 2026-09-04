//! Independent replay of retained proof and fixed-fuel custody.

use omega_machine_code::RankedU32CountdownMachineCodeRecord;

pub(super) fn replay_verifier_custody(record: &RankedU32CountdownMachineCodeRecord) -> Option<()> {
    let custody = &record.custody;
    let proof = psi_terminal_codec::decode_proof_bundle(&custody.proof_replay).ok()?;
    let profile = psi_proof_admission::AdmissionProfile::default();
    let native = psi_terminal_verifier::verify_module_for_native_ranked_countdown(
        &custody.semantic_replay,
        &proof,
        &profile,
    )
    .ok()?;
    let fixed = psi_terminal_verifier::verify_module_for_fixed_fuel(
        &custody.semantic_replay,
        &proof,
        &profile,
    )
    .ok()?;
    let derived = psi_terminal_fixed_fuel::derive_ranked_countdown_entry_fuel(
        &fixed,
        custody.semantic_replay.entry,
    )
    .ok()?;
    if derived.terminal_psi() != custody.fixed_fuel.terminal_psi()
        || derived.schedule() != custody.fixed_fuel.schedule()
        || derived.entry() != custody.fixed_fuel.entry()
        || derived.relevant_preconditions() != custody.fixed_fuel.relevant_preconditions()
        || derived.ceiling_units() != custody.fixed_fuel.ceiling_units()
    {
        return None;
    }

    let projected = &custody.structural_frontiers;
    let verified = native.structural_frontiers().machine(projected.machine)?;
    let verified_header = verified.block_entry(projected.header)?;
    let verified_backedge = verified.edge_exit(projected.backedge)?;
    if !frontier_matches(&projected.header_entry, verified_header)
        || !frontier_matches(&projected.backedge_exit, verified_backedge)
    {
        return None;
    }
    Some(())
}

fn frontier_matches(
    projected: &omega_abstract_operations::RankedStructuralOwnershipFrontier,
    verified: &psi_terminal_verifier::VerifiedStructuralOwnershipFrontier,
) -> bool {
    projected.claims().len() == verified.claims().len()
        && projected
            .claims()
            .iter()
            .zip(verified.claims())
            .all(|(left, right)| {
                left.claim == right.claim
                    && left.input == right.input
                    && left.path == right.path
                    && left.multiplicity == right.multiplicity
            })
        && projected.owned_places().len() == verified.owned_places().len()
        && projected
            .owned_places()
            .iter()
            .zip(verified.owned_places())
            .all(|(left, right)| {
                left.place == right.place && left.multiplicity == right.multiplicity
            })
        && projected.partial_custody().len() == verified.partial_custody().len()
        && projected
            .partial_custody()
            .iter()
            .zip(verified.partial_custody())
            .all(|(left, right)| left.place == right.place && left.moved_paths == right.moved_paths)
}
