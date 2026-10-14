//! Reference formation publishes a storage producer, not an erased alias.
//!
//! A parent's address is captured at its declaration, before later selector
//! mutations. Children project through that retained carrier; they do not
//! rediscover a parent's initializer. Recasts retain the actual backing and
//! the source-selected storage interpretation separately from the logical
//! referee. Borrow admission still owns access, lineage and lifetime.

use checked_trees::{
    CheckedReferenceStorage, CheckedStructuralAccess, CheckedUnitStructuralArgumentPlan,
    CheckedUnitStructuralArgumentSourcePlan,
};
use typed_trees::types::TypeReferenceHandle;

use crate::execution::terminal_unit::calls;
use crate::execution::terminal_unit::types::field_requires_place_observation;

pub(crate) struct ReferenceCapture {
    pub(crate) source: CheckedUnitStructuralArgumentPlan,
    pub(crate) storage: CheckedReferenceStorage,
    pub(crate) backing_type: TypeReferenceHandle,
}

/// A computed pointer value carries its source-selected reference type.
/// Named homes retain their existing projected-loan route; registering them
/// as new values would duplicate their descriptors and custody. This query
/// grants no receiving access and does not evaluate the producer.
pub(crate) fn transported_value_type(
    program: &dyn validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    statement_index: usize,
    expression: typed_trees::expression::ExpressionHandle,
) -> Option<TypeReferenceHandle> {
    // Forming a loan and transporting an existing descriptor are different
    // ownership events. Formation's caller retains the temporary referent
    // and schedules its cleanup; treating it as transport would register only
    // the new descriptor and lose that backing owner's normal continuation.
    if validation::reference_formation_source(program, expression).is_some() {
        return None;
    }
    if validation::canonical_place_from_expression_in_state(
        program,
        state.symbol,
        statement_index,
        expression,
    )
    .is_some_and(|place| place.root.as_symbol().is_some())
    {
        return None;
    }
    let actual = validation::expression_result_type_reference(program, machine, state, expression)?;
    validation::reference_result_custody::stored_loan_parts(program, actual)?;
    Some(actual)
}

/// A source-selected storage view forms its authored reference value before
/// a consumer attenuates access. The receiving formal cannot choose this
/// carrier's type or replace its storage interpretation.
pub(crate) fn formed_view_type(
    program: &dyn validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    expression: typed_trees::expression::ExpressionHandle,
) -> Option<TypeReferenceHandle> {
    let formation = validation::reference_formation_source(program, expression)?;
    if !formation.view_type.is_valid() {
        return None;
    }
    let reference =
        validation::expression_result_type_reference(program, machine, state, expression)?;
    let shell = validation::reference_shell(program, reference)?;
    let referee = shell.referee;
    let access = CheckedStructuralAccess::from(shell.access);
    (access == CheckedStructuralAccess::from(formation.access)
        && program.normalized_type_identity(referee)
            == program.normalized_type_identity(formation.view_type))
    .then_some(reference)
}

