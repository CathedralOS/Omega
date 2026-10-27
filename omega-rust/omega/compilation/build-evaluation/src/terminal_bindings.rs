//! Build binding effects over ordinary Terminal imports.
//!
//! Source production supplies designated-operand custody beside the artifact.
//! An activation authenticates its original receiver before evaluating a
//! delegated description; commit consumes the resulting private ticket. Neither
//! an integer nor a matching debug name grants selection authority. Records
//! remain inert until the product owner resolves and admits them.

use language_semantics::declaration_selection::{
    BuildOperation, BuildSelectionRequest, CompositionMode, ExecutedBuildSelection,
    ExecutedProductSelection, ExecutedRootBinding, ProductDeclarationRequest, RootBindingOperand,
    RootBindingSelection,
};
use psi_compiler::{
    TerminalBuildBindingCall, TerminalBuildBindingOperation, TerminalBuildBindings,
};
use std::sync::Mutex;
use terminal_interpreter::{
    TerminalEffect, TerminalEffectHandler, TerminalEffectRejection, TerminalEffectResult,
    TerminalImportAdapter, TerminalImportSignature, TerminalScalarValue, TerminalStructuralValue,
};

mod product_queries;
pub use product_queries::{BuildProductDescription, BuildProductQuery};

/// One activation shared by its prepare/commit imports, never across builds.
/// The interpreter owns each adapter; the host retains this common state and
/// takes completed requests after dropping those adapters. A lock makes that
/// sharing explicit without granting either adapter mutable aliasing.
pub struct TerminalBuildBindingHost<'context> {
    calls: &'context [TerminalBuildBindingCall],
    queries: &'context [psi_compiler::TerminalBuildQueryCall],
    product_facet: Option<TerminalStructuralValue>,
    account: Option<&'context evaluation_accounting::EvaluationInvocationAccount>,
    declarations: &'context [std::sync::Arc<ProductDeclarationRequest>],
    original: TerminalStructuralValue,
    state: Mutex<BindingState>,
}

#[derive(Default)]
struct BindingState {
    pending: Vec<Option<psi_compiler::TerminalBuildSelectionRequest>>,
    executed: Vec<ExecutedBuildSelection>,
    awaiting_provider: Option<usize>,
    installed_providers: Vec<InstalledBuildProvider>,
    query: Option<product_queries::PendingQuery>,
    descriptions: Vec<product_queries::IssuedDescription>,
}

struct InstalledBuildProvider {
    request: BuildProviderRequest,
    declaration: String,
    package: semantic_vocabulary::PackageKeyIdentity,
}

pub type BuildProviderRequest = language_semantics::declaration_selection::ProviderReadinessRequest<
    psi_compiler::TerminalBuildProviderRequirement,
>;

impl BindingState {
    fn provider_is_installed(&self, request: &BuildProviderRequest) -> bool {
        self.installed_providers.iter().any(|installed| {
            installed.request.requirement.declaration == request.requirement.declaration
                && installed.request.requirement.application == request.requirement.application
                && installed.request.backing.path == request.backing.path
        })
    }

    fn awaiting_provider(&self) -> Result<Option<&BuildProviderRequest>, String> {
        let Some(ticket) = self.awaiting_provider else {
            return Ok(None);
        };
        match self.pending.get(ticket).and_then(Option::as_ref) {
            Some(BuildSelectionRequest::ProviderReadiness(request)) => Ok(Some(request)),
            _ => Err("Build provider pause lost its exact activation ticket".into()),
        }
    }
}

impl BindingState {
    fn take_executed(&mut self) -> Result<Vec<ExecutedBuildSelection>, String> {
        if self.pending.iter().any(Option::is_some) || self.query.is_some() {
            return Err("Build binding activation ended with an uncommitted request".to_owned());
        }
        Ok(std::mem::take(&mut self.executed))
    }
}

impl<'context> TerminalBuildBindingHost<'context> {
    pub fn new(
        artifact: terminal_codec::TerminalExecutionView<'_>,
        bindings: &'context TerminalBuildBindings,
        original: TerminalStructuralValue,
        account: Option<&'context evaluation_accounting::EvaluationInvocationAccount>,
    ) -> Result<Self, String> {
        let calls = bindings
            .calls_for(artifact)
            .ok_or_else(|| "Build bindings belong to another Terminal artifact".to_owned())?;
        let declarations = bindings.declarations_for(artifact).ok_or_else(|| {
            "Build declaration formation belongs to another Terminal artifact".to_owned()
        })?;
        let queries = bindings
            .queries_for(artifact)
            .ok_or("Build queries belong to another Terminal artifact")?;
        let product_facet = if queries.is_empty() {
            None
        } else {
            Some(crate::terminal_requests::build_facet(
                artifact, &original, "product",
            )?)
        };
        Ok(Self {
            calls,
            queries,
            product_facet,
            account,
            declarations,
            original,
            state: Mutex::new(BindingState::default()),
        })
    }

