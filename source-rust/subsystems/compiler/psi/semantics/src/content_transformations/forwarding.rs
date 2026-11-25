//! Check conservation guaranteed by forwarding an unchanged structural claim.
use super::{ancestry, declarations, entry_claims, structural_path};
use semantic_vocabulary::{ContentConservation, ContentPlaceVersion, ContentTerm, Proposition};
use terminal_psi::{OperationKind, StructuralAccess, TerminalMachine, TerminalModule, Terminator};

/// Checked content obligations are indexed by the same clause ordinal as
/// scalar guarantees. Each receipt names the actual claim that reaches every
/// normal return. Bodies and saved artifacts run the same custody judgment.
pub fn checked_returns(
    module: &TerminalModule,
    machine: &TerminalMachine,
) -> Result<Vec<(usize, semantic_vocabulary::ClaimId)>, &'static str> {
    let mut proved = Vec::new();
    for (ordinal, clause) in machine.ensures.iter().enumerate() {
        if let Proposition::ContentConservation(law) = &clause.proposition {
            proved.push((ordinal, forwarded_projection(module, machine, law)?));
        }
    }
    if !proved.is_empty() {
        declarations::contract(module, &machine.contract)?;
        entry_claims::validate_declared_entry_contents(
            &module.types.structural_types,
            &module.types.structural_domains,
            &module.identities,
            &machine.contract,
        )?;
        crate::plain_affine::verify_claims_in(module, machine.id)
            .map_err(|_| "content return differs from its checked ownership flow")?;
    }
    Ok(proved)
}

/// Identity forwarding proves a closed projection unchanged for any algebra.
/// Partition arithmetic and calls that transform storage owe a separate judgment.
fn forwarded_projection(
    module: &TerminalModule,
    machine: &TerminalMachine,
    law: &ContentConservation,
) -> Result<semantic_vocabulary::ClaimId, &'static str> {
    let (
        ContentTerm::Projection {
            projection: left_projection,
            subject: left_subject,
        },
        ContentTerm::Projection {
            projection: right_projection,
            subject: right_subject,
        },
    ) = (law.left(), law.right())
    else {
        return Err("content equation needs a proof for changed or partitioned storage");
    };
    let (entry, output) = match (left_subject.version, right_subject.version) {
        (ContentPlaceVersion::Entry, ContentPlaceVersion::Current) => (left_subject, right_subject),
        (ContentPlaceVersion::Current, ContentPlaceVersion::Entry) => (right_subject, left_subject),
        _ => return Err("content forwarding needs an entry input and the normal result"),
    };
    if left_projection != right_projection {
        return Err("content forwarding changes the owner projection at the result");
    }
    let result = machine
        .result
        .structural()
        .filter(|result| result.place == output.root)
        .ok_or("content forwarding does not observe the declared result")?;
    let returned_path = structural_path(
        module.types.structural_types.iter(),
        &module.identities,
        result.structural_type,
        &output.segments,
    )
    .ok_or("content forwarding refers to an unknown output path")?;
    let formal = machine
        .structural_parameter(entry.root)
        .filter(|formal| formal.access == StructuralAccess::Owned)
        .ok_or("content forwarding needs the actual owned entry parameter")?;
    let path = structural_path(
        module.types.structural_types.iter(),
        &module.identities,
        formal.structural_type,
        &entry.segments,
    )
    .ok_or("content forwarding refers to an unknown structural path")?;
    if path
        .iter()
        .chain(&returned_path)
        .any(|step| matches!(step, terminal_psi::StructuralPathSegment::Case(_)))
    {
        return Err(
            "content forwarding needs its projected case established on every normal return",
        );
    }
    let claim = crate::exactly_one::exactly_one(machine.content_entry_claims.iter(), |binding| {
        binding.input == *entry
            && binding
                .projections
                .iter()
                .any(|row| row.projection == *left_projection && row.algebra == *law.algebra())
            && machine.entry_claims.iter().any(|owned| {
                owned.claim == binding.claim && owned.input == entry.root && owned.path == path
            })
    })
    .ok_or("content forwarding has no exact owned entry claim and projection")?;
    // These operations move existing claims or assemble them beneath new roots.
    // A write can retain the claim while changing the projected fields.
    if machine.operations().any(|op| {
        !matches!(
            op.kind,
            OperationKind::MoveStructuralValue { .. }
                | OperationKind::MoveStructuralField { .. }
                | OperationKind::DecomposeStructuralValue { .. }
                | OperationKind::EstablishRecord { .. }
                | OperationKind::EstablishStructuralCase { .. }
                | OperationKind::EstablishStructuralArray { .. }
                | OperationKind::StructuralLeafCopy { .. }
                | OperationKind::PrimitiveScalarRead { .. }
        )
    }) {
        return Err(
            "content equation needs a proof for operations outside unchanged structural forwarding",
        );
    }
    let mut returned = false;
    let mut origins = ancestry::OriginTraversal::new();
    for block in &machine.blocks {
        if block.terminator.cleanup_rosters().any(|(_, actions)| {
            actions
                .iter()
                .any(|action| action.invoked_machine().is_some())
        }) {
            return Err("content forwarding cannot cross an unexamined cleanup call");
        }
        match &block.terminator {
            Terminator::ReturnStructural {
                returned_claims, ..
            } => {
                let binding = crate::exactly_one::exactly_one(returned_claims.iter(), |binding| {
                    binding.path == returned_path
                })
                .ok_or("normal return lacks the claim at the declared result path")?;
                let roots = ancestry::local_roots(
                    module,
                    machine,
                    binding.claim,
                    *left_projection,
                    &mut origins,
                )?;
                if roots != [claim.claim] {
                    return Err(
                        "normal return does not forward the content claim of the promised input",
                    );
                }
                returned = true;
            }
            Terminator::Jump { .. }
            | Terminator::Conditional { .. }
            | Terminator::StructuralCase { .. }
            | Terminator::Crash { .. }
            | Terminator::EliminateEmpty { .. } => {}
            _ => {
                return Err(
                    "content forwarding needs an authenticated structural return or clean transfer",
                );
            }
        }
    }
    if !returned {
        return Err("content provider has no checked normal structural return");
    }
    Ok(claim.claim)
}
