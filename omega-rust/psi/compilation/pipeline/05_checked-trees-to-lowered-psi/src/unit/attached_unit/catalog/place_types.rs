//! Concrete storage demanded by a checked operation's address path.
//!
//! Owning a pointer does not require publishing its referent's layout. Following
//! that pointer does. Walk only the finite checked path in its state namespace;
//! do not recursively expand reference edges in the type catalog. A caller that
//! happens to construct the referent must not supply an isolated callee's types.
use super::*;
use checked_trees::{
    CheckedUnitStructuralArgumentPlan, CheckedUnitStructuralArgumentSourcePlan as Source,
    CheckedUnitStructuralPathSegment as Segment,
};
use typed_trees::type_identity::TypeIdentityId;

pub(super) fn collect<'plans>(
    checked: &'plans CheckedTrees,
    plans: UnitPlans<'_>,
    body: UnitBody<'plans>,
    roots: &mut Vec<TypeIdentityId>,
) -> Result<(), LoweringError> {
    match body {
        UnitBody::Ordinary(machine) => collect_scope(
            checked,
            plans,
            machine.machine,
            machine.state,
            &machine.structural_parameters,
            &machine.operations,
            None,
            roots,
        ),
        UnitBody::Composed(machine) => {
            for state in &machine.states {
                collect_scope(
                    checked,
                    plans,
                    machine.machine,
                    state.state,
                    &state.structural_parameters,
                    &state.operations,
                    Some(state),
                    roots,
                )?;
            }
            Ok(())
        }
    }
}

