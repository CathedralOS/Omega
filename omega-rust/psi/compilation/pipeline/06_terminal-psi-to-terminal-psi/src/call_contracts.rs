//! One public contract source for every lowered invocation. An evaluated
//! callable contributes its catalog contract, never a predicted body.

use semantic_vocabulary::{
    CallableContractId, ContractId, InvocationSite, ObligationId, ProofTerm, Proposition,
    ScalarTerm, ScalarType, ValueId,
};
use terminal_check_tables::edges::{Callee, InvocationRow};
use terminal_check_tables::{Checked, Construct, InvocationId, MachineTables, RefusalReason};
use terminal_psi::{
    BoundaryParameterKind, ContractClause, CrashRouteBucket, ErasedProofFormal, MachineContract,
    StructuralArgument, StructuralParameterDeclaration, TerminalMachine, TerminalModule,
    Terminator, ValueDeclaration,
};

#[derive(Clone, Copy)]
pub(crate) struct Inputs<'a> {
    pub callee: Callee,
    pub contract: Option<CallableContractId>,
    pub required_contract: Option<ContractId>,
    pub arguments: &'a [ValueId],
    pub structural_arguments: &'a [StructuralArgument],
    pub erased_arguments: &'a [ScalarTerm],
    pub erased_proof_arguments: &'a [ProofTerm],
    pub requirements: &'a [ObligationId],
    pub claim_transfers: &'a [terminal_psi::ClaimTransfer],
    pub completion_receipts: &'a [terminal_psi::CompletionReceipt],
    pub crash_continuations: Option<&'a [CrashRouteBucket]>,
}

impl<'a> Inputs<'a> {
    fn declared(
        inputs: terminal_psi::ModuleInvocationInputs<
            'a,
            semantic_vocabulary::TargetMachineCallable,
        >,
    ) -> Self {
        Self {
            callee: Callee::Declared(inputs.callee),
            contract: None,
            required_contract: inputs.required_contract,
            arguments: inputs.arguments,
            structural_arguments: inputs.structural_arguments,
            erased_arguments: inputs.erased_arguments,
            erased_proof_arguments: inputs.erased_proof_arguments,
            requirements: inputs.requirement_obligations,
            claim_transfers: inputs.claim_transfers,
            completion_receipts: inputs.completion_receipts,
            crash_continuations: matches!(
                inputs.callee,
                semantic_vocabulary::TargetMachineCallable::Machine(_)
            )
            .then_some(inputs.crash_continuations),
        }
    }
}

/// Only invocation terminators form calls. A branch has several successor
/// edges, and its edge obligations belong to the ordinary control-flow walk.
pub(crate) fn terminator_site(terminator: &Terminator) -> Option<InvocationSite> {
    match terminator {
        Terminator::InvokeNoNormal { edge, .. } => Some(InvocationSite::InvokeNoNormal(*edge)),
        Terminator::ExternalCompletion { edge, .. } => {
            Some(InvocationSite::ExternalCompletion(*edge))
        }
        Terminator::Jump { .. }
        | Terminator::Conditional { .. }
        | Terminator::StructuralCase { .. }
        | Terminator::Return { .. }
        | Terminator::ReturnUnit { .. }
        | Terminator::ReturnStructural { .. }
        | Terminator::Crash { .. } => None,
    }
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
    pub result: Option<&'a terminal_psi::TerminalMachineResult>,
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
        result: &'a terminal_psi::TerminalMachineResult,
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
            result: Some(result),
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

/// The actual evaluated operands of either a declared or selected call.
pub(crate) fn operation_inputs(operation: &terminal_psi::Operation) -> Option<Inputs<'_>> {
    if let Some(inputs) = operation.declared_invocation() {
        return Some(Inputs::declared(inputs));
    }
    operation.kind.callable_invocation().map(|inputs| Inputs {
        callee: Callee::Selected {
            callable: inputs.callee,
        },
        contract: Some(inputs.contract),
        required_contract: operation.required_contract,
        arguments: inputs.arguments,
        structural_arguments: inputs.structural_arguments,
        erased_arguments: inputs.erased_arguments,
        erased_proof_arguments: inputs.erased_proof_arguments,
        requirements: inputs.requirement_obligations,
        claim_transfers: inputs.claim_transfers,
        completion_receipts: &[],
        crash_continuations: Some(inputs.crash_continuations),
    })
}

