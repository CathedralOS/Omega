//! Validate the storage, frames and suspension crossings of recorded task activations.
use crate::codec_error::{CodecError, malformed};
use std::collections::BTreeSet;
use terminal_psi::*;

pub(crate) fn validate(module: &TerminalModule) -> Result<(), CodecError> {
    let demands = &module.execution.task_activation_demands;
    if demands.is_empty() {
        return Ok(());
    }
    let identity = |index| {
        if module.identities.text(index).is_none_or(str::is_empty) {
            malformed("task activation has an unresolved identity")
        } else {
            Ok(())
        }
    };
    let value = |value_type| match value_type {
        TerminalTaskValueType::Unit | TerminalTaskValueType::Scalar(_) => Ok(()),
        TerminalTaskValueType::Structural(id) if module.structural_type(id).is_some() => Ok(()),
        _ => malformed("task activation value has no storage declaration"),
    };
    let resident = |row: &TerminalTaskResidentValue| {
        identity(row.normalized_identity)?;
        value(row.value_type)
    };
    for demand in demands {
        for index in [
            demand.requirement_callable_identity,
            demand.requirement_owner_path,
            demand.authored_requirement_identity,
            demand.machine_callable_identity,
            demand.entry,
        ] {
            identity(index)?;
        }
        if demand.report_fingerprint == 0
            || demand.entry_report_fingerprint == 0
            || demand.contract_report_fingerprint == 0
            || demand.specialization_report_fingerprint == 0
            || demand.specialization_commitment == [0; 32]
        {
            return malformed("task activation has an absent commitment");
        }
        for argument in demand
            .signature_arguments
            .iter()
            .chain([&demand.signature_outcome])
        {
            identity(argument.normalized_identity)?;
            value(argument.value_type)?;
        }
        let frames = demand
            .frames
            .iter()
            .map(|frame| frame.identity)
            .collect::<BTreeSet<_>>();
        if frames.len() != demand.frames.len() || frames.contains(&0) {
            return malformed("task activation frame roster repeats an identity");
        }
        let root = demand
            .frames
            .iter()
            .find(|frame| frame.identity == demand.root_frame)
            .ok_or(CodecError::MalformedStructuralFoundation(
                "task activation root frame is absent",
            ))?;
        if root.machine_callable_identity != demand.machine_callable_identity
            || root.entry != demand.entry
        {
            return malformed("task activation root frame differs from its selected entry");
        }
        for frame in &demand.frames {
            identity(frame.machine_callable_identity)?;
            identity(frame.entry)?;
            let mut hash = wire::fnv1a::Fnv1a::new();
            hash.byte(0x57);
            hash.string_terminated(
                module
                    .identities
                    .text(frame.machine_callable_identity)
                    .unwrap_or_default(),
            );
            hash.string_terminated(module.identities.text(frame.entry).unwrap_or_default());
            if frame.identity != hash.finish_or(1) {
                return malformed(
                    "task activation frame identity differs from its callable and entry",
                );
            }
            let mut states = BTreeSet::new();
            if frame.states.is_empty() {
                return malformed("task activation frame has no resident states");
            }
            for state in &frame.states {
                identity(state.state)?;
                if !states.insert(state.state) {
                    return malformed("task activation frame repeats a state");
                }
                for row in state
                    .resident
                    .iter()
                    .chain(state.crossing_arguments.iter().flatten())
                {
                    resident(row)?;
                }
            }
            if !states.contains(&frame.entry) {
                return malformed("task activation frame omits its entry state");
            }
            for call in &frame.calls {
                if !frames.contains(&call.callee) {
                    return malformed("task activation call has no callee frame");
                }
            }
            let mut coordinates = BTreeSet::new();
            for call in &frame.unresolved_calls {
                if !states.contains(&call.state)
                    || !coordinates.insert((call.state, call.statement_index, call.call_ordinal))
                {
                    return malformed(
                        "task activation unresolved call has no unique state coordinate",
                    );
                }
            }
        }
        for crossing in &demand.crossings {
            if crossing.identity == 0 {
                return malformed("task activation has a zero crossing identity");
            }
            let mut places = BTreeSet::new();
            for live in &crossing.live_carry {
                if live.place == 0
                    || live.value_type == 0
                    || !places.insert(live.place)
                    || live.claims.contains(&0)
                {
                    return malformed("task activation carry has an invalid live identity");
                }
            }
        }
    }
    Ok(())
}