fn collect_scope<'plans>(
    checked: &'plans CheckedTrees,
    plans: UnitPlans<'_>,
    machine: symbols::SymbolHandle,
    state: symbols::SymbolHandle,
    parameters: &[checked_trees::CheckedUnitStructuralParameterPlan],
    operations: &'plans [CheckedUnitEffectOperationPlan],
    control: Option<&'plans checked_trees::CheckedComposedUnitControlStatePlan>,
    roots: &mut Vec<TypeIdentityId>,
) -> Result<(), LoweringError> {
    let mut collector = StorageDemand {
        checked,
        plans,
        machine,
        state,
        parameters,
        roots,
    };
    let mut namespace = Vec::new();
    collector.operations(operations.iter(), &mut namespace)?;
    if let Some(control) = control {
        use checked_trees::{
            CheckedComposedUnitControlTerminatorPlan as Terminator,
            CheckedStructuralControlTransferSourcePlan as Transfer,
        };
        if let Terminator::Guarded { return_values, .. } = &control.terminator {
            for operation in return_values {
                collector.operations(std::iter::once(operation), &mut namespace.clone())?;
            }
        }
        for arm in control.terminator.return_arms() {
            collector.operations(arm.operation_dependencies(), &mut namespace.clone())?;
        }
        for successor in control.successors() {
            for transfer in &successor.transfers {
                let mut selected = namespace.clone();
                let mut payload;
                let subject = match &transfer.source {
                    Transfer::EstablishedValue {
                        operands,
                        establishment,
                        presentation,
                    } => {
                        collector.operations(
                            operands
                                .iter()
                                .chain(std::iter::once(establishment.as_ref())),
                            &mut selected,
                        )?;
                        Some(presentation)
                    }
                    Transfer::CasePayload {
                        subject,
                        case_identity,
                        field_identity,
                        path,
                    } => {
                        payload = subject.clone();
                        payload.path.extend([
                            Segment::Case(case_identity.clone()),
                            Segment::Field(field_identity.clone()),
                        ]);
                        payload.path.extend_from_slice(path);
                        Some(&payload)
                    }
                    Transfer::ProjectedValue { subject }
                    | Transfer::ProjectedLoan { subject, .. }
                    | Transfer::ViewSubslice(subject) => Some(subject),
                    _ => None,
                };
                if let Some(subject) = subject {
                    collector.argument(Some(successor.statement_ordinal), subject, &selected)?;
                }
            }
        }
    }
    let mut arguments = Vec::new();
    let values = &checked.facts.values.structural_values;
    let value_roots =
        crate::expression_preparation::computation_graph::reachable_structural_values(
            checked, machine,
        )?;
    let computations = &checked.facts.values.scalar_computations;
    for (_, root) in computations
        .roots
        .iter()
        .filter(|(_, root)| root.machine == machine && root.state == state)
    {
        for handle in crate::expression_preparation::computation_graph::reachable_nodes(
            checked,
            &[root.root],
        )? {
            use checked_trees::{
                CheckedScalarComputationKind as Computation,
                CheckedScalarComputationStructuralArgument as Argument,
            };
            match &computations.nodes.get(handle).kind {
                Computation::Call {
                    structural_arguments,
                    ..
                } => {
                    for argument in computations
                        .structural_arguments
                        .span(*structural_arguments)
                        .ok_or(LoweringError::Unsupported(
                            "operation type demand lost its computed argument roster",
                        ))?
                    {
                        if let Argument::Place(subject) = argument {
                            arguments.push((Some(root.statement_ordinal), subject));
                        }
                    }
                }
                Computation::StructuralField {
                    subject,
                    source_type,
                    ..
                } => {
                    // The checked path is relative to its retained source type.
                    // Named borrows already select referent storage there;
                    // completed values retain a descriptor and an explicit
                    // crossing. Do not peel another reference before walking.
                    if subject.path.contains(&Segment::Referent) {
                        let identity = validation::stored_structural_type_identity(
                            &checked.typed,
                            *source_type,
                            &[],
                            &[],
                        );
                        collector.roots.push(identity);
                        collect_path(collector.plans, identity, &subject.path, collector.roots)
                            .map_err(|error| {
                                collector.at_statement(Some(root.statement_ordinal), error)
                            })?;
                    }
                }
                Computation::ByteSequenceLiteralComparison { subject, .. }
                | Computation::ByteSequencePredicate { subject, .. }
                | Computation::CaseMembership {
                    subject: Argument::Place(subject),
                    ..
                } => {
                    arguments.push((Some(root.statement_ordinal), subject));
                }
                _ => {}
            }
        }
    }
    for root in value_roots {
        let value = values.nodes.get(root);
        if value.state == state && value.machine == machine {
            // Computed roots own their temporary result namespace. Later
            // statements and another root may reuse those ordinals.
            let mut prefix = namespace
                .iter()
                .copied()
                .filter(|operation| {
                    operation
                        .source_statement_index()
                        .is_some_and(|statement| statement < value.statement_index)
                })
                .collect::<Vec<_>>();
            collector.value(root, &prefix)?;
            if let Some(calls) = values.calls_for_root(root) {
                collector.calls(calls, &mut prefix)?;
            }
        }
    }
    for (statement, argument) in arguments {
        collector.argument(statement, argument, &namespace)?;
    }
    Ok(())
}

/// Result ordinals are local to one executing operand frame. A selected edge,
/// return arm or constructed value borrows the completed outer prefix and adds
/// only its own results. Sibling frames never contribute type bindings.
struct StorageDemand<'checked, 'plans, 'output> {
    checked: &'checked CheckedTrees,
    plans: UnitPlans<'plans>,
    machine: symbols::SymbolHandle,
    state: symbols::SymbolHandle,
    parameters: &'checked [checked_trees::CheckedUnitStructuralParameterPlan],
    roots: &'output mut Vec<TypeIdentityId>,
}

impl<'checked> StorageDemand<'checked, '_, '_> {
    fn at_statement(&self, statement: Option<u32>, error: LoweringError) -> LoweringError {
        match (statement, error) {
            (Some(statement), LoweringError::Unsupported(reason)) => LoweringError::at_statement(
                self.checked,
                self.machine,
                self.state,
                statement,
                reason,
            ),
            (_, error) => error,
        }
    }

