//! Independent source occurrence custody for ordered projected scalar stores.
use super::{
    CheckedScalarExpressionRole, CheckedTrees, CheckedUnitEffectMachinePlan,
    CheckedUnitEffectOperationPlan, CheckedUnitStructuralPathSegment, LoweringError,
    terminal_scalar_type, unsupported,
};
use crate::emission::indexed_write::{self, IndexedWriteLeaf};
use checked_trees::expression::ExpressionNode;
use checked_trees::statement::StatementNode;

mod case_fields;
pub(crate) use case_fields::validate as validate_case_store;

pub(crate) fn validate(
    checked: &CheckedTrees,
    plan: &CheckedUnitEffectMachinePlan,
) -> Result<(), LoweringError> {
    crate::unit::attached_unit::primitive_locals::validate_roster(checked, plan)?;
    let (machine, state) =
        crate::expression_preparation::source_custody::authored_state(checked, plan.state)?;
    if machine.symbol != plan.machine {
        return unsupported("structural scalar store has a different authored machine");
    }
    let statements = checked.statement_table.statements(state.statement_nodes);
    // Replay every case row, including an orphan whose ordinal names no
    // assignment. Walking authored assignments alone would miss that row.
    for operation in &plan.operations {
        if matches!(
            operation,
            CheckedUnitEffectOperationPlan::StructuralCaseFieldStore(_)
                | CheckedUnitEffectOperationPlan::StructuralCaseLiteralFieldStore(_)
        ) {
            validate_case_store(
                checked,
                plan.machine,
                plan.state,
                operation,
                &plan.operations,
                &plan.structural_parameters,
            )?;
        }
    }
    let stores = plan
        .operations
        .iter()
        .filter_map(|operation| match operation {
            CheckedUnitEffectOperationPlan::StructuralScalarFieldStore(store) => Some(store),
            _ => None,
        })
        .collect::<Vec<_>>();
    // Select from the authored statement roster too: deleting every store
    // must not disable source coverage validation.
    for (statement_index, statement) in statements.iter().enumerate() {
        let StatementNode::Assignment(assignment) = statement else {
            continue;
        };
        let statement_index = u32::try_from(statement_index).map_err(|_| {
            LoweringError::Unsupported("structural scalar store statement ordinal exceeds u32")
        })?;
        // Every store-family row planned for this assignment. A record
        // literal plans one row per member and an array literal one per
        // element, so one family may own several rows; rows of two families
        // conflict.
        let statement_stores = plan
            .operations
            .iter()
            .filter(|operation| operation.store_statement_index() == Some(statement_index))
            .collect::<Vec<_>>();
        // An atomic carrier assignment is covered by its one atomic event and
        // by nothing else; the event's own source correspondence runs at its
        // emission (`atomic_sources`).
        let atomic_events = plan
            .operations
            .iter()
            .filter(|operation| {
                matches!(operation, CheckedUnitEffectOperationPlan::AtomicAccess(access)
                    if access.statement_index == statement_index)
            })
            .count();
        if atomic_events != 0 {
            if atomic_events != 1
                || has_other_store(&statement_stores, |_| false)
                || validation::atomic_assignment_carrier(checked, assignment).is_none()
            {
                return unsupported("atomic carrier assignment has conflicting store custody");
            }
            continue;
        }
        let writes = statement_stores
            .iter()
            .filter_map(|operation| match operation {
                CheckedUnitEffectOperationPlan::ByteSequenceWrite(write) => Some(write),
                _ => None,
            })
            .collect::<Vec<_>>();
        if let [write] = writes.as_slice() {
            if has_other_store(&statement_stores, |operation| {
                matches!(
                    operation,
                    CheckedUnitEffectOperationPlan::ByteSequenceWrite(_)
                )
            }) {
                return unsupported("byte-view assignment has conflicting store custody");
            }
            indexed_write::validate_assignment(
                checked,
                plan.machine,
                plan.state,
                assignment,
                IndexedWriteLeaf::ByteView(write),
            )?;
            continue;
        } else if !writes.is_empty() {
            return unsupported("byte-view assignment has duplicate store custody");
        }
        let element_writes = statement_stores
            .iter()
            .filter_map(|operation| match operation {
                CheckedUnitEffectOperationPlan::ElementViewWrite(write) => Some(write),
                _ => None,
            })
            .collect::<Vec<_>>();
        if let [write] = element_writes.as_slice() {
            if has_other_store(&statement_stores, |operation| {
                matches!(
                    operation,
                    CheckedUnitEffectOperationPlan::ElementViewWrite(_)
                )
            }) {
                return unsupported("element-view assignment has conflicting store custody");
            }
            indexed_write::validate_assignment(
                checked,
                plan.machine,
                plan.state,
                assignment,
                IndexedWriteLeaf::ElementView(write),
            )?;
            continue;
        } else if !element_writes.is_empty() {
            return unsupported("element-view assignment has duplicate store custody");
        }
        let view_stores = statement_stores
            .iter()
            .filter_map(|operation| match operation {
                CheckedUnitEffectOperationPlan::RepresentationViewStore(write) => Some(write),
                _ => None,
            })
            .collect::<Vec<_>>();
        if let [write] = view_stores.as_slice() {
            if has_other_store(&statement_stores, |operation| {
                matches!(
                    operation,
                    CheckedUnitEffectOperationPlan::RepresentationViewStore(_)
                )
            }) {
                return unsupported("representation view assignment has conflicting store custody");
            }
            crate::emission::representation_view_store::validate_assignment(
                checked,
                plan.machine,
                plan.state,
                assignment,
                write,
            )?;
            continue;
        } else if !view_stores.is_empty() {
            return unsupported("representation view assignment has duplicate store custody");
        }
        let indexed_stores = statement_stores
            .iter()
            .filter_map(|operation| match operation {
                CheckedUnitEffectOperationPlan::StructuralByteSequenceFieldByteStore(store) => {
                    Some(store)
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        let byte_stores = statement_stores
            .iter()
            .filter_map(|operation| match operation {
                CheckedUnitEffectOperationPlan::ByteSequenceStore(store) => Some(store),
                _ => None,
            })
            .collect::<Vec<_>>();
        if let [store] = indexed_stores.as_slice() {
            // An indexed byte store has never excluded primitive stores at its
            // statement; only byte and scalar field stores conflict with it.
            if has_other_store(&statement_stores, |operation| {
                matches!(
                    operation,
                    CheckedUnitEffectOperationPlan::StructuralByteSequenceFieldByteStore(_)
                        | CheckedUnitEffectOperationPlan::WriteOnlyPrimitiveStore { .. }
                )
            }) {
                return unsupported("indexed assignment has conflicting store custody");
            }
            indexed_write::validate_assignment(
                checked,
                plan.machine,
                plan.state,
                assignment,
                IndexedWriteLeaf::ByteField(store),
            )?;
            continue;
        } else if !indexed_stores.is_empty() {
            return unsupported("indexed assignment has duplicate byte-store custody");
        }
        // A record literal with shared view members decomposes into scalar
        // and descriptor stores; its whole roster is one record custody.
        let view_members = byte_stores
            .iter()
            .copied()
            .filter(|store| {
                matches!(
                    store.destination,
                    checked_trees::CheckedByteSequenceStoreDestination::ViewField { .. }
                )
            })
            .collect::<Vec<_>>();
        if !view_members.is_empty()
            && matches!(
                checked.expression_table.expression(assignment.value),
                ExpressionNode::StructLiteral(_)
            )
        {
            if view_members.len() != byte_stores.len()
                || has_other_store(&statement_stores, |operation| {
                    matches!(
                        operation,
                        CheckedUnitEffectOperationPlan::ByteSequenceStore(_)
                            | CheckedUnitEffectOperationPlan::StructuralScalarFieldStore(_)
                    )
                })
            {
                return unsupported("record view assignment has conflicting store custody");
            }
            for store in &view_members {
                crate::emission::structural_byte_sequence_store::validate_assignment(
                    checked,
                    plan.machine,
                    plan.state,
                    assignment,
                    store,
                )?;
            }
            let scalar_members = statement_stores
                .iter()
                .filter_map(|operation| match operation {
                    CheckedUnitEffectOperationPlan::StructuralScalarFieldStore(store) => {
                        Some(store)
                    }
                    _ => None,
                })
                .collect::<Vec<_>>();
            validate_assignment_stores(
                checked,
                plan.machine,
                plan.state,
                statement_index,
                assignment,
                &scalar_members,
                &view_members,
            )?;
            continue;
        }
        if let [store] = byte_stores.as_slice() {
            if has_other_store(&statement_stores, |operation| {
                matches!(
                    operation,
                    CheckedUnitEffectOperationPlan::ByteSequenceStore(_)
                )
            }) {
                return unsupported("assignment has both byte and scalar store custody");
            }
            crate::emission::structural_byte_sequence_store::validate_assignment(
                checked,
                plan.machine,
                plan.state,
                assignment,
                store,
            )?;
            continue;
        }
        let is_primitive_store = |operation: &CheckedUnitEffectOperationPlan| {
            matches!(
                operation,
                CheckedUnitEffectOperationPlan::WriteOnlyPrimitiveStore { .. }
            )
        };
        // Several primitive stores are one array literal's element roster,
        // rejoined below by `primitive_store::validate_element_rosters`.
        if statement_stores.iter().copied().any(is_primitive_store) {
            if has_other_store(&statement_stores, is_primitive_store) {
                return unsupported("primitive assignment has duplicate store custody");
            }
            continue;
        }
        if construction_assignment_owner(checked, plan, statements, statement_index, assignment)
            .is_some()
        {
            continue;
        }
        // A whole array literal of structural elements replaces its array one
        // element window at a time; the roster rejoins its authored literal.
        if let Some(literal) = crate::unit::attached_unit::replaced_array_literal(
            checked,
            plan.state,
            statement_index as usize,
            assignment,
        ) {
            let roster = plan
                .operations
                .iter()
                .filter(|operation| replacement_statement(operation) == Some(statement_index))
                .collect::<Vec<_>>();
            crate::unit::attached_unit::validate_array_element_replacements(
                checked,
                plan.state,
                &plan.structural_parameters,
                statement_index as usize,
                assignment,
                &roster,
                literal,
            )?;
            continue;
        }
        // A borrowed-window repair store covers its authored assignment; the
        // ledger already validated destination and value against the open hole.
        if plan.operations.iter().any(|operation| {
            matches!(operation,
            CheckedUnitEffectOperationPlan::StoreStructuralField { statement_index: ordinal, .. }
                if *ordinal == statement_index)
        }) {
            continue;
        }
        // Sum replacement owns the complete assignment, independently of
        // scalar-store custody. Rejoin the authored destination and value.
        if statement_stores.iter().any(|operation| {
            matches!(
                operation,
                CheckedUnitEffectOperationPlan::StructuralCaseFieldStore(_)
                    | CheckedUnitEffectOperationPlan::StructuralCaseLiteralFieldStore(_)
            )
        }) {
            // The operation sweep above already checked exact source and
            // exclusive replacement custody for this assignment.
            continue;
        }
        let matching = statement_stores
            .iter()
            .filter_map(|operation| match operation {
                CheckedUnitEffectOperationPlan::StructuralScalarFieldStore(store) => Some(store),
                _ => None,
            })
            .collect::<Vec<_>>();
        validate_assignment_stores(
            checked,
            plan.machine,
            plan.state,
            statement_index,
            assignment,
            &matching,
            &[],
        )?;
    }
    crate::emission::primitive_store::validate_element_rosters(
        checked,
        plan.state,
        &plan.operations,
    )?;
    for operation in &plan.operations {
        if let CheckedUnitEffectOperationPlan::WriteOnlyPrimitiveStore {
            statement_index,
            destination,
            path,
            value,
        } = operation
        {
            if let checked_trees::CheckedStoreRoot::Local { symbol } = destination {
                // Projected stores write into an established structural
                // local (a zeroed `let mut` array's element); whole stores
                // stay on initialized primitive storage.
                if crate::unit::attached_unit::primitive_locals::source(
                    checked,
                    plan,
                    *symbol,
                    *statement_index,
                )
                .is_err()
                {
                    let (machine, authored) =
                        crate::expression_preparation::source_custody::authored_state(
                            checked, plan.state,
                        )?;
                    let (root, mut prefix) = local_store_destination(
                        checked,
                        machine.symbol,
                        authored,
                        plan.state,
                        *statement_index,
                        *symbol,
                    )?;
                    prefix.extend(path.iter().cloned());
                    crate::emission::primitive_store::validate_symbol_assignment(
                        checked,
                        plan.state,
                        *statement_index,
                        root,
                        &prefix,
                        value,
                    )?;
                    continue;
                }
                crate::emission::primitive_store::validate_symbol_assignment(
                    checked,
                    plan.state,
                    *statement_index,
                    *symbol,
                    path,
                    value,
                )?;
                continue;
            }
            let checked_trees::CheckedStoreRoot::Parameter { position } = destination else {
                return unsupported("primitive store has no destination");
            };
            let destination = plan
                .structural_parameters
                .iter()
                .find(|parameter| parameter.position == *position)
                .ok_or(LoweringError::Unsupported(
                    "primitive store destination is absent",
                ))?;
            crate::emission::primitive_store::validate_assignment(
                checked,
                plan.state,
                *statement_index,
                destination,
                path,
                value,
            )?;
        }
    }
    for store in &stores {
        if !matches!(
            statements.get(store.statement_index as usize),
            Some(StatementNode::Assignment(_))
        ) {
            return unsupported("structural scalar store has no authored assignment");
        }
    }
    let byte_stores = plan
        .operations
        .iter()
        .filter_map(|operation| match operation {
            CheckedUnitEffectOperationPlan::ByteSequenceStore(store) => Some(store),
            _ => None,
        })
        .collect::<Vec<_>>();
    for store in &byte_stores {
        let Some(StatementNode::Assignment(assignment)) =
            statements.get(store.statement_index as usize)
        else {
            return unsupported("byte-field store has no authored assignment");
        };
        crate::emission::structural_byte_sequence_store::validate_assignment(
            checked,
            plan.machine,
            plan.state,
            assignment,
            store,
        )?;
    }
    let mut has_indexed_stores = false;
    for operation in &plan.operations {
        if let CheckedUnitEffectOperationPlan::ByteSequenceWrite(write) = operation {
            has_indexed_stores = true;
            let Some(StatementNode::Assignment(assignment)) =
                statements.get(write.statement_index as usize)
            else {
                return unsupported("byte-view write has no authored assignment");
            };
            indexed_write::validate_assignment(
                checked,
                plan.machine,
                plan.state,
                assignment,
                IndexedWriteLeaf::ByteView(write),
            )?;
        }
        if let CheckedUnitEffectOperationPlan::ElementViewWrite(write) = operation {
            has_indexed_stores = true;
            let Some(StatementNode::Assignment(assignment)) =
                statements.get(write.statement_index as usize)
            else {
                return unsupported("element-view write has no authored assignment");
            };
            indexed_write::validate_assignment(
                checked,
                plan.machine,
                plan.state,
                assignment,
                IndexedWriteLeaf::ElementView(write),
            )?;
        }
        if let CheckedUnitEffectOperationPlan::StructuralByteSequenceFieldByteStore(store) =
            operation
        {
            has_indexed_stores = true;
            let Some(StatementNode::Assignment(assignment)) =
                statements.get(store.statement_index as usize)
            else {
                return unsupported("byte replacement has no authored assignment");
            };
            indexed_write::validate_assignment(
                checked,
                plan.machine,
                plan.state,
                assignment,
                IndexedWriteLeaf::ByteField(store),
            )?;
        }
    }
    if (!stores.is_empty() || !byte_stores.is_empty() || has_indexed_stores)
        && !matches!(plan.operations.last(),
        Some(CheckedUnitEffectOperationPlan::Complete { statement_index, .. })
            if usize::try_from(*statement_index).ok() == Some(statements.len()))
    {
        return unsupported("structural scalar store return lost its authored statement boundary");
    }
    Ok(())
}

/// Whether a store-family row outside `own` claims the same assignment.
fn has_other_store(
    statement_stores: &[&CheckedUnitEffectOperationPlan],
    own: impl Fn(&CheckedUnitEffectOperationPlan) -> bool,
) -> bool {
    statement_stores.iter().any(|operation| !own(operation))
}

/// The authored statement an operation of a structural field replacement
/// names: its producer, move-out, store or displaced cleanup, or any call the
/// statement performs.
fn replacement_statement(operation: &CheckedUnitEffectOperationPlan) -> Option<u32> {
    match operation {
        CheckedUnitEffectOperationPlan::EstablishStructuralValue { result, .. }
        | CheckedUnitEffectOperationPlan::MoveStructuralField { result, .. } => {
            Some(result.statement_index)
        }
        CheckedUnitEffectOperationPlan::StoreStructuralField {
            statement_index, ..
        } => Some(*statement_index),
        CheckedUnitEffectOperationPlan::CallContinuationCleanup { coordinate, .. } => {
            Some(coordinate.statement_index)
        }
        _ => operation.call().map(|call| call.coordinate.statement_index),
    }
}

/// Existing affine construction writes target a local, not a receiver field.
/// Delegate only the exact source element occurrence retained by that owner.
fn construction_assignment_owner(
    checked: &CheckedTrees,
    plan: &CheckedUnitEffectMachinePlan,
    statements: &[StatementNode],
    statement_index: u32,
    assignment: &checked_trees::statement::TableAssignment,
) -> Option<()> {
    let ExpressionNode::Indexed(target) = checked.expression_table.expression(assignment.target)
    else {
        return None;
    };
    let ExpressionNode::Name(root) = checked.expression_table.expression(target.collection) else {
        return None;
    };
    let ExpressionNode::Integer(index) = checked.expression_table.expression(target.index) else {
        return None;
    };
    let index = index.value_bignum()?.to_u64()?;
    if !root.symbol.is_valid()
        || root.head_symbol != root.symbol
        || checked
            .expression_table
            .name_path_members(root.members)
            .len()
            != 1
    {
        return None;
    }
    let local = statements
        .get(..statement_index as usize)?
        .iter()
        .find_map(|statement| match statement {
            StatementNode::LocalData(local)
                if local.symbol == root.symbol
                    && local.is_mutable
                    && !local.initial_value.is_valid() =>
            {
                Some(local)
            }
            _ => None,
        })?;
    let checked_trees::types::TypeReferenceNode::FixedArray { element_type, .. } = checked
        .type_reference_table
        .type_reference(local.type_reference)
    else {
        return None;
    };
    let checked_trees::types::TypeReferenceNode::Named { symbol, .. } =
        checked.type_reference_table.type_reference(*element_type)
    else {
        return None;
    };
    let ExpressionNode::StructLiteral(literal) =
        checked.expression_table.expression(assignment.value)
    else {
        return None;
    };
    if literal.type_symbol != *symbol
        || literal.case_name.is_some()
        || !checked
            .expression_table
            .struct_fields(literal.fields)
            .is_empty()
    {
        return None;
    }
    let mut operations = plan
        .operations
        .iter()
        .filter_map(|operation| match operation {
            CheckedUnitEffectOperationPlan::EstablishTrivialAffineLocal {
                statement_index: ordinal,
                declaration_ordinal,
                type_identity,
            } if *ordinal == statement_index => Some((*declaration_ordinal, type_identity)),
            _ => None,
        });
    let (ordinal, identity) = operations.next()?;
    if operations.next().is_some() {
        return None;
    }
    let mut declarations = plan
        .trivial_affine_locals
        .iter()
        .filter(|local| local.declaration_ordinal == ordinal);
    let declaration = declarations.next()?;
    if declarations.next().is_some()
        || declaration.type_identity != *identity
        || declaration.construction.as_ref()?.index != index
    {
        return None;
    }
    Some(())
}

/// Validate every field store one authored assignment planned: a single
/// field, or each member of a whole-record replacement as one roster.
pub(crate) fn validate_assignment_stores(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    state_symbol: symbols::SymbolHandle,
    statement_index: u32,
    assignment: &checked_trees::statement::TableAssignment,
    stores: &[&checked_trees::CheckedStructuralScalarFieldStorePlan],
    view_stores: &[&checked_trees::CheckedByteSequenceStorePlan],
) -> Result<(), LoweringError> {
    // The authored value, not the emitted store count, determines custody.
    // A one-field record still assigns the whole record place and selects a
    // RecordField computation; a direct scalar assignment selects its leaf.
    if (!stores.is_empty() || !view_stores.is_empty())
        && matches!(
            checked.expression_table.expression(assignment.value),
            ExpressionNode::StructLiteral(_)
        )
    {
        return validate_record_stores(
            checked,
            machine,
            state_symbol,
            statement_index,
            assignment,
            stores,
            view_stores,
        );
    }
    match stores {
        [] => unsupported("structural scalar store roster omits an authored assignment"),
        [store] => validate_assignment(
            checked,
            machine,
            state_symbol,
            statement_index,
            assignment,
            store,
        ),
        _ => unsupported("structural scalar assignment has duplicate field stores"),
    }
}

pub(crate) fn validate_assignment(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    state_symbol: symbols::SymbolHandle,
    statement_index: u32,
    assignment: &checked_trees::statement::TableAssignment,
    store: &checked_trees::CheckedStructuralScalarFieldStorePlan,
) -> Result<(), LoweringError> {
    let (owner, state) =
        crate::expression_preparation::source_custody::authored_state(checked, state_symbol)?;
    if owner.symbol != machine || statement_index != store.statement_index {
        return unsupported("structural scalar store has a different authored machine");
    }
    let source = crate::emission::call_source_custody::projected_receivers::store_destination(
        checked,
        machine,
        state_symbol,
        Some(statement_index as usize),
        assignment.target,
    )?;
    let (destination, mut path) = match store.destination {
        checked_trees::CheckedStoreRoot::Parameter { position } => (
            checked
                .state_parameters(state)
                .get(position as usize)
                .ok_or(LoweringError::Unsupported(
                    "structural scalar store has no authored destination parameter",
                ))?
                .symbol,
            Vec::new(),
        ),
        checked_trees::CheckedStoreRoot::Local { symbol } => local_store_destination(
            checked,
            machine,
            state,
            state_symbol,
            statement_index,
            symbol,
        )?,
    };
    path.extend(store.carrier_path.iter().cloned());
    path.push(CheckedUnitStructuralPathSegment::Field(
        store.field_identity.clone(),
    ));
    // A same-carrier field of a typed record view writes the source leaf the
    // recast judgment matched; the checked plan names that leaf directly.
    let (authored_root, authored_path) =
        match crate::emission::representation_view_store::record_view_leaf_destination(
            checked,
            machine,
            state_symbol,
            statement_index as usize,
            assignment.target,
        )? {
            Some(rejoined) if rejoined.view == rejoined.leaf => (rejoined.root, rejoined.path),
            Some(_) => {
                return unsupported(
                    "structural scalar store through a record view changes its carrier",
                );
            }
            None => (source.root, source.path),
        };
    if authored_root != destination || authored_path != path {
        return unsupported("structural scalar store destination drifted from its authored place");
    }
    if computation_root(
        checked,
        machine,
        state_symbol,
        store.statement_index,
        &store.value,
        store.primitive_type,
        Some(&store.field_identity),
    )?
    .is_some()
    {
        return Ok(());
    }
    let plans = &checked.facts.values.scalar_expressions;
    let role = CheckedScalarExpressionRole::AssignmentValue;
    let expressions = plans
        .expressions
        .iter()
        .filter(|expression| {
            expression.state == state_symbol
                && expression.statement_ordinal == statement_index
                && expression.role == role
        })
        .collect::<Vec<_>>();
    // A store reading its own statement's call result has no authored scalar
    // expression to select: the right-hand side is the call. The destination
    // place was rejoined above, and the producing call is reconstructed where
    // the operation order and the value are lowered. A selected expression
    // here would mean the checked stage chose a different source.
    if matches!(
        store.value,
        checked_trees::CheckedStoreValue::ScalarResult { .. }
    ) {
        if !expressions.is_empty() {
            return unsupported("structural scalar store replaced a selected RHS with a result");
        }
        if !matches!(
            checked.expression_table.expression(assignment.value),
            ExpressionNode::Call(_)
        ) {
            return unsupported("structural scalar store call result has no authored call");
        }
        return Ok(());
    }
    let [expression] = expressions.as_slice() else {
        return unsupported("structural scalar store has no unique selected RHS");
    };
    if Some(&expression.expression) != store.value.as_pure() {
        return unsupported("structural scalar store RHS drifted from its selected expression");
    }
    let bindings = plans
        .source_bindings
        .iter()
        .filter(|(_, binding)| {
            binding.state == state_symbol
                && binding.statement_ordinal == statement_index
                && binding.role == role
        })
        .map(|(_, binding)| binding)
        .collect::<Vec<_>>();
    let [binding] = bindings.as_slice() else {
        return unsupported("structural scalar store has no unique RHS source binding");
    };
    let destination = match checked.expression_table.expression(assignment.target) {
        ExpressionNode::Name(name) => name.symbol,
        _ => symbols::SymbolHandle::invalid(),
    };
    if binding.expression != assignment.value || binding.destination != destination {
        return unsupported(
            "structural scalar store RHS binding drifted from its authored assignment",
        );
    }
    let primitive_type = store
        .value
        .as_pure()
        .and_then(|value| value.primitive_type())
        .ok_or(LoweringError::Unsupported(
            "structural scalar store RHS has no retained primitive carrier",
        ))?;
    crate::expression_preparation::source_custody::validate_pure(
        checked,
        binding,
        terminal_scalar_type(primitive_type)?,
    )
}

/// A whole-record replacement decomposes into one ordered field store per
/// member. Rejoin the rooted literal and require the exact retained field
/// roster, in authored order, against each store's destination, declared
/// field, and field computation.
fn validate_record_stores(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    state_symbol: symbols::SymbolHandle,
    statement_index: u32,
    assignment: &checked_trees::statement::TableAssignment,
    stores: &[&checked_trees::CheckedStructuralScalarFieldStorePlan],
    view_stores: &[&checked_trees::CheckedByteSequenceStorePlan],
) -> Result<(), LoweringError> {
    let (owner, state) =
        crate::expression_preparation::source_custody::authored_state(checked, state_symbol)?;
    if owner.symbol != machine {
        return unsupported("structural scalar store has a different authored machine");
    }
    let source = crate::emission::call_source_custody::projected_receivers::store_destination(
        checked,
        machine,
        state_symbol,
        Some(statement_index as usize),
        assignment.target,
    )?;
    // The literal may replace a whole root (`local = Pair { .. }`) or a
    // projected record place (`self.pair = Pair { .. }`); either way every
    // field store must write through exactly the authored target's path.
    let root = checked
        .facts
        .values
        .structural_values
        .root_for_expression(state.symbol, statement_index, assignment.value)
        .ok_or(LoweringError::Unsupported(
            "record store has no rooted record literal",
        ))?;
    if root.machine != machine {
        return unsupported("record store literal has different custody");
    }
    let node = checked.facts.values.structural_values.nodes.get(root.root);
    let checked_trees::CheckedStructuralValueKind::Record {
        data_symbol: record_symbol,
        fields: record_fields,
    } = &node.kind
    else {
        return unsupported("record store literal is not a plain record");
    };
    let record_symbol = *record_symbol;
    let fields = checked
        .facts
        .values
        .structural_values
        .record_fields
        .span(*record_fields)
        .ok_or(LoweringError::Unsupported(
            "record store literal field roster is stale",
        ))?;
    let ExpressionNode::StructLiteral(literal) =
        checked.expression_table.expression(assignment.value)
    else {
        return unsupported("record store has no authored literal");
    };
    let authored = checked.expression_table.struct_fields(literal.fields);
    // A shared view member is replaced by its own descriptor store, which
    // `structural_byte_sequence_store::validate_assignment` rejoins to this
    // member; every other member keeps one scalar store, in authored order.
    let view_member = |field: &checked_trees::CheckedStructuralRecordField| {
        let identity = member_identity(checked, field.field);
        view_stores
            .iter()
            .filter(|store| {
                store.statement_index == statement_index
                    && matches!(&store.destination,
                        checked_trees::CheckedByteSequenceStoreDestination::ViewField {
                            field_identity, ..
                        } if Some(field_identity) == identity.as_ref())
            })
            .count()
    };
    let view_members = fields.iter().map(view_member).collect::<Vec<_>>();
    if fields.len() != authored.len()
        || view_members.iter().any(|count| *count > 1)
        || view_members.iter().filter(|count| **count == 1).count() != view_stores.len()
        || fields.len() != stores.len() + view_stores.len()
    {
        return unsupported("record store changed its complete field roster");
    }
    let mut scalar_stores = stores.iter();
    for (ordinal, (field, initializer)) in fields.iter().zip(authored.iter()).enumerate() {
        if view_members[ordinal] == 1 {
            if field.field != initializer.field_symbol || field.expression != initializer.value {
                return unsupported("record store reordered or substituted a field");
            }
            continue;
        }
        let Some(store) = scalar_stores.next() else {
            return unsupported("record store changed its complete field roster");
        };
        if field.field != initializer.field_symbol
            || field.expression != initializer.value
            || store.statement_index != statement_index
        {
            return unsupported("record store reordered or substituted a field");
        }
        let destination = match store.destination {
            checked_trees::CheckedStoreRoot::Parameter { position } => {
                checked
                    .state_parameters(state)
                    .get(position as usize)
                    .ok_or(LoweringError::Unsupported(
                        "record store has no authored destination parameter",
                    ))?
                    .symbol
            }
            checked_trees::CheckedStoreRoot::Local { symbol } => {
                local_destination(checked, state, statement_index, symbol)?;
                symbol
            }
        };
        if source.root != destination || store.carrier_path != source.path {
            return unsupported("record store destination drifted from its authored place");
        }
        let declaration = checked
            .data_definitions()
            .iter()
            .find(|data| data.symbol == record_symbol)
            .and_then(|data| {
                checked
                    .data_members(data)
                    .iter()
                    .find_map(|member| match member {
                        checked_trees::data::DataMember::Field(declaration)
                            if declaration.symbol == field.field =>
                        {
                            Some(declaration)
                        }
                        _ => None,
                    })
            })
            .ok_or(LoweringError::Unsupported(
                "record store field declaration is missing",
            ))?;
        let identity = declaration.path_identity();
        if store.field_identity != identity {
            return unsupported("record store field drifted from its literal ordinal");
        }
        let checked_trees::CheckedStructuralRecordFieldValue::Scalar(expected) = field.value else {
            return unsupported("record store field has no scalar computation");
        };
        let Some((handle, role)) = computation_root(
            checked,
            machine,
            state_symbol,
            store.statement_index,
            &store.value,
            store.primitive_type,
            Some(&store.field_identity),
        )?
        else {
            return unsupported("record store replaced its field computation");
        };
        let expected_role = CheckedScalarExpressionRole::RecordField {
            expression: assignment.value,
            field_ordinal: u32::try_from(ordinal).map_err(|_| {
                LoweringError::Unsupported("record store field ordinal exceeds u32")
            })?,
        };
        if handle != expected || role != expected_role {
            return unsupported("record store computation drifted from its literal field");
        }
    }
    Ok(())
}

/// Rejoin a structural local to its unique earlier declaration. This is source
/// identity, not a transfer or mutation permission: a carrier may hold reference
/// fields without owning their backing. Consumers retain the exact declared
/// type and resolve its current live storage in the ordered namespace, then
/// check the selected operation's access, qualifications and field geometry.
/// Finding this declaration cannot restore a moved home or strengthen a loan.
pub(crate) fn local_destination<'a>(
    checked: &'a CheckedTrees,
    state: &checked_trees::state::State,
    statement: u32,
    symbol: symbols::SymbolHandle,
) -> Result<&'a checked_trees::statement::TableLocalData, LoweringError> {
    let statements = checked.statement_table.statements(state.statement_nodes);
    let mut locals =
        statements
            .iter()
            .enumerate()
            .filter_map(|(ordinal, statement)| match statement {
                StatementNode::LocalData(local) if local.symbol == symbol => Some((ordinal, local)),
                _ => None,
            });
    let (ordinal, local) = locals.next().ok_or(LoweringError::Unsupported(
        "record store has no source local",
    ))?;
    if !symbol.is_valid()
        || locals.next().is_some()
        || ordinal >= statement as usize
        || !local.type_reference.is_valid()
        || checked
            .primitive_type_reference(local.type_reference)
            .is_some()
    {
        return unsupported("record store lost its earlier structural declaration");
    }
    Ok(local)
}

/// A `Local` store destination roots two ways: an owned structural local
/// (`let mut`) is its own root with an empty prefix, while a borrowed-view
/// local writes through its loan — the authored target resolves through
/// that same loan, so the destination's root and path must continue the
/// place the view's own initializer resolves to. Returns the root and the
/// path prefix the store's carrier path completes.
pub(crate) fn local_store_destination(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    state: &checked_trees::state::State,
    state_symbol: symbols::SymbolHandle,
    statement: u32,
    symbol: symbols::SymbolHandle,
) -> Result<(symbols::SymbolHandle, Vec<CheckedUnitStructuralPathSegment>), LoweringError> {
    let local = local_destination(checked, state, statement, symbol)?;
    if !matches!(
        checked.expression_table.expression(local.initial_value),
        ExpressionNode::Borrow(_) | ExpressionNode::Cast(_)
    ) {
        return Ok((symbol, Vec::new()));
    }
    let statements = checked.statement_table.statements(state.statement_nodes);
    let declaration = statements
        .iter()
        .position(|statement| {
            matches!(statement, StatementNode::LocalData(local) if local.symbol == symbol)
        })
        .ok_or(LoweringError::Unsupported(
            "record store has no source local",
        ))?;
    let source = crate::emission::call_source_custody::projected_receivers::store_destination(
        checked,
        machine,
        state_symbol,
        Some(declaration),
        local.initial_value,
    )?;
    Ok((source.root, source.path))
}

/// Select a computation only through its exact authored assignment coordinate.
/// A whole-record replacement selects each field through the literal's own
/// `RecordField` root rather than the assignment's role, so the returned role
/// is the one the resolved root actually carries.
pub(crate) fn computation_root(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    state: symbols::SymbolHandle,
    statement_index: u32,
    value: &checked_trees::CheckedStoreValue,
    primitive_type: typed_trees::types::PrimitiveType,
    field_identity: Option<&str>,
) -> Result<
    Option<(
        checked_trees::CheckedScalarComputationHandle,
        CheckedScalarExpressionRole,
    )>,
    LoweringError,
> {
    let role = CheckedScalarExpressionRole::AssignmentValue;
    let plans = &checked.facts.values.scalar_computations;
    let mut roots = plans.roots.iter().map(|(_, root)| root).filter(|root| {
        root.state == state && root.statement_ordinal == statement_index && root.role == role
    });
    let root = roots.next();
    if roots.next().is_some() {
        return unsupported("field assignment has duplicate computation roots");
    }
    let checked_trees::CheckedStoreValue::Computation(handle) = value else {
        return if root.is_none() {
            Ok(None)
        } else {
            unsupported("field assignment replaced its computation with a pure value")
        };
    };
    let handle = *handle;
    let root = match root {
        Some(root) => root,
        None => {
            let mut record_roots = plans.roots.iter().map(|(_, root)| root).filter(|root| {
                root.state == state
                    && root.statement_ordinal == statement_index
                    && root.root == handle
                    && matches!(root.role, CheckedScalarExpressionRole::RecordField { .. })
            });
            let root = record_roots.next().ok_or(LoweringError::Unsupported(
                "field assignment has no computation root",
            ))?;
            if record_roots.next().is_some() {
                return unsupported("field assignment has duplicate record computation roots");
            }
            root
        }
    };
    if root.machine != machine || root.root != handle || !plans.nodes.is_valid(handle) {
        return unsupported("field assignment computation root has different custody");
    }
    if let CheckedScalarExpressionRole::RecordField {
        expression,
        field_ordinal,
    } = root.role
    {
        // The store's declared field must be exactly the literal position the
        // computation root names; an ordinal pointing at a sibling field would
        // otherwise write that field's value into this store's member.
        let ExpressionNode::StructLiteral(literal) =
            checked.expression_table.expression(expression)
        else {
            return unsupported("record store field has no authored literal owner");
        };
        let field = checked
            .expression_table
            .struct_fields(literal.fields)
            .get(field_ordinal as usize)
            .ok_or(LoweringError::Unsupported(
                "record store field ordinal escaped its literal",
            ))?;
        let identity = checked
            .data_definitions()
            .iter()
            .find_map(|data| {
                checked
                    .data_members(data)
                    .iter()
                    .find_map(|member| match member {
                        checked_trees::data::DataMember::Field(declaration)
                            if declaration.symbol == field.field_symbol =>
                        {
                            Some(declaration.path_identity())
                        }
                        _ => None,
                    })
            })
            .ok_or(LoweringError::Unsupported(
                "record store field declaration is missing",
            ))?;
        let field_identity = field_identity.ok_or(LoweringError::Unsupported(
            "record store field has no field identity",
        ))?;
        if identity != field_identity {
            return unsupported("record store field drifted from its literal ordinal");
        }
    }
    let source = crate::expression_preparation::source_custody::locate(
        checked,
        state,
        statement_index,
        root.role,
    )?;
    let node = plans.nodes.get(handle);
    if source.machine != machine
        || source.expression != node.authored_root
        || source.primitive_type != node.primitive_type
        || node.primitive_type != primitive_type
        || source.destination.is_valid()
    {
        return unsupported("field assignment computation differs from its authored RHS");
    }
    crate::expression_preparation::source_custody::validate_computation_calls(
        checked,
        machine,
        state,
        statement_index,
        handle,
        source.expression,
    )?;
    Ok(Some((handle, root.role)))
}

/// The declaration-path identity of the record member a literal initializer
/// names, the identity a store's destination field carries.
pub(crate) fn member_identity(
    checked: &CheckedTrees,
    field_symbol: symbols::SymbolHandle,
) -> Option<String> {
    checked.data_definitions().iter().find_map(|data| {
        checked
            .data_members(data)
            .iter()
            .find_map(|member| match member {
                checked_trees::data::DataMember::Field(declaration)
                    if declaration.symbol == field_symbol =>
                {
                    Some(declaration.path_identity())
                }
                _ => None,
            })
    })
}

#[cfg(test)]
mod local_destination_tests {
    use super::*;

    #[test]
    fn structural_local_identity_does_not_require_transferable_contents() {
        let checked = crate::front_end::checked_program(
            "data Carrier { prefix: u64; destination: &mut u64; }
             data Main {}
             machine Main::write(destination: &mut u64) {
                 let carrier: Carrier = Carrier { prefix: 3, destination: destination };
                 carrier.destination = 11;
             }",
        );
        let machine = checked
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "Main::write")
            .expect("write machine");
        let state = &checked.machine_states(machine)[0];
        let StatementNode::LocalData(local) =
            &checked.statement_table.statements(state.statement_nodes)[0]
        else {
            panic!("carrier declaration");
        };
        let destination = local_destination(&checked, state, 1, local.symbol)
            .expect("reference-bearing storage retains its declared identity");
        assert_eq!(destination.type_reference, local.type_reference);
        assert!(
            local_destination(&checked, state, 0, local.symbol).is_err(),
            "a declaration is not available before its establishment"
        );
        assert!(
            local_destination(&checked, state, 1, machine.symbol).is_err(),
            "another valid symbol cannot substitute for the local declaration"
        );
    }

    #[test]
    fn primitive_locals_do_not_acquire_a_structural_destination() {
        let checked = crate::front_end::checked_program(
            "data Main {}
             machine Main::write() {
                 let mut value: u64 = 0;
                 value = 11;
             }",
        );
        let machine = checked
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "Main::write")
            .expect("write machine");
        let state = &checked.machine_states(machine)[0];
        let StatementNode::LocalData(local) =
            &checked.statement_table.statements(state.statement_nodes)[0]
        else {
            panic!("scalar declaration");
        };
        assert!(
            local_destination(&checked, state, 1, local.symbol).is_err(),
            "primitive locals keep their scalar-storage owner"
        );
    }
}
