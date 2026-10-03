//! Projected store planning for parameter and local storage.
//!
//! One authored assignment `place = value` is one composition. The
//! destination (`destination.rs`) is resolved from the target alone: its root
//! and that root's write authority, the checked carrier path, and the leaf the
//! path ends at. The value (`value.rs`) is resolved from the right-hand side
//! alone: the already-checked scalar source at the value's own coordinate --
//! a computation root, a bound pure expression, or the result of the call the
//! statement performs. The leaf's declared type then picks the Terminal store
//! operation that joins them:
//!
//! - a primitive field takes one `StructuralScalarFieldStore`;
//! - a primitive-array place takes a closed array literal as one element
//!   store per element (`primitive.rs`);
//! - a payload-free sum field takes a whole owned parameter of its type;
//! - a bounded-owned byte field or a whole borrowed byte view takes the byte
//!   store operations of `byte_stores.rs`.
//!
//! Whole record assignments stay with ordinary structural construction and
//! replacement in `control::statement_sequence`: operands complete before
//! destination mutation, regardless of whether the destination is a root,
//! field, or indexed element. This planner does not decompose constructors.
//!
//! What remains refused is a custody or proof condition, not an arrangement:
//! write frames, linear or qualified roots, reference crossings, and fields
//! whose declared domain the stored value carries no evidence for.
use super::{
    CheckFacts, CheckedScalarExpressionRole, CheckedStructuralAccess,
    CheckedStructuralScalarFieldStorePlan, CheckedStructuralScalarParameterPlan,
    CheckedUnitEffectOperationPlan, CheckedUnitStructuralParameterPlan,
    CheckedUnitStructuralPathSegment, DataMember, DataShapeKind, ExpressionNode, Multiplicity,
    PrimitiveType, StatementNode, TypeReferenceNode, TypedTrees,
};
use crate::execution::terminal_unit::control::LocalConstructionTrace;
use crate::execution::terminal_unit::types::{byte_sequence_carrier, terminal_field_identity};
use destination::{Destination, Root, StoreRoots, exact_relevant_field, plain_record};
use value::AssignmentSource;

pub(in crate::execution::terminal_unit) use primitive::{
    checked_unit_path, primitive_local_before, scalar_custody_is_exact,
};
pub(in crate::execution::terminal_unit) use selectors::TargetSelectors;

mod byte_stores;
pub(in crate::execution::terminal_unit) use byte_stores::byte_value_plan;
mod destination;
mod frame;
mod primitive;
mod selectors;
#[cfg(test)]
mod tests;
mod value;
mod view_field_store;
mod view_stores;

/// The scalar-graph lane's field store into one of its own record locals.
pub(in crate::execution) fn build_local_scalar_field_store(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    statement_index: u32,
    assignment: &typed_trees::statement::TableAssignment,
) -> Option<CheckedStructuralScalarFieldStorePlan> {
    let stores = plan_assignment(
        program,
        facts,
        machine,
        state,
        StoreRoots::RecordLocals,
        statement_index,
        assignment,
        AssignmentSource::Authored,
        &LocalConstructionTrace::default(),
    )?;
    let [CheckedUnitEffectOperationPlan::StructuralScalarFieldStore(store)] =
        <[_; 1]>::try_from(stores).ok()?
    else {
        return None;
    };
    Some(store)
}

