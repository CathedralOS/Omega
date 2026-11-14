/*
    Exact declaration selection that lexical lookup alone cannot settle.

    `resolution::drive` runs these in a fixed order. Operator homes are selected
    before symbol assignment; the rest run once every declaration has a
    symbol: the authored-selection ledger, nominal machine-parameter
    requirements, closed conformance rows, domain establishment routes,
    evidence forwardings, and service reaches. `signature_free_requirements` is the shared law for paths
    that name a trait requirement without a call signature.
*/

#[path = "selection_authored.rs"]
pub(crate) mod authored_selections;
pub(crate) mod body_calls {
    /*
        One traversal over a machine's call sites (statement calls, expression
        calls, transition callees and arguments, signature/contract facts, and `via` expressions)
        shared by every pass that re-points call targets after symbol assignment: closed-conformance
        routing and target-sibling binding. Each pass supplies only its decision;
        the reach over the body is defined once here.
    */

    use crate::tables::body_expressions;
    use crate::tables::body_statements;
    use crate::tables::resolved_declarations;
    use symbol_resolved_trees::SymbolResolvedTrees;
    use symbol_resolved_trees::domain::ProofFact;
    use symbol_resolved_trees::expression::ExpressionHandle;
    use symbol_resolved_trees::expression::ExpressionNode;
    use symbol_resolved_trees::name::DiagnosticName;
    use symbol_resolved_trees::signature::SignatureContract;
    use symbol_resolved_trees::statement::StatementNode;
    use symbol_resolved_trees::statement::TransitionGuardNode;
    use symbol_resolved_trees::statement::TransitionTargetHandle;
    use symbol_resolved_trees::statement::TransitionTargetNode;
    use symbol_resolved_trees::types::TypeReference;
    use symbols::JoinedPath;
    use symbols::SymbolHandle;

    /*
        Preparation-only expression roots retain their machine owner through final
        call routing. They never populate executable state bodies.
    */
    pub(crate) struct CallSelectionInput {
        pub(crate) machine: SymbolHandle,
        pub(crate) expression: ExpressionHandle,
    }

