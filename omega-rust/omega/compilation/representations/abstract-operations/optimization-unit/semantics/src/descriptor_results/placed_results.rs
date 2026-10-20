//! Retained interpretations of returned reference slots, from current bodies.
//!
//! A public reference-origin contract bounds possible backing, but does not say
//! whether the callee formed a new placed interpretation. The existing private
//! descriptor replay supplies that missing relation. Every possible origin of
//! each result slot must either retain an incoming interpretation or name an
//! exact whole-view formation. Neither a result type nor a placement catalog
//! entry establishes one. Storage addresses, loans and call authority remain
//! with the receiving call's independent reference/custody mapping.

use abstract_operations::AbstractOperation;
use semantic_vocabulary::{MachineId, OperationId, StructuralTypeId};
use terminal_psi::{
    PlacedRamCorrespondence, StructuralArgument, StructuralParameterDeclaration,
    StructuralPathSegment, StructuralTypeDeclaration, StructuralTypeShape,
};
use terminal_semantics::borrowed_storage::Projection;
use terminal_semantics::descriptor_lifetimes::descriptor_path;
use terminal_semantics::descriptor_results::{DescriptorResultSlot, DescriptorStoredOrigin};

#[derive(Debug)]
pub enum Source {
    Captured(PlacedRamCorrespondence),
    Incoming(StructuralArgument),
}

#[derive(Debug)]
pub struct ResultSlot {
    pub path: Vec<StructuralPathSegment>,
    /// Unknown alternatives stay present so they cannot hide a known conflict.
    pub sources: Vec<Option<Source>>,
}

/// Adapt the complete reference-slot summary, preserving each carrier location
/// separately from the projection of its referent. A record field containing a
/// whole view is not a projection into that view's backing.
pub fn summarize<'operation>(
    types: &[StructuralTypeDeclaration],
    parameters: &[StructuralParameterDeclaration],
    result_type: StructuralTypeId,
    slots: &[DescriptorResultSlot],
    formation: impl Fn(MachineId, OperationId) -> Option<&'operation AbstractOperation>,
) -> Option<Vec<ResultSlot>> {
    slots
        .iter()
        .map(|slot| {
            let path = descriptor_path(types, result_type, &slot.path)?;
            let StructuralTypeShape::Reference { referent, access } =
                terminal_semantics::runtime_structural_path_tip(types.iter(), result_type, &path)?
                    .shape
            else {
                return None;
            };
            let sources = slot
                .origins
                .iter()
                .map(|origin| match origin {
                    DescriptorStoredOrigin::PlacedFormation {
                        machine,
                        operation,
                        place,
                        carrier,
                        projection,
                    } => {
                        if projection != &Projection::new(&[]) || *carrier != referent {
                            return None;
                        }
                        let AbstractOperation::EstablishPlacedReference {
                            psi_operation,
                            result,
                            correspondence,
                            ..
                        } = formation(*machine, *operation)?
                        else {
                            return None;
                        };
                        if psi_operation != operation
                            || result.place != *place
                            || correspondence.carrier != *carrier
                            || !terminal_psi::find_structural_type(types, result.structural_type)
                                .is_some_and(|declaration| {
                                    matches!(declaration.shape,
                                    StructuralTypeShape::Reference { referent, .. }
                                        if referent == *carrier)
                                })
                        {
                            return None;
                        }
                        Some(Some(Source::Captured(correspondence.clone())))
                    }
                    DescriptorStoredOrigin::Parameter {
                        position,
                        projection,
                    } => {
                        if projection != &Projection::new(&[]) {
                            return None;
                        }
                        let parameter = parameters.get(*position)?;
                        if parameter.structural_type != referent || !access.admits(parameter.access)
                        {
                            return None;
                        }
                        Some(Some(Source::Incoming(StructuralArgument {
                            place: parameter.place,
                            path: Vec::new(),
                            access,
                        })))
                    }
                    DescriptorStoredOrigin::IncomingSlot { position, path } => {
                        let parameter = parameters.get(*position)?;
                        let mut path = descriptor_path(types, parameter.structural_type, path)?;
                        let StructuralTypeShape::Reference {
                            referent: incoming,
                            access: ceiling,
                        } = terminal_semantics::runtime_structural_path_tip(
                            types.iter(),
                            parameter.structural_type,
                            &path,
                        )?
                        .shape
                        else {
                            return None;
                        };
                        if incoming != referent || !access.admits(ceiling) {
                            return None;
                        }
                        path.push(StructuralPathSegment::Referent);
                        Some(Some(Source::Incoming(StructuralArgument {
                            place: parameter.place,
                            path,
                            access,
                        })))
                    }
                    DescriptorStoredOrigin::Empty => Some(None),
                    _ => None,
                })
                .filter_map(|source| match source {
                    Some(None) => None,
                    Some(Some(source)) => Some(Some(source)),
                    None => Some(None),
                })
                .collect();
            Some(ResultSlot { path, sources })
        })
        .collect()
}

