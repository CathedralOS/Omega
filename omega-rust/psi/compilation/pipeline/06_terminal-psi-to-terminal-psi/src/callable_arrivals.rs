//! Ordinary call proofs for installed continuations. Their complete source
//! environment was prepared in Psi 05; this stage reads only the canonical
//! witness and asks the same discharge and kernel as every other call.
//! A successful proof returns no executable closure or installation authority.

use semantic_vocabulary::MachineId;
use terminal_check_tables::compilation::CallableArrivalWitness;
use terminal_check_tables::{Checked, Construct, RefusalReason};

pub(crate) fn check(
    witnesses: Vec<Checked<CallableArrivalWitness>>,
) -> Checked<Vec<crate::UnprovedObligation>> {
    let mut unproved = Vec::new();
    let mut unavailable = None;
    for witness in witnesses {
        match witness.and_then(check_witness) {
            Ok(failures) => unproved.extend(failures),
            Err(refused) => {
                unavailable.get_or_insert(refused);
            }
        }
    }
    // A missing part of any binding's premise theory cannot turn failed
    // search under the other premises into a source rejection.
    match unavailable {
        Some(refused) => Err(refused),
        None => Ok(unproved),
    }
}

fn check_witness(mut witness: CallableArrivalWitness) -> Checked<Vec<crate::UnprovedObligation>> {
    let diagnostic = MachineId::new(1).expect("nonzero diagnostic identity");
    for nominal in &witness.nominal_premises {
        if nominal.path.root != nominal.occurrence.symbol
            || witness.members.span(nominal.path.members).is_none()
            || witness
                .required_inputs
                .get(nominal.occurrence.ordinal)
                .is_none_or(|formal| formal.occurrence != nominal.occurrence)
        {
            return Err(crate::refusal(
                RefusalReason::ForeignIdentity,
                diagnostic,
                Construct::Machine,
                "logical nominal premise lost its exact source input correspondence",
            ));
        }
    }
    let elaboration = &mut witness.elaboration;
    let discharged = crate::discharge(elaboration)?;
    let mut unproved = Vec::new();
    for (machine, result) in discharged.machines.iter() {
        if elaboration.requirements.contains(&machine) {
            continue;
        }
        let tables = terminal_check_tables::row(
            &elaboration.tables.machines,
            machine,
            machine,
            Construct::Machine,
        )?;
        let sources =
            terminal_check_tables::row(&elaboration.sources, machine, machine, Construct::Machine)?;
        for (obligation, verdict) in result.verdicts.iter() {
            let Err(refused) = verdict else { continue };
            let RefusalReason::Unproved(cause) = refused.reason else {
                return Err(refused.clone());
            };
            let row = terminal_check_tables::row(
                &tables.obligations,
                obligation,
                machine,
                Construct::Obligation(obligation),
            )?;
            let site = *terminal_check_tables::row(
                &sources.obligations,
                obligation,
                machine,
                Construct::Obligation(obligation),
            )?;
            unproved.push(crate::UnprovedObligation {
                machine,
                obligation,
                kind: row.kind,
                site,
                cause,
                detail: refused.detail.clone(),
                statement: crate::describe::unproved(elaboration, machine, tables, row.kind, site),
                consumed: None,
            });
        }
        for used in &result.uses {
            used.verdict.as_ref().map_err(Clone::clone)?;
        }
        for consumed in &result.windows {
            consumed.verdict.as_ref().map_err(Clone::clone)?;
        }
        if !result.conflicts.is_empty() {
            return Err(crate::refusal(
                RefusalReason::Unsupported,
                machine,
                Construct::Machine,
                "logical call witness needs its complete loan judgment",
            ));
        }
    }
    Ok(unproved)
}
