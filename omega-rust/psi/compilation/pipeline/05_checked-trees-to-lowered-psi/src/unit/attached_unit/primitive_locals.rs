//! Initialized primitive referents retain authored identity apart from SSA locals.
//!
//! A mutable primitive local is established once, at its authored `let`, from
//! the `StorageInitializer` value checking retained: a pure expression, or the
//! computation rooted there when the initializer evaluates a call or an ordered
//! selection. `OperationFrame::establish_primitive_local` is the one emitter the
//! ordinary machine and a composed state both use. It replays the initializer's
//! exact source custody, evaluates it on the ordinary operand schedule, and
//! publishes the new place in the evaluation's shared structural storage, so later
//! reads, stores and borrows name the place rather than the initializer value.
//! Each route keeps the returned referent in its own roster and declares its
//! place with the rest of the body's places. A `mut` scalar formal of an
//! ordinary body or a composed state is the same storage, established before
//! the first statement from the arriving value (`establish_parameter_storage`);
//! checking reads such a formal only as `StorageRead` of its symbol.
//! An ordinary value binding keeps its evaluated SSA value until its first
//! address is observed. That operation publishes one owned structural home
//! and redirects reads of this binding to it. Copies made before the address
//! was observed retain their own values. Later loans reuse the home, never
//! replay the initializer, and owe the same access and lifetime checks as
//! every other owned place. `let mut` only selects declaration-time storage
//! and replacement permission; it does not confer borrow authority.
//! Before lending a local, publish its exact home in the shared structural
//! binding roster. Mutable locals reuse declaration-time storage; immutable
//! values acquire storage once. Constructor and call consumers must not need
//! separate source-shape routes to find those same homes.
use super::operation_frame::{OperationFrame, StructuralTypeRoster};
use super::{
    CheckedScalarExpressionRole, CheckedTrees, CheckedUnitEffectOperationPlan, LoweringError,
    Operation, OperationKind, StructuralMultiplicity, StructuralPlaceDeclaration,
    StructuralPlaceKind, StructuralTypeShape, allocate_dense, lookup_type_id, place_id,
    terminal_scalar_type, unsupported,
};
use crate::emission::operation_emission::buffer::structural_result_declaration;
use crate::unit::StructuralTypeDeclaration;

#[derive(Clone, Copy)]
pub(crate) struct PrimitiveLocal {
    pub symbol: symbols::SymbolHandle,
    pub declaration: StructuralPlaceDeclaration,
}

/// One `mut` scalar formal's storage, established before the first statement.
pub(super) struct ParameterStorage {
    /// The formal's dense scalar position, which its borrows name.
    pub(super) position: usize,
    pub(super) local: PrimitiveLocal,
    pub(super) scalar_type: semantic_vocabulary::ScalarType,
}

/// Publish every `mut` formal in the common structural home roster and register
/// its scalar signature position for loans. It never enters the primitive
/// referent table: that table is reserved for borrowed ingress. Publishing one
/// owned home in both rosters would make current-content reads ambiguous.
pub(super) fn register_mut_formal_storage_homes(
    evaluation: &mut super::argument_evaluation::Evaluation,
    parameter_storage: &[ParameterStorage],
) -> Result<(), LoweringError> {
    for storage in parameter_storage {
        evaluation.scalar_bindings.register_scalar_parameter_home(
            u32::try_from(storage.position).map_err(|_| {
                LoweringError::Unsupported("parameter storage position exceeds u32")
            })?,
            storage.local.declaration.id,
            storage.scalar_type,
            terminal_psi::StructuralAccess::MutableBorrow,
        )?;
        let StructuralPlaceKind::OperationResult {
            structural_type, ..
        } = storage.local.declaration.kind
        else {
            return unsupported("mutable formal storage has no established declaration");
        };
        let root = facts::PlaceRoot::Symbol(storage.local.symbol);
        if evaluation
            .structural_locals
            .iter()
            .any(|home| home.root == root)
        {
            return unsupported("mutable formal has duplicate original storage homes");
        }
        // Stores, projected consumers and loans resolve the established home,
        // including its exact restricted declaration. The scalar binding only
        // supplies its dense signature position; it is not another storage type.
        evaluation.structural_locals.push(
            crate::expression_preparation::bindings::StructuralLocalBinding {
                result_binding_ordinal: None,
                root,
                structural_type,
                argument: terminal_psi::StructuralArgument {
                    place: storage.local.declaration.id,
                    path: Vec::new(),
                    access: terminal_psi::StructuralAccess::Owned,
                },
            },
        );
    }
    Ok(())
}

