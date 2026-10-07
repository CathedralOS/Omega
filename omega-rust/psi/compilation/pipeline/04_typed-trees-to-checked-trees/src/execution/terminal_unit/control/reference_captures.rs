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
use typed_trees::expression::ExpressionNode;
use typed_trees::types::{PrimitiveType, TypeReferenceHandle};

use crate::execution::terminal_unit::calls;
use crate::execution::terminal_unit::types::base_type_identity;

pub(in crate::execution::terminal_unit) struct ReferenceCapture {
    pub(in crate::execution::terminal_unit) source: CheckedUnitStructuralArgumentPlan,
    pub(in crate::execution::terminal_unit) storage: CheckedReferenceStorage,
    pub(in crate::execution::terminal_unit) backing_type: TypeReferenceHandle,
}

pub(in crate::execution::terminal_unit) fn plan(
    program: &dyn validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    statement_index: usize,
    local: &typed_trees::statement::TableLocalData,
) -> Option<ReferenceCapture> {
    if local.is_mutable || local.relevance.is_erased() {
        return None;
    }
    let (referent, access) =
        validation::reference_result_custody::stored_loan_parts(program, local.type_reference)?;
    let ExpressionNode::Borrow(borrow) = program.expression_table.expression(local.initial_value)
    else {
        return None;
    };
    if CheckedStructuralAccess::from(borrow.access) != access {
        return None;
    }
    let (target, is_view) = match program.expression_table.expression(borrow.target) {
        ExpressionNode::Cast(cast) => {
            let recast = match borrow.access {
                language_core::ReferenceAccess::Shared => {
                    language_core::cast_form::CastForm::RecastShared
                }
                language_core::ReferenceAccess::Mutable => {
                    language_core::cast_form::CastForm::RecastMutable
                }
                language_core::ReferenceAccess::WriteOnly => return None,
            };
            if cast.form != recast
                || program.normalized_type_identity(cast.target_type)
                    != program.normalized_type_identity(referent)
            {
                return None;
            }
            (cast.value, true)
        }
        _ => (borrow.target, false),
    };
    let place = crate::flow::canonical_place_from_expression_in_state(
        program,
        state.symbol,
        statement_index,
        target,
    )?;
    let facts::PlaceRoot::Symbol(root) = place.root else {
        return None;
    };
    let (backing_type, path) = calls::projected_borrowed_path(
        program,
        machine.symbol,
        state.symbol,
        statement_index,
        &place,
    )?;
    if !is_view
        && base_type_identity(program, backing_type, &[])?
            != base_type_identity(program, referent, &[])?
    {
        return None;
    }

    let source = if let Some(parameter) = program
        .state_parameters(state)
        .iter()
        .find(|parameter| parameter.symbol == root)
    {
        if parameter.is_const || parameter.relevance.is_erased() {
            return None;
        }
        if program
            .primitive_type_reference(parameter.type_reference)
            .is_some()
        {
            // Mutable SSA ingress has no coherent addressable home yet. This
            // is a storage boundary, not a license to home a copied snapshot.
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
                root_type_identity: base_type_identity(program, shell.referee, &[])?,
            }
        } else {
            if access != CheckedStructuralAccess::SharedBorrow && !owner.is_mutable {
                return None;
            }
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
            validation::recast_scalar_layout(program, referent)?;
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
            type_identity: base_type_identity(program, backing_type, &[])?,
            access,
        },
        storage,
        backing_type,
    })
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
