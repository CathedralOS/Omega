//! An owned claim follows its actual transfer, independently of erased labels.
//!
//! A same-carrier cast may forget a non-owning meaning while moving an existing
//! content claim. Read only that claim's earlier membership and consuming event;
//! restoring the operand's whole semantic place would also restore erased labels.

use arena::Handle;
use checked_trees::{CheckFacts, FlowOperandFact};
use facts::PlaceSegment;
use language_semantics::content::ContentProjectionPlan;
use language_semantics::{
    PermissionAccess, PermissionClaimIdentity, PermissionEventKind, PermissionEventSource,
    PermissionProvenance, SemanticDomainId,
};
use symbols::SymbolHandle;

pub(super) fn transferred_membership(
    program: &dyn crate::CheckingInput,
    facts: &CheckFacts,
    value: Handle<FlowOperandFact>,
    path: &[PlaceSegment],
    domain: SymbolHandle,
    semantic_domain: SemanticDomainId,
) -> Option<Result<(), &'static str>> {
    let control = &facts.flow.control;
    if !control.operands.is_valid(value)
        || program
            .expression_table
            .cast_node(control.operands.get(value).expression)
            .is_none()
    {
        return None;
    }
    let mut projections = facts
        .qualifications
        .content
        .plans
        .iter()
        .filter(|projection| {
            semantic_domain.is_valid()
                && projection.domain == domain
                && projection.semantic_domain == semantic_domain
        });
    let projection = projections.next()?;
    if projections.next().is_some() {
        return Some(Err("owned membership has ambiguous content projections"));
    }
    Some(preserved_transfer(program, facts, value, path, projection))
}