/// The ordered stores of one attached Unit body, from `statement_start` on.
/// The state's complete write frame is replayed once here; each assignment
/// then composes its destination and value.
pub(super) fn build_structural_scalar_field_store_sequence_traced(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    structural_parameters: &[CheckedUnitStructuralParameterPlan],
    scalar_parameters: &[CheckedStructuralScalarParameterPlan],
    statement_start: usize,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
    trace: &LocalConstructionTrace,
) -> Option<Vec<CheckedUnitEffectOperationPlan>> {
    let statements = program.statement_table.statements(state.statement_nodes);
    if !statements
        .iter()
        .skip(statement_start)
        .any(|statement| matches!(statement, StatementNode::Assignment(_)))
    {
        return Some(Vec::new());
    }
    trace.phase("scalar field store sequence: state write frame");
    let frame = &facts
        .mutation
        .for_machine(machine.symbol)?
        .state_write_frames
        .iter()
        .find(|frame| frame.state == state.symbol)?
        .frame;
    trace.phase("scalar field store sequence: write frame agreement");
    if !frame::matches(program, machine, state, frame, call_frames) {
        return None;
    }
    let roots = StoreRoots::Parameters {
        structural: structural_parameters,
        scalar: scalar_parameters,
    };
    let mut stores = Vec::new();
    for (statement_index, statement) in statements.iter().enumerate().skip(statement_start) {
        let StatementNode::Assignment(assignment) = statement else {
            continue;
        };
        // An atomic carrier is one atomic event, planned by the sequence
        // (`atomic_operations.rs`), never a store of its arithmetic model.
        if validation::atomic_assignment_carrier(program, assignment).is_some() {
            continue;
        }
        let statement_index = u32::try_from(statement_index).ok()?;
        trace.phase("scalar field store sequence: assignment store");
        trace.statement(Some(statement_index));
        if let Some(store) = primitive::store_at(
            program,
            facts,
            machine,
            state,
            structural_parameters,
            statement_index,
            assignment,
        ) {
            stores.push(store);
            continue;
        }
        if let Some(elements) = primitive::array_literal_stores(
            program,
            facts,
            machine,
            state,
            structural_parameters,
            statement_index,
            assignment,
        ) {
            stores.extend(elements);
            continue;
        }
        let baseline = trace.mark();
        if let Some(planned) = plan_assignment(
            program,
            facts,
            machine,
            state,
            roots,
            statement_index,
            assignment,
            AssignmentSource::Authored,
            trace,
        ) {
            stores.extend(planned);
            continue;
        }
        // An assignment whose source is this statement's own call has no
        // authored value to store yet. Its call operation is sequenced with
        // the other calls, and the store consuming that result is appended
        // there; it deliberately produces no row here. A whole structural
        // local as the source is likewise the sequence's own business: it is
        // the repair store of a borrowed-storage window. A constructed
        // structural value the stores above do not decompose is established
        // by the sequence too, and replaces its field through the same window
        // pair a structural call result uses.
        if matches!(
            program.expression_table.expression(assignment.value),
            ExpressionNode::Call(_)
        ) || restores_structural_local(program, state, assignment)
            || bound_structural_parameter(
                program,
                state,
                structural_parameters,
                statement_index,
                assignment,
            )
            || facts
                .values
                .structural_values
                .root_for_expression(state.symbol, statement_index, assignment.value)
                .is_some()
        {
            trace.restore(&baseline);
            continue;
        }
        trace.statement(Some(statement_index));
        return None;
    }
    Some(stores)
}

/// The store consuming the scalar result of the call this same statement
/// performs: the same composition, with the call's SSA result as the value.
pub(super) fn build_structural_call_result_field_store(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    structural_parameters: &[CheckedUnitStructuralParameterPlan],
    scalar_parameters: &[CheckedStructuralScalarParameterPlan],
    statement_index: u32,
    assignment: &typed_trees::statement::TableAssignment,
    (position, primitive_type): (u32, PrimitiveType),
    trace: &LocalConstructionTrace,
) -> Option<CheckedUnitEffectOperationPlan> {
    let [store] = <[_; 1]>::try_from(plan_assignment(
        program,
        facts,
        machine,
        state,
        StoreRoots::Parameters {
            structural: structural_parameters,
            scalar: scalar_parameters,
        },
        statement_index,
        assignment,
        AssignmentSource::CallResult {
            position,
            primitive_type,
        },
        trace,
    )?)
    .ok()?;
    Some(store)
}

