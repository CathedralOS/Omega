/*
Lexical assembly control normalization, register versioning, and exit contracts
(`assembly_control`) alongside selected operator call binding (`operators`).
*/

pub(crate) mod assembly_control {
    pub(crate) mod normal_exit_contracts {
        use crate::tables::expressions_of;
        use arena::HandleSpan;
        use symbols::{SymbolHandle, SymbolKind};
        use typed_trees::TypedTrees;
        use typed_trees::domain::ProofFact;
        use typed_trees::expression::ExpressionNode;
        use typed_trees::machine::Machine;
        use typed_trees::signature::{SignatureContract, SignatureContractKind};
        use typed_trees::state::State;
        use typed_trees::statement::{AssemblyControlFlow, AssemblyExit, AssemblyFactKind};
        use validation::expression_traversal::collect_expression_tree;

        pub(crate) fn derived_contract(
            kind: SignatureContractKind,
            facts: HandleSpan<ProofFact>,
        ) -> SignatureContract {
            SignatureContract {
                kind,
                keyword_source_span: None,
                binding: None,
                facts,
                token_count: 0,
            }
        }

        pub(crate) fn append(
            program: &mut TypedTrees,
            source: &TypedTrees,
            state: &State,
            graph: &AssemblyControlFlow,
            body_start: usize,
            mapping: &[(SymbolHandle, SymbolHandle)],
            captured_references: &[SymbolHandle],
            helper: &mut Machine,
        ) {
            let blocks = source.statement_table.assembly_blocks(graph.blocks);
            let Some((exit_index, block)) = blocks.iter().enumerate().next_back() else {
                return;
            };
            if !matches!(
                block.exit,
                AssemblyExit::Fallthrough | AssemblyExit::ZeroFlag(_, _)
            ) {
                return;
            }
            let window = HandleSpan::from_parts(
                state
                    .statement_nodes
                    .handle_at((body_start + block.start as usize + 1) as u32),
                block.count - 1,
            );
            for statement in window.handles() {
                let Some(fact) = source
                    .statement_table
                    .statement(statement)
                    .as_assembly_fact()
                else {
                    continue;
                };
                if fact.kind != AssemblyFactKind::Ensures {
                    continue;
                }
                let copied = program.tables.copy_statement_nodes_deep_from(
                    &source.tables,
                    HandleSpan::from_parts(statement, 1),
                );
                program.tables.remap_statement_symbols(copied, mapping);
                let Some(fact) = program
                    .statement_table
                    .statement(copied.handle_at(0))
                    .as_assembly_fact()
                else {
                    continue;
                };
                let fact = program.proof_facts.get(fact.fact).clone();
                let ProofFact::Expression(expression_root) = &fact else {
                    continue;
                };
                let mut expressions = Vec::new();
                collect_expression_tree(program, *expression_root, &mut expressions);
                let mut observes_capture = false;
                let mut escapes_frame = false;
                for expression in expressions {
                    if matches!(
                        expressions_of(program).expression(expression),
                        ExpressionNode::SavedSubject(_)
                    ) && !validation::expression_has_closed_value_captures(
                        program,
                        *expression_root,
                    ) {
                        escapes_frame = true;
                    }
                    if let ExpressionNode::Name(path) =
                        expressions_of(program).expression(expression)
                    {
                        if !path.head_symbol.is_valid() {
                            escapes_frame = true;
                            continue;
                        }
                        if captured_references.contains(&path.head_symbol) {
                            observes_capture = true;
                        } else if matches!(
                            program.symbols.get(path.head_symbol).kind,
                            SymbolKind::Local | SymbolKind::Parameter
                        ) {
                            escapes_frame = true;
                        }
                    }
                }
                if !observes_capture || escapes_frame {
                    continue;
                }
                let kinds: &[SignatureContractKind] = if exit_index == 0 {
                    &[
                        SignatureContractKind::Requires,
                        SignatureContractKind::Ensures,
                    ]
                } else {
                    &[SignatureContractKind::Ensures]
                };
                for kind in kinds {
                    let facts = program.proof_facts.insert_many([fact.clone()]);
                    program.push_machine_contract(helper, derived_contract(kind.clone(), facts));
                }
            }
        }
    }
    pub(crate) mod normalization {
        use super::normal_exit_contracts::derived_contract;
        use crate::tables::{expressions_of, expressions_of_mut};
        use arena::HandleSpan;
        use diagnostics::Diagnostic;
        use language_core::inline_assembly::ASM_ZERO_FLAG_SLOT;
        use symbols::{SymbolHandle, SymbolKind};
        use typed_trees::expression::{
            ExpressionHandle, ExpressionNode, TableBorrowExpression, TableNamePath,
            TableUnaryExpression, UnaryOperator,
        };
        use typed_trees::signature::{SignatureContractKind, StateParameter};
        use typed_trees::statement::{
            AssemblyControlFlow, AssemblyControlPoint, AssemblyExit, AssemblyFactKind,
            StatementNode, TableCall, TableTransition, TransitionGuardNode, TransitionTargetNode,
        };
        use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};
        use typed_trees::{TypedTrees, machine::Machine, name::Identifier, state::State};
        use validation::expression_traversal::{
            collect_expression_tree, collect_statement_expression_trees,
        };

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
            expressions_of_mut(program).push_name_path_member(&mut members, identifier.clone());
            expressions_of_mut(program).insert(ExpressionNode::Name(TableNamePath::new(
                members,
                HandleSpan::empty(),
                symbol,
                symbol,
            )))
        }

        pub(crate) fn root(
            program: &TypedTrees,
            expression: ExpressionHandle,
        ) -> Option<SymbolHandle> {
            match expressions_of(program).expression(expression) {
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

        fn target_reads(
            program: &TypedTrees,
            expression: ExpressionHandle,
            reads: &mut Vec<ExpressionHandle>,
        ) {
            match expressions_of(program).expression(expression) {
                ExpressionNode::Name(_) => {}
                ExpressionNode::Member(member) => target_reads(program, member.receiver, reads),
                ExpressionNode::Indexed(indexed) => {
                    target_reads(program, indexed.collection, reads);
                    collect_expression_tree(program, indexed.index, reads);
                }
                ExpressionNode::Borrow(borrow) => target_reads(program, borrow.target, reads),
                _ => collect_expression_tree(program, expression, reads),
            }
        }

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
            let statements = program.statement_table.statements(state.statement_nodes);
            for block in blocks {
                for statement in &statements[body_start + block.start as usize
                    ..body_start + (block.start + block.count) as usize]
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
            const {
                assert!(
                    language_core::inline_assembly::ASM_SCRATCH_SLOT_COUNT <= u64::BITS as usize
                )
            };
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
                            && let Some(index) =
                                root(program, assignment.target).and_then(|symbol| {
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
                for statement in &statements[body_start + block.start as usize
                    ..body_start + (block.start + block.count) as usize]
                {
                    let mut reads = Vec::new();
                    if let StatementNode::Assignment(assignment) = statement {
                        collect_expression_tree(program, assignment.value, &mut reads);
                        target_reads(program, assignment.target, &mut reads);
                    } else {
                        collect_statement_expression_trees(program, statement, &mut reads);
                    }
                    for expression in reads {
                        if let ExpressionNode::Name(path) =
                            expressions_of(program).expression(expression)
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
                        && let Some(register) =
                            root(program, assignment.target).and_then(|symbol| {
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

        fn rereads_mutable_capture(
            program: &TypedTrees,
            value: ExpressionHandle,
            mutable_captures: &[SymbolHandle],
        ) -> bool {
            let mut expressions = Vec::new();
            collect_expression_tree(program, value, &mut expressions);
            expressions.iter().any(|expression| {
                matches!(
                    expressions_of(program).expression(*expression),
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
                .filter(|(binding, _)| {
                    binding.access == Some(language_core::ReferenceAccess::Mutable)
                })
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
                        .filter(|value| {
                            !rereads_mutable_capture(program, *value, &mutable_captures)
                        })
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
                        receiver_symbol: SymbolHandle::invalid(),
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
                let old_flag =
                    root(program, graph.scratch[ASM_ZERO_FLAG_SLOT]).ok_or_else(|| {
                        Diagnostic::error("zero-flag branch has no modeled zero flag")
                    })?;
                let flag = mapping
                    .iter()
                    .find(|(source, _)| *source == old_flag)
                    .ok_or_else(|| {
                        Diagnostic::error("zero-flag branch lost its zero flag version")
                    })?
                    .1;
                let operand = name(program, flag);
                let condition = if when_set {
                    operand
                } else {
                    expressions_of_mut(program).insert(ExpressionNode::Unary(
                        TableUnaryExpression {
                            operator: UnaryOperator::LogicalNot,
                            operand,
                        },
                    ))
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
            for machine_index in 0..program.machines().len() {
                let machine = &program.machines()[machine_index];
                let mut requirement = machine.assembly_target_requirement;
                for state in program.machine_states(machine) {
                    for statement in program.statement_table.statements(state.statement_nodes) {
                        if let StatementNode::Assembly(AssemblyControlPoint::Entry(handle)) =
                            statement
                        {
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
                                    let symbol = root(&source, expression)
                                        .expect("scratch is a declaration name");
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
                                vec![
                                    false;
                                    source.statement_table.assembly_blocks(graph.blocks).len()
                                ];
                            let mut next = Some(0);
                            while let Some(index) = next {
                                if visited[index] {
                                    return Err(vec![Diagnostic::error(
                                        "unconditional assembly cycle requires an explicit checked progress/control model",
                                    )]);
                                }
                                visited[index] = true;
                                let block =
                                    &source.statement_table.assembly_blocks(graph.blocks)[index];
                                for offset in
                                    block.start as usize + 1..(block.start + block.count) as usize
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
                        let result =
                            normalize_graph(program, &source, machine, state, graph, cursor + 1);
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
            program.ranking_expression_custody.push(
                typed_trees::ranking::RankingExpressionCustody {
                    machine: helper.symbol,
                    subjects,
                    view_arguments: custody.view_arguments,
                    rank_range: custody.rank_range,
                },
            );
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
                collect_statement_expression_trees(source, statement, &mut expressions);
                for expression in expressions {
                    let ExpressionNode::Name(path) = expressions_of(source).expression(expression)
                    else {
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
                let parameter_type =
                    program
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
            let helper_symbol = program.symbols.insert_generated_root_from(
                owner.symbol,
                SymbolKind::Machine,
                &helper_name,
            );
            let mut helper = Machine {
                assembly_target_requirement: graph.target_requirement,
                symbol: helper_symbol,
                name: Identifier::generated(&helper_name),
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
                            .ok_or_else(|| {
                                Diagnostic::error("conditional entry has no zero flag")
                            })?;
                        let flag_symbol = entry_mapping
                            .iter()
                            .find(|(old, _)| *old == flag_symbol)
                            .ok_or_else(|| Diagnostic::error("conditional entry lost zero flag"))?
                            .1;
                        let flag = name(program, flag_symbol);
                        let inverse_guard = if (edge == 0) != when_set {
                            flag
                        } else {
                            expressions_of_mut(program).insert(ExpressionNode::Unary(
                                TableUnaryExpression {
                                    operator: UnaryOperator::LogicalNot,
                                    operand: flag,
                                },
                            ))
                        };
                        let implication =
                            expressions_of_mut(program).insert(ExpressionNode::Binary(
                                typed_trees::expression::TableBinaryExpression {
                                    policy: numerics::arithmetic::ArithmeticDomain::Exact,
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
                        derived_contract(SignatureContractKind::Requires, facts),
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
                            derived_contract(SignatureContractKind::Requires, facts),
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
                        expressions_of_mut(program).insert(ExpressionNode::Borrow(
                            TableBorrowExpression { target, access },
                        ))
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
                target_symbol: state_symbols.handle_at(0),
                target: Identifier::generated(&helper_name),
                arguments,
                ..Default::default()
            }));
            Ok(result)
        }
    }
    pub(crate) mod register_versions {
        use crate::tables::{expressions_of, expressions_of_mut};
        use arena::{Handle, HandleSpan};
        use diagnostics::Diagnostic;
        use symbols::{SymbolHandle, SymbolKind};
        use typed_trees::expression::{ExpressionHandle, ExpressionNode};
        use typed_trees::statement::{StatementNode, TableLocalData};
        use typed_trees::types::TypeReferenceHandle;
        use typed_trees::{TypedTrees, name::Identifier};
        use validation::expression_traversal::collect_statement_expression_trees;

        pub(crate) struct RegisterVersion {
            pub source: SymbolHandle,
            pub current: SymbolHandle,
            pub carrier: TypeReferenceHandle,
            pub name: Identifier,
        }

        pub(crate) fn destination(
            program: &TypedTrees,
            target: ExpressionHandle,
            mut registers: impl Iterator<Item = SymbolHandle>,
        ) -> Result<Option<SymbolHandle>, Diagnostic> {
            let Some(root) = super::normalization::root(program, target)
                .filter(|root| registers.any(|register| register == *root))
            else {
                return Ok(None);
            };
            if !matches!(expressions_of(program).expression(target), ExpressionNode::Name(path)
        if path.head_symbol == root && path.symbol == root && path.members.len() == 1)
            {
                return Err(Diagnostic::error(
                    "assembly scratch register destination must be a plain register name; member, index and borrow destinations are not scalar register writes",
                ));
            }
            Ok(Some(root))
        }

        pub(crate) fn statement(
            program: &mut TypedTrees,
            owner: SymbolHandle,
            handle: Handle<StatementNode>,
            registers: &mut [RegisterVersion],
            ordinal: usize,
        ) -> Result<StatementNode, Diagnostic> {
            let mapping: Vec<_> = registers
                .iter()
                .map(|register| (register.source, register.current))
                .collect();
            program
                .tables
                .remap_statement_symbols(HandleSpan::from_parts(handle, 1), &mapping);
            let statement = program.statement_table.statement(handle).clone();
            let mut expressions = Vec::new();
            collect_statement_expression_trees(program, &statement, &mut expressions);
            for expression in expressions {
                let ExpressionNode::Name(path) = expressions_of(program).expression(expression)
                else {
                    continue;
                };
                if !registers.iter().any(|register| {
                    register.current != register.source && register.current == path.symbol
                }) {
                    continue;
                }
                let mut members = HandleSpan::empty();
                let identifier = Identifier::generated(program.symbols.name(path.symbol));
                expressions_of_mut(program).push_name_path_member(&mut members, identifier);
                let ExpressionNode::Name(path) =
                    expressions_of_mut(program).expression_mut(expression)
                else {
                    unreachable!()
                };
                path.members = members;
            }
            let StatementNode::Assignment(assignment) = &statement else {
                return Ok(statement);
            };
            let Some(root) = destination(
                program,
                assignment.target,
                registers.iter().map(|register| register.current),
            )?
            else {
                return Ok(statement);
            };
            let Some(register) = registers
                .iter_mut()
                .find(|register| register.current == root)
            else {
                return Ok(statement);
            };
            let name = Identifier::generated(format!("{}$version${ordinal}", register.name));
            let symbol = program
                .symbols
                .append_generated_children_from(
                    owner,
                    [(register.source, SymbolKind::Local, name.as_str())],
                )
                .handle_at(0);
            register.current = symbol;
            Ok(StatementNode::LocalData(TableLocalData {
                symbol,
                name,
                type_reference: register.carrier,
                initial_value: assignment.value,
                ..Default::default()
            }))
        }
    }
    pub(crate) use normalization::normalize;

    use diagnostics::Diagnostic;
    use typed_trees::TypedTrees;
    use typed_trees::statement::AssemblyControlPoint;
    use typed_trees::statement::StatementNode;

    pub(crate) fn admit(program: &TypedTrees) -> Result<(), Vec<Diagnostic>> {
        for machine in program.machines() {
            for state in program.machine_states(machine) {
                if !program
                    .statement_table
                    .statements(state.statement_nodes)
                    .iter()
                    .any(|statement| matches!(statement, StatementNode::Assembly(_)))
                {
                    continue;
                }
                let order = program
                    .statement_table
                    .finite_execution_order(state.statement_nodes)
                    .map_err(|reason| {
                        vec![Diagnostic::error(format!(
                            "invalid assembly control in `{}` state `{}`: {reason}",
                            machine.name, state.name,
                        ))]
                    })?;
                if order.is_none() {
                    return Err(vec![Diagnostic::error(format!(
                        "cyclic assembly label control in `{}` state `{}` requires checked loop invariants; this control domain is not yet implemented",
                        machine.name, state.name,
                    ))]);
                }
            }
        }
        Ok(())
    }

    pub(crate) fn validate_primitive_meanings(program: &TypedTrees) -> Result<(), Vec<Diagnostic>> {
        for machine in program.machines() {
            for state in program.machine_states(machine) {
                for statement in program.statement_table.statements(state.statement_nodes) {
                    let StatementNode::Assembly(AssemblyControlPoint::Entry(handle)) = statement
                    else {
                        continue;
                    };
                    let graph = program.statement_table.assembly(*handle);
                    for expression in program.statement_table.expression_handles(graph.primitives) {
                        if !validation::has_builtin_binary_expression_meaning(
                            program,
                            machine,
                            Some(state),
                            *expression,
                        ) {
                            return Err(vec![Diagnostic::error(format!(
                                "catalog assembly arithmetic/zero-flag operation in `{}` state `{}` requires builtin operator meaning; an authored operator cannot redefine an instruction",
                                machine.name, state.name
                            ))]);
                        }
                    }
                }
            }
        }
        Ok(())
    }
}

pub(crate) mod operators {
    pub(crate) mod call_binding {
        use crate::monomorphization::insert_subject_name;
        use crate::tables::{expressions_of, expressions_of_mut};
        use checked_trees::{
            CheckedOperatorOccurrence, CheckedOperatorResolutionStatus, CheckedValueOrigin,
        };
        use diagnostics::Diagnostic;
        use numerics::literals::IntegerLiteral;
        use std::collections::HashSet;
        use symbols::SymbolHandle;
        use typed_trees::TypedTrees;
        use typed_trees::expression::TableBorrowExpression;
        use typed_trees::expression::TableMemberExpression;
        use typed_trees::expression::{
            BinaryOperator, ExpressionHandle, ExpressionNode, MatchPattern, TableBinaryExpression,
            TableCallExpression, TableMatchArm, TableMatchExpression, TableRangeExpression,
        };
        use typed_trees::name::Identifier;
        use typed_trees::operator::bound_trait_operator_meanings;
        use typed_trees::operator::selected_trait_operator_meanings;
        use typed_trees::typed_trees::StaticRequirementDispatch;
        use typed_trees::types::{PrimitiveType, TypeReferenceHandle};
        use validation::expression_type_reference_for_origin;
        use validation::indexed_operand_types;

        pub(crate) fn bind_selected_operator_calls(
            program: &mut TypedTrees,
            roots: &typed_trees::references::DeclarationSelection,
        ) -> Result<usize, Vec<Diagnostic>> {
            if program.machine_token_bindings().is_empty()
                && program.operators().is_empty()
                && !program.machines().iter().any(|machine| {
                    machine.supply_mode
                        == language_semantics::MachineSupplyMode::TopLevelRequirement
                })
                && !program.traits().iter().any(|definition| {
                    program
                        .trait_machine_signatures(definition)
                        .iter()
                        .any(|requirement| requirement.spelling.is_some())
                })
            {
                return Ok(0);
            }
            let values = validation::collect_executable_value_origins(program, roots);
            let mut facts = validation::build_operator_uses(program, &values);
            validation::select_pending_domain_operator_meanings(program, &mut facts);
            let mut diagnostics = Vec::new();
            let mut bindings: Vec<(ExpressionHandle, SelectedOperatorCall, CheckedValueOrigin)> =
                Vec::new();
            let mut match_calls: Vec<(
                ExpressionHandle,
                arena::Handle<TableMatchArm>,
                SelectedOperatorCall,
            )> = Vec::new();
            let mut occurrences = facts
                .uses
                .iter()
                .map(|(handle, usage)| {
                    (usage.expression, usage.origin, usage.spelling, Some(handle))
                })
                .collect::<Vec<_>>();
            let mut seen = HashSet::with_hasher(symbols::BuildSymbolHasher);
            seen.extend(
                occurrences
                    .iter()
                    .map(|(expression, origin, _, _)| (*expression, *origin)),
            );
            /*
                Each value fact's subtree is walked in pre-order, but a subtree
                already walked for the same origin is not walked again: every
                pair it holds is already in `seen`. Nested subexpressions carry
                facts of their own origin, so walking each one's whole tree was
                quadratic in an operator chain's length.
            */
            let mut walked = HashSet::with_hasher(symbols::BuildSymbolHasher);
            let mut pending = Vec::new();
            let mut node_children = Vec::new();
            for (_, value) in values
                .values
                .iter()
                .filter(|(_, value)| value.origin.machine_symbol().is_some())
            {
                pending.push(value.expression);
                while let Some(expression) = pending.pop() {
                    if !expression.is_valid() || !walked.insert((expression, value.origin)) {
                        continue;
                    }
                    validation::expression_traversal::children(
                        program,
                        expressions_of(program).expression(expression),
                        |child| node_children.push(child),
                    );
                    pending.extend(node_children.drain(..).rev());
                    let Some(binary) = expressions_of(program).binary_node(expression) else {
                        continue;
                    };
                    let Some(spelling) =
                        typed_trees::operator::binary_operator_spelling(binary.operator)
                    else {
                        continue;
                    };
                    if seen.insert((expression, value.origin)) {
                        occurrences.push((expression, value.origin, spelling, None));
                    }
                }
            }
            for (expression, origin, spelling, use_handle) in occurrences {
                let operator_use = use_handle.map(|handle| facts.uses.get(handle));
                let selected = operator_use.map_or_else(SymbolHandle::invalid, |usage| {
                    usage.selected_operator_symbol
                });
                if program.machine_by_symbol(selected).is_some_and(|machine| {
                    machine.supply_mode
                        == language_semantics::MachineSupplyMode::TopLevelRequirement
                        && !machine.body_is_present
                }) && use_handle
                    .and_then(|handle| facts.selected_float_comparison(program, handle))
                    .is_some()
                {
                    continue;
                }
                if bindings.iter().any(|(bound, _, _)| *bound == expression) {
                    continue;
                }
                let target =
                    match selected_call_target(program, expression, origin, spelling, operator_use)
                    {
                        Ok(Some(target)) => target,
                        Ok(None) => continue,
                        Err(diagnostic) => {
                            diagnostics.push(diagnostic);
                            continue;
                        }
                    };
                if let Some(usage) = operator_use
                    && let CheckedOperatorOccurrence::MatchEquality { source_arm } =
                        usage.occurrence
                    && program
                        .machine_by_symbol(selected)
                        .is_some_and(|machine| machine.body_is_present)
                {
                    match_calls.push((expression, source_arm, target));
                    continue;
                }
                let bindable = operator_use
                    .is_none_or(|usage| usage.occurrence == CheckedOperatorOccurrence::Expression)
                    && matches!(
                        expressions_of(program).expression(expression),
                        ExpressionNode::Binary(_) | ExpressionNode::Indexed(_)
                    );
                if !bindable {
                    diagnostics.push(Diagnostic::error(format!(
                "`{}` was selected for its fixed operator token `{}` in a position whose body supply is not implemented; only binary and indexed expression uses bind the declaration's own body",
                target.name,
                spelling.symbol(),
            )));
                    continue;
                }
                bindings.push((expression, target, origin));
            }

            let match_count = match bind_body_bearing_match_equality(program, match_calls) {
                Ok(count) => count,
                Err(errors) => {
                    diagnostics.extend(errors);
                    0
                }
            };
            let mut bound_count = bindings.len() + match_count;
            let mut named_targets = Vec::new();
            if !program.machine_token_bindings().is_empty() {
                let mut seen_named = HashSet::with_hasher(symbols::BuildSymbolHasher);
                // A named target depends only on the expression, so one walk
                // over every value's tree, each expression taken once, finds
                // them all in first-seen order.
                let mut expressions = Vec::new();
                validation::expression_traversal::collect_expression_trees(
                    program,
                    values.values.iter().map(|(_, value)| value.expression),
                    &mut expressions,
                );
                {
                    for expression in expressions.iter().copied() {
                        let Some(call) = expressions_of(program).call_node(expression) else {
                            continue;
                        };
                        let owner = program
                            .machine_holding_state(call.target_symbol)
                            .or_else(|| program.machine_by_symbol(call.target_symbol));
                        if let Some(owner) = owner
                            && program
                                .machine_token_binding_by_symbol(owner.symbol)
                                .is_some()
                            && seen_named.insert((expression, owner.symbol))
                        {
                            named_targets.push((expression, owner.symbol));
                        }
                    }
                }
            }
            for (expression, requirement) in facts
                .named_uses()
                .map(|fact| (fact.expression, fact.selected_operator_symbol))
                .chain(
                    facts
                        .named_requirement_uses
                        .iter()
                        .map(|(_, fact)| (fact.expression, fact.requirement_symbol)),
                )
                .chain(named_targets)
            {
                let Some(binding) = program.machine_token_binding_by_symbol(requirement) else {
                    continue;
                };
                let Some(call) = expressions_of(program).call_node(expression) else {
                    continue;
                };
                if !call.receiver.is_valid() {
                    continue;
                }
                let Some(receiver) = expressions_of(program).name_path_node(call.receiver) else {
                    continue;
                };
                if receiver.symbol.is_valid()
                    && !matches!(
                        program.symbols.get(receiver.symbol).kind,
                        symbols::SymbolKind::Data
                            | symbols::SymbolKind::Domain
                            | symbols::SymbolKind::Module
                            | symbols::SymbolKind::BuiltinType
                    )
                {
                    continue;
                }
                let path = program.operator_path_members(binding.name);
                let Some((member, namespace)) = path.split_last() else {
                    continue;
                };
                if member.as_str() != call.target.as_str()
                    || namespace != expressions_of(program).name_path_members(receiver.members)
                {
                    continue;
                }
                let Some(machine) = program.machine_by_symbol(requirement) else {
                    continue;
                };
                let Some(entry) = program.machine_states(machine).first() else {
                    continue;
                };
                let entry_symbol = entry.symbol;
                let application = call.authored_machine_application;
                if application.is_valid() {
                    let retained =
                        expressions_of_mut(program).static_machine_application_mut(application);
                    if !retained.registration_operation.is_valid() {
                        retained.registration_operation = entry_symbol;
                    }
                }
                if let ExpressionNode::Call(call) =
                    expressions_of_mut(program).expression_mut(expression)
                {
                    call.receiver = ExpressionHandle::invalid();
                    call.target_symbol = entry_symbol;
                    bound_count += 1;
                }
            }
            for (expression, target, origin) in bindings {
                let operand_source = match expressions_of(program).expression(expression) {
                    ExpressionNode::Binary(binary) => {
                        OperandSource::Pair([binary.left, binary.right])
                    }
                    ExpressionNode::Indexed(indexed) => OperandSource::Indexed {
                        collection: indexed.collection,
                        index: indexed.index,
                    },
                    _ => unreachable!("binding candidates are binary or indexed expressions"),
                };
                let start_parameter = target
                    .start_parameter
                    .is_valid()
                    .then_some(target.start_parameter);
                let operands = match operand_source {
                    OperandSource::Pair(pair) => pair.to_vec(),
                    OperandSource::Indexed { collection, index } => {
                        let collection_argument = indexed_collection_argument(
                            program,
                            collection,
                            target.collection_parameter,
                        );
                        let mut operands = vec![collection_argument];
                        let range = match expressions_of(program).expression(index) {
                            ExpressionNode::Range(range) => Some(*range),
                            _ => None,
                        };
                        match range {
                            Some(range) => {
                                match range_operands(
                                    program,
                                    &target.name,
                                    collection,
                                    range,
                                    start_parameter,
                                    origin,
                                ) {
                                    Ok((start, end)) => {
                                        operands.push(start);
                                        operands.push(end);
                                    }
                                    Err(diagnostic) => {
                                        diagnostics.push(diagnostic);
                                        continue;
                                    }
                                }
                            }
                            None => operands.push(index),
                        }
                        operands
                    }
                };
                let arguments =
                    expressions_of_mut(program).insert_expression_handles(operands.iter().copied());
                let receiver = if target.binder.is_valid() {
                    let binder_name = Identifier::generated(program.symbols.name(target.binder));
                    insert_subject_name(expressions_of_mut(program), binder_name, target.binder)
                } else {
                    ExpressionHandle::invalid()
                };
                let lexical_owner = target.lexical_owner;
                *expressions_of_mut(program).expression_mut(expression) =
                    ExpressionNode::Call(synthesized_operator_call(receiver, target, arguments));
                if let Err(diagnostic) =
                    retain_operator_call_application(program, expression, lexical_owner)
                {
                    diagnostics.push(diagnostic);
                }
            }
            if diagnostics.is_empty() {
                Ok(bound_count)
            } else {
                Err(diagnostics)
            }
        }

        fn bind_body_bearing_match_equality(
            program: &mut TypedTrees,
            calls: Vec<(
                ExpressionHandle,
                arena::Handle<TableMatchArm>,
                SelectedOperatorCall,
            )>,
        ) -> Result<usize, Vec<Diagnostic>> {
            let mut by_expression: Vec<(
                ExpressionHandle,
                Vec<(arena::Handle<TableMatchArm>, SelectedOperatorCall)>,
            )> = Vec::new();
            for (expression, source_arm, target) in calls {
                if let Some((_, supplied)) = by_expression
                    .iter_mut()
                    .find(|(handle, _)| *handle == expression)
                {
                    supplied.push((source_arm, target));
                } else {
                    by_expression.push((expression, vec![(source_arm, target)]));
                }
            }
            let mut bound = 0;
            let mut diagnostics = Vec::new();
            for (expression, supplied) in by_expression {
                match rewrite_match_equality(program, expression, supplied) {
                    Ok(count) => bound += count,
                    Err(diagnostic) => diagnostics.push(diagnostic),
                }
            }
            if diagnostics.is_empty() {
                Ok(bound)
            } else {
                Err(diagnostics)
            }
        }

        fn rewrite_match_equality(
            program: &mut TypedTrees,
            expression: ExpressionHandle,
            mut supplied: Vec<(arena::Handle<TableMatchArm>, SelectedOperatorCall)>,
        ) -> Result<usize, Diagnostic> {
            let dispatch = match expressions_of(program).expression(expression) {
                ExpressionNode::Match(dispatch) => *dispatch,
                _ => {
                    return Err(Diagnostic::error(
                        "body-bearing match equality lost its match expression",
                    ));
                }
            };
            let authored = expressions_of(program).match_arms(dispatch.arms).to_vec();
            let value_count = authored
                .iter()
                .position(|arm| matches!(arm.pattern, MatchPattern::Wildcard))
                .unwrap_or(authored.len());
            let reachable = supplied
                .iter()
                .filter(|(source_arm, _)| {
                    (0..value_count).any(|ordinal| {
                        u32::try_from(ordinal)
                            .ok()
                            .is_some_and(|ordinal| dispatch.arms.handle_at(ordinal) == *source_arm)
                    })
                })
                .count();
            if reachable != value_count {
                return Err(Diagnostic::error(
                    "a body-bearing match equality is missing a call for one of its value arms",
                ));
            }
            let mut remainder = authored
                .get(value_count)
                .filter(|arm| matches!(arm.pattern, MatchPattern::Wildcard))
                .map(|arm| arm.value);
            for ordinal in (0..value_count).rev() {
                let Some(ordinal_index) = u32::try_from(ordinal).ok() else {
                    return Err(Diagnostic::error(
                        "match equality arm index does not fit in u32",
                    ));
                };
                let source_arm = dispatch.arms.handle_at(ordinal_index);
                let Some(position) = supplied
                    .iter()
                    .position(|(handle, _)| *handle == source_arm)
                else {
                    return Err(Diagnostic::error(
                        "a body-bearing match equality is missing its selected call",
                    ));
                };
                let (_, target) = supplied.swap_remove(position);
                let MatchPattern::Value(pattern) = authored[ordinal].pattern else {
                    return Err(Diagnostic::error(
                        "a body-bearing match equality arm has no pattern value",
                    ));
                };
                let Some(fallback) = remainder else {
                    return Err(Diagnostic::error(
                        "a body-bearing match equality has no following arm for a false result",
                    ));
                };
                let arguments = expressions_of_mut(program)
                    .insert_expression_handles([dispatch.subject, pattern]);
                let receiver = if target.binder.is_valid() {
                    let binder_name = Identifier::generated(program.symbols.name(target.binder));
                    insert_subject_name(expressions_of_mut(program), binder_name, target.binder)
                } else {
                    ExpressionHandle::invalid()
                };
                let lexical_owner = target.lexical_owner;
                let call = expressions_of_mut(program).insert(ExpressionNode::Call(
                    synthesized_operator_call(receiver, target, arguments),
                ));
                retain_operator_call_application(program, call, lexical_owner)?;
                let truth = expressions_of_mut(program).insert(ExpressionNode::Boolean(true));
                let arms = expressions_of_mut(program).insert_match_arms([
                    TableMatchArm {
                        pattern: MatchPattern::Value(truth),
                        value: authored[ordinal].value,
                        source_span: authored[ordinal].source_span,
                    },
                    TableMatchArm {
                        pattern: MatchPattern::Wildcard,
                        value: fallback,
                        source_span: authored[ordinal].source_span,
                    },
                ]);
                remainder = Some(expressions_of_mut(program).insert(ExpressionNode::Match(
                    TableMatchExpression {
                        subject: call,
                        capture: Default::default(),
                        arms,
                    },
                )));
            }
            let Some(remainder) = remainder else {
                return Err(Diagnostic::error(
                    "body-bearing match equality produced no call",
                ));
            };
            let node = std::mem::replace(
                expressions_of_mut(program).expression_mut(remainder),
                ExpressionNode::Boolean(false),
            );
            *expressions_of_mut(program).expression_mut(expression) = node;
            Ok(value_count)
        }

        /// Records the source application of an operator call as it is
        /// created, as the program-wide pass does for a call written by
        /// hand. That pass has already run, and a call without the record
        /// loses its result contract once it goes through an implementation.
        fn retain_operator_call_application(
            program: &mut TypedTrees,
            call: ExpressionHandle,
            lexical_owner: SymbolHandle,
        ) -> Result<(), Diagnostic> {
            if !lexical_owner.is_valid() {
                return Err(Diagnostic::error(
                    "selected operator call lost the machine that spells it",
                ));
            }
            program
                .retain_call_static_argument_context(call, lexical_owner)
                .map_err(Diagnostic::error)
        }

        struct SelectedOperatorCall {
            entry: SymbolHandle,
            name: Identifier,
            /// The machine whose body spells the operator.
            lexical_owner: SymbolHandle,
            binder: SymbolHandle,
            collection_parameter: TypeReferenceHandle,
            start_parameter: TypeReferenceHandle,
            dispatch: Option<StaticRequirementDispatch>,
            arguments: Box<[typed_trees::expression::StaticMachineArgument]>,
        }

        fn implementation_formals(
            parameters: &[typed_trees::signature::StateParameter],
        ) -> (TypeReferenceHandle, TypeReferenceHandle) {
            let formal = |index: usize| {
                let parameter = parameters.get(index);
                parameter.map_or_else(TypeReferenceHandle::invalid, |parameter| {
                    parameter.type_reference
                })
            };
            (formal(0), formal(1))
        }

        fn synthesized_operator_call(
            receiver: ExpressionHandle,
            target: SelectedOperatorCall,
            arguments: arena::HandleSpan<ExpressionHandle>,
        ) -> TableCallExpression {
            TableCallExpression {
        authored_call_selection: None,
        authored_call_binding: None,
        authored_machine_application: Default::default(),
        requirement_application: Default::default(),
        exact_coercion_target: TypeReferenceHandle::invalid(),
        receiver,
        target_symbol: target.entry,
        static_machine_parameter: SymbolHandle::invalid(),
        target: target.name,
        static_requirement_dispatch: target.dispatch,
        machine_arguments: target.arguments,
        quotient_operation: None,
        private_layout_operation: None,
        installed_interrupt_operation: None,
        layout_query: None,
        native_code_layout: None,
        build_selection: Default::default(),
        arguments,
        evidence_arguments: Box::default(),
        operational_acknowledgement: language_semantics::CallOperationalAcknowledgement {
            origin: language_semantics::CallOperationalAcknowledgementOrigin::CompilerSynthesized,
            ..Default::default()
        },
    }
        }

        fn selected_call_target(
            program: &TypedTrees,
            expression: ExpressionHandle,
            origin: CheckedValueOrigin,
            spelling: language_core::OperatorSpelling,
            operator_use: Option<&checked_trees::CheckedOperatorUseFact>,
        ) -> Result<Option<SelectedOperatorCall>, Diagnostic> {
            let operand_types = match expressions_of(program).expression(expression) {
                ExpressionNode::Binary(binary) => Some(
                    [binary.left, binary.right]
                        .map(|operand| {
                            expression_type_reference_for_origin(program, operand, origin)
                        })
                        .to_vec(),
                ),
                ExpressionNode::Indexed(indexed) => {
                    Some(indexed_operand_types(program, indexed, origin))
                }
                _ => None,
            };
            if let Some(operand_types) = operand_types
                && let Some(machine) = origin.machine_symbol()
            {
                let bound =
                    bound_trait_operator_meanings(program, machine, spelling, &operand_types);
                if let [meaning] = bound.as_slice() {
                    let definition = program
                        .trait_definition_by_symbol(meaning.bound.trait_symbol)
                        .ok_or_else(|| {
                            Diagnostic::error("selected implementation bound lost its trait")
                        })?;
                    let mut requirements = Vec::new();
                    crate::monomorphization::collect_evidence_requirement_closure(
                        program,
                        definition,
                        &mut Vec::new(),
                        &mut requirements,
                    );
                    let entry = requirements
                        .iter()
                        .position(|requirement| requirement.symbol == meaning.requirement.symbol)
                        .and_then(|position| {
                            program
                                .symbols
                                .child_handles(meaning.bound.binder)
                                .into_iter()
                                .flatten()
                                .nth(position)
                        })
                        .ok_or_else(|| {
                            Diagnostic::error(
                                "selected implementation lost its requirement call placeholder",
                            )
                        })?;
                    let (collection_parameter, start_parameter) = implementation_formals(
                        program.state_signature_parameters(meaning.requirement),
                    );
                    return Ok(Some(SelectedOperatorCall {
                        entry,
                        name: meaning.requirement.name.clone(),
                        lexical_owner: machine,
                        binder: meaning.bound.binder,
                        collection_parameter,
                        start_parameter,
                        dispatch: None,
                        arguments: Box::default(),
                    }));
                }
                if !bound.is_empty() {
                    return Err(Diagnostic::error(
                        "operator has multiple explicitly selected implementation meanings",
                    ));
                }
                let closed =
                    selected_trait_operator_meanings(program, machine, spelling, &operand_types);
                if let [meaning] = closed.as_slice() {
                    let entry = program
                        .state_by_symbol(meaning.row.realization_state)
                        .ok_or_else(|| {
                            Diagnostic::error("selected implementation lost its realization entry")
                        })?;
                    let (collection_parameter, start_parameter) =
                        implementation_formals(program.state_parameters(entry));
                    return Ok(Some(SelectedOperatorCall {
                        entry: entry.symbol,
                        name: entry.name.clone(),
                        lexical_owner: machine,
                        binder: SymbolHandle::invalid(),
                        collection_parameter,
                        start_parameter,
                        dispatch: Some(StaticRequirementDispatch {
                            selection_application: Default::default(),
                            evidence_parameter: SymbolHandle::invalid(),
                            application_report_fingerprint: meaning.application.report_fingerprint,
                            application_commitment: meaning.application.commitment,
                            declaring_trait: meaning.row.declaring_trait,
                            requirement: meaning.row.requirement,
                            contract_signature: SymbolHandle::invalid(),
                            realization_machine: meaning.row.realization_machine,
                            realization_state: meaning.row.realization_state,
                        }),
                        arguments: Box::default(),
                    }));
                }
                if !closed.is_empty() {
                    return Err(Diagnostic::error(
                        "operator has multiple explicitly selected implementation meanings",
                    ));
                }
            }
            let Some(operator_use) = operator_use else {
                return Ok(None);
            };
            let selected = operator_use.selected_operator_symbol;
            if operator_use.status != CheckedOperatorResolutionStatus::Resolved
                || program.machine_token_binding_by_symbol(selected).is_none()
            {
                return Ok(None);
            }
            let machine = program
                .machine_by_symbol(selected)
                .ok_or_else(|| Diagnostic::error("selected token machine lost its declaration"))?;
            let entry = program.machine_states(machine).first().ok_or_else(|| {
                Diagnostic::error("selected token machine declares no entry state")
            })?;
            let (collection_parameter, start_parameter) =
                implementation_formals(program.state_parameters(entry));
            Ok(Some(SelectedOperatorCall {
                entry: entry.symbol,
                name: entry.name.clone(),
                lexical_owner: origin
                    .machine_symbol()
                    .unwrap_or_else(SymbolHandle::invalid),
                binder: SymbolHandle::invalid(),
                collection_parameter,
                start_parameter,
                dispatch: None,
                arguments: Box::default(),
            }))
        }

        enum OperandSource {
            Pair([ExpressionHandle; 2]),
            Indexed {
                collection: ExpressionHandle,
                index: ExpressionHandle,
            },
        }

        fn indexed_collection_argument(
            program: &mut TypedTrees,
            collection: ExpressionHandle,
            parameter: TypeReferenceHandle,
        ) -> ExpressionHandle {
            if !parameter.is_valid() {
                return collection;
            }
            let Some(shell) = validation::reference_shell(program, parameter) else {
                return collection;
            };
            let access = shell.access;
            if !access.is_readable() {
                return collection;
            }
            expressions_of_mut(program).insert(ExpressionNode::Borrow(TableBorrowExpression {
                target: collection,
                access,
            }))
        }

        fn range_operands(
            program: &mut TypedTrees,
            machine_name: &str,
            collection: ExpressionHandle,
            range: TableRangeExpression,
            start_parameter: Option<TypeReferenceHandle>,
            origin: CheckedValueOrigin,
        ) -> Result<(ExpressionHandle, ExpressionHandle), Diagnostic> {
            let end = if range.end.is_valid() {
                range.end
            } else {
                expressions_of_mut(program).insert(ExpressionNode::Member(TableMemberExpression {
                    receiver: collection,
                    member: Identifier::generated("len"),
                    member_symbol: SymbolHandle::invalid(),
                    case_variant: None,
                }))
            };
            let start = if range.start.is_valid() {
                range.start
            } else {
                let start_accepts_zero = start_parameter
                    .and_then(|type_reference| program.primitive_type_reference(type_reference))
                    .is_some_and(PrimitiveType::accepts_integer_literal);
                if !start_accepts_zero {
                    return Err(Diagnostic::error(format!(
                        "`{machine_name}` was selected for its fixed operator token `[..]` but the range use omits its start; the omitted endpoint is zero, which only an integer start parameter forms -- write it explicitly as `0..`",
                    )));
                }
                expressions_of_mut(program)
                    .insert(ExpressionNode::Integer(IntegerLiteral::from_value(0)))
            };
            if !range.end_inclusive {
                return Ok((start, end));
            }
            let end_is_integer = expression_type_reference_for_origin(program, range.end, origin)
                .and_then(|type_reference| {
                    validation::unwrapped_type_reference(program, type_reference)
                })
                .and_then(|type_reference| program.primitive_type_reference(type_reference))
                .is_some_and(PrimitiveType::accepts_integer_literal);
            if !end_is_integer {
                return Err(Diagnostic::error(format!(
                    "`{machine_name}` was selected for its fixed operator token `[..]` but the range use is inclusive (`..=`); the normalized `end + 1` form requires an integer end operand -- write the exclusive bound explicitly",
                )));
            }
            let one = expressions_of_mut(program)
                .insert(ExpressionNode::Integer(IntegerLiteral::from_value(1)));
            let end =
                expressions_of_mut(program).insert(ExpressionNode::Binary(TableBinaryExpression {
                    policy: numerics::arithmetic::ArithmeticDomain::Exact,
                    left: range.end,
                    operator: BinaryOperator::Add,
                    right: one,
                }));
            Ok((start, end))
        }
    }

    pub(crate) use call_binding::bind_selected_operator_calls;
}
