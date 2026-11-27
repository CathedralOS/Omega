/*
    Host embedding lifecycle for Terminal Psi: immutable `TerminalProgram`
    loading, `TerminalInstance` instantiation, import adapter binding, and
    post-return borrowed storage and loan observation.

    We share decoded or borrowed canonical modules across independent guest
    instances, verify import adapters and runtime supplies before execution,
    and expose read-only post-return observations without leaking internal
    storage authority.
*/

pub(crate) mod adapters {
    use crate::boundary::effects::results::TerminalEffectResult;
    use crate::boundary::effects::{
        TerminalEffect, TerminalEffectHandler, TerminalEffectRejection,
    };
    use crate::boundary::values::TerminalScalarValue;
    use semantic_vocabulary::{ScalarType, StructuralDomainId, StructuralTypeId};
    use terminal_psi::{
        BoundaryCompletion, BoundaryMachineDeclaration, BoundaryMachineResult,
        ExternalCompletionKind, StructuralAccess, StructuralMultiplicity,
        StructuralPathQualification,
    };

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct TerminalImportSignature {
        pub completion: BoundaryCompletion,
        pub scalar_parameters: Vec<ScalarType>,
        pub structural_parameters: Vec<TerminalImportStructuralParameter>,
        pub result: BoundaryMachineResult,
        pub result_reference_regions: Vec<terminal_psi::TerminalDynamicReferenceRegion>,
        pub result_lifetimes: Option<Vec<terminal_psi::DeclaredResultLifetime>>,
        pub normal_qualifications: Vec<terminal_psi::StructuralDomainRequirement>,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct TerminalImportStructuralParameter {
        pub structural_type: StructuralTypeId,
        pub multiplicity: StructuralMultiplicity,
        pub access: StructuralAccess,
        pub qualifications: Vec<StructuralDomainId>,
        pub projected_qualifications: Vec<StructuralPathQualification>,
    }

    impl From<&BoundaryMachineDeclaration> for TerminalImportSignature {
        fn from(declaration: &BoundaryMachineDeclaration) -> Self {
            Self {
                completion: declaration.completion,
                scalar_parameters: declaration.scalar_parameters.clone(),
                structural_parameters: declaration
                    .structural_parameters
                    .iter()
                    .map(|parameter| TerminalImportStructuralParameter {
                        structural_type: parameter.structural_type,
                        multiplicity: parameter.multiplicity,
                        access: parameter.access,
                        qualifications: parameter.qualifications.clone(),
                        projected_qualifications: parameter.projected_qualifications.clone(),
                    })
                    .collect(),
                result: declaration.result.clone(),
                result_reference_regions: declaration.result_reference_regions.clone(),
                result_lifetimes: declaration.result_lifetimes.clone(),
                normal_qualifications: declaration.normal_qualifications.clone(),
            }
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum TerminalAdapterError {
        ExternalCompletionUnsupported { kind: ExternalCompletionKind },
        StructuralParametersUnsupported,
        ResultKindUnsupported,
    }

    impl std::fmt::Display for TerminalAdapterError {
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                Self::ExternalCompletionUnsupported { kind } => write!(
                    formatter,
                    "external completion {kind:?} is outside this normal-return adapter's contract"
                ),
                Self::StructuralParametersUnsupported => write!(
                    formatter,
                    "import declares structural parameters outside this adapter's contract"
                ),
                Self::ResultKindUnsupported => write!(
                    formatter,
                    "import result kind is outside this adapter's contract"
                ),
            }
        }
    }

    impl std::error::Error for TerminalAdapterError {}

    pub trait TerminalImportAdapter: TerminalEffectHandler {
        fn implemented_signature(&self) -> TerminalImportSignature;
    }

    pub struct TerminalScalarImport<F> {
        signature: TerminalImportSignature,
        result_scalar_type: ScalarType,
        function: F,
    }

    impl<F> TerminalScalarImport<F>
    where
        F: FnMut(&[TerminalScalarValue]) -> Result<TerminalScalarValue, TerminalEffectRejection>,
    {
        pub fn for_import(
            import: &BoundaryMachineDeclaration,
            function: F,
        ) -> Result<Self, TerminalAdapterError> {
            let signature = TerminalImportSignature::from(import);
            require_normal_completion(&signature)?;
            if !signature.structural_parameters.is_empty() {
                return Err(TerminalAdapterError::StructuralParametersUnsupported);
            }
            let result_scalar_type = signature
                .result
                .scalar()
                .ok_or(TerminalAdapterError::ResultKindUnsupported)?;
            Ok(Self {
                signature,
                result_scalar_type,
                function,
            })
        }
    }

    impl<F> TerminalImportAdapter for TerminalScalarImport<F>
    where
        F: FnMut(&[TerminalScalarValue]) -> Result<TerminalScalarValue, TerminalEffectRejection>,
    {
        fn implemented_signature(&self) -> TerminalImportSignature {
            self.signature.clone()
        }
    }

    impl<F> TerminalEffectHandler for TerminalScalarImport<F>
    where
        F: FnMut(&[TerminalScalarValue]) -> Result<TerminalScalarValue, TerminalEffectRejection>,
    {
        fn handle_effect(
            &mut self,
            _effect: &TerminalEffect,
        ) -> Result<(), TerminalEffectRejection> {
            Err(TerminalEffectRejection::new(
                "scalar-result import requires the result-aware dispatch",
            ))
        }

        fn handle_effect_result(
            &mut self,
            effect: &TerminalEffect,
        ) -> Result<TerminalEffectResult, TerminalEffectRejection> {
            let TerminalEffect::BoundaryCall { arguments, .. } = effect else {
                return Err(TerminalEffectRejection::new(
                    "scalar import received a non-boundary effect",
                ));
            };
            check_scalar_arguments(&self.signature, arguments)?;
            let value = (self.function)(arguments)?;
            if value.scalar_type() != self.result_scalar_type {
                return Err(TerminalEffectRejection::new(
                    "host result scalar does not match the bound signature",
                ));
            }
            Ok(TerminalEffectResult::Scalar(value))
        }
    }

    pub struct TerminalUnitImport<F> {
        signature: TerminalImportSignature,
        function: F,
    }

    impl<F> TerminalUnitImport<F>
    where
        F: FnMut(&[TerminalScalarValue]) -> Result<(), TerminalEffectRejection>,
    {
        pub fn for_import(
            import: &BoundaryMachineDeclaration,
            function: F,
        ) -> Result<Self, TerminalAdapterError> {
            let signature = TerminalImportSignature::from(import);
            require_normal_completion(&signature)?;
            if !signature.structural_parameters.is_empty() {
                return Err(TerminalAdapterError::StructuralParametersUnsupported);
            }
            if !signature.result.is_unit() {
                return Err(TerminalAdapterError::ResultKindUnsupported);
            }
            Ok(Self {
                signature,
                function,
            })
        }
    }

    impl<F> TerminalImportAdapter for TerminalUnitImport<F>
    where
        F: FnMut(&[TerminalScalarValue]) -> Result<(), TerminalEffectRejection>,
    {
        fn implemented_signature(&self) -> TerminalImportSignature {
            self.signature.clone()
        }
    }

    impl<F> TerminalEffectHandler for TerminalUnitImport<F>
    where
        F: FnMut(&[TerminalScalarValue]) -> Result<(), TerminalEffectRejection>,
    {
        fn handle_effect(
            &mut self,
            effect: &TerminalEffect,
        ) -> Result<(), TerminalEffectRejection> {
            let TerminalEffect::BoundaryCall { arguments, .. } = effect else {
                return Err(TerminalEffectRejection::new(
                    "unit import received a non-boundary effect",
                ));
            };
            check_scalar_arguments(&self.signature, arguments)?;
            (self.function)(arguments)
        }
    }

    fn require_normal_completion(
        signature: &TerminalImportSignature,
    ) -> Result<(), TerminalAdapterError> {
        match signature.completion {
            BoundaryCompletion::NormalReturn => Ok(()),
            BoundaryCompletion::External(kind) => {
                Err(TerminalAdapterError::ExternalCompletionUnsupported { kind })
            }
        }
    }

    fn check_scalar_arguments(
        signature: &TerminalImportSignature,
        arguments: &[TerminalScalarValue],
    ) -> Result<(), TerminalEffectRejection> {
        if arguments.len() == signature.scalar_parameters.len()
            && arguments
                .iter()
                .zip(&signature.scalar_parameters)
                .all(|(argument, expected)| argument.scalar_type() == *expected)
        {
            Ok(())
        } else {
            Err(TerminalEffectRejection::new(
                "boundary call arguments do not match the bound signature",
            ))
        }
    }
}
pub(crate) mod instance {
    pub(crate) mod component_binding {
        use build_model::accounting::EvaluationInvocationAccount;
        use std::collections::BTreeMap;

        use semantic_vocabulary::{BoundaryMachineId, OperationId};
        use psi_provider_selections::AdmittedComponentProvider;
        use psi_scalar_applications::AdmittedScalarApplications;
        use terminal_work::ExecutionMeter;

        use super::{ComponentOwnedReturn, TerminalImportBinding, TerminalInstance};
        use crate::boundary::results::TerminalExecutionStatus;
        use crate::embedding::program::TerminalProgram;
        use crate::engine::calls::{ComponentCallScope, ComponentInputCustody};
        use crate::{
            TerminalEffectRejection, TerminalImportAdapter, TerminalRuntimeSupplies,
            TerminalScalarValue, TerminalStructuralInputs,
        };
        use terminal_psi::BoundaryMachineDeclaration;

        pub struct TerminalComponentBinding<'host, 'artifact> {
            selected: AdmittedComponentProvider,
            program: TerminalProgram<'artifact>,
            supplies: TerminalRuntimeSupplies,
            scalar_admission: Option<std::borrow::Cow<'artifact, AdmittedScalarApplications>>,
            required_imports: Vec<BoundaryMachineId>,
            bindings: BTreeMap<BoundaryMachineId, TerminalImportBinding<'host>>,
            storage_resolver:
                Option<std::rc::Rc<dyn crate::TerminalProviderStorageResolver + 'host>>,
            components: ComponentBindings<'host, 'artifact>,
            pending: Option<PendingComponent<'host, 'artifact>>,
        }

        struct PendingComponent<'host, 'artifact> {
            caller: ComponentCallScope,
            operation: OperationId,
            arguments: Vec<TerminalScalarValue>,
            structural_operands: Vec<terminal_psi::StructuralArgument>,
            inputs: ComponentInputCustody,
            instance: Option<Box<TerminalInstance<'host, 'artifact>>>,
            completed: Option<ComponentOwnedReturn<'host, 'artifact>>,
        }

        pub(crate) type ComponentBindings<'host, 'artifact> =
            BTreeMap<BoundaryMachineId, TerminalComponentBinding<'host, 'artifact>>;

        impl<'host, 'artifact> TerminalComponentBinding<'host, 'artifact> {
            pub fn new(
                selected: AdmittedComponentProvider,
                program: TerminalProgram<'artifact>,
                supplies: TerminalRuntimeSupplies,
                scalar_admission: Option<std::borrow::Cow<'artifact, AdmittedScalarApplications>>,
            ) -> Result<Self, String> {
                let provider_identity =
                    terminal_codec::TerminalExecutionIdentity::portable(selected.provider());
                if program.execution_identity() != &provider_identity
                    || supplies.code_identity != provider_identity
                    || program
                        .program
                        .resolve_entry(Some(&supplies))
                        .map_err(|error| format!("component entry admission: {error:?}"))?
                        != selected.candidate()
                {
                    return Err("component binding names another artifact or callable entry".into());
                }
                let requirements = program
                    .execution_requirements(Some(&supplies))
                    .map_err(|error| format!("component closure admission: {error:?}"))?;
                if let Some(admission) = scalar_admission.as_deref() {
                    program
                        .admitted_scalar_inputs(admission, Some(&supplies))
                        .map_err(|error| format!("component scalar admission: {error:?}"))?;
                }
                if !requirements.missing_providers.is_empty()
                    || !requirements.external_completions.is_empty()
                {
                    return Err(
                "component closure needs provider or per-instance external-completion authority"
                    .into(),
            );
                }
                Ok(Self {
                    selected,
                    program,
                    supplies,
                    scalar_admission,
                    required_imports: requirements.imports,
                    bindings: BTreeMap::new(),
                    storage_resolver: None,
                    components: BTreeMap::new(),
                    pending: None,
                })
            }

            pub fn unbound_imports(&self) -> impl Iterator<Item = BoundaryMachineId> + '_ {
                self.required_imports.iter().copied().filter(|boundary| {
                    !self.bindings.contains_key(boundary) && !self.components.contains_key(boundary)
                })
            }

            pub fn bind_storage_resolver(
                &mut self,
                resolver: std::rc::Rc<dyn crate::TerminalProviderStorageResolver + 'host>,
            ) -> Result<(), String> {
                if self.pending.is_some() || self.storage_resolver.is_some() {
                    return Err(
                        "component provider storage resolver is active or already bound".into(),
                    );
                }
                self.storage_resolver = Some(resolver);
                Ok(())
            }

            pub fn import(
                &self,
                boundary: BoundaryMachineId,
            ) -> Option<&BoundaryMachineDeclaration> {
                if !self.required_imports.contains(&boundary) {
                    return None;
                }
                self.program.program.boundary_machines().get(&boundary)
            }

            pub fn bind_import(
                &mut self,
                boundary: BoundaryMachineId,
                adapter: impl TerminalImportAdapter + 'host,
            ) -> Result<(), String> {
                let declaration = self
                    .import(boundary)
                    .ok_or("component slot is not demanded by this entry")?;
                if self.pending.is_some()
                    || self.bindings.contains_key(&boundary)
                    || self.components.contains_key(&boundary)
                {
                    return Err("component import is active or already bound".into());
                }
                if !matches!(
                    self.supplies.supplies.get(&boundary),
                    None | Some(crate::boundary::effects::AdmittedBoundarySupply::HostImport)
                ) {
                    return Err("a host adapter cannot replace a checked component provider".into());
                }
                let signature = crate::TerminalImportSignature::from(declaration);
                if adapter.implemented_signature() != signature {
                    return Err(
                        "component adapter differs from the exact demanded signature".into(),
                    );
                }
                self.bindings.insert(
                    boundary,
                    TerminalImportBinding {
                        adapter: Box::new(adapter),
                    },
                );
                Ok(())
            }

