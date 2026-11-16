/*
Product query suspension and issued description custody.

When a Build activation queries a product entry, schema, or provider, we
authenticate its compiler-issued `Build.product` facet, reserve query storage,
and pause the activation until the coordinator supplies the admitted
`BuildProductDescription`. Completing the query mints an opaque structural
description handle whose provenance we retain in `descriptions` so subsequent
root bindings or path inspections can verify that the handle was issued by this
activation.
*/

use super::{BindingState, TerminalBuildBindingHost, lock_state};
use evaluation_accounting::EvaluationStorageLease;
use language_semantics::declaration_selection::{
    DescribedProductEntry, DescribedProductProvider, DescribedProductSchema,
};
use source_compiler::{TerminalBuildQueryCall, TerminalBuildQueryOperation};
use std::sync::Arc;
use terminal_interpreter::{
    TerminalEffect, TerminalEffectRejection, TerminalEffectResult, TerminalStructuralValue,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildProductQuery {
    pub operation: TerminalBuildQueryOperation,
    pub occurrence: source::SourceSpan,
    pub path: Vec<u8>,
    pub slot: Option<Vec<u8>>,
    pub selecting_machine: Option<source::SourceSpan>,
}

#[derive(Debug, Clone)]
pub enum BuildProductDescription {
    Entry(DescribedProductEntry),
    Schema(DescribedProductSchema),
    Provider(DescribedProductProvider),
    Parameter(semantic_vocabulary::ParameterDescription),
    Field(semantic_vocabulary::FieldInfoDescription),
}

pub(super) struct PendingQuery {
    request: Arc<BuildProductQuery>,
    answer: Option<BuildProductDescription>,
    _request_storage: Option<EvaluationStorageLease>,
    answer_storage: Option<EvaluationStorageLease>,
}

pub(super) struct IssuedDescription {
    value: TerminalStructuralValue,
    description: BuildProductDescription,
    _storage: Option<EvaluationStorageLease>,
}

struct QueryArguments<'effect> {
    path: &'effect [u8],
    slot: Option<&'effect [u8]>,
}

impl QueryArguments<'_> {
    fn matches(&self, call: &TerminalBuildQueryCall, request: &BuildProductQuery) -> bool {
        request.operation == call.action
            && request.occurrence == call.occurrence
            && request.path == self.path
            && request.slot.as_deref() == self.slot
            && request.selecting_machine == call.selecting_machine
    }
}

impl BindingState {
    pub(super) fn entry_description(
        &self,
        value: &TerminalStructuralValue,
    ) -> Result<DescribedProductEntry, TerminalEffectRejection> {
        let selected = self
            .descriptions
            .iter()
            .find(|description| &description.value == value);
        match selected.map(|selected| &selected.description) {
            Some(BuildProductDescription::Entry(entry)) => Ok(entry.clone()),
            _ => Err(TerminalEffectRejection::new(
                "root binding operand is not a compiler-issued product entry description",
            )),
        }
    }
}

impl TerminalBuildBindingHost {
    pub(crate) fn awaiting_query(&self) -> Result<Option<Arc<BuildProductQuery>>, String> {
        let state = lock_state(&self.state)?;
        Ok(state
            .query
            .as_ref()
            .filter(|query| query.answer.is_none())
            .map(|query| Arc::clone(&query.request)))
    }

    pub(crate) fn answer_query(
        &self,
        request: &BuildProductQuery,
        answer: BuildProductDescription,
    ) -> Result<(), String> {
        let mut state = lock_state(&self.state)?;
        let query = state
            .query
            .as_mut()
            .ok_or("Build has no pending Product query")?;
        if query.request.as_ref() != request || query.answer.is_some() {
            return Err("Product answer belongs to another query or was already installed".into());
        }
        let role_matches = matches!(
            (request.operation, &answer),
            (
                TerminalBuildQueryOperation::Entry,
                BuildProductDescription::Entry(_)
            ) | (
                TerminalBuildQueryOperation::Schema,
                BuildProductDescription::Schema(_)
            ) | (
                TerminalBuildQueryOperation::Provider,
                BuildProductDescription::Provider(_)
            ) | (
                TerminalBuildQueryOperation::Parameter,
                BuildProductDescription::Parameter(_)
            ) | (
                TerminalBuildQueryOperation::Field,
                BuildProductDescription::Field(_)
            )
        );
        if !role_matches {
            return Err("Product answer has the wrong description role".into());
        }
        let payload = match &answer {
            BuildProductDescription::Entry(entry) => {
                entry.machine_name.len().checked_add(entry.slot.len())
            }
            BuildProductDescription::Schema(schema) => Some(schema.canonical_path.len()),
            BuildProductDescription::Provider(provider) => Some(provider.canonical_path.len()),
            BuildProductDescription::Parameter(value) => [
                value.requirement_identity.len(),
                value.requirement_application.len(),
                value.parameter_identity.len(),
                value.qualified_type.len(),
                value.source_revision.len(),
            ]
            .into_iter()
            .try_fold(0usize, usize::checked_add),
            BuildProductDescription::Field(value) => [
                value.member_identity.len(),
                value.case_identity.len(),
                value.owner_identity.len(),
                value.owner_application.len(),
                value.name.len(),
                value.qualified_type.len(),
                value.source_revision.len(),
            ]
            .into_iter()
            .try_fold(0usize, usize::checked_add),
        }
        .and_then(|bytes| u64::try_from(bytes).ok())
        .ok_or("Product description payload size overflowed")?;
        query.answer_storage = self
            .account
            .as_ref()
            .map(|account| account.reserve_storage(1, payload))
            .transpose()
            .map_err(|error| format!("Product description storage: {error}"))?;
        query.answer = Some(answer);
        Ok(())
    }

