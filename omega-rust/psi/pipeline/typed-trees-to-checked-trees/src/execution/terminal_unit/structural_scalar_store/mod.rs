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
//! - a record place takes a record literal as one field store per member;
//! - a payload-free sum field takes a whole owned parameter of its type;
//! - a bounded-owned byte field or a whole borrowed byte view takes the byte
//!   store operations of `byte_stores.rs`.
//!
//! What remains refused is a custody or proof condition, not an arrangement:
//! write frames, linear or qualified roots, reference crossings, and fields
//! whose declared domain the stored value carries no evidence for.
use super::{
    CheckFacts, CheckedStructuralAccess, CheckedStructuralScalarFieldStorePlan,
    CheckedStructuralScalarParameterPlan, CheckedUnitEffectOperationPlan,
    CheckedUnitStructuralParameterPlan, CheckedUnitStructuralPathSegment, DataMember,
    DataShapeKind, ExpressionNode, Multiplicity, PrimitiveType, StatementNode, TypeReferenceNode,
    TypedTrees,
};
use crate::execution::terminal_unit::control::LocalConstructionTrace;
use crate::execution::terminal_unit::types::{byte_sequence_carrier, terminal_field_identity};
use destination::{Destination, Root, StoreRoots, exact_relevant_field, plain_record};
use value::AssignmentSource;

