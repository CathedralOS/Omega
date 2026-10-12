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
use semantic_vocabulary::{IeeeFloatFormat, IntegerSign, IntegerType, ScalarType};
use typed_trees::types::{PrimitiveType, TypeReferenceHandle};

use crate::execution::terminal_unit::calls;
use crate::execution::terminal_unit::types::field_retains_scalar_restriction;

pub(crate) struct ReferenceCapture {
    pub(crate) source: CheckedUnitStructuralArgumentPlan,
    pub(crate) storage: CheckedReferenceStorage,
    pub(crate) backing_type: TypeReferenceHandle,
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
    let (referee, access) =
        validation::reference_result_custody::stored_loan_parts(program, reference)?;
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
    let (referent, access) =
        validation::reference_result_custody::stored_loan_parts(program, reference)?;
    let formation = validation::reference_formation_source(program, expression)?;
    if CheckedStructuralAccess::from(formation.access) != access {
        return None;
    }
    let is_view = formation.view_type.is_valid();
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
    // A field restricted by its type or its owner's `where` facts has no
    // borrowed shape: Terminal keeps the restriction on the record field, not
    // on a reference carrier. Its loans and views, including a `u8` bounded
    // to 0/1 viewed as `bool`, keep the proved place observation, where reads
    // transport the field invariant and writes re-prove it.
    if let Some(facts::PlaceSegment::Field { symbol }) = place.segments.last()
        && field_retains_scalar_restriction(program, *symbol, backing_type)
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
    let storage = if is_view {
        let stored_scalar = if let Some(primitive) = program.primitive_type_reference(referent) {
            let geometry = validation::recast_scalar_path_projection(program, referent, &[])?;
            Some(match geometry.stored_integer {
                Some(integer) => ScalarType::Integer(integer),
                None => primitive_storage_scalar(primitive)?,
            })
        } else {
            // Aggregate field geometry is retained in the structural catalog.
            // Formation keeps the address here; field loans choose their exact
            // scalar codec when they project through this established parent.
            // The checked source recast covers every alternative. Formation
            // retains its actual backing; the catalog owns target-dependent
            // offsets and sizes until exact execution closure.
            for target in validation::layout_checking_targets(program, [referent]).ok()? {
                let context = validation::LayoutCheckingContext::new(program, target);
                validation::recast_scalar_layout(&context, referent)?;
            }
            None
        };
        CheckedReferenceStorage::View { stored_scalar }
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

fn primitive_storage_scalar(primitive: PrimitiveType) -> Option<ScalarType> {
    match primitive {
        PrimitiveType::Bool => Some(ScalarType::Boolean),
        PrimitiveType::F32 => Some(ScalarType::IeeeFloat(IeeeFloatFormat::Binary32)),
        PrimitiveType::F64 => Some(ScalarType::IeeeFloat(IeeeFloatFormat::Binary64)),
        primitive if primitive.is_fixed_integer() => {
            let sign = if primitive.is_signed_integer() {
                IntegerSign::Signed
            } else {
                IntegerSign::Unsigned
            };
            let bits = u16::try_from(primitive.scalar_byte_size()?.checked_mul(8)?).ok()?;
            IntegerType::new(sign, bits).ok().map(ScalarType::Integer)
        }
        _ => None,
    }
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
