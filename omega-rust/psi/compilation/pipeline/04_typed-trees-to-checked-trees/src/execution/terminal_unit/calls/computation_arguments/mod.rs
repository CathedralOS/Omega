//! Structural homes, projected borrowed operands, and primitive referents
//! in scalar computations. Whole owned operands use the same storage contract
//! for parameters and locals. Their call occurrence retains identity until
//! checked ownership supplies copy or transfer authority; a nominal hook changes
//! the receiving owner's cleanup, not the argument's storage representation.
use crate::execution::terminal_unit::CheckedStructuralAccess;
use crate::execution::terminal_unit::CheckedUnitStructuralArgumentPlan;
use crate::execution::terminal_unit::CheckedUnitStructuralArgumentSourcePlan;
use crate::execution::terminal_unit::CheckedUnitStructuralTypeShape;
use crate::execution::terminal_unit::ExpressionNode;
use crate::execution::terminal_unit::Multiplicity;
use crate::execution::terminal_unit::PrimitiveType;
use crate::execution::terminal_unit::types::{
    ShapeCollector, base_type_identity, borrowed_slice_view_element, parameter_qualifications,
    strips_erased_parameter, structural_access_for_type_reference,
};

use crate::execution::terminal_unit::StateParameter;
use crate::execution::terminal_unit::SymbolHandle;
use crate::execution::terminal_unit::TypeReferenceHandle;
use crate::execution::terminal_unit::TypeReferenceNode;
use crate::execution::terminal_unit::TypedTrees;

use crate::execution::terminal_unit::calls::argument_paths::projected_evaluated_path;
use crate::execution::terminal_unit::calls::structural_arguments::exact_structural_borrow_access;
use crate::execution::terminal_unit::calls::view_subslice::{ViewKind, view_identity};

pub(crate) fn structural_computation_argument(
    program: &TypedTrees,
    borrow: &checked_trees::BorrowFacts,
    machine: SymbolHandle,
    state: &typed_trees::state::State,
    call: &checked_trees::FlowCallFact,
    expression: typed_trees::expression::ExpressionHandle,
    target: &StateParameter,
    selectors: Option<
        &dyn Fn(
            typed_trees::expression::ExpressionHandle,
        ) -> Option<checked_trees::CheckedUnitStructuralPathSegment>,
    >,
) -> Option<CheckedUnitStructuralArgumentPlan> {
    if target.is_const {
        return None;
    }
    let target_access = structural_access_for_type_reference(program, target.type_reference)?;
    let named = match program.expression_table.expression(expression) {
        ExpressionNode::Borrow(argument) => {
            let shell = validation::reference_shell(program, target.type_reference)?;
            if argument.access != shell.access {
                return None;
            }
            argument.target
        }
        ExpressionNode::Name(_) | ExpressionNode::Member(_) | ExpressionNode::Indexed(_) => {
            expression
        }
        _ => return None,
    };
    let place = crate::flow::canonical_place_from_expression_in_state(
        program,
        state.symbol,
        call.statement_index,
        expression,
    )?;

    rejoin_computation_accesses(program, borrow, machine, state.symbol, call)?;
    exact_actual(program, call, expression, target)?;
    // Mutable scalar locals have their own storage representation. Resolve
    // that representation before the common structural-home borrow join.
    if let facts::PlaceRoot::Symbol(symbol) = place.root
        && super::super::structural_scalar_store::primitive_local_before(
            program,
            state,
            call.statement_index,
            symbol,
        )
        .is_some()
    {
        return primitive_local_argument(
            program,
            borrow,
            machine,
            state,
            call,
            &place,
            target.type_reference,
        );
    }
    if matches!(
        target_access,
        CheckedStructuralAccess::SharedBorrow | CheckedStructuralAccess::MutableBorrow
    ) && let Some(argument) = borrowed_storage_argument(
        program, borrow, machine, state, call, &place, target, selectors,
    ) {
        return Some(argument);
    }
    if let Some(argument) = borrowed_view_argument(
        program, borrow, machine, state, call, &place, target, selectors,
    ) {
        return Some(argument);
    }
    let ExpressionNode::Name(name) = program.expression_table.expression(named) else {
        return None;
    };
    if !name.symbol.is_valid()
        || name.head_symbol != name.symbol
        || program
            .expression_table
            .name_path_members(name.members)
            .len()
            != 1
        || place.root != facts::PlaceRoot::Symbol(name.symbol)
        || !place.segments.is_empty()
    {
        return None;
    }
    if target_access == CheckedStructuralAccess::Owned {
        if named != expression {
            return None;
        }
        // This retains authored actual identity. Graph publication rejoins
        // affine transfers after multiplicity checking produces permissions.
        return owned_argument(program, state, call.statement_index, name.symbol, target);
    }
    let target_type = plain_primitive_referent(program, target.type_reference)?;
    let parameters = program.state_parameters(state);
    let source_position = parameters
        .iter()
        .position(|source| source.symbol == name.symbol)?;
    let source = &parameters[source_position];
    if (source.is_const) || plain_primitive_referent(program, source.type_reference)? != target_type
    {
        return None;
    }
    let source_access = structural_access_for_type_reference(program, source.type_reference)?;
    if !matches!(
        (source_access, target_access),
        (
            CheckedStructuralAccess::SharedBorrow,
            CheckedStructuralAccess::SharedBorrow
        ) | (
            CheckedStructuralAccess::MutableBorrow,
            CheckedStructuralAccess::SharedBorrow
                | CheckedStructuralAccess::MutableBorrow
                | CheckedStructuralAccess::WriteOnlyBorrow
        ) | (
            CheckedStructuralAccess::WriteOnlyBorrow,
            CheckedStructuralAccess::WriteOnlyBorrow
        )
    ) {
        return None;
    }
    let access = exact_structural_borrow_access(
        program,
        borrow,
        machine,
        state.symbol,
        call,
        &place,
        target_access,
        &[],
    )?;
    if access != target_access {
        return None;
    }
    let parameter_index = dense_structural_index(program, parameters, source_position)?;
    Some(CheckedUnitStructuralArgumentPlan {
        source: CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index },
        path: Vec::new(),
        type_identity: base_type_identity(program, target.type_reference, &[])?,
        access,
    })
}