    fn operations(
        &mut self,
        operations: impl Iterator<Item = &'checked CheckedUnitEffectOperationPlan>,
        namespace: &mut Vec<&'checked CheckedUnitEffectOperationPlan>,
    ) -> Result<(), LoweringError> {
        for operation in operations {
            if let CheckedUnitEffectOperationPlan::EstablishLocal(
                checked_trees::CheckedUnitLocalBindingPlan::StructuralValue {
                    value, calls, ..
                },
            ) = operation
            {
                self.value(*value, namespace)?;
                self.calls(calls, &mut namespace.clone())?;
            }
            let subject = match operation {
                CheckedUnitEffectOperationPlan::CaptureAssignmentDestination {
                    destination,
                    ..
                } => Some(destination),
                CheckedUnitEffectOperationPlan::MoveStructuralField { source, .. } => Some(source),
                CheckedUnitEffectOperationPlan::EstablishViewSubslice { source, .. } => {
                    Some(source)
                }
                _ => operation
                    .established_reference_source(&self.checked.facts.values.structural_values),
            };
            for argument in subject.into_iter().chain(operation.structural_operands()) {
                self.argument(operation.source_statement_index(), argument, namespace)?;
            }
            if let CheckedUnitEffectOperationPlan::Store(store) = operation {
                match &store.value {
                    checked_trees::CheckedStoreValue::Place(source) => {
                        self.argument(Some(store.statement_index), source, namespace)?;
                    }
                    checked_trees::CheckedStoreValue::Bytes(value) => {
                        for (_, node) in value.nodes.iter() {
                            if let checked_trees::CheckedByteSequenceValue::Descriptor {
                                source,
                                ..
                            } = node
                            {
                                self.argument(Some(store.statement_index), source, namespace)?;
                            }
                        }
                    }
                    _ => {}
                }
            }
            namespace.push(operation);
        }
        Ok(())
    }

    fn calls(
        &mut self,
        calls: &'checked [checked_trees::CheckedStructuralValueCall],
        namespace: &mut Vec<&'checked CheckedUnitEffectOperationPlan>,
    ) -> Result<(), LoweringError> {
        // One value's later calls can use its earlier completed calls. Only
        // the containing value boundary discards this temporary namespace.
        for call in calls {
            self.operations(
                call.operands()
                    .iter()
                    .chain(std::iter::once(call.operation())),
                namespace,
            )?;
        }
        Ok(())
    }

    fn value(
        &mut self,
        root: checked_trees::CheckedStructuralValueHandle,
        namespace: &[&CheckedUnitEffectOperationPlan],
    ) -> Result<(), LoweringError> {
        let values = &self.checked.facts.values.structural_values;
        for handle in values
            .value_dependencies(root)
            .ok_or(LoweringError::Unsupported(
                "operation type demand lost its structural value dependencies",
            ))?
        {
            let value = values.nodes.get(handle);
            use checked_trees::CheckedStructuralValueKind as Value;
            if let Value::Projection { source, path, .. } = &value.kind
                && path.contains(&Segment::Referent)
            {
                let source = values.nodes.get(*source);
                if let Value::CopiedStructuralPlace { source: copied, .. } = &source.kind {
                    // A copied projection names existing storage; its source
                    // node is provenance, not a materialized reference carrier.
                    // Compose in that storage namespace exactly as emission does.
                    let mut selected = copied.clone();
                    selected.path.extend_from_slice(path);
                    self.argument(Some(value.statement_index), &selected, namespace)?;
                } else {
                    let identity = validation::stored_structural_type_identity(
                        &self.checked.typed,
                        source.type_reference,
                        &[],
                        &[],
                    );
                    self.roots.push(identity);
                    collect_path(self.plans, identity, path, self.roots)?;
                }
            }
            let argument = match &value.kind {
                Value::Reference { source, .. }
                | Value::Place(source)
                | Value::CopiedStructuralPlace { source, .. }
                | Value::BorrowedSliceView { source, .. }
                | Value::MutableSliceView { source, .. } => Some(source),
                _ => None,
            };
            if let Some(argument) = argument {
                self.argument(Some(value.statement_index), argument, namespace)?;
            }
        }
        Ok(())
    }

