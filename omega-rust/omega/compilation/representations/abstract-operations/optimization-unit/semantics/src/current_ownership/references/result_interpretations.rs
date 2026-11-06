//! A returned pointer keeps the interpretation established by its actual body.
//! This relation adds no loan permission: ordinary call-result custody still
//! binds the declared sources, while each call supplies its own live leaves.
use super::*;
use crate::descriptor_results::access_results;
use terminal_semantics::descriptor_results::{DescriptorAnalysis, DescriptorStoredOrigin};

pub(super) struct ResultInterpretation {
    machine: MachineId,
    issuers: Vec<MachineId>,
    parameters: Vec<(PlaceId, usize)>,
    slots: Option<Vec<access_results::ResultSlot>>,
}

pub(super) fn prepare<'unit>(
    functions: impl Clone + Iterator<Item = &'unit PsiOptimizationFunction>,
    boundaries: &[&terminal_psi::BoundaryMachineDeclaration],
    types: &abstract_operations::StructuralTypeCatalog,
    mut analysis: Option<&mut DescriptorAnalysis<'_>>,
) -> Vec<ResultInterpretation> {
    // Authenticate actual issuing coordinates once. An ambiguous operation
    // cannot supply a correspondence merely because its type happens to match.
    let mut operations = functions
        .clone()
        .flat_map(|function| {
            function.block_nodes().filter_map(|node| {
                Some((
                    (function.machine, node.operation.psi_operation()?),
                    &node.operation,
                ))
            })
        })
        .collect::<Vec<_>>();
    operations.sort_by_key(|(key, _)| *key);
    let formation = |machine, operation| {
        let key = (machine, operation);
        let first = operations.partition_point(|(candidate, _)| *candidate < key);
        let after = operations.partition_point(|(candidate, _)| *candidate <= key);
        (after - first == 1).then(|| operations[first].1)
    };
    let boundaries = boundaries.iter().copied().cloned().collect::<Vec<_>>();
    // CallAccesses supplies its already checked, increasing machine roster.
    functions
        .map(|function| {
            let machine = function.machine;
            let mut issuers = Vec::new();
            let slots =
                (|| {
                    let result = function.result.structural()?;
                    let slots = analysis.as_mut()?.result_slots(machine)?;
                    issuers.extend(slots.iter().flat_map(|slot| &slot.origins).filter_map(
                        |origin| match origin {
                            DescriptorStoredOrigin::AccessFormation { machine, .. } => {
                                Some(*machine)
                            }
                            _ => None,
                        },
                    ));
                    access_results::summarize(
                        types,
                        &boundaries,
                        &function.structural_parameters,
                        result.structural_type,
                        &slots,
                        formation,
                    )
                })();
            issuers.sort_unstable();
            issuers.dedup();
            let mut parameters = function
                .structural_parameters
                .iter()
                .enumerate()
                .map(|(index, formal)| (formal.place, index))
                .collect::<Vec<_>>();
            parameters.sort_unstable();
            ResultInterpretation {
                machine,
                issuers,
                parameters,
                slots,
            }
        })
        .collect()
}

pub(super) fn resolve(
    rows: &[ResultInterpretation],
    machine: MachineId,
    path: &[StructuralPathSegment],
    completed: impl Fn(MachineId) -> bool,
    incoming: impl Fn(usize, &StructuralArgument) -> Option<ReferenceInterpretation>,
    public: Option<ReferenceInterpretation>,
) -> ReferenceInterpretation {
    let private = rows
        .binary_search_by_key(&machine, |row| row.machine)
        .ok()
        .and_then(|position| {
            let row = &rows[position];
            // Constructors prepare possible provenance, not checked facts.
            // Every issuing body and the consuming callee must finish its own
            // prerequisite/CFG/return proof before a private derivation is used.
            if !completed(machine) || row.issuers.iter().any(|issuer| !completed(*issuer)) {
                return None;
            }
            access_results::resolve(
                row.slots.as_deref()?,
                path,
                |source| {
                    let first = row
                        .parameters
                        .partition_point(|(place, _)| *place < source.place);
                    let after = row
                        .parameters
                        .partition_point(|(place, _)| *place <= source.place);
                    if after - first != 1 {
                        return None;
                    }
                    incoming(row.parameters[first].1, source)
                },
                |backing| ReferenceInterpretation::Captured(backing.clone()),
            )
        });
    access_results::reconcile(public, private).unwrap_or(ReferenceInterpretation::Unavailable)
}

/// Resolve the authenticated whole incoming pointer at this call, before any
/// output replacement. Pointer contents or an equal root cannot replace it.
#[allow(clippy::too_many_arguments)]
pub(super) fn incoming(
    inputs: &CallReferenceInputs<'_, '_>,
    parameter: usize,
    arguments: &[StructuralArgument],
    source: &StructuralArgument,
    expected: StructuralTypeId,
) -> Option<ReferenceInterpretation> {
    let CallReferenceInputs {
        function: caller,
        types,
        block,
        node,
        live,
    } = *inputs;
    let mut actual = abstract_operations::calls::call_source_at(
        arguments,
        parameter,
        &source.path,
        source.access,
    )?;
    if actual.path.last() == Some(&StructuralPathSegment::Referent) {
        actual.path.pop();
        let slot = call_effects::resolve_slot(caller, types, block, node, live, &actual).ok()?;
        let reference_type = call_effects::slot_type(caller, types, &slot)?;
        if leaf_referent(types, reference_type, &[]) != Some(expected) {
            return None;
        }
        let mut residents = live.iter().filter(|reference| {
            reference.carrier == slot.place && reference.carrier_path == slot.path
        });
        let resident = residents.next()?;
        if residents.next().is_some()
            || resident.interpretation == ReferenceInterpretation::Unavailable
        {
            return None;
        }
        Some(resident.interpretation.clone())
    } else if reference_source_type(caller, types, &actual) == Some(expected) {
        Some(ReferenceInterpretation::Source(actual))
    } else {
        None
    }
}
