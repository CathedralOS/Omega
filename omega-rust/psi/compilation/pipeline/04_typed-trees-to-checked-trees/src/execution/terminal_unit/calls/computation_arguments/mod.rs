//! Structural parameters, projected borrowed operands, and primitive referents
//! in scalar computations. Parameter projections reuse established storage;
//! local construction remains independently restricted to supported whole values.
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

use crate::execution::terminal_unit::calls::projected_borrowed_path;
use crate::execution::terminal_unit::calls::structural_arguments::exact_structural_borrow_access;
use crate::execution::terminal_unit::calls::view_subslice::{ViewKind, view_identity};

#[cfg(test)]
mod tests;

pub(crate) fn structural_computation_argument(
    program: &TypedTrees,
    borrow: &checked_trees::BorrowFacts,
    machine: SymbolHandle,
    state: &typed_trees::state::State,
    call: &checked_trees::FlowCallFact,
    expression: typed_trees::expression::ExpressionHandle,
    target: &StateParameter,
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
    let mut place = crate::flow::canonical_place_from_expression_in_state(
        program,
        state.symbol,
        call.statement_index,
        expression,
    )?;
    crate::flow::normalize_attached_place_root(program, machine, state.symbol, &mut place);
    rejoin_computation_accesses(program, borrow, machine, state.symbol, call)?;
    exact_actual(program, call, expression, target)?;
    if matches!(
        target_access,
        CheckedStructuralAccess::SharedBorrow | CheckedStructuralAccess::MutableBorrow
    ) && let Some(argument) =
        borrowed_nominal_argument(program, borrow, machine, state, call, &place, target)
    {
        return Some(argument);
    }
    if target.is_self {
        return None;
    }
    if let Some(argument) =
        borrowed_view_argument(program, borrow, machine, state, call, &place, target)
    {
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
        return owned_parameter_argument(program, state, name.symbol, target).or_else(|| {
            owned_copy_local_argument(program, state, call.statement_index, name.symbol, target)
        });
    }
    let target_type = plain_primitive_referent(program, target.type_reference)?;
    if super::super::structural_scalar_store::primitive_local_before(
        program,
        state,
        call.statement_index,
        name.symbol,
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
    let parameters = program.state_parameters(state);
    let source_position = parameters
        .iter()
        .position(|source| source.symbol == name.symbol)?;
    let source = &parameters[source_position];
    if source.is_self
        || source.is_const
        || plain_primitive_referent(program, source.type_reference)? != target_type
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
    borrow: &checked_trees::BorrowFacts,
    machine: SymbolHandle,
    state: &typed_trees::state::State,
    call: &checked_trees::FlowCallFact,
    expression: typed_trees::expression::ExpressionHandle,
    target: &StateParameter,
    argument_ordinal: u32,
) -> Option<CheckedUnitStructuralArgumentPlan> {
    if target.is_self || target.is_const {
        return None;
    }
    exact_actual(program, call, expression, target)?;
    rejoin_computation_accesses(program, borrow, machine, state.symbol, call)?;
    let owner = program
        .machines()
        .iter()
        .find(|owner| owner.symbol == machine)?;
    let mut place = crate::flow::canonical_place_from_expression_in_state(
        program,
        state.symbol,
        call.statement_index,
        expression,
    )?;
    crate::flow::normalize_attached_place_root(program, machine, state.symbol, &mut place);
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
    let target_state = crate::semantic::calls::find_state(program, call.target_symbol)?;
    let target_position = program
        .state_parameters(target_state)
        .iter()
        .position(|parameter| parameter == target)?;
    let ExpressionNode::Call(authored) = program
        .expression_table
        .expression(call.authored_expression)
    else {
        return None;
    };
    let target_parameters = program.state_parameters(target_state);
    let receiver_count = target_parameters
        .iter()
        .filter(|parameter| parameter.is_self)
        .count();
    let exact_actual = if target.is_self {
        receiver_count == 1 && target_position == 0 && authored.receiver == expression
    } else {
        program
            .expression_table
            .expression_handles(authored.arguments)
            .get(target_position.checked_sub(receiver_count)?)
            == Some(&expression)
    };
    if authored.target_symbol != call.target_symbol || !exact_actual {
        return None;
    }
    Some(())
}

/// A nominal argument borrows its existing parameter or local result
/// home. Initializer spelling does not select custody; publication independently
/// rejoins this symbol to its dominating structural establishment.
pub(crate) fn borrowed_nominal_argument(
    program: &TypedTrees,
    borrow: &checked_trees::BorrowFacts,
    machine: SymbolHandle,
    state: &typed_trees::state::State,
    call: &checked_trees::FlowCallFact,
    place: &crate::flow::CanonicalPlace,
    target: &StateParameter,
) -> Option<CheckedUnitStructuralArgumentPlan> {
    let facts::PlaceRoot::Symbol(symbol) = place.root else {
        return None;
    };
    // A receiver argument binds `self` to `self` across machines: both sides'
    // receiver referees are machine-keyed `Named { machine, "Self" }` nodes,
    // so nominal identity can never join them. The join key is the shared
    // attachment data, resolved from its declaration rather than an interned
    // Named (a machine that never names its attachment elsewhere has none).
    let self_receiver = if target.is_self {
        crate::semantic::calls::find_machine_by_entry_state(program, call.target_symbol)
    } else {
        None
    };
    let self_attached = self_receiver.and_then(|(owner, _)| {
        program
            .data_definitions()
            .iter()
            .find(|data| data.symbol == owner.attached_data_symbol)
    });
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
    let target_identity = if let Some(attached) = self_attached {
        super::super::types::closed_data_identity(program, attached, &[])
    } else if target.is_self {
        let (owner, _) =
            crate::semantic::calls::find_machine_by_entry_state(program, call.target_symbol)?;
        let reference = program
            .type_reference_table
            .find_named_type_reference(owner.attached_data_symbol)?;
        base_type_identity(program, reference, &[])?
    } else {
        base_type_identity(program, referee, &[])?
    };
    let mut receiver_argument = false;
    let parameters = program.state_parameters(state);
    let (reference, source_carrier, source) = if let Some(position) =
        parameters.iter().position(|parameter| {
            parameter.symbol == symbol || (parameter.is_self && symbol == machine)
        }) {
        let parameter = &parameters[position];
        if parameter.is_const {
            return None;
        }
        let mut reference = match program.type_reference_table.type_reference(
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
                    || parameter.is_mutable
                    || parameter.is_self =>
            {
                parameter.type_reference
            }
            _ => return None,
        };
        if parameter.is_self {
            let owner = (crate::lookup::machine_by_symbol(program, machine))?;
            let TypeReferenceNode::Named { symbol, .. } =
                program.type_reference_table.type_reference(reference)
            else {
                return None;
            };
            if *symbol != owner.symbol && *symbol != owner.attached_data_symbol {
                return None;
            }
            if let Some((callee_owner, _)) = self_receiver {
                // Receiver-to-receiver on the whole attachment: the caller's
                // `self` may loan only the attachment the callee's `self`
                // formal denotes. A projected receiver (`self.field.m()`)
                // binds the member's own attachment instead, so the whole-
                // storage check below handles it through the normal path.
                if place.segments.is_empty() {
                    if owner.attached_data_symbol != callee_owner.attached_data_symbol {
                        return None;
                    }
                    receiver_argument = true;
                }
            }
            if !receiver_argument && place.segments.is_empty() {
                // Unprojected `self` names the whole attachment; canonicalize
                // to the interned data Named when one exists.
                match program
                    .type_reference_table
                    .find_named_type_reference(owner.attached_data_symbol)
                {
                    Some(found) => reference = found,
                    None => {
                        return None;
                    }
                }
            }
        }
        let parameter_index = dense_structural_index(program, parameters, position)?;
        (
            reference,
            validation::reference_shell(program, parameter.type_reference).is_some(),
            CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index },
        )
    } else {
        // A projection borrows the same established local home. Resolve its
        // endpoint below while retaining the declaring local as the owner.
        let (_, local) = program
            .state_local_declaration(state, symbol)
            .filter(|&(ordinal, _)| ordinal < call.statement_index)?;
        // A mutable loan needs authority from the source binding. Filling an
        // owned local field does not by itself authorize lending the whole local.
        if !local.initial_value.is_valid() {
            return None;
        }
        // An established shared-borrow local already loans the referent it was
        // bound to; forwarding it hands that referent carrier to the callee,
        // exactly as a shared-borrow parameter would. The retained symbol and
        // recorded access stay the local's own — nothing unwraps or copies the
        // referent into a fabricated place. A fixed-array local is the same
        // kind of established home: its bounds come from the literal length
        // when a segment projects inside it.
        let (reference, carrier) = match program.type_reference_table.type_reference(
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
                (*referee, true)
            }
            TypeReferenceNode::Named { .. }
            | TypeReferenceNode::Generic { .. }
            | TypeReferenceNode::FixedArray { .. }
                if target_access == CheckedStructuralAccess::SharedBorrow || local.is_mutable =>
            {
                (local.type_reference, false)
            }
            _ => return None,
        };
        (
            reference,
            carrier,
            CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol },
        )
    };
    let whole_storage = place.segments.is_empty()
        || matches!(
            source,
            CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { .. }
        );
    if receiver_argument {
        // The receiver's storage is the attachment itself: its declaration's
        // multiplicity is the whole-storage evidence, and the shape collected
        // below from the same declaration retains its complete field closure.
        if !matches!(
            self_attached.map(|data| data.properties.multiplicity),
            Some(Multiplicity::Affine | Multiplicity::Unrestricted)
        ) {
            return None;
        }
    } else if whole_storage
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
    let (reference, path) = if place.segments.is_empty() {
        (reference, Vec::new())
    } else {
        projected_borrowed_path(program, machine, state.symbol, call.statement_index, place)?
    };
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
    let identity = if let Some((callee_owner, _)) = self_receiver {
        (shapes.add_attached_application(callee_owner, &[]))?
    } else {
        (shapes.add_type(reference, &[], &[]))?
    };
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
    // A whole primitive referent observes the same existing storage when the
    // forwarded source is itself a `&T` carrier. The loan names the carrier's
    // place and access rows directly; nothing materializes an owned scalar
    // copy of the referent into a fabricated place. A `&scratch` borrow of a
    // primitive storage local or scalar parameter stays on its dedicated
    // primitive source plan instead.
    let whole_primitive_scalar = source_carrier
        && path.is_empty()
        && shapes.types.len() == 1
        && program.type_multiplicity(reference) == Multiplicity::Unrestricted
        && shapes.types.get(&identity).is_some_and(|shape| {
            matches!(
                shape.shape,
                CheckedUnitStructuralTypeShape::PrimitiveScalar(_)
            )
        });
    // A whole-attachment receiver loan forwards the caller's `self` as the
    // callee's borrowed receiver: custody stays with the machine's storage.
    // The statement lane performs the same receiver-to-receiver binding without
    // reading the field closure. The is_self tail below still verifies the
    // authored receiver place and rules out an overlapping exclusive actual;
    // other loans retain the supported closure checked above.
    // Constraints qualify the established endpoint without changing its loan.
    // Only represented value constraints enter this route; the formal retains
    // its qualifications and Terminal independently proves them from the exact
    // source field or parameter. No predicate is minted by this carrier join.
    if identity != target_identity
        || (!parameter_qualifications(program, &mut shapes, reference, &[])?.is_empty()
            && !validation::has_plain_owned_contents_with_represented_constraints(
                program, reference,
            ))
        || (!receiver_argument && !structural_referent && !whole_primitive_scalar)
    {
        return None;
    }
    if target.is_self {
        let site = crate::semantic::calls::find_call_site(
            program,
            machine,
            state.symbol,
            call.statement_index,
            call.call_ordinal,
        )?;
        let mut receiver = crate::flow::canonical_receiver_place_for_call_site(
            program,
            machine,
            state.symbol,
            &site,
            call.statement_index,
        )?;
        crate::flow::normalize_attached_place_root(program, machine, state.symbol, &mut receiver);
        // Captured self retains the machine namespace; contextual expression
        // resolution uses its actual formal. Normalize only that exact pair,
        // leaving every projected field and unrelated root unchanged.
        if receiver.root == facts::PlaceRoot::Symbol(machine) {
            receiver.root = facts::PlaceRoot::Symbol(
                parameters
                    .iter()
                    .find(|parameter| parameter.is_self)?
                    .symbol,
            );
        }
        let expected_root = if place.root == facts::PlaceRoot::Symbol(machine) {
            facts::PlaceRoot::Symbol(
                parameters
                    .iter()
                    .find(|parameter| parameter.is_self)?
                    .symbol,
            )
        } else {
            place.root
        };
        if !call.has_receiver
            || receiver.root != expected_root
            || receiver.segments != place.segments
        {
            return None;
        }
        // The receiver is retained separately from the explicit access roster.
        // Shared loans exclude exclusive actuals; mutable loans exclude every
        // simultaneous observation of the same root.
        if borrow
            .argument_accesses
            .span(call.accesses)?
            .iter()
            .any(|access| {
                access.root_symbol == symbol
                    && (target_access == CheckedStructuralAccess::MutableBorrow
                        || access.kind.is_exclusive())
            })
        {
            return None;
        }
    } else if exact_structural_borrow_access(
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
    // backing remains a fixed array. Receivers keep their attachment identity.
    let presentation_identity = if target.is_self {
        identity
    } else {
        shapes.add_type(target.type_reference, &[], &[])?
    };
    let mut path = path;
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
    let (source, source_type) = if let Some(position) = parameters.iter().position(|parameter| {
        super::parameter_argument_paths::parameter_is_root(machine, parameter, symbol)
    }) {
        let parameter = &parameters[position];
        if parameter.is_const || (parameter.is_self && place.segments.is_empty()) {
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
        projected_borrowed_path(program, machine, state.symbol, call.statement_index, place)?
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

fn owned_copy_local_argument(
    program: &TypedTrees,
    state: &typed_trees::state::State,
    statement_index: usize,
    source_symbol: SymbolHandle,
    target: &StateParameter,
) -> Option<CheckedUnitStructuralArgumentPlan> {
    if target.is_mutable
        || program
            .state_parameters(state)
            .iter()
            .any(|parameter| parameter.symbol == source_symbol)
    {
        return None;
    }
    let (_, local) = program
        .state_local_declaration(state, source_symbol)
        .filter(|&(ordinal, _)| ordinal < statement_index)?;
    // Scalar operand sequencing snapshots the current contents before later
    // arguments run. The binding may mutate; the copied value cannot alias it.
    if !program
            .expression_table
            .expression_is_valid(local.initial_value)
        // Copying a closed plain-owned local preserves its established storage
        // regardless of whether its carrier is an array, record, or case.
        // Affine moves need their own transfer receipts and remain excluded.
        || [local.type_reference, target.type_reference]
            .iter()
            .any(|reference| {
                program.primitive_type_reference(*reference).is_some()
                    || !validation::has_plain_owned_contents(program, *reference)
                    || crate::checks::type_multiplicity(program, *reference)
                        != Multiplicity::Unrestricted
            })
    {
        return None;
    }
    let mut shapes = ShapeCollector::new(program);
    let type_identity = shapes.add_type(local.type_reference, &[], &[])?;
    if type_identity != shapes.add_type(target.type_reference, &[], &[])? {
        return None;
    }
    Some(CheckedUnitStructuralArgumentPlan {
        source: CheckedUnitStructuralArgumentSourcePlan::StructuralLocal {
            symbol: source_symbol,
        },
        path: Vec::new(),
        type_identity,
        access: CheckedStructuralAccess::Owned,
    })
}

fn owned_parameter_argument(
    program: &TypedTrees,
    state: &typed_trees::state::State,
    source_symbol: SymbolHandle,
    target: &StateParameter,
) -> Option<CheckedUnitStructuralArgumentPlan> {
    let parameters = program.state_parameters(state);
    let source_position = parameters
        .iter()
        .position(|parameter| parameter.symbol == source_symbol)?;
    let source = &parameters[source_position];
    let mut shapes = ShapeCollector::new(program);
    for parameter in [source, target] {
        if parameter.is_self
            || parameter.is_const
            || parameter.is_mutable
            || !(matches!(
                program
                    .type_reference_table
                    .type_reference(parameter.type_reference),
                TypeReferenceNode::Named { .. }
            ) || validation::is_closed_primitive_array_type(program, parameter.type_reference))
            || program
                .primitive_type_reference(parameter.type_reference)
                .is_some()
            || !matches!(
                crate::checks::type_multiplicity(program, parameter.type_reference),
                Multiplicity::Unrestricted | Multiplicity::Affine
            )
            || !(validation::is_closed_primitive_array_type(program, parameter.type_reference)
                || validation::has_plain_owned_contents_with_numeric_constraints(
                    program,
                    parameter.type_reference,
                ))
            || structural_access_for_type_reference(program, parameter.type_reference)?
                != CheckedStructuralAccess::Owned
            || !parameter_qualifications(program, &mut shapes, parameter.type_reference, &[])?
                .is_empty()
        {
            return None;
        }
    }
    let type_identity = shapes.add_type(source.type_reference, &[], &[])?;
    if type_identity != shapes.add_type(target.type_reference, &[], &[])?
        || crate::checks::type_multiplicity(program, source.type_reference)
            != crate::checks::type_multiplicity(program, target.type_reference)
    {
        return None;
    }
    let parameter_index = dense_structural_index(program, parameters, source_position)?;
    Some(CheckedUnitStructuralArgumentPlan {
        source: CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index },
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