    pub(super) fn poll_query(
        &self,
        call: &TerminalBuildQueryCall,
        effect: &TerminalEffect,
    ) -> Result<bool, TerminalEffectRejection> {
        if matches!(
            call.action,
            TerminalBuildQueryOperation::SchemaPath | TerminalBuildQueryOperation::ProviderPath
        ) {
            return Ok(true);
        }
        let request = self.query_arguments(call, effect)?;
        let mut state = lock_state(&self.state).map_err(TerminalEffectRejection::new)?;
        if let Some(pending) = &state.query {
            if !request.matches(call, &pending.request) {
                return Err(TerminalEffectRejection::new(
                    "Build activation already awaits another Product query",
                ));
            }
            return Ok(pending.answer.is_some());
        }
        let payload = request
            .path
            .len()
            .checked_add(request.slot.map_or(0, <[u8]>::len))
            .and_then(|bytes| u64::try_from(bytes).ok())
            .ok_or_else(|| TerminalEffectRejection::new("Product query payload size overflowed"))?;
        let storage = self
            .account
            .as_ref()
            .map(|account| account.reserve_storage(1, payload))
            .transpose()
            .map_err(|error| {
                TerminalEffectRejection::new(format!("Product query storage: {error}"))
            })?;
        let copy = |bytes: &[u8]| {
            let mut owned = Vec::new();
            owned
                .try_reserve_exact(bytes.len())
                .map_err(|_| TerminalEffectRejection::new("cannot retain Product query bytes"))?;
            owned.extend_from_slice(bytes);
            Ok::<_, TerminalEffectRejection>(owned)
        };
        state.query = Some(PendingQuery {
            request: Arc::new(BuildProductQuery {
                operation: call.action,
                occurrence: call.occurrence,
                path: copy(request.path)?,
                slot: request.slot.map(copy).transpose()?,
                selecting_machine: call.selecting_machine,
            }),
            answer: None,
            _request_storage: storage,
            answer_storage: None,
        });
        Ok(false)
    }

    pub(super) fn complete_query(
        &self,
        call: &TerminalBuildQueryCall,
        effect: &TerminalEffect,
    ) -> Result<TerminalEffectResult, TerminalEffectRejection> {
        if matches!(
            call.action,
            TerminalBuildQueryOperation::SchemaPath | TerminalBuildQueryOperation::ProviderPath
        ) {
            return self.inspect_description(call.action, effect);
        }
        let refuse = TerminalEffectRejection::new;
        let request = self.query_arguments(call, effect)?;
        let TerminalEffect::BoundaryCall {
            result: terminal_psi::BoundaryMachineResult::Structural(result),
            ..
        } = effect
        else {
            return Err(refuse(
                "Product query result is not its structural description",
            ));
        };
        let mut state = lock_state(&self.state).map_err(refuse)?;
        let pending = state
            .query
            .take()
            .ok_or_else(|| refuse("Product query has no pending request"))?;
        if !request.matches(call, &pending.request) {
            return Err(refuse(
                "Product query completion belongs to another request",
            ));
        }
        let description = pending
            .answer
            .ok_or_else(|| refuse("Product query has no admitted answer"))?;
        if matches!(
            call.action,
            TerminalBuildQueryOperation::Parameter | TerminalBuildQueryOperation::Field
        ) {
            return description_contents(call, result.structural_type, description);
        }
        let identity = u64::MAX
            .checked_sub(
                u64::try_from(state.descriptions.len())
                    .map_err(|_| refuse("Product description identity overflow"))?,
            )
            .filter(|identity| *identity > 1)
            .ok_or_else(|| refuse("Product description identity space exhausted"))?;
        state
            .descriptions
            .try_reserve(1)
            .map_err(|_| refuse("cannot retain Product description"))?;
        let value = TerminalStructuralValue::minted(identity, result.structural_type, Vec::new());
        state.descriptions.push(IssuedDescription {
            value: value.clone(),
            description,
            _storage: pending.answer_storage,
        });
        Ok(TerminalEffectResult::Structural(value))
    }

