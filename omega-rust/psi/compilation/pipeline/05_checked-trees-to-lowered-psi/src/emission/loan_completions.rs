//! Return custody to the caller's existing home after a normal exclusive call.
//! Preparation joins checked regions to the caller's published storage; emission
//! attaches the same catalogue to the exact call operation. Statement calls and
//! calls nested in scalar computations share this join, independently of result
//! shape or dispatch. Actual stored guards select live claims during execution.

use crate::emission::operation_emission::buffer::{OperationBuffer, SourceCallCoordinate};
use crate::emission::operation_emission::calls::CallEmissionContext;
use crate::lowering_error::{LoweringError, unsupported};
use checked_trees::CheckedTrees;
use language_semantics::{PermissionClaimIdentity, PermissionEventKind, PermissionEventSource};
use semantic_vocabulary::{PlaceId, StructuralTypeId};
use symbols::SymbolHandle;
use terminal_psi::{
    LoanCompletion, StructuralArgument, StructuralMultiplicity, StructuralPathSegment,
    StructuralResultClaimBinding, StructuralTypeDeclaration,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PreparedLoanCompletion {
    pub(crate) place: PlaceId,
    pub(crate) path: Vec<StructuralPathSegment>,
    pub(crate) multiplicity: StructuralMultiplicity,
    pub(crate) claims: Vec<(PermissionClaimIdentity, Vec<StructuralPathSegment>, bool)>,
    pub(crate) qualifications: Vec<(
        language_semantics::SemanticDomainId,
        Vec<StructuralPathSegment>,
        Option<StructuralTypeId>,
    )>,
}

pub(crate) fn prepare(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    checked: &CheckedTrees,
    machine: SymbolHandle,
    coordinate: SourceCallCoordinate,
    types: &[StructuralTypeDeclaration],
    mut owned_home: impl FnMut(
        facts::PlaceRoot,
    ) -> Result<(StructuralArgument, StructuralTypeId), LoweringError>,
) -> Result<Vec<PreparedLoanCompletion>, LoweringError> {
    let mut completions = Vec::new();
    for (_, region) in checked.facts.flow.ownership.loan_completions.iter() {
        if region.machine != machine
            || region.state != coordinate.state
            || !matches!(region.source, PermissionEventSource::Call { statement_index, call_ordinal, .. }
                if statement_index == coordinate.statement_index && call_ordinal == coordinate.call_ordinal)
        {
            continue;
        }
        let reference = validation::canonical_place_type_reference(
            &checked.typed,
            coordinate.state,
            coordinate.statement_index,
            &validation::CanonicalPlace {
                root: region.root,
                segments: Vec::new(),
            },
        )
        .ok_or(LoweringError::Unsupported(
            "loan completion lost its owned root type",
        ))?;
        let (owner, structural_type) = owned_home(region.root)?;
        if owner.access != terminal_psi::StructuralAccess::Owned {
            return unsupported("loan completion cannot establish borrowed ingress custody");
        }
        let canonical = checked
            .facts
            .flow
            .ownership
            .segments
            .span_or_empty(region.segments);
        let (path, _) = validation::structural_owned_path(&checked.typed, reference, canonical)
            .map_err(LoweringError::Unsupported)?;
        let mut completed_path = owner.path.clone();
        completed_path.extend(
            crate::expression_preparation::bindings::structural_paths::lower_static_argument_path(
                identities,
                &path,
                structural_type,
                types,
            )?,
        );
        let mut claims = Vec::new();
        for (_, event) in checked.facts.flow.ownership.permissions.iter() {
            let event_path = checked
                .facts
                .flow
                .ownership
                .segments
                .span_or_empty(event.segments);
            if event.machine_symbol != machine
                || event.state_symbol != coordinate.state
                || event.source != region.source
                || event.root != region.root
                || event.kind != PermissionEventKind::Restore
                || !event_path.starts_with(canonical)
            {
                continue;
            }
            let path = validation::structural_claim_path(&checked.typed, reference, event_path)
                .map_err(LoweringError::Unsupported)?;
            let mut terminal_path = owner.path.clone();
            terminal_path.extend(crate::expression_preparation::bindings::structural_paths::lower_static_argument_path(
                identities, &path, structural_type, types,
            )?);
            let introduces = crate::emission::claim_bindings::claim_established_here(
                machine,
                coordinate.state,
                coordinate.statement_index,
                event,
            );
            claims.push((event.claim_identity, terminal_path, introduces));
        }
        let point = facts::ProgramPoint::CallEnsures {
            machine_symbol: machine,
            state_symbol: coordinate.state,
            statement_index: coordinate.statement_index,
            call_ordinal: coordinate.call_ordinal,
        };
        let mut qualifications = Vec::new();
        for (_, fact) in checked.facts.semantic.facts.iter() {
            if fact.origin != facts::FactOrigin::CallEnsures || fact.point != point {
                continue;
            }
            let Some(place_handle) = fact.place.as_place() else {
                continue;
            };
            let place = checked.facts.semantic.places.get(place_handle);
            if place.root != region.root {
                continue;
            }
            let segments = checked
                .facts
                .semantic
                .place_segments
                .span_or_empty(place.segments);
            if !segments.starts_with(canonical) {
                continue;
            }
            let Some((_, _, _domain_symbol, semantic_domain)) = fact.payload.domain_membership()
            else {
                continue;
            };
            let (fact_path, _) =
                validation::structural_owned_path(&checked.typed, reference, segments)
                    .map_err(LoweringError::Unsupported)?;
            let mut full_path = owner.path.clone();
            full_path.extend(
                crate::expression_preparation::bindings::structural_paths::lower_static_argument_path(
                    identities,
                    &fact_path,
                    structural_type,
                    types,
                )?,
            );
            let carrier = if full_path.is_empty() {
                Some(structural_type)
            } else {
                terminal_semantics::qualification_path_carrier(
                    types.iter(),
                    structural_type,
                    &full_path,
                )
            };
            qualifications.push((semantic_domain, full_path, carrier));
        }
        completions.push(PreparedLoanCompletion {
            place: owner.place,
            path: completed_path,
            claims,
            qualifications,
            multiplicity: match region.multiplicity {
                language_semantics::Multiplicity::Unrestricted => {
                    StructuralMultiplicity::Unrestricted
                }
                language_semantics::Multiplicity::Affine => StructuralMultiplicity::Affine,
                language_semantics::Multiplicity::Linear => StructuralMultiplicity::Linear,
            },
        });
    }
    Ok(completions)
}

pub(crate) fn emit(
    prepared: &[PreparedLoanCompletion],
    coordinate: SourceCallCoordinate,
    operations: &mut OperationBuffer,
    calls: &mut CallEmissionContext<'_>,
    current_claims: &mut crate::emission::current_claims::CurrentClaimNames,
) -> Result<(), LoweringError> {
    if prepared.is_empty() {
        return Ok(());
    }
    let occurrence = operations
        .source_calls
        .iter()
        .find(|occurrence| {
            occurrence.source_state == coordinate.state
                && occurrence.statement_index == coordinate.statement_index
                && occurrence.call_ordinal == coordinate.call_ordinal
        })
        .ok_or(LoweringError::Unsupported(
            "loan completion has no emitted call",
        ))?;
    let lowered_psi::LoweredSourceCallSite::Operation(operation) = occurrence.terminal_site else {
        return unsupported("loan completion has no returning operation");
    };
    for completion in prepared {
        let mut claims = completion
            .claims
            .iter()
            .map(|(identity, path, introduces)| {
                let claim = calls.claims.receive_loan_completion_claim(*identity)?;
                if *introduces {
                    operations.claim_introductions.push(
                        crate::emission::claim_bindings::ClaimIntroduction { operation, claim },
                    );
                }
                Ok(StructuralResultClaimBinding {
                    claim,
                    path: path.clone(),
                })
            })
            .collect::<Result<Vec<_>, LoweringError>>()?;
        claims.sort();
        let relative = claims
            .iter()
            .map(|claim| {
                let path = claim.path.strip_prefix(completion.path.as_slice()).ok_or(
                    LoweringError::Unsupported("loan completion claim escapes its exact region"),
                )?;
                Ok(StructuralResultClaimBinding {
                    claim: claim.claim,
                    path: path.to_vec(),
                })
            })
            .collect::<Result<Vec<_>, LoweringError>>()?;
        current_claims.replace(completion.place, &completion.path, &relative)?;
        operations.loan_completions.push(LoanCompletion {
            operation,
            place: completion.place,
            path: completion.path.clone(),
            multiplicity: completion.multiplicity,
            claims,
        });

        let mut qualifications = Vec::new();
        let mut projected_qualifications = Vec::new();
        for (semantic_domain, path, carrier) in &completion.qualifications {
            let domain = if let Some(carrier) = carrier {
                crate::terminal_identities::lookup_domain_id_for_carrier(
                    calls.domain_ids,
                    *semantic_domain,
                    *carrier,
                )
                .or_else(|_| {
                    crate::terminal_identities::lookup_domain_id(calls.domain_ids, *semantic_domain)
                })?
            } else {
                crate::terminal_identities::lookup_domain_id(calls.domain_ids, *semantic_domain)?
            };
            if path.is_empty() {
                qualifications.push(domain);
            } else {
                projected_qualifications.push(terminal_psi::StructuralPathQualification {
                    path: path.clone(),
                    domain,
                });
            }
        }
        qualifications.sort();
        qualifications.dedup();
        projected_qualifications.sort();
        projected_qualifications.dedup();
        if !qualifications.is_empty() || !projected_qualifications.is_empty() {
            operations.add_structural_qualifications(
                completion.place,
                &qualifications,
                &projected_qualifications,
            );
        }
    }
    operations.loan_completions.sort_by(|left, right| {
        (left.operation, left.place, &left.path).cmp(&(right.operation, right.place, &right.path))
    });
    Ok(())
}
