//! Exact installed selected-provider closure review for D45's implemented
//! compiler-intrinsic and normalized-foreign terminal roles.

use std::collections::{BTreeMap, BTreeSet};

use omega_abstract_operations::{AbstractFunction, AbstractOperation, AbstractOperationPlan};
use omega_effects::{
    SelectedProviderPlanFacts, TerminalAuthorityClosureLeaf, TerminalAuthorityClosureReviewReceipt,
    TerminalMechanismIdentity,
    provider_plan::{ProviderBinding, ProviderPlan, ProviderPlanRow},
};
use psi_core::{BoundaryMachineId, MachineId};

use super::{TerminalAuthorityPermissionPolicy, TerminalAuthorityPolicy};
use crate::realization::providers::AdmittedTerminalMechanism;

pub(crate) fn review_terminal_authority_closure(
    terminal_artifact_identity: [u8; 32],
    target: omega_target::NativeTarget,
    plan: &AbstractOperationPlan,
    selected: &SelectedProviderPlanFacts,
    physical_policy: &TerminalAuthorityPolicy,
    permission_policy: &TerminalAuthorityPermissionPolicy,
    mechanisms: &[AdmittedTerminalMechanism],
    installed_candidates: &[psi_terminal::ProviderCandidateConformance],
) -> Result<TerminalAuthorityClosureReviewReceipt, String> {
    let context = ReviewContext::new(
        plan,
        selected,
        physical_policy,
        permission_policy,
        mechanisms,
        installed_candidates,
    )?;
    let mut reviewer = Reviewer {
        context,
        active_requirements: BTreeSet::new(),
        leaves: BTreeMap::new(),
    };
    let root_boundaries = reviewer.context.reachable_boundaries(plan.entry)?;
    for boundary in root_boundaries {
        reviewer.expand_boundary(boundary)?;
    }
    TerminalAuthorityClosureReviewReceipt::from_reviewed_leaves(
        terminal_artifact_identity,
        target,
        selected.identity_digest(),
        physical_policy.identity(),
        permission_policy.identity(),
        reviewer.leaves.into_values().collect(),
    )
    .map_err(|error| format!("terminal-authority review receipt rejected: {error:?}"))
}

type LeafKey = (
    omega_effects::provider_plan::ServiceSchemaDigest,
    String,
    omega_effects::provider_plan::ProviderPlanDigest,
);

struct Reviewer<'a> {
    context: ReviewContext<'a>,
    active_requirements: BTreeSet<String>,
    leaves: BTreeMap<LeafKey, TerminalAuthorityClosureLeaf>,
}

