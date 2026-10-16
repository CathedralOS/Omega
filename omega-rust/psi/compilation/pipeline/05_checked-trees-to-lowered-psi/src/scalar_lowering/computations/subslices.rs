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
    pub(crate) expression: ExpressionHandle,
    pub(crate) place: PlaceId,
    pub(crate) structural_type: StructuralTypeId,
    bytes: bool,
    storage: view_ranges::RangeStorage,
    whole_view: Option<PlaceId>,
}

pub(crate) use crate::unit::attached_unit::view_ranges::range;

pub(crate) fn prepare(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    checked: &CheckedTrees,
    machines: &[symbols::SymbolHandle],
    types: &[StructuralTypeDeclaration],
    next_place: &mut u64,
) -> Result<Vec<Slot>, LoweringError> {
    let plans = &checked.facts.values.scalar_computations;
    let mut slots: Vec<Slot> = Vec::new();
    for root in plans.machine_roots(machines) {
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
                let declaration = crate::terminal_identities::structural_declaration_by_identity(
                    types,
                    identities,
                    checked,
                    argument.type_identity,
                    "computed range has no structural type namespace",
                )?;
                if let Some(existing) = slots.iter().find(|slot| slot.expression == expression) {
                    if existing.structural_type != declaration.id || existing.bytes != bytes {
                        return unsupported("computed range occurrence has conflicting types");
                    }
                    continue;
                }
                slots.push(reserve(
                    identities,
                    checked,
                    root.state,
                    root.statement_ordinal,
                    argument,
                    declaration.id,
                    types,
                    next_place,
                )?);
            }
        }
    }
    Ok(slots)
}

pub(crate) fn reserve(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    checked: &CheckedTrees,
    state: symbols::SymbolHandle,
    statement: u32,
    argument: &CheckedUnitStructuralArgumentPlan,
    structural_type: StructuralTypeId,
    types: &[StructuralTypeDeclaration],
    next_place: &mut u64,
) -> Result<Slot, LoweringError> {
    let range =
        range(argument).ok_or(LoweringError::Unsupported("computed range lost its source"))?;
    let bytes =
        range.family == crate::emission::operation_emission::view_subslice::ViewFamily::Bytes;
    // The consumer supplies the exact result carrier: a stored field can retain
    // its reference shell where a call presents the underlying view. Both use
    // this range evaluator; backing geometry cannot replace the receiving type.
    let storage = view_ranges::storage(identities, checked, state, statement, argument, types)?;
    let whole_view = storage
        .requires_establishment()
        .then(|| super::super::allocate_dense(next_place).map(super::super::place_id))
        .transpose()?;
    Ok(Slot {
        expression: range.expression,
        place: super::super::place_id(super::super::allocate_dense(next_place)?),
        structural_type,
        bytes,
        storage,
        whole_view,
    })
}