/// The range is still an actual of this exact checked call, not merely an
/// equal-typed view expression. Its bounds use dense explicit structural
/// ordinals, which exclude the implicit receiver.
pub(crate) fn subslice_computation_argument(
    program: &TypedTrees,
    operators: &checked_trees::CheckedOperatorFacts,
    expressions: &checked_trees::CheckedScalarExpressionPlans,
    computations: &checked_trees::CheckedScalarComputationPlans,
    borrow: &checked_trees::BorrowFacts,
    machine: SymbolHandle,
    state: &typed_trees::state::State,
    call: &checked_trees::FlowCallFact,
    expression: typed_trees::expression::ExpressionHandle,
    target: &StateParameter,
    argument_ordinal: u32,
) -> Option<CheckedUnitStructuralArgumentPlan> {
    if target.is_const {
        return None;
    }
    exact_actual(program, call, expression, target)?;
    rejoin_computation_accesses(program, borrow, machine, state.symbol, call)?;
    let owner = program
        .machines()
        .iter()
        .find(|owner| owner.symbol == machine)?;
    let place = crate::flow::canonical_place_from_expression_in_state(
        program,
        state.symbol,
        call.statement_index,
        expression,
    )?;

    // Local storage already owns its identity. Only a parameter
    // root needs the caller's structural signature namespace; rebuilding the
    // enclosing attachment for a local would unnecessarily require every
    // unrelated field (including service bindings) to be scalar-graph storage.
    let parameters = if !program
        .state_parameters(state)
        .iter()
        .any(|parameter| place.root == facts::PlaceRoot::Symbol(parameter.symbol))
    {
        Vec::new()
    } else if owner.attached_data.is_some() {
        super::ambient_self_scalar_graph_signature(program, owner, state)?.0
    } else {
        super::free_structural_scalar_signature(
            program,
            &mut ShapeCollector::new(program),
            state,
            &[],
        )?
        .0
    };
    let subslice = super::view_subslice::admit_resolved(
        program,
        operators,
        expressions,
        computations,
        borrow,
        owner,
        state,
        &parameters,
        target.type_reference,
        expression,
        call.statement_index,
        checked_trees::CheckedSubsliceSite::CallArgument {
            call_ordinal: u32::try_from(call.call_ordinal).ok()?,
            argument_ordinal,
        },
    )?;
    let argument = subslice.argument();
    // The ordinary range admission already rejoins the explicit actual with
    // its formal. A scalar expression changes no loan: retain that exact
    // access and require the borrow ledger to agree, as a statement call does.
    (exact_structural_borrow_access(
        program,
        borrow,
        machine,
        state.symbol,
        call,
        &place,
        argument.access,
        &[],
    )? == argument.access)
        .then_some(argument)
}

