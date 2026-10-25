//! Normalize scratch-register CFGs to ordinary checked calls and state edges.
//!
//! The initial instruction prefix runs at its original flow point. Its local
//! edge becomes a call whose ordinary preconditions are that edge's label
//! obligations. The helper retains runtime loops, not an unrolled trace. Each
//! label receives fresh register/flag parameters, and external storage is held
//! by one ordinary borrow per exact root. Thus edge proofs, ownership, Terminal
//! execution and native realization use the existing control machinery.
//! A helper call cannot cross an open invariant or invent establishment: its
//! ordinary argument checking remains authoritative. Finite lexical assembly
//! without scratch state retains its existing storage-frame route.
use arena::HandleSpan;
use diagnostics::Diagnostic;
use language_core::inline_assembly::ASM_ZERO_FLAG_SLOT;
use symbols::{SymbolHandle, SymbolKind};
use typed_trees::expression::{
    ExpressionHandle, ExpressionNode, TableBorrowExpression, TableNamePath, TableUnaryExpression,
    UnaryOperator,
};
use typed_trees::signature::{SignatureContract, SignatureContractKind, StateParameter};
use typed_trees::statement::{
    AssemblyControlFlow, AssemblyControlPoint, AssemblyExit, AssemblyFactKind, StatementNode,
    TableCall, TableTransition, TransitionGuardNode, TransitionTargetNode,
};
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};
use typed_trees::{TypedTrees, machine::Machine, name::Identifier, state::State};

#[derive(Clone)]
struct Binding {
    symbol: SymbolHandle,
    name: Identifier,
    parameter_type: TypeReferenceHandle,
    access: Option<language_core::ReferenceAccess>,
}

fn name(program: &mut TypedTrees, symbol: SymbolHandle) -> ExpressionHandle {
    let identifier = Identifier::generated(program.symbols.name(symbol));
    let mut members = HandleSpan::empty();
    program
        .expression_table
        .push_name_path_member(&mut members, identifier.clone());
    program
        .expression_table
        .insert(ExpressionNode::Name(TableNamePath::new(
            members,
            HandleSpan::empty(),
            symbol,
            symbol,
        )))
}

pub(super) fn root(program: &TypedTrees, expression: ExpressionHandle) -> Option<SymbolHandle> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Name(path) => Some(path.head_symbol),
        ExpressionNode::Member(member) => root(program, member.receiver),
        ExpressionNode::Indexed(index) => root(program, index.collection),
        ExpressionNode::Borrow(borrow) => root(program, borrow.target),
        _ => None,
    }
}

fn declaration_type(
    program: &TypedTrees,
    state: &State,
    symbol: SymbolHandle,
) -> Option<TypeReferenceHandle> {
    program
        .state_parameters(state)
        .iter()
        .find(|parameter| parameter.symbol == symbol)
        .map(|parameter| parameter.type_reference)
        .or_else(|| {
            program
                .statement_table
                .statements(state.statement_nodes)
                .iter()
                .find_map(|statement| match statement {
                    StatementNode::LocalData(local) if local.symbol == symbol => {
                        Some(local.type_reference)
                    }
                    _ => None,
                })
        })
}

fn successors(
    program: &TypedTrees,
    graph: &AssemblyControlFlow,
    index: usize,
) -> Result<Vec<usize>, Diagnostic> {
    let blocks = program.statement_table.assembly_blocks(graph.blocks);
    let locate = |target| {
        graph
            .blocks
            .handles()
            .position(|handle| handle == target)
            .ok_or_else(|| Diagnostic::error("assembly edge escaped its lexical graph"))
    };
    Ok(match blocks[index].exit {
        AssemblyExit::Jump(target) => vec![locate(target)?],
        AssemblyExit::ZeroFlag(target, _) => {
            let mut targets = vec![locate(target)?];
            if index + 1 < blocks.len() {
                targets.push(index + 1);
            }
            targets
        }
        AssemblyExit::Fallthrough => {
            if index + 1 < blocks.len() {
                vec![index + 1]
            } else {
                Vec::new()
            }
        }
        AssemblyExit::EnclosingTransition => {
            return Err(Diagnostic::error(
                "scratch assembly with an enclosing-machine exit requires a lexical continuation; an ordinary helper cannot change its destination",
            ));
        }
    })
}

