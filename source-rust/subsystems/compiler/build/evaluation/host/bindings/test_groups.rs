//! A registration lends the Build its own mutable configuration. The guest writes
//! the ordinary settings, and normal Build return reads the same cells.
use super::*;
use semantic_vocabulary::{
    CanonicalStructuralPathSegment, ScalarType, StructuralFieldId, StructuralTypeId,
};
use source_compiler::registered_tests::{TestEnginePolicy, TestTargetEngines};
use source_compiler::representations::declaration_selection::TestGroupConfiguration;
use terminal_interpreter::{
    TerminalAggregateValue, TerminalInvocationScalar, TerminalReferencePayload,
    TerminalReferenceResult, TerminalReferenceScalarBacking, TerminalScalarBacking,
};

pub(super) struct RegisteredTestGroup {
    selection: ExecutedProductSelection,
    providers: Vec<ExecutedProductSelection>,
    engines: Vec<(
        TestTargetEngines,
        Option<evaluation_accounting::EvaluationStorageLease>,
    )>,
    pub(super) fixture: super::test_fixtures::TestFixture,
    disabled: TerminalInvocationScalar,
    private_filesystem: TerminalInvocationScalar,
    _storage: Option<evaluation_accounting::EvaluationStorageLease>,
}

impl RegisteredTestGroup {
    pub(super) fn owns_settings(&self, backing: &dyn TerminalScalarBacking) -> bool {
        self.disabled.shares_storage(backing)
    }

    pub(super) fn select_provider(
        &mut self,
        provider: ExecutedProductSelection,
    ) -> Result<(), TerminalEffectRejection> {
        self.providers.try_reserve(1).map_err(|_| {
            TerminalEffectRejection::new("cannot retain test group provider selection")
        })?;
        self.providers.push(provider);
        Ok(())
    }

    pub(super) fn finish(
        mut self,
        account: Option<&evaluation_accounting::EvaluationInvocationAccount>,
    ) -> Result<ExecutedBuildSelection, String> {
        let boolean = |cell: &TerminalInvocationScalar| match cell.read().map_err(|e| e.reason)? {
            TerminalScalarValue::Boolean(value) => Ok(value),
            _ => Err(String::from("test group lost its boolean setting")),
        };
        let (inputs, expected) = self
            .fixture
            .finish(boolean(&self.private_filesystem)?, account)?;
        if let Some(account) = account {
            let rows = self.engines.len() as u64;
            account
                .charge_result_custody(rows, rows * std::mem::size_of::<TestTargetEngines>() as u64)
                .map_err(|e| e.to_string())?;
        }
        self.selection.test_group = Some(TestGroupConfiguration {
            engines: self.engines.into_iter().map(|(row, _lease)| row).collect(),
            expected,
            disabled: boolean(&self.disabled)?,
            inputs,
            providers: self.providers,
        });
        Ok(ExecutedBuildSelection::Declaration(self.selection))
    }
}

pub(super) struct GroupShape {
    referent: StructuralTypeId,
    disabled: StructuralFieldId,
    inputs: StructuralFieldId,
    private_filesystem: StructuralFieldId,
}

impl GroupShape {
    pub(super) fn from_boundary(
        module: &terminal_psi::TerminalModule,
        site: &TerminalBuildBindingCall,
    ) -> Option<Self> {
        let result = &module.boundary_machine(site.site.boundary)?.result;
        let terminal_psi::BoundaryMachineResult::Structural(result) = result else {
            return None;
        };
        let terminal_psi::StructuralTypeShape::Reference {
            referent,
            access: terminal_psi::StructuralAccess::MutableBorrow,
        } = module.structural_type(result.structural_type)?.shape
        else {
            return None;
        };
        let [disabled, inputs] = module.structural_type(referent)?.shape.record_fields()? else {
            return None;
        };
        let input_type = inputs.field_type.as_structural()?;
        let [filesystem] = module.structural_type(input_type)?.shape.record_fields()? else {
            return None;
        };
        let boolean = terminal_psi::StructuralFieldType::Scalar(ScalarType::Boolean);
        (module.identities.text(disabled.identity) == Some("disabled")
            && disabled.field_type == boolean
            && module.identities.text(inputs.identity) == Some("inputs")
            && module.identities.text(filesystem.identity) == Some("private_filesystem")
            && filesystem.field_type == boolean)
            .then_some(Self {
                referent,
                disabled: disabled.id,
                inputs: inputs.id,
                private_filesystem: filesystem.id,
            })
    }

    pub(super) fn referent(&self) -> StructuralTypeId {
        self.referent
    }

    pub(super) fn disabled_path(&self) -> [CanonicalStructuralPathSegment; 1] {
        [CanonicalStructuralPathSegment::Field(self.disabled)]
    }

    fn issue(
        &self,
        disabled: TerminalInvocationScalar,
        private_filesystem: TerminalInvocationScalar,
    ) -> Result<TerminalEffectResult, TerminalEffectRejection> {
        use CanonicalStructuralPathSegment::Field;
        let mut values = Vec::with_capacity(2);
        let mut scalars = Vec::with_capacity(2);
        for (path, cell) in [
            (self.disabled_path().to_vec(), disabled),
            (
                vec![Field(self.inputs), Field(self.private_filesystem)],
                private_filesystem,
            ),
        ] {
            values.push((path.clone(), cell.read()?));
            scalars.push(TerminalReferenceScalarBacking {
                path,
                backing: std::rc::Rc::new(cell),
            });
        }
        Ok(TerminalEffectResult::Reference(TerminalReferenceResult {
            referent: TerminalReferencePayload::Resident {
                aggregate: TerminalAggregateValue::from_scalars(self.referent, None, values),
                scalars,
            },
            references: Vec::new(),
        }))
    }
}

