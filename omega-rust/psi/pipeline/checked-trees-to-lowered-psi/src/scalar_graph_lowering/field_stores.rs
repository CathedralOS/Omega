//! Complete a field's RHS before mutating its current local home. Writes use the
//! same declaration-based field geometry as parameter stores; local identity is
//! resolved in the ordered namespace so copies and moved homes stay distinct.

use super::*;

pub(super) struct Prepared {
    statement: u32,
    destination: PlaceId,
    path: Vec<StructuralPathSegment>,
    field: StructuralFieldId,
    result_type: QualifiedScalarType,
    expression: Option<LoweredDirectExpression>,
}

pub(super) fn prepare(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    state: symbols::SymbolHandle,
    store: &checked_trees::CheckedStructuralScalarFieldStorePlan,
    bindings: &storage::ScalarBindings,
    types: &[StructuralTypeDeclaration],
) -> Result<Prepared, LoweringError> {
    let (_, source) = source_custody::authored_state(checked, state)?;
    let Some(checked_trees::statement::StatementNode::Assignment(assignment)) = checked
        .statement_table
        .statements(source.statement_nodes)
        .get(store.statement_index as usize)
    else {
        return unsupported("record store lost its authored assignment");
    };
    crate::structural_scalar_store_source::validate_assignment(
        checked,
        machine,
        state,
        store.statement_index,
        assignment,
        store,
    )?;
    let checked_trees::CheckedStructuralScalarFieldStoreDestination::Local { symbol } =
        store.destination
    else {
        return unsupported("scalar graph field store needs its local storage join");
    };
    let local = crate::structural_scalar_store_source::local_destination(
        checked,
        source,
        store.statement_index,
        symbol,
    )?;
    let identity = checked.normalized_type_identity(local.type_reference);
    let declaration = types
        .iter()
        .find(|declaration| declaration.identity == identity.as_str())
        .ok_or(LoweringError::Unsupported(
            "record store lost its root carrier",
        ))?;
    let destination = bindings
        .owned_argument(&checked_trees::CheckedUnitStructuralArgumentPlan {
            source: checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralLocal {
                symbol,
            },
            path: Vec::new(),
            type_identity: identity.into_string(),
            access: checked_trees::CheckedStructuralAccess::Owned,
        })?
        .place;
    let (path, field) = crate::structural_scalar_store::lower_structural_field_path(
        declaration.id,
        &store.carrier_path,
        &store.field_identity,
        types,
    )?;
    let scalar_type = terminal_scalar_type(store.primitive_type)?;
    // Bounded fields need a separate reconstructed store obligation. An exact
    // primitive carrier alone cannot authorize violating the declared interval.
    if field.field_type != StructuralFieldType::Scalar(scalar_type) {
        return unsupported(
            "record store field differs from its exact unconstrained scalar carrier",
        );
    }
    let expression = match store.value {
        checked_trees::CheckedStructuralScalarFieldStoreValue::Pure(_) => {
            Some(bindings.expression_at(
                checked,
                state,
                store.statement_index,
                CheckedScalarExpressionRole::AssignmentValue,
            )?)
        }
        checked_trees::CheckedStructuralScalarFieldStoreValue::Computation(_) => None,
    };
    if expression
        .as_ref()
        .is_some_and(|expression| expression.scalar_type() != scalar_type)
    {
        return unsupported("record store changed its RHS carrier");
    }
    Ok(Prepared {
        statement: store.statement_index,
        destination,
        path,
        field: field.id,
        result_type: QualifiedScalarType {
            scalar_type,
            qualifications: Default::default(),
        },
        expression,
    })
}

impl Prepared {
    pub(super) fn finish(
        self,
        state: symbols::SymbolHandle,
        bindings: &storage::ScalarBindings,
        prefix: &[QualifiedScalarType],
        target: usize,
        computations: &mut computations::Expansion<'_>,
    ) -> Result<usize, LoweringError> {
        let mut completed = prefix.to_vec();
        completed.push(self.result_type);
        let store = computations.push(LoweredScalarBranchState {
            structural_parameters: Vec::new(),
            parameter_types: completed.clone(),
            bindings: Vec::new(),
            structural_effects: vec![LoweredScalarEffect::StoreScalarField {
                destination: self.destination,
                path: self.path,
                field: self.field,
                value_position: prefix.len(),
                scalar_type: self.result_type.scalar_type,
            }],
            terminator: LoweredScalarBranchTerminator::Jump {
                target,
                arguments: computations::parameters(prefix),
                structural_arguments: Vec::new(),
                trivial_affine_discards: Vec::new(),
            },
        });
        match self.expression {
            Some(expression) => Ok(computations.push(LoweredScalarBranchState {
                structural_parameters: Vec::new(),
                parameter_types: prefix.to_vec(),
                bindings: vec![LoweredScalarBinding::Expression(expression)],
                structural_effects: Vec::new(),
                terminator: LoweredScalarBranchTerminator::Jump {
                    target: store,
                    arguments: computations::parameters(&completed),
                    structural_arguments: Vec::new(),
                    trivial_affine_discards: Vec::new(),
                },
            })),
            None => computations.retained_value(
                state,
                self.statement,
                CheckedScalarExpressionRole::AssignmentValue,
                symbols::SymbolHandle::invalid(),
                bindings,
                prefix,
                self.result_type,
                store,
            ),
        }
    }
}