fn exact_actual(
    program: &TypedTrees,
    call: &checked_trees::FlowCallFact,
    expression: typed_trees::expression::ExpressionHandle,
    target: &StateParameter,
) -> Option<()> {
    let target_position =
        crate::semantic::calls::call_target_parameters(program, call.target_symbol)?
            .iter()
            .position(|parameter| parameter == target)?;
    let ExpressionNode::Call(authored) = program
        .expression_table
        .expression(call.authored_expression)
    else {
        return None;
    };
    let exact_actual = program
        .expression_table
        .expression_handles(authored.arguments)
        .get(target_position)
        == Some(&expression);
    if authored.target_symbol != call.target_symbol || !exact_actual {
        return None;
    }
    Some(())
}

/// A borrowed argument names its existing parameter or local result
/// home. Initializer spelling does not select custody; publication independently
/// rejoins this symbol to its dominating structural establishment.
pub(crate) fn borrowed_storage_argument(
    program: &TypedTrees,
    borrow: &checked_trees::BorrowFacts,
    machine: SymbolHandle,
    state: &typed_trees::state::State,
    call: &checked_trees::FlowCallFact,
    place: &crate::flow::CanonicalPlace,
    target: &StateParameter,
    selectors: Option<
        &dyn Fn(
            typed_trees::expression::ExpressionHandle,
        ) -> Option<checked_trees::CheckedUnitStructuralPathSegment>,
    >,
) -> Option<CheckedUnitStructuralArgumentPlan> {
    let facts::PlaceRoot::Symbol(symbol) = place.root else {
        return None;
    };
    // A borrowed formal retains the selected record at its existing root/path.
    // Receiver syntax does not confer additional projection authority; both
    // lanes rejoin exact source type and the captured access occurrence below.
    let target_shell = validation::reference_shell(program, target.type_reference)?;
    let referee = validation::peel_declared_constraints(program, target_shell.referee);
    let target_access = match target_shell.access {
        language_semantics::ReferenceAccess::Shared => CheckedStructuralAccess::SharedBorrow,
        language_semantics::ReferenceAccess::Mutable => CheckedStructuralAccess::MutableBorrow,
        _ => return None,
    };
    // A closed generic application is the same nominal carrier as its retained
    // synthesized instance. Contents, exact identity and collected shape below
    // still decide eligibility; syntax kind alone must not exclude the borrow.
    // Fixed arrays use the same established storage loan. Shape collection and
    // the exact source/formal identity below retain their element and length.
    if !matches!(
        program.type_reference_table.type_reference(referee),
        TypeReferenceNode::Named { .. }
            | TypeReferenceNode::Generic { .. }
            | TypeReferenceNode::FixedArray { .. }
    ) {
        return None;
    }
    let target_identity = base_type_identity(program, referee, &[])?;
    let parameters = program.state_parameters(state);
    let (reference, source) = if let Some(position) = parameters
        .iter()
        .position(|parameter| parameter.symbol == symbol)
    {
        let parameter = &parameters[position];
        if parameter.is_const {
            return None;
        }
        let reference = match program.type_reference_table.type_reference(
            validation::peel_declared_constraints(program, parameter.type_reference),
        ) {
            TypeReferenceNode::Reference {
                access, referee, ..
            } if matches!(
                access,
                language_semantics::ReferenceAccess::Shared
                    | language_semantics::ReferenceAccess::Mutable
            ) && (target_access == CheckedStructuralAccess::SharedBorrow
                || *access == language_semantics::ReferenceAccess::Mutable) =>
            {
                *referee
            }
            TypeReferenceNode::Named { .. }
            | TypeReferenceNode::Generic { .. }
            | TypeReferenceNode::FixedArray { .. }
                if target_access == CheckedStructuralAccess::SharedBorrow
                    || parameter.is_mutable =>
            {
                parameter.type_reference
            }
            _ => return None,
        };
        let parameter_index = dense_structural_index(program, parameters, position)?;
        (
            reference,
            CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index },
        )
    } else {
        // A projection borrows the same established local home. Resolve its
        // endpoint below while retaining the declaring local as the owner.
        let (_, local) = program
            .state_local_declaration(state, symbol)
            .filter(|&(ordinal, _)| ordinal < call.statement_index)?;
        // Zero-initialized storage and later field writes use the same local
        // home as an authored initializer. Source default-domain checks own
        // establishment; the exact borrow event below owns access. Lowering
        // separately requires this local's dominating published storage.
        // An established shared-borrow local already loans the referent it was
        // bound to; forwarding it hands that referent carrier to the callee,
        // exactly as a shared-borrow parameter would. The retained symbol and
        // recorded access stay the local's own — nothing unwraps or copies the
        // referent into a fabricated place. A fixed-array local is the same
        // kind of established home: its bounds come from the literal length
        // when a segment projects inside it.
        let reference = match program.type_reference_table.type_reference(
            validation::peel_declared_constraints(program, local.type_reference),
        ) {
            TypeReferenceNode::Reference {
                access, referee, ..
            } if matches!(
                access,
                language_semantics::ReferenceAccess::Shared
                    | language_semantics::ReferenceAccess::Mutable
            ) && (target_access == CheckedStructuralAccess::SharedBorrow
                || (*access == language_semantics::ReferenceAccess::Mutable
                    && !local.is_mutable)) =>
            {
                *referee
            }
            TypeReferenceNode::Named { .. }
            | TypeReferenceNode::Generic { .. }
            | TypeReferenceNode::FixedArray { .. }
                if target_access == CheckedStructuralAccess::SharedBorrow || local.is_mutable =>
            {
                local.type_reference
            }
            _ => return None,
        };
        (
            reference,
            CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol },
        )
    };
    let whole_storage = place.segments.is_empty()
        || matches!(
            source,
            CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { .. }
        );
    if whole_storage
        && (!matches!(
            program
                .type_reference_table
                .type_reference(validation::peel_declared_constraints(program, reference)),
            TypeReferenceNode::Named { .. }
                | TypeReferenceNode::Generic { .. }
                | TypeReferenceNode::FixedArray { .. }
        ) || !matches!(
            program.type_multiplicity(reference),
            Multiplicity::Affine | Multiplicity::Unrestricted
        ))
    {
        return None;
    }
    let (mut reference, mut path) = if place.segments.is_empty() {
        (reference, Vec::new())
    } else {
        projected_evaluated_path(
            program,
            machine,
            state.symbol,
            call.statement_index,
            place,
            selectors,
        )?
    };
    // The selected storage and the callee's referent are different endpoints
    // when the storage contains a reference. Normalize type and address in
    // lockstep: type-only normalization would pass the descriptor as its
    // referent. Root formal loans were already represented by their referent
    // above; only reference shells still present here require a crossing.
    while let Some(shell) = validation::reference_shell(program, reference) {
        if shell.access != language_semantics::ReferenceAccess::Mutable
            && !(shell.access == language_semantics::ReferenceAccess::Shared
                && target_access == CheckedStructuralAccess::SharedBorrow)
        {
            return None;
        }
        path.push(checked_trees::CheckedUnitStructuralPathSegment::Referent);
        reference = shell.referee;
    }
    // Eligibility belongs to the borrowed endpoint, not its unrelated siblings.
    // Root membership/access and the exact authored receiver loan are retained
    // independently; this never constructs or copies the enclosing record.
    if !path.is_empty()
        && !matches!(
            program.type_multiplicity(reference),
            Multiplicity::Affine | Multiplicity::Unrestricted
        )
    {
        return None;
    }
    let mut shapes = ShapeCollector::new(program);
    let identity = shapes.add_type(reference, &[], &[])?;
    // Borrow the established endpoint in place, including its shared view and
    // provider members. Contents are classified recursively from authored types:
    // the old shape whitelist erased slice access and made valid loans depend
    // on which field families appeared together. This grants no copy, provider
    // establishment or extended backing lifetime; exact source/type/access and
    // call-occurrence replay still own those obligations.
    let structural_root = shapes.types.get(&identity).is_some_and(|shape| {
        matches!(
            shape.shape,
            CheckedUnitStructuralTypeShape::Sum { .. }
                | CheckedUnitStructuralTypeShape::Mixed { .. }
                | CheckedUnitStructuralTypeShape::Record { .. }
                | CheckedUnitStructuralTypeShape::FixedArray { .. }
                | CheckedUnitStructuralTypeShape::ByteSequence(
                    checked_trees::CheckedByteSequenceCarrier::BoundedOwned { .. }
                )
        )
    });
    // A projected receiver still loans the complete selected referent. Its
    // transitive field closure is identical to a whole parameter's; the exact
    // root/path checks above retain custody of the enclosing storage.
    let structural_referent = structural_root
        && matches!(
            program.type_multiplicity(reference),
            Multiplicity::Affine | Multiplicity::Unrestricted
        )
        && validation::has_borrowed_storage_contents(program, reference);
    // Primitive and aggregate endpoints borrow their established home using
    // the same exact root/path/access join. No scalar snapshot is constructed.
    let primitive_scalar = shapes.types.len() == 1
        && program.type_multiplicity(reference) == Multiplicity::Unrestricted
        && shapes.types.get(&identity).is_some_and(|shape| {
            matches!(
                shape.shape,
                CheckedUnitStructuralTypeShape::PrimitiveScalar(_)
            )
        });
    // Constraints qualify the established endpoint without changing its loan.
    // Only represented value constraints enter this route; the formal retains
    // its qualifications and Terminal independently proves them from the exact
    // source field or parameter. No predicate is minted by this carrier join.
    if identity != target_identity
        || (!parameter_qualifications(program, &mut shapes, reference, &[])?.is_empty()
            && !validation::has_plain_owned_contents_with_represented_constraints(
                program, reference,
            ))
        || (!structural_referent && !primitive_scalar)
    {
        return None;
    }
    if exact_structural_borrow_access(
        program,
        borrow,
        machine,
        state.symbol,
        call,
        place,
        target_access,
        &[],
    )? != target_access
    {
        return None;
    }
    // Storage identity was checked against the exact referee above. The call
    // presents that storage through the formal's carrier, as ordinary calls
    // do: a fixed byte array loan uses a bounded byte view, while its owned
    // backing remains a fixed array.
    let presentation_identity = shapes.add_type(target.type_reference, &[], &[])?;
    if let CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol } = source
        && let Some((ordinal, _)) = program.state_local_declaration(state, symbol)
        && validation::reference_result_custody::primitive_local_source(program, state, ordinal)
            .is_some()
    {
        path.insert(0, checked_trees::CheckedUnitStructuralPathSegment::Referent);
    }
    Some(CheckedUnitStructuralArgumentPlan {
        source,
        path,
        type_identity: presentation_identity,
        access: target_access,
    })
}

