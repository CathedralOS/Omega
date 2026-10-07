//! Reserve each computed range's descriptor, then establish it at its formal
//! position. Reservation does not evaluate bounds or allocate element backing.
//! The existing subslice effect proves bounds against the original live view.
use super::{CheckedTrees, LoweringError, unsupported};
use crate::unit::attached_unit::view_ranges;
use checked_trees::expression::ExpressionHandle;
use checked_trees::{
    CheckedScalarComputationKind, CheckedScalarComputationStructuralArgument,
    CheckedUnitStructuralArgumentPlan,
};
use semantic_vocabulary::{PlaceId, StructuralTypeId};
use terminal_psi::StructuralTypeDeclaration;

#[derive(Clone)]
pub(crate) struct Slot {
    pub(super) expression: ExpressionHandle,
    pub(super) place: PlaceId,
    pub(super) structural_type: StructuralTypeId,
    bytes: bool,
    storage: view_ranges::RangeStorage,
    whole_view: Option<PlaceId>,
}

pub(super) use crate::unit::attached_unit::view_ranges::range;

pub(crate) fn prepare(
    checked: &CheckedTrees,
    machines: &[symbols::SymbolHandle],
    types: &[StructuralTypeDeclaration],
    next_place: &mut u64,
) -> Result<Vec<Slot>, LoweringError> {
    let plans = &checked.facts.values.scalar_computations;
    let mut slots: Vec<Slot> = Vec::new();
    for (_, root) in plans
        .roots
        .iter()
        .filter(|(_, root)| machines.contains(&root.machine))
    {
        for handle in super::reachable_nodes(checked, &[root.root])? {
            let arguments: Vec<_> = match &plans.nodes.get(handle).kind {
                CheckedScalarComputationKind::Call {
                    structural_arguments,
                    ..
                } => plans
                    .structural_arguments
                    .span(*structural_arguments)
                    .ok_or(LoweringError::Unsupported(
                        "computed range has stale structural operands",
                    ))?
                    .iter()
                    .filter_map(CheckedScalarComputationStructuralArgument::as_place)
                    .collect(),
                CheckedScalarComputationKind::ByteSequenceLiteralComparison { subject, .. }
                | CheckedScalarComputationKind::ByteSequencePredicate { subject, .. } => {
                    vec![subject]
                }
                _ => Vec::new(),
            };
            for argument in arguments {
                let Some(range) = range(argument) else {
                    continue;
                };
                let expression = range.expression;
                let bytes = range.family
                    == crate::emission::operation_emission::view_subslice::ViewFamily::Bytes;
                if let Some(existing) = slots.iter().find(|slot| slot.expression == expression) {
                    let declaration = types
                        .iter()
                        .find(|declaration| declaration.identity == argument.type_identity)
                        .ok_or(LoweringError::Unsupported(
                            "computed range has no structural type namespace",
                        ))?;
                    if existing.structural_type != declaration.id || existing.bytes != bytes {
                        return unsupported("computed range occurrence has conflicting types");
                    }
                    continue;
                }
                slots.push(reserve(
                    checked,
                    root.state,
                    root.statement_ordinal,
                    argument,
                    types,
                    next_place,
                )?);
            }
        }
    }
    Ok(slots)
}

fn reserve(
    checked: &CheckedTrees,
    state: symbols::SymbolHandle,
    statement: u32,
    argument: &CheckedUnitStructuralArgumentPlan,
    types: &[StructuralTypeDeclaration],
    next_place: &mut u64,
) -> Result<Slot, LoweringError> {
    let range =
        range(argument).ok_or(LoweringError::Unsupported("computed range lost its source"))?;
    let bytes =
        range.family == crate::emission::operation_emission::view_subslice::ViewFamily::Bytes;
    let declaration = types
        .iter()
        .find(|declaration| declaration.identity == argument.type_identity)
        .ok_or(LoweringError::Unsupported(
            "computed range has no structural type namespace",
        ))?;
    let storage = view_ranges::storage(checked, state, statement, argument, types)?;
    let whole_view = storage
        .requires_establishment()
        .then(|| super::super::allocate_dense(next_place).map(super::super::place_id))
        .transpose()?;
    Ok(Slot {
        expression: range.expression,
        place: super::super::place_id(super::super::allocate_dense(next_place)?),
        structural_type: declaration.id,
        bytes,
        storage,
        whole_view,
    })
}

