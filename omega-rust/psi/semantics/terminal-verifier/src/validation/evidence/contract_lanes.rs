//! The evidence contract lanes and the terms they use.

use super::super::{
    BTreeMap, BTreeSet, EvidenceContractLaneKind, EvidenceTermId, MachineId, ModuleError,
    Proposition, TerminalMachine, TerminalModule,
};
use super::validate_outcome_guard;

/// The evidence contract lanes: each lane names a known term at the next
/// position of its machine and kind with a unique output field, and each
/// machine's contract exposes exactly its lanes. Returns the terms the
/// lanes use.
pub(super) fn validate_lane_rosters(
    module: &TerminalModule,
    machines: &BTreeMap<MachineId, &TerminalMachine>,
    terms: &BTreeMap<EvidenceTermId, &terminal_psi::EvidenceTermDeclaration>,
) -> Result<BTreeSet<EvidenceTermId>, ModuleError> {
    let mut next_positions = BTreeMap::new();
    let mut used_terms = BTreeSet::new();
    let mut output_fields = BTreeSet::new();
    for lane in &module.evidence_contract_lanes {
        if !machines.contains_key(&lane.machine) {
            return Err(ModuleError::UnknownEvidenceContractMachine(lane.machine));
        }
        let Some(term) = terms.get(&lane.term) else {
            return Err(ModuleError::UnknownEvidenceContractTerm(lane.term));
        };
        let application = module
            .proposition_applications
            .iter()
            .find(|application| application.id == term.proposition)
            .expect("evidence terms were validated before contract lanes");
        if application.evidence_interface.as_ref() != Some(&term.interface) {
            return Err(ModuleError::EvidenceContractTermMismatch(lane.term));
        }
        used_terms.insert(lane.term);
        match (&lane.kind, &lane.output_field) {
            (EvidenceContractLaneKind::Requires, None) => {}
            (EvidenceContractLaneKind::Ensures, Some(field))
                if !field.is_empty()
                    && field != "value"
                    && output_fields.insert((lane.machine, field.as_str())) => {}
            (EvidenceContractLaneKind::Requires, Some(_)) => {
                return Err(ModuleError::EvidenceRequiresHasOutputField {
                    machine: lane.machine,
                    position: lane.position,
                });
            }
            (EvidenceContractLaneKind::Ensures, None) => {
                return Err(ModuleError::MissingEvidenceOutputField {
                    machine: lane.machine,
                    position: lane.position,
                });
            }
            (EvidenceContractLaneKind::Ensures, Some(field)) if field == "value" => {
                return Err(ModuleError::ReservedEvidenceOutputField(lane.machine));
            }
            (EvidenceContractLaneKind::Ensures, Some(field)) if field.is_empty() => {
                return Err(ModuleError::InvalidEvidenceOutputField(lane.machine));
            }
            (EvidenceContractLaneKind::Ensures, Some(_)) => {
                return Err(ModuleError::DuplicateEvidenceOutputField(lane.machine));
            }
        }
        let expected = next_positions
            .entry((lane.machine, lane.kind))
            .or_insert(0_u32);
        if lane.position != *expected {
            return Err(ModuleError::NonDenseEvidenceContractLane {
                machine: lane.machine,
                kind: lane.kind,
                expected: *expected,
                actual: lane.position,
            });
        }
        *expected = expected
            .checked_add(1)
            .ok_or(ModuleError::EvidenceContractLaneOverflow {
                machine: lane.machine,
                kind: lane.kind,
            })?;
    }
    for machine in machines.values().copied() {
        let mut next_positions = BTreeMap::new();
        let mut previous_key = None;
        for row in &machine.contract.outcome_specific_ensures {
            validate_outcome_guard(module, machine, row.guard)?;
            let key = (row.guard.result_type, row.guard.result_case, row.position);
            if previous_key.is_some_and(|previous| previous >= key) {
                return Err(ModuleError::NonCanonicalOutcomeSpecificEnsures(machine.id));
            }
            previous_key = Some(key);
            let expected = next_positions.entry(row.guard).or_insert(0_u32);
            if row.position != *expected {
                return Err(ModuleError::NonDenseOutcomeSpecificEnsures {
                    machine: machine.id,
                    guard: row.guard,
                    expected: *expected,
                    actual: row.position,
                });
            }
            *expected =
                expected
                    .checked_add(1)
                    .ok_or(ModuleError::OutcomeSpecificEnsureOverflow {
                        machine: machine.id,
                        guard: row.guard,
                    })?;
            if let Some(evidence) = &row.evidence {
                if evidence.output_field.is_empty()
                    || evidence.output_field == "value"
                    || !output_fields.insert((machine.id, evidence.output_field.as_str()))
                {
                    return Err(ModuleError::InvalidOutcomeSpecificEvidenceField {
                        machine: machine.id,
                        position: row.position,
                    });
                }
                let Some(term) = terms.get(&evidence.term) else {
                    return Err(ModuleError::UnknownEvidenceContractTerm(evidence.term));
                };
                let application = module
                    .proposition_applications
                    .iter()
                    .find(|application| application.id == term.proposition)
                    .expect("evidence terms were validated before guarded rows");
                if row.proposition != Proposition::Atom(term.proposition)
                    || application.evidence_interface.as_ref() != Some(&term.interface)
                {
                    return Err(ModuleError::OutcomeSpecificEvidenceMismatch {
                        machine: machine.id,
                        position: row.position,
                    });
                }
                // Proposition terms are copyable. A guarded output may be the
                // exact forwarded identity of a required lane; selector and
                // interface validation above still keep the endpoint exact.
                used_terms.insert(evidence.term);
            }
        }
    }
    Ok(used_terms)
}