/// A whole shared or mutable borrowed-view actual loans its established
/// view carrier: a parameter, local or projected descriptor in existing storage.
/// The reference shell is the loan itself — the same presentation the
/// call-statement lane gives a borrowed view — while this lane names the
/// view's established home. Source and access rules are identical for both
/// view families; only the carrier test and the type identity differ.
pub(in crate::execution) fn borrowed_view_argument(
    program: &TypedTrees,
    borrow: &checked_trees::BorrowFacts,
    machine: SymbolHandle,
    state: &typed_trees::state::State,
    call: &checked_trees::FlowCallFact,
    place: &crate::flow::CanonicalPlace,
    target: &StateParameter,
    selectors: Option<
        &dyn Fn(
            typed_trees::expression::ExpressionHandle,
        ) -> Option<checked_trees::CheckedUnitStructuralPathSegment>,
    >,
) -> Option<CheckedUnitStructuralArgumentPlan> {
    let facts::PlaceRoot::Symbol(symbol) = place.root else {
        return None;
    };
    let kind = if super::super::types::byte_sequence_carrier(program, target.type_reference, &[])
        .is_some_and(|carrier| carrier.is_borrowed_view())
    {
        ViewKind::Bytes
    } else {
        ViewKind::Elements
    };
    let target_access = structural_access_for_type_reference(program, target.type_reference)?;
    if !matches!(
        target_access,
        CheckedStructuralAccess::SharedBorrow | CheckedStructuralAccess::MutableBorrow
    ) {
        return None;
    }
    let is_view = |type_reference| match kind {
        ViewKind::Elements => borrowed_slice_view_element(program, type_reference, &[]).is_some(),
        ViewKind::Bytes => super::super::types::byte_sequence_carrier(program, type_reference, &[])
            .is_some_and(|carrier| carrier.is_borrowed_view()),
    };
    if !is_view(target.type_reference) {
        return None;
    }
    let identity = view_identity(program, kind, target.type_reference)?;
    let parameters = program.state_parameters(state);
    let (source, source_type) = if let Some(position) = parameters
        .iter()
        .position(|parameter| super::super::types::parameter_is_root(parameter, symbol))
    {
        let parameter = &parameters[position];
        if parameter.is_const {
            return None;
        }
        (
            CheckedUnitStructuralArgumentSourcePlan::Parameter {
                parameter_index: dense_structural_index(program, parameters, position)?,
            },
            parameter.type_reference,
        )
    } else {
        // The view local loans the referent its establishment already carries;
        // a second declaration or a moved home breaks that loan's exactness.
        let (_, local) = program
            .state_local_declaration(state, symbol)
            .filter(|&(ordinal, _)| ordinal < call.statement_index)?;
        if !local.initial_value.is_valid() {
            return None;
        }
        (
            CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol },
            local.type_reference,
        )
    };
    // The declaration identifies the storage owner; the projection identifies
    // the descriptor being lent. Whole and field views use one operation, and
    // the endpoint's own reference shell supplies access (never the containing
    // record's mutability). Source replay independently rejoins this exact path.
    let (source_type, path) = if place.segments.is_empty() {
        (source_type, Vec::new())
    } else {
        projected_evaluated_path(
            program,
            machine,
            state.symbol,
            call.statement_index,
            place,
            selectors,
        )?
    };
    let source_access = structural_access_for_type_reference(program, source_type)?;
    if !is_view(source_type)
        || !matches!(
            source_access,
            CheckedStructuralAccess::SharedBorrow | CheckedStructuralAccess::MutableBorrow
        )
        || (target_access == CheckedStructuralAccess::MutableBorrow
            && source_access != target_access)
        || view_identity(program, kind, source_type)? != identity
    {
        return None;
    }
    if exact_structural_borrow_access(
        program,
        borrow,
        machine,
        state.symbol,
        call,
        place,
        target_access,
        &[],
    )? != target_access
    {
        return None;
    }
    Some(CheckedUnitStructuralArgumentPlan {
        source,
        path,
        type_identity: identity,
        access: target_access,
    })
}

