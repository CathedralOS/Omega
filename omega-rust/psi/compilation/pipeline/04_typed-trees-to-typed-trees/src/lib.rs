/*
We elaborate and specialize `TypedTrees` before immutable checking freezes them
(`prepare_typed_trees`, `TypedPreparation`, `PreparationRequest`, and
`PreparationAdmission`).
*/

/*
Our elaboration pipeline runs in `preparation.rs` across child-before-parent
constructed result derivation (`constructed_results.rs`), generic machine and
requirement specialization (`monomorphization.rs`,
`body_cloning_and_rewriting.rs`, `contextual_and_requirement_calls.rs`, and
`selection_and_reflection.rs`), conformance lifetime and requirement call
elaboration (`conformance.rs`), data and product operand applications
(`data_and_product_operands.rs`), selected operator call binding and lexical
assembly control normalization (`assembly_and_operators.rs`), suffixed policy
operator typing (`policy_operators.rs`), and assignment destination and
rebound reference parameter preparation.
*/

/*
Stage 04 (`typed-trees-to-typed-trees`) consumes `TypedTrees` from
`symbol-resolved-trees-to-typed-trees` (Stage 03) and produces sealed
`FrozenTypedTrees` plus `TypedPreparation` judgments for
`typed-trees-to-checked-trees` (Stage 05).
*/

#![forbid(unsafe_code)]

mod assembly_and_operators;
mod body_cloning_and_rewriting;
mod conformance;
mod constructed_results;
mod contextual_and_requirement_calls;
mod data_and_product_operands;
mod monomorphization;
mod policy_operators;
mod preparation;
mod selection_and_reflection;

pub(crate) use assembly_and_operators::{assembly_control, operators};
pub use monomorphization::{prepare_requirement_expression_views, restore_copied_call_sources};
pub use preparation::{
    PreparationAdmission, PreparationRequest, PreparedProductRequirement, TypedPreparation,
    bind_pre_specialization_authored_selections, prepare_constant_execution,
    prepare_execution_applications, prepare_product_operand_application,
    prepare_product_requirement_operand, prepare_retained_product_application,
    prepare_retained_product_requirement_operand, prepare_typed_trees,
};

pub(crate) mod assignment_destinations {
    use crate::tables::expressions_of;
    use typed_trees::TypedTrees;
    use typed_trees::statement::StatementNode;
    use typed_trees::types::{FixedArrayLength, TypeReferenceHandle, TypeReferenceNode};
    use validation::array_window_shape;

    enum ReceivingType {
        Declared(TypeReferenceHandle),
        Window {
            element_type: TypeReferenceHandle,
            length: usize,
        },
    }

    pub(crate) fn prepare(program: &mut TypedTrees) {
        let mut destinations = Vec::new();
        let mut bounds = validation::ImmutableBoundLookup::new(program);
        for machine in program.machines() {
            for state in program.machine_states(machine) {
                for handle in state.statement_nodes.handles() {
                    let Some(assignment) =
                        program.statement_table.statement(handle).as_assignment()
                    else {
                        continue;
                    };
                    let assignment = typed_trees::statement::TableAssignment {
                        receiving_type: TypeReferenceHandle::invalid(),
                        ..*assignment
                    };
                    let is_window = expressions_of(program)
                        .indexed_node(assignment.target)
                        .and_then(|indexed| expressions_of(program).range_node(indexed.index))
                        .is_some();
                    let receiving = if is_window {
                        array_window_shape(program, machine, state, assignment.target, &mut bounds)
                            .and_then(|shape| {
                                let length = shape.length()?;
                                shape
                                    .element_type
                                    .is_valid()
                                    .then_some(ReceivingType::Window {
                                        element_type: shape.element_type,
                                        length,
                                    })
                            })
                    } else {
                        validation::assignment_stored_value_type(
                            program,
                            machine,
                            state,
                            &assignment,
                        )
                        .map(ReceivingType::Declared)
                    };
                    destinations.push((handle, receiving));
                }
            }
        }
        for (handle, receiving) in destinations {
            let reference = match receiving {
                Some(ReceivingType::Declared(reference)) => reference,
                Some(ReceivingType::Window {
                    element_type,
                    length,
                }) => {
                    let node = TypeReferenceNode::FixedArray {
                        element_type,
                        length: FixedArrayLength::Literal(length),
                    };
                    let existing = program.type_reference_table.type_reference_rows().find_map(
                        |(reference, candidate)| (candidate == &node).then_some(reference),
                    );
                    existing.unwrap_or_else(|| program.type_reference_table.insert(node))
                }
                None => TypeReferenceHandle::invalid(),
            };
            if let StatementNode::Assignment(assignment) =
                program.statement_table.statement_mut(handle)
            {
                assignment.receiving_type = reference;
            }
        }
    }
}

