//! Structural homes, projected borrowed operands, and primitive referents
//! in scalar computations. Whole owned operands use the same storage contract
//! for parameters and locals. Their call occurrence retains identity until
//! checked ownership supplies copy or transfer authority; a nominal hook changes
//! the receiving owner's cleanup, not the argument's storage representation.
use crate::execution::terminal_unit::CheckedStructuralAccess;
use crate::execution::terminal_unit::CheckedUnitStructuralArgumentPlan;
use crate::execution::terminal_unit::CheckedUnitStructuralArgumentSourcePlan;
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

use crate::execution::terminal_unit::calls::argument_paths::projected_evaluated_path;
use crate::execution::terminal_unit::calls::structural_arguments::exact_structural_borrow_access;
use crate::execution::terminal_unit::calls::view_subslice::{ViewKind, view_identity};

pub(crate) fn structural_computation_argument(
    program: &dyn ::validation::CheckingInput,
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
    let place = validation::canonical_place_from_expression_in_state(
        program,
        state.symbol,
        call.statement_index,
        expression,
    )?;

    rejoin_computation_accesses(program, borrow, machine, state.symbol, call)?;
    exact_actual(program, call, expression, target)?;
    // Primitive loans retain the declaration's original home. Binding
    // replacement mutability selects its existing value/storage carrier,
    // never the access that the exact borrow occurrence may lend.
    if let Some(symbol) = place.root.as_symbol()
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
            &place,
            target.type_reference,
        );
    }
    if matches!(
        target_access,
        CheckedStructuralAccess::SharedBorrow
            | CheckedStructuralAccess::MutableBorrow
            | CheckedStructuralAccess::WriteOnlyBorrow
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
    if target_access == CheckedStructuralAccess::Owned {
        if named != expression {
            return None;
        }
        // Whole and projected operands name the same established home. The
        // typed path, not name-expression syntax, selects the transferred
        // subtree; ownership events separately supply its live claims.
        return owned_argument(
            program,
            machine,
            state,
            call.statement_index,
            &place,
            target,
            selectors,
        );
    }
    let name = program.expression_table.name_path_node(named)?;
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
        type_identity: base_type_identity(program, target.type_reference, &[], &[])?,
        access,
    })
}