            pub fn bind_component(
                &mut self,
                binding: TerminalComponentBinding<'host, 'artifact>,
            ) -> Result<(), String> {
                let boundary = binding.selected.receiving_boundary();
                if self.pending.is_some()
                    || !self.required_imports.contains(&boundary)
                    || self.bindings.contains_key(&boundary)
                    || self.components.contains_key(&boundary)
                    || binding.selected.receiver() != self.program.execution_identity()
                    || self.supplies.supplies.get(&boundary) != Some(&binding.supply())
                {
                    return Err(
                        "nested component differs from the exact unused receiving slot".into(),
                    );
                }
                binding.require_ready()?;
                self.components.insert(boundary, binding);
                Ok(())
            }

            pub(crate) fn require_ready(&self) -> Result<(), String> {
                let missing = self.unbound_imports().collect::<Vec<_>>();
                if self.pending.is_some() || !missing.is_empty() {
                    return Err(format!(
                        "component closure is active or has unbound imports: {missing:?}"
                    ));
                }
                for component in self.components.values() {
                    component.require_ready()?;
                }
                Ok(())
            }

            pub(crate) fn supply(&self) -> crate::boundary::effects::AdmittedBoundarySupply {
                crate::boundary::effects::AdmittedBoundarySupply::ComponentProvider {
                    provider: self.selected.provider(),
                    provider_boundary: self.selected.provider_boundary(),
                    candidate: self.selected.candidate(),
                }
            }
            pub(crate) fn selected(&self) -> &AdmittedComponentProvider {
                &self.selected
            }
            pub(crate) fn provider_program(&self) -> &TerminalProgram<'artifact> {
                &self.program
            }
            pub(crate) const fn has_pending_invocation(&self) -> bool {
                self.pending.is_some()
            }

            pub(crate) fn pending_at(&self, operation: OperationId) -> bool {
                self.pending
                    .as_ref()
                    .is_some_and(|pending| pending.operation == operation)
            }

            pub(crate) fn prepare(
                &mut self,
                caller: ComponentCallScope,
                operation: OperationId,
                arguments: &[TerminalScalarValue],
                structural_operands: &[terminal_psi::StructuralArgument],
                inputs: ComponentInputCustody,
                account: Option<&EvaluationInvocationAccount>,
            ) -> Result<(), String> {
                self.require_ready()?;
                if caller.namespace.is_none()
                    && structural_operands
                        .iter()
                        .any(|argument| argument.access != terminal_psi::StructuralAccess::Owned)
                {
                    return Err("component lending requires its live caller activation".into());
                }
                let mut instance = self
                    .program
                    .instantiate(crate::TerminalInstantiation {
                        float_evaluation: caller.float_evaluation,
                        scalar_arguments: arguments,
                        structural_inputs: TerminalStructuralInputs {
                            arguments: &inputs.values,
                            contents: &inputs.contents,
                            ..TerminalStructuralInputs::default()
                        },
                        installation: Some(&self.supplies),
                        account,
                        entry_results: None,
                        scalar_application: None,
                        scalar_admission: self.scalar_admission.as_deref(),
                    })
                    .map_err(|error| format!("component instance admission: {error:?}"))?;
                if instance.required_imports != self.required_imports {
                    return Err(
                        "component invocation changed its preflighted import closure".into(),
                    );
                }
                inputs
                    .establish_lent_inputs(
                        instance
                            .execution
                            .as_mut()
                            .ok_or("new component lost its execution")?,
                    )
                    .map_err(|error| format!("component lent input custody: {error:?}"))?;
                instance.bindings = std::mem::take(&mut self.bindings);
                instance.storage_resolver = self.storage_resolver.clone();
                instance.components = std::mem::take(&mut self.components);
                self.pending = Some(PendingComponent {
                    caller,
                    operation,
                    arguments: arguments.to_vec(),
                    structural_operands: structural_operands.to_vec(),
                    inputs,
                    instance: Some(Box::new(instance)),
                    completed: None,
                });
                Ok(())
            }

            pub(crate) fn invoke(
                &mut self,
                caller: ComponentCallScope,
                operation: OperationId,
                arguments: &[TerminalScalarValue],
                structural_operands: &[terminal_psi::StructuralArgument],
                meter: &mut dyn ExecutionMeter,
            ) -> Result<Option<TerminalExecutionStatus>, TerminalEffectRejection> {
                let pending = self.pending.as_mut().ok_or_else(|| {
                    TerminalEffectRejection::new("component invocation was not prepared")
                })?;
                if pending.caller != caller
                    || pending.operation != operation
                    || pending.arguments != arguments
                    || pending.structural_operands != structural_operands
                    || pending.inputs.values.len() != structural_operands.len()
                    || pending
                        .inputs
                        .source
                        .as_ref()
                        .is_some_and(|source| source.values.len() != structural_operands.len())
                {
                    return Err(TerminalEffectRejection::new(
                        "pending component invocation has another operation or input snapshot",
                    ));
                }
                if pending.completed.is_some() {
                    return Ok(None);
                }
                let instance = pending.instance.as_mut().ok_or_else(|| {
                    TerminalEffectRejection::new("component invocation lost its live instance")
                })?;
                match instance
                    .invoke(meter)
                    .map_err(|error| match error.sponsor_exhaustion() {
                        Some(reason) => TerminalEffectRejection::sponsor_exhausted(format!(
                            "component invocation: {reason}"
                        )),
                        None => {
                            TerminalEffectRejection::new(format!("component invocation: {error:?}"))
                        }
                    })? {
                    TerminalExecutionStatus::Complete(_) => {
                        let instance = *pending.instance.take().expect("borrowed above");
                        match instance.into_component_return(&self.selected) {
                            Ok(completed) => pending.completed = Some(completed),
                            Err((instance, error)) => {
                                pending.instance = Some(Box::new(instance));
                                return Err(TerminalEffectRejection::new(format!(
                                    "component return custody: {error:?}"
                                )));
                            }
                        }
                        Ok(None)
                    }
                    status => Ok(Some(status)),
                }
            }

            pub(crate) fn completed(&self) -> Option<&ComponentOwnedReturn<'host, 'artifact>> {
                self.pending.as_ref()?.completed.as_ref()
            }