/// A completed byte value uses the destination's ordinary replacement:
/// owned contents copy bytes and length; shared fields copy the descriptor.
/// Destination selection is independent of how the RHS value was produced.
pub(super) fn build_byte_call_result_store(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    structural_parameters: &[CheckedUnitStructuralParameterPlan],
    scalar_parameters: &[CheckedStructuralScalarParameterPlan],
    statement_index: u32,
    assignment: &typed_trees::statement::TableAssignment,
    binding_ordinal: u32,
    result_type: typed_trees::types::TypeReferenceHandle,
    trace: &LocalConstructionTrace,
) -> Option<CheckedUnitEffectOperationPlan> {
    let [store] = <[_; 1]>::try_from(plan_assignment(
        program,
        facts,
        machine,
        state,
        StoreRoots::Parameters {
            structural: structural_parameters,
            scalar: scalar_parameters,
        },
        statement_index,
        assignment,
        AssignmentSource::StructuralCallResult {
            binding_ordinal,
            result_type,
        },
        trace,
    )?)
    .ok()?;
    matches!(store, CheckedUnitEffectOperationPlan::ByteSequenceStore(_)).then_some(store)
}

/// Test convenience: the traced builder without a trace.
#[cfg(test)]
pub(super) fn build_structural_scalar_field_store_sequence(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    structural_parameters: &[CheckedUnitStructuralParameterPlan],
    scalar_parameters: &[CheckedStructuralScalarParameterPlan],
    statement_start: usize,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
) -> Option<Vec<CheckedUnitEffectOperationPlan>> {
    build_structural_scalar_field_store_sequence_traced(
        program,
        facts,
        machine,
        state,
        structural_parameters,
        scalar_parameters,
        statement_start,
        call_frames,
        &LocalConstructionTrace::default(),
    )
}

/// The scalar coordinate one carrier-path runtime index names at this
/// statement. Every `CheckedRuntimeIndex` variant resolves to the scalar
/// expression the same selector spells as a value: an assignment selector
/// keeps its bound row, a parameter or local its dense position's declared
/// type, and a field snapshot its leaf type below the declared root.
fn runtime_index_expression(
    program: &TypedTrees,
    facts: &CheckFacts,
    state: &typed_trees::state::State,
    statement_index: u32,
    index: &checked_trees::CheckedRuntimeIndex,
) -> Option<checked_trees::CheckedScalarExpression> {
    match index {
        checked_trees::CheckedRuntimeIndex::AssignmentIndex { depth } => Some(
            facts
                .values
                .scalar_expressions
                .bound_expression_at(
                    state.symbol,
                    statement_index,
                    CheckedScalarExpressionRole::AssignmentIndex { depth: *depth },
                )?
                .1
                .clone(),
        ),
        checked_trees::CheckedRuntimeIndex::Parameter { position } => {
            let position = usize::try_from(*position).ok()?;
            let parameter = program
                .state_parameters(state)
                .iter()
                .filter(|parameter| crate::values::occupies_scalar_position(program, parameter))
                .nth(position)?;
            Some(checked_trees::CheckedScalarExpression::Parameter {
                position,
                primitive_type: program.primitive_type_reference(parameter.type_reference)?,
            })
        }
        checked_trees::CheckedRuntimeIndex::Local { position } => {
            let position = usize::try_from(*position).ok()?;
            let mut dense = 0usize;
            for statement in program.statement_table.statements(state.statement_nodes) {
                let StatementNode::LocalData(local) = statement else {
                    continue;
                };
                let primitive_type = program
                    .primitive_type_reference(local.type_reference)
                    .or_else(|| {
                        crate::values::shared_loan_place(program, local)
                            .map(|(_, primitive_type, _)| primitive_type)
                    });
                let Some(primitive_type) = primitive_type else {
                    continue;
                };
                if dense == position {
                    return Some(checked_trees::CheckedScalarExpression::Local {
                        position,
                        primitive_type,
                    });
                }
                dense += 1;
            }
            None
        }
        checked_trees::CheckedRuntimeIndex::StructuralField(selector) => {
            let (_, _, leaf, _) = crate::values::resolve_structural_parameter_path(
                program,
                program.state_parameters(state),
                selector.parameter_position,
                &selector.path,
            )?;
            Some(
                checked_trees::CheckedScalarExpression::StructuralParameterField {
                    parameter_position: selector.parameter_position,
                    path: selector.path.clone(),
                    primitive_type: program.primitive_type_reference(leaf)?,
                },
            )
        }
    }
}