/// Writing a place observes its indices and address-producing expressions,
/// but a plain register destination is not a read of its previous value.
fn target_reads(
    program: &TypedTrees,
    expression: ExpressionHandle,
    reads: &mut Vec<ExpressionHandle>,
) {
    match program.expression_table.expression(expression) {
        ExpressionNode::Name(_) => {}
        ExpressionNode::Member(member) => target_reads(program, member.receiver, reads),
        ExpressionNode::Indexed(indexed) => {
            target_reads(program, indexed.collection, reads);
            validation::expression_traversal::collect_expression_tree(
                program,
                indexed.index,
                reads,
            );
        }
        ExpressionNode::Borrow(borrow) => target_reads(program, borrow.target, reads),
        _ => validation::expression_traversal::collect_expression_tree(program, expression, reads),
    }
}

/// Scalar destination admission and intersection over every incoming edge,
/// including the empty initial entry.
/// Storage zeros never enter this lattice. Assertions are reads, never writes.
fn check_register_flow(
    program: &TypedTrees,
    state: &State,
    graph: &AssemblyControlFlow,
    body_start: usize,
) -> Result<(), Diagnostic> {
    let blocks = program.statement_table.assembly_blocks(graph.blocks);
    let scratch: Vec<_> = graph
        .scratch
        .iter()
        .map(|expression| {
            expression
                .is_valid()
                .then(|| root(program, *expression))
                .flatten()
        })
        .collect();
    // Validate every authored destination before initialization or either
    // normalization route can treat a rooted projection as a scalar write.
    let statements = program.statement_table.statements(state.statement_nodes);
    for block in blocks {
        for statement in &statements
            [body_start + block.start as usize..body_start + (block.start + block.count) as usize]
        {
            if let StatementNode::Assignment(assignment) = statement {
                super::register_versions::destination(
                    program,
                    assignment.target,
                    scratch.iter().flatten().copied(),
                )?;
            }
        }
    }
    const { assert!(language_core::inline_assembly::ASM_SCRATCH_SLOT_COUNT <= u64::BITS as usize) };
    let mut incoming = vec![u64::MAX; blocks.len()];
    let mut outgoing = incoming.clone();
    let mut reachable = vec![false; blocks.len()];
    reachable[0] = true;
    loop {
        let mut changed = false;
        let mut next = vec![u64::MAX; blocks.len()];
        next[0] = 0;
        let mut next_reachable = vec![false; blocks.len()];
        next_reachable[0] = true;
        for (index, block) in blocks
            .iter()
            .enumerate()
            .filter(|(index, _)| reachable[*index])
        {
            let mut defined = incoming[index];
            let statements = program.statement_table.statements(state.statement_nodes);
            for statement in &statements[body_start + block.start as usize
                ..body_start + (block.start + block.count) as usize]
            {
                if let StatementNode::Assignment(assignment) = statement
                    && let Some(index) = root(program, assignment.target).and_then(|symbol| {
                        scratch
                            .iter()
                            .position(|candidate| *candidate == Some(symbol))
                    })
                {
                    defined |= 1 << index;
                }
            }
            outgoing[index] = defined;
            for target in successors(program, graph, index)? {
                next[target] &= defined;
                next_reachable[target] = true;
            }
        }
        for index in 0..blocks.len() {
            if incoming[index] != next[index] || reachable[index] != next_reachable[index] {
                changed = true;
            }
        }
        incoming = next;
        reachable = next_reachable;
        if !changed {
            break;
        }
    }
    let statements = program.statement_table.statements(state.statement_nodes);
    for (index, block) in blocks
        .iter()
        .enumerate()
        .filter(|(index, _)| reachable[*index])
    {
        let mut defined = incoming[index];
        for statement in &statements
            [body_start + block.start as usize..body_start + (block.start + block.count) as usize]
        {
            let mut reads = Vec::new();
            if let StatementNode::Assignment(assignment) = statement {
                validation::expression_traversal::collect_expression_tree(
                    program,
                    assignment.value,
                    &mut reads,
                );
                target_reads(program, assignment.target, &mut reads);
            } else {
                validation::expression_traversal::collect_statement_expression_trees(
                    program, statement, &mut reads,
                );
            }
            for expression in reads {
                if let ExpressionNode::Name(path) = program.expression_table.expression(expression)
                    && let Some(register) = scratch
                        .iter()
                        .position(|candidate| *candidate == Some(path.head_symbol))
                    && defined & (1 << register) == 0
                {
                    return Err(Diagnostic::error(format!(
                        "assembly label `{}` reads uninitialized scratch register/flag {}",
                        block.label,
                        if register == ASM_ZERO_FLAG_SLOT {
                            "ZF"
                        } else {
                            "value"
                        }
                    )));
                }
            }
            if let StatementNode::Assignment(assignment) = statement
                && let Some(register) = root(program, assignment.target).and_then(|symbol| {
                    scratch
                        .iter()
                        .position(|candidate| *candidate == Some(symbol))
                })
            {
                defined |= 1 << register;
            }
        }
        if let AssemblyExit::ZeroFlag(_, when_set) = block.exit
            && defined & (1 << ASM_ZERO_FLAG_SLOT) == 0
        {
            return Err(Diagnostic::error(format!(
                "assembly label `{}` {} reads uninitialized ZF",
                block.label,
                if when_set { "jz" } else { "jnz" }
            )));
        }
    }
    Ok(())
}

