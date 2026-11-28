//! Calls compare complete source applications under their own const tuple.
//! The generic body keeps its original symbolic inputs and storage geometry.
use crate::read::refuse_at;

use super::Program;
use crate::body::ValueType;
use crate::source_state::type_identity::NormalizedTypeIdentity;
use crate::source_state::types::TypeReferenceHandle;
use crate::source_state::{SourceTypeApplication, SourceTypeScope};
use std::sync::Arc;
use symbols::SymbolHandle;
use terminal_psi::generic_calls::CallTypePosition;

impl Program<'_> {
    pub(super) fn nominal_argument_identity(
        &self,
        actual: Option<ValueType>,
        expected: ValueType,
        const_sources: &[(SymbolHandle, TypeReferenceHandle)],
    ) -> Option<(NormalizedTypeIdentity, NormalizedTypeIdentity)> {
        let (Some(ValueType::Record(actual, aq, ar)), ValueType::Record(expected, eq, er)) =
            (actual, expected)
        else {
            return None;
        };
        if aq != eq || ar != er {
            return None;
        }
        let actual = self.record_sources.get(actual)?.application.as_ref()?;
        let expected = self.record_sources.get(expected)?.application.as_ref()?;
        let expected = SourceTypeApplication {
            scope: Arc::new(SourceTypeScope {
                parent: Some(expected.scope.clone()),
                substitutions: const_sources.to_vec(),
                rigid_parameters: Vec::new(),
            }),
            ..expected.clone()
        };
        let a = actual.type_identity(self.typed);
        let e = expected.type_identity(self.typed);
        if !a.is_valid() || !e.is_valid() {
            return None;
        }
        Some((a, e))
    }
}

use crate::body::{CallTarget, Expression, ExpressionKind};
use crate::source_state::types::{FixedArrayLength, TypeConstraintNode, TypeReferenceNode};
use crate::{SourceOrigin, Unserved, refuse};
use semantic_vocabulary::{MachineId, ScalarTerm, StructuralTypeId, ValueId};
use terminal_check_tables::{Checked, Construct};

pub(super) struct NominalTypeSource {
    callee: MachineId,
    position: CallTypePosition,
    actual: StructuralTypeId,
    expected: StructuralTypeId,
    actual_constructor: StructuralTypeId,
    expected_constructor: StructuralTypeId,
    actual_application: NormalizedTypeIdentity,
    expected_application: NormalizedTypeIdentity,
    const_arguments: Vec<(ValueId, ScalarTerm)>,
}

pub(super) struct NominalCallSource {
    pub caller: MachineId,
    pub source: SourceOrigin,
    pub types: Vec<NominalTypeSource>,
}