/// Compose one assignment: resolve the destination, then let the leaf's
/// declared type select the store its value joins.
fn plan_assignment(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    roots: StoreRoots<'_>,
    statement_index: u32,
    assignment: &typed_trees::statement::TableAssignment,
    source: AssignmentSource,
    trace: &LocalConstructionTrace,
) -> Option<Vec<CheckedUnitEffectOperationPlan>> {
    let destination = destination::resolve(
        program,
        facts,
        machine,
        state,
        roots,
        statement_index,
        assignment,
        trace,
    )?;
    // A runtime selector directly on the root indexes a whole borrowed view;
    // that operation carries its own (read-capable) authority.
    if destination.place.segments.is_empty() && destination.dynamic_index.is_some() {
        return byte_stores::view_write(
            program,
            facts,
            machine,
            state,
            &destination,
            statement_index,
            assignment,
            source,
            trace,
        )
        .map(|write| vec![write]);
    }
    if destination.place.segments.is_empty()
        && destination.dynamic_index.is_none()
        && let Some(store) = byte_stores::whole_store(
            program,
            facts,
            state,
            &destination,
            statement_index,
            assignment,
            source,
        )
    {
        return Some(vec![store]);
    }
    destination.exclusive_authority(program, state, roots, trace)?;
    // A store through a recast loan local re-covers its rejoined leaf at
    // the view's declared carrier — a family no leaf-store arm below
    // answers, since the leaf's own primitive is not the stored value's.
    if let Some(view) = destination.recast_view {
        trace.phase("structural field store: representation view");
        return view_stores::plan(
            program,
            facts,
            machine,
            state,
            &destination,
            view,
            statement_index,
            assignment,
            source,
            trace,
        )
        .map(|store| vec![store]);
    }
    trace.phase("structural field store: carrier path");
    let Some((leaf, carriers)) = destination.place.segments.split_last() else {
        // Whole structural replacement is owned by the statement sequencer.
        return None;
    };
    let facts::PlaceSegment::Field { symbol } = leaf else {
        // An indexed leaf reaches a primitive element the `WriteOnlyPrimitiveStore`
        // path already walks — fields and selectors compose the same way over a
        // root the primitive lane's own admission declined (a view local, an
        // erased alias), the destination's resolved authority still carrying
        // the store.
        return primitive::indexed_store(
            program,
            facts,
            machine,
            state,
            &destination,
            statement_index,
            assignment,
        )
        .map(|store| vec![store]);
    };
    let carrier = destination.leaf_carrier(program, carriers)?;
    trace.phase("structural field store: field owner record");
    let field = exact_relevant_field(program, carrier.owner, *symbol)?;
    trace.phase("structural field store: byte sequence carrier");
    // An indexed store through a `&'r mut [u8]` field composes over the
    // caller's live extent exactly as over bounded-owned backing: the mutable
    // borrow already guarantees exclusive access. A shared view still
    // declines; the Reference node's access is the only place that
    // distinction survives before the carrier collapses to `BorrowedView`.
    let byte_leaf = match byte_sequence_carrier(program, field.type_reference, &[]) {
        Some(checked_trees::CheckedByteSequenceCarrier::BoundedOwned { capacity }) => {
            Some(Some(capacity))
        }
        Some(checked_trees::CheckedByteSequenceCarrier::BorrowedView { .. })
            if byte_stores::field_view_is_mutable(program, field.type_reference) =>
        {
            Some(None)
        }
        Some(checked_trees::CheckedByteSequenceCarrier::BoundedView { capacity, .. })
            if byte_stores::field_view_is_mutable(program, field.type_reference) =>
        {
            Some(Some(capacity))
        }
        _ => None,
    };
    // Byte-field and whole-sum stores, and the scalar-graph lane's record
    // locals, lower a static carrier only: none of them evaluates a selector.
    // An exclusive borrowed-view local is the exception — whatever the
    // referee's declared shape, its selector evaluates in the same assignment
    // namespace a parameter-rooted store's would, and the loan ledger already
    // established the write authority emission resolves to the loaned home.
    let static_carrier = !carrier
        .path
        .iter()
        .any(|segment| matches!(segment, CheckedUnitStructuralPathSegment::RuntimeIndex(_)));
    if !static_carrier && matches!(destination.root, Root::Local { .. }) && !destination.view_loan {
        return None;
    }
    if let Some(capacity) = byte_leaf {
        if !static_carrier {
            return None;
        }
        return byte_stores::field_store(
            program,
            facts,
            machine,
            state,
            &destination,
            carrier.path,
            field,
            capacity,
            statement_index,
            assignment,
            source,
            trace,
        )
        .map(|store| vec![store]);
    }
    // Descriptor replacement borrows its actual backing. The operation plan
    // retains that operand; source and receiving loan replay enforce lifetime.
    if is_shared_view_field(program, field.type_reference) {
        return view_field_store::store(
            program,
            facts,
            machine,
            state,
            roots,
            &destination,
            carrier.path,
            field,
            statement_index,
            assignment,
            source,
            trace,
        )
        .map(|store| vec![store]);
    }
    // Any other borrowed view is a reference whose lifetime no store
    // operation relates to the field (structural_access.md, Store
    // vocabulary: reference crossings reject).
    trace.phase("structural field store: borrowed view field");
    if destination::crosses_reference(program, field.type_reference) {
        return None;
    }
    trace.phase("structural field store: scalar field type");
    // A runtime selector below a field names an element, not this field:
    // `record.field[i]` composes through the same indexed primitive store
    // a bare `arr[i]` destination reaches, carrying this channel's resolved
    // root and authority. Elements the primitive leaf walk cannot type
    // decline there as before.
    if destination.dynamic_index.is_some() {
        trace.phase("structural field store: scalar field type: indexed element destination");
        return primitive::indexed_store(
            program,
            facts,
            machine,
            state,
            &destination,
            statement_index,
            assignment,
        )
        .map(|store| vec![store]);
    }
    // The same complete field declaration governs construction and every
    // replacement. Retaining its carrier requests the declaration-derived
    // store obligation; it does not prove the completed right-hand side.
    if program
        .primitive_type_reference(field.type_reference)
        .is_some()
    {
        let primitive_type = scalar_leaf(program, field)?;
        let value = value::assignment_value(
            program,
            facts,
            machine,
            state,
            &destination.root,
            statement_index,
            assignment,
            source,
            primitive_type,
            trace,
        )?;
        if byte_stores::element_view_element_reference(program, state, &destination).is_some() {
            // The existing carrier walk and leaf admission own the field's
            // identity and scalar value. A descriptor contributes its live
            // outer index/length pair; the remaining projection stays within
            // that element, with the same field and index semantics.
            let (outer, projected) = carrier.path.split_first()?;
            let index = match outer {
                CheckedUnitStructuralPathSegment::FixedIndex(index) => {
                    checked_trees::CheckedScalarExpression::IntegerLiteral {
                        literal: numerics::literals::IntegerLiteral::from_value(
                            i64::try_from(*index).ok()?,
                        )
                        .with_landing(numerics::literals::IntegerLanding {
                            landed_type: numerics::literals::LandedIntegerType::U64,
                            domain: numerics::arithmetic::ArithmeticDomain::Exact,
                        }),
                    }
                }
                CheckedUnitStructuralPathSegment::RuntimeIndex(index) => {
                    runtime_index_expression(program, facts, state, statement_index, index)?
                }
                _ => return None,
            };
            let mut path = projected.to_vec();
            path.push(CheckedUnitStructuralPathSegment::Field(
                terminal_field_identity(program, field.symbol)?,
            ));
            return Some(vec![CheckedUnitEffectOperationPlan::ElementViewWrite(
                checked_trees::CheckedElementViewWritePlan {
                    statement_index,
                    destination: destination.root.destination(),
                    index,
                    path,
                    element: primitive_type,
                    value,
                },
            )]);
        }
        return Some(vec![
            CheckedUnitEffectOperationPlan::StructuralScalarFieldStore(
                CheckedStructuralScalarFieldStorePlan {
                    statement_index,
                    destination: destination.root.destination(),
                    carrier_path: carrier.path,
                    field_identity: terminal_field_identity(program, field.symbol)?,
                    primitive_type,
                    value,
                },
            ),
        ]);
    }
    let field_data =
        crate::facts::field_domain::data_definition_for_field_type(program, field.type_reference)?;
    if plain_record(field_data, program, &facts.operators) {
        return None;
    }
    trace.phase("structural field store: case field type");
    if !static_carrier {
        return None;
    }
    case_field_store(
        program,
        state,
        roots,
        &destination,
        carrier.path.clone(),
        field,
        field_data,
        statement_index,
        assignment,
        source,
    )
    .or_else(|| {
        case_literal_field_store(
            program,
            facts,
            state,
            &destination,
            carrier.path,
            field,
            field_data,
            statement_index,
            assignment,
            source,
        )
    })
    .map(|store| vec![store])
}