    /* One call the traversal offers to a decision. */
    pub(crate) struct CallSite<'a> {
        /* The target state symbol ordinary resolution assigned, or invalid. */
        pub(crate) target_symbol: SymbolHandle,
        /* The spelled call target. */
        pub(crate) target: &'a DiagnosticName,
        /* The member name, independent of the call carrier's qualified spelling. */
        pub(crate) member: &'a DiagnosticName,
        /* The resolved declaration the call is qualified by, or invalid. */
        pub(crate) receiver_symbol: SymbolHandle,
        /*
            The complete authored trait application before `::`. Declaration-only
            routing cannot replace it with a lexical implementation of that trait;
            typed application formation owns the argument correspondence.
        */
        pub(crate) requirement_application: arena::Handle<TypeReference>,
        /* The call is unqualified: it names no receiver declaration. */
        pub(crate) local_receiver: bool,
        /* Authored operands, excluding any receiver. */
        pub(crate) argument_count: usize,
    }

    /*
        Headers are refreshed even when a different body's completion caused the
        global contract reference walk. Only selected bodies and explicitly supplied
        preparation inputs belong to the executable work frontier.
    */
    pub(crate) fn retarget_machine_calls(
        program: &mut SymbolResolvedTrees,
        machine: SymbolHandle,
        inputs: &[CallSelectionInput],
        include_body: bool,
        decide: &mut dyn FnMut(&CallSite<'_>) -> Option<SymbolHandle>,
    ) {
        let Some((state_spans, declaration_expressions)) =
            program.machine(machine).map(|machine| {
                let state_spans = program
                    .machine_state_handles(machine.states)
                    .iter()
                    .filter(|_| include_body)
                    .map(|handle| program.machine_state(*handle).statement_nodes)
                    .collect::<Vec<_>>();
                let mut declaration_expressions = Vec::new();
                append_contract_expressions(
                    program,
                    program.signature_contracts(machine.contracts),
                    &mut declaration_expressions,
                );
                declaration_expressions.extend(
                    program
                        .machine_trait_conformances(machine.satisfies)
                        .iter()
                        .map(|conformance| conformance.via_expression),
                );
                for state in program
                    .machine_state_handles(machine.states)
                    .iter()
                    .map(|handle| program.machine_state(*handle))
                {
                    append_contract_expressions(
                        program,
                        program.signature_contracts(state.contracts),
                        &mut declaration_expressions,
                    );
                }
                (state_spans, declaration_expressions)
            })
        else {
            return;
        };
        let mut visited = VisitedExpressions::default();
        for expression in declaration_expressions {
            retarget_expression(program, expression, decide, &mut visited);
        }
        for input in inputs.iter().filter(|input| input.machine == machine) {
            retarget_expression(program, input.expression, decide, &mut visited);
        }
        for statements in state_spans {
            retarget_statement_span(program, statements, decide);
        }
    }

    /*
        Named qualifiers do not depend on an executable owner. Trait, operator and
        structural callable headers need the same exact row after reference refresh.
    */
    pub(crate) fn retarget_signature_contract_calls(
        program: &mut SymbolResolvedTrees,
        decide: &mut dyn FnMut(&CallSite<'_>) -> Option<SymbolHandle>,
    ) {
        let mut expressions = Vec::new();
        append_contract_expressions(
            program,
            program
                .tables
                .declarations
                .signature_contracts
                .iter()
                .map(|(_, contract)| contract),
            &mut expressions,
        );
        let mut visited = VisitedExpressions::default();
        for expression in expressions {
            retarget_expression(program, expression, decide, &mut visited);
        }
    }

    fn append_contract_expressions<'program>(
        program: &'program SymbolResolvedTrees,
        contracts: impl IntoIterator<Item = &'program SignatureContract>,
        expressions: &mut Vec<ExpressionHandle>,
    ) {
        for contract in contracts {
            for fact in program.proof_facts(contract.facts) {
                match fact {
                    ProofFact::Expression(expression) => {
                        expressions.push(*expression);
                    }
                    ProofFact::Membership(membership) => {
                        expressions.push(membership.value);
                    }
                }
            }
        }
    }

    fn retarget_statement_span(
        program: &mut SymbolResolvedTrees,
        statements: arena::HandleSpan<StatementNode>,
        decide: &mut dyn FnMut(&CallSite<'_>) -> Option<SymbolHandle>,
    ) {
        let mut visited = VisitedExpressions::default();
        for handle in statements.handles() {
            let statement = program.tables.bodies.statements.statement(handle).clone();
            let mut expressions = Vec::new();
            match statement {
                StatementNode::Assembly(handle) => {
                    let graph = program.tables.declarations.assembly_graphs.get(handle);
                    let blocks = resolved_declarations(program)
                        .assembly_blocks
                        .span_or_empty(graph.blocks)
                        .to_vec();
                    for block in blocks {
                        expressions.extend_from_slice(
                            resolved_declarations(program)
                                .assembly_expressions
                                .span_or_empty(block.requires),
                        );
                        expressions.extend_from_slice(
                            resolved_declarations(program)
                                .assembly_expressions
                                .span_or_empty(block.ensures),
                        );
                        retarget_statement_span(program, block.statement_nodes, decide);
                    }
                }
                StatementNode::RootBinding(binding) => {
                    expressions.push(binding.receiver);
                    if binding.implementation_operand.is_valid() {
                        expressions.push(binding.implementation_operand);
                    }
                }
                StatementNode::AssemblyFact(fact) => {
                    expressions.push(fact.expression);
                }
                StatementNode::Assignment(assignment) => {
                    expressions.extend([assignment.target, assignment.value]);
                }
                StatementNode::Call(call) => {
                    expressions.extend_from_slice(
                        body_statements(program).expression_handles(call.arguments),
                    );
                    let site = CallSite {
                        target_symbol: call.target_symbol,
                        target: &call.target,
                        member: &call.target,
                        receiver_symbol: call.receiver_symbol,
                        requirement_application: arena::Handle::invalid(),
                        local_receiver: call.receiver.is_empty(),
                        argument_count: body_statements(program)
                            .expression_handles(call.arguments)
                            .len(),
                    };
                    if let Some(target) = decide(&site)
                        && let StatementNode::Call(call) =
                            program.tables.bodies.statements.statement_mut(handle)
                    {
                        call.target_symbol = target;
                    }
                }
                StatementNode::ProofOutputBindingStatement(package) => {
                    expressions.push(package.call);
                }
                StatementNode::Expression(expression) => {
                    expressions.push(expression);
                }
                StatementNode::LocalData(local) => {
                    expressions.push(local.initial_value);
                }
                StatementNode::Transition(transition) => {
                    if let TransitionGuardNode::When(guard) = transition.guard {
                        expressions.push(guard);
                    }
                    retarget_transition_target(
                        program,
                        transition.target,
                        decide,
                        &mut expressions,
                    );
                    if transition.continuation.is_valid() {
                        retarget_transition_target(
                            program,
                            transition.continuation,
                            decide,
                            &mut expressions,
                        );
                    }
                }
            }
            visited.clear();
            for expression in expressions {
                retarget_expression(program, expression, decide, &mut visited);
            }
        }
    }

    fn retarget_transition_target(
        program: &mut SymbolResolvedTrees,
        target: TransitionTargetHandle,
        decide: &mut dyn FnMut(&CallSite<'_>) -> Option<SymbolHandle>,
        expressions: &mut Vec<ExpressionHandle>,
    ) {
        let node = body_statements(program).transition_target(target).clone();
        match node {
            TransitionTargetNode::Named {
                path,
                arguments,
                source_span,
                ..
            } => {
                let members = body_statements(program).name_path_members(path.members);
                let spelling = DiagnosticName::new(members.joined_path(), source_span);
                let replacement = members.last().and_then(|member| {
                    decide(&CallSite {
                        target_symbol: path.symbol,
                        target: &spelling,
                        member,
                        receiver_symbol: path.receiver_symbol,
                        requirement_application: arena::Handle::invalid(),
                        local_receiver: members.len() == 1,
                        argument_count: body_statements(program)
                            .expression_handles(arguments)
                            .len(),
                    })
                });
                if let Some(replacement) = replacement
                    && let TransitionTargetNode::Named { path: retained, .. } = program
                        .tables
                        .bodies
                        .statements
                        .transition_target_mut(target)
                {
                    if retained.head_symbol == retained.symbol {
                        retained.head_symbol = replacement;
                    }
                    retained.symbol = replacement;
                }
                expressions
                    .extend_from_slice(body_statements(program).expression_handles(arguments));
            }
            TransitionTargetNode::Value(value) => {
                expressions.push(value);
            }
            TransitionTargetNode::Terminal => {}
        }
    }

    fn retarget_expression(
        program: &mut SymbolResolvedTrees,
        expression: ExpressionHandle,
        decide: &mut dyn FnMut(&CallSite<'_>) -> Option<SymbolHandle>,
        visited: &mut VisitedExpressions,
    ) {
        if !expression.is_valid() || !visited.insert(expression) {
            return;
        }
        let node = body_expressions(program).expression(expression).clone();
        let mut children = Vec::new();
        match node {
            ExpressionNode::Match(dispatch) => {
                children.push(dispatch.subject);
                for arm in program.tables.bodies.expressions.match_arms(dispatch.arms) {
                    children
                        .extend(body_expressions(program).match_pattern_dependencies(arm.pattern));
                    children.push(arm.value);
                }
            }
            ExpressionNode::SavedSubject(_) => {}
            ExpressionNode::ArrayLiteral(values) => {
                children.extend_from_slice(
                    program.tables.bodies.expressions.expression_handles(values),
                );
            }
            ExpressionNode::Atomic(atomic) => {
                children.extend([atomic.value, atomic.result]);
            }
            ExpressionNode::Binary(binary) => {
                children.extend([binary.left, binary.right]);
            }
            ExpressionNode::Cast(cast) => {
                children.push(cast.value);
            }
            ExpressionNode::Call(call) => {
                if call.receiver.is_valid() {
                    children.push(call.receiver);
                }
                children.extend_from_slice(
                    body_expressions(program).expression_handles(call.arguments),
                );
                let receiver_symbol =
                    match program.tables.bodies.expressions.expression(call.receiver) {
                        ExpressionNode::Name(path) if call.receiver.is_valid() => path.symbol,
                        _ => SymbolHandle::invalid(),
                    };
                let site = CallSite {
                    target_symbol: call.target_symbol,
                    target: &call.target,
                    member: &call.target,
                    receiver_symbol,
                    requirement_application: call.requirement_application,
                    local_receiver: !call.receiver.is_valid(),
                    argument_count: body_expressions(program)
                        .expression_handles(call.arguments)
                        .len(),
                };
                if let Some(target) = decide(&site)
                    && let ExpressionNode::Call(call) =
                        program.tables.bodies.expressions.expression_mut(expression)
                {
                    call.target_symbol = target;
                }
            }
            ExpressionNode::Indexed(indexed) => {
                children.extend([indexed.collection, indexed.index]);
            }
            ExpressionNode::Membership(membership) => {
                children.push(membership.value);
            }
            ExpressionNode::Member(member) => {
                children.push(member.receiver);
            }
            ExpressionNode::Borrow(inner) => {
                children.push(inner.target);
            }
            ExpressionNode::Range(range) => {
                children.extend([range.start, range.end]);
            }
            ExpressionNode::StructLiteral(literal) => {
                children.extend(
                    body_expressions(program)
                        .struct_fields(literal.fields)
                        .iter()
                        .map(|field| field.value),
                );
            }
            ExpressionNode::Unary(unary) => {
                children.push(unary.operand);
            }
            ExpressionNode::Boolean(_)
            | ExpressionNode::Float(_)
            | ExpressionNode::UndeterminedNaN(_)
            | ExpressionNode::Integer(_)
            | ExpressionNode::Name(_)
            | ExpressionNode::String(_)
            | ExpressionNode::TypeExpression(_)
            | ExpressionNode::ZeroValue(_) => {}
        }
        for child in children {
            retarget_expression(program, child, decide, visited);
        }
    }

    /*
        The expressions one retarget walk has already reached. The walk asked a
        `Vec` whether it held each handle, which made a program-wide contract walk
        quadratic in its expression count. Handles are dense arena positions, so one
        bit per position answers the same question in constant time; the visit
        order is unchanged because only the membership test moved.
    */
    #[derive(Default)]
    struct VisitedExpressions {
        words: Vec<u64>,
        /*
            Words holding a set bit, so a per-statement walk clears what it touched
            rather than the whole table.
        */
        touched: Vec<usize>,
    }

    impl VisitedExpressions {
        /* Mark `expression` reached; false when an earlier step reached it. */
        fn insert(&mut self, expression: ExpressionHandle) -> bool {
            let index = expression.arena_index() as usize;
            let (word, bit) = (index / 64, 1u64 << (index % 64));
            if word >= self.words.len() {
                self.words.resize(word + 1, 0);
            }
            if self.words[word] & bit != 0 {
                return false;
            }
            if self.words[word] == 0 {
                self.touched.push(word);
            }
            self.words[word] |= bit;
            true
        }

        fn clear(&mut self) {
            for word in self.touched.drain(..) {
                self.words[word] = 0;
            }
        }
    }
}
pub(crate) mod call_identity {
    /*
        Declaration sources for resolved calls and their compiler copies.

        Constant evaluation retains which declarations its calls selected. A local
        evidence child has no source token of its own, so that record names its
        public method while the call retains the exact child and binder. A generated
        machine entry similarly uses its owning machine's declaration source.
    */

    use source::SourceSpan;
    use symbol_resolved_trees::SymbolResolvedTrees;
    use symbols::{SymbolHandle, SymbolKind};

    /*
        Copies of a declaration selection must still name the same declaration.
        Evidence methods use their exact bound requirement and generated entries
        use their exact owning machine. Ordinary declarations keep their own
        source; selected binders and complete applications remain separate.
    */
    pub(crate) fn selected_declaration_source(
        program: &SymbolResolvedTrees,
        target: SymbolHandle,
    ) -> Result<SourceSpan, String> {
        if let Some(requirement) =
            crate::symbols::requirement_namespaces::evidence_requirement(program, target)?
        {
            return program
                .symbols
                .symbol_source_span(requirement.symbol)
                .ok_or_else(|| {
                    "constant evidence requirement lost its public declaration source".into()
                });
        }
        let symbol = program.symbols.get(target);
        if matches!(
            symbol.kind,
            SymbolKind::Operator | SymbolKind::MachineParameter
        ) || (symbol.kind == SymbolKind::State
            && symbol.parent.is_valid()
            && program.symbols.get(symbol.parent).kind == SymbolKind::Trait)
        {
            return program
                .symbols
                .symbol_provenance_source_span(target)
                .ok_or_else(|| "constant logical call lost its exact authored provenance".into());
        }
        call_declaration_source(program, target)
    }

    pub(crate) fn call_declaration_source(
        program: &SymbolResolvedTrees,
        target: SymbolHandle,
    ) -> Result<SourceSpan, String> {
        if let Some(source) = program.symbols.symbol_source_span(target) {
            return Ok(source);
        }
        let symbol = program.symbols.get(target);
        if symbol.kind != SymbolKind::State
            || program.symbols.get(symbol.parent).kind != SymbolKind::Machine
        {
            return Err("source-less call target is not its exact owning entry".to_owned());
        }
        let entry = program
            .symbols
            .child_handles(symbol.parent)
            .and_then(|mut children| {
                children.find(|child| program.symbols.get(*child).kind == SymbolKind::State)
            });
        if entry != Some(target) {
            return Err("source-less call target is not its exact owning entry".to_owned());
        }
        program
            .symbols
            .symbol_source_span(symbol.parent)
            .ok_or_else(|| "implicit call entry lost its authored machine declaration".to_owned())
    }
}
pub(crate) mod conformance_blocks {
    /*
        Closed conformance blocks: exact requirement rows, and routing of the
        requirement calls conformance member bodies make.

        Each closed block's rows resolve against the trait catalog, including
        inherited defaults and same-named overloads. Calls an inline member makes
        to inherited requirements select this block's rows, never an ambient
        attached machine sharing the leaf name. Every trait default instantiation
        dispatches its authoring trait's requirements through its own
        implementation's rows.
    */

    use crate::selection::body_calls::retarget_machine_calls;
    use diagnostics::Diagnostic;
    use language_semantics::declaration_selection::ResolvedAuthoredCallBinding;
    use symbol_resolved_trees::SymbolResolvedTrees;
    use symbol_resolved_trees::expression::StaticMachineArgument;
    use symbol_resolved_trees::name::DiagnosticName;
    use symbol_resolved_trees::signature::StateSignature;
    use symbol_resolved_trees::state::State;
    use symbol_resolved_trees::trait_definition::{
        Conformance, ConformanceRow, ConformanceRowSource,
    };
    use symbol_resolved_trees::types::{TypeConstraint, TypeReference};
    use symbols::{SymbolHandle, SymbolKind, SymbolLookup};

    #[derive(Clone)]
    struct TraitCatalogEntry {
        symbol: SymbolHandle,
        parents: Vec<SymbolHandle>,
        requirements: Vec<RequirementCatalogEntry>,
    }

    #[derive(Clone)]
    struct RequirementCatalogEntry {
        ordinal: usize,
        declaring_trait: SymbolHandle,
        declaring_trait_name: DiagnosticName,
        requirement: SymbolHandle,
        requirement_name: DiagnosticName,
        is_default: bool,
        signature: StateSignature,
    }

    #[derive(Clone)]
    struct MachineCatalogEntry {
        ordinal: usize,
        symbol: SymbolHandle,
        name: DiagnosticName,
        states: Vec<State>,
    }

    /*
        Returns each machine's new ordinal (`None` for a dropped synthesized
        default that no row selected), so ordinal-keyed sidecars such as deferred
        machine bodies stay joined to the same declaration.
    */
    pub(crate) fn normalize_closed_conformance_blocks(
        program: &mut SymbolResolvedTrees,
    ) -> Result<Vec<Option<usize>>, Diagnostic> {
        let trait_catalog = build_trait_catalog(program);
        let machine_catalog = build_machine_catalog(program);
        let synthesized_default_candidates = program
            .conformances
            .iter()
            .flat_map(|conformance| {
                conformance
                    .rows
                    .iter()
                    .filter(|row| row.source == ConformanceRowSource::TraitDefault)
                    .filter_map(|row| {
                        let ordinal = row.provisional_realization_ordinal?;
                        machine_catalog
                            .iter()
                            .find(|machine| machine.ordinal == ordinal)
                            .map(|machine| machine.symbol)
                    })
            })
            .collect::<Vec<_>>();
        let normalized = program
            .conformances
            .iter()
            .map(|conformance| {
                normalize_one(
                    program,
                    conformance
                        .alias
                        .as_ref()
                        .map_or("<unnamed implementation>", DiagnosticName::as_str),
                    conformance.trait_name.as_str(),
                    conformance.trait_symbol,
                    &conformance.rows,
                    &trait_catalog,
                    &machine_catalog,
                )
            })
            .collect::<Result<Vec<_>, _>>()?;

        let mut normalized = normalized.into_iter();
        program.conformances.for_each_mut(|conformance| {
            conformance.rows = normalized
                .next()
                .expect("one normalized row map per conformance");
        });
        let selected_realizations = program
            .conformances
            .iter()
            .flat_map(|conformance| {
                conformance
                    .rows
                    .iter()
                    .map(|row| row.realization_machine)
                    .filter(|symbol| symbol.is_valid())
            })
            .collect::<Vec<_>>();
        let mut ordinals = Vec::with_capacity(program.machines.len());
        let mut discarded = Vec::new();
        let mut retained_count = 0;
        program.machines.retain(|machine| {
            let retain = !synthesized_default_candidates.contains(&machine.symbol)
                || selected_realizations.contains(&machine.symbol);
            ordinals.push(retain.then_some(retained_count));
            retained_count += usize::from(retain);
            if !retain {
                discarded.push(machine.symbol);
            }
            retain
        });
        for declaration in discarded {
            program.symbols.clear_declaration_owner(declaration);
        }
        for machine in program.machines.iter() {
            if machine.lexical_parent.is_valid()
                && !program.machines.contains(machine.lexical_parent)
            {
                return Err(Diagnostic::error(
                    "retained nested machine lost its enclosing declaration",
                ));
            }
        }
        Ok(ordinals)
    }

    #[derive(Clone)]
    struct RequirementRoute {
        requirement: SymbolHandle,
        requirement_name: DiagnosticName,
        realization_state: SymbolHandle,
        /*
            The requirement's authored formals. A call passing one operand more
            is a selector call (`Trait::member(selector, data...)`), which
            dispatches at runtime instead of naming this block's row.
        */
        parameter_count: usize,
    }

    #[derive(Clone)]
    struct ConformanceRoutingPlan {
        machine: SymbolHandle,
        routes: Vec<RequirementRoute>,
        /*
            Whether a bare name may select a row by spelling. Only an inline
            member is lexically inside the block whose rows it names; a default
            body's names were already bound to its authoring trait's requirements.
        */
        routes_by_name: bool,
        /* Present when `machine` instantiates a trait default body. */
        default_body: Option<DefaultBodyScope>,
    }

    /*
        What a default body may not name: the rows its implementation supplies
        beyond the authoring trait's requirement surface.
    */
    #[derive(Clone)]
    struct DefaultBodyScope {
        declaring_trait: SymbolHandle,
        /* The named implementation this instantiation serves, for diagnostics. */
        implementation: String,
        hidden_names: Vec<DiagnosticName>,
    }

    impl DefaultBodyScope {
        fn new(
            declaring_trait: SymbolHandle,
            implementation: String,
            trait_catalog: &[TraitCatalogEntry],
            conformance_names: impl IntoIterator<Item = DiagnosticName>,
        ) -> Self {
            let mut surface = Vec::new();
            collect_requirement_closure(
                declaring_trait,
                trait_catalog,
                &mut Vec::new(),
                &mut surface,
            );
            let mut hidden_names = Vec::<DiagnosticName>::new();
            for name in conformance_names {
                if !surface
                    .iter()
                    .any(|requirement| requirement.requirement_name.as_str() == name.as_str())
                    && !hidden_names
                        .iter()
                        .any(|hidden| hidden.as_str() == name.as_str())
                {
                    hidden_names.push(name);
                }
            }
            Self {
                declaring_trait,
                implementation,
                hidden_names,
            }
        }
    }

    /*
        A call through a named implementation (`Ascending::before(left, right)`)
        selects that implementation's row for the requirement it names. The row's
        public requirement remains the call target until typed preparation closes
        and retains the selected map. Replacing it with the private realization
        here would erase that source selection before its contract can be checked.
        A requirement name that matches no row, or several inherited rows, leaves
        the call unresolved so checking rejects it rather than choosing one.
        Constant initializers bind that same public requirement during constant
        finalization, before their authored selection is recorded.
    */
    pub(crate) fn route_named_implementation_calls(
        program: &mut SymbolResolvedTrees,
        inputs: &[super::body_calls::CallSelectionInput],
        work: &[SymbolHandle],
    ) {
        let implementation_rows = program
            .conformances
            .iter()
            .map(|conformance| {
                (
                    conformance.symbol,
                    conformance
                        .rows
                        .iter()
                        .map(|row| (row.requirement_name.clone(), row.requirement))
                        .collect::<Vec<_>>(),
                )
            })
            .collect::<Vec<_>>();
        if implementation_rows.is_empty() {
            return;
        }
        let mut select_requirement = |site: &super::body_calls::CallSite<'_>| {
            named_implementation_requirement(&implementation_rows, site)
        };
        super::body_calls::retarget_signature_contract_calls(program, &mut select_requirement);
        let machines = program
            .machines
            .iter()
            .map(|machine| machine.symbol)
            .collect::<Vec<_>>();
        for machine in machines {
            super::body_calls::retarget_machine_calls(
                program,
                machine,
                inputs,
                work.contains(&machine),
                &mut select_requirement,
            );
        }
    }

    fn named_implementation_requirement(
        implementation_rows: &[(SymbolHandle, Vec<(DiagnosticName, SymbolHandle)>)],
        site: &super::body_calls::CallSite<'_>,
    ) -> Option<SymbolHandle> {
        if !site.receiver_symbol.is_valid() {
            return None;
        }
        let (_, rows) = implementation_rows
            .iter()
            .find(|(conformance, _)| *conformance == site.receiver_symbol)?;
        Some(
            named_requirement(
                rows.iter().map(|(name, requirement)| (name, *requirement)),
                site.member.as_str(),
            )
            .unwrap_or_else(|_| SymbolHandle::invalid()),
        )
    }

    fn named_requirement<'name>(
        rows: impl Iterator<Item = (&'name DiagnosticName, SymbolHandle)>,
        member: &str,
    ) -> Result<SymbolHandle, &'static str> {
        let mut matches = rows.filter(|(name, _)| name.as_str() == member);
        let selected = matches
            .next()
            .map(|(_, requirement)| requirement)
            .ok_or("has no matching requirement row")?;
        if matches.next().is_some() {
            return Err("has more than one matching requirement row");
        }
        Ok(selected)
    }

    /*
        Calls a conformance's own member bodies make to its requirements select
        that conformance's exact rows, never an ambient attached machine that
        happens to share the leaf name.

        An inline member is lexically part of one closed block, so its bare names
        select the block's rows by spelling. A trait default body is lexically part
        of its authoring trait instead: symbol assignment already bound its bare
        names to that trait's exact requirements (`LexicalRequirements` in the
        symbols scope), so every instantiation re-points those requirements to its
        own implementation's rows. A default body that names a row outside its
        trait's surface is rejected here, where that implementation is known: the
        trait author cannot see what a particular implementation adds.
        A machine named by a selection row remains that ordinary machine: it has no
        enclosing conformance context and may be shared by several blocks.
    */
    pub(crate) fn route_conformance_requirement_calls(
        program: &mut SymbolResolvedTrees,
        inputs: &[super::body_calls::CallSelectionInput],
        work: &[SymbolHandle],
    ) -> Result<(), Vec<Diagnostic>> {
        let trait_catalog = build_trait_catalog(program);
        let plans = program
            .conformances
            .iter()
            .flat_map(|conformance| {
                conformance_routing_plans(program, conformance, &conformance.rows, &trait_catalog)
            })
            .collect::<Vec<_>>();

        let mut hidden_calls = Vec::new();
        for plan in plans {
            retarget_machine_calls(
                program,
                plan.machine,
                inputs,
                work.contains(&plan.machine),
                &mut |site| {
                    let routed = plan.binding(site).map(|binding| binding.selected);
                    if routed.is_none()
                        && !site.requirement_application.is_valid()
                        && !site.target_symbol.is_valid()
                        && site.local_receiver
                        && let Some(scope) = &plan.default_body
                        && scope
                            .hidden_names
                            .iter()
                            .any(|hidden| hidden.as_str() == site.member.as_str())
                    {
                        hidden_calls.push((
                            scope.declaring_trait,
                            scope.implementation.clone(),
                            site.member.clone(),
                        ));
                    }
                    routed
                },
            );
        }
        let diagnostics = hidden_calls
        .into_iter()
        .filter(|(_, _, call)| {
            program
                .symbols
                .find_top_level_declaration_or_operator_family_from_source(
                    call.as_str(),
                    &[
                        symbols::SymbolKind::Machine,
                        symbols::SymbolKind::Operator,
                        symbols::SymbolKind::BuiltinFunction,
                    ],
                    call.source_span(),
                )
                .is_none()
        })
        .map(|(declaring_trait, implementation, call)| {
            let trait_name = program.symbols.display_path(declaring_trait, "::");
            Diagnostic::error(format!(
                "default body in trait `{trait_name}` calls `{}`, which is not a requirement of `{trait_name}` or a trait it inherits; its instantiation for `{implementation}` cannot name what that conformance adds",
                call.as_str()
            ))
            .with_source_span(call.source_span())
        })
        .collect::<Vec<_>>();
        if diagnostics.is_empty() {
            Ok(())
        } else {
            Err(diagnostics)
        }
    }

    impl ConformanceRoutingPlan {
        fn binding(
            &self,
            site: &super::body_calls::CallSite<'_>,
        ) -> Option<ResolvedAuthoredCallBinding> {
            if site.requirement_application.is_valid() {
                return None;
            }
            routed_binding(
                site.target_symbol,
                site.member,
                site.argument_count,
                self.routes_by_name && site.local_receiver,
                &self.routes,
            )
        }
    }

    fn conformance_routing_plans(
        program: &SymbolResolvedTrees,
        conformance: &Conformance,
        rows: &[ConformanceRow],
        trait_catalog: &[TraitCatalogEntry],
    ) -> Vec<ConformanceRoutingPlan> {
        let parameter_count = |requirement: SymbolHandle| {
            trait_catalog
                .iter()
                .flat_map(|entry| &entry.requirements)
                .find(|entry| entry.requirement == requirement)
                .map_or(0, |entry| entry.signature.parameters.len())
        };
        let routes = rows
            .iter()
            .filter(|row| row.realization_state.is_valid())
            .map(|row| RequirementRoute {
                requirement: row.requirement,
                requirement_name: row.requirement_name.clone(),
                realization_state: row.realization_state,
                parameter_count: parameter_count(row.requirement),
            })
            .collect::<Vec<_>>();
        rows.iter()
            .filter(|row| row.source.owns_realization() && row.realization_machine.is_valid())
            .map(|row| {
                let declaring_trait = program
                    .machine(row.realization_machine)
                    .map_or_else(SymbolHandle::invalid, |machine| {
                        machine.trait_default_origin.declaring_trait
                    });
                ConformanceRoutingPlan {
                    machine: row.realization_machine,
                    routes: routes.clone(),
                    routes_by_name: row.source == ConformanceRowSource::Inline,
                    default_body: declaring_trait.is_valid().then(|| {
                        DefaultBodyScope::new(
                            declaring_trait,
                            conformance
                                .alias
                                .as_ref()
                                .map_or_else(String::new, |name| name.as_str().to_owned()),
                            trait_catalog,
                            rows.iter().map(|row| row.requirement_name.clone()),
                        )
                    }),
                }
            })
            .collect()
    }

    /*
        Source dependencies can need a member body before executable call routing.
        Prepare the same declaration-owned plans against one immutable program.
        Only reached conformances are normalized; unrelated unfinished rows do not
        become dependencies. The caller checks retained typed selections separately.
    */
    pub(crate) struct ProvisionalBodyCallRouting<'program> {
        program: &'program SymbolResolvedTrees,
        catalogs: Option<(Vec<TraitCatalogEntry>, Vec<MachineCatalogEntry>)>,
        owner_namespaces: Vec<(SymbolHandle, SymbolHandle)>,
        conformances: Vec<PreparedConformanceRouting>,
    }

    struct PreparedConformanceRouting {
        namespace: SymbolHandle,
        rows: Vec<ConformanceRow>,
        plans: Vec<ConformanceRoutingPlan>,
    }

    impl<'program> ProvisionalBodyCallRouting<'program> {
        pub(crate) fn new(program: &'program SymbolResolvedTrees) -> Self {
            Self {
                program,
                catalogs: None,
                owner_namespaces: Vec::new(),
                conformances: Vec::new(),
            }
        }

        pub(crate) fn binding(
            &mut self,
            machine: SymbolHandle,
            site: &super::body_calls::CallSite<'_>,
        ) -> Result<Option<ResolvedAuthoredCallBinding>, String> {
            let namespace = if let Some((_, namespace)) = self
                .owner_namespaces
                .iter()
                .find(|(owner, _)| *owner == machine)
            {
                *namespace
            } else {
                let namespace = crate::symbols::owned_conformance_namespace(self.program, machine)?;
                self.owner_namespaces.push((machine, namespace));
                namespace
            };
            if !namespace.is_valid() {
                return Ok(None);
            }
            let conformance = self.prepared_conformance(namespace)?;
            let mut selected = conformance
                .plans
                .iter()
                .filter(|plan| plan.machine == machine);
            let plan = selected
                .next()
                .ok_or("source call has no owned implementation body")?;
            if selected.next().is_some() {
                return Err("source call has ambiguous owned implementation bodies".into());
            }
            Ok(plan.binding(site))
        }

        /*
            A named map selects a public requirement, while its exact row supplies
            the separate body demand. Keep that source identity before and after
            executable routing, including calls in constant initializers.
        */
        pub(crate) fn named_binding(
            &mut self,
            site: &super::body_calls::CallSite<'_>,
            static_arguments: &[StaticMachineArgument],
        ) -> Result<Option<ResolvedAuthoredCallBinding>, String> {
            let program = self.program;
            let target = program.symbols.get(site.target_symbol);
            let parent_kind = program.symbols.get(target.parent).kind;
            let (implementation, explicit_requirement) =
                if program.symbols.get(site.receiver_symbol).kind == SymbolKind::Conformance {
                    (site.receiver_symbol, None)
                } else if target.kind == SymbolKind::State && parent_kind == SymbolKind::Trait {
                    let Some(argument) = static_arguments.last() else {
                        return Ok(None);
                    };
                    if program.symbols.get(argument.symbol).kind != SymbolKind::Conformance {
                        return Ok(None);
                    }
                    (argument.symbol, Some(site.target_symbol))
                } else {
                    return Ok(None);
                };
            let rows = &self.prepared_conformance(implementation)?.rows;
            let requirement = if let Some(requirement) = explicit_requirement {
                requirement
            } else {
                named_requirement(
                    rows.iter()
                        .map(|row| (&row.requirement_name, row.requirement)),
                    site.member.as_str(),
                )
                .map_err(|reason| {
                    format!(
                        "named implementation `{}` {reason} for `{}`",
                        program.symbols.display_path(implementation, "::"),
                        site.member,
                    )
                })?
            };
            let mut matches = rows.iter().filter(|row| row.requirement == requirement);
            let selected = matches
                .next()
                .map(|row| row.realization_state)
                .filter(|state| state.is_valid())
                .unwrap_or(requirement);
            if matches.next().is_some() {
                return Err(
                    "named implementation call has ambiguous exact requirement rows".into(),
                );
            }
            Ok(Some(ResolvedAuthoredCallBinding {
                selected,
                original: requirement,
            }))
        }

        fn prepared_conformance(
            &mut self,
            namespace: SymbolHandle,
        ) -> Result<&PreparedConformanceRouting, String> {
            if !self
                .conformances
                .iter()
                .any(|entry| entry.namespace == namespace)
            {
                self.prepare_namespace(namespace)?;
            }
            self.conformances
                .iter()
                .find(|entry| entry.namespace == namespace)
                .ok_or_else(|| "source call has no prepared implementation declaration".into())
        }

        fn prepare_namespace(&mut self, namespace: SymbolHandle) -> Result<(), String> {
            let mut conformances = self
                .program
                .conformances
                .iter()
                .filter(|candidate| candidate.symbol == namespace);
            let conformance = conformances
                .next()
                .ok_or("source call has no owning implementation declaration")?;
            if conformances.next().is_some() {
                return Err("source call has ambiguous owning implementation declarations".into());
            }
            let program = self.program;
            let (trait_catalog, machine_catalog) = self.catalogs.get_or_insert_with(|| {
                (build_trait_catalog(program), build_machine_catalog(program))
            });
            let rows = normalize_one(
                self.program,
                conformance
                    .alias
                    .as_ref()
                    .map_or("<unnamed implementation>", DiagnosticName::as_str),
                conformance.trait_name.as_str(),
                conformance.trait_symbol,
                &conformance.rows,
                trait_catalog,
                machine_catalog,
            )
            .map_err(|diagnostic| diagnostic.message)?;
            let plans = conformance_routing_plans(self.program, conformance, &rows, trait_catalog);
            self.conformances.push(PreparedConformanceRouting {
                namespace,
                rows,
                plans,
            });
            Ok(())
        }
    }

    fn routed_binding(
        current: SymbolHandle,
        target_name: &DiagnosticName,
        argument_count: usize,
        routes_by_name: bool,
        routes: &[RequirementRoute],
    ) -> Option<ResolvedAuthoredCallBinding> {
        let static_routes = || {
            routes
                .iter()
                .filter(move |route| argument_count != route.parameter_count + 1)
        };
        if current.is_valid()
            && let Some(route) = static_routes().find(|route| route.requirement == current)
        {
            return Some(ResolvedAuthoredCallBinding {
                selected: route.realization_state,
                original: current,
            });
        }
        if !routes_by_name {
            return None;
        }
        let mut matching =
            static_routes().filter(|route| route.requirement_name.as_str() == target_name.as_str());
        let route = matching.next()?;
        matching
            .next()
            .is_none()
            .then_some(ResolvedAuthoredCallBinding {
                selected: route.realization_state,
                original: route.realization_state,
            })
    }

    fn build_trait_catalog(program: &SymbolResolvedTrees) -> Vec<TraitCatalogEntry> {
        program
            .traits
            .iter()
            .map(|trait_definition| TraitCatalogEntry {
                symbol: trait_definition.symbol,
                parents: program
                    .trait_requirements(trait_definition.requires)
                    .iter()
                    .map(|parent| parent.symbol)
                    .collect(),
                requirements: program
                    .trait_machine_signatures(trait_definition.machines)
                    .iter()
                    .enumerate()
                    .map(|(ordinal, requirement)| RequirementCatalogEntry {
                        ordinal,
                        declaring_trait: trait_definition.symbol,
                        declaring_trait_name: trait_definition.name.clone(),
                        requirement: requirement.symbol,
                        requirement_name: requirement.name.clone(),
                        is_default: requirement.is_default,
                        signature: requirement.clone(),
                    })
                    .collect(),
            })
            .collect()
    }

    fn build_machine_catalog(program: &SymbolResolvedTrees) -> Vec<MachineCatalogEntry> {
        program
            .machines
            .iter()
            .enumerate()
            .filter(|(_, machine)| machine.target.is_none())
            .map(|(ordinal, machine)| MachineCatalogEntry {
                ordinal,
                symbol: machine.symbol,
                name: machine.name.clone(),
                states: program
                    .machine_state_handles(machine.states)
                    .iter()
                    .map(|handle| program.machine_state(*handle).clone())
                    .collect(),
            })
            .collect()
    }

    fn normalize_one(
        program: &SymbolResolvedTrees,
        subject_name: &str,
        trait_name: &str,
        trait_symbol: SymbolHandle,
        authored_rows: &[ConformanceRow],
        trait_catalog: &[TraitCatalogEntry],
        machine_catalog: &[MachineCatalogEntry],
    ) -> Result<Vec<ConformanceRow>, Diagnostic> {
        let root = trait_catalog
        .iter()
        .find(|entry| entry.symbol == trait_symbol)
        .ok_or_else(|| {
            Diagnostic::error(format!(
                "closed conformance `{subject_name} satisfies {trait_name}` names unknown trait `{trait_name}`"
            ))
        })?;
        let mut requirements = Vec::new();
        collect_requirement_closure(
            root.symbol,
            trait_catalog,
            &mut Vec::new(),
            &mut requirements,
        );

        let mut normalized = Vec::new();
        for authored in authored_rows
            .iter()
            .filter(|row| row.source != ConformanceRowSource::TraitDefault)
            .chain(
                authored_rows
                    .iter()
                    .filter(|row| row.source == ConformanceRowSource::TraitDefault),
            )
        {
            let inherited_qualifiers =
                (authored.source == ConformanceRowSource::Inline).then(|| {
                    let member = authored
                        .provisional_realization_ordinal
                        .filter(|ordinal| *ordinal < program.machines.len())
                        .map(|ordinal| &program.machines[ordinal])
                        .or_else(|| {
                            program
                                .machines
                                .iter()
                                .find(|machine| machine.symbol == authored.realization_machine)
                        });
                    let qualifiers = member
                        .into_iter()
                        .flat_map(|machine| program.machine_trait_conformances(machine.satisfies))
                        .filter(|edge| {
                            edge.requirement.as_ref() == Some(&authored.requirement_name)
                        })
                        .map(|edge| edge.symbol)
                        .collect::<Vec<_>>();
                    let inherited = qualifiers
                        .iter()
                        .copied()
                        .filter(|qualifier| {
                            requirements.iter().any(|requirement| {
                                requirement.declaring_trait == *qualifier
                                    && requirement.requirement_name == authored.requirement_name
                            })
                        })
                        .collect::<Vec<_>>();
                    // A member's own `satisfies Trait::name` names the slot
                    // it fills. When every such qualifier names a trait
                    // outside this implementation's requirement closure, the
                    // member fills no slot here; an empty list would instead
                    // read as unqualified and let an equal name substitute a
                    // different requirement owner.
                    if inherited.is_empty() {
                        qualifiers
                    } else {
                        inherited
                    }
                });
            let candidates = requirements
                .iter()
                .filter(|instance| {
                    let requirement = *instance;
                    requirement.requirement_name == authored.requirement_name
                        && inherited_qualifiers.as_ref().map_or_else(
                            || {
                                authored.declaring_trait_name.as_str().is_empty()
                                    || requirement.declaring_trait_name
                                        == authored.declaring_trait_name
                                    || program
                                        .symbols
                                        .display_path(requirement.declaring_trait, "::")
                                        == authored.declaring_trait_name.as_str()
                            },
                            |qualifiers| {
                                qualifiers.is_empty()
                                    || qualifiers.contains(&requirement.declaring_trait)
                            },
                        )
                        && authored
                            .provisional_requirement_ordinal
                            .is_none_or(|ordinal| requirement.ordinal == ordinal)
                })
                .collect::<Vec<_>>();
            let realizations = resolve_realizations(program, authored, machine_catalog, None);
            let mut matching = Vec::new();
            for candidate in &candidates {
                let requirement_shape =
                    result_dispatch_shape(program, candidate.signature.return_type.as_ref());
                for (machine, state) in resolve_realizations(
                    program,
                    authored,
                    machine_catalog,
                    Some(&requirement_shape),
                ) {
                    matching.push((*candidate, machine, state));
                }
            }
            let (requirement, machine, state) = match (
                candidates.as_slice(),
                realizations.as_slice(),
                matching.as_slice(),
            ) {
                ([], _, _) => {
                    let qualified = if authored.declaring_trait_name.as_str().is_empty() {
                        authored.requirement_name.as_str().to_owned()
                    } else {
                        format!(
                            "{}::{}",
                            authored.declaring_trait_name, authored.requirement_name
                        )
                    };
                    return Err(Diagnostic::error(format!(
                        "closed conformance `{subject_name} satisfies {trait_name}` has no inherited requirement slot `{qualified}`"
                    )));
                }
                ([requirement], [(machine, state)], _) => (*requirement, *machine, *state),
                (_, _, [(requirement, machine, state)]) => (*requirement, *machine, *state),
                _ => {
                    let inherited_collision = authored.declaring_trait_name.as_str().is_empty()
                        && candidates.first().is_some_and(|first| {
                            candidates
                                .iter()
                                .skip(1)
                                .any(|candidate| candidate.declaring_trait != first.declaring_trait)
                        });
                    if inherited_collision {
                        return Err(Diagnostic::error(format!(
                            "closed conformance `{subject_name} satisfies {trait_name}` member `{}` is ambiguous across inherited traits; use `DeclaringTrait::{}`",
                            authored.requirement_name, authored.requirement_name
                        )));
                    }
                    return Err(Diagnostic::error(format!(
                        "closed conformance `{subject_name} satisfies {trait_name}` member `{}` does not identify one exact inherited overload; qualify the declaring trait and match the complete parameter/result-domain signature",
                        authored.requirement_name
                    )));
                }
            };

            if normalized.iter().any(|row: &ConformanceRow| {
                row.declaring_trait == requirement.declaring_trait
                    && row.requirement == requirement.requirement
            }) {
                if authored.source == ConformanceRowSource::TraitDefault {
                    continue;
                }
                return Err(Diagnostic::error(format!(
                    "closed conformance `{subject_name} satisfies {trait_name}` fills `{}::{}` more than once",
                    requirement.declaring_trait_name, requirement.requirement_name
                )));
            }

            let mut row = authored.clone();
            row.declaring_trait = requirement.declaring_trait;
            row.declaring_trait_name = requirement.declaring_trait_name.clone();
            row.requirement = requirement.requirement;
            row.provisional_requirement_ordinal = None;
            row.realization_machine = machine.symbol;
            row.realization_state = state.symbol;
            row.provisional_realization_ordinal = None;
            normalized.push(row);
        }

        for requirement in requirements {
            if normalized.iter().any(|row| {
                row.declaring_trait == requirement.declaring_trait
                    && row.requirement == requirement.requirement
            }) {
                continue;
            }
            if !requirement.is_default {
                return Err(Diagnostic::error(format!(
                    "closed conformance `{subject_name} satisfies {trait_name}` is incomplete: missing `{}::{}`",
                    requirement.declaring_trait_name, requirement.requirement_name
                )));
            }
            normalized.push(ConformanceRow {
                declaring_trait: requirement.declaring_trait,
                declaring_trait_name: requirement.declaring_trait_name.clone(),
                requirement: requirement.requirement,
                requirement_name: requirement.requirement_name.clone(),
                provisional_requirement_ordinal: None,
                realization_machine: SymbolHandle::invalid(),
                realization_state: SymbolHandle::invalid(),
                realization_name: DiagnosticName::generated(format!(
                    "{}::{}#default",
                    requirement.declaring_trait_name, requirement.requirement_name
                )),
                authored_realization_source_span: None,
                provisional_realization_ordinal: None,
                source: ConformanceRowSource::TraitDefault,
            });
        }

        normalized.sort_by_key(|row| {
            (
                row.declaring_trait.arena_index(),
                row.requirement.arena_index(),
            )
        });
        Ok(normalized)
    }

    fn collect_requirement_closure(
        trait_symbol: SymbolHandle,
        catalog: &[TraitCatalogEntry],
        visited: &mut Vec<SymbolHandle>,
        output: &mut Vec<RequirementCatalogEntry>,
    ) {
        if !trait_symbol.is_valid() || visited.contains(&trait_symbol) {
            return;
        }
        visited.push(trait_symbol);
        let Some(entry) = catalog.iter().find(|entry| entry.symbol == trait_symbol) else {
            return;
        };
        for requirement in &entry.requirements {
            if !output.iter().any(|existing| {
                existing.declaring_trait == requirement.declaring_trait
                    && existing.requirement == requirement.requirement
            }) {
                output.push(requirement.clone());
            }
        }
        for parent in &entry.parents {
            collect_requirement_closure(*parent, catalog, visited, output);
        }
    }

    fn result_dispatch_shape(
        program: &SymbolResolvedTrees,
        return_type: Option<&TypeReference>,
    ) -> String {
        let mut result_dispatch = Vec::new();
        if let Some(return_type) = return_type {
            collect_result_dispatch(program, return_type, &mut result_dispatch, &mut Vec::new());
        }
        result_dispatch.sort();
        result_dispatch.dedup();
        result_dispatch.join("&")
    }

    fn collect_result_dispatch(
        program: &SymbolResolvedTrees,
        type_reference: &TypeReference,
        terms: &mut Vec<String>,
        alias_stack: &mut Vec<SymbolHandle>,
    ) {
        match type_reference {
            TypeReference::AliasUse { expansion, .. } => {
                collect_result_dispatch(program, expansion, terms, alias_stack);
            }
            TypeReference::Reference(reference) => collect_result_dispatch(
                program,
                program.child_type_reference(reference.referee),
                terms,
                alias_stack,
            ),
            TypeReference::Constrained(constrained) => {
                collect_result_dispatch(
                    program,
                    program.child_type_reference(constrained.base_type),
                    terms,
                    alias_stack,
                );
                for constraint in program
                    .tables
                    .types
                    .constraints
                    .span_or_empty(constrained.constraints)
                {
                    match constraint {
                        TypeConstraint::ArithmeticDomain(domain) => {
                            terms.push(format!("arithmetic:{}", domain.name()));
                        }
                        TypeConstraint::Domain(domain) => {
                            let symbol = program
                                .symbols
                                .find_top_level_by_name_and_kinds_from_source(
                                    domain.name.as_str(),
                                    &[symbols::SymbolKind::Domain],
                                    domain.name.source_span(),
                                )
                                .unwrap_or_else(SymbolHandle::invalid);
                            collect_declared_result_dispatch(
                                program,
                                symbol,
                                domain.name.as_str(),
                                terms,
                                alias_stack,
                            );
                        }
                        TypeConstraint::Named(_) => {}
                    }
                }
            }
            super::leaf_carrier_type_reference_pattern!() => {}
        }
    }

    fn collect_declared_result_dispatch(
        program: &SymbolResolvedTrees,
        symbol: SymbolHandle,
        name: &str,
        terms: &mut Vec<String>,
        alias_stack: &mut Vec<SymbolHandle>,
    ) {
        let Some(definition) = program.domain_definition_by_symbol(symbol) else {
            terms.push(format!("declared:{name}"));
            return;
        };
        if alias_stack.contains(&definition.symbol) {
            return;
        }
        if let Some(alias) = definition.alias.as_ref() {
            alias_stack.push(definition.symbol);
            for constituent in &alias.constituents {
                let constituent_name = program
                    .domain_definition_by_symbol(constituent.domain_symbol)
                    .map(|candidate| candidate.name.as_str())
                    .unwrap_or("<unresolved-domain>");
                collect_declared_result_dispatch(
                    program,
                    constituent.domain_symbol,
                    constituent_name,
                    terms,
                    alias_stack,
                );
            }
            alias_stack.pop();
            return;
        }
        if definition.predicate_presence.is_present()
            && definition.semantic_roles.is_empty()
            && definition.establishment_routes.is_empty()
            && definition.access_policy.is_none()
        {
            return;
        }
        terms.push(format!("declared:{}", definition.name));
    }

    fn resolve_realizations<'catalog>(
        program: &SymbolResolvedTrees,
        row: &ConformanceRow,
        catalog: &'catalog [MachineCatalogEntry],
        requirement_shape: Option<&str>,
    ) -> Vec<(&'catalog MachineCatalogEntry, &'catalog State)> {
        let mut candidates = catalog
            .iter()
            .filter(|machine| {
                (!row.realization_machine.is_valid() || machine.symbol == row.realization_machine)
                    && row
                        .provisional_realization_ordinal
                        .is_none_or(|ordinal| machine.ordinal == ordinal)
            })
            .filter_map(|machine| {
                if row.realization_state.is_valid() {
                    return machine
                        .states
                        .iter()
                        .find(|state| state.symbol == row.realization_state)
                        .map(|state| (machine, state));
                }
                let leaf = machine
                    .name
                    .as_str()
                    .rsplit_once("::")
                    .map_or(machine.name.as_str(), |(_, leaf)| leaf);
                machine
                    .states
                    .iter()
                    .find(|state| state.name.as_str() == leaf)
                    .or_else(|| {
                        machine
                            .states
                            .iter()
                            .find(|state| state.name.as_str() == "entry")
                    })
                    .map(|state| (machine, state))
            })
            .filter(|(_, state)| {
                requirement_shape.is_none_or(|shape| {
                    result_dispatch_shape(program, state.return_type.as_ref()) == shape
                })
            })
            .collect::<Vec<_>>();
        if row.realization_machine.is_valid() || row.provisional_realization_ordinal.is_some() {
            return candidates;
        }
        if let Some(reference) = row.authored_realization_source_span {
            let selected = program
                .symbols
                .lookup_signature_free_top_level_from_source_matching(
                    row.realization_name.as_str(),
                    &[SymbolKind::Machine],
                    reference,
                    |symbol| {
                        candidates
                            .iter()
                            .any(|(machine, _)| machine.symbol == symbol)
                    },
                );
            candidates.retain(|(machine, _)| match selected {
                SymbolLookup::Unique(symbol) => machine.symbol == symbol,
                SymbolLookup::Ambiguous { first, second } => {
                    machine.symbol == first || machine.symbol == second
                }
                SymbolLookup::NotFound => false,
            });
        } else {
            candidates.retain(|(machine, _)| machine.name == row.realization_name);
        }
        candidates
    }
}
pub(crate) mod domain_establishment {
    /*
        Domain establishment routes: which requirement or machine may issue a domain.

        This is the sole projection point for route sources. Checked consumers
        read these identities instead of reconstructing owner authority from
        attachment names or contract placement. Public catalog references retain
        interface exposure but have their own authorization occurrence kind: they
        do not publish a private issuer for ordinary consumer selection.
    */

