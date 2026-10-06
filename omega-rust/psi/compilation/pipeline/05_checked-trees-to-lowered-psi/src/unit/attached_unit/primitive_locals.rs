//! Initialized primitive referents retain authored identity apart from SSA locals.
//!
//! A mutable primitive local is established once, at its authored `let`, from
//! the `StorageInitializer` value checking retained: a pure expression, or the
//! computation rooted there when the initializer evaluates a call or an ordered
//! selection. `OperationFrame::establish_primitive_local` is the one emitter the
//! ordinary machine and a composed state both use. It replays the initializer's
//! exact source custody, evaluates it on the ordinary operand schedule, and
//! publishes the new place in the evaluation's primitive storage, so later
//! reads, stores and borrows name the place rather than the initializer value.
//! Each route keeps the returned referent in its own roster and declares its
//! place with the rest of the body's places.
use super::operation_frame::{OperationFrame, StructuralTypeRoster};
use super::{
    CheckedScalarExpressionRole, CheckedTrees, CheckedUnitEffectOperationPlan, LoweringError,
    Operation, OperationKind, StructuralMultiplicity, StructuralPlaceDeclaration,
    StructuralPlaceKind, StructuralTypeShape, allocate_dense, lookup_type_id, place_id,
    terminal_scalar_type, unsupported,
};
use crate::emission::operation_emission::buffer::structural_result_declaration;
use crate::unit::StructuralTypeDeclaration;
use checked_trees::CheckedCallScalarArgument;
use checked_trees::statement::{StatementNode, TableLocalData};

pub(super) mod unit_calls;

pub(crate) struct PrimitiveLocal {
    pub symbol: symbols::SymbolHandle,
    pub declaration: StructuralPlaceDeclaration,
}

/// The one authored mutable primitive declaration of `symbol` in `state`
/// before statement `before`.
fn declaration(
    checked: &CheckedTrees,
    state: symbols::SymbolHandle,
    symbol: symbols::SymbolHandle,
    before: u32,
) -> Result<&TableLocalData, LoweringError> {
    let (_, state) = crate::declarations::state_declaration(checked, state)?;
    let mut matches = checked
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .enumerate()
        .filter_map(|(ordinal, statement)| match statement {
            StatementNode::LocalData(local)
                if local.symbol == symbol && ordinal < before as usize =>
            {
                Some(local)
            }
            _ => None,
        });
    let local = matches.next().ok_or(LoweringError::Unsupported(
        "primitive local has no preceding authored declaration",
    ))?;
    if matches.next().is_some()
        || !symbol.is_valid()
        || !local.is_mutable
        || !(matches!(
            checked
                .type_reference_table
                .type_reference(local.type_reference),
            checked_trees::types::TypeReferenceNode::Named { .. }
        ) || validation::is_arithmetic_policy_only_integer(
            &checked.typed,
            local.type_reference,
        ))
        || checked
            .primitive_type_reference(local.type_reference)
            .is_none()
    {
        return unsupported("primitive local does not have exact mutable primitive custody");
    }
    Ok(local)
}

/// Rejoin one establishment to its authored declaration, declared type and
/// exact retained initializer: the bound pure expression, or the computation
/// rooted at the declaration's `StorageInitializer` coordinate together with
/// the source custody of every call that computation performs. The plan's type
/// identity names storage, so arithmetic policy remains on the authored type
/// and checked computations, not on the primitive storage carrier. Declaration
/// validation rejects every other qualification before this projection.
fn validate_establishment(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    state: symbols::SymbolHandle,
    operation: &CheckedUnitEffectOperationPlan,
) -> Result<(), LoweringError> {
    let CheckedUnitEffectOperationPlan::EstablishPrimitiveLocal {
        statement_index,
        symbol,
        type_identity: _,
        primitive_type,
        value,
    } = operation
    else {
        return unsupported("primitive local establishment received another operation");
    };
    let next = statement_index
        .checked_add(1)
        .ok_or(LoweringError::Unsupported(
            "primitive local statement ordinal exhausted",
        ))?;
    let local = declaration(checked, state, *symbol, next)?;
    let (_, _) = crate::declarations::state_declaration(checked, state)?;
    let role = CheckedScalarExpressionRole::StorageInitializer;
    match value {
        CheckedCallScalarArgument::Pure(_) => {
            let (_, _) = checked
                .facts
                .values
                .scalar_expressions
                .bound_expression_at(state, *statement_index, role)
                .ok_or(LoweringError::Unsupported(
                    "primitive local lost its exact initializer",
                ))?;
        }
        CheckedCallScalarArgument::Computation(handle) => {
            let computations = &checked.facts.values.scalar_computations;
            let mut roots = computations
                .roots
                .iter()
                .map(|(_, root)| root)
                .filter(|root| {
                    root.state == state
                        && root.statement_ordinal == *statement_index
                        && root.role == role
                });
            let root = roots.next().ok_or(LoweringError::Unsupported(
                "primitive local lost its initializer computation",
            ))?;
            if roots.next().is_some()
                || root.machine != machine
                || root.root != *handle
                || !computations.nodes.is_valid(*handle)
                || computations.nodes.get(*handle).authored_root != local.initial_value
                || computations.nodes.get(*handle).primitive_type != *primitive_type
                || checked
                    .facts
                    .values
                    .scalar_expressions
                    .expression_at(state, *statement_index, role)
                    .is_some()
            {
                return unsupported("primitive local substituted its initializer computation");
            }
        }
    }
    Ok(())
}