fn parameters(
    program: &mut TypedTrees,
    state: &mut State,
    bindings: &[Binding],
) -> Vec<(SymbolHandle, SymbolHandle)> {
    let symbols = program.symbols.insert_generated_children_from(
        state.symbol,
        bindings
            .iter()
            .map(|binding| (binding.symbol, SymbolKind::Parameter, binding.name.as_str())),
    );
    let mut mapping = Vec::new();
    for (binding, symbol) in bindings.iter().zip(symbols.handles()) {
        program.push_state_parameter(
            state,
            StateParameter {
                symbol,
                name: binding.name.clone(),
                type_reference: binding.parameter_type,
                is_mutable: false,
                ..Default::default()
            },
        );
        mapping.push((binding.symbol, symbol));
    }
    mapping
}

/// A register version's recorded initializer preserves the exact premise that
/// produced the value (`sub` records the decreased register). Passing that
/// expression across an edge re-reads its operands at the edge: when one of
/// them is a place the same edge also passes as a mutable capture, the edge
/// would hold one place both mutably and read-only, and the register's snapshot
/// would be re-read after the place's write instead of preserved. The immutable
/// register version is the snapshot itself, so an initializer that reads a
/// mutable capture yields that version.
fn rereads_mutable_capture(
    program: &TypedTrees,
    value: ExpressionHandle,
    mutable_captures: &[SymbolHandle],
) -> bool {
    let mut expressions = Vec::new();
    validation::expression_traversal::collect_expression_tree(program, value, &mut expressions);
    expressions.iter().any(|expression| {
        matches!(
            program.expression_table.expression(*expression),
            ExpressionNode::Name(path) if mutable_captures.contains(&path.head_symbol)
        )
    })
}

fn target(
    program: &mut TypedTrees,
    symbol: SymbolHandle,
    identifier: &Identifier,
    bindings: &[Binding],
    mapping: &[(SymbolHandle, SymbolHandle)],
    instructions: &[StatementNode],
) -> typed_trees::statement::TransitionTargetHandle {
    let mutable_captures: Vec<_> = bindings
        .iter()
        .zip(mapping)
        .filter(|(binding, _)| binding.access == Some(language_core::ReferenceAccess::Mutable))
        .map(|(_, (_, symbol))| *symbol)
        .collect();
    let arguments: Vec<_> = bindings
        .iter()
        .zip(mapping)
        .map(|(_, (_, symbol))| {
            let recorded = instructions.iter().find_map(|statement| match statement {
                StatementNode::LocalData(local)
                    if local.symbol == *symbol && local.initial_value.is_valid() =>
                {
                    Some(local.initial_value)
                }
                _ => None,
            });
            recorded
                .filter(|value| !rereads_mutable_capture(program, *value, &mutable_captures))
                .unwrap_or_else(|| name(program, *symbol))
        })
        .collect();
    let arguments = program.statement_table.insert_expression_handles(arguments);
    let mut members = HandleSpan::empty();
    program
        .statement_table
        .push_name_path_member(&mut members, identifier.clone());
    program
        .statement_table
        .insert_transition_target(TransitionTargetNode::Named {
            operational_acknowledgement: Default::default(),
            path: typed_trees::statement::TableNamePath {
                members,
                head_symbol: symbol,
                symbol,
            },
            static_machine_parameter: SymbolHandle::invalid(),
            arguments,
            evidence_arguments: Box::default(),
            source_span: Default::default(),
            authored_call_selection: None,
        })
}