impl super::Expansion<'_> {
    /// Structural and scalar callers use the same range evaluator, including
    /// selector capture, endpoint sequencing and descriptor narrowing.
    pub(crate) fn range_argument(
        &mut self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        state: symbols::SymbolHandle,
        statement: u32,
        argument: &CheckedUnitStructuralArgumentPlan,
        slot: &Slot,
        bindings: &crate::expression_preparation::bindings::ScalarBindings,
        input_types: &[super::QualifiedScalarType],
        target: usize,
    ) -> Result<usize, LoweringError> {
        self.subslice(
            identities,
            argument,
            slot,
            input_types,
            target,
            &super::Site {
                state,
                statement,
                bindings,
            },
            &mut Vec::new(),
        )
    }

    /// Capture each endpoint once, in start/end order, and retire those scalar
    /// temporaries only after the ordinary descriptor operation consumes them.
    pub(super) fn subslice(
        &mut self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        argument: &CheckedUnitStructuralArgumentPlan,
        slot: &Slot,
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
        let (mut source, establishment) = slot.storage.bind(
            identities,
            self.checked,
            site.state,
            site.bindings,
            slot.whole_view,
            self.structural_types,
        )?;
        let (start, end) = (range.start.clone(), range.end.clone());
        let scalar_type =
            super::super::terminal_scalar_type(checked_trees::types::PrimitiveType::U64)?;
        let mut endpoints = Vec::with_capacity(2);
        let mut endpoint_types = input_types.to_vec();
        // Preserve the selected collection before start/end evaluation. Every
        // dynamic selector becomes an ordinary completed scalar operand in the
        // same prefix telescope used for endpoint values.
        for segment in &mut source.path {
            if let crate::emission::operation_emission::expressions::LoweredElementPathSegment::RuntimeIndex {
                selector, extent, ..
            } = segment {
                for value in std::iter::once(selector).chain(extent.iter_mut()) {
                    let operand = super::Argument::Value(value.clone());
                    let value_type = self.argument_type(identities, &operand, site, input_types)?;
                    *value = super::parameter(endpoint_types.len(), value_type);
                    endpoints.push(operand);
                    endpoint_types.push(value_type);
                }
            }
        }
        let mut completed = [None, None];
        for (index, endpoint) in [start, end].into_iter().enumerate() {
            let Some(endpoint) = endpoint else {
                continue;
            };
            let argument = match endpoint {
                checked_trees::CheckedCallScalarArgument::Pure(value) => {
                    super::Argument::Value(site.bindings.expression(
                        identities,
                        &self.checked.facts.proven_scalar_indexes,
                        &value,
                    )?)
                }
                checked_trees::CheckedCallScalarArgument::Computation(handle) => {
                    super::Argument::Computation(handle)
                }
            };
            let value_type = self.argument_type(identities, &argument, site, input_types)?;
            if !value_type
                .scalar_type
                .as_integer()
                .is_some_and(|integer| !integer.is_address())
            {
                return unsupported("computed range endpoint lost its integer type");
            }
            endpoints.push(argument);
            completed[index] = Some(
                crate::emission::operation_emission::view_subslice::coordinate(super::parameter(
                    endpoint_types.len(),
                    value_type,
                ))?,
            );
            endpoint_types.push(value_type);
        }
        let [start, end] = completed;
        let end = view_ranges::exclusive_end(range.end_inclusive, end)?;
        let start = start.unwrap_or(
            crate::emission::operation_emission::expressions::LoweredDirectExpression::IntegerLiteral {
                value: semantic_vocabulary::IntegerValue::Unsigned(0),
                scalar_type,
            },
        );
        use crate::scalar_lowering::branch_states::prepared::{
            LoweredScalarBranchState, LoweredScalarBranchTerminator, LoweredScalarEffect,
        };
        let effect = LoweredScalarEffect::ViewSubslice {
            source,
            start,
            end,
            place: slot.place,
            structural_type: slot.structural_type,
        };
        let finish = self.push(LoweredScalarBranchState {
            structural_parameters: Vec::new(),
            parameter_types: endpoint_types,
            erased_formal_types: Vec::new(),
            erased_proof_formals: Vec::new(),
            bindings: Vec::new(),
            structural_effects: vec![effect],
            terminator: LoweredScalarBranchTerminator::jump(target, super::parameters(input_types)),
        });
        let entry = self.sequence(identities, &endpoints, input_types, finish, site, active)?;
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
            terminator: LoweredScalarBranchTerminator::jump(entry, super::parameters(input_types)),
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

/// Merge each block's computed subslice declarations into
/// `structural_places`: an identical declaration is reused; the same
/// identity with different provenance is an error.
pub(crate) fn merge_declarations<'b>(
    blocks: impl Iterator<Item = &'b terminal_psi::Block>,
    structural_places: &mut Vec<terminal_psi::StructuralPlaceDeclaration>,
) -> Result<(), LoweringError> {
    for declaration in blocks.flat_map(|block| declarations(&block.operations)) {
        match structural_places
            .iter()
            .find(|existing| existing.id == declaration.id)
        {
            Some(existing) if *existing != declaration => {
                return unsupported(
                    "computed subslice conflicts with an existing place declaration",
                );
            }
            Some(_) => {}
            None => structural_places.push(declaration),
        }
    }
    Ok(())
}
