use super::super::super::{
    BlockId, StructuralAccess, StructuralArgument, StructuralParameterDeclaration,
    StructuralTypeDeclaration, SuccessorEdge, ValueId, block_id,
};
use super::super::{
    Block, CheckedScalarExpressionRole, CheckedUnitEffectOperationPlan, LoweringError,
    Multiplicity, Operation, OperationKind, OperationResult, PlaceId, ScalarType,
    StructuralMultiplicity, StructuralPlaceDeclaration, StructuralPlaceKind, StructuralTypeId,
    StructuralTypeShape, Terminator, ValueDeclaration, allocate_dense, argument_evaluation,
    edge_id, lookup_type_id, lower_structural_path, place_id, unsupported, value_id,
};
use super::CheckedTrees;
use crate::emission::operation_emission::buffer::OperationBuffer;
use crate::emission::operation_emission::calls::CallEmissionContext;
use crate::emission::operation_emission::integer::LoweredIntegerComparisonKind;
use crate::expression_preparation::source_custody::structural as source_custody;
use checked_trees::{CheckedStructuralValueHandle, CheckedStructuralValueKind};

pub(super) type StructuralCallEmitter<'a> = dyn FnMut(
        &CheckedUnitEffectOperationPlan,
        Option<&[ValueDeclaration]>,
        &mut CallEmissionContext<'_>,
        &mut OperationBuffer,
        &mut u64,
    ) -> Result<StructuralPlaceDeclaration, LoweringError>
    + 'a;

#[allow(clippy::too_many_arguments)]
pub(crate) fn emit(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    state: symbols::SymbolHandle,
    operation: &CheckedUnitEffectOperationPlan,
    structural_types: &[StructuralTypeDeclaration],
    type_ids: &[(String, StructuralTypeId)],
    next_place: &mut u64,
    temporary_places: &mut Vec<StructuralPlaceDeclaration>,
    call_emitter: &mut StructuralCallEmitter<'_>,
    calls: &mut CallEmissionContext<'_>,
    evaluation: &mut argument_evaluation::Evaluation,
    values: &mut Vec<ValueDeclaration>,
    next_value: &mut u64,
    next_block: &mut u64,
    next_edge: &mut u64,
    operations: &mut OperationBuffer,
) -> Result<StructuralPlaceDeclaration, LoweringError> {
    source_custody::validate(checked, machine, state, operation)?;
    let CheckedUnitEffectOperationPlan::EstablishStructuralValue {
        result,
        value,
        calls: operand_calls,
        ..
    } = operation
    else {
        return unsupported("structural value producer missing");
    };
    let structural_type = lookup_type_id(type_ids, &result.type_identity)?;
    let multiplicity = match result.multiplicity {
        Multiplicity::Affine => StructuralMultiplicity::Affine,
        Multiplicity::Unrestricted => StructuralMultiplicity::Unrestricted,
        Multiplicity::Linear => {
            return unsupported("fresh structural value cannot create linear custody");
        }
    };
    let authored_state =
        crate::expression_preparation::source_custody::authored_state(checked, state)?.1;
    let sources = checked
        .facts
        .flow
        .ownership
        .owned_selection_at(state, result.statement_index)
        .map(|(_, receipt)| {
            checked
                .facts
                .flow
                .ownership
                .selection_sources
                .span(receipt.sources)
                .ok_or(LoweringError::Unsupported(
                    "owned selection source span is stale",
                ))?
                .iter()
                .rev()
                .map(|source| {
                    if let Some(argument) = evaluation
                        .structural_locals
                        .iter()
                        .find(|(symbol, _)| *symbol == source.symbol)
                        .map(|(_, argument)| argument.clone())
                    {
                        return Ok(argument);
                    }
                    // Parameter sources keep their physical ingress place: the
                    // receipt row names the authored position and the
                    // signature declaration supplies its whole-owned custody.
                    let parameter = checked
                        .state_parameters(authored_state)
                        .iter()
                        .enumerate()
                        .find(|(_, parameter)| parameter.symbol == source.symbol)
                        .ok_or(LoweringError::Unsupported(
                            "owned selection source has no established physical local",
                        ))?;
                    if parameter.0 as u32 != source.statement_ordinal
                        || parameter.1.is_self
                        || parameter.1.is_const
                        || parameter.1.is_mutable
                    {
                        return Err(LoweringError::Unsupported(
                            "owned selection parameter source drifted from its authored origin",
                        ));
                    }
                    let declaration = evaluation
                        .structural_parameters
                        .iter()
                        .find(|(position, _)| *position == parameter.0 as u32)
                        .map(|(_, declaration)| declaration)
                        .ok_or(LoweringError::Unsupported(
                            "owned selection parameter source has no signature place",
                        ))?;
                    if declaration.access != StructuralAccess::Owned
                        || declaration.multiplicity != StructuralMultiplicity::Affine
                        || !declaration.qualifications.is_empty()
                        || !declaration.projected_qualifications.is_empty()
                    {
                        return Err(LoweringError::Unsupported(
                            "owned selection parameter source lacks whole plain-affine custody",
                        ));
                    }
                    Ok(StructuralArgument {
                        place: declaration.place,
                        path: Vec::new(),
                        access: StructuralAccess::Owned,
                    })
                })
                .collect::<Result<Vec<_>, _>>()
        })
        .transpose()?
        .unwrap_or_default();
    if !sources.is_empty() && multiplicity != StructuralMultiplicity::Affine {
        return unsupported("owned selection requires uniform affine root cleanup");
    }
    // A `&T` result rejoins borrowed custody at the selection's block
    // parameter instead of transferring a referent owner. The authored local's
    // declared type pins this: `&T` over a plain-owned record produces a
    // shared-borrow join parameter; every other result stays owned.
    let result_access = match checked
        .statement_table
        .statements(authored_state.statement_nodes)
        .get(result.statement_index as usize)
    {
        Some(checked_trees::statement::StatementNode::LocalData(local))
            if source_custody::shared_borrow_record_referent(checked, local.type_reference)
                .is_some() =>
        {
            StructuralAccess::SharedBorrow
        }
        _ => StructuralAccess::Owned,
    };
    if result_access == StructuralAccess::SharedBorrow && !sources.is_empty() {
        return unsupported("borrowed selection cannot carry owned residual custody");
    }
    let owners = if sources.is_empty() {
        Vec::new()
    } else {
        let owners = prepare_owners(
            checked,
            machine,
            state,
            result.statement_index,
            structural_types,
            evaluation,
            operations,
        )?;
        if owners
            .iter()
            .filter(|owner| {
                sources
                    .iter()
                    .any(|source| source.place == owner.value.place)
            })
            .count()
            != sources.len()
        {
            return unsupported("owned selection source is absent from the live emitted frontier");
        }
        owners
    };
    let mut emission = Emission {
        checked,
        machine,
        state,
        statement: result.statement_index,
        structural_type,
        multiplicity,
        structural_types,
        type_ids,
        next_place,
        temporary_places,
        calls,
        operand_calls,
        call_emitter,
        evaluation,
        values,
        next_value,
        next_block,
        next_edge,
        operations,
        sources,
        owners,
        result_access,
    };
    let place = if emission.sources.is_empty()
        && let CheckedStructuralValueKind::Reference { source } = &checked
            .facts
            .values
            .structural_values
            .nodes
            .get(*value)
            .kind
        && source.access == checked_trees::CheckedStructuralAccess::SharedBorrow
    {
        emission.direct_borrow(source)?
    } else {
        let place = emission.value(*value, None)?;
        if emission.sources.is_empty()
            && matches!(
                checked
                    .facts
                    .values
                    .structural_values
                    .nodes
                    .get(*value)
                    .kind,
                CheckedStructuralValueKind::Place(_)
            )
        {
            emission.materialize_place(place)?
        } else {
            place
        }
    };
    // Private arm producers are not authored result ordinals. Publish exactly
    // one completed place for this binding, whether a direct producer or join.
    let declaration = emission
        .temporary_places
        .iter()
        .position(|item| item.id == place)
        .ok_or(LoweringError::Unsupported(
            "structural value place was not declared",
        ))?;
    let declaration = emission.temporary_places.remove(declaration);
    emission.evaluation.establish_structural_result(
        checked,
        state,
        result,
        terminal_psi::StructuralOperationResult {
            place,
            structural_type,
            multiplicity,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
            claims: Vec::new(),
        },
        structural_types,
        emission.operations,
    )?;
    Ok(declaration)
}