    use crate::tables::body_expressions;
    use diagnostics::Diagnostic;
    use language_semantics::DomainEstablishmentRoute;
    use language_semantics::declaration_selection::{
        AuthoredDeclarationSelectionExposure as Exposure,
        AuthoredDeclarationSelectionKind as SelectionKind,
    };
    use source::{SourceSpan, Span};
    use symbol_resolved_trees::SymbolResolvedTrees;
    use symbol_resolved_trees::domain::ProofFact;
    use symbol_resolved_trees::expression::ExpressionNode;
    use symbol_resolved_trees::name::DiagnosticName;
    use symbol_resolved_trees::signature::{SignatureContract, SignatureContractKind};
    use symbol_resolved_trees::types::TypeReference;
    use symbols::JoinedPath;
    use symbols::SymbolHandle;

    use crate::selection::signature_free_requirements::{
        SignatureFreeMachineResolutionError, SignatureFreeRequirementResolutionError,
        resolve_signature_free_machine, resolve_signature_free_requirement,
    };
    use crate::symbols::prefer_module_local_domain;
    use language_core::ReferenceAccess;
    use symbol_resolved_trees::data::DataMember;
    use symbol_resolved_trees::expression::ExpressionHandle;

    #[derive(Debug, Clone)]
    struct AuthoredRouteResolution {
        domain_symbol: SymbolHandle,
        route: DomainEstablishmentRoute,
        /*
            `(source span, selection kind, selected symbol)` records the authored
            path's segment resolutions: requirement routes retain their trait
            prefix and requirement leaf; a machine route retains its optional
            data-owner prefix plus the machine leaf.
        */
        selections: Vec<(SourceSpan, SelectionKind, SymbolHandle)>,
        exposure: Exposure,
    }

    /*
        Normalize authored domain-introduction relationships after every
        declaration and contract fact has a symbol.

        This is the sole projection point for the currently landed route sources.
        Checked consumers consult these identities instead of reconstructing owner
        authority from attachment names or contract placement.
    */
    pub(crate) fn normalize_domain_establishment_routes(
        program: &mut SymbolResolvedTrees,
        retained: Option<&SymbolResolvedTrees>,
    ) -> Result<(), Diagnostic> {
        let mut resolutions = Vec::new();
        collect_authored_requirement_routes(program, retained, &mut resolutions)?;
        let policies = select_access_policies(program, retained)?;

        program.domain_definitions.for_each_mut(|domain| {
            if retained
                .is_some_and(|base| base.domain_definition_by_symbol(domain.symbol).is_some())
            {
                return;
            }
            if let Some(policy) = &mut domain.access_policy
                && let Some((_, implementation)) =
                    policies.iter().find(|(owner, _)| *owner == domain.symbol)
            {
                policy.symbol = *implementation;
            }
            domain.establishment_routes.clear();
            for resolution in &resolutions {
                if resolution.domain_symbol == domain.symbol
                    && !domain.establishment_routes.contains(&resolution.route)
                {
                    domain.establishment_routes.push(resolution.route);
                }
            }
        });

        for resolution in resolutions {
            for (source_span, kind, symbol) in resolution.selections {
                record_route_selection_once(
                    program,
                    source_span,
                    resolution.exposure,
                    kind,
                    symbol,
                )?;
            }
        }
        Ok(())
    }