    pub(crate) fn declared(&self) -> &[std::sync::Arc<ProductDeclarationRequest>] {
        self.declarations
    }

    pub(crate) fn awaiting_provider(&self) -> Result<Option<BuildProviderRequest>, String> {
        let state = self
            .state
            .lock()
            .map_err(|_| "Build binding activation lock was poisoned")?;
        Ok(state.awaiting_provider()?.cloned())
    }

    /// Called only by the activation after every exact component slot was bound.
    pub(crate) fn has_installed_backing(
        &self,
        provider: &psi_compiler::ProducedBuildProvider,
    ) -> Result<bool, String> {
        let state = self
            .state
            .lock()
            .map_err(|_| "Build binding activation lock was poisoned")?;
        let request = state
            .awaiting_provider()?
            .ok_or("Build has no provider readiness pause")?;
        let Some(installed) = state.installed_providers.iter().find(|installed| {
            installed.request.requirement.declaration == request.requirement.declaration
                && installed.request.requirement.application == request.requirement.application
        }) else {
            return Ok(false);
        };
        if installed.declaration != provider.declaration()
            || installed.package != provider.package()
        {
            return Err("Build requirement already has a different exact provider backing".into());
        }
        Ok(true)
    }

    pub(crate) fn provider_installed(
        &self,
        provider: &psi_compiler::ProducedBuildProvider,
    ) -> Result<(), String> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| "Build binding activation lock was poisoned")?;
        let request = state
            .awaiting_provider()?
            .ok_or("Build has no provider readiness pause")?
            .clone();
        state.installed_providers.push(InstalledBuildProvider {
            request,
            declaration: provider.declaration().to_owned(),
            package: provider.package(),
        });
        state.awaiting_provider = None;
        Ok(())
    }

    pub fn adapter(
        &self,
        signature: TerminalImportSignature,
    ) -> TerminalBuildBindingAdapter<'_, 'context> {
        TerminalBuildBindingAdapter {
            host: self,
            signature,
        }
    }

    pub fn into_executed(self) -> Result<Vec<ExecutedBuildSelection>, String> {
        self.state
            .into_inner()
            .map_err(|_| "Build binding activation lock was poisoned".to_owned())?
            .take_executed()
    }

    /// Retire committed selections after the sole activation has returned.
    /// The activation owner prevents a second execution with these adapters.
    pub(crate) fn take_executed(&self) -> Result<Vec<ExecutedBuildSelection>, String> {
        self.state
            .lock()
            .map_err(|_| "Build binding activation lock was poisoned".to_owned())?
            .take_executed()
    }
}

pub struct TerminalBuildBindingAdapter<'host, 'context> {
    host: &'host TerminalBuildBindingHost<'context>,
    signature: TerminalImportSignature,
}

impl TerminalImportAdapter for TerminalBuildBindingAdapter<'_, '_> {
    fn implemented_signature(&self) -> TerminalImportSignature {
        self.signature.clone()
    }
}

impl TerminalEffectHandler for TerminalBuildBindingAdapter<'_, '_> {
    fn poll_host_computation(
        &mut self,
        effect: &TerminalEffect,
    ) -> Result<bool, TerminalEffectRejection> {
        let TerminalEffect::BoundaryCall {
            operation,
            boundary,
            ..
        } = effect
        else {
            return Err(TerminalEffectRejection::new("not a Build binding effect"));
        };
        match self
            .host
            .queries
            .iter()
            .find(|query| query.operation == *operation && query.boundary == *boundary)
        {
            Some(query) => self.host.poll_query(query, effect),
            None => Ok(true),
        }
    }

