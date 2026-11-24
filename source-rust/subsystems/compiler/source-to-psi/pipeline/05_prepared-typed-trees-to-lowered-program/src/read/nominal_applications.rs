//! Calls compare complete source applications under their own const tuple.
//! The generic body keeps its original symbolic inputs and storage geometry.

use super::Program;
use crate::lowering::body::ValueType;
use crate::source_state::type_identity::NormalizedTypeIdentity;
use crate::source_state::types::TypeReferenceHandle;
use crate::source_state::{SourceTypeApplication, SourceTypeScope};
use std::sync::Arc;
use symbols::SymbolHandle;

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

use crate::lowering::body::{CallTarget, Expression, ExpressionKind};
use crate::lowering::{SourceOrigin, Unserved, refuse};
use crate::source_state::types::{FixedArrayLength, TypeConstraintNode, TypeReferenceNode};
use semantic_vocabulary::{MachineId, ScalarTerm, StructuralTypeId, ValueId};
use terminal_check_tables::{Checked, Construct};

pub(super) struct NominalArgumentSource {
    callee: MachineId,
    argument: u32,
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
    pub arguments: Vec<NominalArgumentSource>,
}

impl Program<'_> {
    fn nominal_constructor(&self, mut ty: StructuralTypeId) -> Option<StructuralTypeId> {
        let mut remaining = self.records.len();
        while remaining != 0 {
            remaining -= 1;
            if self.record_sources.get(ty)?.declaration.is_valid() {
                return Some(ty);
            }
            ty = match self.records.get(ty)?.shape {
                crate::lowering::StorageShape::Reference { referent, .. } => referent,
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

    pub(super) fn nominal_call_argument(
        &self,
        caller: MachineId,
        callee: &CallTarget,
        argument: u32,
        actual: Option<ValueType>,
        expected: ValueType,
        values: &[Expression],
        const_sources: &[(SymbolHandle, TypeReferenceHandle)],
        slots: &[super::Slot],
    ) -> Checked<Option<NominalArgumentSource>> {
        let CallTarget::Machine(callee) = callee else {
            return Ok(None);
        };
        let (Some(ValueType::Record(actual_type, ..)), ValueType::Record(expected_type, ..)) =
            (actual, expected)
        else {
            return Ok(None);
        };
        let Some(expected_source) = self
            .record_sources
            .get(expected_type)
            .and_then(|source| source.application.as_ref())
        else {
            return Ok(None);
        };
        let Some(used) = self.nominal_const_inputs(expected_source, *callee) else {
            if !values.is_empty() {
                return Err(refuse(
                    caller,
                    Construct::Machine,
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
        let (Some(actual_constructor), Some(expected_constructor)) = (
            self.nominal_constructor(actual_type),
            self.nominal_constructor(expected_type),
        ) else {
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
                refuse(
                    caller,
                    Construct::Machine,
                    Unserved::Call("nominal index lost its scalar type"),
                )
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
                        return Err(refuse(
                            caller,
                            Construct::Machine,
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
        Ok(Some(NominalArgumentSource {
            callee: *callee,
            argument,
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
        module: &mut terminal_psi::TerminalModule,
    ) -> Checked<Vec<terminal_psi::generic_calls::NominalCallArgument>> {
        use terminal_psi::generic_calls::{NominalCallArgument, NominalConstArgument};
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
                    return Err(refuse(
                        call.machine,
                        Construct::Machine,
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
                    if source.arguments.iter().all(|input| input.callee == callee) {
                        consumed[*position] = true;
                    }
                    for input in &source.arguments {
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
                        result.push(NominalCallArgument {
                            caller: call.machine,
                            operation: operation.id,
                            callee: callee.id,
                            argument: input.argument,
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
            if let Some(position) = consumed.iter().position(|consumed| !consumed) {
                let source = &self.nominal_calls[position];
                return Err(refuse(
                    source.caller,
                    Construct::Machine,
                    Unserved::Call("nominal call has no complete emitted type correspondence"),
                )
                .with_source_span(source.source.span()));
            }
            result.sort_unstable_by_key(|row| (row.caller, row.operation, row.argument));
            if result.windows(2).any(|pair| {
                (pair[0].caller, pair[0].operation, pair[0].argument)
                    == (pair[1].caller, pair[1].operation, pair[1].argument)
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

impl NominalArgumentSource {
    pub(super) fn agrees(&self) -> bool {
        self.actual_application == self.expected_application
    }
}

impl Program<'_> {
    pub(crate) fn nominal_type_inputs(&self) -> Vec<(MachineId, StructuralTypeId)> {
        let mut result = Vec::new();
        for (owner, signature) in self.signatures.iter() {
            for formal in &signature.parameters {
                if let ValueType::Record(storage, ..) = formal
                    && self.nominal_constructor(*storage).is_some()
                    && let Some(source) = self
                        .record_sources
                        .get(*storage)
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
