//! An executable predicate may examine routed membership only when this
//! completed operand retains its authorized provenance on every incoming path.

use super::{
    Checked, Construct, Discharged, Graph, MachineId, MachineTables, RefusalReason, refusal,
};
use terminal_check_tables::obligations::{MembershipSubject, ObligationKind};
use terminal_psi::TerminalModule;
use terminal_semantics::{MembershipTest, TestedMembership};

pub(super) fn judge(
    module: &TerminalModule,
    machine: MachineId,
    graph: &Graph,
    tables: &MachineTables,
) -> Checked<Vec<(usize, Checked<Discharged>)>> {
    let mut questions = Vec::new();
    let mut obligations = Vec::new();
    for (index, (id, row)) in tables.obligations.iter().enumerate() {
        let ObligationKind::ExecutableMembership { subject } = row.kind else {
            continue;
        };
        let (block, position) = graph.locate(row.created_at)?;
        let position = u32::try_from(position).map_err(|_| {
            refusal(
                RefusalReason::ForeignIdentity,
                machine,
                Construct::Obligation(id),
                "membership test has an unrepresentable operation position",
            )
        })?;
        let subject = match subject {
            MembershipSubject::Scalar { value, domain } => {
                TestedMembership::Scalar { value, domain }
            }
            MembershipSubject::Structural { place, domain } => {
                let place = super::places::lineage(machine, &tables.places, place)?
                    .descriptor_location()
                    .ok_or_else(|| {
                        refusal(
                            RefusalReason::Unsupported,
                            machine,
                            Construct::Obligation(id),
                            "executable membership test needs an exact captured storage projection",
                        )
                    })?;
                TestedMembership::Structural { place, domain }
            }
        };
        questions.push(MembershipTest {
            position: (block, position),
            subject,
        });
        obligations.push((index, id));
    }
    let answered =
        terminal_semantics::test_memberships(module, machine, &questions).map_err(|detail| {
            refusal(
                RefusalReason::ForeignIdentity,
                machine,
                Construct::Machine,
                detail,
            )
        })?;
    let verdicts = obligations.into_iter().zip(answered).map(|((index, id), held)| {
        let result = if held {
            Ok(Discharged::RetainedMembership)
        } else {
            Err(refusal(
                RefusalReason::Unestablished,
                machine,
                Construct::Obligation(id),
                "this executable membership test requires retained authorized provenance on the tested value",
            ))
        };
        (index, result)
    });
    Ok(verdicts.collect())
}