/// The range is still an actual of this exact checked call, not merely an
/// equal-typed view expression. Its bounds use dense explicit structural
/// ordinals, which exclude the implicit receiver.
pub(crate) fn subslice_computation_argument(
    program: &dyn ::validation::CheckingInput,
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
    let owner = program.machine(machine)?;
    let place = validation::canonical_place_from_expression_in_state(
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
        super::ambient_self_scalar_signature(program, owner, state)?.0
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
    program: &dyn ::validation::CheckingInput,
    call: &checked_trees::FlowCallFact,
    expression: typed_trees::expression::ExpressionHandle,
    target: &StateParameter,
) -> Option<()> {
    let target_position = validation::call_target_parameters(program, call.target_symbol)?
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
    program: &dyn ::validation::CheckingInput,
    borrow: &checked_trees::BorrowFacts,
    machine: SymbolHandle,
    state: &typed_trees::state::State,
    call: &checked_trees::FlowCallFact,
    place: &validation::CanonicalPlace,
    target: &StateParameter,
    selectors: Option<
        &dyn Fn(
            typed_trees::expression::ExpressionHandle,
        ) -> Option<checked_trees::CheckedUnitStructuralPathSegment>,
    >,
) -> Option<CheckedUnitStructuralArgumentPlan> {
    let symbol = place.root.as_symbol()?;
    // A borrowed formal retains the selected record at its existing root/path.
    // Receiver syntax does not confer additional projection authority; both
    // lanes rejoin exact source type and the captured access occurrence below.
    let target_shell = validation::reference_shell(program, target.type_reference)?;
    let target_access = match target_shell.access {
        language_semantics::ReferenceAccess::Shared => CheckedStructuralAccess::SharedBorrow,
        language_semantics::ReferenceAccess::Mutable => CheckedStructuralAccess::MutableBorrow,
        language_semantics::ReferenceAccess::WriteOnly => CheckedStructuralAccess::WriteOnlyBorrow,
    };
    // Referent shape, contents, identity and the exact loan below decide
    // admission. Primitive leaves and aggregates borrow the same original
    // home; declaration spelling is not an additional eligibility rule.
    let parameters = program.state_parameters(state);
    let (reference, mut source) = if let Some(position) =
        typed_trees::signature::state_parameter_position(parameters, symbol)
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
            } if borrowed_access_supplies(*access, target_shell.access) => *referee,
            TypeReferenceNode::Named { .. }
            | TypeReferenceNode::Generic { .. }
            | TypeReferenceNode::FixedArray { .. } => parameter.type_reference,
            _ => return None,
        };
        let source = if program
            .primitive_type_reference(parameter.type_reference)
            .is_some()
        {
            if !place.segments.is_empty()
                || (target_access != CheckedStructuralAccess::SharedBorrow && !parameter.is_mutable)
            {
                return None;
            }
            let parameter_index = u32::try_from(
                parameters[..position]
                    .iter()
                    .filter(|parameter| {
                        !parameter.is_const
                            && strips_erased_parameter(parameter) != Some(true)
                            && program
                                .primitive_type_reference(parameter.type_reference)
                                .is_some()
                    })
                    .count(),
            )
            .ok()?;
            CheckedUnitStructuralArgumentSourcePlan::ScalarParameter { parameter_index }
        } else {
            let parameter_index = dense_structural_index(program, parameters, position)?;
            CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index }
        };
        (reference, source)
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
            } if (target_access == CheckedStructuralAccess::SharedBorrow || !local.is_mutable)
                && borrowed_access_supplies(*access, target_shell.access) =>
            {
                *referee
            }
            TypeReferenceNode::Named { .. }
            | TypeReferenceNode::Generic { .. }
            | TypeReferenceNode::FixedArray { .. } => local.type_reference,
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
    // The logical root is checked here, while its physical carrier is only
    // known after the dominating operation publishes storage. Keep the root
    // with every local loan; initializer spelling cannot decide whether that
    // carrier needs a pointer crossing (ordinary call results need it too).
    let mut shapes = ShapeCollector::new(program);
    if let CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol } = source {
        source = CheckedUnitStructuralArgumentSourcePlan::StructuralLocalBorrow {
            symbol,
            root_type_identity: shapes.add_type(reference, &[], &[])?,
        };
    }
    let (reference, path) = if place.segments.is_empty() {
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
    let (reference, path) = super::argument_paths::borrowed_argument_endpoint(
        program,
        reference,
        path,
        target.type_reference,
    )?;
    // Access belongs to the exact checked loan below, not just the root's
    // mutability. An immutable aggregate can own a mutable descriptor whose
    // backing is loaned by this call; borrowing that descriptor's storage or
    // the aggregate itself still needs its separate checked permission.
    // Eligibility belongs to the borrowed endpoint, not its unrelated siblings.
    // Root membership/access and the exact authored receiver loan are retained
    // independently; this never constructs or copies the enclosing record.
    let identity = shapes.add_type(reference, &[], &[])?;
    // Borrow the established endpoint in place, including its shared view and
    // provider members. Contents are classified recursively from authored types:
    // the old shape whitelist erased slice access and made valid loans depend
    // on which field families appeared together. This grants no copy, provider
    // establishment or extended backing lifetime; exact source/type/access and
    // call-occurrence replay still own those obligations.
    // A projected receiver still loans the complete selected referent. Its
    // transitive field closure is identical to a whole parameter's; the exact
    // root/path checks above retain custody of the enclosing storage.
    // Lending neither copies nor transfers the owner's linear custody. The
    // same exact loan applies to whole and projected established referents.
    let borrowed_referent = shapes.types.contains_key(&identity)
        && validation::has_contents(
            program,
            reference,
            &[],
            validation::ContentsRequirement::BorrowedStorage,
        );
    // Constraints qualify the established endpoint without changing its loan.
    // Only represented value constraints enter this route; the formal retains
    // its qualifications and Terminal independently proves them from the exact
    // source field or parameter. No predicate is minted by this carrier join.
    let presentation_identity = shapes.add_type(target.type_reference, &[], &[])?;
    // Calls share representation admission for every selected endpoint,
    // including a case payload. This preserves the exact root and path; it
    // grants no access or predicate evidence.
    super::argument_presentation::classify(
        program,
        reference,
        target.type_reference,
        presentation_identity,
        &[],
    )?;
    // The evaluated path proves active cases and initialized storage; the
    // exact loan below proves access. Qualification does not change backing.
    if (!parameter_qualifications(program, &mut shapes, reference, &[])?.is_empty()
        && !validation::has_contents(
            program,
            reference,
            &[],
            validation::ContentsRequirement::RepresentedOwned,
        ))
        || !borrowed_referent
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
    Some(CheckedUnitStructuralArgumentPlan {
        source,
        path,
        type_identity: presentation_identity,
        access: target_access,
    })
}

