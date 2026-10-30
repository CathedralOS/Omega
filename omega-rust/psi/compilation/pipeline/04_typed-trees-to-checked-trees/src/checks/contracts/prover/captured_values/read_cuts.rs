//! Equal-valued completed reads form a cut through a storage-history join.
//! Every read is proved independently before admission. The history query then
//! checks all predecessor paths, without combining mutually exclusive facts.

use crate::flow::CapturedValue;
use checked_trees::{CheckFacts, FlowStorageSnapshot};
use facts::ScalarValue;
use typed_trees::types::PrimitiveType;
use validation::CanonicalPlace;

pub(crate) struct ScalarRead {
    state: symbols::SymbolHandle,
    snapshot: arena::Handle<FlowStorageSnapshot>,
    place: CanonicalPlace,
    primitive: PrimitiveType,
    value: ScalarValue,
}

/// `scalar` is the shared scalar judgment for the complete candidate inventory,
/// not a selected arm's conclusion. Only exact detached read cuts are retained.
pub(crate) fn retain_reads(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    values: &[CapturedValue<'_>],
    primitive: PrimitiveType,
    scalar: &ScalarValue,
    reads: &mut Vec<ScalarRead>,
) {
    for value in values {
        let Some(epoch) = value.producer_epoch else {
            continue;
        };
        let Some(places) = crate::flow::captured_value_places(program, &facts.flow, value) else {
            continue;
        };
        for place in places {
            let reference = validation::canonical_place_type_reference(
                program,
                epoch.state_symbol,
                epoch.statement_index,
                &place.projected,
            )
            .and_then(|reference| {
                validation::field_domains::readable_type_reference(program, reference)
            });
            if reference.and_then(|reference| program.primitive_type_reference(reference))
                != Some(primitive)
            {
                continue;
            }
            reads.push(ScalarRead {
                state: epoch.state_symbol,
                snapshot: epoch.storage_snapshot,
                place: place.projected,
                primitive,
                value: scalar.clone(),
            });
        }
    }
}

pub(crate) fn scalar_preserved_at_values(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    values: &[CapturedValue<'_>],
    primitive: PrimitiveType,
    invocation: arena::Handle<FlowStorageSnapshot>,
    reads: &[ScalarRead],
) -> Option<ScalarValue> {
    if values.is_empty() {
        return None;
    }
    let mut agreed = None;
    for value in values {
        let epoch = value.producer_epoch.unwrap_or(value.operand);
        let live = value.producer_epoch.is_none()
            && crate::flow::operand_binds_live_view(program, value.type_reference);
        let snapshot = if live {
            invocation
        } else {
            epoch.storage_snapshot
        };
        let places = crate::flow::captured_value_places(program, &facts.flow, value)?;
        if places.is_empty() {
            return None;
        }
        for place in places {
            let candidates = reads
                .iter()
                .filter(|read| {
                    read.state == epoch.state_symbol
                        && read.primitive == primitive
                        && read.place == place.projected
                })
                .collect::<Vec<_>>();
            let scalar = candidates.iter().find_map(|candidate| {
                let cuts = candidates
                    .iter()
                    .filter(|read| read.value == candidate.value)
                    .map(|read| read.snapshot)
                    .collect::<Vec<_>>();
                validation::completed_storage::contents_preserved_from(
                    program,
                    &facts.flow.control,
                    snapshot,
                    &place.projected,
                    &cuts,
                )
                .then(|| candidate.value.clone())
            })?;
            if agreed.as_ref().is_some_and(|previous| *previous != scalar) {
                return None;
            }
            agreed = Some(scalar);
        }
    }
    agreed
}