/*
    A record pattern (`let { a, b as _ } = subject;`) reaches this stage as a
    marker local whose roster holds only what the author spelled. Here, where
    the subject's declared type is settled, each row receives the symbol of the
    field it names in that record and the marker the record's own symbol, so
    checking and later readers decide coverage by identity and never read a
    spelling again. A row naming no field, or one spelling two fields share,
    stays without a symbol; a subject that is not a plain record leaves the
    whole roster unresolved, and checking rejects both.
*/
pub(crate) mod record_patterns {
    use crate::tables::{expressions_of, expressions_of_mut};
    use symbols::SymbolHandle;
    use typed_trees::TypedTrees;
    use typed_trees::statement::StatementNode;

    pub(crate) fn prepare(program: &mut TypedTrees) {
        let mut resolutions = Vec::new();
        for machine in program.machines() {
            for state in program.machine_states(machine) {
                for handle in state.statement_nodes.handles() {
                    let Some(local) = program.statement_table.statement(handle).as_local_data()
                    else {
                        continue;
                    };
                    if local.role != language_core::LocalBindingRole::RecordPattern {
                        continue;
                    }
                    let Ok(data) =
                        validation::record_pattern_subject(program, machine, state, local)
                    else {
                        resolutions.push((handle, SymbolHandle::invalid(), Vec::new()));
                        continue;
                    };
                    let roster = expressions_of(program)
                        .match_fields(local.pattern_fields)
                        .iter()
                        .map(|row| {
                            let mut named = program
                                .data_members(data)
                                .iter()
                                .filter_map(|member| member.as_field())
                                .filter(|field| field.name.as_str() == row.member.as_str());
                            let field_symbol = match (named.next(), named.next()) {
                                (Some(field), None) => field.symbol,
                                _ => SymbolHandle::invalid(),
                            };
                            typed_trees::expression::TableMatchField {
                                field_symbol,
                                ..row.clone()
                            }
                        })
                        .collect::<Vec<_>>();
                    resolutions.push((handle, data.symbol, roster));
                }
            }
        }
        for (handle, subject, roster) in resolutions {
            /*
                Each marker receives rows of its own: a specialized body may
                still share its template's rows, and the two subjects need not
                be the same record.
            */
            let pattern_fields = if subject.is_valid() {
                Some(expressions_of_mut(program).insert_match_fields(roster))
            } else {
                None
            };
            if let StatementNode::LocalData(local) = program.statement_table.statement_mut(handle) {
                local.pattern_subject = subject;
                if let Some(pattern_fields) = pattern_fields {
                    local.pattern_fields = pattern_fields;
                }
            }
        }
    }
}

pub(crate) mod expression_roots {
    use typed_trees::TypedTrees;
    use typed_trees::expression::{ExpressionHandle, ExpressionNode};

    pub(crate) fn append_statement_roots(
        program: &TypedTrees,
        state: &typed_trees::state::State,
        roots: &mut Vec<ExpressionHandle>,
    ) {
        for statement in program.statement_table.statements(state.statement_nodes) {
            validation::collect_statement_expression_trees(program, statement, roots);
        }
    }

    pub(crate) fn visit_post_order(
        program: &TypedTrees,
        roots: impl IntoIterator<Item = ExpressionHandle>,
        mut arm: impl FnMut(ExpressionHandle, &ExpressionNode),
    ) {
        let mut visited = validation::expression_traversal::EnteredExpressions::default();
        for root in roots {
            validation::expression_traversal::visit_post_order(
                program,
                root,
                &mut visited,
                &mut arm,
            );
        }
    }
}

pub(crate) mod rebound_parameter_homes {
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
                let Some(formal) = parameters.iter().find(|formal| formal.symbol == parameter)
                else {
                    continue;
                };
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
            program
                .tables
                .remap_statement_symbols(state.statement_nodes, &homes);
            for (_, home) in &homes {
                let name = typed_trees::name::Identifier::generated(
                    program.symbols.name(*home).to_owned(),
                );
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
}

pub(crate) mod tables {
    use typed_trees::TypedTreeTables;
    use typed_trees::expression::ExpressionTable;

    pub(crate) fn expressions_of(tables: &TypedTreeTables) -> &ExpressionTable {
        &tables.expression_table
    }

    pub(crate) fn expressions_of_mut(tables: &mut TypedTreeTables) -> &mut ExpressionTable {
        &mut tables.expression_table
    }
}
