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
    CheckedScalarExpressionRole, CheckedTrees, CheckedUnitEffectMachinePlan,
    CheckedUnitEffectOperationPlan, LoweringError, Operation, OperationKind, OperationResult,
    ScalarType, StructuralMultiplicity, StructuralOperationResult, StructuralPlaceDeclaration,
    StructuralPlaceKind, StructuralTypeShape, allocate_dense, lookup_type_id, parameters, place_id,
    terminal_scalar_type, unsupported,
};
use crate::unit::StructuralTypeDeclaration;
use checked_trees::CheckedCallScalarArgument;
use checked_trees::statement::{StatementNode, TableLocalData};

use super::admission::CallerView;

pub(super) mod unit_calls;

pub(crate) struct PrimitiveLocal {
    pub symbol: symbols::SymbolHandle,
    pub declaration: StructuralPlaceDeclaration,
    pub scalar_type: ScalarType,
}

pub(crate) fn source<'checked>(
    checked: &'checked CheckedTrees,
    plan: &CheckedUnitEffectMachinePlan,
    symbol: symbols::SymbolHandle,
    before: u32,
) -> Result<&'checked TableLocalData, LoweringError> {
    declaration(checked, plan.state, symbol, before)
}

/// The one authored mutable primitive declaration of `symbol` in `state`
/// before statement `before`.
fn declaration(
    checked: &CheckedTrees,
    state: symbols::SymbolHandle,
    symbol: symbols::SymbolHandle,
    before: u32,
) -> Result<&TableLocalData, LoweringError> {
    let (_, state) = crate::expression_preparation::source_custody::authored_state(checked, state)?;
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

pub(crate) fn validate_roster(
    checked: &CheckedTrees,
    plan: &CheckedUnitEffectMachinePlan,
) -> Result<(), LoweringError> {
    let (_, state) =
        crate::expression_preparation::source_custody::authored_state(checked, plan.state)?;
    for (ordinal, statement) in checked
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .enumerate()
    {
        let StatementNode::LocalData(local) = statement else {
            continue;
        };
        if !local.is_mutable
            || checked
                .primitive_type_reference(local.type_reference)
                .is_none()
        {
            continue;
        }
        if plan
            .operations
            .iter()
            .filter(|operation| {
                matches!(operation,
            CheckedUnitEffectOperationPlan::EstablishPrimitiveLocal { statement_index, .. }
                if *statement_index as usize == ordinal)
            })
            .count()
            != 1
        {
            return unsupported(
                "primitive local roster omits or duplicates an authored declaration",
            );
        }
    }
    for operation in &plan.operations {
        if matches!(
            operation,
            CheckedUnitEffectOperationPlan::EstablishPrimitiveLocal { .. }
        ) {
            validate_establishment(checked, plan.machine, plan.state, operation)?;
        }
    }
    Ok(())
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
        type_identity,
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
    let (_, authored) =
        crate::expression_preparation::source_custody::authored_state(checked, state)?;
    if !matches!(checked.statement_table.statements(authored.statement_nodes).get(*statement_index as usize),
        Some(StatementNode::LocalData(actual)) if actual.symbol == *symbol)
        || checked.primitive_type_reference(local.type_reference) != Some(*primitive_type)
        || checked
            .typed
            .normalized_type_identity(validation::peel_arithmetic_policy(
                &checked.typed,
                local.type_reference,
            ))
            .into_string()
            != *type_identity
    {
        return unsupported("primitive local substituted its authored type or declaration");
    }
    let role = CheckedScalarExpressionRole::StorageInitializer;
    match value {
        CheckedCallScalarArgument::Pure(value) => {
            let (binding, expression) = checked
                .facts
                .values
                .scalar_expressions
                .bound_expression_at(state, *statement_index, role)
                .ok_or(LoweringError::Unsupported(
                    "primitive local lost its exact initializer",
                ))?;
            if binding.expression != local.initial_value
                || binding.destination != *symbol
                || expression != value
            {
                return unsupported("primitive local substituted its initializer");
            }
            crate::expression_preparation::source_custody::validate_pure(
                checked,
                binding,
                terminal_scalar_type(*primitive_type)?,
            )?;
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
            crate::expression_preparation::source_custody::validate_computation_calls(
                checked,
                machine,
                state,
                *statement_index,
                *handle,
                local.initial_value,
            )?;
        }
    }
    Ok(())
}

pub(super) fn validate_argument_source(
    checked: &CheckedTrees,
    caller: &CallerView<'_>,
    coordinate: checked_trees::CheckedUnitCallCoordinate,
    source_target: symbols::SymbolHandle,
    argument: &checked_trees::CheckedUnitStructuralArgumentPlan,
    expression: checked_trees::expression::ExpressionHandle,
) -> Result<bool, LoweringError> {
    let checked_trees::CheckedUnitStructuralArgumentSourcePlan::PrimitiveLocal { symbol } =
        argument.source
    else {
        return Ok(false);
    };
    // Ordinary bodies and composed states establish the same primitive home.
    // Rejoin its authored state and borrow occurrence, not the body's family.
    let local = declaration(checked, caller.state, symbol, coordinate.statement_index)?;
    let (machine, _) =
        crate::expression_preparation::source_custody::authored_state(checked, caller.state)?;
    let (root, path, access) =
        parameters::source_path(checked, machine, None, local.type_reference, expression)?;
    if machine.symbol != caller.machine
        || root != symbol
        || !path.is_empty()
        || !argument.path.is_empty()
        || access != Some(argument.access)
        || argument.access == checked_trees::CheckedStructuralAccess::Owned
        || argument.type_identity
            != checked
                .typed
                .normalized_type_identity(validation::peel_arithmetic_policy(
                    &checked.typed,
                    local.type_reference,
                ))
                .into_string()
    {
        return unsupported("primitive local argument substituted its authored borrow");
    }
    crate::expression_preparation::source_custody::borrow_occurrences::validate(
        checked,
        caller.machine,
        caller.state,
        coordinate,
        source_target,
        symbol,
        argument.access,
    )?;
    Ok(true)
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
            result: OperationResult::Structural(StructuralOperationResult {
                qualification_establishments: Vec::new(),
                place,
                structural_type,
                multiplicity: StructuralMultiplicity::Unrestricted,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
                claims: Vec::new(),
            }),
            kind: OperationKind::EstablishPrimitiveLocal {
                value: initializer.id,
            },
        });
        self.evaluation
            .primitive_storage
            .push((*symbol, place, initializer.scalar_type));
        Ok(PrimitiveLocal {
            symbol: *symbol,
            scalar_type: initializer.scalar_type,
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

#[cfg(test)]
mod tests {
    use super::{CheckedUnitEffectOperationPlan, validate_establishment};

    const SOURCE: &str = "
        data Output { value: i32; }
        machine fill(output: &mut Output) {
            let mut value: i32 in Wrapping;
            value = value - 1;
            output.value = value as i32;
        }
    ";

    #[test]
    fn qualified_primitive_local_keeps_checked_storage_custody() {
        let checked = crate::front_end::checked_program(SOURCE);
        let lowered = crate::lower_machine(&checked, crate::TerminalMachineSelection::Name("fill"))
            .expect("zeroed qualified local composes with writes and an output borrow");
        terminal_verifier::verify_module(
            &lowered.semantic_module,
            &lowered.proof_bundle,
            &proof_admission::AdmissionProfile::default(),
        )
        .expect("independent verification preserves local storage and arithmetic");
    }

    #[test]
    fn qualified_primitive_local_rejects_substituted_storage_identity() {
        let checked = crate::front_end::checked_program(SOURCE);
        let plan = checked
            .facts
            .flow
            .terminal_unit_effects
            .machines
            .iter()
            .find(|plan| {
                plan.operations.iter().any(|operation| {
                    matches!(
                        operation,
                        CheckedUnitEffectOperationPlan::EstablishPrimitiveLocal { .. }
                    )
                })
            })
            .expect("qualified local has an ordinary establishment plan");
        let operation = plan
            .operations
            .iter()
            .find(|operation| {
                matches!(
                    operation,
                    CheckedUnitEffectOperationPlan::EstablishPrimitiveLocal { .. }
                )
            })
            .expect("local establishment");
        validate_establishment(&checked, plan.machine, plan.state, operation)
            .expect("authored storage identity is accepted");
        let mut substituted = operation.clone();
        if let CheckedUnitEffectOperationPlan::EstablishPrimitiveLocal { type_identity, .. } =
            &mut substituted
        {
            type_identity.push_str("::substituted");
        }
        let error = validate_establishment(&checked, plan.machine, plan.state, &substituted)
            .expect_err("peeling arithmetic policy must not permit replacing the storage carrier");
        assert!(format!("{error:?}").contains("substituted its authored type"));
    }
}