impl TerminalBuildBindingHost {
    pub(super) fn register_test_group(
        &self,
        state: &mut BindingState,
        call: &TerminalBuildBindingCall,
    ) -> Result<TerminalEffectResult, TerminalEffectRejection> {
        let refuse = TerminalEffectRejection::new;
        let BuildSelectionRequest::Declaration(request) = &call.request else {
            return Err(refuse("test group registration lost its declaration"));
        };
        if request.operation != BuildOperation::TestGroupRegistration {
            return Err(refuse(
                "test group registration has another ticket's operation",
            ));
        }
        let selecting_machine = call
            .selecting_machine
            .ok_or_else(|| refuse("test group lost its selecting machine"))?;
        let shape = self
            .group_shape
            .as_ref()
            .ok_or_else(|| refuse("test group lost its declared configuration"))?;
        state
            .executed
            .try_reserve(1)
            .map_err(|_| refuse("cannot retain registered test group"))?;
        let storage = self
            .account
            .as_ref()
            .map(|account| account.reserve_storage(2, 2))
            .transpose()
            .map_err(|error| {
                TerminalEffectRejection::new(format!("test group configuration storage: {error}"))
            })?;
        let disabled = TerminalInvocationScalar::new(TerminalScalarValue::Boolean(false))?;
        let private_filesystem =
            TerminalInvocationScalar::new(TerminalScalarValue::Boolean(false))?;
        let result = shape.issue(disabled.clone(), private_filesystem.clone())?;
        if let Some(account) = &self.account {
            account.charge_result_custody(2, 2).map_err(|error| {
                TerminalEffectRejection::new(format!("test group configuration result: {error}"))
            })?;
        }
        state
            .executed
            .push(ActivationSelection::TestGroup(RegisteredTestGroup {
                selection: ExecutedProductSelection {
                    request: Arc::clone(request),
                    selecting_machine: Some(selecting_machine),
                    static_environment: call.static_environment.clone(),
                    test_group: None,
                    target: None,
                    composition: None,
                },
                providers: Vec::new(),
                engines: Vec::new(),
                fixture: Default::default(),
                disabled,
                private_filesystem,
                _storage: storage,
            }));
        Ok(result)
    }
}

impl TerminalBuildBindingAdapter {
    pub(super) fn configure_test_engine(
        &mut self,
        registration: usize,
        effect: &TerminalEffect,
    ) -> Result<TerminalEffectResult, TerminalEffectRejection> {
        use semantic_vocabulary::{IntegerValue, source_selections::ProductTarget};
        let refuse = TerminalEffectRejection::new;
        let TerminalEffect::BoundaryCall {
            arguments,
            byte_sequence_arguments,
            result,
            ..
        } = effect
        else {
            return Err(refuse("test engine configuration requires a boundary call"));
        };
        let [target, engine, policy] = arguments.as_slice() else {
            return Err(refuse(
                "test engine configuration lost its target, engine or policy",
            ));
        };
        if byte_sequence_arguments.as_slice() != [None]
            || *result != terminal_psi::BoundaryMachineResult::Unit
        {
            return Err(refuse(
                "test engine configuration lost its group or unit result",
            ));
        }
        let number = |value: &TerminalScalarValue| match value {
            TerminalScalarValue::Integer {
                value: IntegerValue::Unsigned(value),
                ..
            } => u64::try_from(*value).map_err(|_| refuse("test engine selector is too large")),
            _ => Err(refuse("test engine selector must be an unsigned integer")),
        };
        let target = ProductTarget::from_code(number(target)?)
            .ok_or_else(|| refuse("unknown test target"))?;
        let policy = match number(policy)? {
            0 => TestEnginePolicy::IfAvailable,
            1 => TestEnginePolicy::Required,
            2 => TestEnginePolicy::Disabled,
            _ => return Err(refuse("unknown test engine policy")),
        };
        let native = match number(engine)? {
            0 => false,
            1 => true,
            _ => return Err(refuse("unknown test engine")),
        };
        let mut state = lock_state(&self.host.state).map_err(refuse)?;
        let Some(ActivationSelection::TestGroup(group)) = state.executed.get_mut(registration)
        else {
            return Err(refuse(
                "test engine configuration lost its registered group",
            ));
        };
        let index = match group
            .engines
            .binary_search_by_key(&target.code(), |(row, _)| row.target.code())
        {
            Ok(index) => index,
            Err(index) => {
                let lease = self
                    .host
                    .account
                    .as_ref()
                    .map(|account| {
                        account.reserve_storage(1, std::mem::size_of::<TestTargetEngines>() as u64)
                    })
                    .transpose()
                    .map_err(|e| TerminalEffectRejection::new(e.to_string()))?;
                group
                    .engines
                    .try_reserve(1)
                    .map_err(|_| refuse("cannot retain test engine configuration"))?;
                group.engines.insert(
                    index,
                    (
                        TestTargetEngines {
                            target,
                            selection: Default::default(),
                        },
                        lease,
                    ),
                );
                index
            }
        };
        let selection = &mut group.engines[index].0.selection;
        if native {
            selection.native = policy;
        } else {
            selection.psi = policy;
        }
        Ok(TerminalEffectResult::Unit)
    }
}