fn exit(
    program: &mut TypedTrees,
    graph: &AssemblyControlFlow,
    index: usize,
    states: &[(SymbolHandle, Identifier)],
    bindings: &[Binding],
    mapping: &[(SymbolHandle, SymbolHandle)],
    instructions: &[StatementNode],
) -> Result<Vec<StatementNode>, Diagnostic> {
    let block = program.statement_table.assembly_blocks(graph.blocks)[index].clone();
    let terminal = |program: &mut TypedTrees| {
        program
            .statement_table
            .insert_transition_target(TransitionTargetNode::Terminal)
    };
    let next = |program: &mut TypedTrees| {
        if let Some((symbol, identifier)) = states.get(index + 1) {
            target(
                program,
                *symbol,
                identifier,
                bindings,
                mapping,
                instructions,
            )
        } else {
            terminal(program)
        }
    };
    let destination = match block.exit {
        AssemblyExit::Jump(handle) | AssemblyExit::ZeroFlag(handle, _) => {
            let ordinal = graph
                .blocks
                .handles()
                .position(|candidate| candidate == handle)
                .ok_or_else(|| Diagnostic::error("assembly edge has no label"))?;
            target(
                program,
                states[ordinal].0,
                &states[ordinal].1,
                bindings,
                mapping,
                instructions,
            )
        }
        AssemblyExit::Fallthrough => next(program),
        AssemblyExit::EnclosingTransition => {
            return Err(Diagnostic::error(
                "assembly enclosing transition cannot become a helper return",
            ));
        }
    };
    let mut transition = TableTransition {
        target: destination,
        ..Default::default()
    };
    if let AssemblyExit::ZeroFlag(_, when_set) = block.exit {
        let old_flag = root(program, graph.scratch[ASM_ZERO_FLAG_SLOT])
            .ok_or_else(|| Diagnostic::error("zero-flag branch has no modeled zero flag"))?;
        let flag = mapping
            .iter()
            .find(|(source, _)| *source == old_flag)
            .ok_or_else(|| Diagnostic::error("zero-flag branch lost its zero flag version"))?
            .1;
        let operand = name(program, flag);
        let condition = if when_set {
            operand
        } else {
            program
                .expression_table
                .insert(ExpressionNode::Unary(TableUnaryExpression {
                    operator: UnaryOperator::LogicalNot,
                    operand,
                }))
        };
        transition.guard = TransitionGuardNode::When(condition);
        let continuation = next(program);
        Ok(vec![
            StatementNode::Transition(transition),
            StatementNode::Transition(TableTransition {
                target: continuation,
                ..Default::default()
            }),
        ])
    } else {
        Ok(vec![StatementNode::Transition(transition)])
    }
}

fn has_scratch(program: &TypedTrees, state: &State) -> bool {
    program.statement_table.statements(state.statement_nodes).iter().any(|statement| {
        matches!(statement, StatementNode::Assembly(AssemblyControlPoint::Entry(graph)) if program.statement_table.assembly(*graph).scratch.iter().any(|register| register.is_valid()))
    })
}