    /*
        `access Policy` names one named implementation (`Policy satisfies
        AccessPolicy { .. }`) by ordinary namespace selection, as a route names
        one machine: access.md, "The `access` clause selects one ordinary named
        policy implementation". Which trait that implementation satisfies is
        checked by whoever evaluates its plan; nothing here reads the plan.
    */
    fn select_access_policies(
        program: &SymbolResolvedTrees,
        retained: Option<&SymbolResolvedTrees>,
    ) -> Result<Vec<(SymbolHandle, SymbolHandle)>, Diagnostic> {
        let mut selected = Vec::new();
        for domain in &program.domain_definitions {
            if retained
                .is_some_and(|base| base.domain_definition_by_symbol(domain.symbol).is_some())
            {
                continue;
            }
            let Some(policy) = &domain.access_policy else {
                continue;
            };
            let rendered = policy.path.joined_path();
            let use_span = policy
                .path
                .last()
                .expect("an access policy path is nonempty")
                .source_span();
            if domain.alias.is_some() {
                return Err(Diagnostic::error(format!(
                    "domain alias `{}` cannot select an access policy; a policy belongs to its atomic declarations",
                    domain.name
                ))
                .with_source_span(use_span));
            }
            let lookup = program
                .symbols
                .lookup_signature_free_top_level_from_source_matching(
                    &rendered,
                    &[symbols::SymbolKind::Conformance],
                    use_span,
                    |_| true,
                );
            match lookup {
                symbols::SymbolLookup::Unique(implementation) => {
                    selected.push((domain.symbol, implementation));
                }
                symbols::SymbolLookup::NotFound => {
                    return Err(Diagnostic::error(format!(
                        "domain `{}` access policy `{rendered}` names no implementation; write `{rendered} satisfies AccessPolicy {{ ... }}` or import the one meant",
                        domain.name
                    ))
                    .with_source_span(use_span));
                }
                symbols::SymbolLookup::Ambiguous { .. } => {
                    return Err(Diagnostic::error(format!(
                        "domain `{}` access policy `{rendered}` names more than one implementation",
                        domain.name
                    ))
                    .with_source_span(use_span));
                }
            }
        }
        Ok(selected)
    }

    fn record_route_selection_once(
        program: &mut SymbolResolvedTrees,
        source_span: SourceSpan,
        exposure: Exposure,
        kind: SelectionKind,
        symbol: SymbolHandle,
    ) -> Result<(), Diagnostic> {
        use symbol_resolved_trees::AuthoredDeclarationSelectionTarget;

        let retained = program
            .authored_declaration_selections()
            .iter()
            .any(|selection| {
                selection.source_span() == source_span
                    && selection.exposure() == exposure
                    && selection.kind() == kind
                    && matches!(
                        selection.target(),
                        AuthoredDeclarationSelectionTarget::Resolved(target)
                            if target.selected_symbol() == symbol
                    )
            });
        if !retained {
            program
            .record_resolved_authored_declaration_selection(source_span, exposure, kind, symbol)
            .map_err(|error| {
                Diagnostic::error(format!(
                    "failed to retain domain establishment-route declaration selection: {error:?}"
                ))
            })?;
        }
        Ok(())
    }

    fn collect_authored_requirement_routes(
        program: &SymbolResolvedTrees,
        retained: Option<&SymbolResolvedTrees>,
        resolutions: &mut Vec<AuthoredRouteResolution>,
    ) -> Result<(), Diagnostic> {
        for domain in &program.domain_definitions {
            if retained
                .is_some_and(|base| base.domain_definition_by_symbol(domain.symbol).is_some())
            {
                continue;
            }
            if domain.alias.is_some() && !domain.authored_routes.is_empty() {
                return Err(Diagnostic::error(format!(
                    "domain alias `{}` cannot author establishment routes; routes belong to its atomic declarations",
                    domain.name
                )));
            }
            for path in &domain.authored_routes {
                let rendered = path.joined_path();
                let requirement = resolve_signature_free_requirement(program, path);
                let machine = resolve_signature_free_machine(program, path);
                let resolved = match (requirement, machine) {
                (Ok(_), Ok(machine)) => {
                    return Err(Diagnostic::error(format!(
                        "domain `{}` establishment route `{rendered}` is ambiguous across declaration kinds: it names both a trait requirement and machine `{}`",
                        domain.name,
                        machine.machine.name
                    )));
                }
                (Err(_), Ok(machine)) => {
                    if !machine_authorizes_domain_subject(program, machine.machine, domain.symbol)? {
                        return Err(Diagnostic::error(format!(
                            "domain `{}` authorizes `{rendered}` but that machine does not name the domain on its exact result",
                            domain.name
                        )));
                    }
                    let mut selections = Vec::new();
                    if machine.machine.attached_data_symbol.is_valid() {
                        selections.push((
                            path_source_span(&path[..path.len() - 1]),
                            SelectionKind::DomainIssuerAuthorization,
                            machine.machine.attached_data_symbol,
                        ));
                    }
                    selections.push((
                        path.last().expect("route path nonempty").source_span(),
                        SelectionKind::DomainIssuerAuthorization,
                        machine.machine.symbol,
                    ));
                    resolutions.push(AuthoredRouteResolution {
                        domain_symbol: domain.symbol,
                        route: DomainEstablishmentRoute::ExactMachine {
                            machine: machine.machine.symbol,
                        },
                        selections,
                        exposure: if domain.is_public {
                            Exposure::PublicInterface
                        } else {
                            Exposure::PrivateImplementation
                        },
                    });
                    continue;
                }
                (resolved, Err(machine_error)) => resolved.map_err(|error| match error {
                    SignatureFreeRequirementResolutionError::InvalidPath => Diagnostic::error(
                        format!(
                            "domain `{}` establishment route must name an exact `Trait::requirement` or one exact machine declaration",
                            domain.name
                        ),
                    ),
                    SignatureFreeRequirementResolutionError::TraitNotUnique => {
                        Diagnostic::error(format!(
                            "domain `{}` establishment route `{rendered}` does not resolve to one exact trait",
                            domain.name
                        ))
                    }
                    SignatureFreeRequirementResolutionError::RequirementNotUnique => {
                        match machine_error {
                            SignatureFreeMachineResolutionError::NotUnique => Diagnostic::error(format!(
                                "domain `{}` establishment route `{rendered}` resolves to neither one exact trait requirement nor one exact machine declaration",
                                domain.name
                            )),
                        }
                    }
                })?,
            };
                let trait_definition = resolved.declaring_trait;
                let requirement = resolved.requirement;
                if !requirement_authorizes_domain_subject(
                    program,
                    requirement,
                    domain.symbol,
                    trait_definition.is_boundary,
                )? {
                    return Err(Diagnostic::error(format!(
                        "domain `{}` authorizes `{}` but that requirement does not name the domain on its exact result or an exact non-self external-root parameter",
                        domain.name,
                        path.joined_path()
                    )));
                }
                let route = if trait_definition.is_boundary {
                    DomainEstablishmentRoute::BoundaryRequirement {
                        boundary_trait: trait_definition.symbol,
                        requirement: requirement.symbol,
                    }
                } else {
                    DomainEstablishmentRoute::CheckedRequirement {
                        trait_definition: trait_definition.symbol,
                        requirement: requirement.symbol,
                    }
                };
                let [trait_path @ .., requirement_name] = path.as_slice() else {
                    unreachable!("resolved establishment route has a trait and requirement")
                };
                resolutions.push(AuthoredRouteResolution {
                    domain_symbol: domain.symbol,
                    route,
                    selections: vec![
                        (
                            path_source_span(trait_path),
                            SelectionKind::DomainIssuerAuthorization,
                            resolved.trait_definition.symbol,
                        ),
                        (
                            requirement_name.source_span(),
                            SelectionKind::DomainIssuerAuthorization,
                            route.established_declaration(),
                        ),
                    ],
                    exposure: if domain.is_public {
                        Exposure::PublicInterface
                    } else {
                        Exposure::PrivateImplementation
                    },
                });
            }
        }
        Ok(())
    }

    /*
        An exact-machine route is authorized only when the named machine's own
        invocation produces the domain subject: its `ensures` memberships on
        `result` or a state return type that names the domain. Parameter subjects
        stay boundary-requirement-only; a machine route cannot borrow requirement
        authority it does not declare.
    */
    fn machine_authorizes_domain_subject(
        program: &SymbolResolvedTrees,
        machine: &symbol_resolved_trees::machine::Machine,
        domain_symbol: SymbolHandle,
    ) -> Result<bool, Diagnostic> {
        if ensured_result_domain_symbols(program, program.machine_contracts(machine))
            .contains(&domain_symbol)
        {
            return Ok(true);
        }
        for state_handle in program.machine_state_handles(machine.states) {
            let state = program.machine_state(*state_handle);
            if let Some(return_type) = &state.return_type
                && type_reference_domain_symbols(program, return_type)?.contains(&domain_symbol)
            {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn path_source_span(path: &[DiagnosticName]) -> SourceSpan {
        let first = path
            .first()
            .expect("resolved establishment route has a trait path")
            .source_span();
        let last = path
            .last()
            .expect("resolved establishment route has a trait path")
            .source_span();
        if first.source_id == last.source_id {
            SourceSpan::new(first.source_id, Span::new(first.span.start, last.span.end))
        } else {
            first
        }
    }

    fn requirement_authorizes_domain_subject(
        program: &SymbolResolvedTrees,
        requirement: &symbol_resolved_trees::signature::StateSignature,
        domain_symbol: SymbolHandle,
        permits_external_root_parameters: bool,
    ) -> Result<bool, Diagnostic> {
        if ensured_result_domain_symbols(
            program,
            program.signature_contracts(requirement.contracts),
        )
        .contains(&domain_symbol)
            || ensured_mutable_parameter_domain_symbols(program, requirement)
                .contains(&domain_symbol)
        {
            return Ok(true);
        }
        if let Some(return_type) = &requirement.return_type
            && type_reference_domain_symbols(program, return_type)?.contains(&domain_symbol)
        {
            return Ok(true);
        }
        if permits_external_root_parameters {
            for parameter in program.state_parameters(requirement.parameters) {
                if type_reference_domain_symbols(program, &parameter.type_reference)?
                    .contains(&domain_symbol)
                {
                    return Ok(true);
                }
            }
        }
        Ok(false)
    }

    fn type_reference_domain_symbols(
        program: &SymbolResolvedTrees,
        type_reference: &TypeReference,
    ) -> Result<Vec<SymbolHandle>, Diagnostic> {
        let mut domains = Vec::new();
        let mut subjects = DomainSubjectTraversal {
            bindings: Vec::new(),
            ancestors: Vec::new(),
            remaining: 4096,
        };
        collect_type_reference_domain_symbols(
            program,
            BoundDomainSubject {
                reference: type_reference,
                scope: 0,
            },
            DomainSubjectPosition::Root,
            &mut subjects,
            &mut domains,
        )?;
        Ok(domains)
    }

    /*
        A generic argument is interpreted in its caller's environment, not in the
        template being entered. Keeping that prefix is essential for Box<Box<T>>:
        both applications bind the same formal symbol but to different subjects.
    */
    #[derive(Clone, Copy, PartialEq, Eq)]
    struct BoundDomainSubject<'program> {
        reference: &'program TypeReference,
        scope: usize,
    }

    struct DomainSubjectTraversal<'program> {
        bindings: Vec<(SymbolHandle, BoundDomainSubject<'program>)>,
        ancestors: Vec<(SymbolHandle, usize)>,
        remaining: usize,
    }

    #[derive(Clone, Copy)]
    enum DomainSubjectPosition {
        Root,
        OwnedMember,
    }

    /*
        A requirement result or external-root parameter "names" a domain when the
        qualification sits on the type itself or on an owned member position of a
        carrier it reaches: record fields, case payloads, and fixed-array elements
        all carry their declared constraints. The checked side grants those
        case-scoped positions the same way
        (`declared_owned_field_domain_identities`), so the authorization surface
        must traverse them here or `established by` can never route a sum result.
    */
    fn collect_type_reference_domain_symbols<'program>(
        program: &'program SymbolResolvedTrees,
        subject: BoundDomainSubject<'program>,
        position: DomainSubjectPosition,
        traversal: &mut DomainSubjectTraversal<'program>,
        domains: &mut Vec<SymbolHandle>,
    ) -> Result<(), Diagnostic> {
        if traversal.remaining == 0 || traversal.ancestors.len() >= 64 {
            return Err(Diagnostic::error(
                "domain establishment subject exceeds the bounded generic member traversal; recursive expansion cannot establish a route",
            ));
        }
        traversal.remaining -= 1;
        let child = |reference| BoundDomainSubject {
            reference,
            scope: subject.scope,
        };
        let constrained = match subject.reference {
            TypeReference::AliasUse { expansion, .. } => {
                traversal.remaining += 1;
                return collect_type_reference_domain_symbols(
                    program,
                    child(expansion),
                    position,
                    traversal,
                    domains,
                );
            }
            TypeReference::Reference(reference) => {
                if matches!(position, DomainSubjectPosition::OwnedMember) {
                    return Ok(());
                }
                return collect_type_reference_domain_symbols(
                    program,
                    child(program.child_type_reference(reference.referee)),
                    position,
                    traversal,
                    domains,
                );
            }
            TypeReference::Constrained(constrained) => constrained,
            TypeReference::Named { symbol, .. } => {
                if let Some((_, argument)) = traversal.bindings[..subject.scope]
                    .iter()
                    .rev()
                    .find(|(formal, _)| formal == symbol)
                {
                    return collect_type_reference_domain_symbols(
                        program, *argument, position, traversal, domains,
                    );
                }
                return collect_member_domain_symbols(
                    program,
                    *symbol,
                    subject.scope,
                    traversal,
                    domains,
                );
            }
            TypeReference::FixedArray(array) => {
                return collect_type_reference_domain_symbols(
                    program,
                    child(program.child_type_reference(array.element_type)),
                    position,
                    traversal,
                    domains,
                );
            }
            TypeReference::Generic(application) => {
                let Some(data) = program.data_definition_by_symbol(application.base_symbol) else {
                    return Ok(());
                };
                let parameters = program.data_type_parameters(data.type_parameters);
                let arguments = program.child_type_references(application.arguments);
                if parameters.len() != arguments.len() {
                    return Err(Diagnostic::error(
                        "domain establishment subject has a generic carrier with mismatched type arguments",
                    ));
                }
                let previous_binding_count = traversal.bindings.len();
                for (parameter, argument) in parameters.iter().zip(arguments) {
                    traversal.bindings.push((parameter.symbol, child(argument)));
                }
                let result = collect_member_domain_symbols(
                    program,
                    application.base_symbol,
                    traversal.bindings.len(),
                    traversal,
                    domains,
                );
                traversal.bindings.truncate(previous_binding_count);
                return result;
            }
            TypeReference::Slice(_)
            | TypeReference::ConstExpression(_)
            | TypeReference::Callable { .. }
            | TypeReference::DynamicTrait { .. }
            | TypeReference::Unit => return Ok(()),
        };
        for constraint in program
            .tables
            .types
            .constraints
            .span_or_empty(constrained.constraints)
        {
            let symbol_resolved_trees::types::TypeConstraint::Domain(name) = constraint else {
                continue;
            };
            let matched = program
                .domain_definitions
                .iter()
                .filter(|domain| {
                    program
                        .symbols
                        .source_reference_can_see_symbol(name.name.source_span(), domain.symbol)
                        && crate::symbols::domain_name_reaches(
                            &program.symbols,
                            domain.symbol,
                            domain.name.as_str(),
                            name.name.as_str(),
                            name.name.source_span(),
                        )
                })
                .map(|domain| domain.symbol)
                .collect::<Vec<_>>();
            for matching in
                prefer_module_local_domain(&program.symbols, matched, name.name.source_span())
            {
                for atom in atomic_domain_symbols(program, matching) {
                    if !domains.contains(&atom) {
                        domains.push(atom);
                    }
                }
            }
        }
        collect_type_reference_domain_symbols(
            program,
            child(program.child_type_reference(constrained.base_type)),
            position,
            traversal,
            domains,
        )
    }

    /*
        Domains declared on the owned member positions of a named carrier: record
        fields and case payload fields alike. A payload's qualification is
        conditional on its exact case, but for route authorization the question is
        only whether the domain is named on an actual subject. Generic arguments
        are substituted only when a declared member reaches their formal; scanning
        the argument list itself would authorize phantom, unrepresented subjects.
        This discovers a route, not a grant: checking still establishes each exact
        carrier, selected case, target domain and invocation independently.
    */
    fn collect_member_domain_symbols<'program>(
        program: &'program SymbolResolvedTrees,
        data_symbol: SymbolHandle,
        scope: usize,
        traversal: &mut DomainSubjectTraversal<'program>,
        domains: &mut Vec<SymbolHandle>,
    ) -> Result<(), Diagnostic> {
        if traversal.ancestors.contains(&(data_symbol, scope)) {
            return Ok(());
        }
        let Some(data) = program.data_definition_by_symbol(data_symbol) else {
            return Ok(());
        };
        traversal.ancestors.push((data_symbol, scope));
        for member in program.data_members(data.storage.members) {
            let fields = match member {
                DataMember::Field(field) => std::slice::from_ref(field),
                DataMember::Variant(variant) => program.data_payload_fields(variant.payload),
            };
            for field in fields {
                collect_type_reference_domain_symbols(
                    program,
                    BoundDomainSubject {
                        reference: &field.type_reference,
                        scope,
                    },
                    DomainSubjectPosition::OwnedMember,
                    traversal,
                    domains,
                )?;
            }
        }
        traversal.ancestors.pop();
        Ok(())
    }

    fn ensured_result_domain_symbols(
        program: &SymbolResolvedTrees,
        contracts: &[SignatureContract],
    ) -> Vec<SymbolHandle> {
        let mut domains = Vec::new();
        for contract in contracts
            .iter()
            .filter(|contract| contract.kind == SignatureContractKind::Ensures)
        {
            for fact in program.proof_facts(contract.facts) {
                let ProofFact::Membership(membership) = fact else {
                    continue;
                };
                if !expression_is_bare_result(program, membership.value) {
                    continue;
                }
                for domain_symbol in atomic_domain_symbols(program, membership.domain_symbol) {
                    if domain_symbol.is_valid() && !domains.contains(&domain_symbol) {
                        domains.push(domain_symbol);
                    }
                }
            }
        }
        domains
    }

    /*
        The out-parameter establishment surface: an `ensures` membership that
        names an exact mutable non-self parameter mints the domain into caller
        storage the provider writes, the same authority `ensures result in D`
        carries on the result. Other parameters are inputs — a provider cannot
        establish into them — so they stay caller premises here.
    */
    fn ensured_mutable_parameter_domain_symbols(
        program: &SymbolResolvedTrees,
        requirement: &symbol_resolved_trees::signature::StateSignature,
    ) -> Vec<SymbolHandle> {
        let mut domains = Vec::new();
        for contract in program
            .signature_contracts(requirement.contracts)
            .iter()
            .filter(|contract| contract.kind == SignatureContractKind::Ensures)
        {
            for fact in program.proof_facts(contract.facts) {
                let ProofFact::Membership(membership) = fact else {
                    continue;
                };
                let ExpressionNode::Name(path) =
                    body_expressions(program).expression(membership.value)
                else {
                    continue;
                };
                let [name] = body_expressions(program).name_path_members(path.members) else {
                    continue;
                };
                let names_out_parameter = program
                    .state_parameters(requirement.parameters)
                    .iter()
                    .any(|parameter| {
                        mutable_reference_parameter(program, &parameter.type_reference)
                            && (parameter.name.as_str() == name.as_str()
                                || parameter.symbol == path.symbol
                                || parameter.symbol == path.head_symbol)
                    });
                if !names_out_parameter {
                    continue;
                }
                for domain_symbol in atomic_domain_symbols(program, membership.domain_symbol) {
                    if domain_symbol.is_valid() && !domains.contains(&domain_symbol) {
                        domains.push(domain_symbol);
                    }
                }
            }
        }
        domains
    }

