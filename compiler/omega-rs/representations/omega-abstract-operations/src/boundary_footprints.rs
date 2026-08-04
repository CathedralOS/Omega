use omega_calling_conventions::{
    PlanDiagnostic, StateFootprintEvidence, ValidatedBoundaryEntryPlan, compose_state_footprints,
    validate_call_return_mechanics_footprint, validate_outbound_call_footprint,
    validate_runtime_value_guard_footprint, validate_state_footprint,
};

/// Provenance of one independently derived boundary-code footprint fragment.
/// The closed set grows only when the corresponding lowering stage can derive
/// exact evidence from the same target implementation that emits the code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoundaryFootprintFragmentOrigin {
    EntryStorage,
    EntrySliceDescriptor,
    ExitResultRegisters,
    ExitIndirectResultCopy,
    CompilerBodyPlaceCopy,
    CompilerBodyPlaceIntegerWrite,
    CompilerBodyPlaceAddressWrite,
    CompilerBodyConstantHostResult,
    CompilerBodyOutboundSyscall,
    CompilerBodyOutboundSyscallDataArguments,
    CompilerBodyOutboundSyscallResult,
    CompilerBodyOutboundSyscallResultDataArguments,
    CompilerBodyOutboundSyscallResultStorageArguments,
    CompilerBodyOutboundSyscallStorageArguments,
    CompilerBodyStorageBitFieldWrite,
    CompilerBodyPlaceBoundedBufferWrite,
    CompilerBodyPlaceStringWrite,
    CompilerBodyTextAssemblyWrite,
    CompilerBodyPlaceBinaryWrite,
    CompilerBodyStorageConvertWrite,
    CallReturnMechanics,
    DispatchScaffold,
    StaticGuardComparison,
    RuntimeTextGuardComparison,
    PlaceGuardComparison,
    RuntimeValueGuardComparison,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundaryFootprintFragment {
    pub origin: BoundaryFootprintFragmentOrigin,
    pub evidence: StateFootprintEvidence,
}

/// Retained implementation evidence for compiler-owned boundary code. A plan
/// remains explicitly incomplete until body, exit, veneer, thunk, and admitted
/// leaf enumeration are all represented after final placement.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BoundaryFootprintPlan {
    /// Identity of the canonical boundary contract against which every
    /// retained fragment was validated. This references requirement identity;
    /// it does not contribute implementation evidence back into that identity.
    pub boundary_contract_fingerprint: Option<u64>,
    pub fragments: Vec<BoundaryFootprintFragment>,
    pub enumeration_complete: bool,
}

impl BoundaryFootprintPlan {
    /// Validate and retain one fragment under the same canonical boundary as
    /// every fragment already in the plan. Evidence from a different policy or
    /// signature must never be silently composed into this certificate.
    pub fn retain_validated_fragment(
        &mut self,
        boundary: &ValidatedBoundaryEntryPlan,
        fragment: BoundaryFootprintFragment,
    ) -> Result<(), PlanDiagnostic> {
        match fragment.origin {
            BoundaryFootprintFragmentOrigin::CallReturnMechanics => {
                validate_call_return_mechanics_footprint(boundary, &fragment.evidence)?
            }
            BoundaryFootprintFragmentOrigin::CompilerBodyOutboundSyscall
            | BoundaryFootprintFragmentOrigin::CompilerBodyOutboundSyscallDataArguments
            | BoundaryFootprintFragmentOrigin::CompilerBodyOutboundSyscallResult
            | BoundaryFootprintFragmentOrigin::CompilerBodyOutboundSyscallResultDataArguments
            | BoundaryFootprintFragmentOrigin::CompilerBodyOutboundSyscallResultStorageArguments
            | BoundaryFootprintFragmentOrigin::CompilerBodyOutboundSyscallStorageArguments => {
                validate_outbound_call_footprint(boundary, &fragment.evidence)?
            }
            BoundaryFootprintFragmentOrigin::RuntimeValueGuardComparison
            | BoundaryFootprintFragmentOrigin::CompilerBodyPlaceBinaryWrite
            | BoundaryFootprintFragmentOrigin::CompilerBodyStorageConvertWrite => {
                validate_runtime_value_guard_footprint(boundary, &fragment.evidence)?
            }
            _ => validate_state_footprint(boundary, &fragment.evidence)?,
        }
        let fingerprint = boundary.contract_fingerprint();
        match self.boundary_contract_fingerprint {
            Some(retained) if retained != fingerprint => {
                return Err(PlanDiagnostic(
                    "boundary footprint fragments name different validated contracts".into(),
                ));
            }
            Some(_) => {}
            None => self.boundary_contract_fingerprint = Some(fingerprint),
        }
        self.fragments.push(fragment);
        Ok(())
    }

