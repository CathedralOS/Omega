//! Task demand structure is checked at external ingress, independently of the
//! source projection. Runtime/provider admission separately closes the actual
//! selected stack geometry and carry obligations.
use super::{structural_type_foundations, validate_structural_type_graph};
use crate::codec_error::{CodecError, malformed};
use std::collections::BTreeSet;
use terminal_psi::*;

pub(super) fn validate(module: &TerminalModule) -> Result<(), CodecError> {
    let bundle = &module.task_activation_demands;
    if bundle.demands.is_empty()
        && bundle.structural_types.is_empty()
        && bundle.structural_referents.is_empty()
        && bundle.target_layout_applications.is_empty()
    {
        return Ok(());
    }
    let identity = |index| {
        if module.identities.text(index).is_none_or(str::is_empty) {
            malformed("task activation has an unresolved identity")
        } else {
            Ok(())
        }
    };
    // Borrow the independent catalog; validation never clones executable
    // bodies or invents a fake callable merely to inspect type storage.
    let catalog = structural_type_foundations::StructuralCatalog {
        structural_types: &bundle.structural_types,
        structural_referents: &bundle.structural_referents,
    };
    let mut type_ids = BTreeSet::new();
    let mut type_names = BTreeSet::new();
    for (id, name) in bundle
        .structural_types
        .iter()
        .map(|row| (row.id, row.identity))
        .chain(
            bundle
                .structural_referents
                .iter()
                .map(|row| (row.id, row.identity)),
        )
    {
        identity(name)?;
        if !type_ids.insert(id) || !type_names.insert(name) {
            return malformed("task activation structural catalog repeats an identity");
        }
    }
    for declaration in catalog.structural_types {
        structural_type_foundations::validate_catalog_type_shape(
            &catalog,
            declaration,
            &BTreeSet::new(),
        )?;
    }
    validate_structural_type_graph(catalog.structural_types)?;
    let value = |value_type| match value_type {
        TerminalTaskValueType::Unit | TerminalTaskValueType::Scalar(_) => Ok(()),
        TerminalTaskValueType::Structural(id)
            if bundle.structural_types.iter().any(|row| row.id == id) =>
        {
            Ok(())
        }
        _ => malformed("task activation value has no storage declaration"),
    };
    let resident = |row: &TerminalTaskResidentValue| {
        identity(row.normalized_identity)?;
        value(row.value_type)
    };
    let mut layouts = BTreeSet::new();
    for layout in &bundle.target_layout_applications {
        if !layouts.insert(layout.id)
            || layout.realizations.is_empty()
            || !bundle.structural_types.iter().any(|row| {
                row.id == layout.record
                    && matches!(row.shape, StructuralTypeShape::Record {
                    storage: StructuralRecordStorage::TargetLayout(id), ..
                } if id == layout.id)
            })
        {
            return malformed("task activation layout does not bind one exact record");
        }
        let mut targets = BTreeSet::new();
        for row in &layout.realizations {
            if row.target.is_empty()
                || !targets.insert(&row.target)
                || !row.storage.byte_alignment.is_power_of_two()
            {
                return malformed("task activation layout has an invalid target or alignment");
            }
        }
    }
    for declaration in &bundle.structural_types {
        if let StructuralTypeShape::Record {
            storage: StructuralRecordStorage::TargetLayout(id),
            ..
        } = declaration.shape
            && !bundle
                .target_layout_applications
                .iter()
                .any(|layout| layout.id == id && layout.record == declaration.id)
        {
            return malformed("task activation record has no target layout alternatives");
        }
    }
    for demand in &bundle.demands {
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
            let mut hash = semantic_vocabulary::Fnv1a::new();
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