impl Reviewer<'_> {
    fn expand_boundary(&mut self, boundary: BoundaryMachineId) -> Result<(), String> {
        let declaration = self
            .context
            .boundaries
            .get(&boundary)
            .ok_or_else(|| format!("reachable boundary {boundary:?} has no declaration"))?;
        let requirement = declaration.identity.as_str();
        let (selected_plan, selected_row) = self.context.selected_requirement(requirement)?;
        match &selected_row.binding {
            ProviderBinding::CheckedAdapter {
                machine_identity,
                machine_package_identity,
            } => {
                if selected_plan.provider_type.is_empty()
                    || machine_identity.is_empty()
                    || *machine_package_identity != selected_plan.origin_package_identity
                {
                    return Err(format!(
                        "selected checked adapter for `{requirement}` has incomplete or substituted provider custody"
                    ));
                }
                if !self.active_requirements.insert(requirement.to_owned()) {
                    return Err(format!(
                        "selected checked-provider closure contains a cycle through `{requirement}`"
                    ));
                }
                let candidates = self
                    .context
                    .installed_candidates
                    .iter()
                    .filter(|candidate| {
                        candidate.boundary == boundary
                            && candidate.requirement_identity == requirement
                            && candidate.provider_identity == selected_plan.provider_type
                            && candidate.candidate_identity == *machine_identity
                    })
                    .collect::<Vec<_>>();
                let [candidate] = candidates.as_slice() else {
                    self.active_requirements.remove(requirement);
                    return Err(format!(
                        "selected checked adapter for `{requirement}` resolves to {} exact installed candidates",
                        candidates.len()
                    ));
                };
                let nested = self.context.reachable_boundaries(candidate.candidate)?;
                for nested_boundary in nested {
                    self.expand_boundary(nested_boundary)?;
                }
                self.active_requirements.remove(requirement);
            }
            ProviderBinding::CompilerIntrinsic { .. } | ProviderBinding::Import { .. } => {
                let mechanism = self
                    .context
                    .exact_mechanism(boundary, &selected_row.binding)?;
                let exercised = self
                    .context
                    .physical_policy
                    .classify(mechanism)
                    .map_err(|unclassified| {
                        format!(
                            "receiving terminal-authority policy does not classify {:?} required by `{requirement}`",
                            unclassified.mechanism()
                        )
                    })?;
                let schema = selected_plan.schema.identity_digest();
                let permitted = self
                    .context
                    .permission_policy
                    .permission_for(schema, requirement)
                    .map_err(|_| {
                        format!(
                            "receiving terminal-authority permission policy has no exact row for `{requirement}`"
                        )
                    })?;
                let leaf = TerminalAuthorityClosureLeaf::new(
                    schema,
                    requirement.to_owned(),
                    selected_plan.identity_digest(),
                    mechanism,
                    exercised,
                    permitted,
                )
                .map_err(|error| {
                    format!(
                        "terminal mechanism for `{requirement}` exceeds its exact service permission: {error:?}"
                    )
                })?;
                let key = (
                    schema,
                    requirement.to_owned(),
                    selected_plan.identity_digest(),
                );
                if let Some(previous) = self.leaves.insert(key, leaf.clone()) {
                    if previous != leaf {
                        return Err(format!(
                            "selected provider closure substituted the terminal mechanism for `{requirement}`"
                        ));
                    }
                }
            }
            ProviderBinding::StringBackedImportBootstrap { .. } => {
                return Err(format!(
                    "selected requirement `{requirement}` retains a string-backed import with no terminal identity"
                ));
            }
            ProviderBinding::Syscall { .. } => {
                return Err(format!(
                    "selected requirement `{requirement}` uses the unsupported syscall terminal role"
                ));
            }
            ProviderBinding::VtableSlot { .. } => {
                return Err(format!(
                    "selected requirement `{requirement}` uses the unsupported vtable-slot terminal role"
                ));
            }
            ProviderBinding::VtableField { .. } => {
                return Err(format!(
                    "selected requirement `{requirement}` uses the unsupported vtable-field terminal role"
                ));
            }
            ProviderBinding::TableFunction { .. } => {
                return Err(format!(
                    "selected requirement `{requirement}` uses the unsupported table-function terminal role"
                ));
            }
        }
        Ok(())
    }
}

struct ReviewContext<'a> {
    functions: BTreeMap<MachineId, &'a AbstractFunction>,
    boundaries: BTreeMap<BoundaryMachineId, &'a psi_terminal::BoundaryMachineDeclaration>,
    selected: &'a SelectedProviderPlanFacts,
    physical_policy: &'a TerminalAuthorityPolicy,
    permission_policy: &'a TerminalAuthorityPermissionPolicy,
    mechanisms: BTreeMap<BoundaryMachineId, TerminalMechanismIdentity>,
    installed_candidates: &'a [psi_terminal::ProviderCandidateConformance],
}

