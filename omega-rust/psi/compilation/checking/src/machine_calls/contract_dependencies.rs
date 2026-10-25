//! Availability of ordinary callee guarantees under one active proof owner.
//!
//! A normal-return guarantee is partial correctness, so this query requires
//! neither termination nor an observation-free call. It prevents a conclusion
//! from proving itself through bodies, contracts, portable family members or
//! retained callback selections. Recursive induction has its own exact-edge
//! premise and strict-descent owner; this ordinary import grants no induction.
//! Declared result carriers remain valid when an optional guarantee is withheld.

use symbols::SymbolHandle;

/// Contract dependency cycles need the independent induction owner. These
/// ordinary value imports cannot prove a callee by citing this caller again.
pub(crate) fn independent_callee(
    program: &dyn crate::CheckingInput,
    caller: SymbolHandle,
    callee: SymbolHandle,
) -> bool {
    independent_application(program, caller, callee, &[])
}

/// Independence belongs to the selected application, including the exact
/// callback machines supplied to its generic telescope. A helper's authored
/// requirement alone does not identify the selected callback's proof sources.
pub(crate) fn independent_application(
    program: &dyn crate::CheckingInput,
    caller: SymbolHandle,
    callee: SymbolHandle,
    selections: &[typed_trees::expression::StaticMachineArgument],
) -> bool {
    let mut pending = vec![callee];
    pending.extend(
        crate::machine_calls::machine_parameters::machine_selection_dependency_symbols(
            program, selections,
        ),
    );
    independent_declarations(program, caller, pending)
}

/// Exact call-owned selection custody, including operands removed from the
/// executable call by specialization. It shares the ordinary dependency walk;
/// no selection or call formation is inferred by this proof-availability query.
pub fn independent_call_application(
    program: &dyn crate::CheckingInput,
    caller: SymbolHandle,
    callee: SymbolHandle,
    call: typed_trees::references::StaticArgumentCall<'_>,
) -> bool {
    let mut pending = vec![callee];
    pending.extend(
        crate::machine_calls::machine_parameters::call_selection_dependency_symbols(program, call),
    );
    independent_declarations(program, caller, pending)
}

/// Every executable call form owns its dependency selection. Named terminal
/// calls have no static operand telescope; their resolved declaration (including
/// a bodyless binder's public contracts) still enters the same dependency walk.
pub fn independent_call_site(
    program: &dyn crate::CheckingInput,
    caller: SymbolHandle,
    callee: SymbolHandle,
    site: &super::calls::CallSite<'_>,
) -> bool {
    match site {
        super::calls::CallSite::Expression { expression, call } => independent_call_application(
            program,
            caller,
            callee,
            typed_trees::references::StaticArgumentCall::Expression(*expression, call),
        ),
        super::calls::CallSite::Statement(call) => independent_call_application(
            program,
            caller,
            callee,
            typed_trees::references::StaticArgumentCall::Statement(call),
        ),
        super::calls::CallSite::TransitionNamed { path, .. } => {
            independent_declarations(program, caller, vec![path.symbol])
        }
    }
}

fn independent_declarations(
    program: &dyn crate::CheckingInput,
    caller: SymbolHandle,
    mut pending: Vec<SymbolHandle>,
) -> bool {
    // Both named states and machine heads share the declaration dependency
    // owner. Unknown scope cannot make an authored guarantee independent.
    if !caller.is_valid() {
        return false;
    }
    let Some(caller) = program
        .machine_by_symbol(caller)
        .map(|machine| machine.symbol)
        .or_else(|| {
            crate::machine_calls::calls::machine_state_by_symbol(program, caller)
                .map(|(machine, _)| machine.symbol)
        })
        .or_else(|| {
            crate::machine_calls::call_cycles::declaration_call_dependencies(program, caller)
                .map(|(owner, _)| owner)
        })
    else {
        return false;
    };
    let mut visited = Vec::new();
    while let Some(declaration) = pending.pop() {
        let Some((owner, dependencies)) =
            crate::machine_calls::call_cycles::declaration_call_dependencies(program, declaration)
        else {
            return false;
        };
        if owner == caller {
            return false;
        }
        if visited.contains(&owner) {
            continue;
        }
        visited.push(owner);
        pending.extend(dependencies);
    }
    true
}
