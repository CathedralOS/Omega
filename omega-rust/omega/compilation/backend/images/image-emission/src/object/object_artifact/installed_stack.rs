//! Measure an installed arrival using the body and bridge that will be linked.
//! The projected numbers remain conditional until final private-object
//! composition rejoins them to these exact physical products.

use crate::{FunctionFragmentObjectArtifactError as Error, PrivateFunctionObject};
use installation_evidence::InstalledInterruptStackRequirement;
mod arrival;
mod faults;
pub use arrival::compose as compose_installed_arrival_demands;

pub fn installed_interrupt_stack_requirement(
    private: &PrivateFunctionObject,
    operation: semantic_vocabulary::OperationId,
) -> Result<InstalledInterruptStackRequirement, Error> {
    let placement_index =
        private
            .identity
            .callback_thunk_placement_index()
            .ok_or(Error::Mismatch(
                "installed stack demand has no private placement",
            ))?;
    if function_identity::MachineFunctionIdentity::callback_thunk(
        private.body.psi(),
        placement_index,
    ) != Some(private.identity)
        || private.body.composition_source.is_some()
        || private.body.physical_entry.is_some()
    {
        return Err(Error::Mismatch(
            "installed stack demand changed its original private body",
        ));
    }
    let bridge = private
        .entry_bridge
        .as_ref()
        .filter(|bridge| bridge.installed_interrupt().is_some())
        .ok_or(Error::Mismatch(
            "installed stack demand has no admitted arrival bridge",
        ))?;
    // Ordinary synchronous calls compose inside the original namespace. The
    // bridge owns the automatic CPU frame, saved registers and entry locals;
    // its call return address is accounted separately and exactly once.
    let body = super::stack_demand::derive_body_stack_demand(&private.body, private.body.entry())
        .map_err(Error::PrivateFunctions)?;
    let requirement = InstalledInterruptStackRequirement {
        arrival: None,
        fault_exclusion: faults::derive(&private.body)?,
        psi: private.body.psi(),
        target: private.body.target(),
        operation,
        placement_index,
        entry: private.body.entry(),
        entry_plan: bridge
            .installed_interrupt()
            .ok_or(Error::Mismatch("installed entry plan is missing"))?,
        body_bytes: body.ceiling_bytes(),
        bridge_bytes: bridge.frame_bytes(),
        call_stack_bias: bridge.call_stack_bias(),
        alignment: body
            .stack_alignment()
            .max(u32::from(bridge.request().internal.stack_alignment)),
    };
    requirement.required_bytes().ok_or(Error::Overflow)?;
    Ok(requirement)
}