pub(super) fn find(
    locals: &[PrimitiveLocal],
    symbol: symbols::SymbolHandle,
) -> Result<&PrimitiveLocal, LoweringError> {
    let mut matching = locals.iter().filter(|local| local.symbol == symbol);
    let local = matching.next().ok_or(LoweringError::Unsupported(
        "primitive local has not been established",
    ))?;
    if matching.next().is_some() {
        return unsupported("primitive local establishment is duplicated");
    }
    Ok(local)
}

impl OperationFrame<'_, '_> {
    /// Establish one mutable primitive local at its authored declaration and
    /// publish its place in the evaluation's primitive storage. The route
    /// keeps the returned referent in its own roster and declares its place.
    pub(super) fn establish_primitive_local(
        self,
        operation: &CheckedUnitEffectOperationPlan,
    ) -> Result<PrimitiveLocal, LoweringError> {
        let CheckedUnitEffectOperationPlan::EstablishPrimitiveLocal {
            statement_index,
            symbol,
            type_identity,
            primitive_type,
            value,
        } = operation
        else {
            return unsupported("primitive local establishment received another operation");
        };
        if self
            .primitive_locals
            .iter()
            .any(|local| local.symbol == *symbol)
            || self
                .evaluation
                .primitive_storage
                .iter()
                .any(|(storage, _, _)| storage == symbol)
        {
            return unsupported("primitive local is established more than once");
        }
        validate_establishment(self.checked, self.machine, self.state, operation)?;
        // The initializer completes on the ordinary operand schedule: a
        // computation evaluates its calls and selections in authored order
        // before the storage exists, so it can never read its own local.
        let initializer = self.evaluation.source_value(
            self.checked,
            self.machine,
            self.state,
            *statement_index,
            CheckedScalarExpressionRole::StorageInitializer,
            value,
            self.source_value_count,
            self.values,
            self.next_value,
            self.next_block,
            self.next_edge,
            self.operations,
            self.calls,
        )?;
        let structural_type = lookup_type_id(self.type_ids, type_identity)?;
        let declarations: &[StructuralTypeDeclaration] = match &self.structural_types {
            StructuralTypeRoster::Owned(types) => types,
            StructuralTypeRoster::Published(types) => types,
        };
        if initializer.scalar_type != terminal_scalar_type(*primitive_type)?
            || !declarations.iter().any(|declaration| {
                declaration.id == structural_type
                    && declaration.shape
                        == StructuralTypeShape::PrimitiveScalar(initializer.scalar_type)
            })
        {
            return unsupported("primitive local initializer and referent types disagree");
        }
        let producer = self.operations.allocate();
        let place = place_id(allocate_dense(self.next_place)?);
        self.operations.push(Operation {
            static_reach_binding: None,
            suspension_crossing: None,
            id: producer,
            result: structural_result_declaration(
                place,
                structural_type,
                StructuralMultiplicity::Unrestricted,
            ),
            kind: OperationKind::EstablishPrimitiveLocal {
                value: initializer.id,
            },
        });
        self.evaluation
            .primitive_storage
            .push((*symbol, place, initializer.scalar_type));
        Ok(PrimitiveLocal {
            symbol: *symbol,
            declaration: StructuralPlaceDeclaration {
                id: place,
                kind: StructuralPlaceKind::OperationResult {
                    producer,
                    structural_type,
                },
            },
        })
    }
}