/// A storage projection attenuates the existing loan; it never acquires
/// readable authority from a write-only parent. Exact call-occurrence evidence
/// below remains responsible for ownership, suspension and restoration.
fn borrowed_access_supplies(
    supplied: language_semantics::ReferenceAccess,
    requested: language_semantics::ReferenceAccess,
) -> bool {
    supplied == requested || supplied == language_semantics::ReferenceAccess::Mutable
}

/// A whole shared or mutable borrowed-view actual loans its established
/// view carrier: a parameter, local or projected descriptor in existing storage.
/// The reference shell is the loan itself — the same presentation the
/// call-statement lane gives a borrowed view — while this lane names the
/// view's established home. Source and access rules are identical for both
/// view families; only the carrier test and the type identity differ.
pub(in crate::execution) fn borrowed_view_argument(
    program: &dyn ::validation::CheckingInput,
    borrow: &checked_trees::BorrowFacts,
    machine: SymbolHandle,
    state: &typed_trees::state::State,
    call: &checked_trees::FlowCallFact,
    place: &validation::CanonicalPlace,
    target: &StateParameter,
    selectors: Option<
        &dyn Fn(
            typed_trees::expression::ExpressionHandle,
        ) -> Option<checked_trees::CheckedUnitStructuralPathSegment>,
    >,
) -> Option<CheckedUnitStructuralArgumentPlan> {
    let symbol = place.root.as_symbol()?;
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
        checked_trees::checked_borrowed_access_pair_pattern!()
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
            checked_trees::checked_borrowed_access_pair_pattern!()
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
    program: &dyn ::validation::CheckingInput,
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
    program: &dyn ::validation::CheckingInput,
    machine: SymbolHandle,
    state: &typed_trees::state::State,
    statement_index: usize,
    place: &validation::CanonicalPlace,
    target: &StateParameter,
    selectors: Option<
        &dyn Fn(
            typed_trees::expression::ExpressionHandle,
        ) -> Option<checked_trees::CheckedUnitStructuralPathSegment>,
    >,
) -> Option<CheckedUnitStructuralArgumentPlan> {
    if target.is_mutable {
        return None;
    }
    let parameters = program.state_parameters(state);
    let source_symbol = place.root.as_symbol()?;
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
        // A declared local is a structural home whether its declaration
        // carried an initializer or its fields were established by later
        // stores; the statement route admits both the same way, and the
        // ownership events, not the declaration's syntax, prove the operand
        // is established at this call.
        let (_, local) = program
            .state_local_declaration(state, source_symbol)
            .filter(|&(ordinal, _)| ordinal < statement_index)?;
        (
            CheckedUnitStructuralArgumentSourcePlan::StructuralLocal {
                symbol: source_symbol,
            },
            local.type_reference,
        )
    };
    // Owned projection cannot cross a reference to acquire someone else's
    // referent. The root and endpoint both retain owned storage contracts.
    if structural_access_for_type_reference(program, source_type)? != CheckedStructuralAccess::Owned
    {
        return None;
    }
    let (selected_type, path) = projected_evaluated_path(
        program,
        machine,
        state.symbol,
        statement_index,
        place,
        selectors,
    )?;
    if path.iter().any(|segment| {
        matches!(
            segment,
            checked_trees::CheckedUnitStructuralPathSegment::Referent
        )
    }) {
        return None;
    }
    let mut shapes = ShapeCollector::new(program);
    for reference in [selected_type, target.type_reference] {
        // A call consumes established storage, not cleanup-free contents.
        // Shape collection below retains reference descriptors as leaves;
        // the exact call's ownership and loan events authorize their transfer.
        // Requiring CleanupOwned here discarded otherwise valid carriers only
        // when their consuming call was nested inside a scalar computation.
        // Checking proved any membership the formal names, so qualifications
        // ride the formal exactly as on the statement route and the borrowed
        // route: the carrier beneath them is the storage, and the qualified
        // type must remain represented owned storage.
        let qualified = !parameter_qualifications(program, &mut shapes, reference, &[])?.is_empty();
        let storage = if qualified {
            validation::peel_declared_constraints(program, reference)
        } else {
            reference
        };
        if program.primitive_type_reference(storage).is_some()
            || structural_access_for_type_reference(program, reference)?
                != CheckedStructuralAccess::Owned
            || (qualified
                && !validation::has_contents(
                    program,
                    reference,
                    &[],
                    validation::ContentsRequirement::RepresentedOwned,
                ))
        {
            return None;
        }
    }
    // The formal keeps its qualifications; the transferred storage is the
    // same declaration on both sides, as on the statement call route.
    let type_identity = base_type_identity(program, selected_type, &[], &[])?;
    if type_identity != base_type_identity(program, target.type_reference, &[], &[])?
        || crate::checks::type_multiplicity(program, selected_type)
            != crate::checks::type_multiplicity(program, target.type_reference)
    {
        return None;
    }
    Some(CheckedUnitStructuralArgumentPlan {
        source,
        path,
        type_identity,
        access: CheckedStructuralAccess::Owned,
    })
}

