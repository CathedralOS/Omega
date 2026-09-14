//! Contract entailment queries and pristine-template prevalidation.

use crate::contract_entailment;
use crate::contract_entailment::validate_machine_contract_entailment;
use crate::finish_diagnostics;
use diagnostics::Diagnostic;
use typed_trees::TypedTrees;

/// Exact source-independent coordinate of a contract-entailment goal that the
/// current proof engines declined to judge. Ordinary compilation may continue
/// because later semantic checks can still constrain the program; package
/// admission must fail closed until an exact later-discharge ledger exists.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContractEntailmentStandDown {
    pub machine_symbol: ::symbols::SymbolHandle,
    pub contract_index: usize,
    pub fact_index: usize,
    pub reason: ContractEntailmentStandDownReason,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ContractEntailmentStandDownReason {
    /// The ensures fact is a membership/proposition form not consumed by this
    /// entailment path.
    UnsupportedEnsuresFact,
    /// A bodied contract lies outside the recognized inductive body shape.
    UnrecognizedInductiveBody,
    /// The goal or a hypothesis needed to judge it is outside the engine's
    /// current language.
    OutsideEntailmentLanguage,
}

impl ContractEntailmentStandDownReason {
    pub const fn label(self) -> &'static str {
        match self {
            Self::UnsupportedEnsuresFact => "unsupported ensures fact",
            Self::UnrecognizedInductiveBody => "unrecognized inductive body",
            Self::OutsideEntailmentLanguage => "outside entailment language",
        }
    }
}

/// Audit every pristine typed machine, including generic templates, for proof
/// claims that ordinary validation deliberately leaves unjudged. Diagnostics
/// are intentionally discarded here: the normal validation path owns compile
/// failure, while this function supplies only successful-compilation admission
/// accounting.
pub fn collect_contract_entailment_stand_downs(
    program: &TypedTrees,
) -> Vec<ContractEntailmentStandDown> {
    let mut stand_downs = Vec::new();
    let mut diagnostics = Vec::new();
    for machine in program.machines() {
        contract_entailment::validate_machine_contract_entailment_with_stand_downs(
            program,
            machine,
            &mut diagnostics,
            &mut stand_downs,
        );
    }
    stand_downs.sort_unstable_by_key(|stand_down| {
        (
            stand_down.machine_symbol.arena_index(),
            stand_down.machine_symbol.generation(),
            stand_down.contract_index,
            stand_down.fact_index,
            stand_down.reason,
        )
    });
    stand_downs.dedup();
    stand_downs
}

/// Rejudge the exact Boolean postconditions proved by the entailment engines
/// for one checked-body machine. This is a local validation result, not a PCC
/// certificate or an admission grant. Callers must already have completed
/// ordinary program validation, including the recursion checks that justify
/// structural induction. Unknown/stand-down goals never enter the result.
/// Judgments may use entry requirements; an exit consumer must separately
/// establish that their subjects have not changed revision. This result does
/// not substitute for checked mutation frames or path-local exit facts.
///
/// Rejudging the current typed program preserves expression identity after
/// specialization; proof from a different template or program is not reused.
pub fn proven_machine_contract_expressions(
    program: &TypedTrees,
    machine_symbol: ::symbols::SymbolHandle,
) -> Vec<typed_trees::expression::ExpressionHandle> {
    let Some(machine) = program.machines().iter().find(|machine| {
        machine.symbol == machine_symbol
            && machine.supply_mode == language_semantics::MachineSupplyMode::CheckedBody
    }) else {
        return Vec::new();
    };
    let mut proven = Vec::new();
    let mut diagnostics = Vec::new();
    contract_entailment::validate_machine_contract_entailment_with_outcomes(
        program,
        machine,
        &mut diagnostics,
        &mut Vec::new(),
        &mut proven,
    );
    if diagnostics.is_empty() && contract_entailment::entailment_covers_all_exits(program, machine)
    {
        proven
    } else {
        Vec::new()
    }
}

/// Recheck one already-resolved checked-body operator realization at a
/// compiler-internal evidence boundary. Callers must still establish the exact
/// selected operator and establish a retained checked baseline separately; this
/// reruns the contract-coverage judgment as the final semantic gate.
pub fn validate_checked_operator_realization_contract(
    program: &TypedTrees,
    machine: &typed_trees::machine::Machine,
    operator: &typed_trees::operator::OperatorDefinition,
) -> Result<(), Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();
    contract_entailment::check_operator_contract_conformance(
        program,
        machine,
        operator,
        &mut diagnostics,
    );
    finish_diagnostics(diagnostics)
}

/// Capture the exact typed contract structure consumed by checked operator
/// conformance. The bytes are compiler-private custody for equality within one
/// checked compilation; they are deliberately not a persisted format or hash.
pub fn checked_operator_contract_snapshot(
    program: &TypedTrees,
    contracts: &[typed_trees::signature::SignatureContract],
) -> Vec<u8> {
    contract_entailment::checked_operator_contract_snapshot(program, contracts)
}

/// Prove machine-generic contracts before specialization. Static selections
/// must already have been checked against their declared callable contracts.
pub fn validate_generic_machine_contract_entailment(
    program: &TypedTrees,
) -> Result<(), Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();
    for machine in program.machines() {
        if program
            .machine_type_parameters(machine)
            .iter()
            .any(|parameter| {
                matches!(
                    parameter.kind,
                    typed_trees::data::TypeParameterKind::Machine { .. }
                )
            })
        {
            validate_machine_contract_entailment(program, machine, &mut diagnostics);
        }
    }
    finish_diagnostics(diagnostics)
}