    /*
        Caller-visible mutability: the parameter must borrow caller storage it
        can write (`&mut T`, possibly under constraints). An owned `mut` binding
        dies with the callee — nothing it mints reaches the caller.
    */
    fn mutable_reference_parameter(
        program: &SymbolResolvedTrees,
        type_reference: &TypeReference,
    ) -> bool {
        match type_reference {
            TypeReference::Reference(reference) => {
                reference.storage.access == ReferenceAccess::Mutable
            }
            TypeReference::Constrained(constrained) => mutable_reference_parameter(
                program,
                program.child_type_reference(constrained.storage.base_type),
            ),
            _ => false,
        }
    }

    fn atomic_domain_symbols(
        program: &SymbolResolvedTrees,
        domain_symbol: SymbolHandle,
    ) -> Vec<SymbolHandle> {
        fn expand(
            program: &SymbolResolvedTrees,
            domain_symbol: SymbolHandle,
            stack: &mut Vec<SymbolHandle>,
            output: &mut Vec<SymbolHandle>,
        ) {
            if !domain_symbol.is_valid() || stack.contains(&domain_symbol) {
                return;
            }
            let Some(domain) = domain_definition(program, domain_symbol) else {
                return;
            };
            let Some(alias) = domain.alias.as_ref() else {
                if !output.contains(&domain_symbol) {
                    output.push(domain_symbol);
                }
                return;
            };
            stack.push(domain_symbol);
            for constituent in &alias.constituents {
                expand(program, constituent.domain_symbol, stack, output);
            }
            stack.pop();
        }

        let mut output = Vec::new();
        expand(program, domain_symbol, &mut Vec::new(), &mut output);
        output
    }

    fn domain_definition(
        program: &SymbolResolvedTrees,
        symbol: SymbolHandle,
    ) -> Option<&symbol_resolved_trees::domain::DomainDefinition> {
        program.domain_definition_by_symbol(symbol)
    }

    fn expression_is_bare_result(
        program: &SymbolResolvedTrees,
        expression: ExpressionHandle,
    ) -> bool {
        let ExpressionNode::Name(path) = program.tables.bodies.expressions.expression(expression)
        else {
            return false;
        };
        let [name] = body_expressions(program).name_path_members(path.members) else {
            return false;
        };
        language_core::is_reserved_result(name.as_str())
    }
}
pub(crate) mod domain_operator_homes {
    /*
        Operator homes: a top-level operator moves into its exact domain's
        operator family before symbols are assigned.

        Selection follows module name law. A domain declared in the operator's own
        module outranks same-spelled foreign declarations, qualified paths select
        exactly, and relative spellings reach a foreign domain only through an
        exposing import.
    */

    use std::collections::HashMap;
    use syntax_trees::identifier::JoinedSyntaxPath;

    use arena::{Arena, HandleSpan, OrderedRootArena};
    use diagnostics::Diagnostic;
    use source::SourceId;
    use symbol_resolved_trees::SymbolResolvedTrees;
    use symbol_resolved_trees::domain::DomainDefinition;

    use crate::symbols::NamespaceDeclarations;
    use std::mem::take;
    use symbol_resolved_trees::data::TypeParameterKind;
    use symbol_resolved_trees::operator::OperatorDefinition;
    use symbol_resolved_trees::types::TypeReference;
    use symbols::JoinedPath;
    use symbols::same_semantic_name;

    /*
        Logical module/import custody available before the final symbol table is
        assigned. Fresh sources answer from the collected namespace declarations;
        retained base sources (seeded extension) answer through the base table
        already installed on the program.
    */
    struct NamespaceScope {
        source_modules: HashMap<SourceId, String>,
        source_imports: HashMap<SourceId, Vec<String>>,
    }

    impl NamespaceScope {
        fn collect(namespaces: &NamespaceDeclarations) -> Self {
            let mut source_modules = HashMap::new();
            for path in namespaces.module_paths() {
                let Some(first) = path.first() else {
                    continue;
                };
                source_modules.insert(
                    first.source_span().source_id,
                    path.joined_path_in(namespaces),
                );
            }
            let mut source_imports: HashMap<SourceId, Vec<String>> = HashMap::new();
            for path in namespaces.import_paths() {
                let Some(first) = path.first() else {
                    continue;
                };
                source_imports
                    .entry(first.source_span().source_id)
                    .or_default()
                    .push(path.joined_path_in(namespaces));
            }
            Self {
                source_modules,
                source_imports,
            }
        }

        /*
            The declaring module's logical path for one source, or "" for
            unmoduled scope. New sources come from the collected declarations;
            base sources resolve through the retained table.
        */
        fn module_of_source(&self, program: &SymbolResolvedTrees, source: SourceId) -> String {
            if let Some(module) = self.source_modules.get(&source) {
                return module.clone();
            }
            let module = program.symbols.source_module(source);
            if module.is_valid() {
                program.symbols.display_path(module, "::")
            } else {
                String::new()
            }
        }

        fn imports_of_source(
            &self,
            program: &SymbolResolvedTrees,
            source: SourceId,
        ) -> Vec<String> {
            let mut imports = self
                .source_imports
                .get(&source)
                .cloned()
                .unwrap_or_default();
            imports.extend(
                program
                    .symbols
                    .source_module_import_paths(source)
                    .map(str::to_owned),
            );
            imports
        }

        fn domain_module(
            &self,
            program: &SymbolResolvedTrees,
            domain: &DomainDefinition,
        ) -> String {
            if domain.symbol.is_valid() {
                let module = program.symbols.symbol_module(domain.symbol);
                return if module.is_valid() {
                    program.symbols.display_path(module, "::")
                } else {
                    String::new()
                };
            }
            self.module_of_source(program, domain.name.source_span().source_id)
        }

        /*
            The domain's complete logical path: module prefix plus its declared
            carrier-qualified name. Retained symbols answer exactly; unsymbolled
            declarations compose the path from their owning module.
        */
        fn domain_path(&self, program: &SymbolResolvedTrees, domain: &DomainDefinition) -> String {
            if domain.symbol.is_valid() {
                return program.symbols.display_path(domain.symbol, "::");
            }
            let module = self.domain_module(program, domain);
            let local = domain.name.as_str();
            if module.is_empty() {
                local.to_owned()
            } else {
                format!("{module}::{local}")
            }
        }

        /*
            A relative spelling (the domain's declared name or its leaf) reaches a
            module-owned domain only inside its own module or through a narrow
            import of the exact declaration, which exposes the leaf spelling just
            like ordinary name resolution. Fully qualified spellings always
            select exactly; unmoduled domains keep root scope.
        */
        fn domain_exposed_to(
            &self,
            program: &SymbolResolvedTrees,
            domain_module: &str,
            domain_path: &str,
            authored: &str,
            reference_source: SourceId,
        ) -> bool {
            if authored == domain_path || domain_module.is_empty() {
                return true;
            }
            let reference_module = self.module_of_source(program, reference_source);
            if !reference_module.is_empty() && reference_module == domain_module {
                return true;
            }
            !authored.contains("::")
                && self
                    .imports_of_source(program, reference_source)
                    .iter()
                    .any(|import| import == domain_path)
        }
    }

    /*
        Move an ordinary top-level operator into its exact domain's semantic
        operator family before symbols are assigned. The home is supplied either
        by a qualified declaration name (`operator Quantity::Additive::add ...`)
        or by one unique declared-domain constraint across the operand tuple
        (`operator add(left: i32::Degrees, ...)`).

        Selection follows module name law rather than declared spelling alone:
        a domain declared in the operator's own module outranks same-spelled
        foreign declarations, fully qualified paths select exactly, and relative
        spellings reach a foreign module's domain only through an exposing import.

        The declaration remains an ordinary root item in source. This one lowering
        point owns the semantic association; domain bodies are reserved for exact
        establishment requirements and no checked consumer reconstructs ownership
        from an operator name later.
    */
    pub(crate) fn normalize_domain_operator_homes(
        program: &mut SymbolResolvedTrees,
        namespaces: &NamespaceDeclarations,
    ) -> Result<(), Diagnostic> {
        let scope = NamespaceScope::collect(namespaces);
        let domain_names = program
            .domain_definitions
            .iter()
            .map(|domain| domain.name.as_str().to_owned())
            .collect::<Vec<_>>();
        let domain_modules = program
            .domain_definitions
            .iter()
            .map(|domain| scope.domain_module(program, domain))
            .collect::<Vec<_>>();
        let domain_paths = program
            .domain_definitions
            .iter()
            .map(|domain| scope.domain_path(program, domain))
            .collect::<Vec<_>>();
        let mut operators_by_domain = program
            .domain_definitions
            .iter()
            .map(|domain| program.operator_definitions(domain.operators).to_vec())
            .collect::<Vec<_>>();

        let authored_roots = take(&mut program.operators);
        let mut remaining_roots = OrderedRootArena::new();

        for operator in &authored_roots {
            let path = program.operator_path_members(operator.name).to_vec();
            let Some((leaf, owner_path)) = path.split_last() else {
                remaining_roots.push(operator.clone());
                continue;
            };
            let reference_source = path
                .first()
                .map(|member| member.source_span().source_id)
                .unwrap_or_default();
            let reference_module = scope.module_of_source(program, reference_source);
            let explicit_owner = owner_path.joined_path();
            let explicit_matches = explicit_domain_homes(
                program,
                &scope,
                &explicit_owner,
                &domain_names,
                &domain_modules,
                &domain_paths,
                reference_source,
                &reference_module,
            );
            let inferred_matches = inferred_domain_homes(
                program,
                &scope,
                operator,
                &domain_names,
                &domain_modules,
                &domain_paths,
            );
            let matches = if explicit_matches.is_empty() {
                inferred_matches
            } else if inferred_matches.is_empty() {
                explicit_matches
            } else {
                explicit_matches
                    .into_iter()
                    .filter(|candidate| inferred_matches.contains(candidate))
                    .collect()
            };

            let [domain_index] = matches.as_slice() else {
                if matches.is_empty() {
                    if !inferred_domain_homes(
                        program,
                        &scope,
                        operator,
                        &domain_names,
                        &domain_modules,
                        &domain_paths,
                    )
                    .is_empty()
                    {
                        return Err(Diagnostic::error(format!(
                            "operator `{}` names a domain home that conflicts with its operand domains",
                            operator_label(program, operator)
                        )));
                    }
                    remaining_roots.push(operator.clone());
                    continue;
                }
                return Err(Diagnostic::error(format!(
                    "operator `{}` has more than one possible domain home; use an exact \
                 `operator Type::Domain::operation ...` declaration",
                    operator_label(program, operator)
                )));
            };

            let leaf = program
                .tables
                .declarations
                .operator_path_members
                .append(leaf.clone());
            let mut operator = operator.clone();
            operator.name = HandleSpan::from_parts(leaf, 1);
            operators_by_domain[*domain_index].push(operator);
        }

        let mut rebuilt = Arena::new();
        let mut domain_index = 0usize;
        program.domain_definitions.for_each_mut(|domain| {
            let mut operators = HandleSpan::empty();
            for operator in operators_by_domain[domain_index].drain(..) {
                rebuilt.append_to_span(&mut operators, operator);
            }
            domain.operators = operators;
            if !operators.is_empty() {
                domain.semantic_roles.denotation_dimension = Some(domain.semantic_id);
            }
            domain_index += 1;
        });

        program.tables.declarations.operator_definitions = rebuilt;
        program.operators = remaining_roots;
        Ok(())
    }

    /*
        The explicit `Owner::` prefix of an operator name selects its home.
        Module-local declarations bind tighter than imported or absolute matches;
        each narrower set must still be unique.
    */
    fn explicit_domain_homes(
        program: &SymbolResolvedTrees,
        scope: &NamespaceScope,
        explicit_owner: &str,
        domain_names: &[String],
        domain_modules: &[String],
        domain_paths: &[String],
        reference_source: SourceId,
        reference_module: &str,
    ) -> Vec<usize> {
        if explicit_owner.is_empty() {
            return Vec::new();
        }
        let local = (0..domain_names.len())
            .filter(|index| {
                !reference_module.is_empty()
                    && domain_modules[*index] == reference_module
                    && domain_names[*index].as_str() == explicit_owner
            })
            .collect::<Vec<_>>();
        if !local.is_empty() {
            return local;
        }
        let imports = scope.imports_of_source(program, reference_source);
        let imported = (0..domain_names.len())
            .filter(|index| {
                !explicit_owner.contains("::")
                    && domain_names[*index].as_str() == explicit_owner
                    && imports.iter().any(|import| import == &domain_paths[*index])
            })
            .collect::<Vec<_>>();
        if !imported.is_empty() {
            return imported;
        }
        (0..domain_names.len())
            .filter(|index| domain_paths[*index].as_str() == explicit_owner)
            .collect()
    }

    fn inferred_domain_homes(
        program: &SymbolResolvedTrees,
        scope: &NamespaceScope,
        operator: &OperatorDefinition,
        domain_names: &[String],
        domain_modules: &[String],
        domain_paths: &[String],
    ) -> Vec<usize> {
        let mut matches = Vec::new();
        for parameter in program.state_parameters(operator.parameters) {
            collect_type_domain_homes(
                program,
                scope,
                &parameter.type_reference,
                domain_names,
                domain_modules,
                domain_paths,
                &mut matches,
            );
        }
        matches
    }

    fn collect_type_domain_homes(
        program: &SymbolResolvedTrees,
        scope: &NamespaceScope,
        type_reference: &TypeReference,
        domain_names: &[String],
        domain_modules: &[String],
        domain_paths: &[String],
        matches: &mut Vec<usize>,
    ) {
        use symbol_resolved_trees::types::{TypeConstraint, TypeReference};

        match type_reference {
            TypeReference::AliasUse { expansion, .. } => collect_type_domain_homes(
                program,
                scope,
                expansion,
                domain_names,
                domain_modules,
                domain_paths,
                matches,
            ),
            TypeReference::Reference(reference) => collect_type_domain_homes(
                program,
                scope,
                program.child_type_reference(reference.referee),
                domain_names,
                domain_modules,
                domain_paths,
                matches,
            ),
            TypeReference::Constrained(constrained) => {
                let carrier = program.child_type_reference(constrained.base_type);
                for constraint in program
                    .tables
                    .types
                    .constraints
                    .span_or_empty(constrained.constraints)
                {
                    let TypeConstraint::Domain(authored) = constraint else {
                        continue;
                    };
                    let reference_source = authored.name.source_span().source_id;
                    let reference_module = scope.module_of_source(program, reference_source);
                    let mut constraint_matches = Vec::new();
                    for (index, domain) in program.domain_definitions.iter().enumerate() {
                        let local = &domain_names[index];
                        let qualified = &domain_paths[index];
                        let name_matches = qualified.as_str() == authored.name.as_str()
                            || same_semantic_name(local.as_str(), authored.name.as_str());
                        if name_matches
                            && scope.domain_exposed_to(
                                program,
                                &domain_modules[index],
                                qualified,
                                authored.name.as_str(),
                                reference_source,
                            )
                            && domain_accepts_carrier(
                                program,
                                domain,
                                carrier,
                                authored.arguments.len(),
                            )
                        {
                            constraint_matches.push(index);
                        }
                    }
                    let local_matches = constraint_matches
                        .iter()
                        .copied()
                        .filter(|index| {
                            !reference_module.is_empty()
                                && domain_modules[*index] == reference_module
                        })
                        .collect::<Vec<_>>();
                    for index in if local_matches.is_empty() {
                        constraint_matches
                    } else {
                        local_matches
                    } {
                        if !matches.contains(&index) {
                            matches.push(index);
                        }
                    }
                }
                collect_type_domain_homes(
                    program,
                    scope,
                    carrier,
                    domain_names,
                    domain_modules,
                    domain_paths,
                    matches,
                );
            }
            super::leaf_carrier_type_reference_pattern!() => {}
        }
    }

    fn domain_accepts_carrier(
        program: &SymbolResolvedTrees,
        domain: &DomainDefinition,
        carrier: &TypeReference,
        argument_count: usize,
    ) -> bool {
        let parameters = program.data_type_parameters(domain.type_parameters);
        if parameters.is_empty() {
            return argument_count == 0
                && type_references_match(program, carrier, &domain.target_type);
        }
        let Some(parameter) = parameters.first() else {
            return false;
        };
        if !matches!(parameter.kind, TypeParameterKind::Type)
            || argument_count != parameters.len().saturating_sub(1)
        {
            return false;
        }
        matches!(
            &domain.target_type,
            TypeReference::Named { name, .. }
                if name.as_str() == parameter.name.as_str()
        )
    }

    fn type_references_match(
        program: &SymbolResolvedTrees,
        left: &TypeReference,
        right: &TypeReference,
    ) -> bool {
        use symbol_resolved_trees::types::TypeReference;

        match (left, right) {
            (TypeReference::Constrained(left), _) => {
                type_references_match(program, program.child_type_reference(left.base_type), right)
            }
            (_, TypeReference::Constrained(right)) => {
                type_references_match(program, left, program.child_type_reference(right.base_type))
            }
            (TypeReference::Reference(left), TypeReference::Reference(right)) => {
                left.access == right.access
                    && type_references_match(
                        program,
                        program.child_type_reference(left.referee),
                        program.child_type_reference(right.referee),
                    )
            }
            (TypeReference::FixedArray(left), TypeReference::FixedArray(right)) => {
                left.length == right.length
                    && type_references_match(
                        program,
                        program.child_type_reference(left.element_type),
                        program.child_type_reference(right.element_type),
                    )
            }
            (TypeReference::Slice(left), TypeReference::Slice(right)) => type_references_match(
                program,
                program.child_type_reference(left.element_type),
                program.child_type_reference(right.element_type),
            ),
            (TypeReference::Generic(left), TypeReference::Generic(right)) => {
                left.base_name == right.base_name
                    && left.lifetime_arguments == right.lifetime_arguments
                    && program
                        .child_type_references(left.arguments)
                        .iter()
                        .zip(program.child_type_references(right.arguments))
                        .all(|(left, right)| type_references_match(program, left, right))
                    && left.arguments.len() == right.arguments.len()
            }
            (
                TypeReference::DynamicTrait {
                    name: left,
                    arguments: left_arguments,
                    ..
                },
                TypeReference::DynamicTrait {
                    name: right,
                    arguments: right_arguments,
                    ..
                },
            ) => {
                left == right
                    && left_arguments.count() == right_arguments.count()
                    && program
                        .child_type_references(*left_arguments)
                        .iter()
                        .zip(program.child_type_references(*right_arguments))
                        .all(|(left, right)| type_references_match(program, left, right))
            }
            (TypeReference::Named { name: left, .. }, TypeReference::Named { name: right, .. }) => {
                left == right
            }
            (TypeReference::Unit, TypeReference::Unit) => true,
            _ => false,
        }
    }

    fn operator_label(program: &SymbolResolvedTrees, operator: &OperatorDefinition) -> String {
        program.operator_path_string(operator.name)
    }
}
pub(crate) mod evidence_forwardings {
    /*
        Evidence forwardings bound to the exact machine, state, and source
        conformance they name.

        Forwardings are recorded by root index and state name while machines
        lower; once every state has a symbol they bind to it, and a source that
        is not an incoming `requires` binding resolves to the named
        implementation it names.
    */

    use symbol_resolved_trees::SymbolResolvedTrees;