fn preserved_transfer(
    program: &dyn crate::CheckingInput,
    facts: &CheckFacts,
    value: Handle<FlowOperandFact>,
    path: &[PlaceSegment],
    projection: &ContentProjectionPlan,
) -> Result<(), &'static str> {
    let domain = projection.domain;
    let semantic_domain = projection.semantic_domain;
    let control = &facts.flow.control;
    if !control.operands.is_valid(value) {
        return Err("owned transfer has no completed value");
    }
    let completed = control.operands.get(value);
    let mut source = completed;
    let mut source_handle = value;
    let mut visited = std::collections::HashSet::new();
    let mut transferred = false;
    while let Some(expression) = crate::value_preserving_cast_operand(
        program,
        source.state_symbol,
        source.statement_index,
        source.expression,
    ) {
        if !visited.insert(source_handle) {
            return Err("owned transfer has a cyclic captured cast chain");
        }
        let [input] = control
            .operands
            .span(source.evaluated_inputs)
            .ok_or("owned cast has no evaluated operand")?
        else {
            return Err("owned cast does not have one evaluated operand");
        };
        if input.expression != expression
            || input.state_symbol != source.state_symbol
            || input.statement_index != source.statement_index
            || input.evaluation != source.evaluation
            || source
                .conditional_inputs_from
                .is_some_and(|prefix| prefix != 1)
            || !crate::declared_carriers_match(program, input.type_reference, source.type_reference)
        {
            return Err("owned cast lost its exact evaluated operand");
        }
        source_handle = source
            .evaluated_inputs
            .handles()
            .next()
            .ok_or("owned cast has no evaluated operand handle")?;
        source = input;
        transferred = true;
    }
    if !transferred {
        return Err("completed value is not an owned qualification transfer");
    }
    let mut place = crate::canonical_place_from_expression_in_state(
        program,
        source.state_symbol,
        source.statement_index,
        source.expression,
    )
    .ok_or("owned input has no exact semantic place")?;
    let owning_source = crate::ownership_source_expression(
        program,
        completed.state_symbol,
        completed.statement_index,
        completed.expression,
    );
    if owning_source != source.expression {
        return Err("owned transfer lost its captured ownership source");
    }
    // Erasure keeps a new semantic root. Its input still names the original
    // membership, while the owning-source query selects the storage consumed.
    let mut owned = crate::ownership_place_from_expression_in_state(
        program,
        completed.state_symbol,
        completed.statement_index,
        owning_source,
    )
    .ok_or("owned transfer has no exact source place")?;
    place.segments.extend_from_slice(path);
    owned.segments.extend_from_slice(path);
    for length in 0..=path.len() {
        let reference = crate::project_type_reference_from_segments(
            program,
            source.type_reference,
            &path[..length],
        )
        .ok_or("owned membership has an unresolved typed projection")?;
        if crate::reference_shell(program, reference).is_some() {
            return Err("owned membership cannot cross a borrowed referent");
        }
        if length == path.len() {
            let carrier = program
                .unwrapped_type_reference(reference)
                .ok_or("owned membership has no complete carrier")?;
            if program.normalized_type_identity_text(carrier) != projection.carrier_identity {
                return Err("owned membership has a different projection carrier");
            }
        }
    }
    if !control.storage_snapshots.is_valid(source.storage_snapshot)
        || !control
            .storage_snapshots
            .is_valid(completed.storage_snapshot)
        || (source.storage_snapshot != completed.storage_snapshot
            && !super::contents_preserved_from(
                program,
                control,
                completed.storage_snapshot,
                &owned,
                &[source.storage_snapshot],
                super::StoredObservation::InlineContents,
            ))
    {
        return Err("owned input changed after its captured membership");
    }
    let known = facts
        .flow
        .semantic_constraint_contexts(source.constraints)
        .any(|context| {
            facts
                .semantic
                .context_view(facts.semantic.contexts.get(context))
                .facts()
                .any(|fact| {
                    let Some((_, _, candidate, instance)) = fact.payload.domain_membership() else {
                        return false;
                    };
                    candidate == domain
                        && instance == semantic_domain
                        && fact.place.as_place().is_some_and(|handle| {
                            crate::membership_evidence::canonical_place_from_semantic_place(
                                program,
                                &facts.semantic,
                                facts.semantic.places.get(handle),
                            )
                            .is_some_and(|source| source == place)
                        })
                })
        });
    if !known {
        return Err("captured input has no live exact content membership");
    }
    let machine = program
        .machine_holding_state(completed.state_symbol)
        .ok_or("owned transfer has no machine")?;
    let state = control
        .unique_state(machine.symbol, completed.state_symbol)
        .ok_or("owned transfer has no unique state occurrence")?;
    if !super::consumer_captures(
        control,
        completed.state_symbol,
        completed.statement_index,
        completed.expression,
    )
    .is_some_and(|captures| captures.contains(&value))
    {
        return Err("owned transfer is not an actual completed consumer operand");
    }
    if completed.evaluation == checked_trees::FlowPermissionEvaluation::Unknown {
        return Err("owned transfer has no actual evaluation lane");
    }
    let mut identity = None;
    let permissions = facts
        .flow
        .ownership
        .with_state_permissions(completed.state_symbol, |handles| handles.to_vec());
    for handle in permissions {
        let event = facts.flow.ownership.permissions.get(handle);
        // Permission production expands each owned carrier into canonical
        // frontier leaves. This exact leaf is also the earlier membership's
        // typed subject; its transfer supplies the claim identity, not the
        // carrier's multiplicity or the projection's numeric contents.
        if event.machine_symbol != machine.symbol
            || event.state_symbol != completed.state_symbol
            || event.expression != source.expression
            || event.evaluation != completed.evaluation
            || event.root != owned.root
            || facts.flow.ownership.segments.span_or_empty(event.segments) != owned.segments
        {
            continue;
        }
        let statement = match event.source {
            PermissionEventSource::Statement { statement_index } => statement_index,
            PermissionEventSource::Call {
                statement_index,
                call_ordinal,
                target_symbol,
            } => {
                if !control.calls.span_or_empty(state.calls).iter().any(|call| {
                    call.statement_index == statement_index
                        && call.call_ordinal == call_ordinal
                        && call.target_symbol == target_symbol
                        && call.operands.handles().any(|operand| operand == value)
                }) {
                    return Err("owned transfer has no exact consuming invocation");
                }
                statement_index
            }
            _ => continue,
        };
        if statement != completed.statement_index || event.kind != PermissionEventKind::Transfer {
            continue;
        }
        if event.access != PermissionAccess::Owned
            || !event.obligation_live
            || event.claim_identity == PermissionClaimIdentity::Unknown
            || event.provenance == PermissionProvenance::Unknown
            || identity.is_some_and(|previous| previous != event.claim_identity)
        {
            return Err("owned transfer has unavailable or ambiguous claim custody");
        }
        identity = Some(event.claim_identity);
    }
    identity
        .map(|_| ())
        .ok_or("completed cast has no live owned claim transfer")
}