impl Program<'_> {
    fn nominal_constructor(&self, mut ty: StructuralTypeId) -> Option<StructuralTypeId> {
        let mut remaining = self.records.len();
        while remaining != 0 {
            remaining -= 1;
            if self
                .data_declaration(self.record_sources.get(ty)?.declaration)
                .is_some()
            {
                return Some(ty);
            }
            ty = match self.records.get(ty)?.shape {
                crate::StorageShape::Reference {
                    referent: child, ..
                }
                | crate::StorageShape::ElementView { element: child, .. }
                | crate::StorageShape::FixedArray { element: child, .. } => child,
                _ => return None,
            };
        }
        None
    }

    /// Collect the const inputs read by this type occurrence. Identity still
    /// comes from the canonical normalizer, including domains and unused indices.
    fn nominal_const_inputs(
        &self,
        source: &SourceTypeApplication,
        callee: MachineId,
    ) -> Option<Vec<ConstInput>> {
        let inputs = self.const_scopes.get(callee)?;
        let mut pending = vec![(source.reference, source.scope.clone())];
        let mut used = Vec::new();
        let mut visited = std::collections::BTreeSet::new();
        while let Some((reference, scope)) = pending.pop() {
            let supplied = scope.supplied_type_reference(self.typed, reference).ok()?;
            let identity = supplied.type_identity(self.typed);
            if !identity.is_valid() || !visited.insert(identity) {
                continue;
            }
            let scope = supplied.scope;
            let mut child = |reference| pending.push((reference, scope.clone()));
            match self
                .typed
                .type_reference_table
                .type_reference(supplied.reference)
            {
                TypeReferenceNode::Named { symbol, .. } => {
                    if let Some(input) = inputs.iter().find(|input| input.symbol == *symbol) {
                        used.push(ConstInput {
                            symbol: input.symbol,
                            value: input.value,
                        });
                    }
                }
                TypeReferenceNode::Reference { referee, .. } => child(*referee),
                TypeReferenceNode::Slice { element_type } => child(*element_type),
                TypeReferenceNode::FixedArray {
                    element_type,
                    length,
                } => {
                    child(*element_type);
                    match length {
                        FixedArrayLength::Expression(reference) => child(*reference),
                        FixedArrayLength::ConstParameter { symbol, .. } => {
                            if let Some(input) = inputs.iter().find(|input| input.symbol == *symbol)
                            {
                                used.push(ConstInput {
                                    symbol: input.symbol,
                                    value: input.value,
                                });
                            }
                        }
                        FixedArrayLength::Literal(_) => {}
                        FixedArrayLength::ConstCall { .. } => return None,
                    }
                }
                TypeReferenceNode::Generic { arguments, .. } => {
                    for argument in self
                        .typed
                        .type_reference_table
                        .type_reference_handles(*arguments)
                    {
                        child(*argument);
                    }
                }
                TypeReferenceNode::Constrained {
                    base_type,
                    constraints,
                } => {
                    child(*base_type);
                    for constraint in self
                        .typed
                        .type_reference_table
                        .constraint_span(*constraints)?
                    {
                        if let TypeConstraintNode::Domain(domain) = constraint {
                            for argument in &domain.arguments {
                                child(*argument);
                            }
                        }
                    }
                }
                // Literal indices have no machine input. An open expression
                // needs its own exact input accounting before this route admits it.
                TypeReferenceNode::ConstExpression(expression) => {
                    if !matches!(
                        self.typed.expression_table.expression(*expression),
                        crate::source_state::expression::ExpressionNode::Integer(_)
                            | crate::source_state::expression::ExpressionNode::Boolean(_)
                    ) {
                        return None;
                    }
                }
                TypeReferenceNode::Unit => {}
                TypeReferenceNode::Callable { .. } => return None,
            }
        }
        used.sort_unstable_by_key(|input| input.value);
        used.dedup_by_key(|input| input.value);
        Some(used)
    }

    /// Read the declared storage at this invocation’s actual consts. Argument
    /// formation and result construction both need the caller’s element and
    /// invariant identities, while the call receipt retains the callee’s declaration.
    pub(super) fn instantiated_call_type(
        &mut self,
        caller: MachineId,
        callee: &CallTarget,
        declared: ValueType,
        const_sources: &[(SymbolHandle, TypeReferenceHandle)],
    ) -> Checked<Option<ValueType>> {
        let (CallTarget::Machine(callee), ValueType::Record(storage, qualification, regions)) =
            (callee, declared)
        else {
            return Ok(None);
        };
        // Array extent equations and rigid type parameters already have their
        // own call correspondence. Only an applied data constructor introduces
        // a nominal element whose representation must be read in the caller.
        let Some(constructor) = self.nominal_constructor(storage) else {
            return Ok(None);
        };
        if self
            .record_sources
            .get(constructor)
            .and_then(|row| row.constant_input_owner)
            != Some(*callee)
        {
            return Ok(None);
        }
        let Some(source) = self
            .record_sources
            .get(storage)
            .filter(|row| row.constant_input_owner == Some(*callee))
            .and_then(|row| row.application.as_ref())
            .cloned()
        else {
            return Ok(None);
        };
        let scope = Arc::new(SourceTypeScope {
            parent: Some(source.scope),
            substitutions: const_sources.to_vec(),
            rigid_parameters: Vec::new(),
        });
        let saved = std::mem::replace(&mut self.source_scope, scope);
        let result = self.value_type(caller, source.reference);
        self.source_scope = saved;
        let ValueType::Record(storage, ..) = result? else {
            return Err(refuse_at(
                caller,
                Unserved::Call("instantiated call type has no structural storage"),
            ));
        };
        Ok(Some(ValueType::Record(storage, qualification, regions)))
    }

    pub(super) fn nominal_call_type(
        &self,
        caller: MachineId,
        callee: &CallTarget,
        position: CallTypePosition,
        actual: Option<ValueType>,
        expected: ValueType,
        values: &[Expression],
        const_sources: &[(SymbolHandle, TypeReferenceHandle)],
        slots: &[super::Slot],
    ) -> Checked<Option<NominalTypeSource>> {
        let CallTarget::Machine(callee) = callee else {
            return Ok(None);
        };
        let (Some(ValueType::Record(actual_type, ..)), ValueType::Record(expected_type, ..)) =
            (actual, expected)
        else {
            return Ok(None);
        };
        let (Some(actual_constructor), Some(expected_constructor)) = (
            self.nominal_constructor(actual_type),
            self.nominal_constructor(expected_type),
        ) else {
            return Ok(None);
        };
        // Array lengths are checked as geometry at the call. Only inputs of the
        // data constructor need a nominal correspondence, including phantom indices.
        let Some(expected_source) = self
            .record_sources
            .get(expected_constructor)
            .and_then(|source| source.application.as_ref())
        else {
            return Ok(None);
        };
        let Some(used) = self.nominal_const_inputs(expected_source, *callee) else {
            if !values.is_empty() {
                return Err(refuse_at(
                    caller,
                    Unserved::Call("nominal type has no complete const-input account"),
                ));
            }
            return Ok(None);
        };
        if used.is_empty() {
            return Ok(None);
        }
        let Some((actual_application, expected_application)) =
            self.nominal_argument_identity(actual, expected, const_sources)
        else {
            return Ok(None);
        };
        let symbol = self
            .symbols
            .get(*callee)
            .and_then(|symbol| self.typed.machine_by_symbol(*symbol));
        let Some(symbol) = symbol else {
            return Ok(None);
        };
        let parameters = self.typed.machine_type_parameters(symbol);
        let parameters = parameters
            .iter()
            .filter(|parameter| {
                matches!(
                    parameter.kind,
                    crate::source_state::data::TypeParameterKind::Const { .. }
                        | crate::source_state::data::TypeParameterKind::Value { .. }
                ) && self.record_parameter(parameter.symbol).is_none()
            })
            .collect::<Vec<_>>();
        let mut const_arguments = Vec::with_capacity(used.len());
        for input in used {
            let position = parameters
                .iter()
                .position(|parameter| parameter.symbol == input.symbol);
            let Some(value) = position.and_then(|position| values.get(position)) else {
                return Ok(None);
            };
            let scalar_type = value.result.and_then(ValueType::scalar).ok_or_else(|| {
                refuse_at(caller, Unserved::Call("nominal index lost its scalar type"))
            })?;
            let term = match value.kind {
                ExpressionKind::Slot(slot) => {
                    let Some(slot) = slots.get(slot) else {
                        return Ok(None);
                    };
                    let Some(input) = self.const_scopes.get(caller).and_then(|inputs| {
                        inputs
                            .iter()
                            .find(|input| input.owner == caller && input.symbol == slot.symbol)
                    }) else {
                        return Ok(None);
                    };
                    ScalarTerm::value(input.value, input.scalar_type)
                }
                ExpressionKind::Integer(value) => {
                    let semantic_vocabulary::ScalarType::Integer(scalar_type) = scalar_type else {
                        return Err(refuse_at(
                            caller,
                            Unserved::Call("nominal integer index changed its carrier"),
                        ));
                    };
                    ScalarTerm::Integer { value, scalar_type }
                }
                ExpressionKind::Boolean(value) => ScalarTerm::Boolean(value),
                _ => return Ok(None),
            };
            const_arguments.push((input.value, term));
        }
        Ok(Some(NominalTypeSource {
            callee: *callee,
            position,
            actual: actual_type,
            expected: expected_type,
            actual_constructor,
            expected_constructor,
            actual_application,
            expected_application,
            const_arguments,
        }))
    }
}

