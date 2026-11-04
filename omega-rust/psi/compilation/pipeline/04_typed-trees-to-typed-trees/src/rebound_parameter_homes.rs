//! Give a rebound reference parameter a local descriptor home.
//!
//! A borrowed formal names its caller's referent everywhere after this stage:
//! borrow tracking, terminal planning and lowering all read a parameter name
//! as that ingress storage. A `mut` shared-reference parameter that the body
//! assigns is instead a descriptor slot the body replaces (`selected = other`).
//! Rather than teach every consumer that a formal may stop meaning its
//! referent, the body's uses move to an ordinary reference local initialized
//! from the formal on entry, exactly as if the source had written
//! `let mut selected = selected;`. The formal then keeps meaning only its
//! ingress referent, and the local is checked and lowered like any other.
//!
//! The classification is validation's binding-replacement rule for the
//! assignment, the same one write frames use. Parameters the body never
//! rebinds are left untouched.

use arena::HandleSpan;
use symbols::{SymbolHandle, SymbolKind};
use typed_trees::TypedTrees;
use typed_trees::statement::{StatementNode, TableLocalData};

use crate::tables::expressions_of_mut;

pub(crate) fn materialize(program: &mut TypedTrees) {
    let mut pending = Vec::new();
    for machine in program.machines() {
        for (state_index, state) in program.machine_states(machine).iter().enumerate() {
            let mut rebound: Vec<SymbolHandle> = Vec::new();
            for statement in program.statement_table.statements(state.statement_nodes) {
                if let Some(parameter) =
                    validation::assignment_rebinds_parameter(program, machine, statement)
                    && !rebound.contains(&parameter)
                {
                    rebound.push(parameter);
                }
            }
            if !rebound.is_empty() {
                pending.push((machine.clone(), state_index, state.clone(), rebound));
            }
        }
    }
    for (machine, state_index, state, rebound) in pending {
        let parameters = program.state_parameters(&state).to_vec();
        let mut homes = Vec::with_capacity(rebound.len());
        let mut entry = Vec::with_capacity(rebound.len());
        for parameter in rebound {
            let Some(formal) = parameters.iter().find(|formal| formal.symbol == parameter) else {
                continue;
            };
            // A distinct generated name: the formal keeps its own.
            let name = format!("__rebound_{}", formal.name.as_str());
            let children = program.symbols.append_generated_children_from(
                state.symbol,
                [(parameter, SymbolKind::Local, name.as_str())],
            );
            let home = symbols::SymbolTableBuilder::child_handles(children)
                .next()
                .expect("one generated parameter home");
            let initial_value = crate::monomorphization::insert_subject_name(
                expressions_of_mut(program),
                formal.name.clone(),
                parameter,
            );
            entry.push(TableLocalData {
                symbol: home,
                name: typed_trees::name::Identifier::generated(name),
                type_reference: formal.type_reference,
                initial_value,
                is_mutable: true,
                ..Default::default()
            });
            homes.push((parameter, home));
        }
        // Every authored use in this body, including the rebinding
        // assignments, now names the home. Entry initializers are added after
        // the remap so they keep reading the formal.
        program
            .tables
            .remap_statement_symbols(state.statement_nodes, &homes);
        for (_, home) in &homes {
            let name =
                typed_trees::name::Identifier::generated(program.symbols.name(*home).to_owned());
            expressions_of_mut(program).respell_single_member_names(*home, &name);
        }
        let statements = program
            .statement_table
            .statements(state.statement_nodes)
            .to_vec();
        let mut body = HandleSpan::empty();
        for local in entry {
            program
                .statement_table
                .push_statement(&mut body, StatementNode::LocalData(local));
        }
        for statement in statements {
            program.statement_table.push_statement(&mut body, statement);
        }
        program.machine_states_mut(&machine)[state_index].statement_nodes = body;
    }
}
