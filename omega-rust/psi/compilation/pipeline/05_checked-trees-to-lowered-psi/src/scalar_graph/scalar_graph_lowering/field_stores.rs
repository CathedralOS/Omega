//! Complete a field's RHS before mutating its current local home. Writes use the
//! same declaration-based field geometry as parameter stores; local identity is
//! resolved in the ordered namespace so copies and moved homes stay distinct.
use super::super::StructuralFieldId;
use super::{
    CheckedScalarExpressionRole, CheckedTrees, LoweringError, PlaceId, QualifiedScalarType,
    StructuralPathSegment, StructuralTypeDeclaration, computations, storage, terminal_scalar_type,
    unsupported,
};
use crate::emission::operation_emission::LoweredScalarBinding;
use crate::emission::operation_emission::expressions::LoweredDirectExpression;
use crate::scalar_graph::scalar_graph_lowering::prepared_graph::{
    LoweredScalarBranchState, LoweredScalarBranchTerminator, LoweredScalarEffect,
};

pub(super) struct Prepared {
    statement: u32,
    destination: PlaceId,
    path: Vec<StructuralPathSegment>,
    field: StructuralFieldId,
    requires_evidence: bool,
    range_decided: bool,
    result_type: QualifiedScalarType,
    expression: Option<LoweredDirectExpression>,
}

pub(super) fn prepare(
    checked: &CheckedTrees,
    state: symbols::SymbolHandle,
    store: &checked_trees::CheckedStructuralScalarFieldStorePlan,
    bindings: &storage::ScalarBindings,
    types: &[StructuralTypeDeclaration],
) -> Result<Prepared, LoweringError> {
    let (machine, source) = crate::declarations::state_declaration(checked, state)?;
    let Some(checked_trees::statement::StatementNode::Assignment(_)) = checked
        .statement_table
        .statements(source.statement_nodes)
        .get(store.statement_index as usize)
    else {
        return unsupported("record store lost its authored assignment");
    };
    let checked_trees::CheckedStoreRoot::Local { symbol } = store.destination else {
        return unsupported("scalar graph field store needs its local storage join");
    };
    let local = crate::emission::structural_scalar_store_source::local_destination(
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
    let (path, field) = crate::emission::structural_scalar_store::lower_structural_field_path(
        declaration.id,
        &store.carrier_path,
        &store.field_identity,
        types,
    )?;
    let scalar_type = terminal_scalar_type(store.primitive_type)?;
    let expression = match store.value {
        checked_trees::CheckedStoreValue::Pure(_) => Some(bindings.expression_at(
            checked,
            state,
            store.statement_index,
            CheckedScalarExpressionRole::AssignmentValue,
        )?),
        checked_trees::CheckedStoreValue::Computation(_) => None,
        // A call result is established by the ordered call operation of the
        // attached-Unit statement sequence. This scalar-graph store route has
        // no such operation beside it and cannot substitute a local read.
        checked_trees::CheckedStoreValue::ScalarResult { .. } => {
            return unsupported("record store has no call operation supplying its result");
        }
    };
    let range_decided = crate::emission::structural_scalar_store::source_range_decided(
        checked,
        machine.symbol,
        state,
        store,
        match &field.field_type {
            terminal_psi::StructuralFieldType::BoundedInteger(integer) => Some(*integer),
            _ => None,
        },
    )?;
    Ok(Prepared {
        range_decided,
        statement: store.statement_index,
        destination,
        path,
        field: field.id,
        requires_evidence: terminal_semantics::scalar_field_requires_evidence(&field.field_type),
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
            erased_formal_types: Vec::new(),
            erased_proof_formals: Vec::new(),
            bindings: Vec::new(),
            structural_effects: vec![LoweredScalarEffect::StoreScalarField {
                destination: self.destination,
                path: self.path,
                field: self.field,
                value_position: prefix.len(),
                scalar_type: self.result_type.scalar_type,
                requires_evidence: self.requires_evidence,
                range_decided: self.range_decided,
            }],
            terminator: LoweredScalarBranchTerminator::Jump {
                target,
                arguments: computations::parameters(prefix),
                erased_arguments: Vec::new(),
                erased_proof_arguments: Vec::new(),
                structural_arguments: Vec::new(),
                cleanup_actions: Vec::new(),
            },
        });
        match self.expression {
            Some(expression) => Ok(computations.push(LoweredScalarBranchState {
                structural_parameters: Vec::new(),
                parameter_types: prefix.to_vec(),
                erased_formal_types: Vec::new(),
                erased_proof_formals: Vec::new(),
                bindings: vec![LoweredScalarBinding::Expression(expression)],
                structural_effects: Vec::new(),
                terminator: LoweredScalarBranchTerminator::Jump {
                    target: store,
                    arguments: computations::parameters(&completed),
                    erased_arguments: Vec::new(),
                    erased_proof_arguments: Vec::new(),
                    structural_arguments: Vec::new(),
                    cleanup_actions: Vec::new(),
                },
            })),
            None => computations.retained_value(
                state,
                self.statement,
                CheckedScalarExpressionRole::AssignmentValue,
                bindings,
                prefix,
                self.result_type,
                store,
            ),
        }
    }
}