    fn poll_effect_readiness(
        &mut self,
        effect: &TerminalEffect,
    ) -> Result<bool, TerminalEffectRejection> {
        let refuse = TerminalEffectRejection::new;
        let TerminalEffect::BoundaryCall {
            operation,
            boundary,
            arguments,
            structural_arguments,
            ..
        } = effect
        else {
            return Err(refuse("not a Build binding effect"));
        };
        if self
            .host
            .queries
            .iter()
            .any(|query| query.operation == *operation && query.boundary == *boundary)
        {
            return Ok(true);
        }
        let call = self
            .host
            .calls
            .iter()
            .find(|call| call.operation == *operation && call.boundary == *boundary)
            .ok_or_else(|| refuse("Build binding call has no source-issued designated operands"))?;
        if call.action != TerminalBuildBindingOperation::AwaitProvider {
            return Ok(true);
        }
        let BuildSelectionRequest::ProviderReadiness(request) = &call.request else {
            return Err(refuse(
                "Build provider readiness disagrees with its source-issued request",
            ));
        };
        let mut state = self
            .host
            .state
            .lock()
            .map_err(|_| refuse("Build binding activation lock was poisoned"))?;
        let (ticket, values) =
            activation_ticket(arguments, structural_arguments, &state, &call.request)?;
        if !values.is_empty() {
            return Err(refuse(
                "Build provider readiness has unexpected value operands",
            ));
        }
        if state.provider_is_installed(request) {
            return Ok(true);
        }
        if state
            .awaiting_provider
            .is_some_and(|pending| pending != ticket)
        {
            return Err(refuse(
                "Build activation already awaits another provider request",
            ));
        }
        state.awaiting_provider = Some(ticket);
        Ok(false)
    }

    fn handle_effect(&mut self, _: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
        Err(TerminalEffectRejection::new(
            "Build bindings require result-aware dispatch",
        ))
    }

    fn handle_effect_result(
        &mut self,
        effect: &TerminalEffect,
    ) -> Result<TerminalEffectResult, TerminalEffectRejection> {
        use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue};
        let refuse = TerminalEffectRejection::new;
        let TerminalEffect::BoundaryCall {
            operation,
            boundary,
            arguments,
            structural_arguments,
            ..
        } = effect
        else {
            return Err(refuse("not a Build binding effect"));
        };
        if let Some(query) = self
            .host
            .queries
            .iter()
            .find(|query| query.operation == *operation && query.boundary == *boundary)
        {
            return self.host.complete_query(query, effect);
        }
        let call = self
            .host
            .calls
            .iter()
            .find(|call| call.operation == *operation && call.boundary == *boundary)
            .ok_or_else(|| refuse("Build binding call has no source-issued designated operands"))?;
        let mut state = self
            .host
            .state
            .lock()
            .map_err(|_| refuse("Build binding activation lock was poisoned"))?;
        if call.action == TerminalBuildBindingOperation::Prepare {
            if !arguments.is_empty()
                || structural_arguments.as_slice() != std::slice::from_ref(&self.host.original)
            {
                return Err(refuse(
                    "root binding requires the original compiler-issued Build receiver",
                ));
            }
            let ticket = u64::try_from(state.pending.len())
                .map_err(|_| refuse("Build binding ticket storage is exhausted"))?;
            state
                .pending
                .try_reserve(1)
                .map_err(|_| refuse("cannot retain Build binding request"))?;
            state.pending.push(Some(call.request.clone()));
            return Ok(TerminalEffectResult::Scalar(TerminalScalarValue::Integer {
                scalar_type: IntegerType::new(IntegerSign::Unsigned, 64).map_err(|error| {
                    TerminalEffectRejection::new(format!(
                        "invalid Build ticket scalar type: {error:?}"
                    ))
                })?,
                value: IntegerValue::Unsigned(u128::from(ticket)),
            }));
        }