mod byte_stores;
mod destination;
mod frame;
#[cfg(test)]
mod tests;
mod value;

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
        let statement_index = u32::try_from(statement_index).ok()?;
        trace.phase("scalar field store sequence: assignment store");
        trace.statement(Some(statement_index));
        if let Some(store) = super::primitive_store::build_primitive_store_at(
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
        // the repair store of a borrowed-storage window.
        if matches!(
            program.expression_table.expression(assignment.value),
            ExpressionNode::Call(_)
        ) || restores_structural_local(program, state, assignment)
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
    destination.exclusive_authority(program, state, roots, trace)?;
    trace.phase("structural field store: carrier path");
    let Some((leaf, carriers)) = destination.place.segments.split_last() else {
        // The whole root is replaced: only a record literal decomposes into
        // stores the root's own authority covers.
        return record_literal_stores(
            program,
            facts,
            machine,
            state,
            &destination,
            statement_index,
            assignment,
            source,
            trace,
        );
    };
    let facts::PlaceSegment::Field { symbol } = leaf else {
        return None;
    };
    let carrier = destination.carrier(program, carriers)?;
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
        Some(checked_trees::CheckedByteSequenceCarrier::BorrowedView)
            if byte_stores::field_view_is_mutable(program, field.type_reference) =>
        {
            Some(None)
        }
        _ => None,
    };
    if let Some(capacity) = byte_leaf {
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
    // A borrowed view is a reference: storing one into a field would
    // introduce a stored loan, and no store operation relates the field to
    // the view's lifetime (structural_access.md, Store vocabulary: reference
    // crossings reject).
    trace.phase("structural field store: borrowed view field");
    if destination::crosses_reference(program, field.type_reference) {
        return None;
    }
    trace.phase("structural field store: scalar field type");
    // A runtime selector below a field names an element, not this field.
    if destination.dynamic_index.is_some() {
        trace.phase("structural field store: scalar field type: indexed element destination");
        return None;
    }
    // A declared domain needs membership evidence the stored value does not
    // carry, and no store operation retains one.
    if !crate::facts::field_domain::domain_constraint_symbols(program, field.type_reference)
        .is_empty()
    {
        trace.phase("structural field store: scalar field type: domain-constrained field");
        return None;
    }
    if program
        .primitive_type_reference(field.type_reference)
        .is_some()
    {
        let primitive_type = scalar_leaf(program, &destination.root, field)?;
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
    if plain_record(field_data, program) {
        return record_literal_stores(
            program,
            facts,
            machine,
            state,
            &destination,
            statement_index,
            assignment,
            source,
            trace,
        );
    }
    trace.phase("structural field store: case field type");
    case_field_store(
        program,
        state,
        roots,
        &destination,
        carrier.path,
        field,
        field_data,
        statement_index,
        assignment,
        source,
    )
    .map(|store| vec![store])
}

/// The primitive type of a scalar leaf this root's lane can store.
///
/// A borrowed parameter's store retains the field's own declaration, so any
/// Boolean, IEEE or integer carrier is admitted, bounded integers with their
/// own range obligation. The scalar-graph lane replaces a local record's
/// field only through an exact unconstrained carrier: its lowering carries
/// no range obligation or arithmetic-domain policy for the field.
fn scalar_leaf(
    program: &TypedTrees,
    root: &Root<'_>,
    field: &typed_trees::data::DataField,
) -> Option<PrimitiveType> {
    let primitive_type = program.primitive_type_reference(field.type_reference)?;
    match root {
        Root::Parameter { .. } => (matches!(
            primitive_type,
            PrimitiveType::Bool | PrimitiveType::F32 | PrimitiveType::F64
        ) || primitive_type.accepts_integer_literal())
        .then_some(primitive_type),
        Root::Local { .. } => {
            let TypeReferenceNode::Named { symbol, name } = program
                .type_reference_table
                .type_reference(field.type_reference)
            else {
                return None;
            };
            let atom = program.symbols.builtin_type_atom(*symbol)?;
            let exact_integer = matches!(
                primitive_type,
                PrimitiveType::I8
                    | PrimitiveType::I16
                    | PrimitiveType::I32
                    | PrimitiveType::I64
                    | PrimitiveType::U8
                    | PrimitiveType::U16
                    | PrimitiveType::U32
                    | PrimitiveType::U64
            ) && program
                .arithmetic_domain_for_type_reference(field.type_reference)
                == numerics::arithmetic::ArithmeticDomain::Exact;
            (name.as_str() == atom.symbol_name()
                && (exact_integer
                    || matches!(
                        primitive_type,
                        PrimitiveType::Bool | PrimitiveType::F32 | PrimitiveType::F64
                    )))
            .then_some(primitive_type)
        }
    }
}

/// `place = Record { .. }` where `place` is a plain record: the literal
/// decomposes into the ordered member stores an authored `place.member`
/// sequence would produce. Every member supplies its scalar computation at
/// the literal's `RecordField` coordinate, and each store carries the place's
/// full path so the emitted shape matches a field store the source could
/// spell directly.
fn record_literal_stores(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    destination: &Destination<'_>,
    statement_index: u32,
    assignment: &typed_trees::statement::TableAssignment,
    source: AssignmentSource,
    trace: &LocalConstructionTrace,
) -> Option<Vec<CheckedUnitEffectOperationPlan>> {
    trace.phase("structural field store: record literal source");
    // Decomposition is an attached-Unit operation sequence whose custody
    // proof is the borrowed root's write frame. Indexed carriers belong to
    // the borrowed-element custody family, which rejoins its own element.
    let (AssignmentSource::Authored, Root::Parameter { .. }) = (source, &destination.root) else {
        return None;
    };
    if destination
        .place
        .segments
        .iter()
        .any(|segment| !matches!(segment, facts::PlaceSegment::Field { .. }))
    {
        return None;
    }
    let carrier = destination.carrier(program, &destination.place.segments)?;
    let value_root = facts.values.structural_values.root_for_expression(
        state.symbol,
        statement_index,
        assignment.value,
    )?;
    if value_root.machine != machine.symbol {
        return None;
    }
    let checked_trees::CheckedStructuralValueKind::Record {
        data_symbol,
        fields,
    } = facts
        .values
        .structural_values
        .nodes
        .get(value_root.root)
        .kind
        .clone()
    else {
        return None;
    };
    if data_symbol != carrier.owner.symbol {
        return None;
    }
    let fields = facts
        .values
        .structural_values
        .record_fields
        .span(fields)?
        .to_vec();
    trace.phase("structural field store: record literal frame");
    // Sequence admission already replayed the complete frame; this
    // statement's write must be one of its named paths -- the custody proof a
    // field sequence earns one store at a time, applied to the literal's
    // whole record.
    let expected_mutation_path = destination.mutation_path(program)?;
    let frame = &facts
        .mutation
        .for_machine(machine.symbol)?
        .state_write_frames
        .iter()
        .find(|frame| frame.state == state.symbol)?
        .frame;
    if !frame
        .complete_paths()?
        .iter()
        .any(|path| path == &expected_mutation_path)
    {
        return None;
    }
    let mut stores = Vec::with_capacity(fields.len());
    for (ordinal, field) in fields.iter().enumerate() {
        let declaration = exact_relevant_field(program, carrier.owner, field.field)?;
        if !crate::facts::field_domain::domain_constraint_symbols(
            program,
            declaration.type_reference,
        )
        .is_empty()
        {
            return None;
        }
        let primitive_type = scalar_leaf(program, &destination.root, declaration)?;
        let value = value::record_field_value(
            facts,
            machine,
            state,
            statement_index,
            assignment.value,
            u32::try_from(ordinal).ok()?,
            field,
            primitive_type,
        )?;
        stores.push(CheckedUnitEffectOperationPlan::StructuralScalarFieldStore(
            CheckedStructuralScalarFieldStorePlan {
                statement_index,
                destination: destination.root.destination(),
                carrier_path: carrier.path.clone(),
                field_identity: terminal_field_identity(program, declaration.symbol)?,
                primitive_type,
                value,
            },
        ));
    }
    // A fieldless literal writes nothing the coverage checks can name; the
    // authored assignment must still own at least one store.
    (!stores.is_empty()).then_some(stores)
}

/// `place.<sum field> = <whole owned parameter>`: an initialized
/// unrestricted-sum field overwritten by a whole place of the same declared
/// type. The write copies whole, moves nothing, and needs no carrier borrow
/// window -- member-read values, call results, payload sums, records and
/// affine carriers keep declining.
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
    if !value_place.segments.is_empty() {
        return None;
    }
    let source_parameters = program.state_parameters(state);
    let value_position = source_parameters
        .iter()
        .position(|parameter| parameter.symbol == value_root)?;
    let value_state_parameter = source_parameters.get(value_position)?;
    if value_state_parameter.is_self {
        return None;
    }
    let same_named_type = matches!(
        (
            program
                .type_reference_table
                .type_reference(field.type_reference),
            program
                .type_reference_table
                .type_reference(value_state_parameter.type_reference),
        ),
        (
            TypeReferenceNode::Named { symbol: field_name, .. },
            TypeReferenceNode::Named { symbol: value_name, .. }
        ) if field_name == value_name
    );
    if !same_named_type {
        return None;
    }
    let value_plan = structural_parameters
        .iter()
        .find(|parameter| parameter.position as usize == value_position)?;
    if value_plan.is_self
        || value_plan.access != CheckedStructuralAccess::Owned
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
                    parameter_index: u32::try_from(value_position).ok()?,
                },
                path: Vec::new(),
                type_identity: value_plan.type_identity.clone(),
                access: CheckedStructuralAccess::Owned,
            },
        },
    ))
}

/// A closed payload-free pure-sum data declaration: CheckedShape supply, no
/// lifetimes or type parameters, no quotient or where facts, unrestricted
/// multiplicity, and every member a payload-free case. Copying a whole owned
/// place into such a field moves nothing, so the store needs no carrier
/// borrow window.
fn unrestricted_sum(data: &typed_trees::data::DataDefinition, program: &TypedTrees) -> bool {
    data.supply_mode == language_semantics::DataSupplyMode::CheckedShape
        && data.lifetime_parameters.is_empty()
        && program.data_type_parameters(data).is_empty()
        && data.quotient.is_none()
        && data.where_facts.is_empty()
        && !data.zero_gated
        && data.properties.multiplicity == language_semantics::Multiplicity::Unrestricted
        && typed_trees::data::DataDefinition::shape_kind_from_members(program.data_members(data))
            == DataShapeKind::Enum
        && program.data_members(data).iter().all(|member| {
            let DataMember::Variant(variant) = member else {
                return false;
            };
            program.data_payload_fields(variant).is_empty()
        })
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
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .any(|statement| {
            matches!(statement, StatementNode::LocalData(local)
                if local.symbol == path.symbol
                    && program.primitive_type_reference(local.type_reference).is_none())
        })
}