pub(crate) fn normalize(program: &mut TypedTrees) -> Result<(), Vec<Diagnostic>> {
    super::validate_primitive_meanings(program)?;
    // Local control and register operations may lower to ordinary statements.
    // Consume their recorded catalog demands before either normalization route
    // erases the graph; an authored target never supplies missing semantics.
    for machine_index in 0..program.machines().len() {
        let machine = &program.machines()[machine_index];
        let mut requirement = machine.assembly_target_requirement;
        for state in program.machine_states(machine) {
            for statement in program.statement_table.statements(state.statement_nodes) {
                if let StatementNode::Assembly(AssemblyControlPoint::Entry(handle)) = statement {
                    requirement = requirement
                        .intersect(program.statement_table.assembly(*handle).target_requirement)
                        .ok_or_else(|| {
                            vec![Diagnostic::error(
                                "assembly catalog target requirements conflict in one machine",
                            )]
                        })?;
                }
            }
        }
        if machine
            .authored_target
            .as_ref()
            .is_some_and(|target| !requirement.accepts_target_name(target.as_str()))
        {
            return Err(vec![Diagnostic::error(
                "assembly catalog target requirement conflicts with the authored target body",
            )]);
        }
        program.machines_mut()[machine_index].assembly_target_requirement = requirement;
    }

    if !program.machines().iter().any(|machine| {
        program
            .machine_states(machine)
            .iter()
            .any(|state| has_scratch(program, state))
    }) {
        return Ok(());
    }
    let source = program.clone();
    let mut replacements = Vec::new();
    for (machine_index, machine) in source.machines().iter().enumerate() {
        for (state_index, state) in source.machine_states(machine).iter().enumerate() {
            if !has_scratch(&source, state) {
                continue;
            }
            let statements = source.statement_table.statements(state.statement_nodes);
            let mut rewritten = Vec::new();
            let mut cursor = 0;
            while cursor < statements.len() {
                let StatementNode::Assembly(AssemblyControlPoint::Entry(handle)) =
                    statements[cursor]
                else {
                    rewritten.push(statements[cursor].clone());
                    cursor += 1;
                    continue;
                };
                let graph = source.statement_table.assembly(handle);
                if !graph.scratch.iter().any(|expression| expression.is_valid()) {
                    rewritten.push(statements[cursor].clone());
                    cursor += 1;
                    continue;
                }
                check_register_flow(&source, state, graph, cursor + 1)
                    .map_err(|diagnostic| vec![diagnostic])?;
                if !source
                    .statement_table
                    .assembly_blocks(graph.blocks)
                    .iter()
                    .any(|block| matches!(block.exit, AssemblyExit::ZeroFlag(_, _)))
                {
                    let graph_window = HandleSpan::from_parts(
                        state.statement_nodes.handle_at(cursor as u32),
                        1 + graph.statement_count,
                    );
                    let copied = program
                        .tables
                        .copy_statement_nodes_deep_from(&source.tables, graph_window);
                    let mut registers: Vec<_> = graph
                        .scratch
                        .iter()
                        .copied()
                        .filter(|expression| expression.is_valid())
                        .map(|expression| {
                            let symbol =
                                root(&source, expression).expect("scratch is a declaration name");
                            super::register_versions::RegisterVersion {
                                source: symbol,
                                current: symbol,
                                carrier: declaration_type(&source, state, symbol)
                                    .expect("scratch carrier retained"),
                                name: Identifier::generated(source.symbols.name(symbol)),
                            }
                        })
                        .collect();
                    let mut sequence = program.statement_table.statements(copied).to_vec();
                    let mut visited =
                        vec![false; source.statement_table.assembly_blocks(graph.blocks).len()];
                    let mut next = Some(0);
                    while let Some(index) = next {
                        if visited[index] {
                            return Err(vec![Diagnostic::error(
                                "unconditional assembly cycle requires an explicit checked progress/control model",
                            )]);
                        }
                        visited[index] = true;
                        let block = &source.statement_table.assembly_blocks(graph.blocks)[index];
                        for offset in block.start as usize + 1..(block.start + block.count) as usize
                        {
                            sequence[1 + offset] = super::register_versions::statement(
                                program,
                                state.symbol,
                                copied.handle_at((1 + offset) as u32),
                                &mut registers,
                                offset,
                            )
                            .map_err(|diagnostic| vec![diagnostic])?;
                        }
                        next = self::successors(&source, graph, index)
                            .map_err(|diagnostic| vec![diagnostic])?
                            .first()
                            .copied();
                    }
                    rewritten.extend(sequence);
                    cursor += 1 + graph.statement_count as usize;
                    continue;
                }
                let result = normalize_graph(program, &source, machine, state, graph, cursor + 1);
                rewritten.extend(result.map_err(|diagnostic| vec![diagnostic])?);
                cursor += 1 + graph.statement_count as usize;
            }
            replacements.push((
                machine_index,
                state_index,
                program.statement_table.insert_statements(rewritten),
            ));
        }
    }
    for (machine, state, statements) in replacements {
        let handle = program.machines()[machine].states.handle_at(state as u32);
        program.machine_states.get_mut(handle).statement_nodes = statements;
    }
    Ok(())
}

/// The extracted helper owns the loop. A ranking witness whose every subject
/// is a scratch register carried into that helper moves with those registers:
/// `sub` writes the decreased version, and the helper's backedge is the edge
/// that must read it. The enclosing machine keeps a witness that still names
/// some other subject.
fn carry_ranking_witness(
    program: &mut TypedTrees,
    owner_symbol: SymbolHandle,
    helper: &mut Machine,
    entry_mapping: &[(SymbolHandle, SymbolHandle)],
) {
    let Some(mut witness) = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == owner_symbol)
        .and_then(|machine| machine.termination_plan.implementation_witness.clone())
    else {
        return;
    };
    let Some(custody) = program
        .ranking_expression_custody_for(owner_symbol)
        .cloned()
    else {
        return;
    };
    if custody.subjects.is_empty() {
        return;
    }
    let mut subjects = Vec::new();
    let mut rendered = Vec::new();
    for subject in &custody.subjects {
        let text = typed_trees::ranking::witness_expression_text(program, *subject);
        let Some((_, parameter)) = entry_mapping.iter().find(|(source, _)| {
            let hidden = program.symbols.name(*source);
            hidden == text || hidden.rsplit('#').next() == Some(text.as_str())
        }) else {
            return;
        };
        subjects.push(name(program, *parameter));
        rendered.push(program.symbols.name(*parameter).to_string());
    }
    witness.subjects = rendered;
    helper.termination_plan.implementation_witness = Some(witness);
    program
        .ranking_expression_custody
        .push(typed_trees::ranking::RankingExpressionCustody {
            machine: helper.symbol,
            subjects,
            view_arguments: custody.view_arguments,
            rank_range: custody.rank_range,
        });
    if let Some(owner) = program
        .machines_mut()
        .iter_mut()
        .find(|machine| machine.symbol == owner_symbol)
    {
        owner.termination_plan.implementation_witness = None;
    }
    program
        .ranking_expression_custody
        .retain(|row| row.machine != owner_symbol);
}