/// The shared field retention judgment keeps every restriction independently
/// of whether the destination is a parameter or an owned local. Atomic storage
/// retains a different access contract and cannot become an ordinary store.
fn scalar_leaf(
    program: &TypedTrees,
    field: &typed_trees::data::DataField,
) -> Option<PrimitiveType> {
    let primitive = program.primitive_type_reference(field.type_reference)?;
    let retained = crate::execution::terminal_unit::types::retain_scalar_field(
        program,
        field.type_reference,
        &[],
        primitive,
        None,
    )?;
    matches!(
        retained,
        checked_trees::CheckedUnitStructuralFieldType::Scalar(_)
            | checked_trees::CheckedUnitStructuralFieldType::BoundedInteger(_)
            | checked_trees::CheckedUnitStructuralFieldType::RefinedScalar { .. }
    )
    .then_some(primitive)
}

/// `place.<sum field> = <parameter path>`: an initialized unrestricted-sum
/// field overwritten by a whole parameter or a member read below one, both of
/// the same declared type. The write copies whole, moves nothing, and needs no
/// carrier borrow window -- call results, payload sums, records and affine
/// carriers keep declining.
fn case_field_store(
    program: &TypedTrees,
    state: &typed_trees::state::State,
    roots: StoreRoots<'_>,
    destination: &Destination<'_>,
    carrier_path: Vec<CheckedUnitStructuralPathSegment>,
    field: &typed_trees::data::DataField,
    field_data: &typed_trees::data::DataDefinition,
    statement_index: u32,
    assignment: &typed_trees::statement::TableAssignment,
    source: AssignmentSource,
) -> Option<CheckedUnitEffectOperationPlan> {
    let (
        AssignmentSource::Authored,
        StoreRoots::Parameters {
            structural: structural_parameters,
            ..
        },
    ) = (source, roots)
    else {
        return None;
    };
    if !unrestricted_sum(field_data, program) {
        return None;
    }
    let value_place = crate::flow::canonical_place_from_expression_in_state(
        program,
        state.symbol,
        statement_index as usize,
        assignment.value,
    )?;
    let facts::PlaceRoot::Symbol(value_root) = value_place.root else {
        return None;
    };
    let source_parameters = program.state_parameters(state);
    let value_position = source_parameters
        .iter()
        .position(|parameter| parameter.symbol == value_root)?;
    let value_state_parameter = source_parameters.get(value_position)?;
    if value_state_parameter.is_self {
        return None;
    }
    let (projected, value_path) = super::calls::projected_argument_path(
        program,
        state.symbol,
        statement_index as usize,
        &value_place,
    )?;
    // A member read below one structural parameter names the copied leaf; the
    // projected type, not the parameter's own type, decides the field match.
    let same_named_type = matches!(
        (
            program
                .type_reference_table
                .type_reference(field.type_reference),
            program.type_reference_table.type_reference(projected),
        ),
        (
            TypeReferenceNode::Named { symbol: field_name, .. },
            TypeReferenceNode::Named { symbol: value_name, .. }
        ) if field_name == value_name
    );
    if !same_named_type {
        return None;
    }
    let (value_index, value_plan) = structural_parameters
        .iter()
        .enumerate()
        .find(|(_, parameter)| parameter.position as usize == value_position)?;
    if value_plan.is_self
        || !matches!(
            value_plan.access,
            CheckedStructuralAccess::Owned | CheckedStructuralAccess::SharedBorrow
        )
        || value_plan.multiplicity != Multiplicity::Unrestricted
        || !value_plan.qualifications.is_empty()
        || !value_plan.projected_qualifications.is_empty()
        || value_plan.fused_service_erasure.is_some()
    {
        return None;
    }
    Some(CheckedUnitEffectOperationPlan::StructuralCaseFieldStore(
        checked_trees::CheckedStructuralCaseFieldStorePlan {
            statement_index,
            destination: destination.root.destination(),
            carrier_path,
            field_identity: terminal_field_identity(program, field.symbol)?,
            value: checked_trees::CheckedUnitStructuralArgumentPlan {
                source: checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter {
                    parameter_index: u32::try_from(value_index).ok()?,
                },
                path: value_path,
                type_identity: super::types::base_type_identity(program, projected, &[])?,
                access: value_plan.access,
            },
        },
    ))
}