    pub(crate) fn bind_evidence_forwarding_owners(program: &mut SymbolResolvedTrees) {
        let mut owners = Vec::new();
        let mut incoming_evidence_names = Vec::new();
        for (machine_root_index, machine) in program.machines.iter().enumerate() {
            incoming_evidence_names.extend(program.machine_contracts(machine).iter().filter_map(
                |contract| {
                    (contract.kind
                        == symbol_resolved_trees::signature::SignatureContractKind::Requires)
                        .then_some(contract.binding.as_ref())
                        .flatten()
                        .map(|binding| (machine.symbol, binding.as_str().to_owned()))
                },
            ));
            for state_handle in program.machine_state_handles(machine.states) {
                let state = program.machine_state(*state_handle);
                owners.push((
                    machine_root_index,
                    state.name.as_str().to_owned(),
                    machine.symbol,
                    state.symbol,
                ));
            }
        }
        let named_implementations = program
            .conformances
            .iter()
            .filter_map(|conformance| {
                conformance
                    .alias
                    .as_ref()
                    .map(|alias| (alias.as_str().to_owned(), conformance.symbol))
            })
            .collect::<Vec<_>>();
        for forwarding in &mut program.evidence_forwardings {
            if let Some((_, _, machine_symbol, state_symbol)) =
                owners
                    .iter()
                    .find(|(machine_root_index, state_name, _, _)| {
                        *machine_root_index == forwarding.machine_root_index
                            && state_name == forwarding.state_name.as_str()
                    })
            {
                forwarding.machine_symbol = *machine_symbol;
                forwarding.state_symbol = *state_symbol;
            }
            if !incoming_evidence_names.iter().any(|(machine, name)| {
                *machine == forwarding.machine_symbol && name == forwarding.source.as_str()
            }) {
                forwarding.source_conformance =
                    named_implementations.iter().find_map(|(alias, symbol)| {
                        (alias == forwarding.source.as_str()).then_some(*symbol)
                    });
            }
        }
    }
}
#[path = "selection_callable_inference.rs"]
pub(crate) mod callable_inference;

pub(crate) mod machine_parameter_requirements {
    /*
        Nominal machine-parameter requirements and trait requirement-identity
        arguments.

        Both resolve once top-level symbols and direct trait requirements exist,
        staged before mutation so one bad binder cannot leave a partially
        normalized program behind.
    */

    use crate::selection::signature_free_requirements::{
        SignatureFreeRequirementResolutionError, resolve_signature_free_requirement,
    };
    use diagnostics::Diagnostic;
    use symbol_resolved_trees::SymbolResolvedTrees;
    use symbol_resolved_trees::data::{MachineParameterContract, TypeParameterKind};
    use symbol_resolved_trees::name::DiagnosticName;
    use symbol_resolved_trees::types::TypeReference;

    /*
        Resolve every authored nominal machine-parameter requirement only after
        top-level symbols and direct trait requirements exist. Resolution is
        staged before mutation so one bad binder cannot leave a partially
        normalized program behind.
    */
    pub(crate) fn normalize_nominal_machine_parameter_requirements(
        program: &mut SymbolResolvedTrees,
    ) -> Result<(), Vec<Diagnostic>> {
        let mut replacements = Vec::new();

        for (handle, parameter) in program.tables.declarations.data_type_parameters.iter() {
            let Some((requirement, trait_arguments, rendered)) =
                super::authored_nominal_requirement(parameter)
            else {
                continue;
            };
            let exact = resolve_signature_free_requirement(program, requirement).map_err(|error| {
            Diagnostic::error(match error {
                SignatureFreeRequirementResolutionError::InvalidPath => format!(
                    "nominal machine parameter `{}` requirement `{rendered}` must name an exact `Trait::requirement`",
                    parameter.name
                ),
                SignatureFreeRequirementResolutionError::TraitNotUnique => format!(
                    "nominal machine parameter `{}` requirement `{rendered}` does not resolve to one exact trait",
                    parameter.name
                ),
                SignatureFreeRequirementResolutionError::RequirementNotUnique => format!(
                    "nominal machine parameter `{}` requirement `{rendered}` does not resolve to one exact trait requirement; signature-free references reject overloads",
                    parameter.name
                ),
            })
        }).map_err(|diagnostic| vec![diagnostic])?;
            replacements.push((
                handle,
                exact.trait_definition.symbol,
                exact.requirement.symbol,
                requirement.clone(),
                *trait_arguments,
            ));
        }

        for (handle, trait_definition, requirement, authored_path, trait_arguments) in replacements
        {
            let parameter = program
                .tables
                .declarations
                .data_type_parameters
                .get_mut(handle);
            parameter.kind = TypeParameterKind::Machine {
                contract: MachineParameterContract::Nominal {
                    trait_definition,
                    requirement,
                    authored_path,
                    trait_arguments,
                },
            };
        }

        crate::symbols::assign_nominal_machine_requirement_arguments(program)?;
        Ok(())
    }

    /*
        Bind trait-level requirement-identity arguments after the complete trait
        catalog exists. The generic argument carrier is shared with type arguments,
        but this slot accepts only one exact signature-free `Trait::requirement`
        declaration; concrete machines and ordinary types fail closed.
    */
    pub(crate) fn normalize_trait_machine_requirement_arguments(
        program: &mut SymbolResolvedTrees,
    ) -> Result<(), Diagnostic> {
        let mut replacements = Vec::new();

        for conformance in &program.conformances {
            let Some(trait_definition) = program.trait_definition(conformance.trait_symbol) else {
                continue;
            };
            let parameters = program.trait_type_parameters(trait_definition);
            let arguments = program.child_type_references(conformance.arguments);
            for (index, (parameter, argument)) in parameters.iter().zip(arguments).enumerate() {
                if !matches!(
                    parameter.kind,
                    TypeParameterKind::Machine {
                        contract: MachineParameterContract::RequirementIdentity
                    }
                ) {
                    continue;
                }
                let TypeReference::Named { symbol, name, .. } = argument else {
                    continue;
                };
                let rendered = name.as_str();
                let exact = if symbol.is_valid()
                    && program.symbols.get(*symbol).kind == symbols::SymbolKind::State
                    && program
                        .symbols
                        .source_reference_can_see_symbol(name.source_span(), *symbol)
                    && !program.traits.iter().any(|definition| {
                        program
                            .trait_machine_signatures(definition.machines)
                            .iter()
                            .any(|requirement| requirement.symbol == *symbol)
                    }) {
                    *symbol
                } else {
                    let argument_path: Vec<DiagnosticName> = name
                        .as_str()
                        .split("::")
                        .map(|segment| DiagnosticName::from_str(segment, name.source_span()))
                        .collect();
                    resolve_signature_free_requirement(program, &argument_path)
                        .map(|exact| exact.requirement.symbol)
                        .map_err(|error| {
                            Diagnostic::error(format!(
                                "trait machine requirement argument `{rendered}` {}",
                                requirement_resolution_reason(error)
                            ))
                        })?
                };
                let handle = arena::Handle::from_parts(
                    conformance
                        .arguments
                        .start()
                        .arena_index()
                        .checked_add(u32::try_from(index).expect("trait argument index fits u32"))
                        .expect("trait argument handle overflow"),
                    conformance.arguments.start().generation(),
                );
                replacements.push((handle, exact));
            }
        }

        for (handle, requirement) in replacements {
            let TypeReference::Named { symbol, .. } = program
                .tables
                .declarations
                .child_type_references
                .get_mut(handle)
            else {
                unreachable!("staged requirement-identity argument remains a name")
            };
            *symbol = requirement;
        }
        Ok(())
    }

    /* The diagnostic sentences for the shared resolution law's error variants. */
    fn requirement_resolution_reason(
        error: SignatureFreeRequirementResolutionError,
    ) -> &'static str {
        match error {
            SignatureFreeRequirementResolutionError::InvalidPath => {
                "expected one exact `Trait::requirement` path"
            }
            SignatureFreeRequirementResolutionError::TraitNotUnique => {
                "path does not resolve to one exact trait"
            }
            SignatureFreeRequirementResolutionError::RequirementNotUnique => {
                "path does not resolve to one exact trait requirement; signature-free references reject overloads"
            }
        }
    }
}
pub(crate) mod service_reaches {
    /*
        Service reaches: authored `reaches` names bound to exact boundary-trait
        identities.

        Names live in lowering-private sidecars until every declaration and
        trait-parent edge has a symbol; published rows cannot carry an invalid
        member or a parallel spelling contract.
        Invocation permissions are a separate axis. Do not insert provider-family
        names into these rows; checking derives actual calls and exact callback
        contributions after the callable contracts have been resolved.
    */

    use diagnostics::Diagnostic;
    use language_semantics::{
        ServiceReachId, ServiceReachRowId, ServiceReachRowTable, ServiceReachTable,
    };
    use std::fmt;
    use symbol_resolved_trees::SymbolResolvedTrees;
    use symbol_resolved_trees::data::TypeParameter;
    use symbol_resolved_trees::machine::Machine;
    use symbol_resolved_trees::name::DiagnosticName;
    use symbol_resolved_trees::signature::AuthoredServiceReachRow;
    use symbol_resolved_trees::signature::AuthoredServiceReachTarget;
    use symbol_resolved_trees::signature::StateSignature;
    use symbols::{SymbolHandle, SymbolKind, SymbolTable};

    #[derive(Debug, Clone)]
    pub(crate) enum PendingAuthoredServiceReach {
        Authored {
            keyword_source_spans: Vec<source::SourceSpan>,
            authored: Vec<DiagnosticName>,
        },
        /*
            Exact output from this seeded resolver's predecessor. A positional
            service row and its authored ceiling travel together; no name lookup
            reconstructs their meaning under a later consumer's source selection.
        */
        Resolved {
            row: ServiceReachRowId,
            authored: Option<AuthoredServiceReachRow>,
        },
    }

    impl PendingAuthoredServiceReach {
        fn resolve(
            &self,
            symbols: &SymbolTable,
            services: &ServiceReachTable,
            rows: &mut ServiceReachRowTable,
            owner: SymbolHandle,
            installation_bound: bool,
        ) -> Result<(ServiceReachRowId, Option<AuthoredServiceReachRow>), Diagnostic> {
            match self {
                Self::Resolved { row, authored } => {
                    if authored.as_ref().is_some_and(|authored| {
                        authored.owner != owner || authored.installation_bound != installation_bound
                    }) {
                        return Err(Diagnostic::error(
                            "retained service reach changed its exact owner or installation mode",
                        ));
                    }
                    Ok((*row, authored.clone()))
                }
                Self::Authored {
                    keyword_source_spans,
                    authored,
                } => {
                    let authored = resolve_authored_service_reach(
                        symbols,
                        services,
                        owner,
                        keyword_source_spans,
                        authored,
                        installation_bound,
                    )?;
                    let direct = authored
                        .as_ref()
                        .into_iter()
                        .flat_map(|row| &row.targets)
                        .filter_map(|target| services.id_for_symbol(target.service));
                    Ok((row_for_service_ids(services, rows, direct), authored))
                }
            }
        }
    }

    pub(crate) struct PendingSignatureServiceReach {
        pub(crate) symbol: SymbolHandle,
        pub(crate) owner: crate::resolution::lowerer::PendingSignatureOwner,
        pub(crate) reaches: PendingAuthoredServiceReach,
    }

    /*
        Normalize service reach only after every declaration and trait-parent edge
        has a resolved symbol. Authored names live only in lowering-private
        sidecars and are validated before the published resolved trees are
        returned; normalized rows therefore cannot contain an invalid member or
        retain a parallel spelling contract.
    */
    pub(crate) fn normalize_service_reaches(
        program: &mut SymbolResolvedTrees,
        machine_service_reaches: &[(symbols::SymbolHandle, PendingAuthoredServiceReach)],
        signature_service_reaches: &[PendingSignatureServiceReach],
    ) -> Result<(), Diagnostic> {
        normalize_service_reaches_with_retained_tables(
            program,
            machine_service_reaches,
            signature_service_reaches,
            ServiceReachTable::default(),
            ServiceReachRowTable::default(),
        )
    }

    pub(crate) fn normalize_service_reaches_with_retained_tables(
        program: &mut SymbolResolvedTrees,
        machine_service_reaches: &[(symbols::SymbolHandle, PendingAuthoredServiceReach)],
        signature_service_reaches: &[PendingSignatureServiceReach],
        mut services: ServiceReachTable,
        mut rows: ServiceReachRowTable,
    ) -> Result<(), Diagnostic> {
        let mut boundary_traits = program
            .traits
            .iter()
            .filter(|definition| definition.is_boundary)
            .map(|definition| {
                (
                    definition.symbol,
                    definition.name.as_str().to_owned(),
                    program
                        .trait_requirements(definition.requires)
                        .iter()
                        .map(|requirement| requirement.symbol)
                        .collect::<Vec<_>>(),
                )
            })
            .collect::<Vec<_>>();
        boundary_traits.sort_by(|left, right| left.1.cmp(&right.1));

        for (symbol, name, _) in &boundary_traits {
            services.intern(*symbol, name);
        }
        for (symbol, _, parent_symbols) in &boundary_traits {
            let Some(service) = services.id_for_symbol(*symbol) else {
                continue;
            };
            let parents = parent_symbols
                .iter()
                .filter_map(|parent| services.id_for_symbol(*parent))
                .collect();
            services.set_parents(service, parents);
        }

        rows.intern(Vec::new());

        let machine_rows = program
            .machines
            .iter()
            .map(|machine| {
                let pending = machine_service_reaches
                    .iter()
                    .find(|(symbol, _)| *symbol == machine.symbol)
                    .map(|(_, reaches)| reaches)
                    .expect("surviving resolved machine retains its pending authored service row");
                if let PendingAuthoredServiceReach::Authored {
                    keyword_source_spans,
                    authored,
                } = pending
                {
                    validate_machine_service_reaches(
                        program,
                        &services,
                        machine,
                        authored,
                        !keyword_source_spans.is_empty(),
                    )?;
                }
                let (row, authored) = pending.resolve(
                    &program.symbols,
                    &services,
                    &mut rows,
                    machine.symbol,
                    machine.service_reach_is_installation_bound,
                )?;
                Ok((machine.symbol, row, authored))
            })
            .collect::<Result<Vec<_>, Diagnostic>>()?;

        let signature_rows = program
            .traits
            .iter()
            .flat_map(|definition| program.trait_machine_signatures(definition.machines))
            .chain(
                program
                    .tables
                    .declarations
                    .runtime_callable_contracts
                    .iter()
                    .map(|(_, contract)| &contract.signature),
            )
            .map(|signature| {
                let pending =
                    pending_signature_service_reach(signature_service_reaches, signature.symbol)?;
                validate_signature_service_reaches(program, &services, signature, pending)?;
                let (row, authored) = pending.reaches.resolve(
                    &program.symbols,
                    &services,
                    &mut rows,
                    signature.symbol,
                    signature.service_reach_is_installation_bound,
                )?;
                Ok((signature.symbol, row, authored))
            })
            .collect::<Result<Vec<_>, Diagnostic>>()?;

        program.machines.for_each_mut(|machine| {
            machine.service_reach_row = machine_rows
                .iter()
                .find(|(symbol, _, _)| *symbol == machine.symbol)
                .map(|(_, row, _)| *row)
                .unwrap_or(ServiceReachRowTable::EMPTY_ROW);
        });

        let signature_spans = program
            .traits
            .iter()
            .map(|definition| definition.machines)
            .collect::<Vec<_>>();
        let mut prepared_rows = signature_rows.iter();
        let mut apply_signature_row = |signature: &mut StateSignature| -> Result<(), Diagnostic> {
            let (symbol, row, _) = prepared_rows.next().ok_or_else(|| {
                Diagnostic::error("callable signature has no prepared normalized service-reach row")
            })?;
            if *symbol != signature.symbol {
                return Err(Diagnostic::error(
                    "normalized service-reach row does not match its exact callable signature",
                ));
            }
            signature.service_reach_row = *row;
            Ok(())
        };
        let signature_arena = &mut program.tables.declarations.trait_machine_signatures;
        for span in signature_spans {
            for signature in signature_arena.span_mut_or_empty(span) {
                apply_signature_row(signature)?;
            }
        }

        let runtime_contracts = program
            .tables
            .declarations
            .runtime_callable_contracts
            .iter()
            .map(|(handle, _)| handle)
            .collect::<Vec<_>>();
        for handle in runtime_contracts {
            let contract = program
                .tables
                .declarations
                .runtime_callable_contracts
                .get_mut(handle);
            apply_signature_row(&mut contract.signature)?;
        }
        if prepared_rows.next().is_some() {
            return Err(Diagnostic::error(
                "normalized service-reach rows outnumber their exact callable signatures",
            ));
        }

        let parameter_authored_rows = normalize_machine_parameter_rows(
            program,
            &services,
            &mut rows,
            signature_service_reaches,
        )?;

        program.service_reaches = services;
        program.service_reach_rows = rows;
        program.authored_service_reach_rows = machine_rows
            .into_iter()
            .filter_map(|(_, _, row)| row)
            .chain(signature_rows.into_iter().filter_map(|(_, _, row)| row))
            .chain(parameter_authored_rows)
            .collect();
        Ok(())
    }

    fn pending_signature_service_reach(
        pending: &[PendingSignatureServiceReach],
        symbol: SymbolHandle,
    ) -> Result<&PendingSignatureServiceReach, Diagnostic> {
        pending
            .iter()
            .find(|entry| entry.symbol == symbol)
            .ok_or_else(|| {
                Diagnostic::error(
                    "resolved callable signature has no exact pending authored service-reach row",
                )
            })
    }

    fn validate_signature_service_reaches(
        program: &SymbolResolvedTrees,
        services: &ServiceReachTable,
        signature: &StateSignature,
        pending: &PendingSignatureServiceReach,
    ) -> Result<(), Diagnostic> {
        let PendingAuthoredServiceReach::Authored { authored, .. } = &pending.reaches else {
            return Ok(());
        };
        for service in authored {
            if service_for_name(&program.symbols, services, service).is_none() {
                return Err(Diagnostic::error(format!(
                    "{} state `{}` declares unknown boundary service `{service}`",
                    SignatureOwnerDisplay(&pending.owner),
                    signature.name,
                )));
            }
        }
        Ok(())
    }