/// Retain the exact declared receiving restriction as an obligation over the
/// initializer's completed SSA value. A declaration supplies the goal, never
/// the fact that initialization satisfied it. Record fields and primitive homes
/// use the same scalar receiving predicate; no source proof is replayed here.
pub(crate) fn initialization_obligation(
    declarations: &[StructuralTypeDeclaration],
    structural_type: semantic_vocabulary::StructuralTypeId,
    next_obligation: &mut u64,
) -> Result<Option<semantic_vocabulary::ObligationId>, LoweringError> {
    let mut matching = declarations.iter().filter(|row| row.id == structural_type);
    let declaration = matching.next().ok_or(LoweringError::Unsupported(
        "primitive storage has no receiving declaration",
    ))?;
    if matching.next().is_some() {
        return unsupported("primitive storage receiving declaration is ambiguous");
    }
    match &declaration.scalar_restriction {
        None => Ok(None),
        Some(terminal_psi::StructuralFieldType::BoundedInteger(_)) => Ok(Some(
            crate::terminal_identities::obligation_id(allocate_dense(next_obligation)?),
        )),
        _ => unsupported("primitive storage has no supported initialization restriction"),
    }
}

/// Mint the establishing operation and its declared place for one unrestricted
/// primitive local: the producing operation and its `OperationResult` place
/// are minted together so the operation roster and the storage roster agree.
pub(super) fn push_established_local(
    next_place: &mut u64,
    operations: &mut crate::emission::operation_emission::buffer::OperationBuffer,
    value: semantic_vocabulary::ValueId,
    structural_type: semantic_vocabulary::StructuralTypeId,
    declarations: &[StructuralTypeDeclaration],
    range_obligation: Option<semantic_vocabulary::ObligationId>,
) -> Result<(semantic_vocabulary::PlaceId, StructuralPlaceDeclaration), LoweringError> {
    let place = place_id(allocate_dense(next_place)?);
    let declaration = establish_at(
        place,
        operations,
        value,
        structural_type,
        declarations,
        range_obligation,
    )?;
    Ok((place, declaration))
}

/// Establish a completed scalar in a reserved home. Locals and aggregate
/// children owe the same receiving restriction on the actual initialized value;
/// reserving the home before a continuation does not establish its contents.
pub(crate) fn establish_at(
    place: semantic_vocabulary::PlaceId,
    operations: &mut crate::emission::operation_emission::buffer::OperationBuffer,
    value: semantic_vocabulary::ValueId,
    structural_type: semantic_vocabulary::StructuralTypeId,
    declarations: &[StructuralTypeDeclaration],
    range_obligation: Option<semantic_vocabulary::ObligationId>,
) -> Result<StructuralPlaceDeclaration, LoweringError> {
    let mut matching = declarations.iter().filter(|row| row.id == structural_type);
    let declaration = matching.next().ok_or(LoweringError::Unsupported(
        "primitive home has no declared receiving type",
    ))?;
    if matching.next().is_some()
        || !matches!(declaration.shape, StructuralTypeShape::PrimitiveScalar(_))
        || declaration.scalar_restriction.is_some() != range_obligation.is_some()
    {
        return unsupported("primitive home lost its receiving restriction obligation");
    }
    let producer = operations.allocate();
    operations.push(Operation {
        static_reach_binding: None,
        suspension_crossing: None,
        id: producer,
        result: structural_result_declaration(
            place,
            structural_type,
            StructuralMultiplicity::Unrestricted,
        ),
        kind: OperationKind::EstablishPrimitiveLocal {
            value,
            range_obligation,
        },
    });
    Ok(StructuralPlaceDeclaration {
        id: place,
        kind: StructuralPlaceKind::OperationResult {
            producer,
            structural_type,
        },
    })
}