    fn query_arguments<'effect>(
        &self,
        call: &'effect TerminalBuildQueryCall,
        effect: &'effect TerminalEffect,
    ) -> Result<QueryArguments<'effect>, TerminalEffectRejection> {
        let refuse = TerminalEffectRejection::new;
        let TerminalEffect::BoundaryCall {
            arguments,
            structural_arguments,
            byte_sequence_arguments,
            ..
        } = effect
        else {
            return Err(refuse("not a Product query effect"));
        };
        if matches!(
            call.action,
            TerminalBuildQueryOperation::Parameter | TerminalBuildQueryOperation::Field
        ) {
            let selected = call
                .request
                .as_ref()
                .ok_or_else(|| refuse("declaration query has no compiler-selected operand"))?;
            selected
                .validate_shape()
                .map_err(TerminalEffectRejection::new)?;
            let expected = if call.action == TerminalBuildQueryOperation::Parameter {
                language_semantics::declaration_selection::BuildOperation::ParameterDescription
            } else {
                language_semantics::declaration_selection::BuildOperation::FieldDescription
            };
            if selected.operation != expected
                || call.selecting_machine.is_none()
                || call.result_shape.is_none()
                || !arguments.is_empty()
                || !structural_arguments.is_empty()
                || !byte_sequence_arguments.is_empty()
            {
                return Err(refuse(
                    "declaration query changed its selected role or argument roster",
                ));
            }
            let [operand] = selected.operands.as_ref() else {
                return Err(refuse("declaration query has no exact operand"));
            };
            return Ok(QueryArguments {
                path: operand.path.as_bytes(),
                slot: None,
            });
        }
        if call.request.is_some() || call.selecting_machine.is_some() || call.result_shape.is_some()
        {
            return Err(refuse(
                "value query carries another operation's designated operands",
            ));
        }
        if !arguments.is_empty()
            || structural_arguments.first() != self.product_facet.as_ref()
            || self.product_facet.is_none()
            || byte_sequence_arguments.first() != Some(&None)
        {
            return Err(refuse(
                "product selection requires the compiler-issued Build.product facet",
            ));
        }
        let arity = match call.action {
            TerminalBuildQueryOperation::Entry => 3,
            TerminalBuildQueryOperation::Schema | TerminalBuildQueryOperation::Provider => 2,
            TerminalBuildQueryOperation::Parameter | TerminalBuildQueryOperation::Field => {
                unreachable!("designated queries returned above")
            }
            TerminalBuildQueryOperation::SchemaPath | TerminalBuildQueryOperation::ProviderPath => {
                return Err(refuse("description inspection is not a source query"));
            }
        };
        if structural_arguments.len() != arity || byte_sequence_arguments.len() != arity {
            return Err(refuse("Product query has an invalid argument roster"));
        }
        let bytes = |position| {
            byte_sequence_arguments
                .get(position)
                .and_then(Option::as_ref)
                .ok_or_else(|| refuse("Product query lost an evaluated byte argument"))
        };
        Ok(QueryArguments {
            path: bytes(1)?,
            slot: if arity == 3 { Some(bytes(2)?) } else { None },
        })
    }

    fn inspect_description(
        &self,
        action: TerminalBuildQueryOperation,
        effect: &TerminalEffect,
    ) -> Result<TerminalEffectResult, TerminalEffectRejection> {
        let refuse = TerminalEffectRejection::new;
        let TerminalEffect::BoundaryCall {
            arguments,
            structural_arguments,
            byte_sequence_arguments,
            ..
        } = effect
        else {
            return Err(refuse("not a description inspection effect"));
        };
        let [value] = structural_arguments.as_slice() else {
            return Err(refuse(
                "description inspection requires one explicit description",
            ));
        };
        if !arguments.is_empty() || byte_sequence_arguments.as_slice() != [None] {
            return Err(refuse(
                "description inspection has an invalid argument roster",
            ));
        }
        let state = lock_state(&self.state).map_err(refuse)?;
        let issued = state
            .descriptions
            .iter()
            .find(|issued| &issued.value == value);
        let path = match (action, issued.map(|issued| &issued.description)) {
            (
                TerminalBuildQueryOperation::SchemaPath,
                Some(BuildProductDescription::Schema(schema)),
            ) => &schema.canonical_path,
            (
                TerminalBuildQueryOperation::ProviderPath,
                Some(BuildProductDescription::Provider(provider)),
            ) => &provider.canonical_path,
            (TerminalBuildQueryOperation::SchemaPath, _) => {
                return Err(refuse(
                    "product-schema path access requires a compiler-issued ProductTypeSchema",
                ));
            }
            _ => {
                return Err(refuse(
                    "product-provider path access requires a compiler-issued ProductProviderRef",
                ));
            }
        };
        terminal_interpreter::TerminalByteSequenceValue::copy(
            path.as_bytes(),
            self.account.as_ref(),
        )
        .map(TerminalEffectResult::ByteSequence)
        .map_err(|error| TerminalEffectRejection::new(format!("description path storage: {error}")))
    }
}

