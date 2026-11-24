//! A registration lends the Build its own mutable configuration. The guest writes
//! the ordinary settings, and normal Build return reads the same cells.
use super::*;
use semantic_vocabulary::{
    CanonicalStructuralPathSegment, ScalarType, StructuralFieldId, StructuralTypeId,
};
use source_compiler::representations::declaration_selection::{
    TestGroupConfiguration, TestInvocationInputs,
};
use terminal_interpreter::{
    TerminalAggregateValue, TerminalInvocationScalar, TerminalReferencePayload,
    TerminalReferenceResult, TerminalReferenceScalarBacking, TerminalScalarBacking,
};

pub(super) struct RegisteredTestGroup {
    selection: ExecutedProductSelection,
    providers: Vec<ExecutedProductSelection>,
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

    pub(super) fn finish(mut self) -> Result<ExecutedBuildSelection, String> {
        let boolean = |cell: &TerminalInvocationScalar| match cell.read().map_err(|e| e.reason)? {
            TerminalScalarValue::Boolean(value) => Ok(value),
            _ => Err(String::from("test group lost its boolean setting")),
        };
        self.selection.test_group = Some(TestGroupConfiguration {
            disabled: boolean(&self.disabled)?,
            inputs: TestInvocationInputs {
                private_filesystem: boolean(&self.private_filesystem)?,
            },
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
                disabled,
                private_filesystem,
                _storage: storage,
            }));
        Ok(result)
    }
}