/// `place.field = Sum::Case { .. }` — the literal carries no custody root, so
/// the store itself establishes the construction into the field's storage:
/// the tag, every authored scalar operand at its declared carrier, and a zero
/// for each common field the literal leaves unnamed. The destination still
/// proves the same owned place, write-frame and static-carrier custody the
/// whole-place store proves; only the source's shape differs — construction
/// operands where a copy named a structural place.
fn case_literal_field_store(
    program: &TypedTrees,
    facts: &CheckFacts,
    state: &typed_trees::state::State,
    destination: &Destination<'_>,
    carrier_path: Vec<CheckedUnitStructuralPathSegment>,
    field: &typed_trees::data::DataField,
    field_data: &typed_trees::data::DataDefinition,
    statement_index: u32,
    assignment: &typed_trees::statement::TableAssignment,
    source: AssignmentSource,
) -> Option<CheckedUnitEffectOperationPlan> {
    if !matches!(source, AssignmentSource::Authored) {
        return None;
    }
    // The store constructs in place, so the sum is the closed Unrestricted
    // family a whole-place copy needs — minus the payload-free clause: the
    // selected case's payload is exactly what the literal establishes.
    if !closed_sum(field_data, program) {
        return None;
    }
    let constructor =
        validation::structural_case_constructor(program, assignment.value, field.type_reference)?;
    if program.normalized_type_identity(constructor.type_reference)
        != program.normalized_type_identity(field.type_reference)
    {
        return None;
    }
    let variant = program
        .data_members(field_data)
        .iter()
        .find_map(|member| match member {
            DataMember::Variant(variant) if variant.symbol == constructor.case => Some(variant),
            _ => None,
        })?;
    let declarations = program.data_payload_fields(variant);
    let commons = program
        .data_members(field_data)
        .iter()
        .filter_map(|member| match member {
            DataMember::Field(field) => Some(field),
            DataMember::Variant(_) => None,
        })
        .collect::<Vec<_>>();
    let mut fields = Vec::with_capacity(constructor.fields.len());
    for (ordinal, (field_symbol, source_expression, declared)) in
        constructor.fields.iter().enumerate()
    {
        // This arm admits scalar operands only; a structural operand is the
        // nested-construction boundary the plan vocabulary does not carry.
        let primitive_type = program.primitive_type_reference(*declared)?;
        let identity = declarations
            .iter()
            .find(|declaration| declaration.symbol == *field_symbol)
            .map(|declaration| declaration.path_identity())
            .or_else(|| {
                commons
                    .iter()
                    .find(|declaration| declaration.symbol == *field_symbol)
                    .map(|declaration| declaration.path_identity())
            })?;
        let field_ordinal = u32::try_from(ordinal).ok()?;
        let (binding, expression) = facts.values.scalar_expressions.bound_expression_at(
            state.symbol,
            statement_index,
            CheckedScalarExpressionRole::StructuralValueField {
                expression: assignment.value,
                field_ordinal,
            },
        )?;
        if binding.expression != *source_expression || binding.destination.is_valid() {
            return None;
        }
        fields.push(checked_trees::CheckedScalarCaseFieldPlan {
            field_ordinal,
            field_identity: identity,
            primitive_type,
            expression: expression.clone(),
        });
    }
    Some(
        CheckedUnitEffectOperationPlan::StructuralCaseLiteralFieldStore(
            checked_trees::CheckedStructuralCaseLiteralFieldStorePlan {
                statement_index,
                destination: destination.root.destination(),
                carrier_path,
                field_identity: terminal_field_identity(program, field.symbol)?,
                literal: assignment.value,
                case_identity: variant.path_identity(),
                type_identity: super::types::base_type_identity(
                    program,
                    field.type_reference,
                    &[],
                )?,
                fields,
            },
        ),
    )
}