/// All alternatives must answer the same interpretation obligation. This join
/// does not assert that their dynamic backing or invocation identities coincide.
pub fn resolve<Interpretation: PartialEq>(
    slots: &[ResultSlot],
    path: &[StructuralPathSegment],
    incoming: impl Fn(&StructuralArgument) -> Option<Interpretation>,
    captured: impl Fn(&PlacedRamCorrespondence) -> Interpretation,
) -> Option<Result<Interpretation, ConflictingInterpretations>> {
    let mut matching = slots.iter().filter(|slot| slot.path == path);
    let slot = matching.next()?;
    if matching.next().is_some() {
        return Some(Err(ConflictingInterpretations));
    }
    derive(slot.sources.iter().map(|source| match source {
        Some(Source::Captured(correspondence)) => Some(captured(correspondence)),
        Some(Source::Incoming(source)) => incoming(source),
        None => None,
    }))
}

#[derive(Debug, PartialEq, Eq)]
pub struct ConflictingInterpretations;

/// Private body alternatives are derivations, unlike a public backing ceiling.
/// Preserve a demonstrated contradiction even when another origin is unknown.
fn derive<Interpretation: PartialEq>(
    alternatives: impl IntoIterator<Item = Option<Interpretation>>,
) -> Option<Result<Interpretation, ConflictingInterpretations>> {
    let mut selected = None;
    let mut incomplete = false;
    for candidate in alternatives {
        let Some(candidate) = candidate else {
            incomplete = true;
            continue;
        };
        if selected
            .as_ref()
            .is_some_and(|selected| selected != &candidate)
        {
            return Some(Err(ConflictingInterpretations));
        }
        selected = Some(candidate);
    }
    if incomplete { None } else { selected.map(Ok) }
}

/// Unknown alternatives cannot be dropped from a complete interpretation join.
pub fn agree<Interpretation: PartialEq>(
    alternatives: impl IntoIterator<Item = Option<Interpretation>>,
) -> Option<Interpretation> {
    let mut selected = None;
    for candidate in alternatives {
        let candidate = candidate?;
        if selected
            .as_ref()
            .is_some_and(|selected| selected != &candidate)
        {
            return None;
        }
        selected = Some(candidate);
    }
    selected
}

/// A complete public whole-view forwarding contract and a complete private
/// result-slot derivation are independent evidence. Either can establish the
/// association; when both are available they must agree. A contradiction within
/// the private derivation cannot be replaced by a public ceiling. This preserves
/// opaque forwarding without letting a result type manufacture an interpretation.
pub fn reconcile<Interpretation: PartialEq>(
    public: Option<Interpretation>,
    private: Option<Result<Interpretation, ConflictingInterpretations>>,
) -> Option<Interpretation> {
    match (public, private) {
        (_, Some(Err(ConflictingInterpretations))) => None,
        (Some(public), Some(Ok(private))) => (public == private).then_some(private),
        (Some(interpretation), None) | (None, Some(Ok(interpretation))) => Some(interpretation),
        (None, None) => None,
    }
}