/// Find the actual lowered operation, including terminal boundary transfer.
/// Duplicate views of one occurrence are malformed rather than a preference.
pub(crate) fn inputs_at(
    caller: &TerminalMachine,
    site: InvocationSite,
) -> Checked<Option<Inputs<'_>>> {
    let mut inputs = caller.blocks.iter().flat_map(|block| {
        block
            .operations
            .iter()
            .filter_map(|operation| {
                if site != InvocationSite::Operation(operation.id) {
                    return None;
                }
                operation_inputs(operation)
            })
            .chain(
                block
                    .terminator
                    .declared_invocation()
                    .filter(|(candidate, _)| *candidate == site)
                    .map(|(_, inputs)| Inputs::declared(inputs)),
            )
    });
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
    if row.callee != inputs.callee || row.required_contract != inputs.required_contract {
        return Err(refuse(
            "invocation row and lowered call name different targets",
        ));
    }
    let contract = match inputs.callee {
        Callee::Declared(semantic_vocabulary::TargetMachineCallable::Machine(id)) => {
            let contract = module
                .invocation_machine_contract(id, inputs.required_contract)
                .ok_or_else(|| refuse("call has no exact published contract"))?;
            PublicContract::machine(
                contract,
                &contract.structural_parameters,
                &contract.parameters,
                &contract.parameter_order,
                &contract.result,
            )
        }
        Callee::Selected { callable } => {
            if inputs.required_contract.is_some() {
                return Err(refuse(
                    "runtime callable carries a declared implementation contract",
                ));
            }
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
                &contract.proof_result,
            )
        }
        Callee::Declared(semantic_vocabulary::TargetMachineCallable::Boundary(boundary)) => {
            if inputs.required_contract.is_some() {
                return Err(refuse(
                    "boundary call carries a selected implementation contract",
                ));
            }
            let boundary = module
                .boundary_machine(boundary)
                .ok_or_else(|| refuse("boundary invocation names no published declaration"))?;
            if boundary.declaration.is_none()
                || boundary.role != terminal_psi::BoundaryDeclarationRole::Callable
            {
                return Err(refuse("boundary has no exact callable declaration"));
            }
            // External completion transfers control permanently. Its exact
            // declaration and occurrence must agree before any caller reads
            // the published contract.
            match (boundary.completion, row.site) {
                (
                    semantic_vocabulary::BoundaryCompletion::NormalReturn,
                    InvocationSite::Operation(_),
                ) => {}
                (
                    semantic_vocabulary::BoundaryCompletion::External(_),
                    InvocationSite::ExternalCompletion(_),
                ) => {
                    if !boundary.crash_routes.is_empty() {
                        return Err(crate::refusal(
                            RefusalReason::Unsupported,
                            caller.id,
                            concerns,
                            "external completion that declares a crash route",
                        ));
                    }
                }
                _ => {
                    return Err(refuse(
                        "boundary call site does not match its declaration's completion",
                    ));
                }
            }
            // Blocking retains the ordinary stack and loan window. The exact
            // published envelope is still required; suspension needs a separate
            // continuation/carry producer, checked with all call forms below.
            let envelope = module
                .callables
                .boundary_envelope(boundary.id)
                .ok_or_else(|| refuse("boundary has no complete operational envelope"))?;
            if boundary.declaration.as_ref().is_some_and(|declaration| {
                declaration.source.published_may_suspend != envelope.suspends
                    || declaration.source.published_may_block != envelope.blocks
            }) {
                return Err(refuse(
                    "published operational contract differs from its declared or inferred envelope",
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
                result: None,
                crash_routes: &boundary.crash_routes,
                erased_scalar_formals: &[],
                erased_proof_formals: &[],
            }
        }
    };
    Ok(Invocation { inputs, contract })
}