/// The dense structural-namespace index of the authored parameter at
/// `position`: the number of earlier formals the structural roster keeps.
/// The signature builders strip `[erased]` formals before splitting scalars
/// from structural values (`strips_erased_parameter`), and stage 05 replays
/// that roster, so an erased proof formal occupies no structural slot here.
fn dense_structural_index(
    program: &TypedTrees,
    parameters: &[StateParameter],
    position: usize,
) -> Option<u32> {
    let index = parameters[..position]
        .iter()
        .filter(|parameter| {
            strips_erased_parameter(parameter) != Some(true)
                && program
                    .primitive_type_reference(parameter.type_reference)
                    .is_none()
        })
        .count();
    u32::try_from(index).ok()
}

/// Resolve the established home first, then apply one owned-storage contract.
/// Copying snapshots contents; affine arguments move at the exact call occurrence.
/// Neither operation acquires disposal authority from its storage classification.
fn owned_argument(
    program: &TypedTrees,
    state: &typed_trees::state::State,
    statement_index: usize,
    source_symbol: SymbolHandle,
    target: &StateParameter,
) -> Option<CheckedUnitStructuralArgumentPlan> {
    if target.is_mutable {
        return None;
    }
    let parameters = program.state_parameters(state);
    let (source, source_type) = if let Some(position) = parameters
        .iter()
        .position(|source| source.symbol == source_symbol)
    {
        let parameter = &parameters[position];
        if parameter.is_const || parameter.is_mutable {
            return None;
        }
        (
            CheckedUnitStructuralArgumentSourcePlan::Parameter {
                parameter_index: dense_structural_index(program, parameters, position)?,
            },
            parameter.type_reference,
        )
    } else {
        let (_, local) = program
            .state_local_declaration(state, source_symbol)
            .filter(|&(ordinal, _)| ordinal < statement_index)?;
        if !program
            .expression_table
            .expression_is_valid(local.initial_value)
        {
            return None;
        }
        (
            CheckedUnitStructuralArgumentSourcePlan::StructuralLocal {
                symbol: source_symbol,
            },
            local.type_reference,
        )
    };
    let mut shapes = ShapeCollector::new(program);
    for reference in [source_type, target.type_reference] {
        if program.primitive_type_reference(reference).is_some()
            || !matches!(
                crate::checks::type_multiplicity(program, reference),
                Multiplicity::Unrestricted | Multiplicity::Affine
            )
            || !(validation::is_closed_primitive_array_type(program, reference)
                || validation::has_cleanup_owned_contents(program, reference))
            || structural_access_for_type_reference(program, reference)?
                != CheckedStructuralAccess::Owned
            || !parameter_qualifications(program, &mut shapes, reference, &[])?.is_empty()
        {
            return None;
        }
    }
    let type_identity = shapes.add_type(source_type, &[], &[])?;
    if type_identity != shapes.add_type(target.type_reference, &[], &[])?
        || crate::checks::type_multiplicity(program, source_type)
            != crate::checks::type_multiplicity(program, target.type_reference)
    {
        return None;
    }
    Some(CheckedUnitStructuralArgumentPlan {
        source,
        path: Vec::new(),
        type_identity,
        access: CheckedStructuralAccess::Owned,
    })
}