            pub(crate) fn acknowledge(&mut self) -> Vec<crate::TerminalEffect> {
                let pending = self
                    .pending
                    .take()
                    .expect("completed invocation was authenticated");
                let completed = pending
                    .completed
                    .expect("completed return was authenticated");
                let (value, mut owner) = completed.acknowledge();
                drop(value);
                let effects = owner
                    .execution
                    .as_mut()
                    .map_or_else(Vec::new, |execution| std::mem::take(&mut execution.effects));
                self.bindings = std::mem::take(&mut owner.bindings);
                self.components = std::mem::take(&mut owner.components);
                effects
            }
        }
    }
    pub(crate) use component_binding::ComponentBindings;
    pub use component_binding::TerminalComponentBinding;
    pub(crate) mod component_return {
        use semantic_vocabulary::MachineId;
        use terminal_psi::TerminalPsiIdentity;

        use super::{TerminalInstance, TerminalInstanceError};
        use crate::boundary::errors::TerminalInterpretError;
        use crate::boundary::results::TerminalExecutionResult;

        pub(crate) struct ComponentOwnedReturn<'host, 'artifact> {
            provider: TerminalPsiIdentity,
            candidate: MachineId,
            instance_namespace: u64,
            value: TerminalExecutionResult,
            owner: TerminalInstance<'host, 'artifact>,
        }

        impl<'host, 'artifact> ComponentOwnedReturn<'host, 'artifact> {
            pub(crate) const fn provider(&self) -> TerminalPsiIdentity {
                self.provider
            }
            pub(crate) const fn candidate(&self) -> MachineId {
                self.candidate
            }
            pub(crate) const fn instance_namespace(&self) -> u64 {
                self.instance_namespace
            }
            pub(crate) const fn value(&self) -> &TerminalExecutionResult {
                &self.value
            }

            pub(crate) fn returned_claims(
                &self,
            ) -> Option<&[terminal_psi::StructuralResultClaimBinding]> {
                self.owner
                    .execution
                    .as_ref()
                    .map(|execution| execution.returned_claims.as_slice())
            }

            pub(crate) fn acknowledge(
                self,
            ) -> (TerminalExecutionResult, TerminalInstance<'host, 'artifact>) {
                let Self { value, owner, .. } = self;
                (value, owner)
            }
        }

        impl<'host, 'artifact> TerminalInstance<'host, 'artifact> {
            pub(crate) fn into_component_return(
                mut self,
                selected: &psi_provider_selections::AdmittedComponentProvider,
            ) -> Result<ComponentOwnedReturn<'host, 'artifact>, (Self, TerminalInstanceError)>
            {
                let verified = (|| {
                    if self.closed
                        || !self.invoked
                        || self.program.terminal_psi() != selected.provider()
                    {
                        return Err(
                            "component return belongs to another or inactive producing instance",
                        );
                    }
                    let execution = self
                        .execution
                        .as_ref()
                        .ok_or("component return lost its producing execution")?;
                    if self.entry != selected.candidate()
                        || !execution.call_stack.is_empty()
                        || !execution.live_claims.is_empty()
                        || !execution.live_affine_frontier.is_empty()
                    {
                        return Err("component return has unfinished activation or custody");
                    }
                    let value = execution
                        .result
                        .as_ref()
                        .ok_or("component entry has not returned normally")?;
                    match value {
                        TerminalExecutionResult::Unit | TerminalExecutionResult::Scalar(_) => {}
                        TerminalExecutionResult::Structural(returned) => {
                            let declaration = execution
                                .structural_types()
                                .get(&returned.value.structural_type)
                                .ok_or("component return lost its structural declaration")?;
                            if returned
                                .value
                                .qualifications
                                .iter()
                                .any(|domain| selected.result_membership(*domain).is_none())
                                || !returned.value.projected_qualifications.is_empty()
                                || !returned.value.path.is_empty()
                                || (!declaration.owns_linear_claim
                                    && !declaration.no_code_affine_cleanup)
                            {
                                return Err(
                                    "component return requires a checked claim, membership or cleanup transfer carrier",
                                );
                            }
                            let claims = &execution.returned_claims;
                            if declaration.owns_linear_claim {
                                if claims.len() != 1
                                    || !claims[0].path.is_empty()
                                    || returned.claims != [claims[0].claim]
                                {
                                    return Err(
                                        "linear component return lacks its exact whole owned claim",
                                    );
                                }
                            } else if !claims.is_empty() || !returned.claims.is_empty() {
                                return Err("nonlinear component return carries unexpected claims");
                            }
                            if !execution.structural_host_atoms.values().any(|payload| {
                                matches!(payload,
                        crate::storage::structural_storage::HostStructuralPayload::Opaque(identity)
                        if *identity == returned.value.opaque_identity)
                            }) {
                                return Err(
                                    "component return requires materialized structural storage transfer",
                                );
                            }
                        }
                        TerminalExecutionResult::ScalarCase(_)
                        | TerminalExecutionResult::ScalarArray(_) => {
                            return Err(
                                "component return requires a selected case or array storage transfer carrier",
                            );
                        }
                    }
                    execution
                        .local_structural_identities
                        .exposure_namespace
                        .ok_or("component return has no live instance identity namespace")
                })();
                let namespace = match verified {
                    Ok(namespace) => namespace,
                    Err(reason) => {
                        return Err((
                            self,
                            TerminalInstanceError::Execution(
                                TerminalInterpretError::UnsupportedSemanticVariant(reason),
                            ),
                        ));
                    }
                };
                let value = self
                    .execution
                    .as_mut()
                    .and_then(|execution| execution.result.take())
                    .expect("the completed instance was authenticated without mutation");
                self.closed = true;
                Ok(ComponentOwnedReturn {
                    provider: selected.provider(),
                    candidate: selected.candidate(),
                    instance_namespace: namespace,
                    value,
                    owner: self,
                })
            }
        }
    }
    pub(crate) use component_return::ComponentOwnedReturn;
    pub(crate) mod storage_observation;
    pub use storage_observation::{
        TerminalObservationRoot, TerminalStorageObservationError, TerminalStorageObservationValue,
    };
    pub(crate) mod returned_loans {
        use crate::boundary::values::TerminalStructuralValue;
        use crate::engine::execution::TerminalExecution;
        use semantic_vocabulary::{
            CanonicalStructuralPathSegment as Path, PlaceId, StructuralTypeId,
        };
        use terminal_psi::StructuralAccess;

        #[derive(Debug, Clone, PartialEq, Eq)]
        pub enum TerminalReturnedLoanError {
            Closed,
            AdmissionAfterStart,
            AdmissionAlreadyProvided,
            NoReferenceResult,
            AdmissionRequired,
            NormalReturnRequired,
            UnknownPath,
            UnreadableAccess,
            Execution(crate::TerminalInterpretError),
        }

        impl From<crate::TerminalInterpretError> for TerminalReturnedLoanError {
            fn from(error: crate::TerminalInterpretError) -> Self {
                Self::Execution(error)
            }
        }

        pub struct TerminalReturnedLoan<'instance, 'artifact> {
            execution: &'instance TerminalExecution<'artifact>,
            carrier: &'instance TerminalStructuralValue,
            source: PlaceId,
            path: Vec<Path>,
            referent: StructuralTypeId,
            access: StructuralAccess,
        }

        impl TerminalReturnedLoan<'_, '_> {
            pub fn access(&self) -> StructuralAccess {
                self.access
            }
            pub fn structural_type(&self) -> StructuralTypeId {
                self.referent
            }

            pub fn read_scalar(
                &self,
                path: &[Path],
            ) -> Result<crate::TerminalScalarValue, TerminalReturnedLoanError> {
                if !matches!(
                    self.access,
                    StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow
                ) {
                    return Err(TerminalReturnedLoanError::UnreadableAccess);
                }
                let mut complete = self.path.clone();
                complete.push(Path::Referent);
                complete.extend_from_slice(path);
                let types = self.execution.structural_types();
                let storage_path = terminal_semantics::canonical_structural_storage_path(
                    types.values(),
                    self.carrier.structural_type,
                    &complete,
                )
                .ok_or(TerminalReturnedLoanError::UnknownPath)?;
                let expected = terminal_semantics::primitive_read_projection_type(
                    types.values(),
                    self.carrier.structural_type,
                    &storage_path,
                )
                .ok_or(TerminalReturnedLoanError::UnknownPath)?;
                Ok(self.execution.read_retained_reference_scalar(
                    self.source,
                    self.carrier,
                    &storage_path,
                    expected,
                )?)
            }
        }
    }
    pub use returned_loans::{TerminalReturnedLoan, TerminalReturnedLoanError};

    use std::collections::BTreeMap;

    use semantic_vocabulary::BoundaryMachineId;
    use terminal_work::ExecutionMeter;

    use crate::boundary::effects::byte_buffers::TerminalBoundaryByteBuffer;
    use crate::boundary::effects::results::TerminalEffectResult;
    use crate::boundary::effects::storage_inputs::TerminalBoundaryStorageBuffer;
    use crate::boundary::effects::{
        TerminalEffect, TerminalEffectHandler, TerminalEffectRejection,
    };
    use crate::boundary::errors::TerminalInterpretError;
    use crate::boundary::results::TerminalExecutionStatus;
    use crate::boundary::values::TerminalScalarValue;
    use crate::embedding::adapters::TerminalImportAdapter;
    use crate::embedding::program::TerminalProgram;
    use crate::engine::execution::TerminalExecution;

    pub(crate) struct SimulatedProcessDomain;

    pub struct TerminalProcessExitAdmission {
        program: terminal_psi::TerminalPsiIdentity,
        domain: std::sync::Arc<SimulatedProcessDomain>,
        demand: crate::engine::capabilities::TerminalExternalCompletionDemand,
    }

    impl TerminalProcessExitAdmission {
        pub fn admit_isolated_simulated_domain(
            instance: &TerminalInstance<'_, '_>,
            boundary: BoundaryMachineId,
        ) -> Result<Self, TerminalBindingRejection> {
            if instance.closed {
                return Err(TerminalBindingRejection::InstanceClosed);
            }
            let execution = instance
                .execution
                .as_ref()
                .ok_or(TerminalBindingRejection::InstanceClosed)?;
            if !execution.access_view_occurrences.is_empty()
                || execution
                    .call_stack
                    .iter()
                    .any(|frame| !frame.access_view_occurrences.is_empty())
            {
                return Err(
                    TerminalBindingRejection::DomainDeathCrossingAdmissionMissing { boundary },
                );
            }
            let demand = execution
                .external_demands
                .iter()
                .find(|demand| demand.boundary == boundary)
                .copied()
                .ok_or(TerminalBindingRejection::NotRequiredImport { boundary })?;
            match demand.kind {
                semantic_vocabulary::ExternalCompletionKind::ProcessExit => {}
            }
            Ok(Self {
                program: instance.program.terminal_psi(),
                domain: std::sync::Arc::clone(&instance.process_domain),
                demand,
            })
        }

        pub(crate) fn answers(
            &self,
            program: terminal_psi::TerminalPsiIdentity,
            domain: Option<&std::sync::Arc<SimulatedProcessDomain>>,
            demand: crate::engine::capabilities::TerminalExternalCompletionDemand,
        ) -> bool {
            self.program == program
                && self.demand == demand
                && domain.is_some_and(|domain| std::sync::Arc::ptr_eq(domain, &self.domain))
        }
    }

    struct TerminalImportBinding<'host> {
        adapter: Box<dyn TerminalImportAdapter + 'host>,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum TerminalBindingRejection {
        ExternalCompletionRequiresDomainAdmission { boundary: BoundaryMachineId },
        ExternalCompletionDomainMismatch { boundary: BoundaryMachineId },
        DomainDeathCrossingAdmissionMissing { boundary: BoundaryMachineId },
        UnknownImport { boundary: BoundaryMachineId },
        NotRequiredImport { boundary: BoundaryMachineId },
        ProviderRealized { boundary: BoundaryMachineId },
        DuplicateBinding { boundary: BoundaryMachineId },
        SignatureMismatch { boundary: BoundaryMachineId },
        InstanceClosed,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum TerminalInstanceError {
        Closed,
        UnboundImports {
            missing: Vec<BoundaryMachineId>,
        },
        UnboundExternalCompletions {
            missing: Vec<crate::engine::capabilities::TerminalExternalCompletionDemand>,
        },
        Execution(TerminalInterpretError),
    }

    impl TerminalInstanceError {
        /// Follow the interpreter's operation context to the resource the host refused.
        pub fn sponsor_exhaustion(&self) -> Option<&str> {
            let Self::Execution(error) = self else {
                return None;
            };
            let mut current = error;
            loop {
                match current {
                    TerminalInterpretError::ExecutionSite { cause, .. }
                    | TerminalInterpretError::StructuralReplacement { cause, .. } => {
                        current = cause;
                    }
                    TerminalInterpretError::EffectRejected { rejection, .. } => {
                        return rejection.exhaustion_reason();
                    }
                    _ => return None,
                }
            }
        }
    }

    impl std::fmt::Display for TerminalInstanceError {
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                Self::Execution(reason) => reason.fmt(formatter),
                other => write!(formatter, "{other:?}"),
            }
        }
    }

    impl std::error::Error for TerminalInstanceError {}

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum TerminalInstanceCloseError {
        Busy,
    }

    pub struct TerminalInstance<'host, 'artifact> {
        program: TerminalProgram<'artifact>,
        entry: semantic_vocabulary::MachineId,
        process_domain: std::sync::Arc<SimulatedProcessDomain>,
        execution: Option<TerminalExecution<'artifact>>,
        required_imports: Vec<BoundaryMachineId>,
        bindings: BTreeMap<BoundaryMachineId, TerminalImportBinding<'host>>,
        storage_resolver: Option<std::rc::Rc<dyn crate::TerminalProviderStorageResolver + 'host>>,
        components: ComponentBindings<'host, 'artifact>,
        invoked: bool,
        closed: bool,
    }

    impl<'host, 'artifact> TerminalInstance<'host, 'artifact> {
        pub(crate) fn new(
            program: TerminalProgram<'artifact>,
            mut execution: TerminalExecution<'artifact>,
            required_imports: Vec<BoundaryMachineId>,
        ) -> Self {
            let process_domain = std::sync::Arc::new(SimulatedProcessDomain);
            execution.process_domain = Some(std::sync::Arc::clone(&process_domain));
            Self {
                program,
                entry: execution.current_machine,
                process_domain,
                execution: Some(execution),
                required_imports,
                bindings: BTreeMap::new(),
                storage_resolver: None,
                components: BTreeMap::new(),
                invoked: false,
                closed: false,
            }
        }

        pub fn program(&self) -> &TerminalProgram<'artifact> {
            &self.program
        }

        pub(super) fn install_compiler_observations(
            &mut self,
            issued: Option<Box<crate::layout_target::IssuedLayoutTarget>>,
            calls: &[crate::TerminalCallObservation],
        ) -> Result<(), TerminalInterpretError> {
            if !calls.is_empty() && !self.required_imports.is_empty() {
                return Err(TerminalInterpretError::UnsupportedSemanticVariant(
                    "compiler call observation requires an invocation without host imports",
                ));
            }
            let execution = self.execution.as_mut().ok_or(
                TerminalInterpretError::UnsupportedSemanticVariant(
                    "compiler observations require an unopened invocation",
                ),
            )?;
            execution.select_call_observations(calls)?;
            execution.issued_layout_target = issued;
            Ok(())
        }

        pub fn take_call_observations(&mut self) -> Vec<crate::ObservedTerminalCall> {
            self.execution
                .as_mut()
                .and_then(|execution| execution.call_observations.as_mut())
                .filter(|observations| observations.complete)
                .map(|observations| std::mem::take(&mut observations.records))
                .unwrap_or_default()
        }

        pub fn bind_storage_resolver(
            &mut self,
            resolver: std::rc::Rc<dyn crate::TerminalProviderStorageResolver + 'host>,
        ) -> Result<(), String> {
            if self.invoked || self.closed || self.storage_resolver.is_some() {
                return Err(
                    "provider storage resolver is already frozen or instance is closed".into(),
                );
            }
            self.storage_resolver = Some(resolver);
            Ok(())
        }

        pub fn import(
            &self,
            slot: BoundaryMachineId,
        ) -> Option<&terminal_psi::BoundaryMachineDeclaration> {
            if self.closed || !self.required_imports.contains(&slot) {
                return None;
            }
            self.program.program.boundary_machines().get(&slot)
        }

        pub fn bind_import(
            &mut self,
            slot: BoundaryMachineId,
            adapter: impl TerminalImportAdapter + 'host,
        ) -> Result<(), TerminalBindingRejection> {
            if self.closed {
                return Err(TerminalBindingRejection::InstanceClosed);
            }
            if self
                .program
                .program
                .boundary_machines()
                .get(&slot)
                .is_some_and(|declaration| {
                    matches!(
                        declaration.completion,
                        terminal_psi::BoundaryCompletion::External(_)
                    )
                })
            {
                return Err(
                    TerminalBindingRejection::ExternalCompletionRequiresDomainAdmission {
                        boundary: slot,
                    },
                );
            }
            if self.execution.as_ref().is_some_and(|execution| {
                matches!(
                    execution.boundary_supplies.get(&slot),
                    Some(
                        crate::boundary::effects::AdmittedBoundarySupply::ComponentProvider { .. }
                            | crate::boundary::effects::AdmittedBoundarySupply::DeferredComponent
                    )
                )
            }) {
                return Err(TerminalBindingRejection::ProviderRealized { boundary: slot });
            }
            let Some(declaration) = self.import(slot) else {
                return if self.execution.as_ref().is_some_and(|execution| {
                matches!(
                    execution.boundary_supplies.get(&slot),
                    Some(
                        crate::boundary::effects::AdmittedBoundarySupply::CheckedProvider(_)
                            | crate::boundary::effects::AdmittedBoundarySupply::ComponentProvider { .. }
                    )
                )
            }) {
                Err(TerminalBindingRejection::ProviderRealized { boundary: slot })
            } else if self.program.program.boundary_machines().contains_key(&slot) {
                Err(TerminalBindingRejection::NotRequiredImport { boundary: slot })
            } else {
                Err(TerminalBindingRejection::UnknownImport { boundary: slot })
            };
            };
            if self.bindings.contains_key(&slot) {
                return Err(TerminalBindingRejection::DuplicateBinding { boundary: slot });
            }
            let signature = crate::embedding::adapters::TerminalImportSignature::from(declaration);
            if adapter.implemented_signature() != signature {
                return Err(TerminalBindingRejection::SignatureMismatch { boundary: slot });
            }
            self.bindings.insert(
                slot,
                TerminalImportBinding {
                    adapter: Box::new(adapter),
                },
            );
            Ok(())
        }

        pub fn bind_component(
            &mut self,
            binding: TerminalComponentBinding<'host, 'artifact>,
        ) -> Result<(), String> {
            let selected = binding.selected();
            let boundary = selected.receiving_boundary();
            if self.closed
                || selected.receiver() != self.program.execution_identity()
                || !self.required_imports.contains(&boundary)
                || self.bindings.contains_key(&boundary)
                || self.components.contains_key(&boundary)
            {
                return Err(
                    "component binding names another, inactive or already bound receiver slot"
                        .into(),
                );
            }
            let expected = crate::boundary::effects::AdmittedBoundarySupply::ComponentProvider {
                provider: selected.provider(),
                provider_boundary: selected.provider_boundary(),
                candidate: selected.candidate(),
            };
            let execution = self
                .execution
                .as_mut()
                .ok_or("component receiver is closed")?;
            let deferred = execution.boundary_supplies.get(&boundary)
                == Some(&crate::boundary::effects::AdmittedBoundarySupply::DeferredComponent);
            if self.invoked
                && (!deferred
                    || !execution
                        .awaiting_effect
                        .as_ref()
                        .is_some_and(|pending| pending.awaiting_provider_readiness)
                    || execution.failure.is_some())
            {
                return Err(
                "component installation requires its reserved slot and a checked readiness pause"
                    .into(),
            );
            }
            if !deferred && execution.boundary_supplies.get(&boundary) != Some(&expected) {
                return Err(
                    "component binding differs from the receiver's exact admitted supply".into(),
                );
            }
            binding.require_ready()?;
            execution.boundary_supplies.insert(boundary, expected);
            self.components.insert(boundary, binding);
            Ok(())
        }

        pub fn bind_external_completion(
            &mut self,
            boundary: BoundaryMachineId,
            admission: TerminalProcessExitAdmission,
        ) -> Result<(), TerminalBindingRejection> {
            if self.closed {
                return Err(TerminalBindingRejection::InstanceClosed);
            }
            let execution = self
                .execution
                .as_mut()
                .ok_or(TerminalBindingRejection::InstanceClosed)?;
            let demand = execution
                .external_demands
                .iter()
                .find(|demand| demand.boundary == boundary)
                .copied()
                .ok_or(TerminalBindingRejection::NotRequiredImport { boundary })?;
            if !admission.answers(
                self.program.terminal_psi(),
                Some(&self.process_domain),
                demand,
            ) {
                return Err(TerminalBindingRejection::ExternalCompletionDomainMismatch {
                    boundary,
                });
            }
            if execution.external_bindings.contains_key(&boundary) {
                return Err(TerminalBindingRejection::DuplicateBinding { boundary });
            }
            execution.external_bindings.insert(boundary, admission);
            Ok(())
        }

        pub fn unbound_external_completions(
            &self,
        ) -> impl Iterator<Item = crate::engine::capabilities::TerminalExternalCompletionDemand> + '_
        {
            self.execution.iter().flat_map(|execution| {
                execution
                    .external_demands
                    .iter()
                    .copied()
                    .filter(|demand| !execution.external_bindings.contains_key(&demand.boundary))
            })
        }

        pub fn external_completion(
            &self,
        ) -> Option<&crate::boundary::results::TerminalExternalCompletion> {
            self.execution
                .as_ref()
                .and_then(|execution| execution.external_completion.as_ref())
        }

        pub fn established_erased_fields(
            &self,
        ) -> &[crate::boundary::structural_inputs::TerminalEstablishedErasedField] {
            self.execution.as_ref().map_or(&[], |execution| {
                execution.established_erased_fields.as_slice()
            })
        }

        pub fn unbound_imports(&self) -> impl Iterator<Item = BoundaryMachineId> + '_ {
            let closed = self.closed;
            self.required_imports.iter().copied().filter(move |slot| {
                !closed && !self.bindings.contains_key(slot) && !self.components.contains_key(slot)
            })
        }

        /// Active calls, from the immediate caller out to the entry. A no-return
        /// call keeps its edge; cleanup has no authored call operation.
        pub fn caller_sites(
            &self,
        ) -> impl Iterator<Item = (semantic_vocabulary::MachineId, terminal_psi::CallSite)> + '_
        {
            use crate::engine::execution::SuspendedCallResult;
            use terminal_psi::CallSite;

            self.execution.iter().flat_map(|frame| {
                frame.call_stack.iter().rev().filter_map(|caller| {
                    let site = match caller.result {
                        SuspendedCallResult::NoNormal { edge, .. }
                        | SuspendedCallResult::ExternalCompletion { edge, .. } => {
                            CallSite::Edge(edge)
                        }
                        SuspendedCallResult::OwnedCleanups { .. } => return None,
                        SuspendedCallResult::Scalar(_)
                        | SuspendedCallResult::Unit
                        | SuspendedCallResult::Structural { .. } => {
                            let index = caller.next_operation.checked_sub(1)?;
                            let operation = self
                                .program
                                .module()
                                .machine(caller.current_machine)?
                                .block(&caller.current)?
                                .operations
                                .get(index)?;
                            operation.declared_invocation()?;
                            CallSite::Operation(operation.id)
                        }
                    };
                    Some((caller.current_machine, site))
                })
            })
        }

        pub fn invoke(
            &mut self,
            meter: &mut dyn ExecutionMeter,
        ) -> Result<TerminalExecutionStatus, TerminalInstanceError> {
            if self.closed {
                return Err(TerminalInstanceError::Closed);
            }
            let missing =
                self.unbound_imports()
                    .filter(|boundary| {
                        self.execution
                    .as_ref()
                    .and_then(|execution| execution.boundary_supplies.get(boundary))
                    != Some(&crate::boundary::effects::AdmittedBoundarySupply::DeferredComponent)
                    })
                    .collect::<Vec<_>>();
            if !missing.is_empty() {
                return Err(TerminalInstanceError::UnboundImports { missing });
            }
            let missing = self.unbound_external_completions().collect::<Vec<_>>();
            if !missing.is_empty() {
                return Err(TerminalInstanceError::UnboundExternalCompletions { missing });
            }
            let execution = self
                .execution
                .as_mut()
                .ok_or(TerminalInstanceError::Closed)?;
            self.invoked = true;
            let status = execution
                .resume_with_components(
                    meter,
                    &mut ImportDispatch {
                        bindings: &mut self.bindings,
                        storage_resolver: self.storage_resolver.as_deref(),
                    },
                    &mut self.components,
                )
                .map_err(TerminalInstanceError::Execution);
            if let Some(observations) = execution.call_observations.as_mut() {
                match &status {
                    Ok(TerminalExecutionStatus::Complete(_)) => {
                        observations
                            .finish(execution.account.as_ref())
                            .map_err(TerminalInstanceError::Execution)?;
                    }
                    Ok(TerminalExecutionStatus::SponsorExhausted(_)) => {}
                    _ => execution.call_observations = None,
                }
            }
            status
        }

        pub fn observe_private_entry_ram(
            &self,
        ) -> Result<Vec<(u32, Vec<u8>)>, TerminalInstanceError> {
            self.execution
                .as_ref()
                .ok_or(TerminalInstanceError::Closed)?
                .observe_private_entry_ram()
                .map_err(TerminalInstanceError::Execution)
        }

        pub fn bind_provider_storage(
            &mut self,
            input: crate::TerminalProviderStorageInput,
        ) -> Result<(), crate::TerminalProviderStorageRejection> {
            match self.execution.as_mut() {
                Some(execution) => execution.bind_provider_storage(input),
                None => Err(crate::TerminalProviderStorageRejection {
                    input,
                    reason: "provider storage cannot bind a closed instance".into(),
                }),
            }
        }

        pub fn take_returned_provider_storage(
            &mut self,
            path: &[terminal_psi::StructuralPathSegment],
        ) -> Result<crate::TerminalReturnedProviderStorage, String> {
            self.execution
                .as_mut()
                .ok_or("provider storage cannot return from a closed instance")?
                .take_returned_provider_storage(path)
        }

        pub fn active_component_invocation_count(&self) -> usize {
            self.components
                .values()
                .filter(|binding| binding.has_pending_invocation())
                .count()
        }

        pub fn effects(&self) -> &[TerminalEffect] {
            self.execution
                .as_ref()
                .map(|execution| execution.effects())
                .unwrap_or(&[])
        }

        pub fn result(&self) -> Option<&crate::boundary::results::TerminalExecutionResult> {
            self.execution
                .as_ref()
                .and_then(|execution| execution.result())
        }

        pub fn observed_entry_result(
            &self,
            machine: semantic_vocabulary::MachineId,
            result: semantic_vocabulary::ValueId,
        ) -> Result<crate::TerminalScalarValue, TerminalInstanceError> {
            self.execution
                .as_ref()
                .ok_or(TerminalInstanceError::Closed)?
                .observed_entry_result(machine, result)
                .map_err(TerminalInstanceError::Execution)
        }

        pub fn is_finished(&self) -> bool {
            self.execution
                .as_ref()
                .is_none_or(|execution| execution.is_finished())
        }

        pub fn is_closed(&self) -> bool {
            self.closed
        }

        pub fn close(&mut self) -> Result<(), TerminalInstanceCloseError> {
            if self.closed {
                return Ok(());
            }
            if self.invoked
                && self
                    .execution
                    .as_ref()
                    .is_some_and(|execution| !execution.is_finished())
            {
                return Err(TerminalInstanceCloseError::Busy);
            }
            self.execution = None;
            self.bindings.clear();
            self.components.clear();
            self.closed = true;
            Ok(())
        }
    }

    struct ImportDispatch<'a, 'host> {
        bindings: &'a mut BTreeMap<BoundaryMachineId, TerminalImportBinding<'host>>,
        storage_resolver: Option<&'a (dyn crate::TerminalProviderStorageResolver + 'host)>,
    }

    impl<'a, 'host> ImportDispatch<'a, 'host> {
        fn adapter_at(
            &mut self,
            boundary: BoundaryMachineId,
        ) -> Result<&mut (dyn TerminalImportAdapter + 'host), TerminalEffectRejection> {
            Ok(&mut *self
                .bindings
                .get_mut(&boundary)
                .ok_or_else(|| {
                    TerminalEffectRejection::new("boundary call reached an unbound import slot")
                })?
                .adapter)
        }

        fn adapter_for(
            &mut self,
            effect: &TerminalEffect,
        ) -> Result<&mut (dyn TerminalImportAdapter + 'host), TerminalEffectRejection> {
            match effect {
                TerminalEffect::BoundaryCall { site, .. } => self.adapter_at(site.boundary),
                TerminalEffect::ComponentCall { .. } => Err(TerminalEffectRejection::new(
                    "a component completion receipt is not a host effect request",
                )),
                TerminalEffect::MemoryFence { .. } => Err(TerminalEffectRejection::new(
                    "completed ordering observations are not host effect requests",
                )),
                TerminalEffect::PortWrite { .. } => Err(TerminalEffectRejection::new(
                    "port output requires an admitted target-environment provider",
                )),
            }
        }
    }

    impl TerminalEffectHandler for ImportDispatch<'_, '_> {
        fn resolve_provider_storage(
            &mut self,
            request: &crate::TerminalProviderStorageRequest,
        ) -> Result<Option<crate::TerminalProviderStorageSupply>, TerminalEffectRejection> {
            match self.storage_resolver {
                Some(resolver) => resolver.resolve(request),
                None => Ok(None),
            }
        }
        fn poll_host_computation(
            &mut self,
            effect: &TerminalEffect,
        ) -> Result<bool, TerminalEffectRejection> {
            self.adapter_for(effect)?.poll_host_computation(effect)
        }

        fn poll_effect_readiness(
            &mut self,
            effect: &TerminalEffect,
        ) -> Result<bool, TerminalEffectRejection> {
            self.adapter_for(effect)?.poll_effect_readiness(effect)
        }

        fn accepts_reference_results(&self, boundary: BoundaryMachineId) -> bool {
            self.bindings
                .get(&boundary)
                .is_some_and(|binding| binding.adapter.accepts_reference_results(boundary))
        }

        fn storage_requirements(
            &self,
            boundary: BoundaryMachineId,
            scalar_arguments: &[TerminalScalarValue],
        ) -> crate::TerminalCallStorageRequirements<'_> {
            self.bindings.get(&boundary).map_or_else(
                crate::TerminalCallStorageRequirements::default,
                |binding| {
                    binding
                        .adapter
                        .storage_requirements(boundary, scalar_arguments)
                },
            )
        }

        fn prepare_original_storage(
            &mut self,
            boundary: BoundaryMachineId,
            requests: &[crate::boundary::effects::original_storage::TerminalOriginalStorageRequest],
        ) -> Result<
            crate::boundary::effects::original_storage::TerminalOriginalScalarBackings,
            TerminalEffectRejection,
        > {
            self.bindings
                .get_mut(&boundary)
                .ok_or_else(|| {
                    TerminalEffectRejection::new("original storage names no bound import")
                })?
                .adapter
                .prepare_original_storage(boundary, requests)
        }
        fn handle_effect_with_buffers(
            &mut self,
            effect: &TerminalEffect,
            byte_buffers: &mut [TerminalBoundaryByteBuffer],
            storage_buffers: &mut [TerminalBoundaryStorageBuffer],
        ) -> Result<TerminalEffectResult, TerminalEffectRejection> {
            self.adapter_for(effect)?.handle_effect_with_buffers(
                effect,
                byte_buffers,
                storage_buffers,
            )
        }

        fn handle_effect(
            &mut self,
            effect: &TerminalEffect,
        ) -> Result<(), TerminalEffectRejection> {
            self.adapter_for(effect)?.handle_effect(effect)
        }

        fn handle_effect_with_byte_buffers(
            &mut self,
            effect: &TerminalEffect,
            byte_buffers: &mut [TerminalBoundaryByteBuffer],
        ) -> Result<TerminalEffectResult, TerminalEffectRejection> {
            self.adapter_for(effect)?
                .handle_effect_with_byte_buffers(effect, byte_buffers)
        }

        fn handle_effect_result(
            &mut self,
            effect: &TerminalEffect,
        ) -> Result<TerminalEffectResult, TerminalEffectRejection> {
            self.adapter_for(effect)?.handle_effect_result(effect)
        }
    }
}
pub(crate) mod program {
    use crate::boundary::effects::{AdmittedBoundarySupply, TerminalRuntimeSupplies};
    use crate::boundary::errors::{
        TerminalArtifactInterpretError, TerminalInterpretError, malformed,
    };
    use build_model::accounting::EvaluationInvocationAccount;
    pub(crate) mod declarations {
        use crate::boundary::errors::{TerminalInterpretError, malformed};
        use terminal_psi::{
            BoundaryMachineDeclaration, StructuralTypeDeclaration, TerminalMachine,
        };

