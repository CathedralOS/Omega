//! Evidence producer provenance and the evidence term roots it names.

use crate::proofs::evidence_lowering::{
    checked_evidence_machine_identity, checked_evidence_requirement_identity,
};
use crate::proofs::{
    CheckedTrees, EvidenceIdentity, EvidenceProducerProvenance, EvidenceProducerRealization,
    EvidenceProducerRowSource, EvidenceTermId, LoweringError, unsupported,
};

pub(crate) fn lower_evidence_producer_provenance(
    checked: &CheckedTrees,
    selected_machine: symbols::SymbolHandle,
    term_ids: &[Option<EvidenceTermId>],
) -> Result<Vec<EvidenceProducerProvenance>, LoweringError> {
    let mut package_callees = checked
        .facts
        .proof
        .proof_output_calls
        .iter()
        .filter_map(|(_, invocation)| {
            (invocation.caller_machine_symbol == selected_machine
                && invocation.static_requirement_dispatch.is_none())
            .then_some(invocation.target_machine_symbol)
        })
        .collect::<Vec<_>>();
    if let Some(plan) = checked
        .facts
        .flow
        .terminal_structural_call_returns
        .payloadless_guarded_for_machine(selected_machine)
    {
        package_callees.push(plan.target_machine);
    }
    package_callees.sort_unstable_by_key(|machine| machine.arena_index());
    package_callees.dedup();
    let mut producers =
        checked
            .facts
            .proof
            .evidence_forwardings
            .iter()
            .filter_map(|(_, forwarding)| {
                if forwarding.machine_symbol != selected_machine
                    && !package_callees.contains(&forwarding.machine_symbol)
                {
                    return None;
                }
                let checked_trees::EvidenceAssignmentSource::ProducerConformance {
                    conformance,
                    evidence_trait,
                    rows,
                } = &forwarding.source
                else {
                    return None;
                };
                let output_index = usize::try_from(forwarding.output.arena_index() - 1)
                    .expect("arena indices fit the host address space");
                Some((
                    term_ids.get(output_index).copied().flatten().ok_or(
                        LoweringError::Unsupported(
                            "selected evidence producer has no terminal term identity",
                        ),
                    ),
                    forwarding.output,
                    *conformance,
                    *evidence_trait,
                    rows,
                ))
            })
            .map(|(term, output, conformance, evidence_trait, rows)| {
                let interface = checked
                    .facts
                    .proof
                    .evidence_terms
                    .get(output)
                    .evidence_interface
                    .as_ref()
                    .ok_or(LoweringError::Unsupported(
                        "selected evidence producer has an unresolved interface",
                    ))?;
                let mut lowered_rows = rows
                    .iter()
                    .map(|row| {
                        let mut requirement_rows = interface.requirements.iter().filter(|entry| {
                            entry.declaring_trait == row.declaring_trait
                                && entry.requirement == row.requirement
                        });
                        let requirement_row = requirement_rows.next().ok_or(
                            LoweringError::Unsupported(
                                "selected evidence producer row is absent from its interface",
                            ),
                        )?;
                        if requirement_rows.next().is_some() {
                            return unsupported(
                                "selected evidence producer row has ambiguous instantiated interface arguments",
                            );
                        }
                        Ok(EvidenceProducerRealization {
                            declaring_trait_identity: checked
                                .symbols
                                .display_path(row.declaring_trait, "::"),
                            declaring_trait_arguments: requirement_row
                                .declaring_trait_arguments
                                .clone(),
                            requirement_identity: checked_evidence_requirement_identity(
                                checked,
                                row.declaring_trait,
                                row.requirement,
                            )?,
                            realization_machine_identity: checked_evidence_machine_identity(
                                checked,
                                row.realization_machine,
                            )?,
                            realization_state_identity: checked
                                .symbols
                                .display_path(row.realization_state, "::"),
                            source: match row.source {
                                checked_trees::DynamicConformanceRowSource::Inline => {
                                    EvidenceProducerRowSource::Inline
                                }
                                checked_trees::DynamicConformanceRowSource::Reference => {
                                    EvidenceProducerRowSource::Reference
                                }
                                checked_trees::DynamicConformanceRowSource::TraitDefault => {
                                    EvidenceProducerRowSource::TraitDefault
                                }
                            },
                        })
                    })
                    .collect::<Result<Vec<_>, LoweringError>>()?;
                lowered_rows.sort();
                Ok(EvidenceProducerProvenance {
                    id: EvidenceIdentity::new(1).expect("placeholder identity is nonzero"),
                    term: term?,
                    conformance_identity: checked.symbols.display_path(conformance, "::"),
                    evidence_trait_identity: checked.symbols.display_path(evidence_trait, "::"),
                    rows: lowered_rows,
                })
            })
            .collect::<Result<Vec<_>, LoweringError>>()?;
    producers.sort_by_key(|producer| producer.term);
    for (index, producer) in producers.iter_mut().enumerate() {
        producer.id = EvidenceIdentity::new(
            u64::try_from(index)
                .expect("evidence producer count fits u64")
                .checked_add(1)
                .expect("one-based evidence producer identity fits u64"),
        )
        .expect("one-based evidence producer identity is nonzero");
    }
    Ok(producers)
}

pub(crate) fn hex_bytes(bytes: &[u8]) -> String {
    use std::fmt::Write;

    let mut rendered = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(&mut rendered, "{byte:02x}").expect("writing to String cannot fail");
    }
    rendered
}

pub(crate) fn evidence_term_root(parents: &mut [usize], mut index: usize) -> usize {
    while parents[index] != index {
        let parent = parents[index];
        parents[index] = parents[parent];
        index = parents[index];
    }
    index
}