impl<'a> ReviewContext<'a> {
    fn new(
        plan: &'a AbstractOperationPlan,
        selected: &'a SelectedProviderPlanFacts,
        physical_policy: &'a TerminalAuthorityPolicy,
        permission_policy: &'a TerminalAuthorityPermissionPolicy,
        mechanisms: &[AdmittedTerminalMechanism],
        installed_candidates: &'a [psi_terminal::ProviderCandidateConformance],
    ) -> Result<Self, String> {
        let mut functions = BTreeMap::new();
        for function in &plan.functions {
            if functions.insert(function.machine, function).is_some() {
                return Err(format!(
                    "abstract operation plan repeats machine {:?}",
                    function.machine
                ));
            }
        }
        if !functions.contains_key(&plan.entry) {
            return Err(format!(
                "abstract operation plan entry {:?} has no function",
                plan.entry
            ));
        }
        let mut boundaries = BTreeMap::new();
        let mut boundary_identities = BTreeSet::new();
        for boundary in &plan.boundary_machines {
            if boundary.identity.is_empty()
                || boundaries.insert(boundary.id, boundary).is_some()
                || !boundary_identities.insert(boundary.identity.as_str())
            {
                return Err(
                    "abstract operation plan has duplicate or empty boundary identity".into(),
                );
            }
        }
        let mut mechanism_map = BTreeMap::new();
        for admitted in mechanisms {
            if mechanism_map
                .insert(admitted.boundary, admitted.mechanism)
                .is_some()
            {
                return Err(format!(
                    "native settlement repeats terminal mechanism for boundary {:?}",
                    admitted.boundary
                ));
            }
        }
        Ok(Self {
            functions,
            boundaries,
            selected,
            physical_policy,
            permission_policy,
            mechanisms: mechanism_map,
            installed_candidates,
        })
    }

    fn selected_requirement(
        &self,
        requirement: &str,
    ) -> Result<(&'a ProviderPlan, &'a ProviderPlanRow), String> {
        let matches = self
            .selected
            .plans()
            .iter()
            .flat_map(|plan| {
                plan.rows
                    .iter()
                    .filter(move |row| row.requirement_identity == requirement)
                    .map(move |row| (plan, row))
            })
            .collect::<Vec<_>>();
        let [(plan, row)] = matches.as_slice() else {
            return Err(format!(
                "reachable requirement `{requirement}` resolves to {} selected provider rows",
                matches.len()
            ));
        };
        let methods = plan
            .schema
            .methods
            .iter()
            .filter(|method| method.requirement_identity == requirement)
            .collect::<Vec<_>>();
        if methods.len() != 1 {
            return Err(format!(
                "reachable requirement `{requirement}` resolves to {} exact schema methods",
                methods.len()
            ));
        }
        Ok((plan, row))
    }

    fn exact_mechanism(
        &self,
        boundary: BoundaryMachineId,
        binding: &ProviderBinding,
    ) -> Result<TerminalMechanismIdentity, String> {
        let mechanism = self.mechanisms.get(&boundary).copied().ok_or_else(|| {
            format!("reachable terminal boundary {boundary:?} has no admitted physical mechanism")
        })?;
        let role_matches = matches!(
            (binding, mechanism),
            (
                ProviderBinding::CompilerIntrinsic { .. },
                TerminalMechanismIdentity::CompilerIntrinsic(_)
            ) | (
                ProviderBinding::Import { .. },
                TerminalMechanismIdentity::NormalizedForeign(_)
            )
        );
        if !role_matches {
            return Err(format!(
                "reachable terminal boundary {boundary:?} substituted its selected binding role"
            ));
        }
        Ok(mechanism)
    }

    fn reachable_boundaries(
        &self,
        entry: MachineId,
    ) -> Result<BTreeSet<BoundaryMachineId>, String> {
        let mut pending = vec![entry];
        let mut visited = BTreeSet::new();
        let mut boundaries = BTreeSet::new();
        while let Some(machine) = pending.pop() {
            if !visited.insert(machine) {
                continue;
            }
            let function = self.functions.get(&machine).ok_or_else(|| {
                format!("reachable internal call names absent machine {machine:?}")
            })?;
            for operation in &function.operations {
                match authority_edge(operation) {
                    AuthorityEdge::None => {}
                    AuthorityEdge::Internal(callee) => pending.push(callee),
                    AuthorityEdge::Boundary(boundary) => {
                        boundaries.insert(boundary);
                    }
                    AuthorityEdge::UnsupportedCheckedPhysical => {
                        return Err(format!(
                            "reachable machine {machine:?} uses a checked physical terminal operation unsupported by the current D45 role sum"
                        ));
                    }
                }
            }
        }
        Ok(boundaries)
    }
}

enum AuthorityEdge {
    None,
    Internal(MachineId),
    Boundary(BoundaryMachineId),
    UnsupportedCheckedPhysical,
}