/// Borrow rows are an ordered observation roster, not a set of roots. Rebuild
/// with the existing collector so scalar reads occupy their actual positions.
fn rejoin_computation_accesses(
    program: &dyn ::validation::CheckingInput,
    borrow: &checked_trees::BorrowFacts,
    machine: SymbolHandle,
    state: SymbolHandle,
    source: &checked_trees::FlowCallFact,
) -> Option<()> {
    if !matches!(
        crate::borrow::accesses::rejoin_call_accesses(program, borrow, machine, state, source,)?,
        validation::CallSite::Expression { .. }
    ) {
        return None;
    }
    // Replay the exact ordered access roster here. Spatial compatibility is
    // owned by checks::borrows for every call, including computation calls.
    // Counting a root's simultaneous accesses loses their captured paths and
    // rejects valid disjoint field loans; do not re-decide that judgment here.
    Some(())
}

fn plain_primitive_referent(
    program: &dyn ::validation::CheckingInput,
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

/// Bind the resolved storage home while retaining the authored access occurrence.
/// An erased reference alias names the same home, but its declaration is a
/// descriptor rather than the primitive owner. Using that descriptor for the
/// storage/type lookup rejects valid reborrows; using the owner for access would
/// instead lose the exact loan that authorizes the call.
pub(super) fn primitive_local_argument(
    program: &dyn ::validation::CheckingInput,
    borrow: &checked_trees::BorrowFacts,
    machine: SymbolHandle,
    state: &typed_trees::state::State,
    call: &checked_trees::FlowCallFact,
    storage: &validation::CanonicalPlace,
    access_place: &validation::CanonicalPlace,
    target_type: TypeReferenceHandle,
) -> Option<CheckedUnitStructuralArgumentPlan> {
    let symbol = storage.root.as_symbol()?;
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
        access_place,
        target_access,
        &[],
    )?;
    let type_identity = base_type_identity(program, target_type, &[], &[])?;
    if !storage.segments.is_empty()
        || target_access == CheckedStructuralAccess::Owned
        || access != target_access
        || base_type_identity(program, local.type_reference, &[], &[])? != type_identity
    {
        return None;
    }
    Some(CheckedUnitStructuralArgumentPlan {
        source: if local.is_mutable {
            CheckedUnitStructuralArgumentSourcePlan::PrimitiveLocal { symbol }
        } else {
            CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol }
        },
        path: Vec::new(),
        type_identity,
        access,
    })
}