        // Every commit consumes one exact preparation. Decoding the selected
        // value differs; authentication, retirement and publication do not.
        let description = match &call.request {
            BuildSelectionRequest::Entry(binding)
                if binding.implementation == RootBindingOperand::Description =>
            {
                let [value] = structural_arguments.as_slice() else {
                    return Err(refuse("delegated root binding requires one description"));
                };
                Some(state.entry_description(value)?)
            }
            _ => None,
        };
        let structural_operands = if description.is_some() {
            &[][..]
        } else {
            structural_arguments.as_slice()
        };
        let (ticket, values) =
            activation_ticket(arguments, structural_operands, &state, &call.request)?;
        if let (
            BuildSelectionRequest::ProviderReadiness(request),
            TerminalBuildBindingOperation::AwaitProvider,
            [],
        ) = (&call.request, call.action, values)
        {
            if !state.provider_is_installed(request) {
                return Err(refuse(
                    "Build provider readiness requires completed component installation",
                ));
            }
            state.pending[ticket] = None;
            return Ok(TerminalEffectResult::Unit);
        }
        let selection = match (&call.request, call.action, values) {
            (BuildSelectionRequest::Entry(binding), TerminalBuildBindingOperation::Bind, []) => {
                let implementation = match (&binding.implementation, description) {
                    (RootBindingOperand::AuthoredPath(path), None) => {
                        RootBindingSelection::AuthoredPath(path.clone())
                    }
                    (RootBindingOperand::Description, Some(description)) => {
                        RootBindingSelection::Described(description)
                    }
                    _ => {
                        return Err(refuse(
                            "root binding operand is not a compiler-issued product entry description",
                        ));
                    }
                };
                ExecutedBuildSelection::Entry(ExecutedRootBinding {
                    source_span: binding.source_span,
                    slot: binding.slot.clone(),
                    implementation,
                })
            }
            (BuildSelectionRequest::Declaration(request), action, values) => {
                let independent = match (action, values) {
                    (TerminalBuildBindingOperation::Select, []) => None,
                    (
                        TerminalBuildBindingOperation::SelectMode,
                        [TerminalScalarValue::Boolean(value)],
                    ) => Some(*value),
                    _ => {
                        return Err(refuse(
                            "Product selection disagrees with its evaluated operands",
                        ));
                    }
                };
                let composition = match request.operation {
                    BuildOperation::ProviderSelection => Some(if independent.unwrap_or(false) {
                        CompositionMode::Independent
                    } else {
                        CompositionMode::Fused
                    }),
                    operation
                        if operation.selects_product_declarations()
                            && operation != BuildOperation::ProviderSelection
                            && independent.is_none() =>
                    {
                        None
                    }
                    _ => {
                        return Err(refuse(
                            "Product selection operation disagrees with its runtime operands",
                        ));
                    }
                };
                if call.request.requires_lexical_subject() && call.selecting_machine.is_none() {
                    return Err(refuse(
                        "behavior exclusion lost its authored machine coordinate",
                    ));
                }
                ExecutedBuildSelection::Declaration(ExecutedProductSelection {
                    request: request.clone(),
                    selecting_machine: call.selecting_machine,
                    composition,
                })
            }
            (
                BuildSelectionRequest::Exclusion(request),
                TerminalBuildBindingOperation::Exclude,
                [
                    TerminalScalarValue::Integer {
                        value: IntegerValue::Unsigned(code),
                        ..
                    },
                ],
            ) => {
                let code = u64::try_from(*code)
                    .map_err(|_| refuse("behavior exclusion code is outside its vocabulary"))?;
                if code >= request.vocabulary.cases().len() as u64 {
                    return Err(refuse("behavior exclusion code is outside its vocabulary"));
                }
                ExecutedBuildSelection::Exclusion(
                    language_semantics::declaration_selection::ExecutedEnumExclusion {
                        request: request.clone(),
                        code,
                        selecting_machine: call.selecting_machine.ok_or_else(|| {
                            refuse("behavior exclusion lost its authored machine coordinate")
                        })?,
                    },
                )
            }
            _ => {
                return Err(refuse(
                    "Build selection commit disagrees with its source-issued request",
                ));
            }
        };
        state
            .executed
            .try_reserve(1)
            .map_err(|_| refuse("cannot retain executed Build selection"))?;
        state.pending[ticket] = None;
        state.executed.push(selection);
        Ok(TerminalEffectResult::Unit)
    }
}

fn activation_ticket<'values>(
    arguments: &'values [TerminalScalarValue],
    structural_arguments: &[TerminalStructuralValue],
    state: &BindingState,
    request: &psi_compiler::TerminalBuildSelectionRequest,
) -> Result<(usize, &'values [TerminalScalarValue]), TerminalEffectRejection> {
    let refuse = TerminalEffectRejection::new;
    let Some((
        TerminalScalarValue::Integer {
            value: semantic_vocabulary::IntegerValue::Unsigned(ticket),
            ..
        },
        values,
    )) = arguments.split_first()
    else {
        return Err(refuse("Build selection has no activation ticket"));
    };
    if !structural_arguments.is_empty() {
        return Err(refuse("Build selection has unexpected structural operands"));
    }
    let ticket = usize::try_from(*ticket)
        .map_err(|_| refuse("Build selection ticket is outside its activation"))?;
    if state.pending.get(ticket).and_then(Option::as_ref) != Some(request) {
        return Err(refuse(
            "Build selection ticket was consumed or belongs to another request",
        ));
    }
    Ok((ticket, values))
}