/// A guard's numeric capacity is accepted only against the actual private
/// objects supplied for final composition, including requirements emitted
/// inside another private entry's ordinary body.
pub(crate) fn validate_installed_stack_requirements(
    root: &crate::ObjectArtifact,
    private_objects: &[PrivateFunctionObject],
) -> Result<(), Error> {
    let key = |identity: function_identity::MachineFunctionIdentity| {
        (
            identity.caller().vocabulary_marker.get(),
            *identity.caller().program_fingerprint.as_bytes(),
            identity.callback_thunk_placement_index(),
        )
    };
    let mut objects: Vec<_> = private_objects.iter().collect();
    objects.sort_unstable_by_key(|row| key(row.identity));
    if objects
        .windows(2)
        .any(|pair| key(pair[0].identity) == key(pair[1].identity))
    {
        return Err(Error::Mismatch("final private identity is duplicated"));
    }
    for body in std::iter::once(root).chain(private_objects.iter().map(|row| row.body.as_ref())) {
        let Some(replay) = &body.fragment_replay else {
            continue;
        };
        let target = replay
            .0
            .source()
            .source()
            .source()
            .source()
            .optimized_target();
        let mut actual_requirements = Vec::new();
        for expected in target.installed_stack_requirements() {
            let identity = function_identity::MachineFunctionIdentity::callback_thunk(
                expected.psi,
                expected.placement_index,
            )
            .ok_or(Error::Mismatch(
                "installed stack requirement has invalid private identity",
            ))?;
            let private = objects
                .binary_search_by_key(&key(identity), |row| key(row.identity))
                .ok()
                .and_then(|position| objects.get(position))
                .copied()
                .ok_or(Error::Mismatch(
                    "installed stack guard has no final private body",
                ))?;
            actual_requirements.push(installed_interrupt_stack_requirement(
                private,
                expected.operation,
            )?);
        }
        for scope in target.lowering_facts().installed_interrupts().scopes() {
            let first =
                actual_requirements.partition_point(|row| row.operation < scope.operation());
            let last =
                actual_requirements.partition_point(|row| row.operation <= scope.operation());
            if first == last {
                continue;
            }
            let entries: Vec<_> = target
                .lowering_facts()
                .installed_interrupts()
                .entries()
                .iter()
                .filter(|entry| entry.operation == scope.operation())
                .collect();
            // This is still a conditional request. The final physical binding
            // below independently joins the semantic entry and actual adapter.
            let proposed_root = target.installed_stack_requirements()[first]
                .arrival
                .as_ref()
                .map(|arrival| arrival.physical_entry);
            if proposed_root
                .is_some_and(|entry| !std::ptr::eq(body, root) || entry.adapter != root.entry())
            {
                return Err(Error::Mismatch(
                    "private installation cannot borrow the application's physical entry stack",
                ));
            }
            compose_installed_arrival_demands(
                scope,
                &entries,
                &mut actual_requirements[first..last],
                proposed_root,
            )?;
        }
        if actual_requirements != target.installed_stack_requirements() {
            return Err(Error::Mismatch(
                "installed stack guard differs from final body and complete arrival demand",
            ));
        }
    }
    Ok(())
}

/// Rejoin conditional root reserves to the actual physical entry. Code in a
/// private namespace cannot borrow this backing merely by sharing a MachineId.
pub(crate) fn root_reserve(
    artifact: &crate::ObjectArtifact,
    root: installation_evidence::InstalledPhysicalStackRoot,
) -> Result<u64, Error> {
    if artifact.entry() != root.adapter {
        return Err(Error::Mismatch(
            "installed root stack has a different physical adapter",
        ));
    }
    let source = artifact.composition_source.as_deref().unwrap_or(artifact);
    let mut reserve = 0;
    for body in std::iter::once(source).chain(
        artifact
            .private_functions
            .iter()
            .map(|entry| entry.body.as_ref()),
    ) {
        let Some(replay) = &body.fragment_replay else {
            continue;
        };
        let target = replay
            .0
            .source()
            .source()
            .source()
            .source()
            .optimized_target();
        for requirement in target.installed_stack_requirements() {
            let Some(arrival) = &requirement.arrival else {
                continue;
            };
            let expected = arrival.physical_entry;
            if expected != root
                || !std::ptr::eq(body, source)
                || requirement.psi != artifact.psi()
                || requirement.target != artifact.target()
            {
                return Err(Error::Mismatch(
                    "arrival root demand differs from the actual physical entry binding",
                ));
            }
            reserve = reserve.max(arrival.root_reserve_bytes);
        }
    }
    Ok(reserve)
}

pub(crate) fn requires_root_provision(artifact: &crate::ObjectArtifact) -> bool {
    let source = artifact.composition_source.as_deref().unwrap_or(artifact);
    std::iter::once(source)
        .chain(
            artifact
                .private_functions
                .iter()
                .map(|entry| entry.body.as_ref()),
        )
        .filter_map(|body| body.fragment_replay.as_ref())
        .any(|replay| {
            replay
                .0
                .source()
                .source()
                .source()
                .source()
                .optimized_target()
                .installed_stack_requirements()
                .iter()
                .any(|row| row.arrival.is_some())
        })
}

/// Final executable publication cannot leave a conditional installation-stack
/// demand on a hosted or unbound entry. Earlier unbound objects remain valid
/// inputs to composition; they grant no executable arrival surface.
pub fn require_installed_root_provision(artifact: &crate::ObjectArtifact) -> Result<(), Error> {
    if requires_root_provision(artifact) && artifact.physical_entry_binding().is_none() {
        return Err(Error::Mismatch(
            "interrupt installation has no matching physical-entry stack provision",
        ));
    }
    Ok(())
}