fn prepare_owners(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    state: symbols::SymbolHandle,
    statement: u32,
    structural_types: &[StructuralTypeDeclaration],
    evaluation: &argument_evaluation::Evaluation,
    operations: &OperationBuffer,
) -> Result<Vec<argument_evaluation::StructuralValueOwner>, LoweringError> {
    let available = |symbol: symbols::SymbolHandle| -> Result<bool, LoweringError> {
        if !symbol.is_valid() {
            return Ok(true);
        }
        for (_, receipt) in checked.facts.flow.ownership.owned_selections.iter() {
            if receipt.machine == machine
                && receipt.state == state
                && receipt.statement_ordinal < statement
                && checked
                    .facts
                    .flow
                    .ownership
                    .selection_sources
                    .span_or_empty(receipt.sources)
                    .iter()
                    .any(|source| source.symbol == symbol)
            {
                return Ok(false);
            }
        }
        for (_, event) in checked.facts.flow.ownership.permissions.iter() {
            if event.machine_symbol != machine
                || event.state_symbol != state
                || event.root != facts::PlaceRoot::Symbol(symbol)
                || event.access != language_semantics::PermissionAccess::Owned
            {
                continue;
            }
            let ordinal = match event.source {
                language_semantics::PermissionEventSource::Statement { statement_index }
                | language_semantics::PermissionEventSource::Call {
                    statement_index, ..
                } => statement_index,
                _ => continue,
            };
            if ordinal >= statement as usize
                || event.kind == language_semantics::PermissionEventKind::Establish
            {
                continue;
            }
            if !event.segments.is_empty() {
                return unsupported(
                    "selection pass-through of partially moved roots requires retained residual frontier correspondence",
                );
            }
            return Ok(false);
        }
        Ok(true)
    };
    let mut owners = Vec::new();
    for owner in &evaluation.structural_value_owners {
        if available(owner.symbol)? {
            owners.push(owner.clone());
        }
    }
    let statements = checked.statement_table.statements(
        crate::expression_preparation::source_custody::authored_state(checked, state)?
            .1
            .statement_nodes,
    );
    for (symbol, argument) in &evaluation.structural_locals {
        if owners.iter().any(|owner| owner.symbol == *symbol) || !available(*symbol)? {
            continue;
        }
        let (ordinal, local) = statements
            .iter()
            .enumerate()
            .find_map(|(ordinal, source)| match source {
                checked_trees::statement::StatementNode::LocalData(local)
                    if local.symbol == *symbol =>
                {
                    Some((ordinal, local))
                }
                _ => None,
            })
            .ok_or(LoweringError::Unsupported(
                "selection survivor has no authored local",
            ))?;
        if checked.type_multiplicity(local.type_reference) != Multiplicity::Affine {
            continue;
        }
        if ordinal >= statement as usize
            || !argument.path.is_empty()
            || argument.access != StructuralAccess::Owned
            || !validation::has_plain_owned_contents_with_numeric_constraints(
                &checked.typed,
                local.type_reference,
            )
        {
            return unsupported("selection survivor requires whole plain-affine custody");
        }
        let value = operations
            .iter()
            .filter_map(|operation| operation.result.structural())
            .find(|result| result.place == argument.place)
            .ok_or(LoweringError::Unsupported(
                "selection survivor requires an exact emitted operation-result frontier",
            ))?;
        if value.multiplicity != StructuralMultiplicity::Affine
            || !value.claims.is_empty()
            || !value.qualifications.is_empty()
            || !value.projected_qualifications.is_empty()
            || !structural_types
                .iter()
                .any(|declaration| declaration.id == value.structural_type)
        {
            return unsupported("selection survivor requires claim-free plain-affine transport");
        }
        owners.push(argument_evaluation::StructuralValueOwner {
            symbol: *symbol,
            statement: u32::try_from(ordinal).map_err(|_| {
                LoweringError::Unsupported("selection survivor statement exceeds u32")
            })?,
            value: value.clone(),
        });
    }
    // Selected parameter sources join the frontier as owners: their ingress
    // places are real signature places and their residual slots die on the
    // receipt's actual death edge like any local source.
    let source_parameters = checked.state_parameters(
        crate::expression_preparation::source_custody::authored_state(checked, state)?.1,
    );
    if let Some((_, receipt)) = checked
        .facts
        .flow
        .ownership
        .owned_selection_at(state, statement)
    {
        for (position, parameter) in source_parameters.iter().enumerate() {
            if !checked
                .facts
                .flow
                .ownership
                .selection_sources
                .span_or_empty(receipt.sources)
                .iter()
                .any(|source| {
                    source.symbol == parameter.symbol && source.statement_ordinal == position as u32
                })
            {
                continue;
            }
            if parameter.is_self || parameter.is_const || parameter.is_mutable {
                return unsupported("selection parameter source is not immutable owned ingress");
            }
            let declaration = evaluation
                .structural_parameters
                .iter()
                .find(|(source_position, _)| *source_position as usize == position)
                .map(|(_, declaration)| declaration)
                .ok_or(LoweringError::Unsupported(
                    "selection parameter source has no signature place",
                ))?;
            if declaration.access != StructuralAccess::Owned
                || declaration.multiplicity != StructuralMultiplicity::Affine
                || !declaration.qualifications.is_empty()
                || !declaration.projected_qualifications.is_empty()
                || checked.type_multiplicity(parameter.type_reference) != Multiplicity::Affine
                || !validation::has_plain_owned_contents_with_numeric_constraints(
                    &checked.typed,
                    parameter.type_reference,
                )
            {
                return unsupported(
                    "selection parameter source requires whole plain-affine custody",
                );
            }
            if owners
                .iter()
                .any(|owner| owner.value.place == declaration.place)
            {
                return unsupported("selection parameter source duplicated its frontier entry");
            }
            owners.push(argument_evaluation::StructuralValueOwner {
                symbol: parameter.symbol,
                statement: u32::try_from(position).map_err(|_| {
                    LoweringError::Unsupported("selection parameter position exceeds u32")
                })?,
                value: terminal_psi::StructuralOperationResult {
                    place: declaration.place,
                    structural_type: declaration.structural_type,
                    multiplicity: declaration.multiplicity,
                    qualifications: declaration.qualifications.clone(),
                    projected_qualifications: declaration.projected_qualifications.clone(),
                    claims: Vec::new(),
                },
            });
        }
    }
    // Deterministic establishment order: parameters were established at state
    // entry, so their slots precede locals kept in ascending statement order.
    // The join binds these slots positionally against the receipt's source
    // complement, and the return splice's reverse-establishment roster then
    // matches the verifier's reverse parameter disposal exactly.
    owners.sort_by_key(|owner| {
        let is_parameter = source_parameters
            .iter()
            .any(|parameter| parameter.symbol == owner.symbol);
        (!is_parameter, owner.statement)
    });
    for source in statements.iter().take(statement as usize) {
        if let checked_trees::statement::StatementNode::LocalData(local) = source
            && checked.type_multiplicity(local.type_reference) == Multiplicity::Affine
            && available(local.symbol)?
            && !owners.iter().any(|owner| owner.symbol == local.symbol)
        {
            return unsupported(
                "selection transport requires a typed frontier entry for every live owned local",
            );
        }
    }
    Ok(owners)
}