    struct SignatureOwnerDisplay<'a>(&'a crate::resolution::lowerer::PendingSignatureOwner);

    impl fmt::Display for SignatureOwnerDisplay<'_> {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            match self.0 {
                crate::resolution::lowerer::PendingSignatureOwner::Trait(name) => {
                    write!(formatter, "trait `{name}`")
                }
                crate::resolution::lowerer::PendingSignatureOwner::Requirement(name) => {
                    write!(formatter, "machine-parameter requirement `{name}`")
                }
            }
        }
    }

    fn validate_machine_service_reaches(
        program: &SymbolResolvedTrees,
        services: &ServiceReachTable,
        machine: &Machine,
        authored: &[DiagnosticName],
        is_authored: bool,
    ) -> Result<(), Diagnostic> {
        for service in authored {
            if service_for_name(&program.symbols, services, service).is_none() {
                return Err(Diagnostic::error(format!(
                    "machine `{}` declares unknown boundary service `{service}`",
                    machine.name,
                )));
            }
        }

        if matches!(
            machine.supply_mode,
            language_semantics::MachineSupplyMode::ExternalRealization { .. }
        ) && is_authored
        {
            return Err(Diagnostic::error(format!(
                "external leaf `{}` repeats an authored `reaches` row, but `via` derives behavior from the satisfied requirement and admitted binding; remove the leaf's `reaches` clause",
                machine.name,
            )));
        }

        Ok(())
    }

    fn resolve_authored_service_reach(
        symbols: &SymbolTable,
        services: &ServiceReachTable,
        owner: SymbolHandle,
        keyword_source_spans: &[source::SourceSpan],
        authored: &[DiagnosticName],
        installation_bound: bool,
    ) -> Result<Option<AuthoredServiceReachRow>, Diagnostic> {
        if keyword_source_spans.is_empty() {
            if authored.is_empty() {
                return Ok(None);
            }
            return Err(Diagnostic::error(
                "authored service-reach members have no retained `reaches` clause occurrence",
            ));
        }

        let targets = authored
        .iter()
        .map(|name| {
            let service = service_for_name(symbols, services, name).ok_or_else(|| {
                Diagnostic::error(format!(
                    "authored service-reach member `{name}` lost its exact boundary-service identity"
                ))
            })?;
            let definition = services.definition(service).ok_or_else(|| {
                Diagnostic::error(format!(
                    "authored service-reach member `{name}` resolves outside the normalized service table"
                ))
            })?;
            Ok(AuthoredServiceReachTarget {
                service: definition.symbol,
                source_span: name.source_span(),
            })
        })
        .collect::<Result<Vec<_>, Diagnostic>>()?;
        Ok(Some(AuthoredServiceReachRow {
            owner,
            keyword_source_spans: keyword_source_spans.to_vec(),
            targets,
            installation_bound,
        }))
    }

    fn normalize_machine_parameter_rows(
        program: &mut SymbolResolvedTrees,
        services: &ServiceReachTable,
        rows: &mut ServiceReachRowTable,
        signature_service_reaches: &[PendingSignatureServiceReach],
    ) -> Result<Vec<AuthoredServiceReachRow>, Diagnostic> {
        use symbol_resolved_trees::data::TypeParameterKind;

        let mut roots = program
            .data_definitions
            .iter()
            .map(|definition| definition.type_parameters)
            .chain(
                program
                    .machines
                    .iter()
                    .map(|machine| machine.type_parameters),
            )
            .chain(
                program
                    .traits
                    .iter()
                    .map(|definition| definition.type_parameters),
            )
            .collect::<Vec<_>>();
        for definition in &program.traits {
            roots.extend(
                program
                    .trait_machine_signatures(definition.machines)
                    .iter()
                    .map(|signature| signature.type_parameters),
            );
        }

        roots.extend(
            program
                .tables
                .declarations
                .runtime_callable_contracts
                .iter()
                .map(|(_, contract)| contract.signature.type_parameters),
        );

        let type_parameters = &program.tables.declarations.data_type_parameters;
        let mut spans = Vec::new();
        for root in roots {
            collect_parameter_spans(type_parameters, root, &mut spans);
        }

        let mut resolved_rows = Vec::new();
        for parameter in spans
            .iter()
            .flat_map(|span| type_parameters.span_or_empty(*span))
        {
            let TypeParameterKind::Machine { contract } = &parameter.kind else {
                continue;
            };
            let Some(contract) = contract.structural() else {
                continue;
            };
            let pending =
                pending_signature_service_reach(signature_service_reaches, contract.symbol)?;
            validate_signature_service_reaches(program, services, contract, pending)?;
            let (row, authored) = pending.reaches.resolve(
                &program.symbols,
                services,
                rows,
                contract.symbol,
                contract.service_reach_is_installation_bound,
            )?;
            resolved_rows.push((parameter.symbol, row, authored));
        }

        let type_parameters = &mut program.tables.declarations.data_type_parameters;
        for span in spans {
            for parameter in type_parameters.span_mut_or_empty(span) {
                let TypeParameterKind::Machine { contract } = &mut parameter.kind else {
                    continue;
                };
                let Some(contract) = contract.structural_mut() else {
                    continue;
                };
                let row = resolved_rows
                    .iter()
                    .find(|(symbol, _, _)| *symbol == parameter.symbol)
                    .map(|(_, row, _)| *row)
                    .unwrap_or(ServiceReachRowTable::EMPTY_ROW);
                contract.service_reach_row = row;
            }
        }
        Ok(resolved_rows
            .into_iter()
            .filter_map(|(_, _, authored)| authored)
            .collect())
    }

    fn collect_parameter_spans(
        arena: &arena::Arena<TypeParameter>,
        span: arena::HandleSpan<TypeParameter>,
        spans: &mut Vec<arena::HandleSpan<TypeParameter>>,
    ) {
        use symbol_resolved_trees::data::TypeParameterKind;
        if span.is_empty() || spans.contains(&span) {
            return;
        }
        spans.push(span);
        for parameter in arena.span_or_empty(span) {
            if let TypeParameterKind::Machine { contract } = &parameter.kind
                && let Some(contract) = contract.structural()
            {
                collect_parameter_spans(arena, contract.type_parameters, spans);
            }
        }
    }

    fn row_for_service_ids(
        services: &ServiceReachTable,
        rows: &mut ServiceReachRowTable,
        service_ids: impl IntoIterator<Item = ServiceReachId>,
    ) -> ServiceReachRowId {
        let mut members = Vec::new();
        for service in service_ids {
            services.extend_closure(service, &mut members);
        }
        rows.intern(members)
    }

    fn service_for_name(
        symbols: &SymbolTable,
        services: &ServiceReachTable,
        name: &DiagnosticName,
    ) -> Option<ServiceReachId> {
        let symbol = symbols.find_top_level_by_name_and_kinds_from_source(
            name.as_str(),
            &[SymbolKind::Trait],
            name.source_span(),
        )?;
        services.id_for_symbol(symbol)
    }
}
pub(crate) mod signature_free_requirements {
    /*
        The shared law for paths that name one trait requirement without a call
        signature.

        Neither visible satisfiers nor an expected call shape may select among
        overloads. Establishment routes, nominal machine binders, and lookup all
        resolve through this module.
    */

    use symbol_resolved_trees::SymbolResolvedTrees;
    use symbol_resolved_trees::name::DiagnosticName;
    use symbol_resolved_trees::signature::StateSignature;
    use symbol_resolved_trees::trait_definition::TraitDefinition;
    use symbols::JoinedPath;

    /*
        One exact trait-requirement row selected by a path that carries no call
        signature. Domain establishment routes and nominal static-machine binders
        share this resolution law: neither visible satisfiers nor an expected call
        shape may select among overloads.
    */
    pub struct ExactSignatureFreeRequirement<'program> {
        pub trait_definition: &'program TraitDefinition,
        pub declaring_trait: &'program TraitDefinition,
        pub requirement: &'program StateSignature,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum SignatureFreeRequirementResolutionError {
        InvalidPath,
        TraitNotUnique,
        RequirementNotUnique,
    }

    pub fn resolve_signature_free_requirement<'program>(
        program: &'program SymbolResolvedTrees,
        path: &[DiagnosticName],
    ) -> Result<ExactSignatureFreeRequirement<'program>, SignatureFreeRequirementResolutionError>
    {
        let [trait_path @ .., requirement_name] = path else {
            return Err(SignatureFreeRequirementResolutionError::InvalidPath);
        };
        if trait_path.is_empty() {
            return Err(SignatureFreeRequirementResolutionError::InvalidPath);
        }

        let trait_name = trait_path.joined_path();
        let use_span = requirement_name.source_span();
        let matching_traits = signature_free_trait_candidates(program, &trait_name, use_span);
        let [trait_definition] = matching_traits.as_slice() else {
            return Err(SignatureFreeRequirementResolutionError::TraitNotUnique);
        };

        let matching_requirements = inherited_requirement_candidates(
            program,
            trait_definition,
            requirement_name.as_str(),
            use_span,
        );
        let [(declaring_trait, requirement)] = matching_requirements.as_slice() else {
            return Err(SignatureFreeRequirementResolutionError::RequirementNotUnique);
        };

        Ok(ExactSignatureFreeRequirement {
            trait_definition,
            declaring_trait,
            requirement,
        })
    }

    /*
        Inherited names keep their original declarations. Reaching one declaration
        through both sides of a diamond does not create a second overload; distinct
        declarations with the same name remain ambiguous. Typed application checking
        separately checks that every path supplies the same parent arguments.
    */
    fn inherited_requirement_candidates<'program>(
        program: &'program SymbolResolvedTrees,
        selected_trait: &'program TraitDefinition,
        name: &str,
        use_span: source::SourceSpan,
    ) -> Vec<(&'program TraitDefinition, &'program StateSignature)> {
        let mut pending = vec![selected_trait];
        let mut visited = Vec::new();
        let mut requirements = Vec::new();
        while let Some(declaration) = pending.pop() {
            if visited.contains(&declaration.symbol) {
                continue;
            }
            visited.push(declaration.symbol);
            requirements.extend(
                program
                    .trait_machine_signatures(declaration.machines)
                    .iter()
                    .filter(|signature| {
                        signature.name.as_str() == name
                            && program
                                .symbols
                                .source_reference_can_see_symbol(use_span, signature.symbol)
                    })
                    .map(|signature| (declaration, signature)),
            );
            for parent in program
                .trait_requirements(declaration.requires)
                .iter()
                .chain(declaration.refines.iter())
            {
                if let Some(parent) = program.trait_definition(parent.symbol) {
                    pending.push(parent);
                }
            }
        }
        requirements
    }

    /*
        One exact machine declaration selected by a path that carries no call
        signature. Domain establishment routes may name a free or attached machine
        (`a::b::make` or `Data::method`); neither visible conformers nor an
        expected call shape may select among same-named machines.
    */
    pub(crate) struct ExactSignatureFreeMachine<'program> {
        pub(crate) machine: &'program symbol_resolved_trees::machine::Machine,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) enum SignatureFreeMachineResolutionError {
        NotUnique,
    }

    pub(crate) fn resolve_signature_free_machine<'program>(
        program: &'program SymbolResolvedTrees,
        path: &[DiagnosticName],
    ) -> Result<ExactSignatureFreeMachine<'program>, SignatureFreeMachineResolutionError> {
        let machine_name = path.joined_path();
        let use_span = path
            .last()
            .expect("establishment route paths are nonempty")
            .source_span();
        let matching = signature_free_machine_candidates(program, &machine_name, use_span);
        let [machine] = matching.as_slice() else {
            return Err(SignatureFreeMachineResolutionError::NotUnique);
        };
        Ok(ExactSignatureFreeMachine { machine })
    }

    /*
        Resolve machine paths through ordinary namespace selection while retaining
        signature-free ambiguity. Operator declarations are not exact-machine routes.
    */
    pub(crate) fn signature_free_machine_candidates<'program>(
        program: &'program SymbolResolvedTrees,
        machine_name: &str,
        use_span: source::SourceSpan,
    ) -> Vec<&'program symbol_resolved_trees::machine::Machine> {
        let selected = program
            .symbols
            .lookup_signature_free_top_level_from_source_matching(
                machine_name,
                &[symbols::SymbolKind::Machine],
                use_span,
                |symbol| {
                    program
                        .machines
                        .iter()
                        .any(|machine| machine.symbol == symbol && machine.spelling.is_none())
                },
            );
        program
            .machines
            .iter()
            .filter(|machine| lookup_contains(selected, machine.symbol))
            .collect()
    }

    /*
        Share ordinary namespace selection with type and call references. Scanning
        same-leaf traits is insufficient: a loaded module grants no import exposure,
        and a package-qualified path need not equal a declaration's display spelling.
        The lookup retains two witnesses for ambiguity; callers need exactly one
        owner before checking its requirement signatures.
    */
    pub(crate) fn signature_free_trait_candidates<'program>(
        program: &'program SymbolResolvedTrees,
        trait_name: &str,
        use_span: source::SourceSpan,
    ) -> Vec<&'program TraitDefinition> {
        let selected = program
            .symbols
            .lookup_signature_free_top_level_from_source_matching(
                trait_name,
                &[symbols::SymbolKind::Trait],
                use_span,
                |_| true,
            );
        program
            .traits
            .iter()
            .filter(|definition| lookup_contains(selected, definition.symbol))
            .collect()
    }

    fn lookup_contains(lookup: symbols::SymbolLookup, symbol: symbols::SymbolHandle) -> bool {
        match lookup {
            symbols::SymbolLookup::NotFound => false,
            symbols::SymbolLookup::Unique(selected) => selected == symbol,
            symbols::SymbolLookup::Ambiguous { first, second } => {
                first == symbol || second == symbol
            }
        }
    }

    struct AmbiguousUse {
        trait_symbol: symbols::SymbolHandle,
        trait_name: String,
        requirement_name: String,
        use_span: source::SourceSpan,
        message: String,
    }

    /*
        Report overload additions at both sides of the compatibility break before
        either normalizer consumes authored signature-free paths.
    */
    pub(crate) fn validate_signature_free_requirement_compatibility(
        program: &SymbolResolvedTrees,
    ) -> Vec<diagnostics::Diagnostic> {
        let mut uses = Vec::new();
        for (_, parameter) in program.tables.declarations.data_type_parameters.iter() {
            let Some((requirement, _, rendered)) = super::authored_nominal_requirement(parameter)
            else {
                continue;
            };
            collect_ambiguous_use(
                program,
                requirement,
                format!(
                    "nominal machine parameter `{}` requirement `{rendered}` does not resolve to one exact trait requirement; signature-free references reject overloads",
                    parameter.name
                ),
                &mut uses,
            );
        }
        for domain in &program.domain_definitions {
            for route in &domain.authored_routes {
                if resolve_signature_free_machine(program, route).is_ok()
                    && resolve_signature_free_requirement(program, route).is_err()
                {
                    continue;
                }
                let rendered = route.joined_path();
                collect_ambiguous_use(
                    program,
                    route,
                    format!(
                        "domain `{}` establishment route `{rendered}` does not resolve to one exact trait requirement",
                        domain.name
                    ),
                    &mut uses,
                );
            }
        }
        uses.sort_by_key(|use_site| {
            (
                use_site.use_span.source_id.0,
                use_site.use_span.span.start,
                use_site.use_span.span.end,
            )
        });

        let mut families: Vec<(symbols::SymbolHandle, String, String, source::SourceSpan)> =
            Vec::new();
        for use_site in &uses {
            if families.iter().any(|(symbol, requirement, _, _)| {
                *symbol == use_site.trait_symbol && requirement == &use_site.requirement_name
            }) {
                continue;
            }
            let trait_definition = program
                .trait_definition(use_site.trait_symbol)
                .expect("ambiguous signature-free family retains its declaring trait");
            families.push((
                use_site.trait_symbol,
                use_site.requirement_name.clone(),
                use_site.trait_name.clone(),
                trait_definition.name.source_span(),
            ));
        }
        families.sort_by_key(|(_, requirement, _, span)| {
            (span.source_id.0, span.span.start, requirement.clone())
        });

        let mut diagnostics = families
        .into_iter()
        .map(|(_, requirement, trait_name, span)| {
            diagnostics::Diagnostic::error(format!(
                "declaring trait `{trait_name}` overloads requirement `{requirement}`; this is a source-compatibility break for signature-free requirement references"
            ))
            .with_source_span(span)
        })
        .collect::<Vec<_>>();
        diagnostics.extend(uses.into_iter().map(|use_site| {
            diagnostics::Diagnostic::error(use_site.message).with_source_span(use_site.use_span)
        }));
        diagnostics
    }

    fn collect_ambiguous_use(
        program: &SymbolResolvedTrees,
        path: &[DiagnosticName],
        message: String,
        uses: &mut Vec<AmbiguousUse>,
    ) {
        let [trait_path @ .., requirement_name] = path else {
            return;
        };
        let trait_name = trait_path.joined_path();
        let matching_traits =
            signature_free_trait_candidates(program, &trait_name, requirement_name.source_span());
        let [trait_definition] = matching_traits.as_slice() else {
            return;
        };
        let candidates = inherited_requirement_candidates(
            program,
            trait_definition,
            requirement_name.as_str(),
            requirement_name.source_span(),
        );
        if candidates.len() <= 1 {
            return;
        }
        let first_owner = candidates[0].0;
        let trait_definition = if candidates
            .iter()
            .all(|(owner, _)| owner.symbol == first_owner.symbol)
        {
            first_owner
        } else {
            trait_definition
        };
        uses.push(AmbiguousUse {
            trait_symbol: trait_definition.symbol,
            trait_name: trait_definition.name.as_str().to_owned(),
            requirement_name: requirement_name.as_str().to_owned(),
            use_span: requirement_name.source_span(),
            message,
        });
    }
}
mod static_arguments {
    /*
        Named member operands select normalized implementation rows, independently
        of the private spelling used to store an inline realization machine.
    */

    use diagnostics::Diagnostic;
    use source::SourceSpan;
    use std::collections::HashMap;
    use symbol_resolved_trees::SymbolResolvedTrees;
    use symbol_resolved_trees::expression::{
        ExpressionNode, ExpressionTable, StaticMachineArgument,
    };
    use symbol_resolved_trees::statement::Statement;
    use symbol_resolved_trees::types::TypeReference;
    use symbols::{JoinedPath, SymbolHandle, SymbolKind, SymbolLookup, SymbolTable};

    #[derive(Clone, Copy)]
    struct Member {
        machine: SymbolHandle,
        entry: SymbolHandle,
        complete: bool,
    }

    struct Members {
        rows: HashMap<SymbolHandle, HashMap<String, Vec<Member>>, symbols::BuildSymbolHasher>,
    }

    impl Members {
        fn collect(program: &SymbolResolvedTrees) -> Self {
            let mut rows: HashMap<_, HashMap<String, Vec<Member>>, symbols::BuildSymbolHasher> =
                HashMap::default();
            for implementation in program.conformances.iter() {
                let names = rows.entry(implementation.symbol).or_default();
                for row in &implementation.rows {
                    let complete = row.declaring_trait.is_valid()
                        && row.requirement.is_valid()
                        && row.realization_state.is_valid()
                        && program
                            .machine(row.realization_machine)
                            .is_some_and(|machine| {
                                program
                                    .machine_state_handles(machine.states)
                                    .iter()
                                    .any(|state| {
                                        program.machine_state(*state).symbol
                                            == row.realization_state
                                    })
                            });
                    names
                        .entry(row.requirement_name.as_str().to_owned())
                        .or_default()
                        .push(Member {
                            machine: row.realization_machine,
                            entry: row.realization_state,
                            complete,
                        });
                }
            }
            Self { rows }
        }

        fn in_owner(
            &self,
            owner: SymbolHandle,
            name: &str,
            at: SourceSpan,
        ) -> Result<Member, Diagnostic> {
            match self
                .rows
                .get(&owner)
                .and_then(|names| names.get(name))
                .map(Vec::as_slice)
            {
                Some([member]) if member.complete => Ok(*member),
                Some([_]) => Err(error(
                    at,
                    "named implementation member lost its exact normalized realization",
                )),
                Some(_) => Err(error(
                    at,
                    "named implementation member is ambiguous between requirement overloads",
                )),
                None => Err(error(
                    at,
                    "named implementation has no such requirement member",
                )),
            }
        }

        fn path(
            &self,
            symbols: &SymbolTable,
            owner: &str,
            name: &str,
            at: SourceSpan,
        ) -> Result<Option<Member>, Diagnostic> {
            match symbols.lookup_top_level_by_name_and_kinds_from_source_matching(
                owner,
                &[SymbolKind::Conformance],
                at,
                |_| true,
            ) {
                SymbolLookup::Unique(owner) => self.in_owner(owner, name, at).map(Some),
                SymbolLookup::Ambiguous { .. } => {
                    Err(error(at, "named implementation owner is ambiguous"))
                }
                SymbolLookup::NotFound => Ok(None),
            }
        }
    }

    pub(super) struct ResolvedStaticOperand {
        pub(super) source: SourceSpan,
        pub(super) entry: SymbolHandle,
    }