pub(crate) fn capture(
    program: &dyn validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    statement_index: usize,
    expression: typed_trees::expression::ExpressionHandle,
    reference: TypeReferenceHandle,
    selector: &dyn Fn(
        typed_trees::expression::ExpressionHandle,
    ) -> Option<checked_trees::CheckedUnitStructuralPathSegment>,
) -> Option<ReferenceCapture> {
    // Formation captures borrowed storage for every reference descriptor.
    // The pointer-roster query excludes byte/slice descriptors by design;
    // using it here would erase a fixed-byte local's actual establishment.
    let shell = validation::reference_shell(program, reference)?;
    let referent = shell.referee;
    let access = CheckedStructuralAccess::from(shell.access);
    let formation = validation::reference_formation_source(program, expression)?;
    if CheckedStructuralAccess::from(formation.access) != access {
        return None;
    }
    let is_view = formation.view_type.is_valid();
    let is_placed =
        formation.interpretation == validation::ReferenceFormationInterpretation::Placed;
    if is_view
        && program.normalized_type_identity(formation.view_type)
            != program.normalized_type_identity(referent)
    {
        return None;
    }
    let target = formation.source;
    let place = validation::canonical_place_from_expression_in_state(
        program,
        state.symbol,
        statement_index,
        target,
    )?;
    let (backing_type, mut path) = calls::projected_evaluated_path(
        program,
        machine.symbol,
        state.symbol,
        statement_index,
        &place,
        Some(selector),
    )?;
    // An exclusive reference cannot erase a field's receiving restriction,
    // and an atomic cell cannot become ordinary scalar storage. Shared scalar
    // loans can attenuate to the declared carrier while retaining their exact
    // captured source; every write still belongs to the owning record. Keep
    // their selectors in this ordinary value producer rather than replaying
    // mutable selector storage through a silent place alias.
    if let Some(facts::PlaceSegment::Field { symbol }) = place.segments.last()
        && field_requires_place_observation(program, *symbol, backing_type, access)
    {
        return None;
    }
    // Call projections end at their selected carrier. Reference formation
    // instead names the storage it lends: cross every stored reference shell
    // at that leaf. An ordinary storage endpoint needs no additional crossing.
    // Stage05 preserves this ending marker independently of ordinary argument
    // projections; omitting it would borrow a descriptor rather than its referent.
    let mut endpoint = backing_type;
    // A whole borrowed formal denotes ingress referent storage, not a stored
    // pointer value. Only subsequent reference shells are physical crossings.
    if place.segments.is_empty()
        && let facts::PlaceRoot::Symbol(root) = place.root
        && program.state_parameter_by_symbol(state, root).is_some()
        && let Some(shell) = validation::reference_shell(program, endpoint)
    {
        if !access_can_supply(shell.access.into(), access) {
            return None;
        }
        endpoint = shell.referee;
    }
    let mut crossings = 0;
    while let Some(shell) = validation::reference_shell(program, endpoint) {
        if !access_can_supply(shell.access.into(), access) {
            return None;
        }
        crossings += 1;
        endpoint = shell.referee;
    }
    path.extend(std::iter::repeat_n(
        checked_trees::CheckedUnitStructuralPathSegment::Referent,
        crossings,
    ));
    if !is_view
        && validation::structural_type_identity(program, endpoint, &[], &[])
            != validation::structural_type_identity(program, referent, &[], &[])
    {
        return None;
    }
    if is_placed {
        let named_symbol = |reference| {
            let reference = program
                .type_reference_table
                .peeled_type_reference(reference);
            match program.type_reference_table.type_reference(reference) {
                typed_trees::types::TypeReferenceNode::Named { symbol, .. } => Some(*symbol),
                _ => None,
            }
        };
        let view = named_symbol(referent)?;
        let mut plans = program
            .placed_view_plans
            .iter()
            .filter(|plan| plan.data_symbol == view);
        let (Some(plan), None) = (plans.next(), plans.next()) else {
            return None;
        };
        if named_symbol(endpoint)? != plan.schema_symbol {
            return None;
        }
    }

    let source = match place.root {
        // The value owner must establish this producer before lending it.
        // Its captured path and storage interpretation are otherwise the same
        // as a named home's; source spelling does not select another planner.
        facts::PlaceRoot::Expression(expression) => {
            CheckedUnitStructuralArgumentSourcePlan::StructuralExpression { expression }
        }
        facts::PlaceRoot::Symbol(root) => {
            if let Some(parameter) = program.state_parameter_by_symbol(state, root) {
                if parameter.is_const || parameter.relevance.is_erased() {
                    return None;
                }
                if program
                    .primitive_type_reference(parameter.type_reference)
                    .is_some()
                {
                    // Immutable scalar ingress can lend its one stable home. A
                    // mutable formal instead observes live storage established by
                    // its body; homing an ingress snapshot would miss later writes.
                    if parameter.is_mutable || access != CheckedStructuralAccess::SharedBorrow {
                        return None;
                    }
                    let position = program
                        .state_parameters(state)
                        .iter()
                        .filter(|parameter| {
                            !parameter.is_const
                                && !parameter.relevance.is_erased()
                                && program
                                    .primitive_type_reference(parameter.type_reference)
                                    .is_some()
                        })
                        .position(|parameter| parameter.symbol == root)?;
                    CheckedUnitStructuralArgumentSourcePlan::ScalarParameter {
                        parameter_index: u32::try_from(position).ok()?,
                    }
                } else {
                    let position = program
                        .state_parameters(state)
                        .iter()
                        .filter(|parameter| {
                            !parameter.is_const
                                && !parameter.relevance.is_erased()
                                && program
                                    .primitive_type_reference(parameter.type_reference)
                                    .is_none()
                        })
                        .position(|parameter| parameter.symbol == root)?;
                    CheckedUnitStructuralArgumentSourcePlan::Parameter {
                        parameter_index: u32::try_from(position).ok()?,
                    }
                }
            } else {
                let (declaration_index, owner) = program.state_local_declaration(state, root)?;
                if declaration_index >= statement_index || owner.relevance.is_erased() {
                    return None;
                }
                if let Some(shell) = validation::reference_shell(program, owner.type_reference) {
                    let supplied = CheckedStructuralAccess::from(shell.access);
                    if !matches!(
                        (supplied, access),
                        (CheckedStructuralAccess::MutableBorrow, _)
                            | (
                                CheckedStructuralAccess::SharedBorrow,
                                CheckedStructuralAccess::SharedBorrow
                            )
                            | (
                                CheckedStructuralAccess::WriteOnlyBorrow,
                                CheckedStructuralAccess::WriteOnlyBorrow
                            )
                    ) {
                        return None;
                    }
                    CheckedUnitStructuralArgumentSourcePlan::StructuralLocalBorrow {
                        symbol: root,
                        root_type_identity: validation::structural_type_identity(
                            program,
                            shell.referee,
                            &[],
                            &[],
                        ),
                    }
                } else {
                    // Owned contents lend their admitted access independently of
                    // permission to replace the source binding.
                    if owner.is_mutable
                        && program
                            .primitive_type_reference(owner.type_reference)
                            .is_some()
                    {
                        CheckedUnitStructuralArgumentSourcePlan::PrimitiveLocal { symbol: root }
                    } else {
                        CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol: root }
                    }
                }
            }
        }
        _ => return None,
    };
    let storage = if is_placed {
        CheckedReferenceStorage::Placed
    } else if is_view {
        validation::reference_view_storage(program, referent)?
    } else {
        CheckedReferenceStorage::Declared
    };
    Some(ReferenceCapture {
        source: CheckedUnitStructuralArgumentPlan {
            source,
            path,
            type_identity: validation::structural_type_identity(program, endpoint, &[], &[]),
            access,
        },
        storage,
        backing_type: endpoint,
    })
}

fn access_can_supply(source: CheckedStructuralAccess, result: CheckedStructuralAccess) -> bool {
    source == result
        || matches!(
            source,
            CheckedStructuralAccess::Owned | CheckedStructuralAccess::MutableBorrow
        )
}

/// Existing stored descriptors retain their current carrier. Fresh formations
/// use the same source capture as locals, with their selector evaluation owned
/// by the consuming value occurrence.
pub(crate) fn plan(
    program: &dyn validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    statement_index: usize,
    expression: typed_trees::expression::ExpressionHandle,
    reference: TypeReferenceHandle,
    selector: &dyn Fn(
        typed_trees::expression::ExpressionHandle,
    ) -> Option<checked_trees::CheckedUnitStructuralPathSegment>,
) -> Option<CheckedUnitStructuralArgumentPlan> {
    if validation::reference_formation_source(program, expression).is_some() {
        return Some(
            capture(
                program,
                machine,
                state,
                statement_index,
                expression,
                reference,
                selector,
            )?
            .source,
        );
    }
    validation::reference_result_custody::initializer_source(program, state, expression, reference)
}