fn description_contents(
    call: &TerminalBuildQueryCall,
    structural_type: semantic_vocabulary::StructuralTypeId,
    answer: BuildProductDescription,
) -> Result<TerminalEffectResult, TerminalEffectRejection> {
    use semantic_vocabulary::{
        CanonicalStructuralPathSegment as Path, IntegerType, IntegerValue, ScalarType,
    };
    use source_compiler::TerminalBuildValueShape;
    use terminal_interpreter::{
        TerminalAggregateValue, TerminalBoundaryStorageValue, TerminalScalarValue,
        TerminalStorageContents,
    };
    let refuse = TerminalEffectRejection::new;
    let bytes = |value: Vec<u8>| TerminalStorageContents::Bytes(Arc::from(value));
    let word = |value: u64| TerminalStorageContents::Scalar {
        value: TerminalScalarValue::Integer {
            scalar_type: IntegerType::new(semantic_vocabulary::IntegerSign::Unsigned, 64)
                .expect("u64 is a supported carrier"),
            value: IntegerValue::Unsigned(u128::from(value)),
        },
        stored_scalar: ScalarType::Integer(
            IntegerType::new(semantic_vocabulary::IntegerSign::Unsigned, 64)
                .expect("u64 is a supported carrier"),
        ),
    };
    let boolean = |value: bool| TerminalStorageContents::Scalar {
        value: TerminalScalarValue::Boolean(value),
        stored_scalar: ScalarType::Boolean,
    };
    let values = match answer {
        BuildProductDescription::Parameter(value) => vec![
            ("requirement_identity", bytes(value.requirement_identity)),
            (
                "requirement_application",
                bytes(value.requirement_application),
            ),
            ("parameter_identity", bytes(value.parameter_identity)),
            ("qualified_type", bytes(value.qualified_type)),
            ("declaration_position", word(value.declaration_position)),
            ("source_revision", bytes(value.source_revision)),
        ],
        BuildProductDescription::Field(value) => vec![
            ("member_identity", bytes(value.member_identity)),
            ("case_identity", bytes(value.case_identity)),
            ("owner_identity", bytes(value.owner_identity)),
            ("owner_application", bytes(value.owner_application)),
            ("name", bytes(value.name)),
            ("qualified_type", bytes(value.qualified_type)),
            ("declaration_position", word(value.declaration_position)),
            ("has_stable_number", boolean(value.has_stable_number)),
            ("is_erased", boolean(value.is_erased)),
            ("stable_number", word(value.stable_number)),
            ("source_revision", bytes(value.source_revision)),
        ],
        _ => return Err(refuse("record query received another description role")),
    };
    let Some(TerminalBuildValueShape::Record { fields }) = call.result_shape.as_deref() else {
        return Err(refuse("declaration query lost its checked record shape"));
    };
    if fields.len() != values.len() {
        return Err(refuse("declaration query has another result field roster"));
    }
    let mut contents = Vec::with_capacity(fields.len());
    let mut used = vec![false; values.len()];
    for field in fields {
        let mut matching = values
            .iter()
            .enumerate()
            .filter(|(_, (name, _))| *name == field.name);
        let (index, (_, value)) = matching
            .next()
            .ok_or_else(|| refuse("declaration query has an unknown result field"))?;
        if matching.next().is_some() || std::mem::replace(&mut used[index], true) {
            return Err(refuse("declaration query repeats a result field"));
        }
        let matches = match (field.shape.as_ref(), value) {
            (TerminalBuildValueShape::Bytes, TerminalStorageContents::Bytes(_)) => true,
            (
                TerminalBuildValueShape::Scalar(expected),
                TerminalStorageContents::Scalar { stored_scalar, .. },
            ) => expected == stored_scalar,
            _ => false,
        };
        if !matches {
            return Err(refuse("declaration query changed its result field type"));
        }
        contents.push(TerminalBoundaryStorageValue {
            path: vec![Path::Field(field.id)],
            contents: value.clone(),
        });
    }
    Ok(TerminalEffectResult::Aggregate(TerminalAggregateValue {
        structural_type,
        result_case: None,
        fields: contents,
    }))
}