fn normalize_graph(
    program: &mut TypedTrees,
    source: &TypedTrees,
    owner: &Machine,
    state: &State,
    graph: &AssemblyControlFlow,
    body_start: usize,
) -> Result<Vec<StatementNode>, Diagnostic> {
    check_register_flow(source, state, graph, body_start)?;
    if owner.has_specialization_parameters() {
        return Err(Diagnostic::error(
            "scratch assembly requires a closed callable telescope before CFG normalization",
        ));
    }
    let blocks = source.statement_table.assembly_blocks(graph.blocks);
    let successors = successors(source, graph, 0)?;
    let mut reachable = vec![false; blocks.len()];
    let mut pending = successors.clone();
    while let Some(index) = pending.pop() {
        if reachable[index] {
            continue;
        }
        reachable[index] = true;
        pending.extend(self::successors(source, graph, index)?);
    }
    let scratch_symbols: Vec<_> = graph
        .scratch
        .iter()
        .filter(|expression| expression.is_valid())
        .map(|expression| root(source, *expression).expect("scratch declaration is a name"))
        .collect();
    let source_statements = source.statement_table.statements(state.statement_nodes);
    let mut captures = Vec::<(SymbolHandle, bool)>::new();
    for statement in blocks
        .iter()
        .enumerate()
        .filter(|(index, _)| reachable[*index])
        .flat_map(|(_, block)| {
            &source_statements[body_start + block.start as usize
                ..body_start + (block.start + block.count) as usize]
        })
    {
        let written = if let StatementNode::Assignment(assignment) = statement {
            root(source, assignment.target)
        } else {
            None
        };
        let mut expressions = Vec::new();
        validation::expression_traversal::collect_statement_expression_trees(
            source,
            statement,
            &mut expressions,
        );
        for expression in expressions {
            let ExpressionNode::Name(path) = source.expression_table.expression(expression) else {
                continue;
            };
            let symbol = path.head_symbol;
            if scratch_symbols.contains(&symbol)
                || !matches!(
                    source.symbols.get(symbol).kind,
                    SymbolKind::Local | SymbolKind::Parameter
                )
            {
                continue;
            }
            let writes = written == Some(symbol);
            if let Some((_, writable)) = captures
                .iter_mut()
                .find(|(candidate, _)| *candidate == symbol)
            {
                *writable |= writes;
            } else {
                captures.push((symbol, writes));
            }
        }
    }
    let mut bindings = Vec::new();
    for (symbol, writable) in captures {
        let declared = declaration_type(source, state, symbol).ok_or_else(|| {
            Diagnostic::error("assembly capture has no exact storage declaration")
        })?;
        let value_type = match source.type_reference_table.type_reference(declared) {
            TypeReferenceNode::Reference { referee, .. } => *referee,
            _ => declared,
        };
        let access = if writable {
            language_core::ReferenceAccess::Mutable
        } else {
            language_core::ReferenceAccess::Shared
        };
        let parameter_type = program
            .type_reference_table
            .insert(TypeReferenceNode::Reference {
                referee: value_type,
                access,
                lifetime: None,
            });
        bindings.push(Binding {
            symbol,
            name: Identifier::generated(source.symbols.name(symbol)),
            parameter_type,
            access: Some(access),
        });
    }
    for symbol in scratch_symbols {
        let value_type = declaration_type(source, state, symbol)
            .ok_or_else(|| Diagnostic::error("scratch register has no exact carrier"))?;
        bindings.push(Binding {
            symbol,
            name: Identifier::generated(source.symbols.name(symbol)),
            parameter_type: value_type,
            access: None,
        });
    }
    let helper_name = format!(
        "{}$assembly${}${body_start}${}",
        owner.name,
        owner
            .authored_target
            .as_ref()
            .map_or("portable", Identifier::as_str),
        state.name,
    );
    let helper_symbol =
        program
            .symbols
            .insert_generated_root_from(owner.symbol, SymbolKind::Machine, &helper_name);
    let mut helper = Machine {
        assembly_target_requirement: graph.target_requirement,
        symbol: helper_symbol,
        name: Identifier::generated(&helper_name),
        // Extraction preserves the original direct-service declaration. The
        // ordinary inference pass recomputes the helper/caller closure; union
        // is idempotent, so retaining this contribution adds no net reach.
        service_reach_row: owner.service_reach_row,
        service_reach_is_installation_bound: owner.service_reach_is_installation_bound,
        target: owner.target.clone(),
        authored_target: owner.authored_target.clone(),
        termination_plan: language_semantics::MachineTerminationPlan {
            interface: owner.termination_plan.interface.clone(),
            ..Default::default()
        },
        body_is_present: true,
        body_status: owner.body_status,
        ..Default::default()
    };
    // The helper's ordinary entry state is independent of every assembly
    // label. Only an authored `entry` collides with that ordinary entry state:
    // give it an unspellable ordinal name and use the same map for declarations
    // and edge targets. Preserve every other label so proof diagnostics retain
    // the authored names. Block handles, contracts and statement windows stay
    // unchanged, including backward edges to an authored `entry`.
    let state_names: Vec<_> = blocks
        .iter()
        .enumerate()
        .map(|(index, block)| {
            if block.label.as_str() == "entry" {
                Identifier::generated(format!("asm#label#{index}#entry"))
            } else {
                block.label.clone()
            }
        })
        .collect();
    let state_symbols = program.symbols.insert_generated_children(
        helper_symbol,
        std::iter::once((SymbolKind::State, "entry")).chain(
            state_names
                .iter()
                .map(|identifier| (SymbolKind::State, identifier.as_str())),
        ),
    );
    let states: Vec<_> = state_symbols.handles().skip(1).zip(state_names).collect();
    let unit_type = program.type_reference_table.insert(TypeReferenceNode::Unit);
    let mut entry = State {
        return_type: unit_type,
        symbol: state_symbols.handle_at(0),
        name: Identifier::generated("entry"),
        ..Default::default()
    };
    let entry_mapping = parameters(program, &mut entry, &bindings);
    let dispatch = exit(program, graph, 0, &states, &bindings, &entry_mapping, &[])?;
    entry.statement_nodes = program.statement_table.insert_statements(dispatch);
    // Destination obligations become ordinary checked call preconditions. They
    // are not admissions: the caller proves them after its executed prefix.
    for (edge, destination) in successors.iter().copied().enumerate() {
        let target_block = &blocks[destination];
        let window = HandleSpan::from_parts(
            state
                .statement_nodes
                .handle_at((body_start + target_block.start as usize + 1) as u32),
            target_block.count - 1,
        );
        let copied = program
            .tables
            .copy_statement_nodes_deep_from(&source.tables, window);
        program
            .tables
            .remap_statement_symbols(copied, &entry_mapping);
        let requires: Vec<_> = program
            .statement_table
            .statements(copied)
            .iter()
            .filter_map(|statement| {
                statement
                    .as_assembly_fact()
                    .filter(|fact| fact.kind == AssemblyFactKind::Requires)
                    .map(|fact| fact.fact)
            })
            .collect();
        for fact in requires {
            let mut fact = program.proof_facts.get(fact).clone();
            if let AssemblyExit::ZeroFlag(_, when_set) = blocks[0].exit {
                let typed_trees::domain::ProofFact::Expression(obligation) = fact else {
                    return Err(Diagnostic::error(
                        "conditional assembly entry requires a Boolean edge proposition",
                    ));
                };
                let flag_symbol = root(source, graph.scratch[ASM_ZERO_FLAG_SLOT])
                    .ok_or_else(|| Diagnostic::error("conditional entry has no zero flag"))?;
                let flag_symbol = entry_mapping
                    .iter()
                    .find(|(old, _)| *old == flag_symbol)
                    .ok_or_else(|| Diagnostic::error("conditional entry lost zero flag"))?
                    .1;
                let flag = name(program, flag_symbol);
                // Every destination owes !edge_guard || obligation. The
                // taken and fallthrough guards swap for JZ/B.EQ vs JNZ/B.NE.
                let inverse_guard = if (edge == 0) != when_set {
                    flag
                } else {
                    program
                        .expression_table
                        .insert(ExpressionNode::Unary(TableUnaryExpression {
                            operator: UnaryOperator::LogicalNot,
                            operand: flag,
                        }))
                };
                let implication = program.expression_table.insert(ExpressionNode::Binary(
                    typed_trees::expression::TableBinaryExpression {
                        left: inverse_guard,
                        operator: typed_trees::expression::BinaryOperator::Or,
                        right: obligation,
                    },
                ));
                fact = typed_trees::domain::ProofFact::Expression(implication);
            }
            let facts = program.proof_facts.insert_many([fact]);
            program.push_machine_contract(
                &mut helper,
                SignatureContract {
                    kind: SignatureContractKind::Requires,
                    keyword_source_span: None,
                    binding: None,
                    facts,
                    token_count: 0,
                },
            );
        }
    }
    let captured_references: Vec<_> = bindings
        .iter()
        .zip(&entry_mapping)
        .filter(|(binding, _)| binding.access.is_some())
        .map(|(_, (_, parameter))| *parameter)
        .collect();
    super::normal_exit_contracts::append(
        program,
        source,
        state,
        graph,
        body_start,
        &entry_mapping,
        &captured_references,
        &mut helper,
    );
    program.push_machine_state(&mut helper, entry);
    for (index, block) in blocks
        .iter()
        .enumerate()
        .filter(|(index, _)| reachable[*index])
    {
        let mut label = State {
            return_type: unit_type,
            symbol: states[index].0,
            name: states[index].1.clone(),
            ..Default::default()
        };
        let mapping = parameters(program, &mut label, &bindings);
        let window = HandleSpan::from_parts(
            state
                .statement_nodes
                .handle_at((body_start + block.start as usize + 1) as u32),
            block.count - 1,
        );
        let copied = program
            .tables
            .copy_statement_nodes_deep_from(&source.tables, window);
        program.tables.remap_statement_symbols(copied, &mapping);
        let mut registers: Vec<_> = bindings
            .iter()
            .zip(&mapping)
            .filter(|(binding, _)| binding.access.is_none())
            .map(
                |(binding, (_, symbol))| super::register_versions::RegisterVersion {
                    source: *symbol,
                    current: *symbol,
                    carrier: binding.parameter_type,
                    name: binding.name.clone(),
                },
            )
            .collect();
        let mut instructions = Vec::new();
        for (ordinal, handle) in copied.handles().enumerate() {
            let statement = super::register_versions::statement(
                program,
                label.symbol,
                handle,
                &mut registers,
                ordinal,
            )?;
            if let StatementNode::AssemblyFact(fact) = &statement
                && fact.kind == AssemblyFactKind::Requires
            {
                let fact = program.proof_facts.get(fact.fact).clone();
                let facts = program.proof_facts.insert_many([fact]);
                program.push_state_contract(
                    &mut label,
                    SignatureContract {
                        kind: SignatureContractKind::Requires,
                        keyword_source_span: None,
                        binding: None,
                        facts,
                        token_count: 0,
                    },
                );
                continue;
            }
            instructions.push(statement);
        }
        let edge_mapping: Vec<_> = mapping
            .iter()
            .map(|(old, parameter)| {
                (
                    *old,
                    registers
                        .iter()
                        .find(|register| register.source == *parameter)
                        .map_or(*parameter, |register| register.current),
                )
            })
            .collect();
        instructions.extend(exit(
            program,
            graph,
            index,
            &states,
            &bindings,
            &edge_mapping,
            &instructions,
        )?);
        label.statement_nodes = program.statement_table.insert_statements(instructions);
        program.push_machine_state(&mut helper, label);
    }
    carry_ranking_witness(program, owner.symbol, &mut helper, &entry_mapping);
    let prefix = HandleSpan::from_parts(
        state
            .statement_nodes
            .handle_at((body_start + blocks[0].start as usize + 1) as u32),
        blocks[0].count - 1,
    );
    let prefix = program
        .tables
        .copy_statement_nodes_deep_from(&source.tables, prefix);
    let mut registers: Vec<_> = bindings
        .iter()
        .filter(|binding| binding.access.is_none())
        .map(|binding| super::register_versions::RegisterVersion {
            source: binding.symbol,
            current: binding.symbol,
            carrier: binding.parameter_type,
            name: binding.name.clone(),
        })
        .collect();
    let mut result: Vec<_> = prefix
        .handles()
        .enumerate()
        .map(|(ordinal, handle)| {
            super::register_versions::statement(
                program,
                state.symbol,
                handle,
                &mut registers,
                ordinal,
            )
        })
        .collect::<Result<_, _>>()?;
    let arguments: Vec<_> = bindings
        .iter()
        .map(|binding| {
            let symbol = registers
                .iter()
                .find(|register| register.source == binding.symbol)
                .map_or(binding.symbol, |register| register.current);
            let target = name(program, symbol);
            if let Some(access) = binding.access {
                program
                    .expression_table
                    .insert(ExpressionNode::Borrow(TableBorrowExpression {
                        target,
                        access,
                    }))
            } else {
                target
            }
        })
        .collect();
    let arguments = program.statement_table.insert_expression_handles(arguments);
    helper.lexical_region_origin = Some(typed_trees::machine::LexicalRegionOrigin {
        machine: owner.symbol,
        state: state.symbol,
        arguments,
    });
    program.push_machine(helper);
    result.push(StatementNode::Call(TableCall {
        // Ordinary typed calls name the executable entry, not its machine
        // declaration. Effect inference, flow, and execution join that exact
        // state identity; a root-machine symbol loses the call's service row.
        target_symbol: state_symbols.handle_at(0),
        target: Identifier::generated(&helper_name),
        arguments,
        ..Default::default()
    }));
    Ok(result)
}
