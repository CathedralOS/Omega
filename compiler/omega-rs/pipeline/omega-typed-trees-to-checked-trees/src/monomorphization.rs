//! STAGE-1 machine monomorphization (generic VALUE calls).
//!
//! A generic machine's type parameters have no backend lowering: a value call
//! like `let v: i32 = self.id(70)` (callee `id<T>(x: T) -> T`) compiled but the
//! `T`-typed result slot never materialized -- a silent zero (#40). This pass
//! runs on the TYPED trees BEFORE validation: when every VALUE call of a
//! generic machine agrees on ONE instantiation, the machine's type-parameter
//! references are substituted IN PLACE and its parameter list cleared, so the
//! whole downstream pipeline (validation fence included) treats the machine as
//! concrete and the call materializes like any concrete value call.
//!
//! INFERENCE (stage-1, deliberately narrow): RETURN-position only -- a call
//! that is the root initializer of an annotated `let` binds `P := <let type>`
//! when the callee's return type is the bare parameter `P` (the language has
//! no local inference, so every `let` carries its type). Machines left with
//! unbound parameters keep them, and the existing validation fence rejects
//! their value calls cleanly; a CONFLICTING second instantiation likewise
//! stays fenced. Substitution is a whole-table sweep: a type parameter's
//! symbol is unique to its declaring machine, so replacing every
//! `Named{param}` node is exact.

use omega_core::symbols::SymbolHandle;
use omega_typed_trees::TypedTrees;
use omega_typed_trees::expression::ExpressionNode;
use omega_typed_trees::statement::StatementNode;
use omega_typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

pub(crate) fn monomorphize_generic_machine_value_calls(program: &mut TypedTrees) {
    // Generic machines under consideration.
    struct Candidate {
        machine_index: usize,
        parameter_symbols: Vec<SymbolHandle>,
        bindings: Vec<Option<TypeReferenceHandle>>,
        conflicted: bool,
    }

    let mut candidates: Vec<Candidate> = Vec::new();
    // Every state of every generic machine: (state symbol, state name,
    // bare-return parameter position or usize::MAX, candidate index).
    let mut callee_states: Vec<(SymbolHandle, String, usize, usize)> = Vec::new();
    // All generic type-parameter symbols (to refuse a generic caller
    // forwarding its own parameter as a "concrete" binding).
    let mut all_parameter_symbols: Vec<SymbolHandle> = Vec::new();

    for (machine_index, machine) in program.machines().iter().enumerate() {
        let parameters = program.machine_type_parameters(machine);
        if parameters.is_empty() {
            continue;
        }
        let parameter_symbols: Vec<SymbolHandle> =
            parameters.iter().map(|parameter| parameter.symbol).collect();
        all_parameter_symbols.extend_from_slice(&parameter_symbols);
        let candidate_index = candidates.len();
        for state in program.machine_states(machine) {
            let return_parameter = if state.return_type.is_valid() {
                match program
                    .tables
                    .type_reference_table
                    .type_reference(state.return_type)
                {
                    TypeReferenceNode::Named { symbol, .. } => parameter_symbols
                        .iter()
                        .position(|parameter| parameter == symbol)
                        .unwrap_or(usize::MAX),
                    _ => usize::MAX,
                }
            } else {
                usize::MAX
            };
            callee_states.push((
                state.symbol,
                state.name.as_str().to_owned(),
                return_parameter,
                candidate_index,
            ));
        }
        candidates.push(Candidate {
            machine_index,
            parameter_symbols,
            bindings: vec![None; parameters.len()],
            conflicted: false,
        });
    }
    if candidates.is_empty() {
        return;
    }

    // Scan every annotated `let` whose root initializer is a call to a generic
    // machine's state returning a bare parameter; propose `P := <let type>`.
    let mut proposals: Vec<(usize, usize, TypeReferenceHandle)> = Vec::new();
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            for statement in program
                .tables
                .statement_table
                .statements(state.statement_nodes)
            {
                let StatementNode::LocalData(local_data) = statement else {
                    continue;
                };
                if !local_data.initial_value.is_valid() || !local_data.type_reference.is_valid() {
                    continue;
                }
                let ExpressionNode::Call(call) = program
                    .tables
                    .expression_table
                    .expression(local_data.initial_value)
                else {
                    continue;
                };
                // Resolve the callee among GENERIC machines' states: by symbol
                // when resolved, otherwise by UNIQUE state name (absent or
                // ambiguous names are left for the validation fence).
                let resolved = callee_states
                    .iter()
                    .find(|(symbol, _, _, _)| {
                        call.target_symbol.is_valid() && *symbol == call.target_symbol
                    })
                    .or_else(|| {
                        let mut by_name = callee_states
                            .iter()
                            .filter(|(_, name, _, _)| name == call.target.as_str());
                        match (by_name.next(), by_name.next()) {
                            (Some(only), None) => Some(only),
                            _ => None,
                        }
                    });
                let Some((_, _, return_parameter, candidate_index)) = resolved else {
                    continue;
                };
                if *return_parameter == usize::MAX {
                    continue;
                }
                proposals.push((*candidate_index, *return_parameter, local_data.type_reference));
            }
        }
    }

    for (candidate_index, parameter_index, binding) in proposals {
        // A binding that itself names any generic type parameter (a generic
        // caller forwarding its own T) is not a concrete instantiation.
        if let TypeReferenceNode::Named { symbol, .. } = program
            .tables
            .type_reference_table
            .type_reference(binding)
            && all_parameter_symbols.contains(symbol)
        {
            continue;
        }
        let existing = candidates[candidate_index].bindings[parameter_index];
        match existing {
            None => candidates[candidate_index].bindings[parameter_index] = Some(binding),
            Some(existing) => {
                let existing_display = program
                    .tables
                    .type_reference_table
                    .display_name_with_constraints(existing, &program.tables.expression_table);
                let new_display = program
                    .tables
                    .type_reference_table
                    .display_name_with_constraints(binding, &program.tables.expression_table);
                if existing_display != new_display {
                    candidates[candidate_index].conflicted = true;
                }
            }
        }
    }

    // Apply: fully-bound, conflict-free machines are substituted and their
    // parameter lists cleared (the validation fence then treats them as
    // concrete). Everything else stays for the fence.
    for candidate in candidates {
        if candidate.conflicted || candidate.bindings.iter().any(Option::is_none) {
            continue;
        }
        for (parameter_symbol, binding) in candidate
            .parameter_symbols
            .iter()
            .zip(candidate.bindings.iter())
        {
            let binding = binding.expect("all bindings checked above");
            let replacement = program
                .tables
                .type_reference_table
                .type_reference(binding)
                .clone();
            let occurrences: Vec<TypeReferenceHandle> = program
                .tables
                .type_reference_table
                .named_references()
                .filter(|(_, symbol)| symbol == parameter_symbol)
                .map(|(handle, _)| handle)
                .collect();
            for occurrence in occurrences {
                program
                    .tables
                    .type_reference_table
                    .substitute_node(occurrence, replacement.clone());
            }
        }
        program.machines_mut()[candidate.machine_index].type_parameters =
            omega_core::arena::HandleSpan::empty();
    }
}
