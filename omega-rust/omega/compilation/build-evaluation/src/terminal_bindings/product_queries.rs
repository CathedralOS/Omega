//! Pause for an exact source query and retain only issued description custody.

use super::{BindingState, TerminalBuildBindingHost};
use evaluation_accounting::EvaluationStorageLease;
use language_semantics::declaration_selection::{
    DescribedProductEntry, DescribedProductProvider, DescribedProductSchema,
};
use psi_compiler::{TerminalBuildQueryCall, TerminalBuildQueryOperation};
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
}

#[derive(Debug, Clone)]
pub enum BuildProductDescription {
    Entry(DescribedProductEntry),
    Schema(DescribedProductSchema),
    Provider(DescribedProductProvider),
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

impl TerminalBuildBindingHost<'_> {
    pub(crate) fn awaiting_query(&self) -> Result<Option<Arc<BuildProductQuery>>, String> {
        let state = self
            .state
            .lock()
            .map_err(|_| "Build binding activation lock was poisoned")?;
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
        let mut state = self
            .state
            .lock()
            .map_err(|_| "Build binding activation lock was poisoned")?;
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
        }
        .and_then(|bytes| u64::try_from(bytes).ok())
        .ok_or("Product description payload size overflowed")?;
        query.answer_storage = self
            .account
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
        let request = self.query_arguments(call.action, effect)?;
        let mut state = self.state.lock().map_err(|_| {
            TerminalEffectRejection::new("Build binding activation lock was poisoned")
        })?;
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
        let request = self.query_arguments(call.action, effect)?;
        let TerminalEffect::BoundaryCall {
            result: terminal_psi::BoundaryMachineResult::Structural(result),
            ..
        } = effect
        else {
            return Err(refuse(
                "Product query result is not its structural description",
            ));
        };
        let mut state = self
            .state
            .lock()
            .map_err(|_| refuse("Build binding activation lock was poisoned"))?;
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
        action: TerminalBuildQueryOperation,
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
        if !arguments.is_empty()
            || structural_arguments.first() != self.product_facet.as_ref()
            || self.product_facet.is_none()
            || byte_sequence_arguments.first() != Some(&None)
        {
            return Err(refuse(
                "product selection requires the compiler-issued Build.product facet",
            ));
        }
        let arity = match action {
            TerminalBuildQueryOperation::Entry => 3,
            TerminalBuildQueryOperation::Schema | TerminalBuildQueryOperation::Provider => 2,
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
        let state = self
            .state
            .lock()
            .map_err(|_| refuse("Build binding activation lock was poisoned"))?;
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
        terminal_interpreter::TerminalByteSequenceValue::copy(path.as_bytes(), self.account)
            .map(TerminalEffectResult::ByteSequence)
            .map_err(|error| {
                TerminalEffectRejection::new(format!("description path storage: {error}"))
            })
    }
}
