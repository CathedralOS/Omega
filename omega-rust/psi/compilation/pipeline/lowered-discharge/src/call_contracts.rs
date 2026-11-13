//! One public contract source for every lowered invocation. An evaluated
//! callable contributes its catalog contract, never a predicted body.

use semantic_vocabulary::{
    CallableContractId, InvocationSite, ObligationId, ProofTerm, Proposition, ScalarTerm,
    ScalarType, ValueId,
};
use terminal_check_tables::edges::{Callee, InvocationRow};
use terminal_check_tables::{Checked, Construct, InvocationId, MachineTables, RefusalReason};
use terminal_psi::{
    BoundaryParameterKind, ContractClause, CrashRouteBucket, ErasedProofFormal, MachineContract,
    StructuralArgument, StructuralParameterDeclaration, TerminalMachine, TerminalModule,
    ValueDeclaration,
};

#[derive(Clone, Copy)]
pub(crate) struct Inputs<'a> {
    pub callee: Callee,
    pub contract: Option<CallableContractId>,
    pub arguments: &'a [ValueId],
    pub structural_arguments: &'a [StructuralArgument],
    pub erased_arguments: &'a [ScalarTerm],
    pub erased_proof_arguments: &'a [ProofTerm],
    pub requirements: &'a [ObligationId],
    pub claim_transfers: &'a [terminal_psi::ClaimTransfer],
    pub completion_receipts: &'a [terminal_psi::CompletionReceipt],
    pub crash_continuations: Option<&'a [CrashRouteBucket]>,
}

/// Guarantees have the same propositions whether their owner also assigns
/// clause obligations (a checked body) or publishes a boundary declaration.
#[derive(Clone, Copy)]
pub(crate) struct Guarantees<'a> {
    clauses: &'a [ContractClause],
    propositions: &'a [Proposition],
}
impl<'a> Guarantees<'a> {
    pub fn clause(self, position: usize) -> Option<&'a ContractClause> {
        self.clauses.get(position)
    }
    pub fn iter(self) -> impl Iterator<Item = &'a Proposition> {
        self.clauses
            .iter()
            .map(|clause| &clause.proposition)
            .chain(self.propositions)
    }
}

pub(crate) enum ScalarParameters<'a> {
    Values(&'a [ValueDeclaration]),
    Types(&'a [ScalarType]),
}
impl ScalarParameters<'_> {
    pub fn len(&self) -> usize {
        match self {
            Self::Values(values) => values.len(),
            Self::Types(types) => types.len(),
        }
    }
    pub fn scalar_type(&self, position: usize) -> Option<ScalarType> {
        match self {
            Self::Values(values) => values.get(position).map(|value| value.scalar_type),
            Self::Types(types) => types.get(position).copied(),
        }
    }
}

pub(crate) struct PublicContract<'a> {
    pub requires: &'a [Proposition],
    pub ensures: Guarantees<'a>,
    pub structural_parameters: &'a [StructuralParameterDeclaration],
    pub scalar_parameters: ScalarParameters<'a>,
    pub parameter_order: &'a [BoundaryParameterKind],
    pub crash_routes: &'a [CrashRouteBucket],
    pub erased_scalar_formals: &'a [ValueDeclaration],
    pub erased_proof_formals: &'a [ErasedProofFormal],
}
impl<'a> PublicContract<'a> {
    fn machine(
        contract: &'a MachineContract,
        structural_parameters: &'a [StructuralParameterDeclaration],
        scalar_parameters: &'a [ValueDeclaration],
        parameter_order: &'a [BoundaryParameterKind],
    ) -> Self {
        Self {
            requires: &contract.requires,
            ensures: Guarantees {
                clauses: &contract.ensures,
                propositions: &[],
            },
            structural_parameters,
            scalar_parameters: ScalarParameters::Values(scalar_parameters),
            parameter_order,
            crash_routes: &contract.crash_routes,
            erased_scalar_formals: &contract.erased_scalar_formals,
            erased_proof_formals: &contract.erased_proof_formals,
        }
    }
}

pub(crate) struct Invocation<'a> {
    pub inputs: Inputs<'a>,
    pub contract: PublicContract<'a>,
}

/// Find the actual lowered operation, including terminal boundary transfer.
/// Duplicate views of one occurrence are malformed rather than a preference.
pub(crate) fn inputs_at(
    caller: &TerminalMachine,
    site: InvocationSite,
) -> Checked<Option<Inputs<'_>>> {
    let mut inputs = caller
        .module_invocations()
        .filter(|call| call.site == site)
        .map(|call| Inputs {
            callee: Callee::Declared(semantic_vocabulary::TargetMachineCallable::Machine(
                call.inputs.callee,
            )),
            contract: None,
            arguments: call.inputs.arguments,
            structural_arguments: call.inputs.structural_arguments,
            erased_arguments: call.inputs.erased_arguments,
            erased_proof_arguments: call.inputs.erased_proof_arguments,
            requirements: call.inputs.requirement_obligations,
            claim_transfers: call.inputs.claim_transfers,
            completion_receipts: call.inputs.completion_receipts,
            crash_continuations: Some(call.inputs.crash_continuations),
        })
        .chain(
            caller
                .callable_invocations()
                .filter(|call| call.site == site)
                .map(|call| Inputs {
                    callee: Callee::Selected {
                        callable: call.inputs.callee,
                    },
                    contract: Some(call.inputs.contract),
                    arguments: call.inputs.arguments,
                    structural_arguments: call.inputs.structural_arguments,
                    erased_arguments: call.inputs.erased_arguments,
                    erased_proof_arguments: call.inputs.erased_proof_arguments,
                    requirements: call.inputs.requirement_obligations,
                    claim_transfers: call.inputs.claim_transfers,
                    completion_receipts: &[],
                    crash_continuations: Some(call.inputs.crash_continuations),
                }),
        )
        .chain(
            caller
                .boundary_invocations()
                .filter(|call| call.site == site)
                .map(|call| Inputs {
                    callee: Callee::Declared(semantic_vocabulary::TargetMachineCallable::Boundary(
                        call.boundary,
                    )),
                    contract: None,
                    arguments: call.arguments,
                    structural_arguments: call.structural_arguments,
                    erased_arguments: &[],
                    erased_proof_arguments: &[],
                    requirements: call.requirement_obligations,
                    claim_transfers: &[],
                    completion_receipts: match call.completion {
                        terminal_psi::control_flow::BoundaryInvocationCompletion::Normal {
                            receipts,
                            ..
                        } => receipts,
                        terminal_psi::control_flow::BoundaryInvocationCompletion::External(_) => {
                            &[]
                        }
                    },
                    crash_continuations: None,
                }),
        );
    let first = inputs.next();
    if inputs.next().is_some() {
        return Err(crate::refusal(
            RefusalReason::ForeignIdentity,
            caller.id,
            Construct::Machine,
            "one invocation site has multiple lowered call targets",
        ));
    }
    Ok(first)
}

