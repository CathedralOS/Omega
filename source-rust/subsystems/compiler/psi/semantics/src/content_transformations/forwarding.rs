//! Check conservation guaranteed by forwarding an unchanged structural claim.
use super::{ancestry, declarations, entry_claims, structural_path};
use semantic_vocabulary::{
    ClaimId, ContentConservation, ContentPlaceVersion, ContentProjectionIdentity,
    ContentStructuralPlace, ContentTerm, Proposition,
};
use terminal_psi::{
    OperationKind, StructuralAccess, StructuralPathSegment, TerminalMachine, TerminalModule,
    Terminator,
};

/// Checked content obligations are indexed by the same clause ordinal as
/// scalar guarantees. Each receipt names all the original claims conserved on
/// every normal return. Bodies and saved artifacts use the same judgment.
pub fn checked_returns(
    module: &TerminalModule,
    machine: &TerminalMachine,
) -> Result<Vec<(usize, Vec<ClaimId>)>, &'static str> {
    checked_returns_with(module, None, machine)
}

/// `checked_returns` over the bundle's module. The ownership walks that
/// authenticate the machine's and its callees' claims reuse the bundle's
/// descriptor rows instead of adapting each closure again.
pub fn checked_returns_in(
    analyses: &crate::module_analyses::ModuleAnalyses<'_>,
    machine: &TerminalMachine,
) -> Result<Vec<(usize, Vec<ClaimId>)>, &'static str> {
    checked_returns_with(analyses.module(), Some(analyses), machine)
}

fn checked_returns_with(
    module: &TerminalModule,
    analyses: Option<&crate::module_analyses::ModuleAnalyses<'_>>,
    machine: &TerminalMachine,
) -> Result<Vec<(usize, Vec<ClaimId>)>, &'static str> {
    let mut proved = Vec::new();
    let mut origins = ancestry::OriginTraversal::unchanged_content().with_analyses(analyses);
    for (ordinal, clause) in machine.ensures.iter().enumerate() {
        if let Proposition::ContentConservation(law) = &clause.proposition {
            let input = if super::partition::is_interval_partition(law) {
                super::partition::checked_return(module, machine, law)?.inputs
            } else {
                vec![forwarded_projection(module, machine, law, &mut origins)?]
            };
            proved.push((ordinal, input));
        }
    }
    if !proved.is_empty() {
        validate_claims(module, analyses, machine)?;
    }
    Ok(proved)
}

/// `analyses`, when given, must be a bundle over `module`; a bundle over any
/// other module is ignored and the walk builds its own closure.
pub(crate) fn validate_claims(
    module: &TerminalModule,
    analyses: Option<&crate::module_analyses::ModuleAnalyses<'_>>,
    machine: &TerminalMachine,
) -> Result<(), &'static str> {
    declarations::contract(module, &machine.contract)?;
    entry_claims::validate_declared_entry_contents(
        &module.types.structural_types,
        &module.types.structural_domains,
        &module.identities,
        &machine.contract,
    )?;
    match analyses.filter(|analyses| std::ptr::eq(analyses.module(), module)) {
        Some(analyses) => analyses
            .affine(machine.id)
            .and_then(|walks| walks.verify_claims()),
        None => crate::plain_affine::verify_claims_in(module, machine.id),
    }
    .map(|_| ())
    .map_err(|_| "content return differs from its checked ownership flow")
}

fn projection_subjects(
    law: &ContentConservation,
) -> Result<
    (
        ContentProjectionIdentity,
        &ContentStructuralPlace,
        &ContentStructuralPlace,
    ),
    &'static str,
> {
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
    Ok((*left_projection, entry, output))
}

fn result_path(
    module: &TerminalModule,
    machine: &TerminalMachine,
    output: &ContentStructuralPlace,
) -> Result<Vec<StructuralPathSegment>, &'static str> {
    let result = machine
        .result
        .structural()
        .filter(|result| result.place == output.root)
        .ok_or("content forwarding does not observe the declared result")?;
    structural_path(
        module.types.structural_types.iter(),
        &module.identities,
        result.structural_type,
        &output.segments,
    )
    .ok_or("content forwarding refers to an unknown output path")
}

/// Identity forwarding proves a closed projection unchanged for any algebra.
/// Partition arithmetic and calls that transform storage owe a separate judgment.
fn forwarded_projection(
    module: &TerminalModule,
    machine: &TerminalMachine,
    law: &ContentConservation,
    origins: &mut ancestry::OriginTraversal,
) -> Result<ClaimId, &'static str> {
    let (projection, entry, output) = projection_subjects(law)?;
    let returned_path = result_path(module, machine, output)?;
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
                .any(|row| row.projection == projection && row.algebra == *law.algebra())
            && machine.entry_claims.iter().any(|owned| {
                owned.claim == binding.claim && owned.input == entry.root && owned.path == path
            })
    })
    .ok_or("content forwarding has no exact owned entry claim and projection")?;
    // These operations move existing claims or assemble them beneath new roots.
    // A write can retain the claim while changing the projected fields.
    if machine.operations().any(|op| {
        // An owned direct call can affect the observed content only through a
        // returned claim. Its exact published projection is checked at that use.
        // A boundary call is the same kind of step: its result claim relates
        // to a consumed input only through the boundary's declared unchanged
        // projection (`ancestry::boundary_input_origins`).
        let owned_call = (op.kind.direct_callee().is_some()
            || matches!(op.kind, OperationKind::BoundaryCall { .. }))
            && op.result.as_structural().is_some()
            && op
                .kind
                .structural_arguments()
                .iter()
                .all(|arg| arg.access == StructuralAccess::Owned);
        !owned_call
            && !matches!(
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
                let roots =
                    ancestry::local_roots(module, machine, binding.claim, projection, origins)?;
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

/// A caller can use the specific projection the callee promised on this
/// result field. Replay that guarantee before binding its input to an actual.
pub(super) fn call_input(
    module: &TerminalModule,
    callee: &TerminalMachine,
    path: &[StructuralPathSegment],
    projection: ContentProjectionIdentity,
    origins: &mut ancestry::OriginTraversal,
) -> Result<ClaimId, &'static str> {
    validate_claims(module, origins.analyses(), callee)?;
    let mut input = None;
    for clause in &callee.ensures {
        let Proposition::ContentConservation(law) = &clause.proposition else {
            continue;
        };
        let Ok((owner, _, output)) = projection_subjects(law) else {
            continue;
        };
        if owner == projection && result_path(module, callee, output)? == path {
            let claim = forwarded_projection(module, callee, law, origins)?;
            if input.replace(claim).is_some_and(|old| old != claim) {
                return Err("call result has conflicting content inputs");
            }
        }
    }
    input.ok_or("call result has no checked forwarding guarantee for its projection")
}