    fn argument(
        &mut self,
        statement: Option<u32>,
        argument: &CheckedUnitStructuralArgumentPlan,
        namespace: &[&CheckedUnitEffectOperationPlan],
    ) -> Result<(), LoweringError> {
        if !argument.path.contains(&Segment::Referent) {
            // Inline children already belong to their owner's declaration.
            return Ok(());
        }
        let root = source_type(
            self.checked,
            self.state,
            statement,
            self.parameters,
            namespace,
            argument,
        )?;
        let mut path = argument.path.as_slice();
        if let Source::StructuralLocalBorrow { symbol, .. } = argument.source
            && matches!(path.first(), Some(Segment::Referent))
            && let Some(local) = self.checked.facts.state_locals.named(self.state, symbol)
        {
            let carrier = validation::stored_structural_type_identity(
                &self.checked.typed,
                local.declared_type,
                &[],
                &[],
            );
            if carrier != root
                && matches!(selected_declaration(self.plans, carrier)?.shape,
                    CheckedUnitStructuralTypeShape::Reference { referent_identity, .. } if referent_identity == root)
            {
                // This path is in the local's logical referent namespace.
                // Its leading carrier presentation was consumed at that
                // boundary; it is not a second dereference of the referent.
                path = &path[1..];
            }
        }
        self.roots.push(root);
        collect_path(self.plans, root, path, self.roots)
            .map_err(|error| self.at_statement(statement, error))
    }
}

fn source_type(
    checked: &CheckedTrees,
    state: symbols::SymbolHandle,
    statement: Option<u32>,
    parameters: &[checked_trees::CheckedUnitStructuralParameterPlan],
    operations: &[&CheckedUnitEffectOperationPlan],
    argument: &CheckedUnitStructuralArgumentPlan,
) -> Result<TypeIdentityId, LoweringError> {
    let identity = match argument.source {
        Source::Parameter { parameter_index } => parameters
            .get(parameter_index as usize)
            .map(|parameter| parameter.type_identity),
        Source::StructuralLocalBorrow {
            root_type_identity, ..
        } => Some(root_type_identity),
        Source::StructuralLocal { symbol } => checked
            .state_by_symbol(state)
            .and_then(|state| {
                let formals = checked.state_parameters(state);
                parameters
                    .iter()
                    .find(|parameter| {
                        formals
                            .get(parameter.position as usize)
                            .is_some_and(|formal| formal.symbol == symbol)
                    })
                    .map(|parameter| parameter.type_identity)
            })
            .or_else(|| {
                operations.iter().find_map(|operation| {
                    operation
                        .statement_structural_result_binding()
                        .filter(|result| result.local.symbol == symbol)
                        .map(|result| result.type_identity)
                })
            }),
        Source::StructuralResult { binding_ordinal } => operations.iter().find_map(|operation| {
            operation
                .structural_result_binding()
                .filter(|result| result.binding_ordinal == binding_ordinal)
                .map(|result| result.type_identity)
        }),
        Source::StructuralExpression { expression } => statement
            .and_then(|statement| {
                checked
                    .facts
                    .values
                    .structural_values
                    .root_for_expression(state, statement, expression)
            })
            .map(|root| {
                validation::stored_structural_type_identity(
                    &checked.typed,
                    root.type_reference,
                    &[],
                    &[],
                )
            }),
        Source::ByteSequenceSubslice { root, .. } | Source::ElementViewSubslice { root, .. } => {
            match root {
                checked_trees::CheckedStorageRoot::Parameter { index } => parameters
                    .get(index as usize)
                    .map(|parameter| parameter.type_identity),
                checked_trees::CheckedStorageRoot::OwnedLocal { symbol }
                | checked_trees::CheckedStorageRoot::ViewLocal { symbol } => {
                    operations.iter().find_map(|operation| {
                        operation
                            .statement_structural_result_binding()
                            .filter(|result| result.local.symbol == symbol)
                            .map(|result| result.type_identity)
                    })
                }
            }
        }
        _ => None,
    };
    identity.ok_or(LoweringError::Unsupported(
        "operation storage path has no retained source type",
    ))
}