        pub(crate) trait ProgramDeclaration {
            type Identity: Ord;
            fn identity(&self) -> &Self::Identity;
        }

        pub(crate) struct ProgramDeclarations<'storage, Declaration> {
            declarations: &'storage [Declaration],
            replacements: &'storage [Declaration],
        }

        impl<Declaration> Clone for ProgramDeclarations<'_, Declaration> {
            fn clone(&self) -> Self {
                *self
            }
        }

        impl<Declaration> Copy for ProgramDeclarations<'_, Declaration> {}

        // Each answer is the one a scan of `values()` gives: a canonical
        // module declares each identity once, in order, and a replacement
        // stands for the row with its identity.
        impl<'storage> terminal_semantics::StructuralDeclarations<'storage>
            for ProgramDeclarations<'storage, StructuralTypeDeclaration>
        {
            fn unique(
                &self,
                id: semantic_vocabulary::StructuralTypeId,
            ) -> Option<&'storage StructuralTypeDeclaration> {
                self.get(&id)
            }

            fn representation_view(
                &self,
                view: semantic_vocabulary::StructuralTypeId,
                stored: Option<semantic_vocabulary::ScalarType>,
            ) -> Option<&'storage StructuralTypeDeclaration> {
                terminal_semantics::representation_view_declaration(
                    std::iter::once(self.get(&view)?),
                    view,
                    stored,
                )
            }

            fn inline_carrier(
                &self,
                field: &terminal_psi::StructuralFieldDeclaration,
            ) -> Option<&'storage StructuralTypeDeclaration> {
                if field.relevance.is_erased() {
                    return None;
                }
                terminal_semantics::inline_carrier_declaration(
                    std::iter::once(self.get(&field.inline_carrier?)?),
                    field,
                )
            }

            fn leaf_declaration(
                &self,
                field_type: &terminal_psi::StructuralFieldType,
            ) -> Option<&'storage StructuralTypeDeclaration> {
                if let terminal_psi::StructuralFieldType::Structural(id) = field_type {
                    return self.get(id);
                }
                field_type.leaf_declaration(self.values())
            }
        }

        impl<'storage, Declaration: ProgramDeclaration> ProgramDeclarations<'storage, Declaration> {
            pub(crate) const fn new(declarations: &'storage [Declaration]) -> Self {
                Self {
                    declarations,
                    replacements: &[],
                }
            }

            pub(crate) const fn with_replacements(
                declarations: &'storage [Declaration],
                replacements: &'storage [Declaration],
            ) -> Self {
                Self {
                    declarations,
                    replacements,
                }
            }

            pub(crate) fn get(
                &self,
                identity: &Declaration::Identity,
            ) -> Option<&'storage Declaration> {
                if let Ok(position) = self
                    .replacements
                    .binary_search_by(|row| row.identity().cmp(identity))
                {
                    return self.replacements.get(position);
                }
                let position = self
                    .declarations
                    .binary_search_by(|row| row.identity().cmp(identity))
                    .ok()?;
                self.declarations.get(position)
            }

            pub(crate) fn declaration(
                &self,
                identity: &Declaration::Identity,
            ) -> Result<&'storage Declaration, TerminalInterpretError> {
                self.get(identity).ok_or(malformed())
            }

            pub(crate) fn values(
                &self,
            ) -> impl DoubleEndedIterator<Item = &'storage Declaration>
            + ExactSizeIterator
            + Clone
            + use<'storage, Declaration> {
                let replacements = self.replacements;
                self.declarations.iter().map(move |row| {
                    replacements
                        .binary_search_by(|replacement| replacement.identity().cmp(row.identity()))
                        .ok()
                        .map_or(row, |position| &replacements[position])
                })
            }

            pub(crate) fn contains_key(&self, identity: &Declaration::Identity) -> bool {
                self.get(identity).is_some()
            }
        }

        impl<'storage> terminal_semantics::StructuralDeclarations<'storage>
            for &ProgramDeclarations<'storage, StructuralTypeDeclaration>
        {
            fn unique(
                &self,
                id: semantic_vocabulary::StructuralTypeId,
            ) -> Option<&'storage StructuralTypeDeclaration> {
                (**self).unique(id)
            }

            fn representation_view(
                &self,
                view: semantic_vocabulary::StructuralTypeId,
                stored: Option<semantic_vocabulary::ScalarType>,
            ) -> Option<&'storage StructuralTypeDeclaration> {
                (**self).representation_view(view, stored)
            }

            fn inline_carrier(
                &self,
                field: &terminal_psi::StructuralFieldDeclaration,
            ) -> Option<&'storage StructuralTypeDeclaration> {
                (**self).inline_carrier(field)
            }

            fn leaf_declaration(
                &self,
                field_type: &terminal_psi::StructuralFieldType,
            ) -> Option<&'storage StructuralTypeDeclaration> {
                (**self).leaf_declaration(field_type)
            }
        }

        impl ProgramDeclaration for TerminalMachine {
            type Identity = semantic_vocabulary::MachineId;
            fn identity(&self) -> &Self::Identity {
                &self.id
            }
        }

        impl ProgramDeclaration for StructuralTypeDeclaration {
            type Identity = semantic_vocabulary::StructuralTypeId;
            fn identity(&self) -> &Self::Identity {
                &self.id
            }
        }

        impl ProgramDeclaration for BoundaryMachineDeclaration {
            type Identity = semantic_vocabulary::BoundaryMachineId;
            fn identity(&self) -> &Self::Identity {
                &self.id
            }
        }
    }
    pub(crate) use declarations::ProgramDeclarations;

    use crate::boundary::structural_inputs::TerminalStructuralInputs;
    use crate::boundary::values::TerminalScalarValue;
    use crate::embedding::instance::TerminalInstance;
    use semantic_vocabulary::{BlockId, BoundaryMachineId, MachineId, StructuralTypeId};
    use std::collections::BTreeMap;
    use terminal_psi::{
        BoundaryMachineDeclaration, StructuralTypeDeclaration, TerminalModule, TerminalPsiIdentity,
    };

    pub(crate) struct AdmittedTerminalProgram<'artifact> {
        pub(crate) identity: terminal_codec::TerminalExecutionIdentity,
        pub(crate) catalogs: std::sync::Arc<ProgramStorage<'artifact>>,
        pub(crate) target_layouts:
            Option<std::sync::Arc<psi_target_layouts::AdmittedTargetLayouts>>,
        pub(crate) scalar_geometry: Option<std::sync::Arc<[StructuralTypeDeclaration]>>,
        /// Prepared from this exact declaration view, including any layout
        /// or scalar-geometry replacements, the first time execution asks,
        /// and kept unchanged for the life of this program.
        memberships: std::sync::OnceLock<terminal_semantics::IntrinsicMemberships>,
        /// The declaration-dependent part of every primitive store, prepared
        /// for the whole program the first time one runs.
        primitive_stores: std::sync::OnceLock<Vec<PreparedPrimitiveStore>>,
        /// Which declared types hold claims, prepared for this exact view the
        /// first time an owned value's claims are checked.
        claim_roster: std::sync::OnceLock<
            Result<terminal_semantics::owned_structure::ClaimRoster, &'static str>,
        >,
    }

    /// One `WriteOnlyPrimitiveStore` of this program, by machine and
    /// operation, with the exact destination and path its plan answers for.
    struct PreparedPrimitiveStore {
        machine: MachineId,
        operation: semantic_vocabulary::OperationId,
        destination: semantic_vocabulary::PlaceId,
        path: Vec<terminal_psi::StructuralPathSegment>,
        plan: Option<crate::storage::primitive_storage::PrimitiveStorePlan>,
    }

    pub(crate) enum ProgramStorage<'artifact> {
        Borrowed(terminal_codec::TerminalExecutionView<'artifact>),
        // Existing engine fixtures intentionally edit unsealed rows. They
        // cannot borrow or mutate a production closed-code owner.
        #[cfg(test)]
        Fixture(TerminalModule),
        Retained(terminal_codec::RetainedTerminalCode),
    }

    impl std::ops::Deref for ProgramStorage<'_> {
        type Target = TerminalModule;

        fn deref(&self) -> &Self::Target {
            match self {
                Self::Borrowed(code) => code.module(),
                Self::Retained(code) => code.view().module(),
                #[cfg(test)]
                Self::Fixture(module) => module,
            }
        }
    }

    impl ProgramStorage<'_> {
        pub(crate) fn closed_types(
            &self,
        ) -> Result<&terminal_codec::ClosedTypeCatalog, TerminalInterpretError> {
            match self {
                Self::Borrowed(code) => Ok(code.closed_types()),
                Self::Retained(code) => Ok(code.view().closed_types()),
                #[cfg(test)]
                Self::Fixture(_) => Err(TerminalInterpretError::UnsupportedSemanticVariant(
                    "mutable engine fixture has no sealed type catalogue",
                )),
            }
        }

        pub(crate) fn closed_module(
            &self,
        ) -> Result<&terminal_codec::ClosedTerminalModule, TerminalInterpretError> {
            match self {
                Self::Borrowed(code) => Ok(code.closed_module()),
                Self::Retained(code) => Ok(code.view().closed_module()),
                #[cfg(test)]
                Self::Fixture(_) => Err(TerminalInterpretError::UnsupportedSemanticVariant(
                    "mutable engine fixture has no closed publication owner",
                )),
            }
        }
    }

    impl AsRef<TerminalModule> for ProgramStorage<'_> {
        fn as_ref(&self) -> &TerminalModule {
            self
        }
    }

    impl<'artifact> AdmittedTerminalProgram<'artifact> {
        pub(crate) fn new(
            identity: terminal_codec::TerminalExecutionIdentity,
            catalogs: std::sync::Arc<ProgramStorage<'artifact>>,
            target_layouts: Option<std::sync::Arc<psi_target_layouts::AdmittedTargetLayouts>>,
            scalar_geometry: Option<std::sync::Arc<[StructuralTypeDeclaration]>>,
        ) -> Self {
            Self {
                identity,
                catalogs,
                target_layouts,
                scalar_geometry,
                memberships: std::sync::OnceLock::new(),
                primitive_stores: std::sync::OnceLock::new(),
                claim_roster: std::sync::OnceLock::new(),
            }
        }

        fn prepare_memberships(&self) -> terminal_semantics::IntrinsicMemberships {
            terminal_semantics::IntrinsicMemberships::prepare(
                self.structural_types().values(),
                self.catalogs.types.structural_domains.iter(),
            )
        }

        /// Which declared restrictions confer which whole-scalar domains,
        /// prepared once for this program. Programs that never ask, such as
        /// most compile-time invocations, never pay for it. An engine
        /// fixture edits its module after construction, so it prepares on
        /// each request.
        pub(crate) fn intrinsic_memberships(
            &self,
        ) -> std::borrow::Cow<'_, terminal_semantics::IntrinsicMemberships> {
            #[cfg(test)]
            if matches!(*self.catalogs, ProgramStorage::Fixture(_)) {
                return std::borrow::Cow::Owned(self.prepare_memberships());
            }
            std::borrow::Cow::Borrowed(self.memberships.get_or_init(|| self.prepare_memberships()))
        }

        /// The claim roster of this program's declaration view, or why the
        /// view has none. An engine fixture prepares on each request.
        pub(crate) fn claim_roster(
            &self,
        ) -> Result<
            std::borrow::Cow<'_, terminal_semantics::owned_structure::ClaimRoster>,
            &'static str,
        > {
            let prepare = || {
                terminal_semantics::owned_structure::ClaimRoster::prepare(
                    self.structural_types().values(),
                )
            };
            #[cfg(test)]
            if matches!(*self.catalogs, ProgramStorage::Fixture(_)) {
                return prepare().map(std::borrow::Cow::Owned);
            }
            self.claim_roster
                .get_or_init(prepare)
                .as_ref()
                .map(std::borrow::Cow::Borrowed)
                .map_err(|reason| *reason)
        }

        fn prepare_primitive_stores(&self) -> Vec<PreparedPrimitiveStore> {
            let mut stores = Vec::new();
            for machine in &self.catalogs.machines {
                for operation in machine.operations() {
                    let terminal_psi::OperationKind::WriteOnlyPrimitiveStore {
                        destination,
                        ref path,
                        ..
                    } = operation.kind
                    else {
                        continue;
                    };
                    stores.push(PreparedPrimitiveStore {
                        machine: machine.id,
                        operation: operation.id,
                        destination,
                        path: path.clone(),
                        plan: crate::storage::primitive_storage::primitive_store_plan(
                            self,
                            machine,
                            destination,
                            path,
                        ),
                    });
                }
            }
            stores.sort_by_key(|store| (store.machine, store.operation));
            stores
        }

        /// The prepared plan for this store. A store the program does not
        /// declare in `machine`, such as an engine fixture's, is planned
        /// here instead, with the same answer.
        pub(crate) fn primitive_store_plan(
            &self,
            machine: MachineId,
            operation: &terminal_psi::Operation,
            destination: semantic_vocabulary::PlaceId,
            path: &[terminal_psi::StructuralPathSegment],
        ) -> Result<
            std::borrow::Cow<'_, crate::storage::primitive_storage::PrimitiveStorePlan>,
            TerminalInterpretError,
        > {
            #[cfg(test)]
            let fixture = matches!(*self.catalogs, ProgramStorage::Fixture(_));
            #[cfg(not(test))]
            let fixture = false;
            if !fixture {
                let stores = self
                    .primitive_stores
                    .get_or_init(|| self.prepare_primitive_stores());
                if let Ok(position) = stores
                    .binary_search_by_key(&(machine, operation.id), |store| {
                        (store.machine, store.operation)
                    })
                    && stores[position].destination == destination
                    && stores[position].path == path
                {
                    return stores[position]
                        .plan
                        .as_ref()
                        .map(std::borrow::Cow::Borrowed)
                        .ok_or(malformed());
                }
            }
            crate::storage::primitive_storage::primitive_store_plan(
                self,
                self.machine(machine)?,
                destination,
                path,
            )
            .map(std::borrow::Cow::Owned)
            .ok_or(malformed())
        }
    }

    impl AdmittedTerminalProgram<'_> {
        #[cfg(test)]
        pub(crate) fn fixture_module_mut(&mut self) -> &mut TerminalModule {
            let storage = std::sync::Arc::get_mut(&mut self.catalogs)
                .expect("fixture construction requires unshared storage");
            let ProgramStorage::Fixture(module) = storage else {
                panic!("fixture construction requires its own module");
            };
            module
        }

        pub(crate) fn resolve_entry(
            &self,
            installation: Option<&crate::boundary::effects::TerminalRuntimeSupplies>,
        ) -> Result<MachineId, TerminalInterpretError> {
            if installation.is_some_and(|installation| installation.code_identity != self.identity)
            {
                return Err(TerminalInterpretError::ProviderInstallationIdentityMismatch);
            }
            if let Some(target) = self.catalogs.scalars.folded_constants.target.as_deref() {
                let supplies = installation
                    .filter(|supplies| supplies.folded_constant_target.as_deref() == Some(target))
                    .ok_or(TerminalInterpretError::FoldedConstantClosure(
                        "missing or incompatible realization closure",
                    ))?;
                if supplies
                    .target_machines
                    .as_ref()
                    .map(|selected| selected.target())
                    != Some(target)
                {
                    return Err(TerminalInterpretError::FoldedConstantClosure(
                        "execution target differs from folded constant closure",
                    ));
                }
            }
            let root = installation
                .and_then(|supplies| supplies.entry)
                .or(self.catalogs.entry)
                .ok_or(TerminalInterpretError::VerifiedEntryMachineMissing)?;
            let selected =
                installation.and_then(|installation| installation.target_machines.as_ref());
            let machine = psi_target_layouts::TerminalTargetContext::resolve_entry(
                selected,
                self.identity.semantic(),
                &self.catalogs,
                root,
            )
            .map_err(|error| match error {
                psi_target_layouts::TerminalEntryError::MissingMachine(_) => {
                    TerminalInterpretError::VerifiedEntryMachineMissing
                }
                other => TerminalInterpretError::TerminalEntry(other),
            })?;
            self.machines()
                .get(&machine)
                .ok_or(TerminalInterpretError::VerifiedEntryMachineMissing)?;
            Ok(machine)
        }

        fn validate_layout_selection(
            &self,
            installation: Option<&crate::boundary::effects::TerminalRuntimeSupplies>,
        ) -> Result<(), TerminalInterpretError> {
            let layouts = installation.and_then(|supplies| supplies.target_layouts.as_ref());
            if let Some(layouts) = layouts {
                let machines = installation.and_then(|supplies| supplies.target_machines.as_ref());
                if layouts.terminal_psi() != self.identity.semantic()
                    || machines.is_none_or(|machines| {
                        machines.terminal_psi() != self.identity.semantic()
                            || machines.target() != layouts.target()
                    })
                {
                    return Err(TerminalInterpretError::TargetLayoutSelectionIdentityMismatch);
                }
            } else if let Some(application) = self.catalogs.layouts.target_layout_applications.first() {
                return Err(TerminalInterpretError::TargetLayoutSelectionMissing(
                    application.id,
                ));
            } else if self
                .catalogs
                .access.access_lifecycle_plans
                .iter()
                .any(|plan| plan.target.is_some())
            {
                return Err(TerminalInterpretError::UnsupportedSemanticVariant(
                    "whole-value lifecycle requires admitted target geometry",
                ));
            }
            Ok(())
        }

        pub(crate) fn machines(&self) -> ProgramDeclarations<'_, terminal_psi::TerminalMachine> {
            ProgramDeclarations::new(&self.catalogs.machines)
        }

        pub(crate) fn machine(
            &self,
            machine_id: MachineId,
        ) -> Result<&terminal_psi::TerminalMachine, TerminalInterpretError> {
            self.machines().get(&machine_id).ok_or(malformed())
        }

        pub(crate) fn verified_machine(
            &self,
            machine: MachineId,
        ) -> Result<&terminal_psi::TerminalMachine, TerminalInterpretError> {
            self.machines()
                .get(&machine)
                .ok_or(TerminalInterpretError::VerifiedCallTargetMissing(machine))
        }

        pub(crate) fn verified_block(
            &self,
            machine: MachineId,
            block: BlockId,
        ) -> Result<&terminal_psi::Block, TerminalInterpretError> {
            self.verified_machine(machine)?
                .block(&block)
                .ok_or(TerminalInterpretError::VerifiedBlockMissing)
        }

        pub(crate) fn verified_boundary_machine(
            &self,
            boundary: &BoundaryMachineId,
        ) -> Result<&BoundaryMachineDeclaration, TerminalInterpretError> {
            self.boundary_machines().get(boundary).ok_or(
                TerminalInterpretError::VerifiedBoundaryMachineMissing(*boundary),
            )
        }

        pub(crate) fn structural_types(
            &self,
        ) -> ProgramDeclarations<'_, StructuralTypeDeclaration> {
            ProgramDeclarations::with_replacements(
                &self.catalogs.types.structural_types,
                self.scalar_geometry.as_deref().unwrap_or_else(|| {
                    self.target_layouts
                        .as_ref()
                        .map_or(&[], |layouts| layouts.resolved_records())
                }),
            )
        }

        pub(crate) fn access_lifecycle_plans(
            &self,
        ) -> &[terminal_psi::TerminalAccessLifecyclePlan] {
            self.target_layouts
                .as_ref()
                .map_or(self.catalogs.access.access_lifecycle_plans.as_slice(), |layouts| {
                    layouts.resolved_lifecycles()
                })
        }

        pub(crate) fn structural_declaration(
            &self,
            structural_type: StructuralTypeId,
        ) -> Result<&StructuralTypeDeclaration, TerminalInterpretError> {
            self.structural_types().declaration(&structural_type)
        }

        pub(crate) fn boundary_machines(
            &self,
        ) -> ProgramDeclarations<'_, BoundaryMachineDeclaration> {
            ProgramDeclarations::new(&self.catalogs.interfaces.boundary_machines)
        }

        pub(crate) fn has_provider(&self, boundary: &BoundaryMachineId) -> bool {
            self.catalogs
                .interfaces.provider_candidates
                .binary_search_by_key(boundary, |candidate| candidate.boundary)
                .is_ok()
        }

        pub(crate) fn import_declaration(
            &self,
            slot: BoundaryMachineId,
        ) -> Option<&BoundaryMachineDeclaration> {
            if self.has_provider(&slot) {
                return None;
            }
            self.boundary_machines().get(&slot).filter(|declaration| {
                declaration.role == terminal_psi::BoundaryDeclarationRole::Callable
                    && !interpreter_owns_boundary_execution(declaration)
            })
        }
    }

    #[derive(Clone)]
    pub struct TerminalProgram<'artifact> {
        pub(crate) program: std::sync::Arc<AdmittedTerminalProgram<'artifact>>,
    }

    impl std::fmt::Debug for TerminalProgram<'_> {
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter
                .debug_struct("TerminalProgram")
                .field("identity", &self.program.identity)
                .finish()
        }
    }

    pub struct TerminalProgramEntry<'program> {
        pub machine: MachineId,
        pub scalar_parameters: &'program [terminal_psi::ValueDeclaration],
        pub structural_parameters: &'program [terminal_psi::StructuralParameterDeclaration],
        access_view_inputs: &'program [terminal_psi::TerminalAccessViewInput],
        pub result: &'program terminal_psi::TerminalMachineResult,
        requires: &'program [semantic_vocabulary::Proposition],
    }

    impl TerminalProgramEntry<'_> {
        /// Whether the entry's published `requires` clauses hold with nothing
        /// known about its arguments. A host that invokes an entry is its
        /// caller and owes every clause; a host that establishes no facts
        /// about what it passes can owe only clauses that are closed and
        /// true. `Some(true)` when every clause is, `Some(false)` when one is
        /// closed and false, and `None` when one depends on an argument.
        pub fn unconditional_precondition(&self) -> Result<Option<bool>, String> {
            self.precondition_with(&[])
        }

        /// The caller has already established these exact propositions on the
        /// supplied arguments. Other requirements still owe their normal check.
        pub fn precondition_with(
            &self,
            established: &[semantic_vocabulary::Proposition],
        ) -> Result<Option<bool>, String> {
            let mut integers = closed_predicates::ClosedIntegerEvaluator::default();
            let mut undecided = false;
            for clause in self.requires {
                if established.contains(clause) {
                    continue;
                }
                match closed_predicates::closed_guard(clause, &mut integers)
                    .map_err(|error| format!("entry precondition evaluation: {error:?}"))?
                {
                    Some(true) => {}
                    Some(false) => return Ok(Some(false)),
                    None => undecided = true,
                }
            }
            Ok((!undecided).then_some(true))
        }

        pub fn access_view_inputs(
            &self,
        ) -> impl Iterator<Item = &terminal_psi::TerminalAccessViewInput> {
            self.access_view_inputs
                .iter()
                .filter(move |row| row.machine == self.machine)
        }
    }

    /// Received code needs the host's authorization; only its producer may
    /// reuse produced code without one.
    fn require_produced(
        code: terminal_codec::TerminalExecutionView<'_>,
    ) -> Result<(), psi_authorization::ArtifactTrustError> {
        if code.closed_module().requires_host_trust() {
            return Err(psi_authorization::ArtifactTrustError::ProducerTrustRequired);
        }
        Ok(())
    }

    impl<'artifact> TerminalProgram<'artifact> {
        pub fn module(&self) -> &TerminalModule {
            &self.program.catalogs
        }

        pub fn closed_module(
            &self,
        ) -> Result<&terminal_codec::ClosedTerminalModule, TerminalInterpretError> {
            self.program.catalogs.closed_module()
        }

        pub fn closed_types(
            &self,
        ) -> Result<&terminal_codec::ClosedTypeCatalog, TerminalInterpretError> {
            self.program.catalogs.closed_types()
        }

        pub fn entry(&self) -> Result<TerminalProgramEntry<'_>, TerminalInterpretError> {
            self.describe_entry(self.program.resolve_entry(None)?)
        }

        pub fn entry_installed(
            &self,
            installation: &crate::boundary::effects::TerminalRuntimeSupplies,
        ) -> Result<TerminalProgramEntry<'_>, TerminalInterpretError> {
            self.program.validate_layout_selection(Some(installation))?;
            self.describe_entry(self.program.resolve_entry(Some(installation))?)
        }

        fn describe_entry(
            &self,
            entry: MachineId,
        ) -> Result<TerminalProgramEntry<'_>, TerminalInterpretError> {
            let machine = self
                .program
                .machines()
                .get(&entry)
                .ok_or(TerminalInterpretError::VerifiedEntryMachineMissing)?;
            Ok(TerminalProgramEntry {
                machine: entry,
                scalar_parameters: &machine.parameters,
                structural_parameters: &machine.structural_parameters,
                access_view_inputs: &self.program.catalogs.access.access_view_inputs,
                result: &machine.result,
                requires: &machine.contract.requires,
            })
        }

        pub fn structural_type(
            &self,
            identity: StructuralTypeId,
        ) -> Option<&StructuralTypeDeclaration> {
            self.program.structural_types().get(&identity)
        }

        pub fn execution_requirements(
            &self,
            installation: Option<&crate::boundary::effects::TerminalRuntimeSupplies>,
        ) -> Result<
            crate::engine::capabilities::TerminalExecutionRequirements,
            TerminalInterpretError,
        > {
            self.program.validate_layout_selection(installation)?;
            crate::engine::capabilities::check(&self.program, installation)
        }

        /// Build this program's callable projection once for `providers`, so
        /// several checks and instances of its entries under the same
        /// providers reuse it. Every check that receives it refuses a
        /// projection built for another program or provider selection.
        pub fn callable_analysis(
            &self,
            providers: Option<&psi_provider_selections::AdmittedProviderSelections>,
        ) -> crate::engine::capabilities::TerminalCallableAnalysis<'_, 'artifact> {
            let installation =
                providers.map(crate::boundary::effects::TerminalRuntimeSupplies::from);
            crate::engine::capabilities::TerminalCallableAnalysis::new(self, installation.as_ref())
        }

        /// [`Self::execution_requirements`] over a projection prepared by
        /// [`Self::callable_analysis`].
        pub fn execution_requirements_with_callable_analysis(
            &self,
            installation: Option<&crate::boundary::effects::TerminalRuntimeSupplies>,
            analysis: &crate::engine::capabilities::TerminalCallableAnalysis<'_, '_>,
        ) -> Result<
            crate::engine::capabilities::TerminalExecutionRequirements,
            TerminalInterpretError,
        > {
            self.program.validate_layout_selection(installation)?;
            crate::engine::capabilities::check_with_private_entry_ram(
                &self.program,
                installation,
                &[],
                None,
                Some(analysis),
            )
        }

        pub fn execution_requirements_with_private_entry_ram(
            &self,
            installation: Option<&crate::boundary::effects::TerminalRuntimeSupplies>,
            recipes: &[psi_entry_memory::CheckedPrivateEntryRamRecipe],
        ) -> Result<
            crate::engine::capabilities::TerminalExecutionRequirements,
            TerminalInterpretError,
        > {
            self.program.validate_layout_selection(installation)?;
            crate::engine::capabilities::check_with_private_entry_ram(
                &self.execution_program(installation, None),
                installation,
                recipes,
                None,
                None,
            )
        }

        /// Execute an artifact the receiving host has authorized. The witness
        /// binds the host's producer trust to this exact artifact; bytes,
        /// a manifest digest or a producer name copied from the artifact
        /// cannot construct it.
        pub fn admit(artifact: &'artifact psi_authorization::AuthorizedTerminalArtifact) -> Self {
            let view = terminal_codec::TerminalExecutionView::from(artifact.artifact().view());
            Self::from_admitted(AdmittedTerminalProgram::new(
                view.custody_identity(),
                std::sync::Arc::new(ProgramStorage::Borrowed(view)),
                None,
                None,
            ))
        }

        /// [`Self::admit`] for a program that outlives the borrowed witness.
        pub fn retain_authorized(
            artifact: &psi_authorization::AuthorizedTerminalArtifact,
        ) -> TerminalProgram<'static> {
            let code = artifact.artifact().retain_code();
            TerminalProgram::from_admitted(AdmittedTerminalProgram::new(
                code.view().custody_identity(),
                std::sync::Arc::new(ProgramStorage::Retained(code)),
                None,
                None,
            ))
        }

        /// Reuse code this process produced, such as a compiler stage's
        /// published artifact or invocation code. Code decoded from bytes is
        /// refused here: it executes only through [`Self::admit`] with the
        /// host's authorization of its exact artifact and producer.
        pub fn borrow(
            artifact: impl Into<terminal_codec::TerminalExecutionView<'artifact>>,
        ) -> Result<Self, psi_authorization::ArtifactTrustError> {
            let artifact = artifact.into();
            require_produced(artifact)?;
            Ok(Self::from_admitted(AdmittedTerminalProgram::new(
                artifact.custody_identity(),
                std::sync::Arc::new(ProgramStorage::Borrowed(artifact)),
                None,
                None,
            )))
        }

        /// [`Self::borrow`] for retained code that outlives its producer's view.
        pub fn retain(
            code: terminal_codec::RetainedTerminalCode,
        ) -> Result<Self, psi_authorization::ArtifactTrustError> {
            require_produced(code.view())?;
            Ok(Self::from_admitted(AdmittedTerminalProgram::new(
                code.view().custody_identity(),
                std::sync::Arc::new(ProgramStorage::Retained(code)),
                None,
                None,
            )))
        }

        pub fn terminal_psi(&self) -> TerminalPsiIdentity {
            self.program.identity.semantic()
        }

        pub fn execution_identity(&self) -> &terminal_codec::TerminalExecutionIdentity {
            &self.program.identity
        }

        pub fn admit_runtime_supplies(
            &self,
            request: crate::boundary::effects::TerminalRuntimeSupplyRequest<'_>,
        ) -> Result<crate::boundary::effects::TerminalRuntimeSupplies, TerminalInterpretError>
        {
            if request.installation.is_some_and(|installation| {
                installation.execution_identity() != &self.program.identity
            }) {
                return Err(TerminalInterpretError::ProviderInstallationIdentityMismatch);
            }
            if request
                .target_machines
                .is_some_and(|selected| selected.terminal_psi() != self.program.identity.semantic())
            {
                return Err(TerminalInterpretError::TargetContextIdentityMismatch);
            }
            let mut installation = request
                .installation
                .map(TerminalRuntimeSupplies::from)
                .unwrap_or_else(|| TerminalRuntimeSupplies {
                    provider_selections: None,
                    x86_flags: None,
                    memory_ordering: None,
                    code_identity: self.program.identity.clone(),
                    entry: None,
                    supplies: BTreeMap::new(),
                    folded_constant_target: None,
                    target_machines: None,
                    target_layouts: None,
                    native_code_layouts: None,
                });
            if request.x86_flags.is_some()
                && request.target_machines.is_none_or(|selected| {
                    !terminal_psi::RegisterTargetApplicability::X86_64
                        .accepts_target_name(selected.target())
                })
            {
                return Err(TerminalInterpretError::UnsupportedSemanticVariant(
                    "x86 flags environment requires an admitted x86 target",
                ));
            }
            if request.memory_ordering.is_some_and(|model| {
                request
                    .target_machines
                    .is_none_or(|selected| !model.accepts_target(selected.target()))
            }) {
                return Err(TerminalInterpretError::UnsupportedSemanticVariant(
                    "memory-ordering environment requires its admitted matching target",
                ));
            }
            if let Some(layouts) = request.native_code_layouts {
                let Some(target) = request.target_machines else {
                    return Err(TerminalInterpretError::UnsupportedSemanticVariant(
                        "native layout inspection requires its admitted target",
                    ));
                };
                if !layouts.accepts(self.program.identity.clone(), target.target())
                    || layouts
                        .layouts()
                        .iter()
                        .any(|layout| self.program.catalogs.machine(layout.machine).is_none())
                {
                    return Err(TerminalInterpretError::UnsupportedSemanticVariant(
                        "native layout inspection names another program, target or machine",
                    ));
                }
                installation.native_code_layouts = Some(std::sync::Arc::new(layouts.clone()));
            }
            installation.memory_ordering = request.memory_ordering;
            installation.x86_flags = request.x86_flags;
            installation.target_machines = request.target_machines.cloned();
            installation.entry = request.entry;
            installation.target_layouts = request.target_layouts.cloned().map(std::sync::Arc::new);
            self.program
                .validate_layout_selection(Some(&installation))?;
            self.program.resolve_entry(Some(&installation))?;
            for &boundary in request.host_imports {
                self.select_external_supply(
                    &mut installation,
                    boundary,
                    AdmittedBoundarySupply::HostImport,
                )?;
            }
            for &boundary in request.external_completions {
                self.select_external_supply(
                    &mut installation,
                    boundary,
                    AdmittedBoundarySupply::ExternalCompletion,
                )?;
            }
            Ok(installation)
        }

        pub fn defer_component(
            &self,
            mut installation: TerminalRuntimeSupplies,
            boundary: BoundaryMachineId,
        ) -> Result<TerminalRuntimeSupplies, TerminalInterpretError> {
            if installation.code_identity != self.program.identity {
                return Err(TerminalInterpretError::ProviderInstallationIdentityMismatch);
            }
            self.select_external_supply(
                &mut installation,
                boundary,
                AdmittedBoundarySupply::DeferredComponent,
            )?;
            Ok(installation)
        }

        fn select_external_supply(
            &self,
            installation: &mut TerminalRuntimeSupplies,
            boundary: BoundaryMachineId,
            supply: AdmittedBoundarySupply,
        ) -> Result<(), TerminalInterpretError> {
            let rejected = |reason| TerminalInterpretError::RuntimeSupplySelectionRejected {
                boundary,
                reason,
            };
            let declaration = self
                .program
                .boundary_machines()
                .get(&boundary)
                .ok_or_else(|| rejected("external supply names no declared boundary"))?;
            if declaration.role != terminal_psi::BoundaryDeclarationRole::Callable {
                return Err(rejected(
                    "external supply names contract evidence rather than a callable",
                ));
            }
            let execution = declaration
                .declaration
                .as_ref()
                .map(|callable| callable.execution);
            // Numeric declarations remain compiler owned, but their admitted
            // scalar adapter executes through a host-import slot. A component
            // or checked provider may not replace that declaration.
            if execution.is_some_and(|execution| execution.compiler_owned())
                && !(execution == Some(terminal_psi::CheckedBoundaryExecution::NumericPrimitive)
                    && supply == AdmittedBoundarySupply::HostImport)
            {
                return Err(rejected(
                    "compiler primitive cannot be replaced by an external supply",
                ));
            }
            if supply == AdmittedBoundarySupply::ExternalCompletion {
                if !matches!(
                    declaration.completion,
                    terminal_psi::BoundaryCompletion::External(_)
                ) {
                    return Err(rejected(
                        "selected external completion requires a declared external outcome",
                    ));
                }
            } else if declaration.completion != terminal_psi::BoundaryCompletion::NormalReturn {
                return Err(rejected(
                    "external completion requires separate domain admission",
                ));
            }
            if installation.supplies.contains_key(&boundary)
            {
                return Err(rejected(
                    "external supply duplicates or competes with a selected supply",
                ));
            }
            installation.supplies.insert(boundary, supply);
            Ok(())
        }

        pub fn imports(&self) -> impl Iterator<Item = &BoundaryMachineDeclaration> {
            self.program
                .boundary_machines()
                .values()
                .filter(move |declaration| {
                    declaration.role == terminal_psi::BoundaryDeclarationRole::Callable
                        && !interpreter_owns_boundary_execution(declaration)
                        && !self.program.has_provider(&declaration.id)
                })
        }

        pub fn import(&self, slot: BoundaryMachineId) -> Option<&BoundaryMachineDeclaration> {
            self.program.import_declaration(slot)
        }

        pub fn executes_boundary_internally(&self, slot: BoundaryMachineId) -> bool {
            self.program
                .boundary_machines()
                .get(&slot)
                .is_some_and(interpreter_owns_boundary_execution)
        }

        pub fn external_completion_requirement(
            &self,
            slot: BoundaryMachineId,
        ) -> Option<&BoundaryMachineDeclaration> {
            self.program
                .boundary_machines()
                .get(&slot)
                .filter(|declaration| {
                    declaration.role == terminal_psi::BoundaryDeclarationRole::Callable
                        && matches!(
                            declaration.completion,
                            terminal_psi::BoundaryCompletion::External(_)
                        )
                })
        }

        pub fn instantiate<'host>(
            &self,
            request: TerminalInstantiation<'_>,
        ) -> Result<TerminalInstance<'host, 'artifact>, TerminalArtifactInterpretError> {
            let (execution, requirements) = self.prepare_execution(request, None)?;
            Ok(TerminalInstance::new(
                self.clone(),
                execution,
                requirements.imports,
            ))
        }

        /// [`Self::instantiate`] over a projection prepared by
        /// [`Self::callable_analysis`].
        pub fn instantiate_with_callable_analysis<'host>(
            &self,
            request: TerminalInstantiation<'_>,
            analysis: &crate::engine::capabilities::TerminalCallableAnalysis<'_, '_>,
        ) -> Result<TerminalInstance<'host, 'artifact>, TerminalArtifactInterpretError> {
            let (execution, requirements) = self.prepare_execution(request, Some(analysis))?;
            Ok(TerminalInstance::new(
                self.clone(),
                execution,
                requirements.imports,
            ))
        }

        /// Start the exact compiler invocation admitted for this artifact and target.
        pub fn instantiate_compiler_evaluation<'host>(
            &self,
            mut request: TerminalInstantiation<'_>,
            admitted: &build_model::admission::AdmittedCompilerEvaluation,
            issued_layout: Option<(&layout_plans::LayoutTargetFacts, &[usize])>,
            calls: &[crate::TerminalCallObservation],
        ) -> Result<TerminalInstance<'host, 'artifact>, TerminalArtifactInterpretError> {
            let refusal = || {
                TerminalArtifactInterpretError::Execution(
                    TerminalInterpretError::UnsupportedSemanticVariant(
                        "compiler invocation requires its admitted artifact, entry, installation and target",
                    ),
                )
            };
            let installation = request.installation.ok_or_else(refusal)?;
            let view = match self.program.catalogs.as_ref() {
                ProgramStorage::Borrowed(view) => *view,
                ProgramStorage::Retained(code) => code.view(),
                #[cfg(test)]
                ProgramStorage::Fixture(_) => return Err(refusal()),
            };
            let root = view.module().entry.ok_or_else(refusal)?;
            admitted
                .require_invocation(view, admitted.target(), root)
                .map_err(|_| refusal())?;
            installation
                .require_compiler_evaluation(view, admitted, request.scalar_admission)
                .map_err(|_| refusal())?;
            let issued = if let Some((facts, indices)) = issued_layout {
                if admitted.target() != Some(facts.profile.as_str())
                    || indices.is_empty()
                    || installation
                        .target_layouts
                        .as_ref()
                        .is_some_and(|layouts| layouts.target_facts() != facts)
                {
                    return Err(refusal());
                }
                let entry = self.entry_installed(installation).map_err(|_| refusal())?;
                let mut arguments = Vec::with_capacity(indices.len());
                for &position in indices {
                    let parameter = entry
                        .structural_parameters
                        .get(position)
                        .ok_or_else(refusal)?;
                    let input = request
                        .structural_inputs
                        .arguments
                        .get(position)
                        .ok_or_else(refusal)?;
                    let declaration = self
                        .module()
                        .structural_type(parameter.structural_type)
                        .ok_or_else(refusal)?;
                    if parameter.access != terminal_psi::StructuralAccess::Owned
                        || declaration.shape != terminal_psi::StructuralTypeShape::Opaque
                        || self
                            .module()
                            .identities
                            .text(declaration.declaration_identity)
                            != Some("toolchain::LayoutTarget")
                        || input.structural_type != parameter.structural_type
                        || arguments.contains(input)
                    {
                        return Err(refusal());
                    }
                    arguments.push(input.clone());
                }
                Some(Box::new(crate::layout_target::IssuedLayoutTarget {
                    facts: facts.clone(),
                    arguments,
                }))
            } else {
                None
            };
            request.float_evaluation = crate::float_result_policy::FloatEvaluation::CompileTime;
            let mut instance = self.instantiate(request)?;
            instance
                .install_compiler_observations(issued, calls)
                .map_err(TerminalArtifactInterpretError::Execution)?;
            Ok(instance)
        }

        pub(crate) fn from_admitted(program: AdmittedTerminalProgram<'artifact>) -> Self {
            Self {
                program: std::sync::Arc::new(program),
            }
        }

        pub(crate) fn start_execution(
            &self,
            scalar_arguments: &[TerminalScalarValue],
            structural_inputs: TerminalStructuralInputs<'_>,
            installation: Option<&crate::boundary::effects::TerminalRuntimeSupplies>,
        ) -> Result<
            crate::engine::execution::TerminalExecution<'artifact>,
            TerminalArtifactInterpretError,
        > {
            self.prepare_execution(TerminalInstantiation {
                float_evaluation: Default::default(),
                scalar_arguments,
                structural_inputs,
                installation,
                account: None,
                entry_results: None,
                scalar_application: None,
                scalar_admission: None,
            }, None)
            .map(|(execution, _)| execution)
        }

        fn execution_program(
            &self,
            installation: Option<&crate::boundary::effects::TerminalRuntimeSupplies>,
            scalars: Option<&psi_scalar_applications::AdmittedScalarApplications>,
        ) -> std::sync::Arc<AdmittedTerminalProgram<'artifact>> {
            let layouts = installation.and_then(|supplies| supplies.target_layouts.as_ref());
            if layouts.is_none() && scalars.is_none() {
                return std::sync::Arc::clone(&self.program);
            }
            std::sync::Arc::new(AdmittedTerminalProgram::new(
                self.program.identity.clone(),
                std::sync::Arc::clone(&self.program.catalogs),
                layouts.map(std::sync::Arc::clone),
                scalars.map(|scalars| std::sync::Arc::clone(scalars.geometry())),
            ))
        }

        fn prepare_execution(
            &self,
            request: TerminalInstantiation<'_>,
            analysis: Option<&crate::engine::capabilities::TerminalCallableAnalysis<'_, '_>>,
        ) -> Result<
            (
                crate::engine::execution::TerminalExecution<'artifact>,
                crate::engine::capabilities::TerminalExecutionRequirements,
            ),
            TerminalArtifactInterpretError,
        > {
            let TerminalInstantiation {
                float_evaluation,
                scalar_arguments,
                structural_inputs,
                installation,
                account,
                entry_results,
                scalar_application,
                scalar_admission,
            } = request;
            let scalar_values = match (scalar_application, scalar_admission) {
                (Some(_), Some(_)) => {
                    return Err(TerminalArtifactInterpretError::Execution(
                        TerminalInterpretError::ScalarApplicationEvaluation(
                            "execution cannot mix provisional and completed scalar inputs".into(),
                        ),
                    ));
                }
                (Some(request), None) => Some(
                    self.scalar_application_inputs(request, installation)
                        .map_err(TerminalArtifactInterpretError::Execution)?,
                ),
                (None, Some(admission)) => Some(
                    self.admitted_scalar_inputs(admission, installation)
                        .map_err(TerminalArtifactInterpretError::Execution)?,
                ),
                (None, None) => None,
            };
            if scalar_values.is_none()
                && !self
                    .program
                    .catalogs
                    .scalars.scalar_applications
                    .conditions
                    .is_empty()
            {
                return Err(TerminalArtifactInterpretError::Execution(
                    TerminalInterpretError::ScalarApplicationEvaluation(
                        "ordinary execution requires admission of retained scalar conditions"
                            .into(),
                    ),
                ));
            }
            if let Some(account) = account
                && !matches!(
                    account.measurement(),
                    build_model::contracts::EvaluationMeasurementIdentity::TerminalPsi { .. }
                )
            {
                return Err(TerminalArtifactInterpretError::Execution(TerminalInterpretError::Storage(
                build_model::contracts::EvaluationAccountingError::UnsupportedMeasurementIdentity,
            )));
            }
            self.program
                .validate_layout_selection(installation)
                .map_err(TerminalArtifactInterpretError::Execution)?;
            let program = self.execution_program(installation, scalar_admission);
            let requirements = crate::engine::capabilities::check_with_private_entry_ram(
                &program,
                installation,
                structural_inputs.private_entry_ram,
                scalar_admission.map(|admission| admission.primary()),
                analysis,
            )
            .map_err(TerminalArtifactInterpretError::Execution)?;
            for application in &requirements.scalar_applications {
                if scalar_values
                    .as_ref()
                    .and_then(|values| values.value(*application))
                    .is_none()
                {
                    return Err(TerminalArtifactInterpretError::Execution(
                        TerminalInterpretError::ScalarApplicationEvaluation(format!(
                            "execution requires unevaluated scalar application {}",
                            application.get(),
                        )),
                    ));
                }
            }
            if scalar_application.is_none() {
                for application in &requirements.scalar_entry_conditions {
                    if scalar_values
                        .as_ref()
                        .and_then(|values| values.value(*application))
                        .is_none()
                    {
                        return Err(TerminalArtifactInterpretError::Execution(
                            TerminalInterpretError::ScalarApplicationEvaluation(format!(
                                "execution invokes unchecked scalar entry {}",
                                application.get(),
                            )),
                        ));
                    }
                }
            }
            if let Some(boundary) = requirements.missing_providers.first() {
                return Err(TerminalArtifactInterpretError::Execution(
                    TerminalInterpretError::ProviderInstallationMissing(*boundary),
                ));
            }
            crate::boundary::structural_inputs::initialization::validate(structural_inputs)
                .map_err(TerminalArtifactInterpretError::Execution)?;
            let converted_boolean_fields;
            let scalar_fields = if structural_inputs.boolean_fields.is_empty() {
                structural_inputs.scalar_fields
            } else {
                converted_boolean_fields = structural_inputs
                .boolean_fields
                .iter()
                .map(crate::boundary::structural_inputs::scalar_fields::TerminalStructuralScalarFieldValue::from)
                .collect::<Vec<_>>();
                &converted_boolean_fields
            };
            let mut execution = crate::engine::execution::TerminalExecution::start_with_program(
                program,
                scalar_arguments,
                TerminalStructuralInputs {
                    scalar_fields,
                    ..structural_inputs
                },
                installation,
                account.cloned(),
            )
            .map_err(TerminalArtifactInterpretError::Execution)
            .map_err(|error| match error {
                TerminalArtifactInterpretError::Execution(
                    TerminalInterpretError::StructuralScalarFieldArgumentInvalid {
                        argument_index,
                        field,
                    },
                ) if !structural_inputs.boolean_fields.is_empty() => {
                    TerminalArtifactInterpretError::Execution(
                        TerminalInterpretError::StructuralBooleanFieldArgumentInvalid {
                            argument_index,
                            field,
                        },
                    )
                }
                other => other,
            })?;
            execution.float_evaluation = float_evaluation;
            if let Some(request) = scalar_application {
                execution
                    .request_scalar_entry_checks(request.admission.checks(), entry_results)
                    .map_err(TerminalArtifactInterpretError::Execution)?;
            } else if let Some(request) = entry_results {
                execution
                    .request_entry_results(request)
                    .map_err(TerminalArtifactInterpretError::Execution)?;
            }
            execution.scalar_applications = scalar_values;
            if !structural_inputs.byte_arrays.is_empty() {
                execution
                    .bind_byte_arrays(structural_inputs.byte_arrays)
                    .map_err(TerminalArtifactInterpretError::Execution)?;
            }
            execution
                .retain_host_structural_payloads(structural_inputs)
                .map_err(TerminalArtifactInterpretError::Execution)?;
            execution.external_demands = requirements.external_completions.clone();
            Ok((execution, requirements))
        }
    }

    #[derive(Default)]
    pub struct TerminalInstantiation<'input> {
        pub float_evaluation: crate::float_result_policy::FloatEvaluation,
        pub scalar_arguments: &'input [TerminalScalarValue],
        pub structural_inputs: TerminalStructuralInputs<'input>,
        pub installation: Option<&'input crate::boundary::effects::TerminalRuntimeSupplies>,
        pub account: Option<&'input EvaluationInvocationAccount>,
        pub entry_results: Option<crate::TerminalEntryResultRequest<'input>>,
        pub scalar_application: Option<TerminalScalarApplicationRequest<'input>>,
        pub scalar_admission: Option<&'input psi_scalar_applications::AdmittedScalarApplications>,
    }

    #[derive(Clone, Copy)]
    pub struct TerminalScalarApplicationRequest<'input> {
        pub admission: &'input psi_scalar_applications::AdmittedScalarEntry<'input, 'input>,
    }

    impl TerminalProgram<'_> {
        pub(crate) fn admitted_scalar_inputs(
            &self,
            admission: &psi_scalar_applications::AdmittedScalarApplications,
            installation: Option<&TerminalRuntimeSupplies>,
        ) -> Result<psi_scalar_applications::ScalarApplicationValues, TerminalInterpretError>
        {
            let reject = || {
                TerminalInterpretError::ScalarApplicationEvaluation(
            "completed scalar inputs differ from the exact artifact, target or provider selections".into(),
        )
            };
            let installation = installation.ok_or_else(reject)?;
            let primary = admission.primary();
            let entry = self.program.resolve_entry(Some(installation))?;
            if self
                .module()
                .machine(entry)
                .is_some_and(|row| row.declaration.is_some())
                && !primary.admits_machine(self.module(), entry)
            {
                return Err(reject());
            }
            let expected = TerminalRuntimeSupplies::from(primary.providers());
            if self.program.identity != *admission.execution_identity()
                || installation.target_machines.as_ref() != primary.targets()
                || installation.provider_selections.as_ref() != Some(primary.providers())
                || installation.target_layouts.as_deref() != primary.layouts()
                || expected
                    .supplies
                    .iter()
                    .any(|(boundary, supply)| installation.supplies.get(boundary) != Some(supply))
            {
                return Err(reject());
            }
            Ok(admission.values().clone())
        }

        fn scalar_application_inputs(
            &self,
            request: TerminalScalarApplicationRequest<'_>,
            installation: Option<&TerminalRuntimeSupplies>,
        ) -> Result<psi_scalar_applications::ScalarApplicationValues, TerminalInterpretError>
        {
            let reject = || {
                TerminalInterpretError::UnsupportedSemanticVariant(
                    "scalar evaluation inputs differ from the exact artifact, entry, target or provider selections",
                )
            };
            let observations = request.admission.observations();
            let application = request.admission.application();
            let installation = installation.ok_or_else(reject)?;
            let expected_providers = TerminalRuntimeSupplies::from(observations.providers());
            if self.program.identity != observations.artifact().custody_identity()
                || installation.target_machines.as_ref() != observations.targets()
                || installation.provider_selections.as_ref() != Some(observations.providers())
                || installation.supplies != expected_providers.supplies
                || installation.target_layouts.as_deref() != observations.layouts()
                || observations
                    .entry(application)
                    .map_err(TerminalInterpretError::ScalarApplicationEvaluation)?
                    != self.program.resolve_entry(Some(installation))?
                || observations
                    .value(application)
                    .map_err(TerminalInterpretError::ScalarApplicationEvaluation)?
                    .is_some()
            {
                return Err(reject());
            }
            Ok(observations.values())
        }
    }

    pub(crate) fn interpreter_owns_boundary_execution(
        declaration: &BoundaryMachineDeclaration,
    ) -> bool {
        declaration.declaration.as_ref().is_some_and(|callable| {
            !matches!(
                callable.execution,
                terminal_psi::CheckedBoundaryExecution::Provider
                    | terminal_psi::CheckedBoundaryExecution::NumericPrimitive
            )
        })
    }

    #[cfg(test)]
    mod host_trust_tests {
        use super::*;

        #[test]
        fn received_artifacts_execute_only_with_host_trust_in_their_exact_bytes() {
            let mut module = TerminalModule::declarations();
            let mut identities = terminal_psi::TerminalIdentityTableBuilder::default();
            let declaration = identities.intern("envelope_owned_declaration");
            module.identities = identities.build();
            let closed = terminal_codec::ClosedTerminalModule::new(module).unwrap();
            let identity = closed.identity();
            let record = terminal_codec::PsiOptimizationExecutionRecord::new(
                Default::default(),
                identity,
                identity,
            )
            .unwrap();
            let artifact =
                terminal_codec::CanonicalTerminalArtifact::from_closed_parts(closed, record, None)
                    .unwrap();
            // The producer may reuse its own immutable output.
            assert!(TerminalProgram::borrow(artifact.view()).is_ok());
            assert!(TerminalProgram::retain(artifact.retain_code()).is_ok());

            // The same bytes received from outside the process stay
            // inspectable, but neither the decoded view nor retained code
            // executes without the host's decision.
            let received = std::sync::Arc::new(
                terminal_codec::CanonicalTerminalArtifact::from_bytes(&artifact.to_bytes())
                    .unwrap(),
            );
            assert_eq!(received.view().identity(), identity);
            assert!(matches!(
                TerminalProgram::borrow(received.view()),
                Err(psi_authorization::ArtifactTrustError::ProducerTrustRequired)
            ));
            assert!(matches!(
                TerminalProgram::retain(received.retain_code()),
                Err(psi_authorization::ArtifactTrustError::ProducerTrustRequired)
            ));
            // Rebuilding the artifact from the received closed module keeps
            // the received marker: identity fields cannot launder it.
            let rebuilt = terminal_codec::CanonicalTerminalArtifact::from_closed_parts(
                received.closed_module().clone(),
                received.optimization().clone(),
                None,
            )
            .unwrap();
            assert!(TerminalProgram::borrow(rebuilt.view()).is_err());
            assert!(matches!(
                psi_authorization::authorize_terminal_artifact(
                    std::sync::Arc::clone(&received),
                    None
                ),
                Err(psi_authorization::ArtifactTrustError::ProducerTrustRequired)
            ));

            let host = psi_authorization::HostProducerTrust::accept("embedding test host");
            let authorized =
                psi_authorization::authorize_terminal_artifact(received, Some(&host)).unwrap();
            let program = TerminalProgram::admit(&authorized);
            assert_eq!(program.terminal_psi(), identity);
            assert_eq!(program.closed_module().unwrap().identity(), identity);
            assert_eq!(
                program.module().identities.text(declaration),
                Some("envelope_owned_declaration")
            );
        }
    }

    #[cfg(test)]
    mod prepared_membership_tests {
        use super::*;

        #[test]
        fn an_admitted_program_prepares_its_declaration_plans_once() {
            let bytes = std::fs::read(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../../tests/fixtures/calls/run/scalar_contract_route_exit/linux_x86_64.psi"
            ))
            .unwrap();
            let received = std::sync::Arc::new(
                terminal_codec::CanonicalTerminalArtifact::from_bytes(&bytes).unwrap(),
            );
            let host = psi_authorization::HostProducerTrust::accept("embedding test host");
            let authorized =
                psi_authorization::authorize_terminal_artifact(received, Some(&host)).unwrap();
            let program = TerminalProgram::admit(&authorized);
            let admitted = &program.program;
            let std::borrow::Cow::Borrowed(first) = admitted.intrinsic_memberships() else {
                panic!("an admitted program keeps the plan it prepared");
            };
            let std::borrow::Cow::Borrowed(second) = admitted.intrinsic_memberships() else {
                panic!("an admitted program keeps the plan it prepared");
            };
            assert!(std::ptr::eq(first, second));
            assert_eq!(
                first,
                &terminal_semantics::IntrinsicMemberships::prepare(
                    admitted.structural_types().values(),
                    admitted.catalogs.types.structural_domains.iter(),
                )
            );
            let std::borrow::Cow::Borrowed(roster) = admitted.claim_roster().unwrap() else {
                panic!("an admitted program keeps the claim roster it prepared");
            };
            let std::borrow::Cow::Borrowed(again) = admitted.claim_roster().unwrap() else {
                panic!("an admitted program keeps the claim roster it prepared");
            };
            assert!(std::ptr::eq(roster, again));
            assert_eq!(
                Ok(roster),
                terminal_semantics::owned_structure::ClaimRoster::prepare(
                    admitted.structural_types().values()
                )
                .as_ref()
            );
            // An execution with no other layout or geometry reuses the same
            // prepared program rather than preparing again.
            assert!(std::sync::Arc::ptr_eq(
                &program.execution_program(None, None),
                &program.program
            ));
        }
    }

    #[cfg(test)]
    mod instance_containment_tests {
        use super::*;
        use crate::{TerminalExecutionStatus, TerminalInstanceCloseError, TerminalInstanceError};

        fn instantiate<'artifact>(
            authorized: &'artifact psi_authorization::AuthorizedTerminalArtifact,
        ) -> TerminalInstance<'static, 'artifact> {
            TerminalProgram::admit(authorized)
                .instantiate(TerminalInstantiation {
                    float_evaluation: Default::default(),
                    scalar_arguments: &[],
                    structural_inputs: Default::default(),
                    installation: None,
                    account: None,
                    entry_results: None,
                    scalar_application: None,
                    scalar_admission: None,
                })
                .unwrap()
        }

        // A work-budget pause keeps the activation and its custody: the
        // instance cannot be closed out from under it, and resuming
        // continues it rather than starting over. Once it settles, closing
        // ends every entry, observation and binding.
        #[test]
        fn a_paused_instance_keeps_its_activation_until_it_settles() {
            let bytes = std::fs::read(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../../tests/fixtures/calls/run/scalar_contract_route_exit/linux_x86_64.psi"
            ))
            .unwrap();
            let received = std::sync::Arc::new(
                terminal_codec::CanonicalTerminalArtifact::from_bytes(&bytes).unwrap(),
            );
            let host = psi_authorization::HostProducerTrust::accept("containment test host");
            let authorized =
                psi_authorization::authorize_terminal_artifact(received, Some(&host)).unwrap();

            let mut instance = instantiate(&authorized);
            assert!(matches!(
                instance.invoke(&mut terminal_work::WorkMeter::with_budget(1)),
                Ok(TerminalExecutionStatus::SponsorExhausted(_))
            ));
            assert!(!instance.is_finished());
            assert_eq!(instance.close(), Err(TerminalInstanceCloseError::Busy));
            assert!(!instance.is_closed());
            assert!(matches!(
                instance.invoke(&mut terminal_work::WorkMeter::unbounded()),
                Ok(TerminalExecutionStatus::Complete(_))
            ));
            assert!(instance.is_finished());

            assert_eq!(instance.close(), Ok(()));
            assert!(instance.is_closed());
            assert!(matches!(
                instance.invoke(&mut terminal_work::WorkMeter::unbounded()),
                Err(TerminalInstanceError::Closed)
            ));
            assert!(matches!(
                instance.observe_private_entry_ram(),
                Err(TerminalInstanceError::Closed)
            ));
            assert!(instance.take_returned_provider_storage(&[]).is_err());
            assert!(instance.effects().is_empty());

            // A fresh instance of the same program is independent: closing
            // the first left nothing it could reach.
            let mut fresh = instantiate(&authorized);
            assert!(matches!(
                fresh.invoke(&mut terminal_work::WorkMeter::unbounded()),
                Ok(TerminalExecutionStatus::Complete(_))
            ));
        }
    }

    #[cfg(test)]
    mod private_entry_target_tests {
        use super::*;

        #[test]
        #[ignore = "explicit qualification consumes OMEGA_ENTRY_RAM_CANONICAL_ARTIFACT"]
        fn private_entry_recipe_requires_exact_installed_geometry() {
            let path = std::env::var_os("OMEGA_ENTRY_RAM_CANONICAL_ARTIFACT")
                .expect("qualification must supply actual public CLI canonical output");
            let bytes = std::fs::read(path).expect("read actual canonical bytes");
            let artifact = terminal_codec::CanonicalTerminalArtifact::from_bytes(&bytes)
                .expect("independent canonical admission");
            let host = psi_authorization::HostProducerTrust::accept("qualification host");
            let artifact = psi_authorization::authorize_terminal_artifact(
                std::sync::Arc::new(artifact),
                Some(&host),
            )
            .unwrap();
            let program = TerminalProgram::admit(&artifact);
            let artifact = artifact.artifact();
            let Some(terminal_psi::TargetMachineCallable::Machine(entry)) =
                artifact.semantic_module().entry
            else {
                panic!("ordinary direct entry required")
            };
            let row = artifact
                .semantic_module()
                .access.access_view_inputs
                .iter()
                .find(|row| row.machine == entry)
                .expect("actual direct placed row")
                .clone();
            let machines =
                psi_target_layouts::terminal_target_context(artifact.view(), "linux_x86_64");
            let layouts = psi_target_layouts::admit_target_layouts(
                artifact.view(),
                &machines,
                &psi_target_layouts::LayoutTargetFacts {
                    profile: "linux_x86_64".into(),
                    pointer_size: 8,
                    pointer_alignment: 8,
                    case_tag_size: 4,
                    case_tag_alignment: 4,
                    little_endian: true,
                },
            )
            .unwrap();
            let recipe = psi_entry_memory::admit_private_entry_ram(
                artifact.view(),
                &layouts,
                entry,
                &psi_entry_memory::PrivateEntryRamRecipe {
                    input: row,
                    contents: vec![41, 0, 0, 0, 3, 0, 0, 0],
                },
            )
            .expect("actual checked Linux typed initializer");
            let recipes = std::slice::from_ref(&recipe);
            assert!(
                program
                    .execution_requirements_with_private_entry_ram(None, recipes)
                    .is_err()
            );
            {
                let name = "macos_arm64";
                let other_machines =
                    psi_target_layouts::terminal_target_context(artifact.view(), name);
                let other_layouts = psi_target_layouts::admit_target_layouts(
                    artifact.view(),
                    &other_machines,
                    &psi_target_layouts::LayoutTargetFacts {
                        profile: name.into(),
                        pointer_size: 8,
                        pointer_alignment: 8,
                        case_tag_size: 4,
                        case_tag_alignment: 4,
                        little_endian: true,
                    },
                )
                .unwrap();
                let installation = program
                    .admit_runtime_supplies(
                        crate::boundary::effects::TerminalRuntimeSupplyRequest {
                            native_code_layouts: None,
                            x86_flags: None,
                            memory_ordering: None,
                            entry: artifact.semantic_module().entry,
                            installation: None,
                            host_imports: &[],
                            external_completions: &[],
                            target_machines: Some(&other_machines),
                            target_layouts: Some(&other_layouts),
                        },
                    )
                    .expect("genuine admitted other-target installation");
                assert!(matches!(
                program.execution_requirements_with_private_entry_ram(Some(&installation), recipes),
                Err(crate::boundary::errors::TerminalInterpretError::TargetLayoutSelectionIdentityMismatch)
            ));
            }
        }
    }
}