    /*
        Rejoin all declaration-owned static operand graphs after row normalization.
        The receiving telescope continues to check operand categories and contracts.
    */
    pub(super) fn bind(program: &mut SymbolResolvedTrees) -> Result<(), Diagnostic> {
        let members = Members::collect(program);
        let expressions = program
            .tables
            .bodies
            .expressions
            .iter_expressions()
            .filter_map(|(handle, node)| matches!(node, ExpressionNode::Call(_)).then_some(handle))
            .collect::<Vec<_>>();
        for expression in expressions {
            let ExpressionNode::Call(call) =
                program.tables.bodies.expressions.expression_mut(expression)
            else {
                continue;
            };
            let mut arguments = std::mem::take(&mut call.machine_arguments);
            let mut resolved = Vec::new();
            let result = bind_arguments(
                &mut arguments,
                &members,
                &program.symbols,
                &mut program.tables.bodies.expressions,
                &mut resolved,
            );
            if let ExpressionNode::Call(call) =
                program.tables.bodies.expressions.expression_mut(expression)
            {
                call.machine_arguments = arguments;
            }
            result?;
            let table = &program.tables.bodies.expressions;
            let occurrences = table
                .authored_selection_occurrences(expression)
                .collect::<Vec<_>>();
            let exposure = table.authored_expression_exposure(expression);
            let partition = table.compiler_selection_partition(expression);
            super::authored_selections::finalize_normalized_static_operands(
                program,
                &occurrences,
                exposure,
                partition,
                &resolved,
            )?;
        }
        let statements = program
            .tables
            .declarations
            .state_statements
            .iter()
            .filter_map(|(handle, statement)| {
                matches!(statement, Statement::Call(_)).then_some(handle)
            })
            .collect::<Vec<_>>();
        for statement in statements {
            let Statement::Call(call) = program
                .tables
                .declarations
                .state_statements
                .get_mut(statement)
            else {
                continue;
            };
            let mut arguments = std::mem::take(&mut call.machine_arguments);
            let occurrences = call.authored_static_argument_selections.to_vec();
            let mut resolved = Vec::new();
            let result = bind_arguments(
                &mut arguments,
                &members,
                &program.symbols,
                &mut program.tables.bodies.expressions,
                &mut resolved,
            );
            if let Statement::Call(call) = program
                .tables
                .declarations
                .state_statements
                .get_mut(statement)
            {
                call.machine_arguments = arguments;
            }
            result?;
            super::authored_selections::finalize_normalized_static_operands(
                program,
                &occurrences,
                None,
                None,
                &resolved,
            )?;
        }
        let mut result = Ok(());
        let symbols = &program.symbols;
        let table = &mut program.tables.bodies.expressions;
        program.roots.data_definitions.for_each_mut(|data| {
            if result.is_ok()
                && let Some(argument) = &mut data.cleanup_selection
            {
                result = bind_argument(
                    argument,
                    None,
                    &members,
                    symbols,
                    table,
                    &mut Vec::new(),
                    &mut Vec::new(),
                );
            }
        });
        result?;
        let mut result = Ok(());
        program.roots.domain_definitions.for_each_mut(|domain| {
            for argument in domain
                .cleanup_selection
                .iter_mut()
                .chain(domain.access_policy.iter_mut())
            {
                if result.is_err() {
                    break;
                }
                result = bind_argument(
                    argument,
                    None,
                    &members,
                    symbols,
                    table,
                    &mut Vec::new(),
                    &mut Vec::new(),
                );
            }
        });
        result?;
        let references = program
            .tables
            .declarations
            .child_type_references
            .iter()
            .filter_map(|(handle, reference)| match reference {
                TypeReference::Named { symbol, name, .. } if !symbol.is_valid() => {
                    Some((handle, name.clone()))
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        for (handle, name) in references {
            let Some((owner, member)) = name.as_str().rsplit_once("::") else {
                continue;
            };
            let Some(selected) =
                members.path(&program.symbols, owner, member, name.source_span())?
            else {
                continue;
            };
            if !program
                .symbols
                .source_reference_can_see_symbol(name.source_span(), selected.machine)
            {
                return Err(error(
                    name.source_span(),
                    "named implementation realization is not visible from this source",
                ));
            }
            if let TypeReference::Named { symbol, .. } = program
                .tables
                .declarations
                .child_type_references
                .get_mut(handle)
            {
                *symbol = selected.entry;
            }
        }
        Ok(())
    }

    fn bind_arguments(
        arguments: &mut [StaticMachineArgument],
        members: &Members,
        symbols: &SymbolTable,
        table: &mut ExpressionTable,
        resolved: &mut Vec<ResolvedStaticOperand>,
    ) -> Result<(), Diagnostic> {
        let mut active = Vec::new();
        for argument in arguments {
            bind_argument(
                argument,
                None,
                members,
                symbols,
                table,
                resolved,
                &mut active,
            )?;
        }
        Ok(())
    }

    fn bind_argument(
        argument: &mut StaticMachineArgument,
        owner: Option<SymbolHandle>,
        members: &Members,
        symbols: &SymbolTable,
        table: &mut ExpressionTable,
        resolved: &mut Vec<ResolvedStaticOperand>,
        active: &mut Vec<arena::Handle<StaticMachineArgument>>,
    ) -> Result<(), Diagnostic> {
        if argument.type_reference.is_valid()
            || argument.const_literal.is_some()
            || argument.evidence_projection.is_some()
        {
            return Ok(());
        }
        let source = argument
            .path
            .first()
            .zip(argument.path.last())
            .map(|(first, last)| {
                SourceSpan::new(
                    first.source_span().source_id,
                    source::Span::new(first.source_span().span.start, last.source_span().span.end),
                )
            });
        let selected = if !argument.symbol.is_valid()
            && let Some(source) = source
        {
            match (owner, argument.path.split_last()) {
                (Some(owner), Some((name, []))) => {
                    Some(members.in_owner(owner, name.as_str(), source)?)
                }
                (_, Some((name, prefix))) if !prefix.is_empty() => {
                    members.path(symbols, &prefix.joined_path(), name.as_str(), source)?
                }
                _ => None,
            }
        } else {
            None
        };
        if let Some(member) = selected {
            let at = source.unwrap_or_default();
            if !symbols.source_reference_can_see_symbol(at, member.machine) {
                return Err(error(
                    at,
                    "named implementation realization is not visible from this source",
                ));
            }
            argument.symbol = member.entry;
            if argument.path.iter().any(|name| name.is_source_backed()) {
                resolved.push(ResolvedStaticOperand {
                    source: at,
                    entry: member.entry,
                });
            }
        }
        let selected_owner = (argument.symbol.is_valid()
            && symbols.get(argument.symbol).kind == SymbolKind::Conformance)
            .then_some(argument.symbol);
        if let Some(application) = &mut argument.application {
            for nested in &mut application.arguments {
                bind_argument(nested, None, members, symbols, table, resolved, active)?;
            }
            if application.member.is_valid() {
                if active.contains(&application.member) {
                    return Err(Diagnostic::error("cyclic static member operand graph"));
                }
                let mut member = table
                    .static_member(application.member)
                    .ok_or_else(|| {
                        Diagnostic::error("static member lost its receiving expression table")
                    })?
                    .clone();
                active.push(application.member);
                let result = bind_argument(
                    &mut member,
                    selected_owner,
                    members,
                    symbols,
                    table,
                    resolved,
                    active,
                );
                active.pop();
                *table.static_member_mut(application.member).ok_or_else(|| {
                    Diagnostic::error("static member lost its receiving expression table")
                })? = member;
                result?;
            }
        }
        Ok(())
    }

    fn error(at: SourceSpan, message: &str) -> Diagnostic {
        Diagnostic::error(message).with_source_span(at)
    }
}

use crate::resolution::lowerer::Lowerer;
use diagnostics::Diagnostic;
use symbols::JoinedPath;

/*
    The kinds a top-level source name may resolve to — the lookup list
    selection checks share when they accept any visible declaration.
*/
pub(crate) const ANY_DECLARATION_LOOKUP_KINDS: &[symbols::SymbolKind] = &[
    symbols::SymbolKind::Module,
    symbols::SymbolKind::BuiltinType,
    symbols::SymbolKind::BuiltinFunction,
    symbols::SymbolKind::Data,
    symbols::SymbolKind::Domain,
    symbols::SymbolKind::Machine,
    symbols::SymbolKind::Operator,
    symbols::SymbolKind::Measure,
    symbols::SymbolKind::Proposition,
    symbols::SymbolKind::MathematicalDefinition,
    symbols::SymbolKind::Trait,
    symbols::SymbolKind::Conformance,
    symbols::SymbolKind::Const,
    symbols::SymbolKind::ConformanceParameter,
    symbols::SymbolKind::WireSchema,
];

/*
    Move every top-level operator into its exact domain's operator family.
    Runs before symbol assignment.
*/
pub(crate) fn select_operator_homes(lowerer: &mut Lowerer) -> Result<(), Vec<Diagnostic>> {
    domain_operator_homes::normalize_domain_operator_homes(
        &mut lowerer.symbol_resolved_trees,
        &lowerer.namespace_declarations,
    )
    .map_err(|diagnostic| vec![diagnostic])?;
    Ok(())
}

/*
    Record the authored-selection ledger: one exact occurrence per authored
    source token, after constants have been substituted.
*/
pub(crate) fn finalize_authored_selections(lowerer: &mut Lowerer) -> Result<(), Vec<Diagnostic>> {
    let const_roots = lowerer
        .pending_const_declarations
        .iter()
        .map(|pending| {
            lowerer
                .symbol_resolved_trees
                .const_declarations
                .get(pending.declaration)
        })
        .flat_map(|declaration| [declaration.initializer, declaration.authored_initializer])
        .filter(|expression| expression.is_valid())
        .collect::<Vec<_>>();
    bind_named_implementation_const_calls(&mut lowerer.symbol_resolved_trees, &const_roots)
        .map_err(|message| vec![Diagnostic::error(message)])?;
    authored_selections::normalize_case_membership_facts(
        &mut lowerer.symbol_resolved_trees,
        &lowerer.pending_authored_proof_memberships,
    )
    .map_err(|diagnostic| vec![diagnostic])?;
    let expression_work = lowerer
        .symbol_resolved_trees
        .tables
        .bodies
        .expressions
        .iter_expressions()
        .filter_map(|(expression, _)| {
            lowerer
                .seed
                .as_ref()
                .is_none_or(|seed| {
                    !seed
                        .source
                        .tables
                        .bodies
                        .expressions
                        .expression_is_valid(expression)
                })
                .then_some(expression)
        })
        .collect::<Vec<_>>();
    authored_selections::finalize_authored_expression_selections(
        &mut lowerer.symbol_resolved_trees,
        &expression_work,
        &lowerer.pending_authored_expressions,
        &lowerer.pending_authored_proof_memberships,
    )
    .map_err(|diagnostic| vec![diagnostic])?;
    Ok(())
}

/*
    Settle every remaining selection once the ledger exists: signature-free
    compatibility, machine-parameter requirements, evidence forwardings,
    closed conformance rows and their reference selections, establishment
    routes and service reaches. Resolution then normalizes the selected table
    work and routes conformance member requirement calls over those tables.
*/
pub(crate) fn finalize(lowerer: &mut Lowerer) -> Result<(), Vec<Diagnostic>> {
    let compatibility =
        validate_signature_free_requirement_compatibility(&lowerer.symbol_resolved_trees);
    if !compatibility.is_empty() {
        return Err(compatibility);
    }
    normalize_nominal_machine_parameter_requirements(&mut lowerer.symbol_resolved_trees)?;
    normalize_trait_machine_requirement_arguments(&mut lowerer.symbol_resolved_trees)
        .map_err(|diagnostic| vec![diagnostic])?;
    evidence_forwardings::bind_evidence_forwarding_owners(&mut lowerer.symbol_resolved_trees);
    let (pending_machine_service_reaches, pending_signature_service_reaches) =
        lowerer.pending_service_reaches()?;
    let machine_ordinals =
        conformance_blocks::normalize_closed_conformance_blocks(&mut lowerer.symbol_resolved_trees)
            .map_err(|diagnostic| vec![diagnostic])?;
    static_arguments::bind(&mut lowerer.symbol_resolved_trees)
        .map_err(|diagnostic| vec![diagnostic])?;
    lowerer.pending_machine_bodies.retain_mut(|pending| {
        match machine_ordinals
            .get(pending.machine_ordinal)
            .copied()
            .flatten()
        {
            Some(ordinal) => {
                pending.machine_ordinal = ordinal;
                true
            }
            None => false,
        }
    });
    finalize_conformance_reference_selections(&mut lowerer.symbol_resolved_trees)
        .map_err(|diagnostic| vec![diagnostic])?;
    let seed = lowerer.seed.take();
    domain_establishment::normalize_domain_establishment_routes(
        &mut lowerer.symbol_resolved_trees,
        seed.as_ref().map(|seed| seed.source.as_ref()),
    )
    .map_err(|diagnostic| vec![diagnostic])?;
    match seed {
        None => service_reaches::normalize_service_reaches(
            &mut lowerer.symbol_resolved_trees,
            &pending_machine_service_reaches,
            &pending_signature_service_reaches,
        ),
        Some(seed) => service_reaches::normalize_service_reaches_with_retained_tables(
            &mut lowerer.symbol_resolved_trees,
            &pending_machine_service_reaches,
            &pending_signature_service_reaches,
            seed.source.service_reaches.clone(),
            seed.source.service_reach_rows.clone(),
        ),
    }
    .map_err(|diagnostic| vec![diagnostic])?;
    Ok(())
}

/*
    The authored-nominal machine contract beneath a type parameter, when it
    carries one, with its `::`-rendered requirement path.
*/
pub(super) fn authored_nominal_requirement(
    parameter: &symbol_resolved_trees::data::TypeParameter,
) -> Option<(
    &Vec<symbol_resolved_trees::name::DiagnosticName>,
    &arena::HandleSpan<symbol_resolved_trees::types::TypeReference>,
    String,
)> {
    let symbol_resolved_trees::data::TypeParameterKind::Machine {
        contract:
            symbol_resolved_trees::data::MachineParameterContract::AuthoredNominal {
                requirement,
                trait_arguments,
            },
    } = &parameter.kind
    else {
        return None;
    };
    let rendered = requirement.joined_path();
    Some((requirement, trait_arguments, rendered))
}

/*
    The `TypeReference` carriers that cannot hold a member or collection —
    the no-op tail several selection walks repeat verbatim.
*/
macro_rules! leaf_carrier_type_reference_pattern {
    () => {
        symbol_resolved_trees::types::TypeReference::FixedArray(_)
            | symbol_resolved_trees::types::TypeReference::Slice(_)
            | symbol_resolved_trees::types::TypeReference::Generic(_)
            | symbol_resolved_trees::types::TypeReference::ConstExpression(_)
            | symbol_resolved_trees::types::TypeReference::Callable { .. }
            | symbol_resolved_trees::types::TypeReference::DynamicTrait { .. }
            | symbol_resolved_trees::types::TypeReference::Named { .. }
            | symbol_resolved_trees::types::TypeReference::Unit
    };
}
use crate::constant::initializer_dependencies::bind_named_implementation_const_calls;
use authored_selections::finalize_conformance_reference_selections;
pub(super) use leaf_carrier_type_reference_pattern;
use machine_parameter_requirements::normalize_nominal_machine_parameter_requirements;
use machine_parameter_requirements::normalize_trait_machine_requirement_arguments;
use signature_free_requirements::validate_signature_free_requirement_compatibility;

#[cfg(test)]
mod runtime_callable_reach_tests {
    use std::sync::Arc;
    use symbol_resolved_trees::SymbolResolvedTrees;

    fn parse(text: &str, source: source::SourceId) -> syntax_trees::SyntaxTrees {
        let tokens = source_files_to_tokens::Lexer::new(text).tokenize().unwrap();
        tokens_to_syntax_trees::parse_syntax_trees_with_id(source, &tokens).unwrap()
    }

    fn check_rows(program: &SymbolResolvedTrees) {
        let contracts = program
            .tables
            .declarations
            .runtime_callable_contracts
            .iter()
            .map(|(_, contract)| &contract.signature)
            .collect::<Vec<_>>();
        assert_eq!(contracts.len(), 3);
        let service = program
            .traits
            .iter()
            .find(|declaration| declaration.name.as_str() == "Io")
            .unwrap()
            .symbol;
        let service_id = program.service_reaches.id_for_symbol(service).unwrap();
        for (position, signature) in contracts.iter().enumerate() {
            let authored = program
                .authored_service_reach_rows_for(signature.symbol)
                .collect::<Vec<_>>();
            let normalized = program
                .service_reach_rows
                .services(signature.service_reach_row);
            assert!(
                signature.blocks,
                "operational metadata remains independent of reach"
            );
            match position {
                0 => {
                    assert_eq!(
                        authored.len(),
                        1,
                        "explicit reaches none retains writtenness"
                    );
                    assert_eq!(authored[0].owner, signature.symbol);
                    assert!(!authored[0].keyword_source_spans.is_empty());
                    assert!(authored[0].targets.is_empty());
                    assert!(normalized.is_empty());
                }
                1 => {
                    assert_eq!(authored.len(), 1);
                    assert_eq!(authored[0].owner, signature.symbol);
                    assert_eq!(authored[0].targets.len(), 1);
                    assert_eq!(authored[0].targets[0].service, service);
                    assert_eq!(normalized, &[service_id]);
                }
                2 => {
                    assert!(
                        authored.is_empty(),
                        "omission must not become authored reaches none"
                    );
                    assert!(normalized.is_empty());
                }
                _ => unreachable!(),
            }
        }
    }

    #[test]
    fn runtime_callable_reach_rows_keep_authored_and_omitted_ceilings_across_seeding() {
        let text = r#"
boundary trait Io { machine read() reaches none crashes none; }
machine hold(
    first: machine() -> u32 reaches none blocks crashes none,
    second: machine() -> u32 reaches Io blocks crashes none,
    third: machine() -> u32 blocks crashes none,
    value: u32
) {}
"#;
        let mut sources = source::SourceMap::default();
        let source = sources
            .add("callable/base.omg".into(), text.into())
            .source_id;
        let sources = Arc::new(sources);
        let base = crate::resolve(crate::ResolutionRequest {
            syntax: parse(text, source),
            sources: Some(Arc::clone(&sources)),
            top_level_bindings: Vec::new(),
            retained_base: None,
        })
        .unwrap();
        check_rows(&base);
        let original_rows = base.authored_service_reach_rows.clone();
        let original_contracts = base
            .tables
            .declarations
            .runtime_callable_contracts
            .iter()
            .map(|(handle, contract)| (handle, contract.signature.clone()))
            .collect::<Vec<_>>();
        let mut extended_sources = sources.as_ref().clone();
        let added = "machine extension() {}";
        let extension = extended_sources
            .add("callable/extension.omg".into(), added.into())
            .source_id;
        let resumed = crate::resolve_extension(crate::ExtensionRequest {
            base: Arc::new(base),
            syntax: parse(added, extension),
            sources: Arc::new(extended_sources),
            top_level_bindings: Vec::new(),
        })
        .unwrap();
        check_rows(resumed.trees());
        assert_eq!(resumed.trees().authored_service_reach_rows, original_rows);
        for (handle, signature) in original_contracts {
            assert_eq!(
                resumed
                    .trees()
                    .tables
                    .declarations
                    .runtime_callable_contracts
                    .get(handle)
                    .signature,
                signature
            );
        }
    }
}
