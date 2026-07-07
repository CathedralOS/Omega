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
        parameter_symbols: Vec<(SymbolHandle, String)>,
        parameter_bounds: Vec<Vec<String>>,
        bindings: Vec<Option<TypeReferenceHandle>>,
        conflicted: bool,
    }

    let mut candidates: Vec<Candidate> = Vec::new();
    // Every state of every generic machine: (state symbol, state name,
    // bare-return parameter position or usize::MAX, candidate index,
    // the state's parameter type handles for param-position inference).
    let mut callee_states: Vec<(SymbolHandle, String, usize, usize, Vec<TypeReferenceHandle>)> =
        Vec::new();
    // All generic type-parameter symbols (to refuse a generic caller
    // forwarding its own parameter as a "concrete" binding).
    let mut all_parameter_symbols: Vec<(SymbolHandle, String)> = Vec::new();

    for (machine_index, machine) in program.machines().iter().enumerate() {
        let parameters = program.machine_type_parameters(machine);
        if parameters.is_empty() {
            continue;
        }
        let parameter_symbols: Vec<(SymbolHandle, String)> = parameters
            .iter()
            .map(|parameter| (parameter.symbol, parameter.name.as_str().to_owned()))
            .collect();
        let parameter_bounds: Vec<Vec<String>> = parameters
            .iter()
            .map(|parameter| {
                omega_validation::declared_property_names(&parameter.bounds)
                    .into_iter()
                    .map(str::to_owned)
                    .collect()
            })
            .collect();
        all_parameter_symbols.extend(parameter_symbols.iter().cloned());
        let candidate_index = candidates.len();
        for state in program.machine_states(machine) {
            let return_parameter = if state.return_type.is_valid() {
                match program
                    .tables
                    .type_reference_table
                    .type_reference(state.return_type)
                {
                    TypeReferenceNode::Named { symbol, name } => parameter_symbols
                        .iter()
                        .position(|(parameter_symbol, parameter_name)| {
                            parameter_symbol == symbol || parameter_name == name.as_str()
                        })
                        .unwrap_or(usize::MAX),
                    _ => usize::MAX,
                }
            } else {
                usize::MAX
            };
            let parameter_types: Vec<TypeReferenceHandle> = program
                .state_parameters(state)
                .iter()
                .map(|parameter| parameter.type_reference)
                .collect();
            callee_states.push((
                state.symbol,
                state.name.as_str().to_owned(),
                return_parameter,
                candidate_index,
                parameter_types,
            ));
        }
        candidates.push(Candidate {
            machine_index,
            parameter_symbols,
            parameter_bounds,
            bindings: vec![None; parameters.len()],
            conflicted: false,
        });
    }
    if candidates.is_empty() {
        return;
    }

    let candidate_parameter_symbols: Vec<Vec<(SymbolHandle, String)>> = candidates
        .iter()
        .map(|candidate| candidate.parameter_symbols.clone())
        .collect();

    // Scan every annotated `let` whose root initializer is a call to a generic
    // machine: RETURN-position inference (P := <let type> when the callee
    // returns bare P) plus PARAM-position inference (P := <declared place
    // type> when a callee parameter is bare P or a reference to P).
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
                    .find(|(symbol, _, _, _, _)| {
                        call.target_symbol.is_valid() && *symbol == call.target_symbol
                    })
                    .or_else(|| {
                        let mut by_name = callee_states
                            .iter()
                            .filter(|(_, name, _, _, _)| name == call.target.as_str());
                        match (by_name.next(), by_name.next()) {
                            (Some(only), None) => Some(only),
                            _ => None,
                        }
                    });
                let Some((_, _, return_parameter, candidate_index, parameter_types)) = resolved
                else {
                    continue;
                };
                // RETURN-position inference: the callee returns the bare
                // parameter, so the annotated let binds it.
                if *return_parameter != usize::MAX {
                    proposals.push((
                        *candidate_index,
                        *return_parameter,
                        local_data.type_reference,
                    ));
                }
                // PARAM-position inference: a callee parameter that IS the bare
                // type parameter (or a reference to it) binds to the declared
                // type of the corresponding PLACE argument.
                let parameter_symbols = &candidate_parameter_symbols[*candidate_index];
                let arguments = program
                    .tables
                    .expression_table
                    .expression_handles(call.arguments);
                // The receiver (`&self`) occupies leading parameter slot(s): align the
                // call arguments with the TRAILING parameters.
                let skip = parameter_types.len().saturating_sub(arguments.len());
                for (argument, parameter_type) in
                    arguments.iter().zip(parameter_types.iter().skip(skip))
                {
                    let parameter_position = match program
                        .tables
                        .type_reference_table
                        .type_reference(*parameter_type)
                    {
                        TypeReferenceNode::Named { symbol, name } => parameter_symbols
                            .iter()
                            .position(|(parameter_symbol, parameter_name)| {
                                parameter_symbol == symbol || parameter_name == name.as_str()
                            }),
                        TypeReferenceNode::Reference { referee, .. } => {
                            match program.tables.type_reference_table.type_reference(*referee) {
                                TypeReferenceNode::Named { symbol, name } => parameter_symbols
                                    .iter()
                                    .position(|(parameter_symbol, parameter_name)| {
                                        parameter_symbol == symbol
                                            || parameter_name == name.as_str()
                                    }),
                                _ => None,
                            }
                        }
                        _ => None,
                    };
                    let Some(parameter_position) = parameter_position else {
                        continue;
                    };
                    let Some(argument_type) = omega_validation::declared_place_type_raw(
                        program,
                        machine,
                        Some(state),
                        *argument,
                    ) else {
                        continue;
                    };
                    proposals.push((*candidate_index, parameter_position, argument_type));
                }
            }
        }
    }

    for (candidate_index, parameter_index, binding) in proposals {
        // A binding that itself names any generic type parameter (a generic
        // caller forwarding its own T) is not a concrete instantiation.
        if let TypeReferenceNode::Named { symbol, name } = program
            .tables
            .type_reference_table
            .type_reference(binding)
            && all_parameter_symbols
                .iter()
                .any(|(parameter_symbol, parameter_name)| {
                    parameter_symbol == symbol || parameter_name == name.as_str()
                })
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

    // Bound check (frozen decision 13) with SHARED borrows before any mutation:
    // substituting a bound-VIOLATING instantiation would erase the type
    // parameters before validation could reject it, so a violating candidate is
    // left generic -- validation then emits the proper bound diagnostic (and the
    // value-call fence). Only bound-SATISFYING candidates are substituted.
    let approved: Vec<bool> = {
        let mut symbol_diagnostics = Vec::new();
        let symbols = omega_validation::TopLevelSymbols::build(program, &mut symbol_diagnostics);
        candidates
            .iter()
            .map(|candidate| {
                candidate
                    .parameter_bounds
                    .iter()
                    .zip(candidate.bindings.iter())
                    .all(|(bounds, binding)| {
                        let Some(binding) = binding else {
                            return true; // unbound candidates are skipped anyway
                        };
                        let Some(unwrapped) =
                            omega_validation::unwrapped_type_reference(program, *binding)
                        else {
                            return false;
                        };
                        bounds.iter().all(|property| {
                            omega_validation::type_satisfies_declared_property(
                                program, &symbols, &[], unwrapped, property,
                            )
                        })
                    })
            })
            .collect()
    };

    // Apply: fully-bound, conflict-free, bound-satisfying machines are
    // substituted and their parameter lists cleared (the validation fence then
    // treats them as concrete). Everything else stays for the fence.
    for (candidate, approved) in candidates.into_iter().zip(approved) {
        if !approved || candidate.conflicted || candidate.bindings.iter().any(Option::is_none) {
            continue;
        }
        for ((parameter_symbol, parameter_name), binding) in candidate
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
                .filter(|(_, symbol, name)| {
                    symbol == parameter_symbol || *name == parameter_name.as_str()
                })
                .map(|(handle, _, _)| handle)
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
