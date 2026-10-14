//! Call continuations retire the complement of an already transferred subtree.
//! Establishment and transfer belong to the shared occurrence ledger, not this
//! cleanup. The original owner, rather than the projected argument, dies here.
use super::owners::{TemporaryConsumption, TemporaryOwner};
use super::{
    CheckFacts, FlowPermissionEventFact, Multiplicity, PermissionAccess, PermissionClaimIdentity,
    PermissionEventKind, PermissionProvenance, SymbolHandle, TypeReferenceNode,
};
use crate::checks::type_multiplicity;
use language_semantics::PermissionEventSource;

#[allow(clippy::too_many_arguments)]
pub(in crate::checks::multiplicity) fn append_cleanup(
    program: &dyn validation::CheckingInput,
    facts: &mut CheckFacts,
    machine: SymbolHandle,
    state: SymbolHandle,
    consumer: PermissionEventSource,
    evaluation: checked_trees::FlowPermissionEvaluation,
    owners: &[TemporaryOwner],
    places: &mut [super::super::linear_obligations::LinearPlace],
    remaining: &super::super::cleanup_dispositions::RemainingCustody,
    permissions: &mut Vec<FlowPermissionEventFact>,
) -> Result<Vec<facts::PlaceRoot>, diagnostics::Diagnostic> {
    let mut retired = Vec::new();
    for owner in owners
        .iter()
        .rev()
        .filter(|owner| owner.consumer == consumer && owner.evaluation == evaluation)
    {
        let TemporaryConsumption::OwnedProjection { path } = &owner.consumption else {
            continue;
        };
        let root = facts::PlaceRoot::Expression(owner.expression);
        let statement = match consumer {
            PermissionEventSource::Call {
                statement_index, ..
            } => statement_index,
            _ => continue,
        };
        let mut residuals = Vec::new();
        if remaining.is_present(root, path)
            || complement(
                program,
                state,
                statement,
                &validation::CanonicalPlace {
                    root,
                    segments: Vec::new(),
                },
                path,
                &mut residuals,
            )
            .is_none()
        {
            return Err(diagnostics::Diagnostic::error(
                "projected temporary cleanup lacks its exact completed transfer and residual partition",
            ));
        }
        for path in residuals {
            if !remaining.is_present(root, &path) {
                continue;
            }
            permissions.push(FlowPermissionEventFact {
                expression: owner.expression,
                evaluation,
                machine_symbol: machine,
                state_symbol: state,
                source: consumer,
                kind: PermissionEventKind::AffineDrop,
                multiplicity: Multiplicity::Affine,
                access: PermissionAccess::Owned,
                claim_identity: PermissionClaimIdentity::Unknown,
                provenance: PermissionProvenance::Established {
                    machine_symbol: machine,
                    state_symbol: state,
                    source: owner.producer,
                },
                root,
                segments: facts.flow.ownership.segments.insert_many(path),
                obligation_live: false,
            });
        }
        for place in places.iter_mut().filter(|place| place.root == root) {
            place.live = false;
        }
        retired.push(root);
    }
    Ok(retired)
}

// Only traverse the selected branch. Untouched siblings are maximal residuals,
// not leaf expansion, and retain their exact authored field/index identities.
fn complement(
    program: &dyn ::validation::CheckingInput,
    state: SymbolHandle,
    statement: usize,
    root: &validation::CanonicalPlace,
    selected: &[facts::PlaceSegment],
    output: &mut Vec<Vec<facts::PlaceSegment>>,
) -> Option<()> {
    let Some(next) = selected.first() else {
        return Some(());
    };
    let reference = validation::canonical_place_type_reference(program, state, statement, root)?;
    let children = match program.type_reference_table.type_reference(reference) {
        TypeReferenceNode::Named { symbol, .. }
        | TypeReferenceNode::Generic {
            base_symbol: symbol,
            ..
        } => {
            let data = program.data_definition_by_symbol(*symbol)?;
            program
                .data_members(data)
                .iter()
                .filter(|member| {
                    !matches!(member,
                    typed_trees::data::DataMember::Field(field) if field.relevance.is_erased())
                })
                .map(|member| match member {
                    typed_trees::data::DataMember::Field(field) => {
                        Some(facts::PlaceSegment::Field {
                            symbol: field.symbol,
                        })
                    }
                    _ => None,
                })
                .collect::<Option<Vec<_>>>()?
        }
        TypeReferenceNode::FixedArray {
            length: typed_trees::types::FixedArrayLength::Literal(length),
            ..
        } if *length > 0 => (0..*length)
            .map(|index| facts::PlaceSegment::FixedIndex { index })
            .collect(),
        _ => return None,
    };
    if !children.contains(next) {
        return None;
    }
    for child in children.into_iter().rev() {
        let mut place = root.clone();
        place.segments.push(child);
        if child == *next {
            complement(program, state, statement, &place, &selected[1..], output)?;
        } else {
            let reference =
                validation::canonical_place_type_reference(program, state, statement, &place)?;
            if type_multiplicity(program, reference) == Multiplicity::Unrestricted {
                continue;
            }
            if type_multiplicity(program, reference) != Multiplicity::Affine {
                return None;
            }
            output.push(place.segments);
        }
    }
    Some(())
}