pub(super) struct Emission<'a, 'b, 'calls> {
    pub(super) checked: &'a CheckedTrees,
    pub(super) machine: symbols::SymbolHandle,
    pub(super) state: symbols::SymbolHandle,
    pub(super) statement: u32,
    pub(super) structural_type: StructuralTypeId,
    pub(super) multiplicity: StructuralMultiplicity,
    pub(super) structural_types: &'a [StructuralTypeDeclaration],
    pub(super) type_ids: &'a [(String, StructuralTypeId)],
    pub(super) next_place: &'b mut u64,
    pub(super) temporary_places: &'b mut Vec<StructuralPlaceDeclaration>,
    pub(super) calls: &'b mut CallEmissionContext<'calls>,
    pub(super) operand_calls: &'a [checked_trees::CheckedStructuralValueCall],
    pub(super) call_emitter: &'b mut StructuralCallEmitter<'a>,
    pub(super) evaluation: &'b mut argument_evaluation::Evaluation,
    pub(super) values: &'b mut Vec<ValueDeclaration>,
    pub(super) next_value: &'b mut u64,
    pub(super) next_block: &'b mut u64,
    pub(super) next_edge: &'b mut u64,
    pub(super) operations: &'b mut OperationBuffer,
    sources: Vec<StructuralArgument>,
    owners: Vec<argument_evaluation::StructuralValueOwner>,
    /// The join result slot's custody: `Owned` for an owned selection,
    /// `SharedBorrow` for a `&T` result whose branches rejoin shared borrows.
    result_access: StructuralAccess,
}

pub(super) struct ValueContinuation {
    block: BlockId,
    parameters: Vec<ValueDeclaration>,
    structural_parameters: Vec<StructuralParameterDeclaration>,
    place: PlaceId,
    remaining_owners: Vec<argument_evaluation::StructuralValueOwner>,
    pass_through: Vec<(PlaceId, PlaceId)>,
    residuals: Vec<PlaceId>,
}

/// One projected affine child moved on a selected edge. The root place keeps
/// its once-evaluated owner, `path` is the exact checked field/fixed-index
/// projection, and the two identities let the edge argument carry the moved
/// leaf type while residual reconstruction replays the root's complement.
pub(super) struct ProjectedMove {
    pub(super) root: PlaceId,
    pub(super) path: Vec<checked_trees::CheckedUnitStructuralPathSegment>,
    pub(super) leaf_type_identity: String,
    pub(super) root_type_identity: String,
    pub(super) root_source: checked_trees::CheckedUnitStructuralArgumentSourcePlan,
}

