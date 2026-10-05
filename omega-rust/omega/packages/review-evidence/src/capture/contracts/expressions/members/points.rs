//! Declaration projection follows the checked precondition's entry scope.

use crate::capture::PackageReviewInput;
use crate::capture::contracts::facts::ContractProjectionContext;
use crate::capture::semantics::facts::exactly_one;
use checked_trees::{ContractProofFactKind, ContractProofFactOwner};
use diagnostics::Diagnostic;
use facts::{FactOrigin, ProgramPoint};

pub(super) fn contract_point(
    compilation: &PackageReviewInput<'_>,
    context: &ContractProjectionContext<'_>,
    fact: arena::Handle<typed_trees::domain::ProofFact>,
) -> Result<(ProgramPoint, Option<FactOrigin>), Vec<Diagnostic>> {
    let ContractProofFactOwner::Machine { machine_symbol } = context.owner else {
        return Ok((context.point, None));
    };
    if context.point != (ProgramPoint::Machine { machine_symbol }) {
        return Err(vec![Diagnostic::error(
            "reviewed machine contract has a different declaration point",
        )]);
    }
    let checked = exactly_one(
        compilation
            .facts
            .proof
            .contract_facts
            .iter()
            .filter_map(|(_, checked)| {
                (checked.owner == context.owner && checked.fact == fact).then_some(checked)
            }),
        "contract member path",
        "checked declaration fact",
    )?;
    let origin = Some(FactOrigin::MachineContract { machine_symbol });
    if checked.kind != ContractProofFactKind::Requires {
        return Ok((context.point, origin));
    }
    let machine = exactly_one(
        compilation
            .machines()
            .iter()
            .filter(|machine| machine.symbol == machine_symbol),
        "contract member path",
        "entry owner",
    )?;
    let entry = compilation.machine_states(machine).first().ok_or_else(|| {
        vec![Diagnostic::error(
            "reviewed precondition has no canonical entry state",
        )]
    })?;
    Ok((
        ProgramPoint::State {
            machine_symbol,
            state_symbol: entry.symbol,
        },
        origin,
    ))
}