/// A closed payload-free pure-sum data declaration: CheckedShape supply, no
/// lifetimes or type parameters, no quotient or where facts, unrestricted
/// multiplicity, and every member a payload-free case. Copying a whole owned
/// place into such a field moves nothing, so the store needs no carrier
/// borrow window.
fn unrestricted_sum(data: &typed_trees::data::DataDefinition, program: &TypedTrees) -> bool {
    closed_sum(data, program)
        && typed_trees::data::DataDefinition::shape_kind_from_members(program.data_members(data))
            == DataShapeKind::Enum
        && program.data_members(data).iter().all(|member| {
            let DataMember::Variant(variant) = member else {
                return false;
            };
            program.data_payload_fields(variant).is_empty()
        })
}

/// The closed Unrestricted family both sum stores share: CheckedShape supply,
/// no lifetimes or type parameters, no quotient or where facts, and no zero
/// gate — the whole-place arm adds its payload-free Enum shape on top.
fn closed_sum(data: &typed_trees::data::DataDefinition, program: &TypedTrees) -> bool {
    data.supply_mode == language_semantics::DataSupplyMode::CheckedShape
        && data.lifetime_parameters.is_empty()
        && program.data_type_parameters(data).is_empty()
        && data.quotient.is_none()
        && data.where_facts.is_empty()
        && !data.zero_gated
        && data.properties.multiplicity == language_semantics::Multiplicity::Unrestricted
}

