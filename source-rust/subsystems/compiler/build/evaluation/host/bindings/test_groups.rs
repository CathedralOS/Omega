//! A registration lends the Build its own mutable configuration. The guest writes
//! the ordinary field, and normal Build return reads the same cell.
use super::*;
use semantic_vocabulary::{
    CanonicalStructuralPathSegment, ScalarType, StructuralFieldId, StructuralTypeId,
};
use source_compiler::representations::declaration_selection::TestGroupConfiguration;
use terminal_interpreter::{
    TerminalAggregateValue, TerminalInvocationScalar, TerminalReferencePayload,
    TerminalReferenceResult, TerminalReferenceScalarBacking, TerminalScalarBacking,
};

pub(super) struct RegisteredTestGroup {
    selection: ExecutedProductSelection,
    disabled: TerminalInvocationScalar,
    _storage: Option<evaluation_accounting::EvaluationStorageLease>,
}

impl RegisteredTestGroup {
    pub(super) fn finish(mut self) -> Result<ExecutedBuildSelection, String> {
        let TerminalScalarValue::Boolean(disabled) = self.disabled.read().map_err(|e| e.reason)?
        else {
            return Err("test group lost its boolean setting".into());
        };
        self.selection.test_group = Some(TestGroupConfiguration { disabled });
        Ok(ExecutedBuildSelection::Declaration(self.selection))
    }
}

pub(super) struct GroupShape {
    referent: StructuralTypeId,
    disabled: StructuralFieldId,
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
        let [disabled] = module.structural_type(referent)?.shape.record_fields()? else {
            return None;
        };
        (module.identities.text(disabled.identity) == Some("disabled")
            && disabled.field_type
                == terminal_psi::StructuralFieldType::Scalar(ScalarType::Boolean))
        .then_some(Self {
            referent,
            disabled: disabled.id,
        })
    }

    fn issue(
        &self,
        disabled: TerminalInvocationScalar,
    ) -> Result<TerminalEffectResult, TerminalEffectRejection> {
        let path = vec![CanonicalStructuralPathSegment::Field(self.disabled)];
        Ok(TerminalEffectResult::Reference(TerminalReferenceResult {
            referent: TerminalReferencePayload::Resident {
                aggregate: TerminalAggregateValue::from_scalars(
                    self.referent,
                    None,
                    vec![(path.clone(), disabled.read()?)],
                ),
                scalars: vec![TerminalReferenceScalarBacking {
                    path,
                    backing: std::rc::Rc::new(disabled),
                }],
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
            .map(|account| account.reserve_storage(1, 1))
            .transpose()
            .map_err(|error| {
                TerminalEffectRejection::new(format!("test group configuration storage: {error}"))
            })?;
        let disabled = TerminalInvocationScalar::new(TerminalScalarValue::Boolean(false))?;
        let result = shape.issue(disabled.clone())?;
        if let Some(account) = &self.account {
            account.charge_result_custody(1, 1).map_err(|error| {
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
                disabled,
                _storage: storage,
            }));
        Ok(result)
    }
}