impl super::argument_evaluation::Evaluation {
    /// Immutable scalar locals are values until an address is observed. Rejoin
    /// their authored ordinal to the completed SSA value, then give that value
    /// one shared home using the same operation as addressable scalar ingress.
    #[allow(clippy::too_many_arguments)]
    pub(in crate::unit::attached_unit) fn immutable_local_borrow_home(
        &mut self,
        checked: &CheckedTrees,
        state_symbol: symbols::SymbolHandle,
        root: Option<facts::PlaceRoot>,
        values: &[terminal_psi::ValueDeclaration],
        structural_types: &[StructuralTypeDeclaration],
        next_place: &mut u64,
        operations: &mut crate::emission::operation_emission::buffer::OperationBuffer,
        places: &mut Vec<StructuralPlaceDeclaration>,
    ) -> Result<
        Option<crate::expression_preparation::bindings::StructuralLocalBinding>,
        LoweringError,
    > {
        let Some(facts::PlaceRoot::Symbol(symbol)) = root else {
            return Ok(None);
        };
        let (_, state) = crate::declarations::state_declaration(checked, state_symbol)?;
        let Some((statement, local)) = checked.typed.state_local_declaration(state, symbol) else {
            return Ok(None);
        };
        let Some(primitive) = checked.primitive_type_reference(local.type_reference) else {
            return Ok(None);
        };
        if local.is_mutable {
            return Ok(None);
        }
        let scalar_type = crate::emission::scalar_types::terminal_scalar_type(primitive)?;
        let ordinal = crate::expression_preparation::source_custody::source_local_ordinal(
            checked,
            state_symbol,
            u32::try_from(statement).map_err(|_| {
                LoweringError::Unsupported("primitive local statement ordinal overflow")
            })?,
        )?;
        let bindings = self
            .scalar_bindings
            .as_ref()
            .ok_or(LoweringError::Unsupported(
                "immutable primitive borrow has no completed scalar namespace",
            ))?;
        let position = bindings.immutable_local_value(ordinal as usize, scalar_type)?;
        let value = values.get(position).ok_or(LoweringError::Unsupported(
            "immutable primitive borrow lost its completed scalar value",
        ))?;
        if value.scalar_type != scalar_type {
            return unsupported("immutable primitive borrow changes its completed scalar type");
        }
        let mut carriers = structural_types.iter().filter(|declaration| {
            declaration.shape == StructuralTypeShape::PrimitiveScalar(scalar_type)
        });
        let structural_type = carriers
            .next()
            .ok_or(LoweringError::Unsupported(
                "immutable primitive home has no storage carrier",
            ))?
            .id;
        if carriers.next().is_some() {
            return unsupported("immutable primitive home has ambiguous storage carriers");
        }
        let place = self.immutable_primitive_home(
            symbol,
            value,
            structural_type,
            structural_types,
            next_place,
            operations,
            places,
        )?;
        Ok(Some(
            crate::expression_preparation::bindings::StructuralLocalBinding {
                result_binding_ordinal: None,
                root: facts::PlaceRoot::Symbol(symbol),
                structural_type,
                argument: terminal_psi::StructuralArgument {
                    place,
                    path: Vec::new(),
                    access: terminal_psi::StructuralAccess::SharedBorrow,
                },
            },
        ))
    }

    /// One immutable scalar value receives one original addressable home.
    /// Callers authenticate its source declaration and already-evaluated value;
    /// all later loans reuse this home, independent of their observation type.
    #[allow(clippy::too_many_arguments)]
    pub(in crate::unit::attached_unit) fn immutable_primitive_home(
        &mut self,
        symbol: symbols::SymbolHandle,
        value: &terminal_psi::ValueDeclaration,
        structural_type: semantic_vocabulary::StructuralTypeId,
        declarations: &[StructuralTypeDeclaration],
        next_place: &mut u64,
        operations: &mut crate::emission::operation_emission::buffer::OperationBuffer,
        places: &mut Vec<StructuralPlaceDeclaration>,
    ) -> Result<semantic_vocabulary::PlaceId, LoweringError> {
        let mut types = declarations.iter().filter(|row| row.id == structural_type);
        if !symbol.is_valid()
            || types.next().map(|row| &row.shape)
                != Some(&StructuralTypeShape::PrimitiveScalar(value.scalar_type))
            || types.next().is_some()
        {
            return unsupported("primitive value home lost its exact storage declaration");
        }
        let mut homes = self.primitive_storage.iter().filter(|row| row.0 == symbol);
        if let Some((_, place, scalar_type)) = homes.next() {
            if homes.next().is_some() || *scalar_type != value.scalar_type {
                return unsupported("primitive value has ambiguous original storage");
            }
            return Ok(*place);
        }
        let producer = operations.allocate();
        let place = place_id(allocate_dense(next_place)?);
        operations.push(Operation {
            static_reach_binding: None,
            suspension_crossing: None,
            id: producer,
            result: structural_result_declaration(
                place,
                structural_type,
                StructuralMultiplicity::Unrestricted,
            ),
            kind: OperationKind::EstablishPrimitiveLocal { value: value.id },
        });
        places.push(StructuralPlaceDeclaration {
            id: place,
            kind: StructuralPlaceKind::OperationResult {
                producer,
                structural_type,
            },
        });
        self.primitive_storage
            .push((symbol, place, value.scalar_type));
        Ok(place)
    }
}