/// Establish each mutable scalar formal from its exact incoming value before
/// the first statement. Later reads, stores and loans name the same place;
/// the incoming snapshot is never read as the current stored contents.
pub(super) fn establish_parameter_storage(
    storage: &[checked_trees::CheckedUnitParameterStoragePlan],
    scalar_parameters: &[checked_trees::CheckedStructuralScalarParameterPlan],
    values: &[terminal_psi::ValueDeclaration],
    type_ids: &[(
        typed_trees::type_identity::TypeIdentityId,
        semantic_vocabulary::StructuralTypeId,
    )],
    declarations: &[StructuralTypeDeclaration],
    next_place: &mut u64,
    next_obligation: &mut u64,
    operations: &mut crate::emission::operation_emission::buffer::OperationBuffer,
) -> Result<Vec<ParameterStorage>, LoweringError> {
    storage
        .iter()
        .map(|storage| {
            let position = scalar_parameters
                .iter()
                .position(|parameter| parameter.source_position == storage.source_position)
                .ok_or(LoweringError::Unsupported(
                    "parameter storage names no scalar formal",
                ))?;
            let value = values.get(position).ok_or(LoweringError::Unsupported(
                "parameter storage has no incoming value",
            ))?;
            let scalar_type = terminal_scalar_type(storage.primitive_type)?;
            let structural_type = lookup_type_id(type_ids, &storage.type_identity)?;
            if !storage.symbol.is_valid()
                || value.scalar_type != scalar_type
                || !declarations.iter().any(|declaration| {
                    declaration.id == structural_type
                        && declaration.shape == StructuralTypeShape::PrimitiveScalar(scalar_type)
                })
            {
                return unsupported("parameter storage and its incoming value disagree");
            }
            let range_obligation =
                initialization_obligation(declarations, structural_type, next_obligation)?;
            let (_, declaration) = push_established_local(
                next_place,
                operations,
                value.id,
                structural_type,
                declarations,
                range_obligation,
            )?;
            Ok(ParameterStorage {
                position,
                local: PrimitiveLocal {
                    symbol: storage.symbol,
                    declaration,
                },
                scalar_type,
            })
        })
        .collect()
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
    /// publish its place in the evaluation's shared structural storage. The route
    /// keeps the returned referent in its own roster and declares its place.
    pub(super) fn establish_primitive_local(
        self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        operation: &CheckedUnitEffectOperationPlan,
    ) -> Result<PrimitiveLocal, LoweringError> {
        let CheckedUnitEffectOperationPlan::EstablishLocal(
            checked_trees::CheckedUnitLocalBindingPlan::Primitive {
                statement_index,
                symbol,
                type_identity,
                primitive_type,
                value,
            },
        ) = operation
        else {
            return unsupported("primitive local establishment received another operation");
        };
        if self
            .primitive_locals
            .iter()
            .any(|local| local.symbol == *symbol)
            || self
                .evaluation
                .structural_locals
                .iter()
                .any(|local| local.root == facts::PlaceRoot::Symbol(*symbol))
            || self
                .evaluation
                .primitive_storage
                .iter()
                .any(|(storage, _, _)| storage == symbol)
        {
            return unsupported("primitive local is established more than once");
        }
        // The initializer completes on the ordinary operand schedule: a
        // computation evaluates its calls and selections in authored order
        // before the storage exists, so it can never read its own local.
        let initializer = self.evaluation.source_value(
            identities,
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
        let range_obligation = initialization_obligation(
            declarations,
            structural_type,
            &mut self.calls.next_obligation_identity,
        )?;
        let (place, declaration) = push_established_local(
            self.next_place,
            self.operations,
            initializer.id,
            structural_type,
            declarations,
            range_obligation,
        )?;
        // Publish the original home once, rather than copying its initializer
        // or leaving constructor consumers with a private primitive index.
        self.evaluation.structural_locals.push(
            crate::expression_preparation::bindings::StructuralLocalBinding {
                result_binding_ordinal: None,
                root: facts::PlaceRoot::Symbol(*symbol),
                structural_type,
                argument: terminal_psi::StructuralArgument {
                    place,
                    path: Vec::new(),
                    access: terminal_psi::StructuralAccess::Owned,
                },
            },
        );
        Ok(PrimitiveLocal {
            symbol: *symbol,
            declaration,
        })
    }
}

impl super::argument_evaluation::Evaluation {
    /// Publish original backing demanded by a checked borrowed operand. The
    /// operand remains responsible for its loan; this only makes owned scalar
    /// storage addressable on the caller's completed-value frontier.
    #[allow(clippy::too_many_arguments)]
    pub(in crate::unit::attached_unit) fn prepare_borrowed_argument(
        &mut self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        checked: &CheckedTrees,
        state: symbols::SymbolHandle,
        source: &checked_trees::CheckedUnitStructuralArgumentPlan,
        values: &[terminal_psi::ValueDeclaration],
        types: &[StructuralTypeDeclaration],
        next_place: &mut u64,
        operations: &mut crate::emission::operation_emission::buffer::OperationBuffer,
        places: &mut Vec<StructuralPlaceDeclaration>,
    ) -> Result<(), LoweringError> {
        if let checked_trees::CheckedUnitStructuralArgumentSourcePlan::ScalarParameter {
            parameter_index,
        } = source.source
        {
            let (_, authored) = crate::declarations::state_declaration(checked, state)?;
            let parameter = checked
                .state_parameters(authored)
                .iter()
                .filter(|parameter| {
                    !parameter.is_const
                        && !parameter.relevance.is_erased()
                        && checked
                            .primitive_type_reference(parameter.type_reference)
                            .is_some()
                })
                .nth(parameter_index as usize)
                .ok_or(LoweringError::Unsupported(
                    "primitive reference ingress is absent",
                ))?;
            let scalar_type = terminal_scalar_type(
                checked
                    .primitive_type_reference(parameter.type_reference)
                    .ok_or(LoweringError::Unsupported(
                        "primitive reference ingress lost its carrier",
                    ))?,
            )?;
            let position = self
                .scalar_bindings
                .immutable_position(parameter_index as usize)?;
            let value = values.get(position).ok_or(LoweringError::Unsupported(
                "primitive ingress value is absent",
            ))?;
            let identity = checked
                .typed
                .normalized_type_identity_text(parameter.type_reference);
            let mut carriers = types
                .iter()
                .filter(|actual| actual.identity == identities.intern(&identity));
            let carrier = carriers.next().ok_or(LoweringError::Unsupported(
                "primitive ingress has no original storage carrier",
            ))?;
            if carriers.next().is_some()
                || carrier.shape != StructuralTypeShape::PrimitiveScalar(scalar_type)
                || value.scalar_type != scalar_type
                || !value.qualifications.is_empty()
            {
                return unsupported("primitive ingress value and storage carrier disagree");
            }
            let place = if let Some(place) = self
                .scalar_bindings
                .scalar_parameter_home(parameter_index, scalar_type)?
            {
                place
            } else if parameter.is_mutable {
                // A `mut` formal is read through its storage, which the body
                // established at entry. A home seeded here from the incoming
                // value would miss every assignment before this borrow.
                return unsupported("borrowed `mut` scalar formal has no storage");
            } else {
                self.immutable_primitive_home(
                    parameter.symbol,
                    value,
                    carrier.id,
                    types,
                    next_place,
                    operations,
                    places,
                )?
            };
            self.scalar_bindings.register_scalar_parameter_home(
                parameter_index,
                place,
                scalar_type,
                if parameter.is_mutable {
                    terminal_psi::StructuralAccess::MutableBorrow
                } else {
                    terminal_psi::StructuralAccess::SharedBorrow
                },
            )?;
        } else {
            self.prepare_immutable_local_borrow(
                checked,
                state,
                source.structural_storage_root(),
                values,
                types,
                next_place,
                operations,
                places,
            )?;
        }
        Ok(())
    }
    /// Immutable scalar locals are values until an address is observed. Rejoin
    /// their authored ordinal to the completed SSA value, then publish that
    /// value's one owned home in the ordinary storage roster. Reference
    /// formation, observations and call operands all use this same join;
    /// a temporary binding available only to reference construction loses
    /// custody when a call borrows the scalar directly.
    #[allow(clippy::too_many_arguments)]
    pub(in crate::unit::attached_unit) fn prepare_immutable_local_borrow(
        &mut self,
        checked: &CheckedTrees,
        state_symbol: symbols::SymbolHandle,
        root: Option<facts::PlaceRoot>,
        values: &[terminal_psi::ValueDeclaration],
        structural_types: &[StructuralTypeDeclaration],
        next_place: &mut u64,
        operations: &mut crate::emission::operation_emission::buffer::OperationBuffer,
        places: &mut Vec<StructuralPlaceDeclaration>,
    ) -> Result<bool, LoweringError> {
        let Some(facts::PlaceRoot::Symbol(symbol)) = root else {
            return Ok(false);
        };
        let locals = &checked.facts.state_locals;
        let Some(local) = locals.named(state_symbol, symbol) else {
            return Ok(false);
        };
        let Some(primitive) = checked.primitive_type_reference(local.declared_type) else {
            return Ok(false);
        };
        if local.is_mutable {
            return Ok(false);
        }
        let scalar_type = crate::emission::scalar_types::terminal_scalar_type(primitive)?;
        let ordinal = locals.scalar_locals_before(state_symbol, local.statement_index);
        let bindings = &self.scalar_bindings;
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
        let home = crate::expression_preparation::bindings::StructuralLocalBinding {
            result_binding_ordinal: None,
            root: facts::PlaceRoot::Symbol(symbol),
            structural_type,
            argument: terminal_psi::StructuralArgument {
                place,
                path: Vec::new(),
                access: terminal_psi::StructuralAccess::Owned,
            },
        };
        let mut established = self
            .structural_locals
            .iter()
            .filter(|binding| binding.root == home.root);
        if let Some(binding) = established.next() {
            if established.next().is_some()
                || binding.result_binding_ordinal.is_some()
                || binding.structural_type != home.structural_type
                || binding.argument != home.argument
            {
                return unsupported("immutable primitive home changes its established custody");
            }
        } else {
            self.structural_locals.push(home);
        }
        // This home now belongs to the common structural roster, not two
        // competing storage indexes. Later scalar reads follow its authored
        // binding to the original home; separately copied values stay snapshots.
        self.primitive_storage.retain(|row| row.0 != symbol);
        self.scalar_bindings
            .register_local_storage(ordinal as usize, symbol, scalar_type)?;
        Ok(true)
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
        let mut homes = self
            .structural_locals
            .iter()
            .filter(|row| row.root == facts::PlaceRoot::Symbol(symbol));
        if let Some(home) = homes.next() {
            if homes.next().is_some()
                || home.structural_type != structural_type
                || !home.argument.path.is_empty()
                || home.argument.access != terminal_psi::StructuralAccess::Owned
            {
                return unsupported("primitive value changes its original storage custody");
            }
            return Ok(home.argument.place);
        }
        let (place, declaration) = push_established_local(
            next_place,
            operations,
            value.id,
            structural_type,
            declarations,
            None,
        )?;
        places.push(declaration);
        self.primitive_storage
            .push((symbol, place, value.scalar_type));
        Ok(place)
    }
}