pub(crate) fn invocation<'a>(
    module: &'a TerminalModule,
    caller: &'a TerminalMachine,
    tables: &MachineTables,
    id: InvocationId,
    row: &InvocationRow,
) -> Checked<Invocation<'a>> {
    let concerns = Construct::Invocation(id);
    let refuse =
        |detail| crate::refusal(RefusalReason::ForeignIdentity, caller.id, concerns, detail);
    let inputs = inputs_at(caller, row.site)?
        .ok_or_else(|| refuse("invocation row restates no lowered call"))?;
    if row.callee != inputs.callee {
        return Err(refuse(
            "invocation row and lowered call name different targets",
        ));
    }
    let contract = match inputs.callee {
        Callee::Declared(semantic_vocabulary::TargetMachineCallable::Machine(id)) => {
            let selected = crate::terminal_machine(&module.machines, id, caller.id)?;
            PublicContract::machine(
                &selected.contract,
                &selected.structural_parameters,
                &selected.parameters,
                &selected.parameter_order,
            )
        }
        Callee::Selected { callable } => {
            let value = tables
                .values
                .get(callable)
                .ok_or_else(|| refuse("callable operand has no value declaration"))?;
            let ScalarType::Callable(contract) = value.scalar_type else {
                return Err(refuse(
                    "indirect call operand has no callable contract type",
                ));
            };
            if value.rigid.is_some() || inputs.contract != Some(contract) {
                return Err(refuse(
                    "indirect call changes the operand's retained callable contract",
                ));
            }
            let contract = module
                .callables
                .contract(contract)
                .ok_or_else(|| refuse("indirect call names no retained callable contract"))?;
            PublicContract::machine(
                &contract.proof,
                &contract.structural_parameters,
                &contract.scalar_parameters,
                &contract.parameter_order,
            )
        }
        Callee::Declared(semantic_vocabulary::TargetMachineCallable::Boundary(boundary)) => {
            let boundary = module
                .boundary_machine(boundary)
                .ok_or_else(|| refuse("boundary invocation names no published declaration"))?;
            if boundary.declaration.is_none()
                || boundary.role != terminal_psi::BoundaryDeclarationRole::Callable
            {
                return Err(refuse("boundary has no exact callable declaration"));
            }
            if boundary.declaration.as_ref().is_some_and(|declaration| {
                declaration.source.published_may_suspend || declaration.source.published_may_block
            }) {
                return Err(crate::refusal(
                    RefusalReason::Unsupported,
                    caller.id,
                    concerns,
                    "boundary call needs its suspension and blocking proof",
                ));
            }
            if boundary.structural_parameters.iter().any(|formal| {
                !formal.qualifications.is_empty()
                    || !formal.projected_qualifications.is_empty()
                    || !formal.additional_input_qualifications.is_empty()
            }) || matches!(&boundary.result, terminal_psi::BoundaryMachineResult::Structural(result)
                if !result.qualifications.is_empty() || !result.projected_qualifications.is_empty())
            {
                return Err(crate::refusal(
                    RefusalReason::Unsupported,
                    caller.id,
                    concerns,
                    "boundary call needs its structural qualification proof",
                ));
            }
            if !boundary.requires.is_empty()
                || !boundary.normal_qualifications.is_empty()
                || !boundary.scalar_domain_requirements.is_empty()
                || !boundary.reference_outputs.is_empty()
                || !boundary.content_guarantees.is_empty()
                || !boundary.result_reference_regions.is_empty()
                || !boundary.program_local_root_introductions.is_empty()
            {
                return Err(crate::refusal(
                    RefusalReason::Unsupported,
                    caller.id,
                    concerns,
                    "boundary contract needs structural domain, reference or content publication",
                ));
            }
            PublicContract {
                requires: &boundary.scalar_requires,
                ensures: Guarantees {
                    clauses: &[],
                    propositions: &boundary.scalar_ensures,
                },
                structural_parameters: &boundary.structural_parameters,
                scalar_parameters: ScalarParameters::Types(&boundary.scalar_parameters),
                parameter_order: &boundary.parameter_order,
                crash_routes: &boundary.crash_routes,
                erased_scalar_formals: &[],
                erased_proof_formals: &[],
            }
        }
    };
    Ok(Invocation { inputs, contract })
}