struct ConstInput {
    symbol: SymbolHandle,
    value: ValueId,
}

fn source_key(
    machine: MachineId,
    node: crate::source_state::SourceNode,
) -> (MachineId, u8, u32, u32) {
    match node {
        crate::source_state::SourceNode::Expression(handle) => {
            (machine, 0, handle.generation(), handle.arena_index())
        }
        crate::source_state::SourceNode::Statement(handle) => {
            (machine, 1, handle.generation(), handle.arena_index())
        }
    }
}

impl Program<'_> {
    pub(crate) fn publish_nominal_calls(
        &self,
        calls: &[crate::source_state::SourceCallCorrespondence],
        folded_calls: &[(
            MachineId,
            semantic_vocabulary::EdgeId,
            SymbolHandle,
            crate::source_state::SourceNode,
        )],
        module: &mut terminal_psi::TerminalModule,
    ) -> Checked<Vec<terminal_psi::generic_calls::NominalCallType>> {
        use terminal_psi::generic_calls::{NominalCallType, NominalConstArgument};
        if self.nominal_calls.is_empty() {
            return Ok(Vec::new());
        }
        let mut consumed = vec![false; self.nominal_calls.len()];
        let mut identities = std::mem::take(&mut module.identities).into_builder();
        let published = (|| {
            let mut sources = self
                .nominal_calls
                .iter()
                .enumerate()
                .filter_map(|(position, call)| {
                    call.source
                        .node()
                        .map(|(_, node)| (source_key(call.caller, node), (position, call)))
                })
                .collect::<Vec<_>>();
            sources.sort_unstable_by_key(|(key, _)| *key);
            let mut operations = module
                .machines
                .iter()
                .flat_map(|machine| {
                    machine
                        .operations()
                        .map(move |operation| ((machine.id, operation.id), operation))
                })
                .collect::<Vec<_>>();
            operations.sort_unstable_by_key(|(key, _)| *key);
            let mut result = Vec::new();
            for call in calls {
                let key = source_key(call.source_member, call.source);
                let first = sources.partition_point(|(candidate, _)| *candidate < key);
                let last = sources.partition_point(|(candidate, _)| *candidate <= key);
                if first == last {
                    continue;
                }
                let semantic_vocabulary::InvocationSite::Operation(operation) = call.site else {
                    return Err(refuse_at(
                        call.machine,
                        Unserved::Call("nominal const call lost its operation coordinate"),
                    ));
                };
                let fault = |detail| {
                    refuse(
                        call.machine,
                        Construct::Operation(operation),
                        Unserved::Call(detail),
                    )
                };
                let operation = operations
                    .binary_search_by_key(&(call.machine, operation), |(key, _)| *key)
                    .ok()
                    .and_then(|position| operations.get(position))
                    .map(|(_, operation)| *operation)
                    .ok_or_else(|| fault("nominal call lost its emitted operation"))?;
                let invocation = operation
                    .declared_invocation()
                    .ok_or_else(|| fault("nominal call lost its declared invocation"))?;
                let semantic_vocabulary::TargetMachineCallable::Machine(callee) = invocation.callee
                else {
                    return Err(fault("nominal call changed its callee kind"));
                };
                for (_, (position, source)) in &sources[first..last] {
                    if source.types.iter().all(|input| input.callee == callee) {
                        consumed[*position] = true;
                    }
                    for input in &source.types {
                        if input.callee != callee {
                            continue;
                        }
                        let callee = module
                            .machines
                            .get(
                                usize::try_from(callee.get() - 1)
                                    .map_err(|_| fault("nominal callee index overflow"))?,
                            )
                            .filter(|machine| machine.id == callee)
                            .ok_or_else(|| fault("nominal call lost its exact callee"))?;
                        let actual_constructor =
                            module
                                .structural_type(input.actual_constructor)
                                .ok_or_else(|| fault("nominal call lost its actual constructor"))?;
                        let expected_constructor = module
                            .structural_type(input.expected_constructor)
                            .ok_or_else(|| fault("nominal call lost its formal constructor"))?;
                        if actual_constructor.declaration_identity.is_absent()
                            || actual_constructor.declaration_identity
                                != expected_constructor.declaration_identity
                        {
                            return Err(fault("nominal call changed its authored constructor"));
                        }
                        let const_arguments = input
                            .const_arguments
                            .iter()
                            .map(|(formal, actual)| {
                                let position = callee
                                    .parameters
                                    .iter()
                                    .position(|parameter| parameter.id == *formal)
                                    .and_then(|position| u32::try_from(position).ok())
                                    .ok_or_else(|| {
                                        fault("nominal call lost its const parameter coordinate")
                                    })?;
                                Ok(NominalConstArgument {
                                    parameter: position,
                                    actual: actual.clone(),
                                })
                            })
                            .collect::<Checked<Vec<_>>>()?;
                        result.push(NominalCallType {
                            caller: call.machine,
                            operation: operation.id,
                            callee: callee.id,
                            position: input.position,
                            actual: input.actual,
                            expected: input.expected,
                            actual_constructor: input.actual_constructor,
                            expected_constructor: input.expected_constructor,
                            declaration: actual_constructor.declaration_identity,
                            actual_application: identities.intern(
                                self.typed
                                    .type_identities
                                    .text(input.actual_application)
                                    .ok_or_else(|| {
                                        fault("nominal actual lost its canonical key")
                                    })?,
                            ),
                            expected_application: identities.intern(
                                self.typed
                                    .type_identities
                                    .text(input.expected_application)
                                    .ok_or_else(|| {
                                        fault("nominal formal lost its canonical key")
                                    })?,
                            ),
                            const_arguments,
                        });
                    }
                }
            }
            for (position, source) in self.nominal_calls.iter().enumerate() {
                if !consumed[position]
                    && let Some((owner, node)) = source.source.node()
                    && folded_calls
                        .iter()
                        .any(|&(_, _, folded_owner, folded_node)| {
                            folded_owner == owner && folded_node == node
                        })
                {
                    consumed[position] = true;
                }
            }
            if let Some(position) = consumed.iter().position(|consumed| !consumed) {
                let source = &self.nominal_calls[position];
                return Err(refuse_at(
                    source.caller,
                    Unserved::Call("nominal call has no complete emitted type correspondence"),
                )
                .with_source_span(source.source.span()));
            }
            result.sort_unstable_by_key(|row| (row.caller, row.operation, row.position));
            if result.windows(2).any(|pair| {
                (pair[0].caller, pair[0].operation, pair[0].position)
                    == (pair[1].caller, pair[1].operation, pair[1].position)
            }) {
                let row = &result[0];
                return Err(refuse(
                    row.caller,
                    Construct::Operation(row.operation),
                    Unserved::Call("nominal call repeats its source correspondence"),
                ));
            }
            Ok(result)
        })();
        module.identities = identities.build();
        published
    }
}

impl NominalTypeSource {
    pub(super) fn agrees(&self) -> bool {
        self.actual_application == self.expected_application
    }
}

impl Program<'_> {
    pub(crate) fn nominal_type_inputs(&self) -> Vec<(MachineId, StructuralTypeId)> {
        let mut result = Vec::new();
        for (owner, signature) in self.signatures.iter() {
            for formal in signature.parameters.iter().chain(signature.result.iter()) {
                if let ValueType::Record(storage, ..) = formal
                    && let Some(constructor) = self.nominal_constructor(*storage)
                    && let Some(source) = self
                        .record_sources
                        .get(constructor)
                        .and_then(|source| source.application.as_ref())
                    && self
                        .nominal_const_inputs(source, owner)
                        .is_some_and(|inputs| !inputs.is_empty())
                {
                    result.push((owner, *storage));
                }
            }
        }
        result.sort_unstable();
        result.dedup();
        result
    }
}
