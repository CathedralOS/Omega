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
use crate::emission::operation_emission::expressions::{
    LoweredDirectExpression, emit_direct_expression, emit_scalar_leaf,
};
use crate::expression_preparation::bindings::LoanAccess;
use crate::expression_preparation::source_custody::structural as source_custody;
use checked_trees::{CheckedStructuralValueHandle, CheckedStructuralValueKind};

mod completed_storage;
mod copied_place;
mod dispatch;

pub(super) type StructuralOperandEmitter<'a> = dyn FnMut(
        &CheckedUnitEffectOperationPlan,
        Option<&[ValueDeclaration]>,
        usize,
        &mut argument_evaluation::Evaluation,
        &mut Vec<ValueDeclaration>,
        &mut u64,
        &mut u64,
        &mut u64,
        &mut CallEmissionContext<'_>,
        &mut OperationBuffer,
        &mut u64,
        &mut Vec<StructuralPlaceDeclaration>,
    ) -> Result<Option<StructuralPlaceDeclaration>, LoweringError>
    + 'a;

#[allow(clippy::too_many_arguments)]
pub(crate) fn emit(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    state: symbols::SymbolHandle,
    operation: &CheckedUnitEffectOperationPlan,
    structural_types: &[StructuralTypeDeclaration],
    type_ids: &[(String, StructuralTypeId)],
    domain_ids: &[crate::terminal_identities::StructuralDomainBinding],
    next_place: &mut u64,
    temporary_places: &mut Vec<StructuralPlaceDeclaration>,
    operand_emitter: &mut StructuralOperandEmitter<'_>,
    calls: &mut CallEmissionContext<'_>,
    evaluation: &mut argument_evaluation::Evaluation,
    values: &mut Vec<ValueDeclaration>,
    next_value: &mut u64,
    next_block: &mut u64,
    next_edge: &mut u64,
    operations: &mut OperationBuffer,
) -> Result<Option<StructuralPlaceDeclaration>, LoweringError> {
    source_custody::validate(checked, machine, state, operation)?;
    let CheckedUnitEffectOperationPlan::EstablishStructuralValue {
        result,
        value,
        calls: operand_calls,
        operand_source,
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
                        .find(|local| local.symbol == source.symbol)
                        .map(|local| local.argument.clone())
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
        domain_ids,
        type_reference: checked
            .facts
            .values
            .structural_values
            .root_for_expression(
                state,
                result.statement_index,
                checked
                    .facts
                    .values
                    .structural_values
                    .nodes
                    .get(*value)
                    .expression,
            )
            .map(|root| root.type_reference)
            .unwrap_or_default(),
        next_place,
        temporary_places,
        calls,
        operand_calls,
        operand_emitter,
        evaluation,
        values,
        next_value,
        next_block,
        next_edge,
        operations,
        sources,
        owners,
        result_access,
        published_places: Vec::new(),
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
    } else if emission.sources.is_empty()
        && let Some((source, region, loan)) = match &checked
            .facts
            .values
            .structural_values
            .nodes
            .get(*value)
            .kind
        {
            CheckedStructuralValueKind::BorrowedSliceView { source, region }
                if source.access == checked_trees::CheckedStructuralAccess::SharedBorrow =>
            {
                Some((source, region, LoanAccess::Shared))
            }
            CheckedStructuralValueKind::MutableSliceView { source, region }
                if source.access == checked_trees::CheckedStructuralAccess::MutableBorrow =>
            {
                Some((source, region, LoanAccess::Mutable))
            }
            _ => None,
        }
    {
        emission.direct_element_view(
            source,
            region,
            checked
                .facts
                .values
                .structural_values
                .nodes
                .get(*value)
                .expression,
            loan,
        )?
    } else {
        let place = emission.value(*value, None)?;
        // A moved place is materialized into the result binding; a copied
        // parameter is already a fresh operation result.
        if emission.sources.is_empty()
            && let CheckedStructuralValueKind::Place(argument) = &checked
                .facts
                .values
                .structural_values
                .nodes
                .get(*value)
                .kind
            && emission.copied_parameter(argument)?.is_none()
            && !matches!(
                argument.source,
                checked_trees::CheckedUnitStructuralArgumentSourcePlan::ByteSequenceLiteral { .. }
            )
        {
            emission.materialize_place(place)?
        } else {
            place
        }
    };
    // Private arm producers are not authored result ordinals. Publish exactly
    // one completed place for this binding, whether a direct producer or join.
    // A call-rooted producer's place was catalogued by the call emitter as its
    // own call result, so it claims no temporary row and yields no declaration
    // for the caller to republish.
    let declaration = emission
        .temporary_places
        .iter()
        .position(|item| item.id == place)
        .map(|position| emission.temporary_places.remove(position));
    let mut published = emission
        .published_places
        .iter()
        .filter(|row| row.id == place);
    let published_declaration = published.next();
    if published.next().is_some() || (declaration.is_some() && published_declaration.is_some()) {
        return unsupported("structural value place was declared more than once");
    }
    let completed_declaration =
        declaration
            .as_ref()
            .or(published_declaration)
            .ok_or(LoweringError::Unsupported(
                "structural value place was not declared",
            ))?;
    let completed = completed_storage::completed_result(
        place,
        completed_declaration,
        emission.operations,
        emission.evaluation.current,
        &emission.evaluation.block_structural_parameters,
    )?;
    emission.evaluation.establish_structural_binding(
        checked,
        state,
        result,
        !matches!(
            operand_source,
            Some(checked_trees::CheckedArrayConstructionSource::CallArgument { .. })
        ),
        completed.into(),
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
    let (_, authored) =
        crate::expression_preparation::source_custody::authored_state(checked, state)?;
    let statements = checked.statement_table.statements(authored.statement_nodes);
    for crate::expression_preparation::bindings::StructuralLocalBinding {
        symbol, argument, ..
    } in &evaluation.structural_locals
    {
        if owners.iter().any(|owner| owner.symbol == *symbol) || !available(*symbol)? {
            continue;
        }
        let (ordinal, local) = checked.state_local_declaration(authored, *symbol).ok_or(
            LoweringError::Unsupported("selection survivor has no authored local"),
        )?;
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
                    qualification_establishments: Vec::new(),
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
    pub(super) domain_ids: &'a [crate::terminal_identities::StructuralDomainBinding],
    type_reference: checked_trees::types::TypeReferenceHandle,
    pub(super) next_place: &'b mut u64,
    pub(super) temporary_places: &'b mut Vec<StructuralPlaceDeclaration>,
    pub(super) calls: &'b mut CallEmissionContext<'calls>,
    pub(super) operand_calls: &'a [checked_trees::CheckedStructuralValueCall],
    pub(super) operand_emitter: &'b mut StructuralOperandEmitter<'a>,
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
    /// Exact declarations already published by nested operation emission.
    /// These are receipts, not new catalog entries owned by this value.
    published_places: Vec<StructuralPlaceDeclaration>,
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
    /// Execute this call's retained preparation only when the value evaluator
    /// reaches its node. The ordinary argument schedule interleaves scalar
    /// observations and structural producers in authored formal order.
    fn emit_operand_call(
        &mut self,
        call: &checked_trees::CheckedStructuralValueCall,
    ) -> Result<StructuralPlaceDeclaration, LoweringError> {
        use super::super::argument_schedule::{self, Step};
        let planned = call
            .operands()
            .iter()
            .chain(std::iter::once(call.operation()))
            .cloned()
            .collect::<Vec<_>>();
        let schedule =
            argument_schedule::build_operations(self.checked, self.machine, self.state, &planned)?;
        let mut staged_values = vec![Vec::new(); planned.len()];
        let mut source_count = None;
        let mut completed = None;
        for step in schedule {
            let (index, staged) = match step {
                Step::CaptureBefore(statement) => {
                    self.evaluation.capture_record_coordinates_before(
                        statement,
                        self.structural_types,
                        self.values,
                        self.next_place,
                        self.next_value,
                        self.operations,
                        self.temporary_places,
                    )?;
                    continue;
                }
                Step::Begin => {
                    if source_count.replace(self.values.len()).is_some() {
                        return unsupported("structural operand argument group is already active");
                    }
                    continue;
                }
                Step::End => {
                    let count = source_count.take().ok_or(LoweringError::Unsupported(
                        "structural operand argument group is absent",
                    ))?;
                    self.values.truncate(count);
                    continue;
                }
                Step::Argument { operation, ordinal } => {
                    let count = source_count.ok_or(LoweringError::Unsupported(
                        "structural operand scalar has no source prefix",
                    ))?;
                    if staged_values[operation].len() != ordinal {
                        return unsupported("structural operand scalar ordinals are not dense");
                    }
                    let value = self.evaluation.argument_at(
                        self.checked,
                        self.machine,
                        self.state,
                        &planned[operation],
                        ordinal,
                        count,
                        self.values,
                        self.next_value,
                        self.next_block,
                        self.next_edge,
                        self.operations,
                        self.calls,
                    )?;
                    staged_values[operation].push(self.values.len());
                    self.values.push(value);
                    continue;
                }
                Step::Subslice { .. } => {
                    return unsupported(
                        "structural value call requires retained slice argument transport",
                    );
                }
                Step::Constructor(index) | Step::Call(index) if source_count.is_some() => {
                    (index, true)
                }
                Step::Ordinary(index) if source_count.is_none() => (index, false),
                _ => return unsupported("structural operand argument schedule is unbalanced"),
            };
            let operation = &planned[index];
            let count = source_count.unwrap_or(self.values.len());
            let evaluated = if staged && operation.call().is_some() {
                Some(
                    staged_values[index]
                        .iter()
                        .map(|slot| {
                            self.values
                                .get(*slot)
                                .copied()
                                .ok_or(LoweringError::Unsupported(
                                    "structural operand scalar lost its retained slot",
                                ))
                        })
                        .collect::<Result<Vec<_>, _>>()?,
                )
            } else if operation.call().is_some() {
                self.evaluation.arguments(
                    self.checked,
                    self.machine,
                    self.state,
                    operation,
                    self.values,
                    self.next_value,
                    self.next_block,
                    self.next_edge,
                    self.operations,
                    self.calls,
                )?
            } else {
                None
            };
            let declaration = (self.operand_emitter)(
                operation,
                evaluated.as_deref(),
                count,
                self.evaluation,
                self.values,
                self.next_value,
                self.next_block,
                self.next_edge,
                self.calls,
                self.operations,
                self.next_place,
                self.temporary_places,
            )?;
            if index + 1 == planned.len() {
                completed = declaration;
            }
        }
        let completed = completed.ok_or(LoweringError::Unsupported(
            "structural operand call did not publish its result",
        ))?;
        self.published_places.push(completed);
        Ok(completed)
    }

    pub(super) fn record_field_value(
        &mut self,
        value: CheckedStructuralValueHandle,
        reference: checked_trees::types::TypeReferenceHandle,
    ) -> Result<PlaceId, LoweringError> {
        // A fresh field's continuation joins that field, not the enclosing
        // selection's result/residual slots. Its constructors leave incoming
        // owners live on every branch; only the outer completion may displace
        // a candidate. Source replay still refuses hidden owned child transfers.
        let sources = std::mem::take(&mut self.sources);
        let owners = std::mem::take(&mut self.owners);
        let parent_reference = std::mem::replace(&mut self.type_reference, reference);
        let result = self.value(value, None);
        self.type_reference = parent_reference;
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
            // A borrowed `&[T]` view lowers to `EstablishElementView` and a
            // `&mut [T]` view to `EstablishMutableElementView`: the collection
            // argument resolves against the current locals and signature
            // parameters exactly as a `&T` leaf does, and the fresh view result
            // joins the selection's block parameter under the view's loan.
            CheckedStructuralValueKind::BorrowedSliceView { source, region } => self
                .joined_element_view(
                    &source,
                    &region,
                    node.expression,
                    LoanAccess::Shared,
                    continuation,
                ),
            CheckedStructuralValueKind::MutableSliceView { source, region } => self
                .joined_element_view(
                    &source,
                    &region,
                    node.expression,
                    LoanAccess::Mutable,
                    continuation,
                ),
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
                        self.values.iter().map(|value| value.scalar_type),
                    )
                    .with_structural_parameters(&self.evaluation.structural_parameters)
                    .with_structural_locals(&self.evaluation.structural_locals)
                    .shared_structural_argument(
                        &source,
                        crate::expression_preparation::source_custody::authored_state(
                            self.checked,
                            self.state,
                        )?
                        .0,
                        self.checked.state_parameters(
                            crate::expression_preparation::source_custody::authored_state(
                                self.checked,
                                self.state,
                            )?
                            .1,
                        ),
                    )?;
                    self.complete_loan(argument, continuation, LoanAccess::Shared)?;
                    return Ok(continuation.place);
                }
                let source_place = match source.source {
                    checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter {
                        parameter_index,
                    } => {
                        let (_, parameter) = self
                            .evaluation
                            .structural_parameters
                            .get(parameter_index as usize)
                            .ok_or(LoweringError::Unsupported(
                                "reference initializer parameter missing",
                            ))?;
                        if parameter.access != StructuralAccess::MutableBorrow
                            || parameter.structural_type
                                != lookup_type_id(self.type_ids, &source.type_identity)?
                            || !parameter.qualifications.is_empty()
                            || !parameter.projected_qualifications.is_empty()
                        {
                            return unsupported("reference initializer changes ingress custody");
                        }
                        parameter.place
                    }
                    // A local-rooted loan mints that local's established place:
                    // `Indirect { target: &mut owner }` lends `owner`'s own
                    // storage whole, so the source must name one complete
                    // owned local and the local's carrier must be the exact
                    // referent the field declared.
                    checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralLocal {
                        symbol,
                    } => {
                        let mut locals = self
                            .evaluation
                            .structural_locals
                            .iter()
                            .filter(|local| local.symbol == symbol);
                        let local = locals.next().ok_or(LoweringError::Unsupported(
                            "reference initializer local ingress missing",
                        ))?;
                        if locals.next().is_some()
                            || !local.argument.path.is_empty()
                            || local.argument.access != StructuralAccess::Owned
                            || local.structural_type
                                != lookup_type_id(self.type_ids, &source.type_identity)?
                        {
                            return unsupported("reference initializer changes ingress custody");
                        }
                        local.argument.place
                    }
                    _ => {
                        return unsupported("reference initializer has no established ingress");
                    }
                };
                if !source.path.is_empty()
                    || source.access != checked_trees::CheckedStructuralAccess::MutableBorrow
                {
                    return unsupported("reference initializer changes ingress custody");
                }
                let declaration = super::super::reference_results::emit_carrier(
                    self.structural_type,
                    StructuralArgument {
                        place: source_place,
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
            CheckedStructuralValueKind::FixedArray { elements } => {
                let place = self.fixed_array(&elements)?;
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
                let declaration = self.emit_operand_call(call)?;
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
                // Copied provenance observes its original backing; an owned
                // projection instead transfers one child on the selected edge
                // and disposes of the root's residual complement there. Only
                // the owned transfer requires a structural continuation.
                if path.is_empty()
                    || lookup_type_id(self.type_ids, &type_identity)? != self.structural_type
                {
                    return unsupported("projected selection changed its moved child type");
                }
                if let CheckedStructuralValueKind::CopiedStructuralPlace {
                    source: copied, ..
                } = &self
                    .checked
                    .facts
                    .values
                    .structural_values
                    .nodes
                    .get(source)
                    .kind
                {
                    // A projection of a copy observes the selected subtree of
                    // its original backing. It neither transfers the root nor
                    // creates affine residuals. Source replay has rejoined the
                    // copied root independently; retain its complete path when
                    // appending the projection and publish one fresh leaf.
                    if copied.access != checked_trees::CheckedStructuralAccess::SharedBorrow
                        || self.multiplicity != StructuralMultiplicity::Unrestricted
                    {
                        return unsupported("copied projection changed its readable copy custody");
                    }
                    let mut argument = self.copied_place_argument(copied)?;
                    argument.path.extend(lower_structural_path(&path)?);
                    return self.copied_leaf_place(&argument, continuation);
                }
                let continuation = continuation.ok_or(LoweringError::Unsupported(
                    "projected selection requires a structural continuation",
                ))?;
                let projected = self.projected_root(source, path, type_identity)?;
                self.complete_value(projected.root, Some(&projected), continuation)?;
                Ok(continuation.place)
            }
            CheckedStructuralValueKind::ScalarCasePlace { source, .. } => {
                if lookup_type_id(self.type_ids, &source.type_identity)? != self.structural_type {
                    return unsupported("scalar case place changed its leaf type");
                }
                let authored = crate::expression_preparation::source_custody::authored_state(
                    self.checked,
                    self.state,
                )?;
                let argument = crate::expression_preparation::bindings::ScalarBindings::new(
                    self.values.iter().map(|value| value.scalar_type),
                )
                .with_structural_parameters(&self.evaluation.structural_parameters)
                .with_structural_locals(&self.evaluation.structural_locals)
                .shared_structural_argument(
                    &source,
                    authored.0,
                    self.checked.state_parameters(authored.1),
                )?;
                self.projected_case_place(&argument, continuation)
            }
            CheckedStructuralValueKind::ViewElementCopy { reads } => {
                self.view_element_copy(&reads, continuation)
            }
            CheckedStructuralValueKind::CopiedStructuralPlace { source, leaf } => {
                // Source replay retains the qualified leaf identity, while
                // the catalog names its storage carrier. Resolve through the
                // retained type, never by deleting text from an identity.
                let identity = if self.checked.normalized_type_identity(leaf).as_str()
                    == source.type_identity
                {
                    let carrier = super::super::structural_carrier_type(self.checked, leaf)?;
                    self.checked.normalized_type_identity(carrier).into_string()
                } else {
                    // Borrowed view sources already name their projected
                    // carrier; source custody independently checks that join.
                    source.type_identity.clone()
                };
                if lookup_type_id(self.type_ids, &identity)? != self.structural_type {
                    return unsupported("copied place changed its leaf type");
                }
                let argument = self.copied_place_argument(&source)?;
                self.copied_leaf_place(&argument, continuation)
            }
            CheckedStructuralValueKind::ZeroedScalarArray {
                element,
                element_count,
            } => {
                // The planner's synthesized zero has no authored operand to
                // replay — emit the verified literal leaf directly.
                let computation = self
                    .checked
                    .facts
                    .values
                    .scalar_computations
                    .nodes
                    .get(element);
                let checked_trees::CheckedScalarComputationKind::Value(
                    checked_trees::CheckedScalarExpression::IntegerLiteral { literal },
                ) = &computation.kind
                else {
                    return unsupported("zeroed scalar array element is not a literal zero");
                };
                let scalar_type = crate::emission::scalar_types::terminal_scalar_type(
                    computation.primitive_type,
                )?;
                let value = crate::emission::scalar_types::integer_value(literal, scalar_type)?;
                let zero = emit_scalar_leaf(
                    OperationKind::IntegerConstant { value },
                    scalar_type,
                    self.next_value,
                    self.operations,
                );
                self.values.push(ValueDeclaration {
                    qualifications: Default::default(),
                    id: zero,
                    scalar_type,
                });
                let place = place_id(allocate_dense(self.next_place)?);
                let operation = self.operations.allocate();
                self.operations.push(Operation {
                    static_reach_binding: None,
                    suspension_crossing: None,
                    id: operation,
                    result: OperationResult::Structural(terminal_psi::StructuralOperationResult {
                        qualification_establishments: Vec::new(),
                        place,
                        structural_type: self.structural_type,
                        multiplicity: self.multiplicity,
                        qualifications: Vec::new(),
                        projected_qualifications: Vec::new(),
                        claims: Vec::new(),
                    }),
                    kind: OperationKind::EstablishScalarArray {
                        elements: vec![
                            zero;
                            usize::try_from(element_count).map_err(|_| {
                                LoweringError::Unsupported(
                                    "zeroed scalar array leaf count exceeds usize",
                                )
                            })?
                        ],
                    },
                });
                self.temporary_places.push(StructuralPlaceDeclaration {
                    id: place,
                    kind: StructuralPlaceKind::OperationResult {
                        producer: operation,
                        structural_type: self.structural_type,
                    },
                });
                if let Some(continuation) = continuation {
                    self.complete_value(place, None, continuation)?;
                }
                Ok(place)
            }
            CheckedStructuralValueKind::ScalarArray { elements } => {
                // Every leaf replays its own retained source computation in
                // authored row-major order, keyed by this array occurrence.
                // Private control flow may remap earlier saved values, so the
                // roster reads its final slots only after the last leaf.
                let source_count = self.values.len();
                for (ordinal, element) in elements.iter().enumerate() {
                    let evaluated = self.scalar(
                        CheckedScalarExpressionRole::StructuralArrayElement {
                            expression: node.expression,
                            element_ordinal: u32::try_from(ordinal).map_err(|_| {
                                LoweringError::Unsupported("scalar array leaf ordinal overflow")
                            })?,
                        },
                        *element,
                        source_count,
                    )?;
                    self.values.push(evaluated);
                }
                let leaves = self.values[source_count..]
                    .iter()
                    .map(|value| value.id)
                    .collect::<Vec<_>>();
                self.values.truncate(source_count);
                let place = place_id(allocate_dense(self.next_place)?);
                let operation = self.operations.allocate();
                self.operations.push(Operation {
                    static_reach_binding: None,
                    suspension_crossing: None,
                    id: operation,
                    result: OperationResult::Structural(terminal_psi::StructuralOperationResult {
                        qualification_establishments: Vec::new(),
                        place,
                        structural_type: self.structural_type,
                        multiplicity: self.multiplicity,
                        qualifications: Vec::new(),
                        projected_qualifications: Vec::new(),
                        claims: Vec::new(),
                    }),
                    kind: OperationKind::EstablishScalarArray { elements: leaves },
                });
                self.temporary_places.push(StructuralPlaceDeclaration {
                    id: place,
                    kind: StructuralPlaceKind::OperationResult {
                        producer: operation,
                        structural_type: self.structural_type,
                    },
                });
                if let Some(continuation) = continuation {
                    self.complete_value(place, None, continuation)?;
                }
                Ok(place)
            }
            // Source custody already refuses this kind; emission has no
            // shared view operand to establish it with either.
            CheckedStructuralValueKind::ByteLiteralView { .. } => Err(LoweringError::Unsupported(
                "structural construction has a literal view member",
            )),
            CheckedStructuralValueKind::ZeroedStructural { .. } => {
                let place = self.establish_zeroed()?;
                if let Some(continuation) = continuation {
                    self.complete_value(place, None, continuation)?;
                }
                Ok(place)
            }
            CheckedStructuralValueKind::Place(argument) => {
                if let checked_trees::CheckedUnitStructuralArgumentSourcePlan::ByteSequenceLiteral { bytes } = &argument.source {
                    let domains = source_custody::byte_literals::validate(
                        self.checked, node.expression, self.type_reference, &argument,
                    )?;
                    let qualifications = domains.into_iter().map(|domain|
                        crate::terminal_identities::lookup_domain_id_for_carrier(
                            self.domain_ids, domain, self.structural_type,
                        )
                    ).collect::<Result<Vec<_>, _>>()?;
                    let (declaration, _) = crate::emission::structural_byte_sequence_store::emit_owned_literal(
                        bytes, self.structural_type, qualifications, self.structural_types,
                        self.temporary_places, self.next_place, self.next_value,
                        &mut self.calls.next_obligation_identity, self.operations,
                    )?;
                    let place = declaration.id;
                    self.temporary_places.push(declaration);
                    if let Some(continuation) = continuation {
                        self.complete_value(place, None, continuation)?;
                    }
                    return Ok(place);
                }
                if let Some(source) = self.copied_parameter(&argument)? {
                    return self.copied_leaf_place(&source, continuation);
                }
                if self.sources.is_empty() && continuation.is_none() {
                    // An ordinary constructor consumes this whole source place,
                    // including a sum's active case. Source replay already checks
                    // its exact owned transfer. Only a selected value needs the
                    // per-arm source receipt and continuation handled below.
                    if let Some(place) = self.parameter_source(&argument)? {
                        return Ok(place);
                    }
                    return crate::expression_preparation::bindings::ScalarBindings::new(
                        self.values.iter().map(|value| value.scalar_type),
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
                        .find(|local| local.symbol == *symbol)
                        .map(|local| local.argument.place)
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
                    self.calls,
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
            CheckedStructuralValueKind::StructuralCase { .. } => {
                let place = self.structural_case(value)?;
                if let Some(continuation) = continuation {
                    self.complete_value(place, None, continuation)?;
                }
                Ok(place)
            }
            CheckedStructuralValueKind::Dispatch { subject, arms } => {
                self.dispatch(node.expression, subject, arms, continuation)
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
            self.evaluation.structural_locals.retain(
                |crate::expression_preparation::bindings::StructuralLocalBinding {
                     argument,
                     ..
                 }| argument.place != source,
            );
            for crate::expression_preparation::bindings::StructuralLocalBinding {
                argument, ..
            } in &mut self.evaluation.structural_locals
            {
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
        let argument = crate::expression_preparation::bindings::ScalarBindings::new(
            self.values.iter().map(|value| value.scalar_type),
        )
        .with_structural_parameters(&self.evaluation.structural_parameters)
        .with_structural_locals(&self.evaluation.structural_locals)
        .shared_structural_argument(
            source,
            crate::expression_preparation::source_custody::authored_state(
                self.checked,
                self.state,
            )?
            .0,
            self.checked.state_parameters(
                crate::expression_preparation::source_custody::authored_state(
                    self.checked,
                    self.state,
                )?
                .1,
            ),
        )?;
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
        self.complete_loan(argument, &continuation, LoanAccess::Shared)?;
        self.start(join);
        *self.values = continuation.parameters.clone();
        self.evaluation.parameters = continuation.parameters;
        self.evaluation.block_structural_parameters = continuation.structural_parameters;
        Ok(place)
    }

    /// Establish one borrowed element view without a selection join: the op
    /// result is itself the checked value, so the fresh place publishes
    /// directly as this binding's completed operation result. A shared view
    /// result is `Unrestricted`; an exclusive loan is affine custody, so a
    /// mutable view result carries this binding's declared multiplicity.
    fn direct_element_view(
        &mut self,
        source: &checked_trees::CheckedUnitStructuralArgumentPlan,
        region: &checked_trees::CheckedElementViewRegion,
        expression: checked_trees::expression::ExpressionHandle,
        loan: LoanAccess,
    ) -> Result<PlaceId, LoweringError> {
        let multiplicity = match loan {
            LoanAccess::Shared => StructuralMultiplicity::Unrestricted,
            LoanAccess::Mutable => self.multiplicity,
        };
        self.establish_element_view(source, region, expression, loan, multiplicity)
    }

    /// Establish one element view inside a selection: the fresh `Unrestricted`
    /// view result joins the selection's block parameter under the view's
    /// loan, and that parameter carries the binding's custody.
    fn joined_element_view(
        &mut self,
        source: &checked_trees::CheckedUnitStructuralArgumentPlan,
        region: &checked_trees::CheckedElementViewRegion,
        expression: checked_trees::expression::ExpressionHandle,
        loan: LoanAccess,
        continuation: Option<&ValueContinuation>,
    ) -> Result<PlaceId, LoweringError> {
        // A nested position — a record/case field value — names no selection
        // block parameter to join. The view establishes directly as a fresh
        // operation result, exactly as a top-level view binding does; the
        // caller already repointed `structural_type` and `multiplicity` at the
        // field's declared custody.
        let Some(continuation) = continuation else {
            return self.direct_element_view(source, region, expression, loan);
        };
        let place = self.establish_element_view(
            source,
            region,
            expression,
            loan,
            StructuralMultiplicity::Unrestricted,
        )?;
        self.complete_loan(
            StructuralArgument {
                place,
                path: Vec::new(),
                access: loan.structural_access(),
            },
            continuation,
            loan,
        )?;
        Ok(continuation.place)
    }

    /// Establish one element view over the collection `source` lends, as a
    /// fresh operation result of `multiplicity`. A shared loan lowers to
    /// `EstablishElementView` and binds only a `SharedBorrow` argument; an
    /// exclusive loan lowers to `EstablishMutableElementView` and binds the
    /// argument through the readable-loan binder, which admits either access.
    fn establish_element_view(
        &mut self,
        source: &checked_trees::CheckedUnitStructuralArgumentPlan,
        region: &checked_trees::CheckedElementViewRegion,
        expression: checked_trees::expression::ExpressionHandle,
        loan: LoanAccess,
        multiplicity: StructuralMultiplicity,
    ) -> Result<PlaceId, LoweringError> {
        // A range source contains an operation, not just an address. Use the
        // same storage binding and endpoint replay as calls and state edges;
        // publishing the enclosing collection would silently widen the view.
        if let Some(range) = super::super::view_ranges::range(source) {
            use super::super::view_ranges;
            if !matches!(
                region,
                checked_trees::CheckedElementViewRegion::WholeCollection
            ) {
                return unsupported("element range cannot replace a representation-byte region");
            }
            let storage = view_ranges::storage(
                self.checked,
                self.state,
                self.statement,
                source,
                self.structural_types,
            )?;
            let bindings = crate::expression_preparation::bindings::ScalarBindings::new(
                self.values.iter().map(|value| value.scalar_type),
            )
            .with_structural_parameters(&self.evaluation.structural_parameters)
            .with_structural_locals(&self.evaluation.structural_locals);
            let whole = storage
                .requires_establishment()
                .then(|| allocate_dense(self.next_place).map(place_id))
                .transpose()?;
            let (bound, establishment) =
                storage.bind(self.checked, self.state, &bindings, whole)?;
            if let Some(establishment) = establishment {
                self.temporary_places
                    .push(view_ranges::establish(&establishment, self.operations));
            }
            let (_, authored) = crate::expression_preparation::source_custody::authored_state(
                self.checked,
                self.state,
            )?;
            let site = match self
                .checked
                .statement_table
                .statements(authored.statement_nodes)
                .get(self.statement as usize)
            {
                Some(checked_trees::statement::StatementNode::LocalData(local))
                    if local.initial_value == expression =>
                {
                    checked_trees::CheckedSubsliceSite::LocalBinding
                }
                _ => checked_trees::CheckedSubsliceSite::Expression { expression },
            };
            let place = place_id(allocate_dense(self.next_place)?);
            self.temporary_places.push(view_ranges::emit(
                self.checked,
                view_ranges::ViewRangeSite {
                    state: self.state,
                    statement: self.statement,
                    site,
                    expression: range.expression,
                    retained: Some((range.start, range.end)),
                },
                bound,
                &source.path,
                self.structural_type,
                place,
                &bindings,
                self.values,
                self.next_value,
                self.operations,
            )?);
            return Ok(place);
        }
        let element = self
            .structural_types
            .iter()
            .find_map(|declaration| match declaration.shape {
                StructuralTypeShape::ElementView { element }
                    if declaration.id == self.structural_type =>
                {
                    Some(element)
                }
                // A byte-sequence view's element is always the `u8` scalar:
                // the carrier keeps no element field, so the view op's stride
                // resolves through the primitive row directly.
                StructuralTypeShape::ByteSequence(_) if declaration.id == self.structural_type => {
                    crate::terminal_identities::lookup_type_id(
                        self.type_ids,
                        crate::terminal_identities::BYTE_VIEW_ELEMENT_TYPE_IDENTITY,
                    )
                    .ok()
                }
                _ => None,
            })
            .ok_or(LoweringError::Unsupported(match loan {
                LoanAccess::Shared => "borrowed element view lost its element carrier",
                LoanAccess::Mutable => "mutable element view lost its element carrier",
            }))?;
        let bindings = crate::expression_preparation::bindings::ScalarBindings::new(
            self.values.iter().map(|value| value.scalar_type),
        )
        .with_structural_parameters(&self.evaluation.structural_parameters)
        .with_structural_locals(&self.evaluation.structural_locals);
        let (machine, authored) = crate::expression_preparation::source_custody::authored_state(
            self.checked,
            self.state,
        )?;
        let parameters = self.checked.state_parameters(authored);
        let argument = match loan {
            LoanAccess::Shared => {
                bindings.shared_structural_argument(source, machine, parameters)?
            }
            LoanAccess::Mutable => {
                bindings.borrowed_structural_argument(source, machine, parameters)?
            }
        };
        // The descriptor captures the byte endpoints now. Later field writes
        // and state transfers keep this formed region; they never reread the
        // authored selector to reconstruct a different borrowed window.
        let endpoints = match region {
            checked_trees::CheckedElementViewRegion::WholeCollection => None,
            checked_trees::CheckedElementViewRegion::RepresentationBytes { start, end } => {
                let start = self.scalar(
                    CheckedScalarExpressionRole::RepresentationViewOffset { expression },
                    *start,
                    self.sources.len(),
                )?;
                let index = crate::expression_preparation::prepare_expression::exact_u64_index(
                    LoweredDirectExpression::Parameter {
                        position: 0,
                        scalar_type: start.scalar_type,
                    },
                )?;
                let start = emit_direct_expression(
                    &index,
                    std::slice::from_ref(&start),
                    self.next_value,
                    self.operations,
                );
                let end = emit_direct_expression(
                    &LoweredDirectExpression::IntegerLiteral {
                        value: semantic_vocabulary::IntegerValue::Unsigned((*end).into()),
                        scalar_type: crate::emission::scalar_types::terminal_scalar_type(
                            checked_trees::types::PrimitiveType::U64,
                        )?,
                    },
                    &[],
                    self.next_value,
                    self.operations,
                );
                Some((start, end))
            }
        };
        let id = self.operations.allocate();
        let region = match endpoints {
            None => terminal_psi::ElementViewRegion::WholeCollection,
            Some((start, end)) => terminal_psi::ElementViewRegion::RepresentationBytes {
                start,
                end,
                obligation: crate::terminal_identities::obligation_id(
                    id.get().checked_add(1).ok_or(LoweringError::Unsupported(
                        "representation view obligation identity overflows",
                    ))?,
                ),
            },
        };
        let place = place_id(allocate_dense(self.next_place)?);
        let kind = match loan {
            LoanAccess::Shared => OperationKind::EstablishElementView {
                destination: place,
                source: argument,
                element,
                region,
            },
            LoanAccess::Mutable => OperationKind::EstablishMutableElementView {
                destination: place,
                source: argument,
                element,
                region,
            },
        };
        self.operations.push(Operation {
            static_reach_binding: None,
            suspension_crossing: None,
            id,
            result: OperationResult::Structural(terminal_psi::StructuralOperationResult {
                qualification_establishments: Vec::new(),
                place,
                structural_type: self.structural_type,
                multiplicity,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
                claims: Vec::new(),
            }),
            kind,
        });
        self.temporary_places.push(StructuralPlaceDeclaration {
            id: place,
            kind: StructuralPlaceKind::OperationResult {
                producer: id,
                structural_type: self.structural_type,
            },
        });
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
        let Some(declaration) = self.parameter_declaration(argument)? else {
            return Ok(None);
        };
        if declaration.multiplicity != StructuralMultiplicity::Affine {
            return unsupported("owned selection parameter lacks whole plain-affine custody");
        }
        Ok(Some(declaration.place))
    }

    /// Naming a whole `Unrestricted` parameter reads it: its value is a copy
    /// and the parameter stays usable, so it is never moved. Returns the
    /// parameter's ingress place to copy from.
    fn copied_parameter(
        &self,
        argument: &checked_trees::CheckedUnitStructuralArgumentPlan,
    ) -> Result<Option<StructuralArgument>, LoweringError> {
        Ok(self
            .parameter_declaration(argument)?
            .filter(|declaration| declaration.multiplicity == StructuralMultiplicity::Unrestricted)
            .map(|declaration| StructuralArgument {
                place: declaration.place,
                path: Vec::new(),
                access: StructuralAccess::Owned,
            }))
    }

    /// The signature declaration of the immutable owned parameter a checked
    /// source plan names, with its whole, unqualified custody checked against
    /// the argument; `None` when the plan names no parameter.
    fn parameter_declaration(
        &self,
        argument: &checked_trees::CheckedUnitStructuralArgumentPlan,
    ) -> Result<Option<&StructuralParameterDeclaration>, LoweringError> {
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
            || declaration.structural_type
                != lookup_type_id(self.type_ids, &argument.type_identity)?
            || !declaration.qualifications.is_empty()
            || !declaration.projected_qualifications.is_empty()
        {
            return unsupported("owned selection parameter lacks whole plain owned custody");
        }
        Ok(Some(declaration))
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
                        .find(|local| local.symbol == *symbol)
                        .map(|local| local.argument.place)
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
                let Some(result) = call
                    .operation()
                    .call()
                    .filter(|planned| !planned.is_boundary())
                    .and_then(checked_trees::CheckedUnitCallPlan::structural_result)
                else {
                    return unsupported(
                        "projected selection call drifted from its structural producer",
                    );
                };
                let root_type_identity = result.type_identity.clone();
                let binding_ordinal = result.binding_ordinal;
                let declaration = self.emit_operand_call(call)?;
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
            .transpose()?
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
                        path: lower_structural_path(&residual.path)?,
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
            erased_proof_arguments: Vec::new(),
            structural_arguments: edge.structural_arguments,
            trivial_affine_discards,
            residual_affine_discards,
        });
        Ok(())
    }

    /// A borrowed branch joins one referent place at the result parameter
    /// under its loan. Unlike `complete_value` there is no owner roster, no
    /// displaced candidate and no residual schedule: the referent's owner
    /// stays live, and the single structural parameter must be the result
    /// slot of the loan's access this edge's argument fills. A shared slot is
    /// also `Unrestricted`; an exclusive slot keeps the multiplicity the
    /// binding declares.
    fn complete_loan(
        &mut self,
        argument: StructuralArgument,
        continuation: &ValueContinuation,
        loan: LoanAccess,
    ) -> Result<(), LoweringError> {
        if continuation.structural_parameters.len() != 1
            || !continuation.pass_through.is_empty()
            || !continuation.residuals.is_empty()
        {
            return unsupported(match loan {
                LoanAccess::Shared => "borrowed selection joined residual ownership",
                LoanAccess::Mutable => "mutable borrowed selection joined residual ownership",
            });
        }
        let parameter = &continuation.structural_parameters[0];
        let multiplicity_matches = match loan {
            LoanAccess::Shared => parameter.multiplicity == StructuralMultiplicity::Unrestricted,
            LoanAccess::Mutable => true,
        };
        if parameter.place != continuation.place
            || parameter.access != loan.structural_access()
            || !multiplicity_matches
            || parameter.structural_type != self.structural_type
            || !parameter.qualifications.is_empty()
            || !parameter.projected_qualifications.is_empty()
        {
            return unsupported(match loan {
                LoanAccess::Shared => "borrowed selection parameter changed its custody",
                LoanAccess::Mutable => "mutable borrowed selection parameter changed its custody",
            });
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
            erased_proof_arguments: Vec::new(),
            structural_arguments: edge.structural_arguments,
            trivial_affine_discards: Vec::new(),
            residual_affine_discards: Vec::new(),
        });
        Ok(())
    }

    /// A copyable payloadless case leaf projected out of shared-borrowed
    /// storage: `self.bitness` observes which case the borrowed record holds
    /// through `StructuralCaseMembership`, then establishes that same
    /// constructor fresh under `EstablishScalarCase`. Each non-final case
    /// guards its own block; the closed sum makes the last case unconditional.
    /// Owners join untouched exactly as a scalar dispatch's residual frontier.
    fn projected_case_place(
        &mut self,
        argument: &StructuralArgument,
        continuation: Option<&ValueContinuation>,
    ) -> Result<PlaceId, LoweringError> {
        let leaf_type = self.structural_type;
        let StructuralTypeShape::Sum { cases } = &self
            .structural_types
            .iter()
            .find(|declaration| declaration.id == leaf_type)
            .ok_or(LoweringError::Unsupported(
                "projected case copy lost its leaf type",
            ))?
            .shape
        else {
            return unsupported("projected case copy requires a case-sum leaf");
        };
        if cases.is_empty() || cases.iter().any(|case| !case.fields.is_empty()) {
            return unsupported("projected case copy requires payloadless cases");
        }
        let cases = cases.clone();
        let source = argument.place;
        let path = argument.path.clone();
        let owned_continuation;
        let is_root = continuation.is_none();
        let continuation = if let Some(continuation) = continuation {
            continuation
        } else {
            let join = block_id(allocate_dense(self.next_block)?);
            let joined_values = self
                .values
                .iter()
                .map(|value| {
                    Ok(ValueDeclaration {
                        id: value_id(allocate_dense(self.next_value)?),
                        ..*value
                    })
                })
                .collect::<Result<Vec<_>, LoweringError>>()?;
            let mut structural_parameters = Vec::new();
            let mut remaining_owners = self.owners.clone();
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
                    LoweringError::Unsupported("owned selection parameter count exceeds u32")
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
        let source_count = self.values.len();
        for (position, case) in cases.iter().enumerate() {
            let mut fallback = None;
            if position + 1 < cases.len() {
                let condition = value_id(allocate_dense(self.next_value)?);
                let id = self.operations.allocate();
                self.operations.push(Operation {
                    static_reach_binding: None,
                    suspension_crossing: None,
                    id,
                    result: OperationResult::Scalar(ValueDeclaration {
                        id: condition,
                        scalar_type: ScalarType::Boolean,
                        qualifications: Default::default(),
                    }),
                    kind: OperationKind::StructuralCaseMembership {
                        source,
                        path: path.clone(),
                        case: case.id,
                    },
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
            self.values.truncate(source_count);
            let place = place_id(allocate_dense(self.next_place)?);
            let operation = self.operations.allocate();
            self.operations.push(Operation {
                static_reach_binding: None,
                suspension_crossing: None,
                id: operation,
                result: OperationResult::Structural(terminal_psi::StructuralOperationResult {
                    qualification_establishments: Vec::new(),
                    place,
                    structural_type: leaf_type,
                    multiplicity: self.multiplicity,
                    qualifications: Vec::new(),
                    projected_qualifications: Vec::new(),
                    claims: Vec::new(),
                }),
                kind: OperationKind::EstablishScalarCase {
                    result_case: case.id,
                    fields: Vec::new(),
                },
            });
            self.temporary_places.push(StructuralPlaceDeclaration {
                id: place,
                kind: StructuralPlaceKind::OperationResult {
                    producer: operation,
                    structural_type: leaf_type,
                },
            });
            self.complete_value(place, None, continuation)?;
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
                self.evaluation
                    .selection_cleanups
                    .push(argument_evaluation::SelectionCleanup {
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
                    });
                self.evaluation.structural_value_owners = continuation.remaining_owners.clone();
                self.evaluation.structural_locals.retain(
                    |crate::expression_preparation::bindings::StructuralLocalBinding {
                         argument,
                         ..
                     }| {
                        !self
                            .sources
                            .iter()
                            .any(|source| source.place == argument.place)
                    },
                );
                for crate::expression_preparation::bindings::StructuralLocalBinding {
                    argument,
                    ..
                } in &mut self.evaluation.structural_locals
                {
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

    /// Copy an `Unrestricted` leaf out of a live root into the continuation's
    /// owned place with one `StructuralLeafCopy`: the source subtree stays
    /// fully intact, so shared loans admit the observation where a move
    /// would vacate borrowed storage. Only the op-join path is needed — no
    /// case fan-out, no residual bookkeeping.
    /// Establish a fresh owned record from one view element's scalar fields.
    /// Each retained read is an ordinary element read through the view at the
    /// copy's selector, so each carries the view's own bound; the record then
    /// commits the leaves in declaration order. A field that is bounded,
    /// erased or structural is refused: a plain read carries none of the
    /// evidence establishing it would owe.
    pub(super) fn view_element_copy(
        &mut self,
        reads: &[checked_trees::CheckedScalarExpression],
        continuation: Option<&ValueContinuation>,
    ) -> Result<PlaceId, LoweringError> {
        let shape = self
            .structural_types
            .iter()
            .find(|item| item.id == self.structural_type)
            .ok_or(LoweringError::Unsupported(
                "view element copy structural type missing",
            ))?;
        let StructuralTypeShape::Record { fields } = &shape.shape else {
            return unsupported("view element copy establishes a nonrecord");
        };
        let fields = fields.clone();
        if fields.len() != reads.len() {
            return unsupported("view element copy does not read every field once");
        }
        let bindings = match &self.evaluation.scalar_bindings {
            Some(bindings) => bindings.clone(),
            None => crate::expression_preparation::bindings::ScalarBindings::new(
                self.values.iter().map(|value| value.scalar_type),
            )
            .with_primitive_storage(&self.evaluation.primitive_storage)
            .with_element_views(&self.evaluation.element_views)
            .with_structural_parameters(&self.evaluation.structural_parameters)
            .with_resolved_structural_observations(
                &self.evaluation.structural_fields,
                &self.evaluation.structural_cases,
            ),
        }
        .with_view_locals(&self.evaluation.view_locals);
        let mut initialized = Vec::with_capacity(fields.len());
        for (field, read) in fields.iter().zip(reads) {
            let checked_trees::CheckedScalarExpression::StructuralParameterIndexedRead {
                element_path,
                ..
            } = read
            else {
                return unsupported("view element copy field is not an element read");
            };
            let scalar_type = match field.field_type {
                terminal_psi::StructuralFieldType::Scalar(scalar_type) => scalar_type,
                terminal_psi::StructuralFieldType::IeeeFloat(format) => {
                    semantic_vocabulary::ScalarType::IeeeFloat(format)
                }
                _ => {
                    return unsupported("view element copy reads only plain scalar fields");
                }
            };
            if field.relevance.is_erased()
                || element_path.as_slice()
                    != [checked_trees::CheckedStructuralPredicatePathSegment::Field(
                        field.identity.clone(),
                    )]
            {
                return unsupported("view element copy reordered or renamed a field");
            }
            let lowered = bindings.expression(read)?;
            if lowered.scalar_type() != scalar_type {
                return unsupported("view element copy read changed its field carrier");
            }
            let value = crate::emission::operation_emission::expressions::emit_direct_expression(
                &lowered,
                self.values,
                self.next_value,
                self.operations,
            );
            initialized.push(terminal_psi::RecordFieldInitializer {
                field: field.id,
                value: terminal_psi::RecordFieldValue::Scalar {
                    value,
                    range_obligation: None,
                },
            });
        }
        let place = place_id(allocate_dense(self.next_place)?);
        self.temporary_places.push(super::record::emit_completed(
            place,
            self.structural_type,
            self.multiplicity,
            initialized,
            self.operations,
        ));
        let Some(continuation) = continuation else {
            return Ok(place);
        };
        self.complete_value(place, None, continuation)?;
        Ok(continuation.place)
    }

    fn copied_leaf_place(
        &mut self,
        argument: &StructuralArgument,
        continuation: Option<&ValueContinuation>,
    ) -> Result<PlaceId, LoweringError> {
        let path = self.copied_place_path(argument)?;
        // Snapshot whole byte operands before later record members execute.
        // Copy the source's exact established roster, not the destination's
        // requested predicates; the independent copy checker rejoins both.
        let qualifications = if argument.path.is_empty()
            && self.structural_types.iter().any(|declaration| {
                declaration.id == self.structural_type
                    && matches!(
                        declaration.shape,
                        StructuralTypeShape::ByteSequence(
                            terminal_psi::ByteSequenceCarrier::BoundedOwned { .. }
                        )
                    )
            }) {
            self.evaluation
                .structural_parameters
                .iter()
                .map(|(_, parameter)| parameter)
                .chain(self.evaluation.block_structural_parameters.iter())
                .find(|parameter| parameter.place == argument.place)
                .map(|parameter| parameter.qualifications.clone())
                .or_else(|| {
                    self.operations
                        .iter()
                        .filter_map(|operation| operation.result.structural())
                        .find(|result| result.place == argument.place)
                        .map(|result| result.qualifications.clone())
                })
                .ok_or(LoweringError::Unsupported(
                    "copied byte operand has no established source contract",
                ))?
        } else {
            Vec::new()
        };
        let place = place_id(allocate_dense(self.next_place)?);
        let operation = self.operations.allocate();
        self.operations.push(Operation {
            static_reach_binding: None,
            suspension_crossing: None,
            id: operation,
            result: OperationResult::Structural(terminal_psi::StructuralOperationResult {
                qualification_establishments: Vec::new(),
                place,
                structural_type: self.structural_type,
                multiplicity: self.multiplicity,
                qualifications,
                projected_qualifications: Vec::new(),
                claims: Vec::new(),
            }),
            kind: OperationKind::StructuralLeafCopy {
                source: argument.place,
                path,
            },
        });
        self.temporary_places.push(StructuralPlaceDeclaration {
            id: place,
            kind: StructuralPlaceKind::OperationResult {
                producer: operation,
                structural_type: self.structural_type,
            },
        });
        let Some(continuation) = continuation else {
            // At the value root the fresh place publishes directly — the copy
            // consumes nothing, so there is no custody edge to join.
            return Ok(place);
        };
        self.complete_value(place, None, continuation)?;
        Ok(continuation.place)
    }

    fn rebind_local_cases(&mut self) -> Result<(), LoweringError> {
        let (_, authored) = crate::expression_preparation::source_custody::authored_state(
            self.checked,
            self.state,
        )?;
        let mut cases = Vec::new();
        for crate::expression_preparation::bindings::StructuralLocalBinding {
            symbol,
            argument,
            ..
        } in &self.evaluation.structural_locals
        {
            let Some((_, local)) = self.checked.state_local_declaration(authored, *symbol) else {
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
            erased_proof_arguments: Vec::new(),
            trivial_affine_discards: Vec::new(),
        })
    }

    fn finish(&mut self, terminator: Terminator) {
        self.evaluation.blocks.push(Block {
            id: self.evaluation.current,
            parameters: std::mem::take(&mut self.evaluation.parameters),
            erased_scalar_formals: Vec::new(),
            erased_proof_formals: Vec::new(),
            structural_parameters: std::mem::take(&mut self.evaluation.block_structural_parameters),
            operations: self.operations[self.evaluation.operation_start..].to_vec(),
            terminator,
        });
    }

    fn start(&mut self, block: BlockId) {
        self.evaluation.current = block;
        self.evaluation.operation_start = self.operations.len();
        self.operations.byte_lengths.clear();
        self.operations.field_byte_lengths.clear();
    }
}