/// Borrow rows are an ordered observation roster, not a set of roots. Rebuild
/// with the existing collector so scalar reads occupy their actual positions.
fn rejoin_computation_accesses(
    program: &TypedTrees,
    borrow: &checked_trees::BorrowFacts,
    machine: SymbolHandle,
    state: SymbolHandle,
    source: &checked_trees::FlowCallFact,
) -> Option<()> {
    if !matches!(
        crate::borrow::accesses::rejoin_call_accesses(program, borrow, machine, state, source,)?,
        crate::semantic::calls::CallSite::Expression { .. }
    ) {
        return None;
    }
    for (access_ordinal, actual) in borrow
        .argument_accesses
        .span_or_empty(source.accesses)
        .iter()
        .enumerate()
    {
        if actual.kind.is_exclusive()
            && actual.end_before_access.is_none()
            && borrow
                .argument_accesses
                .span_or_empty(source.accesses)
                .iter()
                .filter(|candidate| {
                    candidate.root_symbol == actual.root_symbol
                        && !candidate
                            .end_before_access
                            .is_some_and(|end| end <= access_ordinal)
                })
                .count()
                != 1
        {
            return None;
        }
    }
    Some(())
}

fn plain_primitive_referent(
    program: &TypedTrees,
    reference: TypeReferenceHandle,
) -> Option<PrimitiveType> {
    let TypeReferenceNode::Reference { referee, .. } =
        program.type_reference_table.type_reference(reference)
    else {
        return None;
    };
    if !matches!(
        program.type_reference_table.type_reference(*referee),
        TypeReferenceNode::Named { .. }
    ) {
        return None;
    }
    program.primitive_type_reference(*referee)
}