fn collect_path(
    plans: UnitPlans<'_>,
    mut identity: TypeIdentityId,
    path: &[Segment],
    roots: &mut Vec<TypeIdentityId>,
) -> Result<(), LoweringError> {
    let mut payload = None;
    for (position, segment) in path.iter().enumerate() {
        let declaration = selected_declaration(plans, identity)?;
        match segment {
            Segment::RepresentationView { type_identity, .. } => {
                identity = *type_identity;
                roots.push(identity);
            }
            Segment::FixedByteRange { .. } if position + 1 == path.len() => return Ok(()),
            Segment::FixedByteRange { .. } | Segment::AllElements => {
                return unsupported("operation storage path has a non-addressable segment");
            }
            Segment::Referent => {
                let CheckedUnitStructuralTypeShape::Reference {
                    referent_identity, ..
                } = declaration.shape
                else {
                    return unsupported("operation storage path crosses a non-reference");
                };
                identity = referent_identity;
                roots.push(identity);
            }
            Segment::Case(member) => {
                let (CheckedUnitStructuralTypeShape::Sum { cases }
                | CheckedUnitStructuralTypeShape::Mixed { cases, .. }) = &declaration.shape
                else {
                    return unsupported("operation storage path selects a case of a non-sum");
                };
                let mut matching = cases.iter().filter(|case| case.identity == *member);
                payload = Some(
                    matching
                        .next()
                        .ok_or(LoweringError::Unsupported(
                            "operation storage path case is absent",
                        ))?
                        .fields
                        .as_slice(),
                );
                if matching.next().is_some() {
                    return unsupported("operation storage path case is ambiguous");
                }
            }
            Segment::Field(member) => {
                let fields = match payload.take() {
                    Some(fields) => fields,
                    None => match &declaration.shape {
                        CheckedUnitStructuralTypeShape::Record { fields, .. }
                        | CheckedUnitStructuralTypeShape::Mixed { fields, .. } => fields.as_slice(),
                        _ => return unsupported("operation storage path field has no record"),
                    },
                };
                let mut matching = fields.iter().filter(|field| field.identity == *member);
                let field = matching.next().ok_or(LoweringError::Unsupported(
                    "operation storage path field is absent",
                ))?;
                if matching.next().is_some() || field.relevance.is_erased() {
                    return unsupported("operation storage path field is ambiguous or erased");
                }
                if position + 1 == path.len() {
                    // Scalar fields have no structural row of their own. The
                    // enclosing declaration retains their storage contract.
                    return Ok(());
                }
                identity = field.field_type.structural_type_identity().ok_or(
                    LoweringError::Unsupported("operation storage path crosses a scalar field"),
                )?;
            }
            Segment::FixedIndex(_) | Segment::RuntimeIndex(_) | Segment::ComputedIndex(_) => {
                identity = match declaration.shape {
                    CheckedUnitStructuralTypeShape::FixedArray {
                        element_type_identity,
                        ..
                    }
                    | CheckedUnitStructuralTypeShape::BorrowedSliceView {
                        element_type_identity,
                        ..
                    } => element_type_identity,
                    _ => return unsupported("operation storage path index has no element type"),
                };
            }
        }
    }
    Ok(())
}