/// `place = local` where `local` is a structural (non-primitive) local of the
/// same state: no scalar store row exists for it, so the statement sequence
/// decides whether it repairs an open borrowed-storage window.
fn restores_structural_local(
    program: &TypedTrees,
    state: &typed_trees::state::State,
    assignment: &typed_trees::statement::TableAssignment,
) -> bool {
    let ExpressionNode::Name(path) = program.expression_table.expression(assignment.value) else {
        return false;
    };
    if path.head_symbol != path.symbol
        || program
            .expression_table
            .name_path_members(path.members)
            .len()
            != 1
    {
        return false;
    }
    program
        .state_local_declaration(state, path.symbol)
        .is_some_and(|(_, local)| {
            program
                .primitive_type_reference(local.type_reference)
                .is_none()
        })
}

/// The value is an owned structural parameter read whole: no scalar store
/// decomposes it — the sequence's bound-value field store plans the move.
fn bound_structural_parameter(
    program: &TypedTrees,
    state: &typed_trees::state::State,
    structural_parameters: &[CheckedUnitStructuralParameterPlan],
    statement_index: u32,
    assignment: &typed_trees::statement::TableAssignment,
) -> bool {
    let Ok(index) = usize::try_from(statement_index) else {
        return false;
    };
    let Some(place) = crate::flow::canonical_place_from_expression_in_state(
        program,
        state.symbol,
        index,
        assignment.value,
    ) else {
        return false;
    };
    if !place.segments.is_empty() {
        return false;
    }
    let facts::PlaceRoot::Symbol(symbol) = place.root else {
        return false;
    };
    let Some(position) = program
        .state_parameters(state)
        .iter()
        .position(|parameter| parameter.symbol == symbol)
    else {
        return false;
    };
    let Ok(position) = u32::try_from(position) else {
        return false;
    };
    structural_parameters.iter().any(|parameter| {
        parameter.position == position
            && !parameter.is_self
            && parameter.access == CheckedStructuralAccess::Owned
            && parameter.qualifications.is_empty()
            && parameter.projected_qualifications.is_empty()
    })
}

/// Whether a declared field type is a shared `&[u8]` view, through any domain
/// constraint shells: the field stores a relocatable descriptor.
fn is_shared_view_field(
    program: &TypedTrees,
    type_reference: typed_trees::types::TypeReferenceHandle,
) -> bool {
    matches!(
        byte_sequence_carrier(program, type_reference, &[]),
        Some(checked_trees::CheckedByteSequenceCarrier::BorrowedView { .. })
    ) && !byte_stores::field_view_is_mutable(program, type_reference)
}