/// Exhaustive match: adding an abstract operation forces an explicit D45
/// decision rather than silently treating a new call/physical role as pure.
fn authority_edge(operation: &AbstractOperation) -> AuthorityEdge {
    match operation {
        AbstractOperation::CallUnit { callee, .. }
        | AbstractOperation::CallStructuralScalar { callee, .. }
        | AbstractOperation::CallStructural { callee, .. }
        | AbstractOperation::Call { callee, .. } => AuthorityEdge::Internal(*callee),
        AbstractOperation::BoundaryCall { boundary, .. } => AuthorityEdge::Boundary(*boundary),
        AbstractOperation::PortWrite { .. } => AuthorityEdge::UnsupportedCheckedPhysical,
        AbstractOperation::WriteOnlyPrimitiveStore { .. }
        | AbstractOperation::StructuralScalarFieldStore { .. }
        | AbstractOperation::EstablishPayloadlessCase { .. }
        | AbstractOperation::EstablishByteSequenceLiteral { .. }
        | AbstractOperation::EstablishTrivialAffineLocal { .. }
        | AbstractOperation::IntegerConstant { .. }
        | AbstractOperation::IeeeFloatConstant { .. }
        | AbstractOperation::NearestIeeeFloatFusedMultiplyAdd { .. }
        | AbstractOperation::BooleanConstant { .. }
        | AbstractOperation::BooleanStructuralField { .. }
        | AbstractOperation::IntegerStructuralField { .. }
        | AbstractOperation::BooleanNot { .. }
        | AbstractOperation::BooleanEqual { .. }
        | AbstractOperation::IntegerEqual { .. }
        | AbstractOperation::IntegerLessThan { .. }
        | AbstractOperation::IntegerLessOrEqual { .. }
        | AbstractOperation::IntegerBitwiseNot { .. }
        | AbstractOperation::IntegerWiden { .. }
        | AbstractOperation::IntegerExactCast { .. }
        | AbstractOperation::IntegerBitwiseAnd { .. }
        | AbstractOperation::IntegerBitwiseOr { .. }
        | AbstractOperation::IntegerBitwiseXor { .. }
        | AbstractOperation::WrappingIntegerShiftLeft { .. }
        | AbstractOperation::WrappingIntegerShiftRight { .. }
        | AbstractOperation::ExactIntegerShiftLeft { .. }
        | AbstractOperation::ExactIntegerShiftRight { .. }
        | AbstractOperation::WrappingIntegerAdd { .. }
        | AbstractOperation::ExactIntegerAdd { .. }
        | AbstractOperation::SaturatingIntegerAdd { .. }
        | AbstractOperation::WrappingIntegerSubtract { .. }
        | AbstractOperation::ExactIntegerSubtract { .. }
        | AbstractOperation::SaturatingIntegerSubtract { .. }
        | AbstractOperation::WrappingIntegerMultiply { .. }
        | AbstractOperation::ExactIntegerMultiply { .. }
        | AbstractOperation::ExactIntegerDivide { .. }
        | AbstractOperation::ExactIntegerRemainder { .. }
        | AbstractOperation::WrappingIntegerDivide { .. }
        | AbstractOperation::WrappingIntegerRemainder { .. }
        | AbstractOperation::SaturatingIntegerDivide { .. }
        | AbstractOperation::SaturatingIntegerRemainder { .. }
        | AbstractOperation::SaturatingIntegerMultiply { .. }
        | AbstractOperation::Jump { .. }
        | AbstractOperation::Conditional { .. }
        | AbstractOperation::Return { .. }
        | AbstractOperation::ReturnUnit { .. }
        | AbstractOperation::ReturnStructural { .. }
        | AbstractOperation::Crash { .. } => AuthorityEdge::None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use omega_effects::{
        CompilerIntrinsicExecutionIdentity, TerminalAuthorityClass, TerminalAuthorityDisposition,
        provider_plan::{ProviderPlanRow, ServiceMethod, ServiceSchema},
    };

    const ADAPTER_REQUIREMENT: &str = "test::Adapter::run()";
    const LEAF_REQUIREMENT: &str = "test::Console::exit()";

    fn service_method(requirement: &str) -> ServiceMethod {
        let (owner, name) = requirement
            .rsplit_once("::")
            .expect("fixture requirement has an owner");
        ServiceMethod {
            name: name.trim_end_matches("()").to_owned(),
            requirement_owner: owner.to_owned(),
            requirement_owner_package_identity: None,
            requirement_identity: requirement.to_owned(),
            parameter_count: 0,
            parameter_type_identities: Vec::new(),
            entry_claims: Vec::new(),
            has_result: false,
            result_type_identity: None,
            result_claims: Vec::new(),
            service_reach: vec![owner.to_owned()],
            synchronous_invocations: Vec::new(),
            may_suspend: false,
            may_block: false,
            terminates_guarantee: false,
            termination_premises: Vec::new(),
            calling_plan_report_fingerprint: None,
            calling_plan_commitment: None,
        }
    }

    fn selected_plan(
        name: &str,
        provider_type: &str,
        requirement: &str,
        binding: ProviderBinding,
    ) -> ProviderPlan {
        let method = service_method(requirement);
        ProviderPlan {
            name: name.to_owned(),
            provider_type: provider_type.to_owned(),
            provider_type_package_identity: None,
            target: "linux_x86_64".to_owned(),
            schema: ServiceSchema {
                trait_name: method.requirement_owner.clone(),
                trait_package_identity: None,
                methods: vec![method.clone()],
            },
            rows: vec![ProviderPlanRow {
                method: method.name,
                requirement_identity: requirement.to_owned(),
                requirement_lifetime_partition: Vec::new(),
                binding,
            }],
            origin_package_identity: None,
            origin_package: "test".to_owned(),
        }
    }

    fn boundary(id: u32, requirement: &str) -> psi_terminal::BoundaryMachineDeclaration {
        psi_terminal::BoundaryMachineDeclaration {
            id: psi_core::BoundaryMachineId::new(u64::from(id)).unwrap(),
            identity: requirement.to_owned(),
            attachment: None,
            scalar_parameters: Vec::new(),
            structural_parameters: Vec::new(),
            result: None,
            requires: Vec::new(),
            program_local_root_introductions: Vec::new(),
            content_guarantees: Vec::new(),
            published_service_ceiling: Vec::new(),
        }
    }

    fn function(machine: u32, boundary_ids: &[u32]) -> AbstractFunction {
        let machine = MachineId::new(u64::from(machine)).unwrap();
        let block = psi_core::BlockId::new(machine.get()).unwrap();
        let mut operations = boundary_ids
            .iter()
            .enumerate()
            .map(|(index, boundary)| AbstractOperation::BoundaryCall {
                psi_operation: psi_core::OperationId::new(
                    machine.get().saturating_mul(10) + index as u64 + 1,
                )
                .unwrap(),
                result: None,
                boundary: BoundaryMachineId::new(u64::from(*boundary)).unwrap(),
                arguments: Vec::new(),
                structural_arguments: Vec::new(),
                completion_claim_sources: Vec::new(),
                completion_receipts: Vec::new(),
            })
            .collect::<Vec<_>>();
        operations.push(AbstractOperation::ReturnUnit {
            psi_edge: psi_core::EdgeId::new(machine.get().saturating_mul(10) + 9).unwrap(),
            cleanup_actions: Vec::new(),
        });
        AbstractFunction {
            machine,
            attachment: None,
            entry: block,
            parameters: Vec::new(),
            structural_parameters: Vec::new(),
            result: omega_abstract_operations::AbstractFunctionResult::Unit,
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            block_entries: vec![omega_abstract_operations::AbstractBlockEntry {
                block,
                parameters: Vec::new(),
                operation_offset: 0,
            }],
            operations,
        }
    }

    fn abstract_plan(
        boundaries: Vec<psi_terminal::BoundaryMachineDeclaration>,
        provider_candidates: Vec<psi_terminal::ProviderCandidateConformance>,
        functions: Vec<AbstractFunction>,
    ) -> AbstractOperationPlan {
        AbstractOperationPlan {
            psi: psi_terminal::TerminalPsiIdentity {
                vocabulary_marker: psi_terminal::VocabularyMarker::CURRENT,
                program_fingerprint: psi_terminal::SemanticFingerprint::from_bytes([41; 32]),
            },
            entry: MachineId::new(1).unwrap(),
            structural_types: Vec::new(),
            boundary_machines: boundaries,
            provider_candidates,
            functions,
        }
    }

    fn installed_candidate(
        boundary: u32,
        requirement: &str,
        provider: &str,
        candidate_identity: &str,
        machine: u32,
    ) -> psi_terminal::ProviderCandidateConformance {
        psi_terminal::ProviderCandidateConformance {
            boundary: BoundaryMachineId::new(u64::from(boundary)).unwrap(),
            requirement_identity: requirement.to_owned(),
            provider_identity: provider.to_owned(),
            candidate_identity: candidate_identity.to_owned(),
            candidate: MachineId::new(u64::from(machine)).unwrap(),
            signature: psi_terminal::ProviderUnitSignature {
                parameters: Vec::new(),
            },
            refinement: psi_terminal::ProviderUnitRefinement {
                positional_parameters: Vec::new(),
                required_domains: Vec::new(),
                realized_service_ceiling: Vec::new(),
            },
        }
    }

    fn permission_policy(
        plans: &[&ProviderPlan],
        permitted: TerminalAuthorityDisposition,
    ) -> TerminalAuthorityPermissionPolicy {
        super::super::terminal_authority_permission_policy::terminal_authority_permission_policy_with_rows(
            plans
                .iter()
                .map(|plan| {
                    super::super::terminal_authority_permission_policy::TerminalAuthorityPermissionPolicyRow::new(
                        plan.schema.identity_digest(),
                        plan.rows[0].requirement_identity.clone(),
                        permitted.clone(),
                    )
                })
                .collect(),
        )
        .expect("exact fixture permissions")
    }

    fn intrinsic_mechanism(boundary: u32) -> AdmittedTerminalMechanism {
        AdmittedTerminalMechanism {
            boundary: BoundaryMachineId::new(u64::from(boundary)).unwrap(),
            mechanism: CompilerIntrinsicExecutionIdentity::LinuxExitGroupI32.into(),
        }
    }

    #[test]
    fn intrinsic_leaf_requires_exact_service_permission() {
        let leaf = selected_plan(
            "leaf",
            "LeafProvider",
            LEAF_REQUIREMENT,
            ProviderBinding::CompilerIntrinsic {
                machine: "test::linux_exit_group_i32".to_owned(),
            },
        );
        let selected = SelectedProviderPlanFacts::from_selected_plans(vec![leaf.clone()])
            .expect("selected leaf");
        let plan = abstract_plan(
            vec![boundary(1, LEAF_REQUIREMENT)],
            Vec::new(),
            vec![function(1, &[1])],
        );
        let physical = super::super::terminal_authority_policy::current_terminal_authority_policy();
        let permitted = permission_policy(
            &[&leaf],
            TerminalAuthorityDisposition::from_classes([
                TerminalAuthorityClass::ProcessTermination,
            ]),
        );
        let receipt = review_terminal_authority_closure(
            [7; 32],
            omega_target::NativeTarget::linux_x64(),
            &plan,
            &selected,
            &physical,
            &permitted,
            &[intrinsic_mechanism(1)],
            &[],
        )
        .expect("contained intrinsic closure");
        assert_eq!(receipt.leaves().len(), 1);
        assert_eq!(receipt.leaves()[0].requirement_identity(), LEAF_REQUIREMENT);
        assert_eq!(
            receipt.leaves()[0].exercised().classes(),
            &[TerminalAuthorityClass::ProcessTermination]
        );

        let denied = permission_policy(&[&leaf], TerminalAuthorityDisposition::from_classes([]));
        assert!(
            review_terminal_authority_closure(
                [7; 32],
                omega_target::NativeTarget::linux_x64(),
                &plan,
                &selected,
                &physical,
                &denied,
                &[intrinsic_mechanism(1)],
                &[],
            )
            .expect_err("exercised class exceeds empty permission")
            .contains("exceeds")
        );
        assert!(
            review_terminal_authority_closure(
                [7; 32],
                omega_target::NativeTarget::linux_x64(),
                &plan,
                &selected,
                &physical,
                &super::super::terminal_authority_permission_policy::current_terminal_authority_permission_policy(),
                &[intrinsic_mechanism(1)],
                &[],
            )
            .expect_err("missing exact permission rejects")
            .contains("no exact row")
        );
    }

    #[test]
    fn checked_adapter_expands_to_selected_terminal_leaf() {
        let adapter = selected_plan(
            "adapter",
            "AdapterProvider",
            ADAPTER_REQUIREMENT,
            ProviderBinding::CheckedAdapter {
                machine_identity: "AdapterProvider::run".to_owned(),
                machine_package_identity: None,
            },
        );
        let leaf = selected_plan(
            "leaf",
            "LeafProvider",
            LEAF_REQUIREMENT,
            ProviderBinding::CompilerIntrinsic {
                machine: "test::linux_exit_group_i32".to_owned(),
            },
        );
        let selected =
            SelectedProviderPlanFacts::from_selected_plans(vec![adapter.clone(), leaf.clone()])
                .expect("selected adapter and leaf");
        let candidate = installed_candidate(
            1,
            ADAPTER_REQUIREMENT,
            "AdapterProvider",
            "AdapterProvider::run",
            2,
        );
        let plan = abstract_plan(
            vec![
                boundary(1, ADAPTER_REQUIREMENT),
                boundary(2, LEAF_REQUIREMENT),
            ],
            vec![candidate.clone()],
            vec![function(1, &[1]), function(2, &[2])],
        );
        let physical = super::super::terminal_authority_policy::current_terminal_authority_policy();
        let permitted = permission_policy(
            &[&leaf],
            TerminalAuthorityDisposition::from_classes([
                TerminalAuthorityClass::ProcessTermination,
            ]),
        );
        let receipt = review_terminal_authority_closure(
            [8; 32],
            omega_target::NativeTarget::linux_x64(),
            &plan,
            &selected,
            &physical,
            &permitted,
            &[intrinsic_mechanism(2)],
            &[candidate],
        )
        .expect("checked adapter closure reaches exact leaf");
        assert_eq!(receipt.leaves().len(), 1);
        assert_eq!(receipt.leaves()[0].requirement_identity(), LEAF_REQUIREMENT);
    }

    #[test]
    fn internal_call_edges_are_part_of_the_reviewed_closure() {
        let leaf = selected_plan(
            "leaf",
            "LeafProvider",
            LEAF_REQUIREMENT,
            ProviderBinding::CompilerIntrinsic {
                machine: "test::linux_exit_group_i32".to_owned(),
            },
        );
        let selected = SelectedProviderPlanFacts::from_selected_plans(vec![leaf.clone()])
            .expect("selected leaf");
        let mut entry = function(1, &[]);
        entry.operations.insert(
            0,
            AbstractOperation::CallUnit {
                psi_operation: psi_core::OperationId::new(1).unwrap(),
                callee: MachineId::new(2).unwrap(),
                structural_arguments: Vec::new(),
                claim_transfers: Vec::new(),
                requirement_obligations: Vec::new(),
                crash_continuations: Vec::new(),
            },
        );
        let plan = abstract_plan(
            vec![boundary(1, LEAF_REQUIREMENT)],
            Vec::new(),
            vec![entry, function(2, &[1])],
        );
        let physical = super::super::terminal_authority_policy::current_terminal_authority_policy();
        let permitted = permission_policy(
            &[&leaf],
            TerminalAuthorityDisposition::from_classes([
                TerminalAuthorityClass::ProcessTermination,
            ]),
        );
        let receipt = review_terminal_authority_closure(
            [11; 32],
            omega_target::NativeTarget::linux_x64(),
            &plan,
            &selected,
            &physical,
            &permitted,
            &[intrinsic_mechanism(1)],
            &[],
        )
        .expect("internal call reaches exact terminal leaf");
        assert_eq!(receipt.leaves().len(), 1);
        assert_eq!(receipt.leaves()[0].requirement_identity(), LEAF_REQUIREMENT);
    }

    #[test]
    fn checked_physical_operations_without_a_role_fail_closed() {
        let mut entry = function(1, &[]);
        entry.operations.insert(
            0,
            AbstractOperation::PortWrite {
                psi_operation: psi_core::OperationId::new(1).unwrap(),
                service: psi_core::ServiceId::new(1).unwrap(),
                port: 0x80,
                value: 0,
            },
        );
        let plan = abstract_plan(Vec::new(), Vec::new(), vec![entry]);
        let selected = SelectedProviderPlanFacts::from_selected_plans(Vec::new())
            .expect("empty selected closure");
        assert!(
            review_terminal_authority_closure(
                [12; 32],
                omega_target::NativeTarget::linux_x64(),
                &plan,
                &selected,
                &super::super::terminal_authority_policy::current_terminal_authority_policy(),
                &super::super::terminal_authority_permission_policy::current_terminal_authority_permission_policy(),
                &[],
                &[],
            )
            .expect_err("unsupported checked physical role rejects")
            .contains("checked physical terminal operation unsupported")
        );
    }

    #[test]
    fn checked_adapter_cycles_and_unsupported_roles_fail_closed() {
        let first = selected_plan(
            "first",
            "FirstProvider",
            ADAPTER_REQUIREMENT,
            ProviderBinding::CheckedAdapter {
                machine_identity: "FirstProvider::run".to_owned(),
                machine_package_identity: None,
            },
        );
        let second_requirement = "test::Second::run()";
        let second = selected_plan(
            "second",
            "SecondProvider",
            second_requirement,
            ProviderBinding::CheckedAdapter {
                machine_identity: "SecondProvider::run".to_owned(),
                machine_package_identity: None,
            },
        );
        let selected = SelectedProviderPlanFacts::from_selected_plans(vec![first, second])
            .expect("selected cycle plans remain ordinary data");
        let first_candidate = installed_candidate(
            1,
            ADAPTER_REQUIREMENT,
            "FirstProvider",
            "FirstProvider::run",
            2,
        );
        let second_candidate = installed_candidate(
            2,
            second_requirement,
            "SecondProvider",
            "SecondProvider::run",
            3,
        );
        let plan = abstract_plan(
            vec![
                boundary(1, ADAPTER_REQUIREMENT),
                boundary(2, second_requirement),
            ],
            vec![first_candidate.clone(), second_candidate.clone()],
            vec![function(1, &[1]), function(2, &[2]), function(3, &[1])],
        );
        assert!(
            review_terminal_authority_closure(
                [9; 32],
                omega_target::NativeTarget::linux_x64(),
                &plan,
                &selected,
                &super::super::terminal_authority_policy::current_terminal_authority_policy(),
                &super::super::terminal_authority_permission_policy::current_terminal_authority_permission_policy(),
                &[],
                &[first_candidate, second_candidate],
            )
            .expect_err("checked-provider cycle rejects")
            .contains("cycle")
        );

        let syscall = selected_plan(
            "syscall",
            "SyscallProvider",
            LEAF_REQUIREMENT,
            ProviderBinding::Syscall { number: 1 },
        );
        let syscall_selected = SelectedProviderPlanFacts::from_selected_plans(vec![syscall])
            .expect("unsupported role remains selectable data");
        let syscall_plan = abstract_plan(
            vec![boundary(1, LEAF_REQUIREMENT)],
            Vec::new(),
            vec![function(1, &[1])],
        );
        assert!(
            review_terminal_authority_closure(
                [10; 32],
                omega_target::NativeTarget::linux_x64(),
                &syscall_plan,
                &syscall_selected,
                &super::super::terminal_authority_policy::current_terminal_authority_policy(),
                &super::super::terminal_authority_permission_policy::current_terminal_authority_permission_policy(),
                &[],
                &[],
            )
            .expect_err("unsupported syscall role rejects")
            .contains("unsupported syscall")
        );
    }
}