pub(super) fn primitive_local_argument(
    program: &TypedTrees,
    borrow: &checked_trees::BorrowFacts,
    machine: SymbolHandle,
    state: &typed_trees::state::State,
    call: &checked_trees::FlowCallFact,
    place: &crate::flow::CanonicalPlace,
    target_type: TypeReferenceHandle,
) -> Option<CheckedUnitStructuralArgumentPlan> {
    let facts::PlaceRoot::Symbol(symbol) = place.root else {
        return None;
    };
    let local = super::super::structural_scalar_store::primitive_local_before(
        program,
        state,
        call.statement_index,
        symbol,
    )?;
    let target_access = structural_access_for_type_reference(program, target_type)?;
    let access = exact_structural_borrow_access(
        program,
        borrow,
        machine,
        state.symbol,
        call,
        place,
        target_access,
        &[],
    )?;
    let type_identity = base_type_identity(program, target_type, &[])?;
    if !place.segments.is_empty()
        || target_access == CheckedStructuralAccess::Owned
        || access != target_access
        || base_type_identity(program, local.type_reference, &[])? != type_identity
    {
        return None;
    }
    Some(CheckedUnitStructuralArgumentPlan {
        source: CheckedUnitStructuralArgumentSourcePlan::PrimitiveLocal { symbol },
        path: Vec::new(),
        type_identity,
        access,
    })
}
