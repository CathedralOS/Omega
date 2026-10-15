//! Compiler-internal package-review seams: rederivation queries that let the
//! orchestration layer reject drifted retained facts without trusting them.
//! These are review joins over the private subsystems, not a public IR
//! contract; handles and private coordinates never enter review identity.

use checked_trees::{CheckFacts, CheckedSemanticDependencies};

use crate::{authored_selections, checks, facts, flow, values};

/// Conservative pre-check classification used by compiler-run semantic
/// evaluation. `true` means the typed expression cannot select an authored
/// operator declaration; final checking still validates builtin semantics.
pub fn typed_operator_has_no_authored_selection(
    program: &dyn ::validation::CheckingInput,
    expression: typed_trees::expression::ExpressionHandle,
) -> bool {
    authored_selections::typed_operator_has_no_authored_selection(program, expression)
}

/// Exact declaration selected by one late-bound member access whose typed
/// `member_symbol` is still invalid, derived from the receiver's owner type
/// exactly as checked binding derives it. Build-time authority confines that
/// declaration's package instead of every same-spelled member in the program.
pub fn late_bound_member_declaration_from_exact_owner(
    program: &dyn ::validation::CheckingInput,
    expression: typed_trees::expression::ExpressionHandle,
) -> Option<symbols::SymbolHandle> {
    authored_selections::exact_owner_member_declaration(program, expression)
}

/// Conservative declaration candidates for an operator before checked
/// selection is final. Build-time authority uses this set only to prove that
/// every possible authored meaning is already within the package's admitted
/// source graph; ordinary checked lowering still chooses the exact meaning.
pub fn typed_operator_authored_selection_candidates(
    program: &dyn ::validation::CheckingInput,
    expression: typed_trees::expression::ExpressionHandle,
) -> Vec<symbols::SymbolHandle> {
    authored_selections::typed_operator_authored_selection_candidates(program, expression)
}

/// Reuse the current checked value traversal without claiming proof validity.
/// Exact expression origins also identify cast-owned constant type positions.
pub fn derive_pre_flow_value_origins(
    program: &dyn ::validation::CheckingInput,
) -> checked_trees::CheckedValueFacts {
    let proof_plan = ::proof::obligations::build_proof_plan(program);
    values::build_value_facts(program, &proof_plan)
}

/// Rederive the complete, canonically ordered checked semantic-dependency
/// table from the final typed program and its checked facts.
///
/// This is the compiler-internal package-review entry point. It does not trust
/// or consult the semantic-dependency table already retained in `facts`.
pub fn derive_checked_semantic_dependencies(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
) -> CheckedSemanticDependencies {
    flow::derive_checked_semantic_dependencies(program, facts)
}

/// Rederive canonical authored crash buckets for an exact typed machine.
/// Scalar lowering metadata and private sites are deliberately outside this
/// read-only package-policy join; canonical predicate equality is unchanged.
pub fn derive_authored_machine_crash_buckets(
    program: &dyn ::validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
) -> Vec<checked_trees::CrashRouteBucket> {
    facts::derive_authored_machine_crash_buckets(program, machine)
}

/// Exact authored crash identity for a retained structural signature, using
/// the same canonical owner as its checked contract capsule.
pub fn derive_authored_signature_crash_buckets(
    program: &dyn ::validation::CheckingInput,
    signature: &typed_trees::signature::StateSignature,
) -> Vec<checked_trees::CrashRouteBucket> {
    facts::derive_authored_signature_crash_buckets(program, signature)
}

/// Query conservative causes from the checker's closed local crash summary.
/// `None` means no complete summary, not a crash-free machine. This read-only
/// query exposes no private guard, site, call, or proof coordinates.
pub fn infer_checked_machine_crash_causes(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    machine: symbols::SymbolHandle,
) -> Option<Vec<checked_trees::CrashCause>> {
    facts::infer_checked_machine_crash_causes(program, facts, machine)
}

/// Query all closed local crash cause summaries with one shared analysis.
/// Missing machine keys mean unknown, not complete empty summaries. Each row
/// retains only its exact machine key and conservative canonical cause set.
pub fn infer_checked_crash_causes(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
) -> Vec<(symbols::SymbolHandle, Vec<checked_trees::CrashCause>)> {
    facts::infer_checked_crash_causes(program, facts)
}

/// Rederive one nominal call target from the final typed program and the exact
/// checked owner environments that contain the expression. Package review uses
/// this compiler-internal seam for proof-owned attached and path-qualified
/// calls whose typed node intentionally carries no direct target symbol.
pub fn derive_checked_nominal_call_target(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    expression: typed_trees::expression::ExpressionHandle,
) -> Option<symbols::SymbolHandle> {
    authored_selections::derive_checked_nominal_call_target(program, facts, expression)
}

/// Rederive the erased requires lane for one exact proof-expression call
/// target from checked contract facts. This is a compiler-internal package
/// review seam; handles remain private joins and never enter review identity.
pub fn derive_checked_contract_expression_evidence_parameters(
    facts: &CheckFacts,
    target_machine_symbol: symbols::SymbolHandle,
    target_state_symbol: symbols::SymbolHandle,
) -> Vec<arena::Handle<checked_trees::CheckedEvidenceTerm>> {
    checks::contracts::exact_target_evidence_parameters(
        facts,
        target_machine_symbol,
        target_state_symbol,
    )
}

/// Freshly instantiate one erased requires proposition against the current
/// typed call arguments. Package review uses this after checking to reject a
/// coordinated typed-tree edit paired with stale checked evidence custody.
pub fn derive_checked_contract_expression_evidence_instantiation(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    expression: typed_trees::expression::ExpressionHandle,
    target_state_symbol: symbols::SymbolHandle,
    parameter: arena::Handle<checked_trees::CheckedEvidenceTerm>,
) -> Option<checked_trees::CheckedPropositionApplication> {
    checks::contracts::instantiate_contract_expression_evidence_parameter(
        program,
        facts,
        expression,
        target_state_symbol,
        parameter,
    )
}

/// Derive the checked body-local termination summary for one typed machine.
///
/// Constant and plan positions must run before checked lowering because their
/// values refine the typed program. This exposes the checker's pure judgment
/// for those admission sites while keeping the proof implementation single-
/// sourced with the facts produced by [`crate::lower_typed_trees`].
pub fn infer_machine_termination_summary(
    program: &dyn ::validation::CheckingInput,
    machine_symbol: symbols::SymbolHandle,
) -> Option<language_semantics::TerminationGuarantee> {
    let machine = program.machine_by_symbol(machine_symbol)?;
    Some(checks::termination::infer_machine_checked_summary(
        program, machine, None,
    ))
}