/// A range's authored position is independent of the scalar lanes it produces.
/// Endpoint temporaries are consumed by descriptor formation, not the successor.
pub(crate) struct Scheduled {
    pub(super) argument_ordinal: u32,
    pub(super) argument: CheckedUnitStructuralArgumentPlan,
    pub(super) slot: Slot,
    pub(super) site: checked_trees::CheckedSubsliceSite,
}

impl Scheduled {
    pub(crate) fn completed_argument(&self) -> terminal_psi::StructuralArgument {
        terminal_psi::StructuralArgument {
            place: self.slot.place,
            path: Vec::new(),
            access: crate::expression_preparation::bindings::structural_paths::terminal_access(
                self.argument.access,
            ),
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn transfer(
    checked: &CheckedTrees,
    state: symbols::SymbolHandle,
    statement: u32,
    transfer: &checked_trees::CheckedStructuralControlTransferSourcePlan,
    target: &checked_trees::CheckedUnitStructuralParameterPlan,
    bindings: &crate::expression_preparation::bindings::ScalarBindings,
    types: &[StructuralTypeDeclaration],
    next_place: &mut u64,
) -> Result<Scheduled, LoweringError> {
    use checked_trees::{
        CheckedStructuralControlTransferSourcePlan as Transfer,
        CheckedUnitStructuralArgumentSourcePlan as Source,
    };
    let (source, path, expression) = match transfer {
        Transfer::ByteSequenceSubslice {
            root,
            path,
            expression,
            storage_type_identity,
        } => (
            Source::ByteSequenceSubslice {
                root: *root,
                expression: *expression,
                storage_type_identity: storage_type_identity.clone(),
                start: None,
                end: None,
            },
            path.clone(),
            *expression,
        ),
        Transfer::ElementViewSubslice {
            root,
            path,
            expression,
            storage_type_identity,
        } => (
            Source::ElementViewSubslice {
                root: *root,
                expression: *expression,
                storage_type_identity: storage_type_identity.clone(),
                start: None,
                end: None,
            },
            path.clone(),
            *expression,
        ),
        _ => return unsupported("scalar transfer has no range operation"),
    };
    let mut argument = CheckedUnitStructuralArgumentPlan {
        source,
        path,
        type_identity: target.type_identity.clone(),
        access: target.access,
    };
    let slot = reserve(checked, state, statement, &argument, types, next_place)?;
    slot.storage
        .bind(checked, state, bindings, slot.whole_view, types)?;
    let site = checked_trees::CheckedSubsliceSite::TransitionArgument {
        argument_ordinal: target.position,
    };
    let (start, end) = view_ranges::replay(
        checked,
        &view_ranges::ViewRangeSite {
            state,
            statement,
            site,
            expression,
            retained: None,
        },
    )?;
    match &mut argument.source {
        Source::ByteSequenceSubslice {
            start: retained_start,
            end: retained_end,
            ..
        }
        | Source::ElementViewSubslice {
            start: retained_start,
            end: retained_end,
            ..
        } => {
            *retained_start = start;
            *retained_end = end;
        }
        _ => return unsupported("scalar transfer lost its range operation"),
    }
    Ok(Scheduled {
        argument_ordinal: target.position,
        argument,
        slot,
        site,
    })
}

impl super::Expansion<'_> {
    /// Capture each endpoint once, in start/end order, and retire those scalar
    /// temporaries only after the ordinary descriptor operation consumes them.
    pub(super) fn subslice(
        &mut self,
        argument: &CheckedUnitStructuralArgumentPlan,
        slot: &Slot,
        range_site: checked_trees::CheckedSubsliceSite,
        input_types: &[super::QualifiedScalarType],
        target: usize,
        site: &super::Site<'_>,
        active: &mut Vec<super::Computation>,
    ) -> Result<usize, LoweringError> {
        let range = range(argument).ok_or(LoweringError::Unsupported(
            "computed range lost its retained source",
        ))?;
        let bytes =
            range.family == crate::emission::operation_emission::view_subslice::ViewFamily::Bytes;
        if slot.expression != range.expression || slot.bytes != bytes {
            return unsupported("computed range disagrees with its descriptor reservation");
        }
        let (source, establishment) = slot.storage.bind(
            self.checked,
            site.state,
            site.bindings,
            slot.whole_view,
            self.structural_types,
        )?;
        let (start, end) = view_ranges::replay(
            self.checked,
            &view_ranges::ViewRangeSite {
                state: site.state,
                statement: site.statement,
                site: range_site,
                expression: range.expression,
                retained: Some((range.start, range.end)),
            },
        )?;
        let scalar_type =
            super::super::terminal_scalar_type(checked_trees::types::PrimitiveType::U64)?;
        let mut endpoints = Vec::with_capacity(2);
        let mut endpoint_types = input_types.to_vec();
        let mut completed = [None, None];
        for (index, endpoint) in [start, end].into_iter().enumerate() {
            let Some(endpoint) = endpoint else {
                continue;
            };
            let argument = match endpoint {
                checked_trees::CheckedCallScalarArgument::Pure(value) => super::Argument::Value(
                    site.bindings
                        .expression(&self.checked.facts.proven_scalar_indexes, &value)?,
                ),
                checked_trees::CheckedCallScalarArgument::Computation(handle) => {
                    super::Argument::Computation(handle)
                }
            };
            let value_type = self.argument_type(&argument, site, input_types)?;
            if value_type.scalar_type != scalar_type {
                return unsupported("computed range endpoint lost its count type");
            }
            endpoints.push(argument);
            completed[index] = Some(super::parameter(endpoint_types.len(), value_type));
            endpoint_types.push(value_type);
        }
        let [start, end] = completed;
        let end = view_ranges::exclusive_end(self.checked, range.expression, end)?;
        let start = start.unwrap_or(
            crate::emission::operation_emission::expressions::LoweredDirectExpression::IntegerLiteral {
                value: semantic_vocabulary::IntegerValue::Unsigned(0),
                scalar_type,
            },
        );
        use crate::scalar_graph::scalar_graph_lowering::prepared_graph::{
            LoweredScalarBranchState, LoweredScalarBranchTerminator, LoweredScalarEffect,
        };
        let effect = if bytes {
            LoweredScalarEffect::ByteSequenceSubslice {
                source: source.place,
                access: source.access,
                start,
                end,
                place: slot.place,
                structural_type: slot.structural_type,
            }
        } else {
            LoweredScalarEffect::ElementViewSubslice {
                source: source.place,
                access: source.access,
                start,
                end,
                place: slot.place,
                structural_type: slot.structural_type,
            }
        };
        let finish = self.push(LoweredScalarBranchState {
            structural_parameters: Vec::new(),
            parameter_types: endpoint_types,
            erased_formal_types: Vec::new(),
            erased_proof_formals: Vec::new(),
            bindings: Vec::new(),
            structural_effects: vec![effect],
            terminator: LoweredScalarBranchTerminator::Jump {
                cleanup_actions: Vec::new(),
                target,
                arguments: super::parameters(input_types),
                erased_arguments: Vec::new(),
                erased_proof_arguments: Vec::new(),
                structural_arguments: Vec::new(),
            },
        });
        let entry = self.sequence(&endpoints, input_types, finish, site, active)?;
        let Some(establishment) = establishment else {
            return Ok(entry);
        };
        Ok(self.push(LoweredScalarBranchState {
            structural_parameters: Vec::new(),
            parameter_types: input_types.to_vec(),
            erased_formal_types: Vec::new(),
            erased_proof_formals: Vec::new(),
            bindings: Vec::new(),
            structural_effects: vec![LoweredScalarEffect::EstablishElementView(establishment)],
            terminator: LoweredScalarBranchTerminator::Jump {
                cleanup_actions: Vec::new(),
                target: entry,
                arguments: super::parameters(input_types),
                erased_arguments: Vec::new(),
                erased_proof_arguments: Vec::new(),
                structural_arguments: Vec::new(),
            },
        }))
    }
}

pub(crate) fn declarations(
    operations: &[terminal_psi::Operation],
) -> impl Iterator<Item = terminal_psi::StructuralPlaceDeclaration> + '_ {
    operations.iter().filter_map(|operation| {
        if !matches!(
            operation.kind,
            terminal_psi::OperationKind::ByteSequenceSubslice { .. }
                | terminal_psi::OperationKind::ElementViewSubslice { .. }
                | terminal_psi::OperationKind::EstablishElementView { .. }
                | terminal_psi::OperationKind::EstablishMutableElementView { .. }
        ) {
            return None;
        }
        let terminal_psi::OperationResult::Structural(result) = &operation.result else {
            return None;
        };
        Some(terminal_psi::StructuralPlaceDeclaration {
            id: result.place,
            kind: semantic_vocabulary::StructuralPlaceKind::OperationResult {
                producer: operation.id,
                structural_type: result.structural_type,
            },
        })
    })
}