    pub fn composed_evidence(&self) -> StateFootprintEvidence {
        compose_state_footprints(self.fragments.iter().map(|fragment| &fragment.evidence))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use omega_calling_conventions::{MachineRegister, MachineState, MachineStateSet, RegisterSet};

    #[test]
    fn partial_plan_composes_fragment_evidence_without_claiming_completeness() {
        let plan = BoundaryFootprintPlan {
            boundary_contract_fingerprint: Some(0x1234),
            fragments: vec![BoundaryFootprintFragment {
                origin: BoundaryFootprintFragmentOrigin::EntryStorage,
                evidence: StateFootprintEvidence::new(
                    RegisterSet::new([MachineRegister::X86R15]),
                    MachineStateSet::new([MachineState::Flags]),
                ),
            }],
            enumeration_complete: false,
        };

        assert!(!plan.enumeration_complete);
        assert_eq!(plan.boundary_contract_fingerprint, Some(0x1234));
        assert_eq!(
            plan.composed_evidence().registers().as_slice(),
            &[MachineRegister::X86R15]
        );
        assert!(
            plan.composed_evidence()
                .machine_state()
                .contains_all(MachineStateSet::new([MachineState::Flags]))
        );
    }

    #[test]
    fn retained_fragments_cannot_cross_boundary_contracts() {
        use omega_calling_conventions::{
            CallSignature, CallingPolicy, ValueShape, evaluate_ordinary_boundary_entry_plan,
        };

        let first = evaluate_ordinary_boundary_entry_plan(
            CallingPolicy::SystemVAMD64,
            &CallSignature {
                parameters: vec![ValueShape::integer(8, 8)],
                result: None,
            },
        )
        .expect("first boundary");
        let second = evaluate_ordinary_boundary_entry_plan(
            CallingPolicy::SystemVAMD64,
            &CallSignature {
                parameters: vec![ValueShape::integer(4, 4)],
                result: None,
            },
        )
        .expect("second boundary");
        let fragment = || BoundaryFootprintFragment {
            origin: BoundaryFootprintFragmentOrigin::EntryStorage,
            evidence: StateFootprintEvidence::new(
                RegisterSet::new([MachineRegister::X86R15]),
                MachineStateSet::empty(),
            ),
        };
        let mut plan = BoundaryFootprintPlan::default();

        plan.retain_validated_fragment(&first, fragment())
            .expect("first fragment binds the plan");
        let error = plan
            .retain_validated_fragment(&second, fragment())
            .expect_err("a different boundary contract must reject");

        assert!(error.0.contains("different validated contracts"));
        assert_eq!(plan.fragments.len(), 1);
        assert_eq!(
            plan.boundary_contract_fingerprint,
            Some(first.contract_fingerprint())
        );
    }

    #[test]
    fn prescribed_control_state_is_authorized_only_for_call_return_mechanics() {
        use omega_calling_conventions::{
            CallSignature, CallingPolicy, evaluate_ordinary_boundary_entry_plan,
        };

        let boundary = evaluate_ordinary_boundary_entry_plan(
            CallingPolicy::SystemVAMD64,
            &CallSignature {
                parameters: Vec::new(),
                result: None,
            },
        )
        .expect("ordinary boundary");
        let control_evidence = || {
            StateFootprintEvidence::new(
                RegisterSet::new([MachineRegister::X86Rsp]),
                MachineStateSet::new([
                    MachineState::InstructionPointer,
                    MachineState::StackPointer,
                    MachineState::ControlState,
                ]),
            )
        };

        let mut transitive = BoundaryFootprintPlan::default();
        transitive
            .retain_validated_fragment(
                &boundary,
                BoundaryFootprintFragment {
                    origin: BoundaryFootprintFragmentOrigin::EntryStorage,
                    evidence: control_evidence(),
                },
            )
            .expect_err("body-like evidence must not acquire prescribed control authority");

        let mut mechanics = BoundaryFootprintPlan::default();
        mechanics
            .retain_validated_fragment(
                &boundary,
                BoundaryFootprintFragment {
                    origin: BoundaryFootprintFragmentOrigin::CallReturnMechanics,
                    evidence: control_evidence(),
                },
            )
            .expect("call-return mechanics may use their prescribed control state");

        let mut outbound = BoundaryFootprintPlan::default();
        outbound
            .retain_validated_fragment(
                &boundary,
                BoundaryFootprintFragment {
                    origin: BoundaryFootprintFragmentOrigin::CompilerBodyOutboundSyscall,
                    evidence: control_evidence(),
                },
            )
            .expect("outbound calls may use their prescribed control state");

        outbound
            .retain_validated_fragment(
                &boundary,
                BoundaryFootprintFragment {
                    origin:
                        BoundaryFootprintFragmentOrigin::CompilerBodyOutboundSyscallDataArguments,
                    evidence: control_evidence(),
                },
            )
            .expect("data-argument outbound calls may use their prescribed control state");

        outbound
            .retain_validated_fragment(
                &boundary,
                BoundaryFootprintFragment {
                    origin: BoundaryFootprintFragmentOrigin::CompilerBodyOutboundSyscallResult,
                    evidence: control_evidence(),
                },
            )
            .expect("result-bearing outbound calls may use their prescribed control state");

        outbound
            .retain_validated_fragment(
                &boundary,
                BoundaryFootprintFragment {
                    origin: BoundaryFootprintFragmentOrigin::CompilerBodyOutboundSyscallResultDataArguments,
                    evidence: control_evidence(),
                },
            )
            .expect(
                "result-bearing data-argument outbound calls may use prescribed control state",
            );

        outbound
            .retain_validated_fragment(
                &boundary,
                BoundaryFootprintFragment {
                    origin:
                        BoundaryFootprintFragmentOrigin::CompilerBodyOutboundSyscallStorageArguments,
                    evidence: control_evidence(),
                },
            )
            .expect("storage-argument outbound calls may use their prescribed control state");

        outbound
            .retain_validated_fragment(
                &boundary,
                BoundaryFootprintFragment {
                    origin: BoundaryFootprintFragmentOrigin::CompilerBodyOutboundSyscallResultStorageArguments,
                    evidence: control_evidence(),
                },
            )
            .expect(
                "result-bearing storage-argument outbound calls may use prescribed control state",
            );
    }
}