impl Emission<'_, '_, '_> {
    pub(super) fn record_field_value(
        &mut self,
        value: CheckedStructuralValueHandle,
    ) -> Result<PlaceId, LoweringError> {
        // A fresh field's continuation joins that field, not the enclosing
        // selection's result/residual slots. Its constructors leave incoming
        // owners live on every branch; only the outer completion may displace
        // a candidate. Source replay still refuses hidden owned child transfers.
        let sources = std::mem::take(&mut self.sources);
        let owners = std::mem::take(&mut self.owners);
        let result = self.value(value, None);
        self.sources = sources;
        self.owners = owners;
        result
    }

    pub(super) fn value(
        &mut self,
        value: CheckedStructuralValueHandle,
        continuation: Option<&ValueContinuation>,
    ) -> Result<PlaceId, LoweringError> {
        let node = self
            .checked
            .facts
            .values
            .structural_values
            .nodes
            .get(value)
            .clone();
        match node.kind {
            CheckedStructuralValueKind::Reference { source } => {
                if source.access == checked_trees::CheckedStructuralAccess::SharedBorrow {
                    // A `&T` branch borrows its exact referent place: resolve
                    // the authored root/path against the current locals and
                    // signature parameters, then join that shared custody at
                    // the selection's block parameter. No owner moves and no
                    // residual custody dies on this edge.
                    let continuation = continuation.ok_or(LoweringError::Unsupported(
                        "borrowed selection requires a structural continuation",
                    ))?;
                    if lookup_type_id(self.type_ids, &source.type_identity)? != self.structural_type
                    {
                        return unsupported("borrowed selection changed its referent type");
                    }
                    let argument = crate::expression_preparation::bindings::ScalarBindings::new(
                        self.values.len(),
                    )
                    .with_structural_parameters(&self.evaluation.structural_parameters)
                    .with_structural_locals(&self.evaluation.structural_locals)
                    .shared_structural_argument(&source)?;
                    self.complete_borrowed(argument, continuation)?;
                    return Ok(continuation.place);
                }
                let checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter {
                    parameter_index,
                } = source.source
                else {
                    return unsupported("reference initializer has no formal ingress");
                };
                let (_, parameter) = self
                    .evaluation
                    .structural_parameters
                    .get(parameter_index as usize)
                    .ok_or(LoweringError::Unsupported(
                        "reference initializer parameter missing",
                    ))?;
                if !source.path.is_empty()
                    || source.access != checked_trees::CheckedStructuralAccess::MutableBorrow
                    || parameter.access != StructuralAccess::MutableBorrow
                    || parameter.structural_type
                        != lookup_type_id(self.type_ids, &source.type_identity)?
                    || !parameter.qualifications.is_empty()
                    || !parameter.projected_qualifications.is_empty()
                {
                    return unsupported("reference initializer changes ingress custody");
                }
                let declaration = super::super::reference_results::emit_carrier(
                    self.structural_type,
                    StructuralArgument {
                        place: parameter.place,
                        path: Vec::new(),
                        access: StructuralAccess::MutableBorrow,
                    },
                    self.next_place,
                    self.operations,
                )?;
                let place = declaration.id;
                self.temporary_places.push(declaration);
                if let Some(continuation) = continuation {
                    self.complete_value(place, None, continuation)?;
                }
                Ok(place)
            }
            CheckedStructuralValueKind::Record { .. } => {
                let place = self.record(value)?;
                if let Some(continuation) = continuation {
                    self.complete_value(place, None, continuation)?;
                }
                Ok(place)
            }
            CheckedStructuralValueKind::Call { .. } => {
                let mut matching = self.operand_calls.iter().filter(|call| call.value == value);
                let call = matching.next().ok_or(LoweringError::Unsupported(
                    "structural operand call missing",
                ))?;
                if matching.next().is_some() {
                    return unsupported("structural operand call is duplicated");
                }
                let evaluated = self.evaluation.arguments(
                    self.checked,
                    self.machine,
                    self.state,
                    call.operation(),
                    self.values,
                    self.next_value,
                    self.next_block,
                    self.next_edge,
                    self.operations,
                    self.calls,
                )?;
                let declaration = (self.call_emitter)(
                    call.operation(),
                    evaluated.as_deref(),
                    self.calls,
                    self.operations,
                    self.next_place,
                )?;
                let StructuralPlaceKind::OperationResult {
                    structural_type, ..
                } = declaration.kind
                else {
                    return unsupported("structural operand call has no result place");
                };
                if structural_type != self.structural_type {
                    return unsupported("structural operand call changed its returned type");
                }
                if let Some(continuation) = continuation {
                    self.complete_value(declaration.id, None, continuation)?;
                }
                Ok(declaration.id)
            }
            CheckedStructuralValueKind::Projection {
                source,
                path,
                type_identity,
            } => {
                // The root owner evaluates once: a local keeps its established
                // place, a structural product emits its producing call. The
                // selected edge then moves only the projected child while the
                // root's residual complement dies on that same edge.
                let continuation = continuation.ok_or(LoweringError::Unsupported(
                    "projected selection requires a structural continuation",
                ))?;
                if path.is_empty()
                    || lookup_type_id(self.type_ids, &type_identity)? != self.structural_type
                {
                    return unsupported("projected selection changed its moved child type");
                }
                let projected = self.projected_root(source, path, type_identity)?;
                self.complete_value(projected.root, Some(&projected), continuation)?;
                Ok(continuation.place)
            }
            CheckedStructuralValueKind::Place(argument) => {
                if self.sources.is_empty()
                    && continuation.is_none()
                    && self.structural_types.iter().any(|declaration| {
                        declaration.id == self.structural_type
                            && matches!(declaration.shape, StructuralTypeShape::Record { .. })
                    })
                {
                    // Source replay distinguishes a whole record child from a
                    // selected sum leaf, whose receipt and continuation remain mandatory.
                    if let Some(place) = self.parameter_source(&argument)? {
                        return Ok(place);
                    }
                    return crate::expression_preparation::bindings::ScalarBindings::new(
                        self.values.len(),
                    )
                    .with_structural_parameters(&self.evaluation.structural_parameters)
                    .with_structural_locals(&self.evaluation.structural_locals)
                    .owned_argument(&argument)
                    .map(|binding| binding.place);
                }
                if !argument.path.is_empty()
                    || argument.access != checked_trees::CheckedStructuralAccess::Owned
                {
                    return unsupported("owned selection requires whole owned sources");
                }
                let selected = match &argument.source {
                    checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralLocal {
                        symbol,
                    } => self
                        .evaluation
                        .structural_locals
                        .iter()
                        .find(|(source, _)| *source == *symbol)
                        .map(|(_, argument)| argument.place)
                        .or(self.parameter_source(&argument)?)
                        .ok_or(LoweringError::Unsupported(
                            "owned selection local place missing",
                        ))?,
                    checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter {
                        ..
                    } => self
                        .parameter_source(&argument)?
                        .ok_or(LoweringError::Unsupported(
                            "owned selection parameter place missing",
                        ))?,
                    _ => {
                        return unsupported(
                            "owned selection requires an established structural local or parameter",
                        );
                    }
                };
                if !self.sources.iter().any(|source| source.place == selected) {
                    return unsupported("owned selection place is absent from its receipt sources");
                }
                let continuation = continuation.ok_or(LoweringError::Unsupported(
                    "direct owned place requires a structural continuation",
                ))?;
                self.complete_value(selected, None, continuation)?;
                Ok(continuation.place)
            }
            CheckedStructuralValueKind::Case(construction) => {
                let source = validation::scalar_case_constructor(
                    &self.checked.typed,
                    construction.expression,
                )
                .ok_or(LoweringError::Unsupported(
                    "structural case lost its authored constructor",
                ))?;
                let place = place_id(allocate_dense(self.next_place)?);
                let slot = crate::scalar_graph::scalar_computations::cases::reserve(
                    self.checked,
                    construction.expression,
                    &source,
                    self.structural_types,
                    place,
                )?;
                let fields = crate::expression_preparation::computation_graph::fields(
                    self.checked,
                    &construction,
                )?
                .to_vec();
                let field_start = self.values.len();
                for (ordinal, field) in fields.iter().enumerate() {
                    let field_ordinal = u32::try_from(ordinal).map_err(|_| {
                        LoweringError::Unsupported("structural field ordinal overflow")
                    })?;
                    let value = self.scalar(
                        CheckedScalarExpressionRole::StructuralValueField {
                            expression: construction.expression,
                            field_ordinal,
                        },
                        field.value,
                        field_start,
                    )?;
                    self.values.push(value);
                }
                let completed = slot.construction(&self.values[field_start..])?;
                crate::scalar_graph::scalar_computations::cases::emit(
                    &completed,
                    &self.values[field_start..],
                    self.next_value,
                    self.operations,
                )?;
                let declaration = crate::scalar_graph::scalar_computations::cases::declarations(
                    &self.operations.operations,
                )
                .find(|declaration| declaration.id == place)
                .ok_or(LoweringError::Unsupported(
                    "structural case result declaration missing",
                ))?;
                self.temporary_places.push(declaration);
                self.values.truncate(field_start);
                if let Some(continuation) = continuation {
                    self.complete_value(place, None, continuation)?;
                }
                Ok(place)
            }
            CheckedStructuralValueKind::Dispatch { subject, arms } => {
                let arms = self
                    .checked
                    .facts
                    .values
                    .structural_values
                    .dispatch_arms
                    .span(arms)
                    .ok_or(LoweringError::Unsupported(
                        "structural dispatch arm span is stale",
                    ))?
                    .to_vec();
                let source_count = self.values.len();
                let subject = self.scalar(
                    CheckedScalarExpressionRole::StructuralValueSubject {
                        expression: node.expression,
                    },
                    subject,
                    source_count,
                )?;
                self.values.push(subject);
                let owned_continuation;
                let is_root = continuation.is_none();
                let continuation = if let Some(continuation) = continuation {
                    continuation
                } else {
                    let join = block_id(allocate_dense(self.next_block)?);
                    let joined_values = self.values[..source_count]
                        .iter()
                        .map(|value| {
                            Ok(ValueDeclaration {
                                id: value_id(allocate_dense(self.next_value)?),
                                ..*value
                            })
                        })
                        .collect::<Result<Vec<_>, LoweringError>>()?;
                    let mut structural_parameters = Vec::new();
                    let first_candidate = self.first_candidate();
                    let mut remaining_owners = self
                        .owners
                        .iter()
                        .enumerate()
                        .filter(|(position, _)| Some(*position) != first_candidate)
                        .map(|(_, owner)| owner.clone())
                        .collect::<Vec<_>>();
                    let mut pass_through = Vec::new();
                    let mut residuals = Vec::new();
                    for position in 0..remaining_owners.len() + 1 {
                        let (structural_type, multiplicity) = remaining_owners
                            .get(position)
                            .map_or((self.structural_type, self.multiplicity), |owner| {
                                (owner.value.structural_type, owner.value.multiplicity)
                            });
                        let is_result_slot = position == remaining_owners.len();
                        let owner = remaining_owners.get_mut(position);
                        let position = u32::try_from(position).map_err(|_| {
                            LoweringError::Unsupported(
                                "owned selection parameter count exceeds u32",
                            )
                        })?;
                        let place = place_id(allocate_dense(self.next_place)?);
                        if let Some(owner) = owner {
                            if self
                                .sources
                                .iter()
                                .any(|source| source.place == owner.value.place)
                            {
                                owner.symbol = symbols::SymbolHandle::invalid();
                                residuals.push(place);
                            } else {
                                pass_through.push((owner.value.place, place));
                            }
                            owner.value.place = place;
                        }
                        self.temporary_places.push(StructuralPlaceDeclaration {
                            id: place,
                            kind: StructuralPlaceKind::BlockParameter {
                                block: join,
                                position,
                            },
                        });
                        structural_parameters.push(StructuralParameterDeclaration {
                            place,
                            position,
                            is_self: false,
                            structural_type,
                            multiplicity,
                            // Owner slots keep owned custody; the result slot
                            // carries the result's own access (`SharedBorrow`
                            // for a `&T` branch join, `Owned` otherwise).
                            access: if is_result_slot {
                                self.result_access
                            } else {
                                StructuralAccess::Owned
                            },
                            qualifications: Vec::new(),
                            projected_qualifications: Vec::new(),
                        });
                    }
                    let place = structural_parameters
                        .last()
                        .ok_or(LoweringError::Unsupported(
                            "structural continuation has no result",
                        ))?
                        .place;
                    owned_continuation = ValueContinuation {
                        block: join,
                        parameters: joined_values,
                        structural_parameters,
                        place,
                        remaining_owners,
                        pass_through,
                        residuals,
                    };
                    &owned_continuation
                };
                for (position, arm) in arms.iter().enumerate() {
                    let mut fallback = None;
                    if let checked_trees::CheckedScalarDispatchPattern::Value(pattern) = arm.pattern
                    {
                        let pattern = self.scalar(
                            CheckedScalarExpressionRole::StructuralValuePattern {
                                source_arm: arm.source_arm,
                            },
                            pattern,
                            source_count,
                        )?;
                        let subject = self.values[source_count];
                        if pattern.value_type() != subject.value_type()
                            || arm.equality_use.is_valid()
                        {
                            return unsupported(
                                "structural dispatch comparison requires matching builtin scalar operands",
                            );
                        }
                        // Replay requires coverage. A final value pattern is
                        // therefore the last literal Boolean alternative, not
                        // permission to default to an arbitrary integer arm.
                        if position + 1 < arms.len() {
                            let kind = if subject.scalar_type == ScalarType::Boolean {
                                OperationKind::BooleanEqual {
                                    left: subject.id,
                                    right: pattern.id,
                                }
                            } else if matches!(subject.scalar_type, ScalarType::Integer { .. }) {
                                LoweredIntegerComparisonKind::Equal
                                    .operation(subject.id, pattern.id)
                            } else {
                                return unsupported(
                                    "structural dispatch needs selected floating comparison lowering",
                                );
                            };
                            let condition = value_id(allocate_dense(self.next_value)?);
                            let id = self.operations.allocate();
                            self.operations.push(Operation {
                                static_reach_binding: None,
                                id,
                                result: OperationResult::Scalar(ValueDeclaration {
                                    id: condition,
                                    scalar_type: ScalarType::Boolean,
                                    qualifications: Default::default(),
                                }),
                                kind,
                            });
                            let selected = block_id(allocate_dense(self.next_block)?);
                            let next = block_id(allocate_dense(self.next_block)?);
                            let when_true = self.edge(selected, Vec::new(), Vec::new())?;
                            let when_false = self.edge(next, Vec::new(), Vec::new())?;
                            self.finish(Terminator::Conditional {
                                condition,
                                when_true,
                                when_false,
                            });
                            fallback = Some((next, self.values.clone()));
                            self.start(selected);
                        }
                    }
                    self.values.truncate(source_count);
                    self.value(arm.value, Some(continuation))?;
                    if let Some((next, values)) = fallback {
                        self.start(next);
                        *self.values = values;
                    }
                }
                if is_root {
                    self.start(continuation.block);
                    *self.values = continuation.parameters.clone();
                    self.evaluation.parameters = continuation.parameters.clone();
                    self.evaluation.block_structural_parameters =
                        continuation.structural_parameters.clone();
                    if !self.sources.is_empty() {
                        self.evaluation.selection_cleanups.push(
                            argument_evaluation::SelectionCleanup {
                                selected: continuation.place,
                                sources: self
                                    .sources
                                    .iter()
                                    .rev()
                                    .map(|source| source.place)
                                    .collect(),
                                remaining: continuation.residuals.iter().rev().copied().collect(),
                                pass_through: continuation.pass_through.clone(),
                                next_operation: self.operations.next_identity,
                            },
                        );
                        self.evaluation.structural_value_owners =
                            continuation.remaining_owners.clone();
                        self.evaluation.structural_locals.retain(|(_, argument)| {
                            !self
                                .sources
                                .iter()
                                .any(|source| source.place == argument.place)
                        });
                        for (_, argument) in &mut self.evaluation.structural_locals {
                            if let Some((_, target)) = continuation
                                .pass_through
                                .iter()
                                .find(|(source, _)| *source == argument.place)
                            {
                                argument.place = *target;
                            }
                        }
                        self.rebind_local_cases()?;
                    }
                }
                Ok(continuation.place)
            }
        }
    }

    fn first_candidate(&self) -> Option<usize> {
        self.owners.iter().position(|owner| {
            self.sources
                .iter()
                .any(|source| source.place == owner.value.place)
        })
    }

    /// Whole-value assignment uses ordinary owned edge transport. Returning the
    /// source place directly would alias unrestricted records instead of copying
    /// their backing, and would republish an earlier result's declaration.
    /// The existing block binder copies unrestricted payloads and transfers
    /// affine ownership; source replay has already checked the exact operand.
    fn materialize_place(&mut self, source: PlaceId) -> Result<PlaceId, LoweringError> {
        let block = block_id(allocate_dense(self.next_block)?);
        let place = place_id(allocate_dense(self.next_place)?);
        if self.multiplicity == StructuralMultiplicity::Affine {
            // The return frontier orders operation results before block owners.
            // Transport surviving affine locals together so a moved result and
            // its older survivors retain their declaration-ordered cleanup.
            self.owners = prepare_owners(
                self.checked,
                self.machine,
                self.state,
                self.statement,
                self.structural_types,
                self.evaluation,
                self.operations,
            )?;
        }
        let mut remaining_owners = self
            .owners
            .iter()
            .filter(|owner| owner.value.place != source)
            .cloned()
            .collect::<Vec<_>>();
        let mut pass_through = Vec::new();
        let mut structural_parameters = Vec::new();
        for owner in &mut remaining_owners {
            let target = place_id(allocate_dense(self.next_place)?);
            let position = u32::try_from(structural_parameters.len()).map_err(|_| {
                LoweringError::Unsupported("owned move parameter count exceeds u32")
            })?;
            self.temporary_places.push(StructuralPlaceDeclaration {
                id: target,
                kind: StructuralPlaceKind::BlockParameter { block, position },
            });
            structural_parameters.push(StructuralParameterDeclaration {
                place: target,
                position,
                is_self: false,
                structural_type: owner.value.structural_type,
                multiplicity: owner.value.multiplicity,
                access: StructuralAccess::Owned,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
            });
            pass_through.push((owner.value.place, target));
            owner.value.place = target;
        }
        let position = u32::try_from(structural_parameters.len())
            .map_err(|_| LoweringError::Unsupported("owned move parameter count exceeds u32"))?;
        structural_parameters.push(StructuralParameterDeclaration {
            place,
            position,
            is_self: false,
            structural_type: self.structural_type,
            multiplicity: self.multiplicity,
            access: StructuralAccess::Owned,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        });
        let parameters = self
            .values
            .iter()
            .map(|value| {
                Ok(ValueDeclaration {
                    id: value_id(allocate_dense(self.next_value)?),
                    ..*value
                })
            })
            .collect::<Result<Vec<_>, LoweringError>>()?;
        self.temporary_places.push(StructuralPlaceDeclaration {
            id: place,
            kind: StructuralPlaceKind::BlockParameter { block, position },
        });
        let continuation = ValueContinuation {
            block,
            parameters,
            structural_parameters,
            place,
            remaining_owners,
            pass_through,
            residuals: Vec::new(),
        };
        self.complete_value(source, None, &continuation)?;
        self.start(block);
        if self.multiplicity == StructuralMultiplicity::Affine {
            self.evaluation
                .structural_locals
                .retain(|(_, argument)| argument.place != source);
            for (_, argument) in &mut self.evaluation.structural_locals {
                if let Some((_, target)) = continuation
                    .pass_through
                    .iter()
                    .find(|(source, _)| *source == argument.place)
                {
                    argument.place = *target;
                }
            }
            self.evaluation.structural_value_owners = continuation.remaining_owners;
            self.evaluation
                .selection_cleanups
                .push(argument_evaluation::SelectionCleanup {
                    selected: place,
                    sources: Vec::new(),
                    remaining: Vec::new(),
                    pass_through: continuation.pass_through,
                    next_operation: self.operations.next_identity,
                });
            self.rebind_local_cases()?;
        }
        *self.values = continuation.parameters.clone();
        self.evaluation.parameters = continuation.parameters;
        self.evaluation.block_structural_parameters = continuation.structural_parameters;
        Ok(place)
    }

    /// A `let view: &T = &place` establishment carries the same borrowed
    /// custody a selection arm does, but there is no dispatch to join through:
    /// the result still binds at a fresh block parameter so the sole incoming
    /// edge can present the exact borrowed referent place. No owner moves and
    /// no residual dies on that edge -- the referent's owner stays live under
    /// the shared loan the joined parameter records, exactly as it does for
    /// every arm of a borrowed selection.
    fn direct_borrow(
        &mut self,
        source: &checked_trees::CheckedUnitStructuralArgumentPlan,
    ) -> Result<PlaceId, LoweringError> {
        if lookup_type_id(self.type_ids, &source.type_identity)? != self.structural_type {
            return unsupported("borrowed establishment changed its referent type");
        }
        let argument =
            crate::expression_preparation::bindings::ScalarBindings::new(self.values.len())
                .with_structural_parameters(&self.evaluation.structural_parameters)
                .with_structural_locals(&self.evaluation.structural_locals)
                .shared_structural_argument(source)?;
        let join = block_id(allocate_dense(self.next_block)?);
        let place = place_id(allocate_dense(self.next_place)?);
        let position = 0u32;
        self.temporary_places.push(StructuralPlaceDeclaration {
            id: place,
            kind: StructuralPlaceKind::BlockParameter {
                block: join,
                position,
            },
        });
        let structural_parameters = vec![StructuralParameterDeclaration {
            place,
            position,
            is_self: false,
            structural_type: self.structural_type,
            multiplicity: self.multiplicity,
            access: self.result_access,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        }];
        let parameters = self
            .values
            .iter()
            .map(|value| {
                Ok(ValueDeclaration {
                    id: value_id(allocate_dense(self.next_value)?),
                    ..*value
                })
            })
            .collect::<Result<Vec<_>, LoweringError>>()?;
        let continuation = ValueContinuation {
            block: join,
            parameters,
            structural_parameters,
            place,
            remaining_owners: Vec::new(),
            pass_through: Vec::new(),
            residuals: Vec::new(),
        };
        self.complete_borrowed(argument, &continuation)?;
        self.start(join);
        *self.values = continuation.parameters.clone();
        self.evaluation.parameters = continuation.parameters;
        self.evaluation.block_structural_parameters = continuation.structural_parameters;
        Ok(place)
    }

    /// Resolve a checked source plan to a whole-owned state parameter's
    /// ingress place. `StructuralLocal` names a parameter directly by symbol;
    /// `Parameter` counts the authored parameter list filtered to non-const,
    /// non-primitive entries, matching the checker's record-place producer.
    /// Returns `None` when the plan names no parameter at all.
    fn parameter_source(
        &self,
        argument: &checked_trees::CheckedUnitStructuralArgumentPlan,
    ) -> Result<Option<PlaceId>, LoweringError> {
        let (_, authored) = crate::expression_preparation::source_custody::authored_state(
            self.checked,
            self.state,
        )?;
        let parameters = self.checked.state_parameters(authored);
        let (position, parameter) = match &argument.source {
            checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol } => {
                let Some(entry) = parameters
                    .iter()
                    .enumerate()
                    .find(|(_, parameter)| parameter.symbol == *symbol)
                else {
                    return Ok(None);
                };
                entry
            }
            checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter {
                parameter_index,
            } => {
                let Some(entry) = parameters
                    .iter()
                    .enumerate()
                    .filter(|(_, parameter)| {
                        !parameter.is_const
                            && self
                                .checked
                                .primitive_type_reference(parameter.type_reference)
                                .is_none()
                    })
                    .nth(*parameter_index as usize)
                else {
                    return unsupported("owned selection parameter index is out of range");
                };
                entry
            }
            _ => return Ok(None),
        };
        if parameter.is_self || parameter.is_const || parameter.is_mutable {
            return unsupported("owned selection parameter is not immutable owned ingress");
        }
        let declaration = self
            .evaluation
            .structural_parameters
            .iter()
            .find(|(source_position, _)| *source_position as usize == position)
            .map(|(_, declaration)| declaration)
            .ok_or(LoweringError::Unsupported(
                "owned selection parameter has no signature place",
            ))?;
        if declaration.access != StructuralAccess::Owned
            || declaration.multiplicity != StructuralMultiplicity::Affine
            || declaration.structural_type
                != lookup_type_id(self.type_ids, &argument.type_identity)?
            || !declaration.qualifications.is_empty()
            || !declaration.projected_qualifications.is_empty()
        {
            return unsupported("owned selection parameter lacks whole plain-affine custody");
        }
        Ok(Some(declaration.place))
    }

    /// The once-evaluated root carrying a projected move: an established local
    /// keeps its emitted place; a structural product emits its producing call.
    /// Returns the root place, the root's checked type identity for residual
    /// reconstruction, and its argument source plan for stamped evidence.
    fn projected_root(
        &mut self,
        source: CheckedStructuralValueHandle,
        path: Vec<checked_trees::CheckedUnitStructuralPathSegment>,
        leaf_type_identity: String,
    ) -> Result<ProjectedMove, LoweringError> {
        let node = self
            .checked
            .facts
            .values
            .structural_values
            .nodes
            .get(source)
            .clone();
        let (root, root_type_identity, root_source) = match node.kind {
            CheckedStructuralValueKind::Place(argument) => {
                let root_source = argument.source.clone();
                if !argument.path.is_empty()
                    || argument.access != checked_trees::CheckedStructuralAccess::Owned
                {
                    return unsupported("projected selection root requires whole owned custody");
                }
                // The root is an established local place or a whole-owned
                // parameter's signature place; both arrive under
                // `StructuralLocal`/`Parameter` source plans that
                // `parameter_source` and the local namespace resolve exactly.
                let place = match &root_source {
                    checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralLocal {
                        symbol,
                    } => match self
                        .evaluation
                        .structural_locals
                        .iter()
                        .find(|(source, _)| *source == *symbol)
                        .map(|(_, argument)| argument.place)
                    {
                        Some(place) => place,
                        None => {
                            self.parameter_source(&argument)?
                                .ok_or(LoweringError::Unsupported(
                                    "projected selection root place missing",
                                ))?
                        }
                    },
                    checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter {
                        ..
                    } => self
                        .parameter_source(&argument)?
                        .ok_or(LoweringError::Unsupported(
                            "projected selection root place missing",
                        ))?,
                    _ => {
                        return unsupported("projected selection root is not a structural local");
                    }
                };
                if !self.sources.iter().any(|source| source.place == place) {
                    return unsupported(
                        "projected selection root is absent from its receipt sources",
                    );
                }
                (place, argument.type_identity, root_source)
            }
            CheckedStructuralValueKind::Call { .. } => {
                let mut matching = self
                    .operand_calls
                    .iter()
                    .filter(|call| call.value == source);
                let call = matching.next().ok_or(LoweringError::Unsupported(
                    "projected selection operand call missing",
                ))?;
                if matching.next().is_some() {
                    return unsupported("projected selection operand call is duplicated");
                }
                let CheckedUnitEffectOperationPlan::StructuralCall { result, .. } =
                    call.operation()
                else {
                    return unsupported(
                        "projected selection call drifted from its structural producer",
                    );
                };
                let root_type_identity = result.type_identity.clone();
                let binding_ordinal = result.binding_ordinal;
                let evaluated = self.evaluation.arguments(
                    self.checked,
                    self.machine,
                    self.state,
                    call.operation(),
                    self.values,
                    self.next_value,
                    self.next_block,
                    self.next_edge,
                    self.operations,
                    self.calls,
                )?;
                let declaration = (self.call_emitter)(
                    call.operation(),
                    evaluated.as_deref(),
                    self.calls,
                    self.operations,
                    self.next_place,
                )?;
                let StructuralPlaceKind::OperationResult {
                    structural_type, ..
                } = declaration.kind
                else {
                    return unsupported("projected selection call has no result place");
                };
                if structural_type != lookup_type_id(self.type_ids, &root_type_identity)? {
                    return unsupported("projected selection call changed its returned type");
                }
                (
                    declaration.id,
                    root_type_identity,
                    checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                        binding_ordinal,
                    },
                )
            }
            _ => {
                return unsupported("projected selection requires a place or call root");
            }
        };
        Ok(ProjectedMove {
            root,
            path,
            leaf_type_identity,
            root_type_identity,
            root_source,
        })
    }

    fn complete_value(
        &mut self,
        selected: PlaceId,
        projection: Option<&ProjectedMove>,
        continuation: &ValueContinuation,
    ) -> Result<(), LoweringError> {
        // The join binds owner slots positionally, so arguments are built
        // against the same roster: the displaced slot (the first candidate,
        // or the moved root on an ordinary move) is absent, every other
        // candidate slot receives the next still-unselected source in roster
        // order, and owners interleaved between candidates keep their own
        // places. A fresh result cannot occupy a source's residual slot, so
        // the unselected source the result slot displaced dies on this edge.
        // Plain-affine contents have no cleanup, making it a trivial discard,
        // and the join frontier is identical on every incoming path.
        let displaced = (!self.sources.is_empty()
            && !self.sources.iter().any(|source| source.place == selected))
        .then(|| self.first_candidate())
        .flatten();
        let displaced_place = displaced.map(|position| self.owners[position].value.place);
        // The first candidate's owner slot is the one the result parameter
        // displaced; a selected non-first candidate's slot is an ordinary
        // residual slot filled by the surviving candidates. An ordinary move
        // instead removes the moved root's own slot.
        let removed = self.first_candidate().or_else(|| {
            self.owners
                .iter()
                .position(|owner| owner.value.place == selected)
        });
        let mut unselected = self
            .sources
            .iter()
            .map(|source| source.place)
            .filter(|place| *place != selected && Some(*place) != displaced_place);
        let mut structural_arguments = Vec::new();
        for (position, owner) in self.owners.iter().enumerate() {
            if Some(position) == removed {
                continue;
            }
            let place = if self
                .sources
                .iter()
                .any(|source| source.place == owner.value.place)
            {
                let residual = unselected.next().ok_or(LoweringError::Unsupported(
                    "owned selection residual sources do not cover their slots",
                ))?;
                // A residual slot carries the surviving source at the slot's
                // own type. The roster's ordered complement can only rebind a
                // source of the same type: on an edge selecting a later
                // alternative the survivor sequence shifts, so a differently
                // typed root would land in its sibling's slot. Reject that
                // custody join here rather than emit an edge the terminal
                // verifier must refuse.
                let residual_type = self
                    .owners
                    .iter()
                    .find(|candidate| candidate.value.place == residual)
                    .map(|candidate| candidate.value.structural_type)
                    .ok_or(LoweringError::Unsupported(
                        "owned selection residual source has no frontier type",
                    ))?;
                if residual_type != owner.value.structural_type {
                    return unsupported(
                        "owned selection residual sources need uniform custody types",
                    );
                }
                residual
            } else {
                owner.value.place
            };
            structural_arguments.push(StructuralArgument {
                place,
                path: Vec::new(),
                access: StructuralAccess::Owned,
            });
        }
        if unselected.next().is_some() {
            return unsupported("owned selection has untransported residual candidates");
        }
        let selected_path = projection
            .map(|projected| lower_structural_path(&projected.path))
            .unwrap_or_default();
        structural_arguments.push(StructuralArgument {
            place: selected,
            path: selected_path,
            access: StructuralAccess::Owned,
        });
        if structural_arguments.len() != continuation.structural_parameters.len() {
            return unsupported("structural value has unequal residual ownership at its join");
        }
        // A projected move keeps the root's untouched structural complement
        // owned on this same edge. Reconstruction against the checked root
        // type yields the exact residual schedule in positional order; the
        // terminal verifier replays it independently.
        let residual_affine_discards = match projection {
            Some(projected) => {
                if projected.root != selected || projected.path.is_empty() {
                    return unsupported(
                        "projected selection root disagrees with its moved argument",
                    );
                }
                let moved = [(
                    projected.path.as_slice(),
                    projected.leaf_type_identity.as_str(),
                )];
                crate::unit::unit_cleanup::checked_partial_affine_residuals(
                    &self
                        .checked
                        .facts
                        .flow
                        .terminal_unit_effects
                        .structural_types,
                    &projected.root_source,
                    &projected.root_type_identity,
                    &moved,
                    usize::MAX,
                )?
                .iter()
                .map(|residual| {
                    Ok(terminal_psi::StructuralAffineDiscard {
                        place: selected,
                        path: lower_structural_path(&residual.path),
                        structural_type: lookup_type_id(self.type_ids, &residual.type_identity)?,
                    })
                })
                .collect::<Result<Vec<_>, LoweringError>>()?
            }
            None => Vec::new(),
        };
        let mut trivial_affine_discards = Vec::new();
        if let Some(place) = displaced_place {
            trivial_affine_discards.push(place);
        }
        let mut edge = self.edge(
            continuation.block,
            self.values.iter().map(|value| value.id).collect(),
            structural_arguments,
        )?;
        edge.trivial_affine_discards = trivial_affine_discards.clone();
        self.finish(Terminator::Jump {
            edge: edge.edge,
            target: edge.target,
            arguments: edge.arguments,
            erased_arguments: Vec::new(),
            structural_arguments: edge.structural_arguments,
            trivial_affine_discards,
            residual_affine_discards,
        });
        Ok(())
    }

    /// A borrowed branch joins one shared referent place at the result
    /// parameter. Unlike `complete_value` there is no owner roster, no
    /// displaced candidate and no residual schedule: the referent's owner
    /// stays live, and the single structural parameter must be the
    /// `SharedBorrow` result slot this edge's argument fills.
    fn complete_borrowed(
        &mut self,
        argument: StructuralArgument,
        continuation: &ValueContinuation,
    ) -> Result<(), LoweringError> {
        if continuation.structural_parameters.len() != 1
            || !continuation.pass_through.is_empty()
            || !continuation.residuals.is_empty()
        {
            return unsupported("borrowed selection joined residual ownership");
        }
        let parameter = &continuation.structural_parameters[0];
        if parameter.place != continuation.place
            || parameter.access != StructuralAccess::SharedBorrow
            || parameter.multiplicity != StructuralMultiplicity::Unrestricted
            || parameter.structural_type != self.structural_type
            || !parameter.qualifications.is_empty()
            || !parameter.projected_qualifications.is_empty()
        {
            return unsupported("borrowed selection parameter changed its custody");
        }
        let edge = self.edge(
            continuation.block,
            self.values.iter().map(|value| value.id).collect(),
            vec![argument],
        )?;
        self.finish(Terminator::Jump {
            edge: edge.edge,
            target: edge.target,
            arguments: edge.arguments,
            erased_arguments: Vec::new(),
            structural_arguments: edge.structural_arguments,
            trivial_affine_discards: Vec::new(),
            residual_affine_discards: Vec::new(),
        });
        Ok(())
    }

    fn rebind_local_cases(&mut self) -> Result<(), LoweringError> {
        let statements = self.checked.statement_table.statements(
            crate::expression_preparation::source_custody::authored_state(
                self.checked,
                self.state,
            )?
            .1
            .statement_nodes,
        );
        let mut cases = Vec::new();
        for (symbol, argument) in &self.evaluation.structural_locals {
            let Some(local) = statements.iter().find_map(|statement| match statement {
                checked_trees::statement::StatementNode::LocalData(local)
                    if local.symbol == *symbol =>
                {
                    Some(local)
                }
                _ => None,
            }) else {
                continue;
            };
            let identity = self.checked.normalized_type_identity(local.type_reference);
            if self.structural_types.iter().any(|declaration| {
                declaration.identity == identity.as_str()
                    && matches!(declaration.shape, StructuralTypeShape::Sum { .. })
            }) {
                cases.push(
                    crate::expression_preparation::bindings::structural_cases::LocalCaseBinding::new(
                        self.checked,
                        *symbol,
                        local.type_reference,
                        argument.place,
                        self.structural_types,
                    )?,
                );
            }
        }
        self.evaluation.local_cases = cases;
        Ok(())
    }

    pub(super) fn scalar(
        &mut self,
        role: CheckedScalarExpressionRole,
        value: checked_trees::CheckedScalarComputationHandle,
        source_count: usize,
    ) -> Result<ValueDeclaration, LoweringError> {
        let value = self.evaluation.source_value(
            self.checked,
            self.machine,
            self.state,
            self.statement,
            role,
            &checked_trees::CheckedCallScalarArgument::Computation(value),
            source_count,
            self.values,
            self.next_value,
            self.next_block,
            self.next_edge,
            self.operations,
            self.calls,
        )?;
        Ok(value)
    }

    fn edge(
        &mut self,
        target: BlockId,
        arguments: Vec<ValueId>,
        structural_arguments: Vec<StructuralArgument>,
    ) -> Result<SuccessorEdge, LoweringError> {
        Ok(SuccessorEdge {
            edge: edge_id(allocate_dense(self.next_edge)?),
            target,
            arguments,
            structural_arguments,
            erased_arguments: Vec::new(),
            trivial_affine_discards: Vec::new(),
        })
    }

    fn finish(&mut self, terminator: Terminator) {
        self.evaluation.blocks.push(Block {
            id: self.evaluation.current,
            parameters: std::mem::take(&mut self.evaluation.parameters),
            erased_scalar_formals: Vec::new(),
            structural_parameters: std::mem::take(&mut self.evaluation.block_structural_parameters),
            operations: self.operations[self.evaluation.operation_start..].to_vec(),
            terminator,
        });
    }

    fn start(&mut self, block: BlockId) {
        self.evaluation.current = block;
        self.evaluation.operation_start = self.operations.len();
        self.operations.byte_lengths.clear();
    }
}
