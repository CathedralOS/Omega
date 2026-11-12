/*
    Typed probe evaluation of closed integer and Boolean const-generic
    expressions, lexical selection verification, call custody, and scalar
    `match` dispatch rational-bound lattices.

    We evaluate closed const-generic arguments and scalar dispatches in isolated
    typed probes while preserving original lexical selections, exact rational
    intermediate landing rules, and call custody.
*/

use crate::machine_execution::admission::require_const_expression_selection;
use crate::syntax_probes::prepare_call_free_probe_typing;
use numerics::arithmetic::ArithmeticDomain;
use symbols::SymbolHandle;
use syntax_trees_to_symbol_resolved_trees::pre_resolution::resolve_deferred_initializer_probe;
use validation::has_builtin_binary_expression_meaning;

mod call_custody {
    use crate::build_time_evaluation::expression_table;
    use crate::unique::push_unique;
    use language_semantics::declaration_selection::{
        AuthoredDeclarationSelectionKind as Kind, AuthoredDeclarationSelectionTarget as Target,
        ResolvedAuthoredCallBinding,
    };
    use source::SourceSpan;
    use symbols::{SymbolHandle, SymbolKind};
    use syntax_trees::{SyntaxTrees, types::ConstArgumentOrigin};
    use typed_trees::{
        TypedTrees,
        expression::{ExpressionHandle, ExpressionNode},
        machine::Machine,
        state::State,
        statement::{StatementNode, TransitionGuardNode, TransitionTargetNode},
    };

    mod call_selections {
        use crate::build_time_evaluation::type_reference_table;
        use symbols::{SymbolHandle, SymbolKind};
        use typed_trees::expression::TableCallExpression;
        use typed_trees::{TypedTrees, static_applications::StaticMachineBindingValue};
        use validation::CallSite;

        pub(super) fn call_custody_source(
            program: &TypedTrees,
            target: SymbolHandle,
        ) -> Result<source::SourceSpan, String> {
            if let Some(requirement) = program.evidence_requirement_declaration(target)? {
                return program
                    .symbols
                    .symbol_source_span(requirement.symbol)
                    .ok_or_else(|| {
                        "constant evidence requirement lost its public declaration source".into()
                    });
            }
            if let Some(source) = program.symbols.symbol_source_span(target) {
                return Ok(source);
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
                    .ok_or_else(|| {
                        "constant logical call lost its exact authored provenance".into()
                    });
            }
            if symbol.kind != SymbolKind::State
                || program.symbols.get(symbol.parent).kind != SymbolKind::Machine
            {
                return Err("constant call lost its exact selected declaration source".into());
            }
            let mut owners = program
                .machines()
                .iter()
                .filter(|owner| owner.symbol == symbol.parent);
            let owner = owners
                .next()
                .ok_or("constant callable entry lost its owning machine")?;
            if owners.next().is_some() {
                return Err("constant callable entry has ambiguous owning machine identity".into());
            }
            let states = program.machine_states(owner);
            if states.len() != owner.states.len()
                || states.first().map(|state| state.symbol) != Some(target)
            {
                return Err("constant source-less call is not its exact owning entry".into());
            }
            program
                .symbols
                .symbol_source_span(symbol.parent)
                .ok_or_else(|| {
                    "constant callable entry lost its authored machine declaration".into()
                })
        }

        macro_rules! selected_requirement {
            ($program:ident, $call:ident, $authored:ident, $requirement_trait:ident, $conformance:ident) => {
                let authored_symbol = $program.symbols.get($authored);
                if authored_symbol.kind != SymbolKind::State
                    || $program.symbols.get(authored_symbol.parent).kind != SymbolKind::Trait
                {
                    return None;
                }
                let $requirement_trait = authored_symbol.parent;
                let implementation = if let Some(path) =
                    $program.expression_table.name_path_node($call.receiver)
                    && $program.symbols.get(path.symbol).kind == SymbolKind::Conformance
                {
                    path.symbol
                } else {
                    let argument = $call.machine_arguments.last()?;
                    if !argument.names_implementation(&$program.symbols) {
                        return None;
                    }
                    argument.symbol
                };
                let $conformance = $program.conformance_by_symbol(implementation)?;
            };
        }

        pub(super) fn owned_realization_entry(
            program: &TypedTrees,
            call: &TableCallExpression,
            authored: SymbolHandle,
        ) -> Option<SymbolHandle> {
            selected_requirement!(program, call, authored, requirement_trait, conformance);
            if !trait_supplies_requirement(program, conformance.trait_symbol, requirement_trait) {
                return None;
            }
            let mut rows = conformance.rows.iter().filter(|row| {
                (row.requirement.is_valid() && row.requirement == authored)
                    || row.requirement_name.as_str() == call.target.as_str()
            });
            let row = rows.next()?;
            if rows.next().is_some() || !row.realization_state.is_valid() {
                return None;
            }
            Some(row.realization_state)
        }

        pub(super) fn unsupplied_requirement(
            program: &TypedTrees,
            call: &TableCallExpression,
            authored: SymbolHandle,
        ) -> Option<String> {
            selected_requirement!(program, call, authored, requirement_trait, conformance);
            if trait_supplies_requirement(program, conformance.trait_symbol, requirement_trait) {
                return None;
            }
            let implemented = format!(
                "{}<{}>",
                conformance.trait_name.as_str(),
                type_reference_table(program)
                    .type_reference_handles(conformance.arguments)
                    .iter()
                    .map(|argument| program.display_type_reference(*argument))
                    .collect::<Vec<_>>()
                    .join(", ")
            );
            let name = program.symbols.name(conformance.symbol);
            Some(format!(
                "`{name}` implements `{implemented}`, which neither is nor inherits `{}`, the trait that declares `{}`",
                program.symbols.display_path(requirement_trait, "::"),
                program.symbols.display_path(authored, "::"),
            ))
        }

        pub(super) fn requirement_realized_by(
            program: &TypedTrees,
            requirement: SymbolHandle,
            target: SymbolHandle,
        ) -> bool {
            if !requirement.is_valid() || !target.is_valid() || requirement == target {
                return false;
            }
            program.conformances().iter().any(|conformance| {
                conformance.rows.iter().any(|row| {
                    row.requirement == requirement
                        && row.realization_state.is_valid()
                        && (row.realization_state == target
                            || same_authored_declaration(program, row.realization_state, target))
                })
            })
        }

        fn same_authored_declaration(
            program: &TypedTrees,
            left: SymbolHandle,
            right: SymbolHandle,
        ) -> bool {
            program.symbols.get(left).kind == program.symbols.get(right).kind
                && program.symbols.symbol_source_span(left).is_some()
                && program.symbols.symbol_source_span(left)
                    == program.symbols.symbol_source_span(right)
        }

        fn trait_supplies_requirement(
            program: &TypedTrees,
            implementation_trait: SymbolHandle,
            requirement_trait: SymbolHandle,
        ) -> bool {
            if !implementation_trait.is_valid() || !requirement_trait.is_valid() {
                return false;
            }
            let mut pending = vec![implementation_trait];
            let mut visited = Vec::new();
            while let Some(current) = pending.pop() {
                if !current.is_valid() || visited.contains(&current) {
                    continue;
                }
                if current == requirement_trait {
                    return true;
                }
                visited.push(current);
                let Some(definition) = program.trait_definition_by_symbol(current) else {
                    continue;
                };
                pending.extend(
                    program
                        .trait_requirements(definition)
                        .iter()
                        .map(|parent| parent.symbol),
                );
            }
            false
        }

        pub(super) fn retained_target(
            program: &TypedTrees,
            site: &CallSite<'_>,
        ) -> Result<SymbolHandle, String> {
            let target = match site {
                CallSite::Statement(call) => call.target_symbol,
                CallSite::Expression { call, .. } => call.target_symbol,
                CallSite::TransitionNamed { path, .. } => path.symbol,
            };
            callable_entry(program, target)
        }

        pub(super) fn callable_entry(
            program: &TypedTrees,
            target: SymbolHandle,
        ) -> Result<SymbolHandle, String> {
            if program.symbols.get(target).kind != SymbolKind::Machine {
                return Ok(target);
            }
            let mut machines = program
                .machines()
                .iter()
                .filter(|machine| machine.symbol == target);
            let machine = machines
                .next()
                .ok_or("constant call lost its selected machine")?;
            if machines.next().is_some() {
                return Err("constant call has ambiguous selected machine identity".into());
            }
            let states = program.machine_states(machine);
            if states.len() != machine.states.len() {
                return Err("constant call selected machine has stale states".into());
            }
            states
                .first()
                .map(|state| state.symbol)
                .ok_or_else(|| "constant call selected machine has no entry state".into())
        }

        pub(super) fn static_callable_matches(
            program: &TypedTrees,
            owner: SymbolHandle,
            site: &CallSite<'_>,
            authored: SymbolHandle,
        ) -> Result<bool, String> {
            let parameter = site.static_machine_parameter();
            if parameter != authored || program.retained_static_machine_contract(authored).is_none()
            {
                return Ok(false);
            }
            program
                .static_machine_binding_selects_target(
                    owner,
                    authored,
                    retained_target(program, site)?,
                )
                .map_err(str::to_owned)
        }

        pub(super) fn logical_requirement_matches(
            program: &TypedTrees,
            owner: SymbolHandle,
            lexical_owner: SymbolHandle,
            site: &CallSite<'_>,
            authored: SymbolHandle,
        ) -> Result<bool, String> {
            let source = program.symbols.get(authored);
            if source.kind != SymbolKind::State || !source.parent.is_valid() {
                return Ok(false);
            }
            let binder = program.symbols.get(source.parent);
            if binder.kind == SymbolKind::Trait {
                let Some(dispatch) = site.static_requirement_dispatch() else {
                    return Ok(false);
                };
                if dispatch.requirement != authored || dispatch.declaring_trait != source.parent {
                    return Ok(false);
                }
                let (_, signature) =
                    validation::resolve_requirement_contract(program, lexical_owner, site)
                        .map_err(|reason| {
                            format!(
                                "constant selected requirement is not admitted: {}",
                                logical_requirement_refusal(
                                    program,
                                    owner,
                                    lexical_owner,
                                    site,
                                    reason
                                )
                            )
                        })?;
                let template = program
                    .closed_requirement_application(signature.symbol)
                    .map_or(signature.symbol, |application| application.template);
                return Ok(template == authored);
            }
            if binder.kind != SymbolKind::ConformanceParameter {
                return Ok(false);
            }
            let Some(declaration) = program.machine_by_symbol(binder.parent) else {
                return Ok(false);
            };
            let Some(requirement) =
                validation::named_conformance_target_requirement(program, declaration, authored)?
            else {
                return Ok(false);
            };
            let (selected_trait, selected_signature) = validation::resolve_requirement_contract(
                program,
                lexical_owner,
                site,
            )
            .map_err(|reason| {
                format!(
                    "constant evidence requirement is not admitted: {}",
                    logical_requirement_refusal(program, owner, lexical_owner, site, reason)
                )
            })?;
            let template = program
                .closed_requirement_application(selected_signature.symbol)
                .map_or(selected_signature.symbol, |application| {
                    application.template
                });
            if template != requirement.signature.symbol {
                return Ok(false);
            }
            let mut current = owner;
            let mut visited = Vec::new();
            let mut source_invocation = None;
            while current != declaration.symbol {
                if visited.contains(&current) {
                    return Err("constant logical call has cyclic invocation ownership".into());
                }
                visited.push(current);
                let Some(template) = program.executable_application_template(current)? else {
                    return Ok(false);
                };
                if template == declaration.symbol {
                    let instances = program
                        .machine_specializations
                        .iter()
                        .filter(|instance| {
                            instance.instance == current && instance.template == template
                        })
                        .collect::<Vec<_>>();
                    let [instance] = instances.as_slice() else {
                        return Ok(false);
                    };
                    source_invocation = Some(*instance);
                }
                current = template;
            }
            if let Some(dispatch) = site.static_requirement_dispatch() {
                let Some(instance) = source_invocation else {
                    return Ok(false);
                };
                let Some(StaticMachineBindingValue::Conformance(argument)) =
                    program.specialization_binding(instance, source.parent)
                else {
                    return Ok(false);
                };
                let expected = validation::conformance_applications::close_conformance_application(
                    program, argument,
                )
                .map_err(|diagnostic| diagnostic.message)?;
                let applications = program.selected_conformance_applications(
                    owner,
                    dispatch.selection_application,
                    dispatch.evidence_parameter,
                    dispatch.requirement,
                    dispatch.application_report_fingerprint,
                    dispatch.application_commitment,
                );
                let [selected] = applications.as_slice() else {
                    return Ok(false);
                };
                let mut selected = (*selected).clone();
                selected.callable_application = Default::default();
                selected.trait_scope = Default::default();
                return Ok(expected == selected);
            }
            if source_invocation.is_some() {
                return Ok(false);
            }
            let Some(scope) = requirement
                .application
                .declaring_scope(program, selected_trait.symbol)
            else {
                return Ok(false);
            };
            let expected = program
                .applied_callable_signature(&scope, requirement.signature.symbol)
                .or_else(|| {
                    (scope.substitutions.is_empty()
                        && scope.declaration.lifetime_parameters.is_empty())
                    .then_some(requirement.signature)
                });
            Ok(expected == Some(selected_signature))
        }

        fn logical_requirement_refusal(
            program: &TypedTrees,
            owner: SymbolHandle,
            lexical_owner: SymbolHandle,
            site: &CallSite<'_>,
            reason: &'static str,
        ) -> String {
            let context = format!("execution owner {owner:?}, source owner {lexical_owner:?}");
            if reason == "selected call has no exact applied public signature"
                && let CallSite::Expression { expression, call } = site
                && let Err(unavailable) =
                    program.resolve_requirement_call_signature(lexical_owner, call)
            {
                return format!("{reason}; {context}, expression {expression:?}: {unavailable}");
            }
            format!("{reason}; {context}")
        }
    }
    mod types {
        use super::{Collector, Context};
        use crate::build_time_evaluation::type_reference_table;
        use crate::unique::push_unique;
        use symbols::{SymbolHandle, SymbolKind};
        use typed_trees::domain::ProofFact;
        use typed_trees::signature::SignatureContract;
        use typed_trees::{
            TypedTrees,
            expression::ExpressionHandle,
            machine::Machine,
            references::{self, TypedReferences},
            types::{TypeConstantOrigin, TypeReferenceHandle},
        };

        pub(super) enum Root<'program> {
            Type(TypeReferenceHandle),
            Expression(ExpressionHandle),
            Machine(&'program Machine),
            Contracts(arena::HandleSpan<SignatureContract>),
        }

        impl<'program> Collector<'program> {
            pub(super) fn type_dependencies(
                &mut self,
                root: Root<'program>,
                context: Context<'program>,
            ) -> Result<(), String> {
                let program = self.program;
                let mut inventory = TypeDependencies {
                    program,
                    root_expression: ExpressionHandle::invalid(),
                    lexical_owner: context.lexical_owner,
                    contract_owner: None,
                    expressions: Vec::new(),
                    types: Vec::new(),
                    pending_data: Vec::new(),
                    visited_data: Vec::new(),
                    current_data: SymbolHandle::invalid(),
                    origins: Vec::new(),
                    error: None,
                };
                match root {
                    Root::Type(reference) => {
                        references::visit_type_reference(program, &mut inventory, reference)
                    }
                    Root::Expression(expression) => {
                        inventory.root_expression = expression;
                        references::visit_expression(program, &mut inventory, expression);
                    }
                    Root::Machine(machine) => {
                        references::visit_machine(program, &mut inventory, machine)
                    }
                    Root::Contracts(contracts) => {
                        inventory.contracts(contracts, context.lexical_owner)
                    }
                }
                while let Some(symbol) = inventory.pending_data.pop() {
                    if inventory.visited_data.contains(&symbol) {
                        continue;
                    }
                    inventory.visited_data.push(symbol);
                    if program
                        .data_definitions()
                        .iter()
                        .filter(|data| data.symbol == symbol)
                        .count()
                        != 1
                    {
                        return Err(
                            "constant type dependency lost its exact data declaration".into()
                        );
                    }
                    inventory.current_data = symbol;
                    references::visit_declarations(program, &mut inventory);
                }
                if let Some(error) = inventory.error {
                    return Err(error.into());
                }
                for origin in inventory.origins {
                    self.constant(
                        origin.declaration,
                        origin.reference,
                        ExpressionHandle::invalid(),
                        context,
                    )?;
                }
                self.expressions
                    .extend(inventory.expressions.into_iter().map(
                        |(expression, lexical_owner)| {
                            (
                                expression,
                                Context {
                                    lexical_owner,
                                    initializer: false,
                                    ..context
                                },
                            )
                        },
                    ));
                Ok(())
            }
        }

        struct TypeDependencies<'program> {
            program: &'program TypedTrees,
            root_expression: ExpressionHandle,
            lexical_owner: SymbolHandle,
            contract_owner: Option<SymbolHandle>,
            expressions: Vec<(ExpressionHandle, SymbolHandle)>,
            types: Vec<TypeReferenceHandle>,
            pending_data: Vec<SymbolHandle>,
            visited_data: Vec<SymbolHandle>,
            current_data: SymbolHandle,
            origins: Vec<TypeConstantOrigin>,
            error: Option<&'static str>,
        }

        impl TypeDependencies<'_> {
            fn contracts(
                &mut self,
                contracts: arena::HandleSpan<SignatureContract>,
                owner: SymbolHandle,
            ) {
                let program = self.program;
                let clauses = program.tables.signature_contracts.span_or_empty(contracts);
                if clauses.len() != contracts.len() {
                    self.error = Some("constant custody has stale signature clauses");
                    return;
                }
                for clause in clauses {
                    self.contract_facts(clause.facts, owner);
                }
            }

            fn contract_facts(&mut self, facts: arena::HandleSpan<ProofFact>, owner: SymbolHandle) {
                let program = self.program;
                let clauses = program.proof_facts.span_or_empty(facts);
                if clauses.len() != facts.len() {
                    self.error = Some("constant custody has stale contract facts");
                    return;
                }
                let previous = self.contract_owner.replace(owner);
                for fact in clauses {
                    references::visit_proof_fact(program, self, fact);
                }
                self.contract_owner = previous;
            }
        }

        impl Extend<SymbolHandle> for TypeDependencies<'_> {
            fn extend<T: IntoIterator<Item = SymbolHandle>>(&mut self, symbols: T) {
                for symbol in symbols {
                    let symbol = if self.program.symbols.get(symbol).kind == SymbolKind::Variant {
                        self.program.symbols.get(symbol).parent
                    } else {
                        symbol
                    };
                    if self.program.symbols.get(symbol).kind == SymbolKind::Data {
                        self.pending_data.push(symbol);
                    }
                }
            }
        }

        impl TypedReferences for TypeDependencies<'_> {
            fn callable_signature(&mut self, signature: &typed_trees::signature::StateSignature) {
                self.contracts(signature.contracts, signature.symbol);
            }

            fn declaration(&mut self, symbol: SymbolHandle) -> bool {
                symbol == self.current_data
            }

            fn type_reference(&mut self, reference: TypeReferenceHandle) -> bool {
                if self.types.contains(&reference) {
                    return false;
                }
                self.types.push(reference);
                match type_reference_table(self.program).constant_origins(reference) {
                    Ok(origins) => {
                        for origin in origins {
                            push_unique(&mut self.origins, *origin);
                        }
                        true
                    }
                    Err(error) => {
                        self.error = Some(error);
                        false
                    }
                }
            }

            fn expression(&mut self, expression: ExpressionHandle) -> bool {
                if let Some(owner) = self.contract_owner {
                    self.expressions.push((expression, owner));
                    return false;
                }
                if expression == self.root_expression {
                    self.root_expression = ExpressionHandle::invalid();
                    return true;
                }
                false
            }

            fn type_expression(&mut self, expression: ExpressionHandle) -> bool {
                if expression.is_valid() {
                    self.expressions.push((
                        expression,
                        self.contract_owner.unwrap_or(self.lexical_owner),
                    ));
                }
                false
            }

            fn proof_fact(&mut self, _: arena::Handle<ProofFact>) -> bool {
                false
            }
        }
    }

    #[derive(Default)]
    pub(super) struct Custody {
        pub origins: Vec<ConstArgumentOrigin>,
        pub operators: Vec<SourceSpan>,
        pub calls: Vec<(SourceSpan, SourceSpan)>,
        pub pending_constants: Vec<(SourceSpan, SourceSpan)>,
        call_targets: Vec<(SourceSpan, SymbolHandle)>,
        call_occurrences: Vec<typed_trees::AuthoredDeclarationSelectionOccurrenceId>,
        dependency_values: Vec<DependencyValue>,
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub(crate) struct DependencyValue {
        pub declaration: SymbolHandle,
        pub expression: ExpressionHandle,
    }

    #[derive(Clone, Copy)]
    struct Context<'program> {
        machine: &'program Machine,
        state: &'program State,
        lexical_owner: SymbolHandle,
        initializer: bool,
    }

    pub(super) fn contains_call(
        program: &TypedTrees,
        root: ExpressionHandle,
    ) -> Result<bool, String> {
        let mut contains_call = false;
        for_each_initializer_expression(program, &[root], expression_children, |expression| {
            contains_call |= matches!(
                program.expression_table.expression(expression),
                ExpressionNode::Call(_)
            );
            true
        })?;
        Ok(contains_call)
    }

    fn for_each_initializer_expression(
        program: &TypedTrees,
        roots: &[ExpressionHandle],
        children_at: impl Fn(&TypedTrees, ExpressionHandle) -> Result<Vec<ExpressionHandle>, String>,
        mut visit: impl FnMut(ExpressionHandle) -> bool,
    ) -> Result<(), String> {
        let mut pending: Vec<(ExpressionHandle, bool)> =
            roots.iter().rev().map(|root| (*root, false)).collect();
        let mut active = Vec::new();
        let mut complete = Vec::new();
        while let Some((expression, finish)) = pending.pop() {
            if finish {
                if active.pop() != Some(expression) {
                    return Err("constant initializer has invalid expression traversal".into());
                }
                complete.push(expression);
                continue;
            }
            if active.contains(&expression) {
                return Err("constant initializer contains a cyclic expression".into());
            }
            if complete.contains(&expression) {
                continue;
            }
            let children = children_at(program, expression)?;
            let descend = visit(expression);
            active.push(expression);
            pending.push((expression, true));
            if descend {
                pending.extend(children.into_iter().rev().map(|child| (child, false)));
            }
        }
        Ok(())
    }

    pub(super) fn immediate_call_expressions(
        program: &TypedTrees,
        roots: &[ExpressionHandle],
    ) -> Result<Vec<ExpressionHandle>, String> {
        let mut calls = Vec::new();
        for_each_initializer_expression(
            program,
            roots,
            expression_operand_children,
            |expression| {
                if matches!(
                    program.expression_table.expression(expression),
                    ExpressionNode::Call(_)
                ) {
                    calls.push(expression);
                    false
                } else {
                    true
                }
            },
        )?;
        Ok(calls)
    }

    fn expression_children(
        program: &TypedTrees,
        expression: ExpressionHandle,
    ) -> Result<Vec<ExpressionHandle>, String> {
        let mut children = expression_operand_children(program, expression)?;
        let authored = program.expression_table.materialization_source(expression);
        if authored.is_valid() {
            children.insert(0, authored);
        }
        Ok(children)
    }

    fn expression_operand_children(
        program: &TypedTrees,
        expression: ExpressionHandle,
    ) -> Result<Vec<ExpressionHandle>, String> {
        let table = &program.expression_table;
        if !table.expression_is_valid(expression) {
            return Err("constant initializer contains a stale expression".into());
        }
        let mut children = Vec::new();
        match table.expression(expression) {
            ExpressionNode::SavedSubject(_) => {}
            ExpressionNode::Call(call) => {
                if call.receiver.is_valid() {
                    children.push(call.receiver);
                }
                let arguments = table.expression_handles(call.arguments);
                if arguments.len() != call.arguments.len() {
                    return Err("constant call closure has stale argument span".into());
                }
                children.extend(arguments.iter().copied());
            }
            ExpressionNode::Binary(binary) => children.extend([binary.left, binary.right]),
            ExpressionNode::Unary(unary) => children.push(unary.operand),
            ExpressionNode::Cast(cast) => children.push(cast.value),
            ExpressionNode::Member(member) => children.push(member.receiver),
            ExpressionNode::Borrow(borrow) => children.push(borrow.target),
            ExpressionNode::Indexed(indexed) => {
                children.extend([indexed.collection, indexed.index])
            }
            ExpressionNode::Atomic(atomic) => {
                children.push(atomic.value);
                if atomic.result.is_valid() {
                    children.push(atomic.result);
                }
            }
            ExpressionNode::Range(range) => children.extend(
                [range.start, range.end]
                    .into_iter()
                    .filter(|handle| handle.is_valid()),
            ),
            ExpressionNode::ArrayLiteral(elements) => {
                let values = table.expression_handles(*elements);
                if values.len() != elements.len() {
                    return Err("constant call closure has stale array span".into());
                }
                children.extend(values.iter().copied());
            }
            ExpressionNode::StructLiteral(literal) => {
                let fields = table.struct_fields(literal.fields);
                if fields.len() != literal.fields.len() {
                    return Err("constant call closure has stale field span".into());
                }
                children.extend(fields.iter().map(|field| field.value));
            }
            ExpressionNode::Match(dispatch) => {
                let arms = table.match_arms(dispatch.arms);
                if arms.len() != dispatch.arms.len() {
                    return Err("constant call closure has stale Match span".into());
                }
                children.push(dispatch.subject);
                for arm in arms {
                    children.extend(table.match_pattern_dependencies(arm.pattern));
                    children.push(arm.value);
                }
            }
            ExpressionNode::Name(_)
            | ExpressionNode::Integer(_)
            | ExpressionNode::Float(_)
            | ExpressionNode::UndeterminedNaN(_)
            | ExpressionNode::Boolean(_)
            | ExpressionNode::String(_)
            | ExpressionNode::TypeExpression(_)
            | ExpressionNode::ZeroValue(_) => {}
        }
        Ok(children)
    }

    macro_rules! inner {
        ($a:expr => $b:expr => $c:expr => $d:expr => $e:expr => $f:expr => $g:expr => $h:expr) => {
            collect_internal($a, $b, $c, $d, $e, $f, $g, $h)
        };
    }

    pub(super) fn collect(
        program: &TypedTrees,
        machine: &Machine,
        state: &State,
        root: ExpressionHandle,
        public: bool,
        syntax: &SyntaxTrees,
    ) -> Result<Custody, String> {
        inner!(program => machine => state => &[root] => public => Some(syntax) => true => true)
    }

    pub(super) fn collect_authored_roots(
        program: &TypedTrees,
        machine: &Machine,
        state: &State,
        roots: &[ExpressionHandle],
        syntax: &SyntaxTrees,
    ) -> Result<Custody, String> {
        inner!(program => machine => state => roots => false => Some(syntax) => false => true)
    }

    pub(super) fn collect_roots(
        program: &TypedTrees,
        machine: &Machine,
        state: &State,
        roots: &[ExpressionHandle],
        public: bool,
        syntax: &SyntaxTrees,
    ) -> Result<Custody, String> {
        inner!(program => machine => state => roots => public => Some(syntax) => true => true)
    }

    pub(super) fn collect_pending_roots(
        program: &TypedTrees,
        machine: &Machine,
        state: &State,
        roots: &[ExpressionHandle],
        public: bool,
        syntax: &SyntaxTrees,
    ) -> Result<Custody, String> {
        inner!(program => machine => state => roots => public => Some(syntax) => true => false)
    }

    pub(crate) fn retain_current_initializer_calls(
        program: &mut TypedTrees,
        owner: symbols::SymbolHandle,
        root: ExpressionHandle,
    ) -> Result<(), String> {
        let machine = program
            .machine(owner)
            .ok_or("pending initializer lost its custody owner")?;
        let [state] = program.machine_states(machine) else {
            return Err("pending initializer lost its custody state".into());
        };
        let custody = collect_internal(program, machine, state, &[root], false, None, false, true)?;
        let mut targets = Vec::new();
        for occurrence in &custody.call_occurrences {
            let selection = program
                .authored_declaration_selections()
                .get(*occurrence)
                .ok_or("pending initializer lost its original call occurrence")?;
            let target = selection
                .target()
                .as_resolved()
                .ok_or("pending initializer retained an unresolved call occurrence")?
                .selected_symbol();
            if selection.kind() != Kind::Call {
                return Err("pending initializer changed its original selection kind".into());
            }
            let target = (selection.source_span(), target);
            push_unique(&mut targets, target);
        }
        if targets.len() != custody.call_targets.len()
            || custody
                .call_targets
                .iter()
                .any(|target| !targets.contains(target))
        {
            return Err("pending initializer call closure lost original occurrence custody".into());
        }
        for occurrence in custody.call_occurrences {
            program
                .expression_table
                .attach_authored_selection_occurrences(root, [occurrence]);
        }
        Ok(())
    }

    pub(super) fn validate_retained(
        program: &TypedTrees,
        machine: &Machine,
        state: &State,
        declaration: &typed_trees::constant::ConstDeclaration,
    ) -> Result<Vec<DependencyValue>, String> {
        validate(program, machine, state, declaration, true)
    }

    pub(super) fn validate_authored(
        program: &TypedTrees,
        machine: &Machine,
        state: &State,
        declaration: &typed_trees::constant::ConstDeclaration,
    ) -> Result<Vec<DependencyValue>, String> {
        validate(program, machine, state, declaration, false)
    }

    fn validate(
        program: &TypedTrees,
        machine: &Machine,
        state: &State,
        declaration: &typed_trees::constant::ConstDeclaration,
        specialized_calls: bool,
    ) -> Result<Vec<DependencyValue>, String> {
        let root = declaration.authored_initializer;
        let materialized = declaration.materialized_initializer;
        if !program.expression_table.expression_is_valid(materialized) {
            return Err("constant call receipt has a stale materialized root".into());
        }
        let mut expected = Vec::new();
        for occurrence in
            expression_table(program).authored_selection_occurrences(if specialized_calls {
                materialized
            } else {
                root
            })
        {
            let selection = program
                .authored_declaration_selections()
                .get(occurrence)
                .ok_or("constant call receipt lost an authored selection")?;
            if selection.kind() != Kind::Call {
                continue;
            }
            let selection_target = selection.target();
            let Some(selected) = selection_target.as_resolved() else {
                return Err("constant call receipt has no exact selected target".into());
            };
            let target = (selection.source_span(), selected.selected_symbol());
            push_unique(&mut expected, target);
        }
        let actual = collect_in_context(
            program,
            Context {
                machine,
                state,
                lexical_owner: declaration.symbol,
                initializer: true,
            },
            &[root],
            false,
            None,
            specialized_calls,
            true,
        )?;
        if actual.call_targets.len() != expected.len()
            || actual
                .call_targets
                .iter()
                .any(|target| !expected.contains(target))
        {
            return Err(
                "retained constant call roster differs from its authored invocation closure".into(),
            );
        }
        Ok(actual.dependency_values)
    }

    fn collect_internal(
        program: &TypedTrees,
        machine: &Machine,
        state: &State,
        roots: &[ExpressionHandle],
        public: bool,
        syntax: Option<&SyntaxTrees>,
        specialized_calls: bool,
        expand_callees: bool,
    ) -> Result<Custody, String> {
        let context = Context {
            machine,
            state,
            lexical_owner: machine.symbol,
            initializer: true,
        };
        collect_in_context(
            program,
            context,
            roots,
            public,
            syntax,
            specialized_calls,
            expand_callees,
        )
    }

    fn collect_in_context<'program>(
        program: &'program TypedTrees,
        context: Context<'program>,
        roots: &[ExpressionHandle],
        public: bool,
        syntax: Option<&'program SyntaxTrees>,
        specialized_calls: bool,
        expand_callees: bool,
    ) -> Result<Custody, String> {
        let mut collector = Collector {
            program,
            syntax,
            public,
            specialized_calls,
            expand_callees,
            custody: Custody::default(),
            expressions: roots.iter().rev().map(|root| (*root, context)).collect(),
            machines: Vec::new(),
            retained_call_targets: Vec::new(),
            expanded_constants: Vec::new(),
        };
        let mut visited_expressions = Vec::new();
        let mut visited_machines = Vec::new();
        loop {
            if let Some((expression, context)) = collector.expressions.pop() {
                let occurrence = (
                    expression,
                    context.machine.symbol,
                    context.state.symbol,
                    context.lexical_owner,
                    context.initializer,
                );
                if visited_expressions.contains(&occurrence) {
                    continue;
                }
                visited_expressions.push(occurrence);
                collector.expression(expression, context)?;
            } else if let Some(machine) = collector.machines.pop() {
                if visited_machines.contains(&machine) {
                    continue;
                }
                visited_machines.push(machine);
                collector.machine(machine)?;
            } else {
                if collector
                    .retained_call_targets
                    .iter()
                    .any(|target| !collector.custody.call_targets.contains(target))
                {
                    return Err(
                        "retained call selection has no authored call in its constant closure"
                            .into(),
                    );
                }
                return Ok(collector.custody);
            }
        }
    }

    struct Collector<'program> {
        program: &'program TypedTrees,
        syntax: Option<&'program SyntaxTrees>,
        public: bool,
        specialized_calls: bool,
        expand_callees: bool,
        custody: Custody,
        expressions: Vec<(ExpressionHandle, Context<'program>)>,
        machines: Vec<SymbolHandle>,
        retained_call_targets: Vec<(SourceSpan, SymbolHandle)>,
        expanded_constants: Vec<SymbolHandle>,
    }

    impl<'program> Collector<'program> {
        fn authored_target(
            &self,
            target: SymbolHandle,
            initializer: bool,
        ) -> Result<SymbolHandle, String> {
            if initializer && !self.specialized_calls {
                return Ok(target);
            }
            let mut target = target;
            let mut visited = Vec::new();
            loop {
                let symbol = self.program.symbols.get(target);
                let owner = match symbol.kind {
                    SymbolKind::Machine => target,
                    SymbolKind::State => symbol.parent,
                    _ => return Ok(target),
                };
                if visited.contains(&owner) {
                    return Err("constant call has cyclic executable source custody".into());
                }
                visited.push(owner);
                let Some(template) = self.program.executable_application_template(owner)? else {
                    return Ok(target);
                };
                let captures = self
                    .program
                    .captured_machine_applications
                    .iter()
                    .filter(|(_, capture)| capture.instance == owner)
                    .map(|(_, capture)| capture)
                    .collect::<Vec<_>>();
                if let [capture] = captures.as_slice() {
                    let entry = if symbol.kind == SymbolKind::Machine {
                        let machine = self
                            .program
                            .machine_by_symbol(owner)
                            .ok_or("constant capture lost its concrete machine")?;
                        self.program
                            .machine_states(machine)
                            .first()
                            .ok_or("constant capture lost its entry state")?
                            .symbol
                    } else {
                        target
                    };
                    let (source_machine, source_state) = self
                        .program
                        .realization_entry_template(entry, &capture.conformance_application)
                        .ok_or("constant capture lost its exact source-state correspondence")?;
                    return Ok(if symbol.kind == SymbolKind::Machine {
                        source_machine
                    } else {
                        source_state
                    });
                }
                if !captures.is_empty() {
                    return Err("constant call has ambiguous capture custody".into());
                }
                if symbol.kind == SymbolKind::Machine {
                    target = template;
                    continue;
                }
                let concrete = self
                    .program
                    .machine_by_symbol(owner)
                    .ok_or("constant call lost its concrete specialization")?;
                let source = self
                    .program
                    .machine_by_symbol(template)
                    .ok_or("constant call lost its authored template")?;
                let states = self.program.machine_states(concrete);
                let templates = self.program.machine_states(source);
                if states.len() != concrete.states.len() || templates.len() != states.len() {
                    return Err("constant call specialization lost its state correspondence".into());
                }
                let ordinal = states
                    .iter()
                    .position(|state| state.symbol == target)
                    .ok_or("constant call has no exact specialization state")?;
                target = templates[ordinal].symbol;
            }
        }

        fn selected_target_matches(
            &self,
            selected: SymbolHandle,
            target: SymbolHandle,
            initializer: bool,
        ) -> Result<bool, String> {
            let selected = call_selections::callable_entry(self.program, selected)?;
            Ok(selected == target || selected == self.authored_target(target, initializer)?)
        }

        fn call_selection_matches(
            &self,
            site: &validation::CallSite<'_>,
            context: Context<'program>,
            authored: SymbolHandle,
        ) -> Result<bool, String> {
            let target = call_selections::retained_target(self.program, site)?;
            let selected = if let validation::CallSite::Expression { call, .. } = site
                && let Some(binding) = call.authored_call_binding
            {
                if authored != binding.original {
                    return Err("constant call changed its original binding occurrence".into());
                }
                binding.selected
            } else {
                authored
            };
            if self.selected_target_matches(selected, target, context.initializer)? {
                return Ok(true);
            }
            if call_selections::static_callable_matches(
                self.program,
                context.machine.symbol,
                site,
                selected,
            )? {
                return Ok(true);
            }
            call_selections::logical_requirement_matches(
                self.program,
                context.machine.symbol,
                context.lexical_owner,
                site,
                selected,
            )
        }

        fn selected_call(
            &mut self,
            site: &validation::CallSite<'_>,
            context: Context<'program>,
            authored: SymbolHandle,
            reference: SourceSpan,
        ) -> Result<(), String> {
            let target = call_selections::retained_target(self.program, site)?;
            let selected = if let validation::CallSite::Expression { call, .. } = site
                && let Some(binding) = call.authored_call_binding
            {
                if authored != binding.original {
                    return Err("constant call changed its original binding occurrence".into());
                }
                binding.selected
            } else {
                authored
            };
            if self.selected_target_matches(selected, target, context.initializer)? {
                return self.call(
                    ResolvedAuthoredCallBinding {
                        selected: target,
                        original: authored,
                    },
                    reference,
                );
            }
            if !self.call_selection_matches(site, context, authored)? {
                return Err("constant call differs from its exact logical selection".into());
            }
            self.call(
                ResolvedAuthoredCallBinding {
                    selected: authored,
                    original: authored,
                },
                reference,
            )?;
            if self.expand_callees
                && (site.static_requirement_dispatch().is_some()
                    || call_selections::static_callable_matches(
                        self.program,
                        context.machine.symbol,
                        site,
                        selected,
                    )?)
            {
                let machine = self
                    .program
                    .machine_holding_state(target)
                    .ok_or("constant logical call lost its selected executable body")?;
                self.machines.push(machine.symbol);
            }
            Ok(())
        }

        fn selected_retained_call(
            &mut self,
            site: validation::CallSite<'_>,
            context: Context<'program>,
            reference: SourceSpan,
            occurrence: Option<typed_trees::AuthoredDeclarationSelectionOccurrenceId>,
        ) -> Result<(), String> {
            if let validation::CallSite::Statement(call) = &site
                && let Some(origin) = &call.reflected_field_origin
            {
                if occurrence.is_some() {
                    return Err(
                        "reflected callback cannot claim an authored call occurrence".into(),
                    );
                }
                self.reflected_query_dependency(origin.query, context)?;
                let target = call_selections::retained_target(self.program, &site)?;
                if !self.selected_target_matches(origin.callback.symbol, target, false)? {
                    return Err("reflected callback changed its selected source operation".into());
                }
                if self.expand_callees {
                    let machine = self
                        .program
                        .machine_holding_state(target)
                        .ok_or("reflected callback lost its selected executable body")?;
                    self.machines.push(machine.symbol);
                }
                return Ok(());
            }
            if occurrence.is_none()
                && self.reflection_body_origin(context.machine).is_some()
                && let validation::CallSite::TransitionNamed { .. } = &site
            {
                let target = call_selections::retained_target(self.program, &site)?;
                if self
                    .program
                    .machine_holding_state(target)
                    .is_some_and(|owner| owner.symbol == context.machine.symbol)
                {
                    return Ok(());
                }
            }
            let selection = occurrence
                .and_then(|occurrence| {
                    self.program
                        .authored_declaration_selections()
                        .get(occurrence)
                })
                .ok_or("constant call lost its authored selection occurrence")?;
            let authored = selection
                .target()
                .as_resolved()
                .ok_or("constant call has no resolved authored target")?
                .selected_symbol();
            if selection.kind() != Kind::Call
                || selection.source_span() != reference
                || !self.call_selection_matches(&site, context, authored)?
            {
                let target = call_selections::retained_target(self.program, &site)?;
                return Err(format!(
                    "constant call differs from its exact authored selection: owner {}, state {:?}, occurrence {:?}, kind {:?}, retained source {:?}, call source {:?}, authored {:?}, target {:?}, source target {:?}",
                    context.machine.name.as_str(),
                    context.state.symbol,
                    occurrence,
                    selection.kind(),
                    selection.source_span(),
                    reference,
                    authored,
                    target,
                    self.authored_target(target, context.initializer)?,
                ));
            }
            let occurrence = selection.occurrence_id();
            self.selected_call(&site, context, authored, reference)?;
            push_unique(&mut self.custody.call_occurrences, occurrence);
            Ok(())
        }

        fn reflection_body_origin(
            &self,
            machine: &Machine,
        ) -> Option<(typed_trees::statement::StatementHandle, SymbolHandle)> {
            let program = self.program;
            let source_state = program.symbols.get(machine.symbol).generated_from;
            if !source_state.is_valid()
                || program.symbols.get(source_state).kind != SymbolKind::State
            {
                return None;
            }
            let entry = program.machine_states(machine).first()?;
            let mut markers = program
                .statement_table
                .statements(entry.statement_nodes)
                .iter()
                .filter_map(|statement| {
                    statement
                        .as_local_data()
                        .and_then(|local| local.reflected_query_origin)
                });
            let query = markers.next()?;
            if markers.next().is_some() || !program.statement_table.statement_is_valid(query) {
                return None;
            }
            let StatementNode::Call(call) = program.statement_table.statement(query) else {
                return None;
            };
            let symbol = program.symbols.get(call.target_symbol);
            let declaration = if symbol.kind == SymbolKind::State {
                symbol.parent
            } else {
                call.target_symbol
            };
            program
                .core_vocabulary(declaration)
                .is_some_and(|vocabulary| vocabulary.is_reflection_field_visitor())
                .then_some((query, source_state))
        }

        fn reflected_query_dependency(
            &mut self,
            handle: typed_trees::statement::StatementHandle,
            context: Context<'program>,
        ) -> Result<(), String> {
            let program = self.program;
            let context =
                if let Some((query, source_state)) = self.reflection_body_origin(context.machine) {
                    if query != handle {
                        return Err("active reflection helper changed its original query".into());
                    }
                    let machine = program
                        .machine_holding_state(source_state)
                        .ok_or("active reflection helper lost its author")?;
                    let state = program
                        .machine_states(machine)
                        .iter()
                        .find(|state| state.symbol == source_state)
                        .ok_or("active reflection helper lost its authored state")?;
                    Context {
                        machine,
                        state,
                        lexical_owner: machine.symbol,
                        initializer: context.initializer,
                    }
                } else {
                    context
                };
            if !program.statement_table.statement_is_valid(handle) {
                return Err("reflected callback retained a stale query".into());
            }
            let StatementNode::Call(query) = program.statement_table.statement(handle) else {
                return Err("reflected callback lost its original query".into());
            };
            if query.reflected_field_origin.is_some() {
                return Err("reflected callback has cyclic query custody".into());
            }
            let symbol = program.symbols.get(query.target_symbol);
            let declaration = if symbol.kind == SymbolKind::State {
                symbol.parent
            } else {
                query.target_symbol
            };
            if !program
                .core_vocabulary(declaration)
                .is_some_and(|vocabulary| vocabulary.is_reflection_field_visitor())
            {
                return Err(
                    "reflected callback does not retain its selected reflection declaration".into(),
                );
            }
            self.selected_retained_call(
                validation::CallSite::Statement(query),
                context,
                query.source_span,
                query.authored_call_selection,
            )?;
            self.static_arguments(&query.machine_arguments);
            Ok(())
        }

        fn reflected_expression_call(
            &mut self,
            expression: ExpressionHandle,
            call: &typed_trees::expression::TableCallExpression,
            context: Context<'program>,
        ) -> Result<bool, String> {
            let Some(machine) = self.program.machine_holding_state(call.target_symbol) else {
                return Ok(false);
            };
            let Some((query, _)) = self.reflection_body_origin(machine) else {
                return Ok(false);
            };
            let StatementNode::Call(original) = self.program.statement_table.statement(query)
            else {
                return Err("reflected expression lost its original query".into());
            };
            let occurrence = original
                .authored_call_selection
                .ok_or("reflected expression query lost its authored call occurrence")?;
            if call.authored_call_selection != Some(occurrence)
                || !self
                    .program
                    .expression_table
                    .authored_selection_occurrences(expression)
                    .any(|retained| retained == occurrence)
            {
                return Err("reflected expression changed its original query selection".into());
            }
            self.reflected_query_dependency(query, context)?;
            if self.expand_callees {
                self.machines.push(machine.symbol);
            }
            Ok(true)
        }

        fn call(
            &mut self,
            binding: ResolvedAuthoredCallBinding,
            reference: SourceSpan,
        ) -> Result<(), String> {
            let executable = binding.selected;
            let target = binding.original;
            let symbol = self.program.symbols.get(target);
            let bound_requirement = self
                .program
                .evidence_requirement_declaration(target)?
                .is_some();
            let trait_requirement = symbol.kind == SymbolKind::State
                && symbol.parent.is_valid()
                && self.program.symbols.get(symbol.parent).kind == SymbolKind::Trait;
            let logical = matches!(
                symbol.kind,
                SymbolKind::Operator | SymbolKind::MachineParameter
            ) || bound_requirement
                || trait_requirement;
            match symbol.kind {
                SymbolKind::Machine => (),
                SymbolKind::State
                    if self.program.symbols.get(symbol.parent).kind == SymbolKind::Machine => {}
                _ if logical => (),
                _ => {
                    return Err(
                        "constant call has no exact ordinary machine or state selection".into(),
                    );
                }
            };
            let declaration = call_selections::call_custody_source(self.program, target)?;
            let selection = (reference, declaration);
            push_unique(&mut self.custody.calls, selection);
            push_unique(&mut self.custody.call_targets, (reference, target));
            let executable_symbol = self.program.symbols.get(executable);
            let executable_machine = match executable_symbol.kind {
                SymbolKind::Machine => Some(executable),
                SymbolKind::State
                    if self.program.symbols.get(executable_symbol.parent).kind
                        == SymbolKind::Machine =>
                {
                    Some(executable_symbol.parent)
                }
                _ if logical => None,
                _ => return Err("constant call lost its selected executable declaration".into()),
            };
            if self.expand_callees
                && let Some(machine) = executable_machine
            {
                self.machines.push(machine);
            }
            Ok(())
        }

        fn constant(
            &mut self,
            symbol: SymbolHandle,
            reference: SourceSpan,
            expression: ExpressionHandle,
            context: Context<'program>,
        ) -> Result<(), String> {
            let mut declarations = self
                .program
                .const_declarations()
                .iter()
                .filter(|declaration| declaration.symbol == symbol);
            let declaration = declarations
                .next()
                .ok_or("constant selection lost its declaration")?;
            if declarations.next().is_some() {
                return Err("constant selection has ambiguous declaration custody".into());
            }
            let source = self
                .program
                .symbols
                .symbol_source_span(symbol)
                .ok_or("constant selection lost its declaration source")?;
            if !declaration.is_public
                && ((self.public
                    && context.initializer
                    && !self.syntax.is_some_and(|syntax| {
                        syntax.constant_initializer_owns_selection(reference)
                    }))
                    || !self.program.symbols.same_source_package(reference, source))
            {
                return Err(
                    "private constant declaration cannot be selected by this initializer owner"
                        .into(),
                );
            }
            if declaration.evaluated_value.is_none()
                && !self.specialized_calls
                && self.syntax.is_some()
            {
                let pending = (reference, source);
                push_unique(&mut self.custody.pending_constants, pending);
                return Ok(());
            }
            let origin = ConstArgumentOrigin {
                reference,
                declaration: source,
                initializer: declaration.initializer_source_span,
                evaluated_value: declaration
                    .evaluated_value
                    .clone()
                    .ok_or("selected constant has no ready canonical value")?,
            };
            push_unique(&mut self.custody.origins, origin);
            if self.syntax.is_none() {
                let table = &self.program.expression_table;
                let materialized = declaration.materialized_initializer;
                if !table.expression_is_valid(materialized)
                    || table.source_span(materialized) != declaration.initializer_source_span
                {
                    return Err("selected constant lost its exact materialized initializer".into());
                }
                let subject = DependencyValue {
                    declaration: symbol,
                    expression: materialized,
                };
                push_unique(&mut self.custody.dependency_values, subject);
                if table.expression_is_valid(expression)
                    && table.source_span(expression) == reference
                {
                    let subject = DependencyValue {
                        declaration: symbol,
                        expression,
                    };
                    push_unique(&mut self.custody.dependency_values, subject);
                }
            }
            if self.expanded_constants.contains(&symbol) {
                return Ok(());
            }
            self.expanded_constants.push(symbol);
            let context = Context {
                lexical_owner: declaration.symbol,
                ..context
            };
            self.type_dependencies(types::Root::Type(declaration.declared_type), context)?;
            let context = Context {
                initializer: true,
                ..context
            };
            if self.specialized_calls && declaration.authored_initializer.is_valid() {
                let materialized = declaration.materialized_initializer;
                if !expression_table(self.program).expression_is_valid(materialized)
                    || self.program.expression_table.source_span(materialized)
                        != declaration.initializer_source_span
                {
                    return Err("constant dependency lost its owned materialized custody".into());
                }
                let calls = expression_table(self.program)
                    .authored_selection_occurrences(materialized)
                    .map(|occurrence| {
                        let selection = self
                            .program
                            .authored_declaration_selections()
                            .get(occurrence)
                            .ok_or("constant dependency lost its retained selection")?;
                        if selection.kind() != Kind::Call {
                            return Ok(None);
                        }
                        let target = selection
                            .target()
                            .as_resolved()
                            .ok_or("constant dependency retained an unresolved call")?
                            .selected_symbol();
                        Ok(Some((selection.source_span(), target, occurrence)))
                    })
                    .collect::<Result<Vec<_>, String>>()?;
                for (reference, target, occurrence) in calls.into_iter().flatten() {
                    self.call(
                        ResolvedAuthoredCallBinding {
                            selected: target,
                            original: self.authored_target(target, false)?,
                        },
                        reference,
                    )?;
                    push_unique(&mut self.custody.call_occurrences, occurrence);
                }
                self.expressions.push((materialized, context));
            } else if declaration.authored_initializer.is_valid() {
                self.expressions
                    .push((declaration.authored_initializer, context));
            }
            Ok(())
        }

        fn expression(
            &mut self,
            expression: ExpressionHandle,
            context: Context<'program>,
        ) -> Result<(), String> {
            let program = self.program;
            let table = &program.expression_table;
            if !table.expression_is_valid(expression) {
                return Err("constant call closure contains a stale expression".into());
            }
            self.type_dependencies(types::Root::Expression(expression), context)?;
            for occurrence in table.authored_selection_occurrences(expression) {
                if !self.specialized_calls {
                    let reference = table.source_span(expression);
                    let inherited =
                        table
                            .authored_selection_occurrences(expression)
                            .any(|direct| {
                                program
                                    .authored_declaration_selections()
                                    .get(direct)
                                    .is_some_and(|selection| {
                                        if selection.source_span() != reference {
                                            return false;
                                        }
                                        let selected_target = selection.target();
                                        let Some(selected) = selected_target.as_resolved() else {
                                            return false;
                                        };
                                        let Some(owner) =
                                            program.const_declarations().iter().find(|owner| {
                                                owner.symbol == selected.selected_symbol()
                                            })
                                        else {
                                            return false;
                                        };
                                        owner.materialized_initializer.is_valid()
                                            && table
                                                .authored_selection_occurrences(
                                                    owner.materialized_initializer,
                                                )
                                                .any(|owned| owned == occurrence)
                                    })
                            });
                    if inherited {
                        continue;
                    }
                }
                let selection = program
                    .authored_declaration_selections()
                    .get(occurrence)
                    .ok_or("constant call closure lost an authored selection")?;
                if selection.kind() == Kind::Call {
                    let selection_target = selection.target();
                    let Some(selected) = selection_target.as_resolved() else {
                        return Err(format!(
                            "constant closure retained an unresolved call selection: occurrence {:?}, source {:?}, owner {}, expression {:?}, call {:?}",
                            occurrence,
                            selection.source_span(),
                            context.machine.name.as_str(),
                            table.source_span(expression),
                            table.call_node(expression),
                        ));
                    };
                    let target = (selection.source_span(), selected.selected_symbol());
                    push_unique(&mut self.retained_call_targets, target);
                } else if selection.kind() == Kind::Operator {
                    if !context.initializer
                        && !self.syntax.is_some_and(|syntax| {
                            syntax.constant_initializer_owns_selection(selection.source_span())
                        })
                    {
                        continue;
                    }
                    let builtin = super::has_builtin_operator_selection(
                        program,
                        context.machine,
                        context.state,
                        expression,
                        selection.target(),
                    );
                    if !builtin {
                        return Err(
                            "evaluated initializer operator has no checked builtin meaning".into(),
                        );
                    }
                    if !self.custody.operators.contains(&selection.source_span()) {
                        self.custody.operators.push(selection.source_span());
                    }
                } else if let Target::Resolved(selected) = selection.target()
                    && program.symbols.get(selected.selected_symbol()).kind == SymbolKind::Const
                {
                    let source = table.source_span(expression);
                    let terminal = selection.source_span();
                    let leaf = program
                        .symbols
                        .name(selected.selected_symbol())
                        .rsplit("::")
                        .next()
                        .unwrap_or_default();
                    let direct_segment = terminal.source_id == source.source_id
                        && terminal.span.end == source.span.end
                        && source.span.end.checked_sub(leaf.len()) == Some(terminal.span.start)
                        && terminal.span.start >= source.span.start
                        && table
                            .authored_selection_occurrences(expression)
                            .any(|occurrence| {
                                program
                                    .authored_declaration_selections()
                                    .get(occurrence)
                                    .is_some_and(|other| {
                                        other.source_span() == source
                                            && matches!(other.target(), Target::Resolved(other)
                                    if other.selected_symbol() == selected.selected_symbol())
                                    })
                            });
                    let reference = if direct_segment { source } else { terminal };
                    self.constant(selected.selected_symbol(), reference, expression, context)?;
                }
            }
            let children = expression_children(program, expression)?;
            match table.expression(expression) {
                ExpressionNode::Call(call)
                    if self.reflected_expression_call(expression, call, context)? => {}
                ExpressionNode::Call(call) => {
                    self.static_arguments(&call.machine_arguments);
                    if call.authored_machine_application.is_valid() {
                        self.static_arguments(table.static_machine_application_arguments(
                            call.authored_machine_application,
                        ));
                        self.static_arguments(table.static_machine_application_selected_machines(
                            call.authored_machine_application,
                        ));
                    }
                    let site = validation::CallSite::Expression { expression, call };
                    let mut selections = Vec::new();
                    for occurrence in call.authored_call_selection.iter().copied() {
                        if !table
                            .authored_selection_occurrences(expression)
                            .any(|retained| retained == occurrence)
                        {
                            return Err(
                                "constant call lost its source-owned Call occurrence attachment"
                                    .into(),
                            );
                        }
                        let Some(selection) =
                            program.authored_declaration_selections().get(occurrence)
                        else {
                            return Err("constant call lost an attached authored selection".into());
                        };
                        if selection.kind() != Kind::Call {
                            return Err(
                                "constant call occurrence has a different selection role".into()
                            );
                        }
                        let Some(target) = selection
                            .target()
                            .as_resolved()
                            .map(|target| target.selected_symbol())
                        else {
                            continue;
                        };
                        let matches = self.call_selection_matches(&site, context, target)?;
                        let owned_entry =
                            call_selections::owned_realization_entry(program, call, target);
                        let realized = owned_entry == Some(call.target_symbol)
                            || call_selections::requirement_realized_by(
                                program,
                                target,
                                call.target_symbol,
                            );
                        if !realized && !matches {
                            continue;
                        }
                        let entry = if realized {
                            Some(call.target_symbol)
                        } else {
                            owned_entry
                        };
                        let selected = (target, selection.source_span(), entry, occurrence);
                        push_unique(&mut selections, selected);
                    }
                    let [(authored, reference, entry, occurrence)] = selections.as_slice() else {
                        return Err(format!(
                            "{}: owner {}, call {:?}, retained {:?}",
                            if selections.is_empty() {
                                "constant call lost its exact authored target selection"
                            } else {
                                "constant call has ambiguous authored target selections"
                            },
                            context.machine.name.as_str(),
                            call,
                            table
                                .authored_selection_occurrences(expression)
                                .map(|occurrence| {
                                    (
                                        occurrence,
                                        program
                                            .authored_declaration_selections()
                                            .get(occurrence)
                                            .map(|selection| {
                                                (
                                                    selection.kind(),
                                                    selection.source_span(),
                                                    selection.target(),
                                                )
                                            }),
                                    )
                                })
                                .collect::<Vec<_>>(),
                        ));
                    };
                    if let Some(entry) = *entry {
                        self.call(
                            ResolvedAuthoredCallBinding {
                                selected: entry,
                                original: *authored,
                            },
                            *reference,
                        )?;
                    } else if let Some(message) =
                        call_selections::unsupplied_requirement(program, call, *authored)
                    {
                        return Err(message);
                    } else {
                        self.selected_call(&site, context, *authored, *reference)?;
                    }
                    push_unique(&mut self.custody.call_occurrences, *occurrence);
                }
                ExpressionNode::Name(name)
                    if program.symbols.get(name.symbol).kind == SymbolKind::Const =>
                {
                    self.constant(
                        name.symbol,
                        table.source_span(expression),
                        ExpressionHandle::invalid(),
                        context,
                    )?;
                }
                _ => {}
            }
            self.expressions
                .extend(children.into_iter().rev().map(|child| (child, context)));
            Ok(())
        }

        fn machine(&mut self, symbol: SymbolHandle) -> Result<(), String> {
            let program = self.program;
            let mut machines = program
                .machines()
                .iter()
                .filter(|machine| machine.symbol == symbol);
            let machine = machines
                .next()
                .ok_or("constant call closure lost its selected machine")?;
            if machines.next().is_some() {
                return Err("constant call closure has ambiguous machine identity".into());
            }
            let states = program.machine_states(machine);
            if states.len() != machine.states.len() {
                return Err("constant call closure has stale states".into());
            }
            let entry = states
                .first()
                .ok_or("constant call closure has no entry state")?;
            self.type_dependencies(
                types::Root::Machine(machine),
                Context {
                    machine,
                    state: entry,
                    lexical_owner: machine.symbol,
                    initializer: false,
                },
            )?;
            self.type_dependencies(
                types::Root::Contracts(machine.contracts),
                Context {
                    machine,
                    state: entry,
                    lexical_owner: machine.symbol,
                    initializer: false,
                },
            )?;
            let owned = program.machine_owned_data(machine);
            if owned.len() != machine.owned_data.len() {
                return Err("constant call closure has stale owned initializers".into());
            }
            for owned in owned {
                if owned.initial_value.is_valid() {
                    self.expressions.push((
                        owned.initial_value,
                        Context {
                            machine,
                            state: entry,
                            lexical_owner: machine.symbol,
                            initializer: false,
                        },
                    ));
                }
            }
            let table = &program.statement_table;
            for state in states {
                let context = Context {
                    machine,
                    state,
                    lexical_owner: machine.symbol,
                    initializer: false,
                };
                self.type_dependencies(types::Root::Contracts(state.contracts), context)?;
                let body = table.statements(state.statement_nodes);
                if body.len() != state.statement_nodes.len() {
                    return Err("constant call closure has stale statements".into());
                }
                for statement in body {
                    let mut expressions = Vec::new();
                    match statement {
                        StatementNode::Assembly(_) => {}
                        StatementNode::AssemblyFact(fact) => expressions
                            .extend(program.tables.proof_fact_expression_roots(fact.fact)),
                        StatementNode::Expression(expression) => expressions.push(*expression),
                        StatementNode::LocalData(local) => {
                            if let Some(query) = local.reflected_query_origin {
                                self.reflected_query_dependency(query, context)?;
                            }
                            if local.initial_value.is_valid() {
                                expressions.push(local.initial_value);
                            }
                        }
                        StatementNode::Assignment(assignment) => {
                            expressions.extend([assignment.target, assignment.value])
                        }
                        StatementNode::Call(call) => {
                            self.static_arguments(&call.machine_arguments);
                            if call.authored_machine_application.is_valid() {
                                self.static_arguments(
                                    expression_table(program).static_machine_application_arguments(
                                        call.authored_machine_application,
                                    ),
                                );
                                self.static_arguments(
                                    expression_table(program)
                                        .static_machine_application_selected_machines(
                                            call.authored_machine_application,
                                        ),
                                );
                            }
                            self.selected_retained_call(
                                validation::CallSite::Statement(call),
                                context,
                                call.source_span,
                                call.authored_call_selection,
                            )?;
                            let arguments = table.expression_handles(call.arguments);
                            if arguments.len() != call.arguments.len() {
                                return Err("constant statement call has stale arguments".into());
                            }
                            expressions.extend(arguments.iter().copied());
                        }
                        StatementNode::Transition(transition) => {
                            if let TransitionGuardNode::When(guard) = transition.guard {
                                expressions.push(guard);
                            }
                            for target in transition.targets() {
                                if !table.transition_target_is_valid(target) {
                                    return Err("constant call closure has stale transition".into());
                                }
                                match table.transition_target(target) {
                                    TransitionTargetNode::Named {
                                        path,
                                        static_machine_parameter,
                                        arguments,
                                        evidence_arguments,
                                        source_span,
                                        authored_call_selection,
                                        ..
                                    } => {
                                        self.selected_retained_call(
                                            validation::CallSite::TransitionNamed {
                                                path,
                                                static_machine_parameter: *static_machine_parameter,
                                                arguments: *arguments,
                                                evidence_arguments,
                                                source_span: *source_span,
                                                authored_call_selection: *authored_call_selection,
                                            },
                                            context,
                                            *source_span,
                                            *authored_call_selection,
                                        )?;
                                        let values = table.expression_handles(*arguments);
                                        if values.len() != arguments.len() {
                                            return Err(
                                                "constant transition call has stale arguments"
                                                    .into(),
                                            );
                                        }
                                        expressions.extend(values.iter().copied());
                                    }
                                    TransitionTargetNode::Value(value) => expressions.push(*value),
                                    TransitionTargetNode::Terminal => {}
                                }
                            }
                        }
                    }
                    self.expressions.extend(
                        expressions
                            .into_iter()
                            .rev()
                            .map(|expression| (expression, context)),
                    );
                }
            }
            Ok(())
        }

        fn static_arguments(
            &mut self,
            arguments: &[typed_trees::expression::StaticMachineArgument],
        ) {
            if !self.expand_callees {
                return;
            }
            for argument in arguments {
                let selected = self.program.symbols.get(argument.symbol);
                match selected.kind {
                    SymbolKind::Machine => self.machines.push(argument.symbol),
                    SymbolKind::State
                        if self.program.symbols.get(selected.parent).kind
                            == SymbolKind::Machine =>
                    {
                        self.machines.push(selected.parent);
                    }
                    _ => {}
                }
                if let Some(application) = &argument.application {
                    self.static_arguments(&application.arguments);
                }
            }
        }
    }
}
pub(crate) use call_custody::{DependencyValue, retain_current_initializer_calls};
pub(crate) mod lexical_selection {
    use crate::unique::push_unique;
    use language_semantics::declaration_selection::AuthoredDeclarationSelectionTarget;
    use source::SourceSpan;
    use symbols::SymbolKind;
    use syntax_trees::SyntaxTrees;
    use syntax_trees::expression::{ExpressionHandle, ExpressionNode};
    use syntax_trees::types::ConstArgumentOrigin;
    use syntax_trees_to_symbol_resolved_trees::pre_resolution::ConstArgumentSelection;

    fn enqueue_expression_children(
        syntax: &SyntaxTrees,
        expression: ExpressionHandle,
        pending: &mut Vec<ExpressionHandle>,
    ) {
        match syntax.expressions.expression(expression) {
            ExpressionNode::Match(dispatch) => {
                for arm in syntax.expressions.match_arms(dispatch.arms).iter().rev() {
                    pending.push(arm.value);
                    if let syntax_trees::expression::MatchPattern::Value(pattern) = arm.pattern {
                        pending.push(pattern);
                    }
                }
                pending.push(dispatch.subject);
            }
            ExpressionNode::Unary(unary) => pending.push(unary.operand),
            ExpressionNode::Cast(cast) => pending.push(cast.value),
            ExpressionNode::Member(member) => pending.push(member.receiver),
            ExpressionNode::Borrow(borrow) => pending.push(borrow.target),
            ExpressionNode::Indexed(indexed) => {
                pending.push(indexed.index);
                pending.push(indexed.collection);
            }
            ExpressionNode::Binary(binary) => {
                pending.push(binary.right);
                pending.push(binary.left);
            }
            _ => {}
        }
    }

    pub(crate) fn needs_resolution(syntax: &SyntaxTrees, root: ExpressionHandle) -> bool {
        let mut pending = vec![root];
        let mut visited = Vec::new();
        while let Some(expression) = crate::worklist::unvisited(&mut pending, &mut visited) {
            if matches!(
                syntax.expressions.expression(expression),
                ExpressionNode::Name(_) | ExpressionNode::Call(_)
            ) {
                return true;
            }
            enqueue_expression_children(syntax, expression, &mut pending);
        }
        false
    }

    pub(crate) fn retain(
        syntax: &SyntaxTrees,
        resolved: &ConstArgumentSelection,
        root: ExpressionHandle,
    ) -> Result<Vec<ConstArgumentOrigin>, String> {
        let mut pending = vec![root];
        let mut visited = Vec::new();
        let mut origins = Vec::new();
        while let Some(expression) = crate::worklist::unvisited(&mut pending, &mut visited) {
            match syntax.expressions.expression(expression) {
                ExpressionNode::Name(path) => {
                    let members = syntax.expressions.identifier_path_members(*path);
                    let (Some(first), Some(last)) = (members.first(), members.last()) else {
                        return Err("machine index operand lost its authored name path".to_owned());
                    };
                    let reference = SourceSpan::new(
                        first.source_span().source_id,
                        source::Span::new(
                            first.source_span().span.start,
                            last.source_span().span.end,
                        ),
                    );
                    let origin = selected_constant(resolved, reference)?;
                    if let Some(normalization) = syntax.root_items().find_map(|item| match item {
                        syntax_trees::item::Item::Const(definition)
                            if definition.name.source_span() == origin.declaration =>
                        {
                            definition.normalization.as_ref()
                        }
                        _ => None,
                    }) {
                        for dependency in &normalization.selections {
                            if !origins.contains(dependency) {
                                origins.push(dependency.clone());
                            }
                        }
                    }
                    push_unique(&mut origins, origin);
                }
                ExpressionNode::Call(call) => {
                    push_selected_call_children(&mut pending, syntax, resolved, call)?;
                }
                _ => {}
            }
            enqueue_expression_children(syntax, expression, &mut pending);
        }
        Ok(origins)
    }

    pub(crate) fn retain_calls(
        syntax: &SyntaxTrees,
        resolved: &ConstArgumentSelection,
        root: ExpressionHandle,
    ) -> Result<Vec<(SourceSpan, SourceSpan)>, String> {
        let mut pending = vec![root];
        let mut visited = Vec::new();
        let mut calls = Vec::new();
        while let Some(expression) = crate::worklist::unvisited(&mut pending, &mut visited) {
            if let ExpressionNode::Call(call) = syntax.expressions.expression(expression) {
                let (selected, target) = selected_call(syntax, resolved, call)?;
                let reference = selected.target.source_span();
                let symbol = resolved.program().symbols.get(target);
                if symbol.kind != SymbolKind::Machine
                    && !(symbol.kind == SymbolKind::State
                        && resolved.program().symbols.get(symbol.parent).kind
                            == SymbolKind::Machine)
                {
                    return Err(
                        "machine index call must select an ordinary machine or state".into(),
                    );
                }
                let declaration = resolved
                    .program()
                    .symbols
                    .symbol_source_span(target)
                    .or_else(|| {
                        if symbol.kind != SymbolKind::State
                            || resolved.program().symbols.get(symbol.parent).kind
                                != SymbolKind::Machine
                        {
                            return None;
                        }
                        let mut owners = resolved
                            .program()
                            .machines
                            .iter()
                            .filter(|owner| owner.symbol == symbol.parent);
                        let owner = owners.next()?;
                        if owners.next().is_some() {
                            return None;
                        }
                        let storage = &resolved.program().tables.declarations;
                        let states = storage.machine_state_handles.span_or_empty(owner.states);
                        if states.len() != owner.states.len()
                            || storage.machine_states.get(*states.first()?).symbol != target
                        {
                            return None;
                        }
                        resolved.program().symbols.symbol_source_span(symbol.parent)
                    })
                    .ok_or("machine index call lost its exact selected declaration source")?;
                let selection = (reference, declaration);
                push_unique(&mut calls, selection);
                push_selected_call_children(&mut pending, syntax, resolved, call)?;
            }
            enqueue_expression_children(syntax, expression, &mut pending);
        }
        Ok(calls)
    }

    fn selected_call<'resolved>(
        syntax: &SyntaxTrees,
        resolved: &'resolved ConstArgumentSelection,
        authored: &syntax_trees::expression::TableCallExpression,
    ) -> Result<
        (
            &'resolved symbol_resolved_trees::expression::TableCallExpression,
            symbols::SymbolHandle,
        ),
        String,
    > {
        let reference = authored.target.source_span();
        let qualified_reference = match syntax.expressions.expression(authored.receiver) {
            ExpressionNode::Name(path) => syntax
                .expressions
                .identifier_path_members(*path)
                .first()
                .filter(|first| first.source_span().source_id == reference.source_id)
                .map(|first| {
                    SourceSpan::new(
                        reference.source_id,
                        source::Span::new(first.source_span().span.start, reference.span.end),
                    )
                }),
            _ => None,
        };
        let mut selected_call = None;
        for (expression, node) in resolved.expressions() {
            let symbol_resolved_trees::expression::ExpressionNode::Call(call) = node else {
                continue;
            };
            let selected_reference = call.target.source_span();
            if selected_reference != reference
                && !(qualified_reference == Some(selected_reference) && !call.receiver.is_valid())
            {
                continue;
            }
            let original = resolved
                .original_call_target(expression)
                .ok_or("machine index call lost its original lexical selection")?;
            if selected_call.is_some_and(
                |(previous, target): (
                    &symbol_resolved_trees::expression::TableCallExpression,
                    symbols::SymbolHandle,
                )| {
                    target != original
                        || has_namespace_receiver(resolved, previous)
                            != has_namespace_receiver(resolved, call)
                },
            ) {
                return Err("machine index call has conflicting original lexical owners".into());
            }
            selected_call = Some((call, original));
        }
        selected_call
            .ok_or_else(|| "machine index call lost its original lexical occurrence".into())
    }

    fn selected_call_has_namespace_receiver(
        syntax: &SyntaxTrees,
        resolved: &ConstArgumentSelection,
        authored: &syntax_trees::expression::TableCallExpression,
    ) -> Result<bool, String> {
        let (call, _) = selected_call(syntax, resolved, authored)?;
        Ok((authored.receiver.is_valid() && !call.receiver.is_valid())
            || has_namespace_receiver(resolved, call))
    }

    fn has_namespace_receiver(
        resolved: &ConstArgumentSelection,
        call: &symbol_resolved_trees::expression::TableCallExpression,
    ) -> bool {
        if !call.receiver.is_valid() {
            return false;
        }
        let symbol_resolved_trees::expression::ExpressionNode::Name(path) = resolved
            .program()
            .tables
            .bodies
            .expressions
            .expression(call.receiver)
        else {
            return false;
        };
        matches!(
            resolved.program().symbols.get(path.symbol).kind,
            SymbolKind::Module
                | SymbolKind::Data
                | SymbolKind::Domain
                | SymbolKind::BuiltinType
                | SymbolKind::Machine
                | SymbolKind::Trait
        )
    }

    fn selected_constant(
        resolved: &ConstArgumentSelection,
        reference: SourceSpan,
    ) -> Result<ConstArgumentOrigin, String> {
        let table = &resolved.program().tables.bodies.expressions;
        let mut origin = None;
        for (expression, node) in resolved.expressions() {
            let mut matches_reference = false;
            if let symbol_resolved_trees::expression::ExpressionNode::Name(path) = node {
                let members = table.name_path_members(path.members);
                if let (Some(first), Some(last)) = (members.first(), members.last()) {
                    matches_reference = first.source_span().source_id == reference.source_id
                        && first.source_span().span.start == reference.span.start
                        && last.source_span().span.end == reference.span.end;
                }
            }
            let mut selected_origin = None;
            for occurrence in table.authored_selection_occurrences(expression) {
                let selection = resolved
                    .program()
                    .authored_declaration_selections()
                    .get(occurrence)
                    .ok_or("machine index lost its original declaration selection")?;
                if selection.source_span() != reference {
                    continue;
                }
                matches_reference = true;
                let AuthoredDeclarationSelectionTarget::Resolved(selected) = selection.target()
                else {
                    continue;
                };
                let Some(declaration) = resolved
                    .program()
                    .const_declaration(selected.selected_symbol())
                else {
                    continue;
                };
                let candidate = ConstArgumentOrigin {
                    reference,
                    declaration: resolved
                        .program()
                        .symbols
                        .symbol_source_span(declaration.symbol)
                        .ok_or("machine index constant lost its declaration source")?,
                    initializer: declaration.initializer_source_span,
                    evaluated_value: declaration
                        .evaluated_value
                        .clone()
                        .ok_or("machine index constant has no canonical value")?,
                };
                if selected_origin
                    .as_ref()
                    .is_some_and(|previous| previous != &candidate)
                {
                    return Err(
                        "machine index operand has conflicting declaration custody".to_owned()
                    );
                }
                selected_origin = Some(candidate);
            }
            if !matches_reference {
                continue;
            }
            let selected_origin = selected_origin.ok_or(
            "machine index operand must select a constant in its original lexical scope; runtime locals, parameters and open binders cannot be evaluated",
        )?;
            if origin
                .as_ref()
                .is_some_and(|previous| previous != &selected_origin)
            {
                return Err("machine index operand has conflicting lexical owners".to_owned());
            }
            origin = Some(selected_origin);
        }
        origin
            .ok_or_else(|| "machine index operand lost its original lexical occurrence".to_owned())
    }

    fn push_selected_call_children(
        pending: &mut Vec<ExpressionHandle>,
        syntax: &SyntaxTrees,
        resolved: &ConstArgumentSelection,
        call: &syntax_trees::expression::TableCallExpression,
    ) -> Result<(), String> {
        pending.extend(
            syntax
                .expressions
                .expression_handles(call.arguments)
                .iter()
                .rev()
                .copied(),
        );
        if call.receiver.is_valid()
            && !selected_call_has_namespace_receiver(syntax, resolved, call)?
        {
            pending.push(call.receiver);
        }
        Ok(())
    }
}
pub(crate) mod value {
    use crate::build_time_evaluation::{expression_table, type_reference_table};
    use crate::unique::extend_unique;
    use diagnostics::Diagnostic;
    use language_semantics::const_value::{
        CanonicalConstIdentity, CanonicalConstValue, DecodedCanonicalConstValue,
    };
    use numerics::{
        arithmetic::ArithmeticDomain,
        float_semantics::{FloatFormat as SemanticFloatFormat, FloatMeaning, FloatSemantics},
        literals::{FloatFormat, IntegerLiteral, LandedIntegerType},
    };
    use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue};
    use typed_trees::types::TypeReferenceHandle;
    use typed_trees::{
        TypedTrees,
        expression::{BinaryOperator, ExpressionHandle, ExpressionNode, UnaryOperator},
        machine::Machine,
        state::State,
        types::PrimitiveType,
    };
    use validation::{
        evaluate_anonymous_numeric_comparison, evaluate_anonymous_numeric_expression,
    };

    mod context {
        use super::{Machine, Shape, State, primitive};
        use crate::build_time_evaluation::type_reference_table;
        use typed_trees::closed_numeric::has_builtin_anonymous_operands;
        use typed_trees::{
            TypedTrees,
            expression::{BinaryOperator, ExpressionHandle},
            types::{PrimitiveType, TypeReferenceHandle},
        };
        use validation::has_builtin_binary_expression_meaning;

        #[derive(Clone, Copy)]
        pub(super) enum EvaluationContext<'program> {
            Machine(&'program Machine, &'program State),
            Closed,
        }

        impl EvaluationContext<'_> {
            pub(super) fn is_closed(self) -> bool {
                matches!(self, Self::Closed)
            }

            pub(super) fn arithmetic_result(
                self,
                program: &TypedTrees,
                shape: Shape,
            ) -> Result<Shape, String> {
                if !self.is_closed() {
                    return Ok(shape);
                }
                let Shape::Integer(carrier, _, policy) = shape else {
                    return Ok(shape);
                };
                let reference = reference(program, shape)?
                    .and_then(|reference| {
                        validation::arithmetic_result_type_reference(program, reference)
                    })
                    .ok_or("closed arithmetic result has unresolved type qualifications")?;
                Ok(Shape::Integer(carrier, reference, policy))
            }

            pub(super) fn joined_result(
                self,
                program: &TypedTrees,
                shapes: &[Shape],
                peer: Shape,
            ) -> Result<Shape, String> {
                if !self.is_closed() {
                    return Ok(peer);
                }
                let Shape::Integer(carrier, _, policy) = peer else {
                    return Ok(peer);
                };
                let references = shapes
                    .iter()
                    .map(|shape| reference(program, *shape))
                    .collect::<Result<Vec<_>, _>>()?;
                let anonymous = shapes
                    .iter()
                    .map(|shape| matches!(shape, Shape::Anonymous(_)))
                    .collect::<Vec<_>>();
                let reference =
                    validation::join_result_type_references(program, &references, &anonymous)
                        .ok_or("closed Match result has incompatible type qualifications")?;
                Ok(Shape::Integer(carrier, reference, policy))
            }

            pub(super) fn has_builtin(
                self,
                program: &TypedTrees,
                expression: ExpressionHandle,
            ) -> bool {
                match self {
                    Self::Machine(machine, state) => has_builtin_binary_expression_meaning(
                        program,
                        machine,
                        Some(state),
                        expression,
                    ),
                    Self::Closed => true,
                }
            }

            pub(super) fn require_operator(
                self,
                program: &TypedTrees,
                expression: ExpressionHandle,
                left: Shape,
                right: Shape,
            ) -> Result<(), String> {
                if !self.is_closed() {
                    return Ok(());
                }
                use language_core::OperatorSpelling as Spelling;
                let Some(binary) = program.expression_table.binary_node(expression) else {
                    return Err("constant operator lost its binary node".into());
                };
                let spelling = match binary.operator {
                    BinaryOperator::Add => Spelling::Add,
                    BinaryOperator::Subtract => Spelling::Subtract,
                    BinaryOperator::Multiply => Spelling::Multiply,
                    BinaryOperator::Divide => Spelling::Divide,
                    BinaryOperator::Modulo => Spelling::Modulo,
                    BinaryOperator::Equal => Spelling::Equal,
                    BinaryOperator::NotEqual => Spelling::NotEqual,
                    BinaryOperator::Less => Spelling::Less,
                    BinaryOperator::LessOrEqual => Spelling::LessEqual,
                    BinaryOperator::Greater => Spelling::Greater,
                    BinaryOperator::GreaterOrEqual => Spelling::GreaterEqual,
                    BinaryOperator::CaseMembership => {
                        return Err("constant case membership needs its selected owner".into());
                    }
                    BinaryOperator::And
                    | BinaryOperator::Or
                    | BinaryOperator::BitwiseAnd
                    | BinaryOperator::BitwiseOr
                    | BinaryOperator::BitwiseXor
                    | BinaryOperator::ShiftLeft
                    | BinaryOperator::ShiftRight => return Ok(()),
                };
                let references = [reference(program, left)?, reference(program, right)?];
                let anonymous = matches!((left, right), (Shape::Anonymous(_), Shape::Anonymous(_)));
                let builtin = if anonymous {
                    has_builtin_anonymous_operands(program, expression, spelling)
                } else {
                    typed_trees::operator::has_builtin_spelled_expression_meaning(
                        program,
                        symbols::SymbolHandle::invalid(),
                        expression,
                        spelling,
                        &references,
                    )
                };
                let selected_elsewhere =
                    program
                        .machine_specializations
                        .iter()
                        .any(|specialization| {
                            !typed_trees::operator::selected_trait_operator_meanings(
                                program,
                                specialization.instance,
                                spelling,
                                &references,
                            )
                            .is_empty()
                        });
                if builtin && !selected_elsewhere {
                    Ok(())
                } else {
                    Err(
                "closed constant operator requires exact context-independent builtin meaning"
                    .into(),
            )
                }
            }
        }

        pub(super) fn reference(
            program: &TypedTrees,
            shape: Shape,
        ) -> Result<Option<TypeReferenceHandle>, String> {
            let primitive = match shape {
                Shape::Anonymous(_) => return Ok(None),
                Shape::Boolean => PrimitiveType::Bool,
                Shape::Float(format) => super::float_primitive(format),
                Shape::Integer(_, reference, policy) if reference.is_valid() => {
                    if !type_reference_table(program).contains_type_reference(reference) {
                        return Err("constant call result type belongs to another program".into());
                    }
                    return if super::reference_policy(program, reference)? == policy {
                        Ok(Some(reference))
                    } else {
                        type_reference_table(program)
                            .find_policy_qualified_type_reference(reference, policy)
                            .map(Some)
                            .ok_or_else(|| {
                                "constant operand lost its selected arithmetic policy type".into()
                            })
                    };
                }
                Shape::Integer(carrier, _, _) => primitive(carrier)?,
            };
            use symbols::BuiltinTypeAtom as Atom;
            let atom = match primitive {
                PrimitiveType::Bool => Atom::Bool,
                PrimitiveType::F32 => Atom::F32,
                PrimitiveType::F64 => Atom::F64,
                PrimitiveType::I8 => Atom::I8,
                PrimitiveType::I16 => Atom::I16,
                PrimitiveType::I32 => Atom::I32,
                PrimitiveType::I64 => Atom::I64,
                PrimitiveType::U8 => Atom::U8,
                PrimitiveType::U16 => Atom::U16,
                PrimitiveType::U32 => Atom::U32,
                PrimitiveType::U64 => Atom::U64,
                _ => return Err("constant operand has no exact scalar carrier".into()),
            };
            let symbol = program
                .symbols
                .child_handles(program.symbols.root())
                .and_then(|mut children| {
                    children.find(|symbol| program.symbols.builtin_type_atom(*symbol) == Some(atom))
                })
                .ok_or("constant operand lost its builtin type")?;
            type_reference_table(program)
                .find_arithmetic_result_type_reference(
                    symbol,
                    match shape {
                        Shape::Integer(_, _, policy) => policy,
                        _ => numerics::arithmetic::ArithmeticDomain::Exact,
                    },
                )
                .map(Some)
                .ok_or_else(|| "constant operand lost its type reference".into())
        }
    }
    mod match_dispatch {
        use validation::evaluate_anonymous_numeric_expression;

        mod rational_bounds {
            use numerics::bignum::{BigInt, BigRational};
            use numerics::literals::LandedIntegerType;
            use typed_trees::{
                TypedTrees,
                expression::{BinaryOperator, ExpressionHandle, ExpressionNode},
            };
            use validation::evaluate_anonymous_numeric_expression;

            pub(super) fn excludes_zero(
                program: &TypedTrees,
                root: ExpressionHandle,
                builtin: impl FnMut(ExpressionHandle) -> bool,
            ) -> Result<bool, String> {
                Ok(analyze(program, root, builtin)?.excludes_zero())
            }

            pub(super) fn validate_integer_landing(
                program: &TypedTrees,
                root: ExpressionHandle,
                carrier: LandedIntegerType,
                builtin: impl FnMut(ExpressionHandle) -> bool,
            ) -> Result<bool, String> {
                let bounds = analyze(program, root, builtin)?;
                if !bounds.has_integral_lattice() {
                    return Err(
            "anonymous constant Match landing requires an all-arm integral result proof".into(),
        );
                }
                let width = carrier.bit_width();
                let (minimum, maximum) = if carrier.is_signed() {
                    let magnitude = 1i128 << (width - 1);
                    (-magnitude, magnitude - 1)
                } else {
                    (0, (1i128 << width) - 1)
                };
                let minimum = BigRational::from_integer(BigInt::from_i128(minimum));
                let maximum = BigRational::from_integer(BigInt::from_i128(maximum));
                if !bounds.intervals().all(|interval| {
                    !interval.low.cmp_value(&minimum).is_lt()
                        && !interval.high.cmp_value(&maximum).is_gt()
                }) {
                    return Err(format!(
                        "anonymous constant Match landing requires every arm to fit `{}`",
                        carrier.name()
                    ));
                }
                Ok(bounds.fractional_history)
            }

            fn analyze(
                program: &TypedTrees,
                root: ExpressionHandle,
                mut builtin: impl FnMut(ExpressionHandle) -> bool,
            ) -> Result<RationalBounds, String> {
                enum Step {
                    Enter(ExpressionHandle),
                    Binary(BinaryOperator),
                    Join(usize),
                }
                let mut pending = vec![Step::Enter(root)];
                let mut values: Vec<RationalBounds> = Vec::new();
                while let Some(step) = pending.pop() {
                    match step {
                        Step::Enter(expression) => match program
                            .expression_table
                            .expression(expression)
                        {
                            ExpressionNode::Match(dispatch) => {
                                let arms = program.expression_table.match_arms(dispatch.arms);
                                if arms.is_empty() || arms.len() != dispatch.arms.len() {
                                    return Err(
                                        "anonymous rational bounds require valid Match arms".into(),
                                    );
                                }
                                pending.push(Step::Join(arms.len()));
                                pending.extend(arms.iter().rev().map(|arm| Step::Enter(arm.value)));
                            }
                            ExpressionNode::Binary(binary) => {
                                if !builtin(expression) {
                                    return Err(
                            "anonymous rational bounds require selected builtin meaning".into()
                        );
                                }
                                pending.push(Step::Binary(binary.operator));
                                pending.push(Step::Enter(binary.right));
                                pending.push(Step::Enter(binary.left));
                            }
                            ExpressionNode::Integer(_) | ExpressionNode::Float(_) => {
                                let value = evaluate_anonymous_numeric_expression(
                                    program,
                                    expression,
                                    &[],
                                    &mut builtin,
                                )
                                .ok_or(
                                    "anonymous rational bounds require exact anonymous literals",
                                )?;
                                values.push(RationalBounds::constant(value));
                            }
                            _ => {
                                return Err(
                                    "unsupported expression in anonymous rational bounds".into()
                                );
                            }
                        },
                        Step::Binary(operator) => {
                            let right = values
                                .pop()
                                .ok_or("missing right anonymous rational bounds")?;
                            let left = values
                                .pop()
                                .ok_or("missing left anonymous rational bounds")?;
                            values.push(left.apply(operator, &right)?);
                        }
                        Step::Join(count) => {
                            let mut joined = values
                                .pop()
                                .ok_or("missing anonymous Match result bounds")?;
                            for _ in 1..count {
                                joined.include(
                                    values.pop().ok_or("missing anonymous Match arm bounds")?,
                                );
                            }
                            values.push(joined);
                        }
                    }
                }
                if values.len() != 1 {
                    return Err("anonymous arithmetic did not produce one rational range".into());
                }
                values
                    .pop()
                    .ok_or("missing anonymous rational bounds".into())
            }

            #[derive(Clone, Default)]
            struct RationalBounds {
                negative: Option<RationalCell>,
                positive: Option<RationalCell>,
                containing_zero: Option<RationalCell>,
                fractional_history: bool,
                points: Option<Vec<BigRational>>,
            }

            #[derive(Clone)]
            struct RationalCell {
                interval: RationalInterval,
                lattice: Option<RationalLattice>,
            }

            impl RationalCell {
                fn evidence(&self) -> Option<RationalLattice> {
                    self.lattice
                        .clone()
                        .or_else(|| singleton_lattice(&self.interval))
                }

                fn include(
                    &mut self,
                    interval: RationalInterval,
                    lattice: Option<RationalLattice>,
                ) {
                    self.lattice = self
                        .evidence()
                        .zip(lattice.or_else(|| singleton_lattice(&interval)))
                        .and_then(|(left, right)| left.join(&right));
                    self.interval.include(interval);
                }
            }

            impl RationalBounds {
                fn constant(value: BigRational) -> Self {
                    let mut bounds = Self {
                        fractional_history: value.to_integer_exact().is_none(),
                        points: Some(vec![value.clone()]),
                        ..Self::default()
                    };
                    bounds.include_cell(
                        RationalInterval::constant(value.clone()),
                        Some(RationalLattice {
                            offset: value,
                            stride: BigRational::zero(),
                        }),
                    );
                    bounds
                }

                fn excludes_zero(&self) -> bool {
                    if let Some(points) = &self.points {
                        return !points.is_empty() && points.iter().all(|point| !point.is_zero());
                    }
                    self.containing_zero.is_none()
                        && (self.negative.is_some() || self.positive.is_some())
                }

                fn has_integral_lattice(&self) -> bool {
                    if let Some(points) = &self.points {
                        return !points.is_empty()
                            && points
                                .iter()
                                .all(|point| point.to_integer_exact().is_some());
                    }
                    self.cells().next().is_some()
                        && self.cells().all(|cell| {
                            cell.evidence().is_some_and(|lattice| lattice.is_integral())
                        })
                }

                fn cells(&self) -> impl Iterator<Item = &RationalCell> {
                    self.negative
                        .iter()
                        .chain(&self.positive)
                        .chain(&self.containing_zero)
                }

                fn intervals(&self) -> impl Iterator<Item = &RationalInterval> {
                    self.cells().map(|cell| &cell.interval)
                }

                fn include_cell(
                    &mut self,
                    interval: RationalInterval,
                    lattice: Option<RationalLattice>,
                ) {
                    if !interval.excludes_zero()
                        && let Some([negative, positive]) =
                            lattice.as_ref().and_then(RationalLattice::zero_neighbors)
                    {
                        if !interval.low.cmp_value(&negative).is_gt() {
                            self.include_cell(
                                RationalInterval {
                                    low: interval.low.clone(),
                                    high: negative,
                                },
                                lattice.clone(),
                            );
                        }
                        if !interval.high.cmp_value(&positive).is_lt() {
                            self.include_cell(
                                RationalInterval {
                                    low: positive,
                                    high: interval.high,
                                },
                                lattice,
                            );
                        }
                        return;
                    }
                    let destination = if interval.high.cmp_value(&BigRational::zero()).is_lt() {
                        &mut self.negative
                    } else if interval.low.cmp_value(&BigRational::zero()).is_gt() {
                        &mut self.positive
                    } else {
                        &mut self.containing_zero
                    };
                    if let Some(existing) = destination {
                        existing.include(interval, lattice);
                    } else {
                        *destination = Some(RationalCell { interval, lattice });
                    }
                }

                fn include(&mut self, other: Self) {
                    self.fractional_history |= other.fractional_history;
                    self.points = match (self.points.take(), other.points) {
                        (Some(mut left), Some(right)) => {
                            left.extend(right);
                            sorted_unique(&mut left);
                            (left.len() <= MAX_EXACT_POINTS).then_some(left)
                        }
                        _ => None,
                    };
                    for cell in [other.negative, other.positive, other.containing_zero]
                        .into_iter()
                        .flatten()
                    {
                        self.include_cell(cell.interval, cell.lattice);
                    }
                }

                fn admissible_divisors(&self, cell: &RationalCell) -> Option<Vec<BigRational>> {
                    if let Some(points) = &self.points {
                        return Some(
                            points
                                .iter()
                                .filter(|point| cell.interval.contains(point))
                                .cloned()
                                .collect(),
                        );
                    }
                    cell.evidence()?.points_within(&cell.interval)
                }

                fn include_quotient(
                    &mut self,
                    left: &RationalCell,
                    right: &RationalBounds,
                    right_cell: &RationalCell,
                ) -> Result<(), String> {
                    let divisors = right.admissible_divisors(right_cell);
                    if !right_cell.interval.excludes_zero() {
                        match &divisors {
                            Some(divisors) if divisors.iter().all(|divisor| !divisor.is_zero()) => {
                            }
                            _ => {
                                return Err("anonymous rational division requires a nonzero divisor proof; its rational bounds include zero".into());
                            }
                        }
                    }
                    match divisors {
                        Some(divisors) if !divisors.is_empty() => {
                            for divisor in divisors {
                                let low = left
                                    .interval
                                    .low
                                    .div(&divisor)
                                    .ok_or("undefined anonymous rational quotient")?;
                                let high = left
                                    .interval
                                    .high
                                    .div(&divisor)
                                    .ok_or("undefined anonymous rational quotient")?;
                                let interval = if low.cmp_value(&high).is_gt() {
                                    RationalInterval {
                                        low: high,
                                        high: low,
                                    }
                                } else {
                                    RationalInterval { low, high }
                                };
                                let lattice = left.evidence().and_then(|left| {
                                    Some(RationalLattice {
                                        offset: left.offset.div(&divisor)?,
                                        stride: left.stride.div(&divisor)?,
                                    })
                                });
                                self.include_cell(interval, lattice);
                            }
                        }
                        _ => {
                            let interval = left
                                .interval
                                .apply(BinaryOperator::Divide, &right_cell.interval)?;
                            self.include_cell(interval, None);
                        }
                    }
                    Ok(())
                }

                fn apply(&self, operator: BinaryOperator, right: &Self) -> Result<Self, String> {
                    let mut result = Self {
                        fractional_history: self.fractional_history || right.fractional_history,
                        points: exact_apply(operator, &self.points, &right.points),
                        ..Self::default()
                    };
                    for left_cell in self.cells() {
                        for right_cell in right.cells() {
                            if operator == BinaryOperator::Divide {
                                result.include_quotient(left_cell, right, right_cell)?;
                                continue;
                            }
                            let interval =
                                left_cell.interval.apply(operator, &right_cell.interval)?;
                            let lattice = left_cell
                                .evidence()
                                .zip(right_cell.evidence())
                                .and_then(|(left, right)| left.apply(operator, &right));
                            result.include_cell(interval, lattice);
                        }
                    }
                    let unproven_integrality = result
                        .cells()
                        .any(|cell| !cell.evidence().is_some_and(|lattice| lattice.is_integral()));
                    result.fractional_history |= unproven_integrality;
                    if result.intervals().next().is_none() {
                        return Err("anonymous rational arithmetic requires nonempty bounds".into());
                    }
                    Ok(result)
                }
            }

            fn exact_apply(
                operator: BinaryOperator,
                left: &Option<Vec<BigRational>>,
                right: &Option<Vec<BigRational>>,
            ) -> Option<Vec<BigRational>> {
                let (left, right) = left.as_ref().zip(right.as_ref())?;
                if left.len().checked_mul(right.len())? > MAX_EXACT_POINT_PRODUCTS {
                    return None;
                }
                let mut points = Vec::new();
                for left in left {
                    for right in right {
                        let value = match operator {
                            BinaryOperator::Add => left.add(right),
                            BinaryOperator::Subtract => left.sub(right),
                            BinaryOperator::Multiply => left.mul(right),
                            BinaryOperator::Divide => left.div(right)?,
                            _ => return None,
                        };
                        points.push(value);
                    }
                }
                sorted_unique(&mut points);
                (points.len() <= MAX_EXACT_POINTS).then_some(points)
            }

            fn sorted_unique(points: &mut Vec<BigRational>) {
                points.sort_by(|left, right| left.cmp_value(right));
                points.dedup_by(|left, right| left.cmp_value(right).is_eq());
            }

            const MAX_EXACT_POINTS: usize = 256;
            const MAX_EXACT_POINT_PRODUCTS: usize = 4096;

            fn singleton_lattice(interval: &RationalInterval) -> Option<RationalLattice> {
                (interval.low == interval.high).then(|| RationalLattice {
                    offset: interval.low.clone(),
                    stride: BigRational::zero(),
                })
            }

            #[derive(Clone)]
            struct RationalLattice {
                offset: BigRational,
                stride: BigRational,
            }

            impl RationalLattice {
                fn is_integral(&self) -> bool {
                    self.offset.to_integer_exact().is_some()
                        && self.stride.to_integer_exact().is_some()
                }

                fn zero_neighbors(&self) -> Option<[BigRational; 2]> {
                    let stride = if self.stride.is_negative() {
                        self.stride.negate()
                    } else {
                        self.stride.clone()
                    };
                    let coordinate = self.offset.div(&stride)?;
                    let (numerator, denominator) = coordinate.as_integer_ratio();
                    let (_, mut remainder) = numerator.div_rem(denominator)?;
                    if remainder.is_zero() {
                        return None;
                    }
                    if remainder.is_negative() {
                        remainder = remainder.add(denominator);
                    }
                    let residue = BigRational::from_integer(remainder)
                        .div(&BigRational::from_integer(denominator.clone()))?;
                    let positive = stride.mul(&residue);
                    Some([positive.sub(&stride), positive])
                }

                fn points_within(&self, interval: &RationalInterval) -> Option<Vec<BigRational>> {
                    if self.stride.is_zero() {
                        let covered = !self.offset.cmp_value(&interval.low).is_lt()
                            && !self.offset.cmp_value(&interval.high).is_gt();
                        return Some(if covered {
                            vec![self.offset.clone()]
                        } else {
                            Vec::new()
                        });
                    }
                    let stride = if self.stride.is_negative() {
                        self.stride.negate()
                    } else {
                        self.stride.clone()
                    };
                    let lowest = ceiling(&interval.low.sub(&self.offset).div(&stride)?);
                    let highest = floor(&interval.high.sub(&self.offset).div(&stride)?);
                    if highest < lowest {
                        return Some(Vec::new());
                    }
                    let width = highest.sub(&lowest);
                    let width = width.to_u64()?;
                    if width >= MAX_ENUMERATED_LATTICE_POINTS as u64 {
                        return None;
                    }
                    let mut points = Vec::with_capacity(width as usize + 1);
                    let mut point = self
                        .offset
                        .add(&stride.mul(&BigRational::from_integer(lowest.clone())));
                    let mut index = lowest;
                    loop {
                        points.push(point.clone());
                        if index == highest {
                            break;
                        }
                        index = index.add(&BigInt::from_u64(1));
                        point = point.add(&stride);
                    }
                    Some(points)
                }

                fn join(&self, other: &Self) -> Option<Self> {
                    Some(Self {
                        offset: self.offset.clone(),
                        stride: common_divisor(
                            &common_divisor(&self.stride, &other.stride)?,
                            &self.offset.sub(&other.offset),
                        )?,
                    })
                }

                fn apply(&self, operator: BinaryOperator, right: &Self) -> Option<Self> {
                    let (offset, stride) = match operator {
                        BinaryOperator::Add | BinaryOperator::Subtract => (
                            if operator == BinaryOperator::Add {
                                self.offset.add(&right.offset)
                            } else {
                                self.offset.sub(&right.offset)
                            },
                            common_divisor(&self.stride, &right.stride)?,
                        ),
                        BinaryOperator::Multiply => (
                            self.offset.mul(&right.offset),
                            common_divisor(
                                &common_divisor(
                                    &self.stride.mul(&right.offset),
                                    &self.offset.mul(&right.stride),
                                )?,
                                &self.stride.mul(&right.stride),
                            )?,
                        ),
                        BinaryOperator::Divide if right.stride.is_zero() => (
                            self.offset.div(&right.offset)?,
                            self.stride.div(&right.offset)?,
                        ),
                        _ => return None,
                    };
                    Some(Self { offset, stride })
                }
            }

            const MAX_ENUMERATED_LATTICE_POINTS: usize = 256;

            fn floor(value: &BigRational) -> BigInt {
                let (numerator, denominator) = value.as_integer_ratio();
                let (quotient, remainder) = numerator
                    .div_rem(denominator)
                    .expect("a rational denominator is nonzero");
                if remainder.is_negative() {
                    quotient.sub(&BigInt::from_u64(1))
                } else {
                    quotient
                }
            }

            fn ceiling(value: &BigRational) -> BigInt {
                let (numerator, denominator) = value.as_integer_ratio();
                let (quotient, remainder) = numerator
                    .div_rem(denominator)
                    .expect("a rational denominator is nonzero");
                if remainder.is_zero() || remainder.is_negative() {
                    quotient
                } else {
                    quotient.add(&BigInt::from_u64(1))
                }
            }

            fn common_divisor(left: &BigRational, right: &BigRational) -> Option<BigRational> {
                let (left_numerator, left_denominator) = left.as_integer_ratio();
                let (right_numerator, right_denominator) = right.as_integer_ratio();
                let numerator = left_numerator
                    .mul(right_denominator)
                    .gcd(&right_numerator.mul(left_denominator));
                let denominator = left_denominator.mul(right_denominator);
                BigRational::from_integer(numerator).div(&BigRational::from_integer(denominator))
            }

            #[derive(Clone)]
            struct RationalInterval {
                low: BigRational,
                high: BigRational,
            }

            impl RationalInterval {
                fn constant(value: BigRational) -> Self {
                    Self {
                        low: value.clone(),
                        high: value,
                    }
                }

                fn excludes_zero(&self) -> bool {
                    self.low.cmp_value(&BigRational::zero()).is_gt()
                        || self.high.cmp_value(&BigRational::zero()).is_lt()
                }

                fn contains(&self, point: &BigRational) -> bool {
                    !point.cmp_value(&self.low).is_lt() && !point.cmp_value(&self.high).is_gt()
                }

                fn include(&mut self, other: Self) {
                    if other.low.cmp_value(&self.low).is_lt() {
                        self.low = other.low;
                    }
                    if other.high.cmp_value(&self.high).is_gt() {
                        self.high = other.high;
                    }
                }

                fn corners([first, second, third, fourth]: [BigRational; 4]) -> Self {
                    let mut bounds = Self::constant(first);
                    for value in [second, third, fourth] {
                        bounds.include(Self::constant(value));
                    }
                    bounds
                }

                fn apply(&self, operator: BinaryOperator, right: &Self) -> Result<Self, String> {
                    Ok(match operator {
                        BinaryOperator::Add => Self {
                            low: self.low.add(&right.low),
                            high: self.high.add(&right.high),
                        },
                        BinaryOperator::Subtract => Self {
                            low: self.low.sub(&right.high),
                            high: self.high.sub(&right.low),
                        },
                        BinaryOperator::Multiply => Self::corners([
                            self.low.mul(&right.low),
                            self.low.mul(&right.high),
                            self.high.mul(&right.low),
                            self.high.mul(&right.high),
                        ]),
                        BinaryOperator::Divide => {
                            if !right.excludes_zero() {
                                return Err("anonymous rational division requires a nonzero divisor proof; its rational bounds include zero".into());
                            }
                            let divide = |left: &BigRational, right: &BigRational| {
                                left.div(right)
                                    .ok_or("undefined anonymous rational quotient")
                            };
                            Self::corners([
                                divide(&self.low, &right.low)?,
                                divide(&self.low, &right.high)?,
                                divide(&self.high, &right.low)?,
                                divide(&self.high, &right.high)?,
                            ])
                        }
                        _ => {
                            return Err(
                                "anonymous rational bounds require arithmetic meaning".into()
                            );
                        }
                    })
                }
            }
        }

        use super::{Shape, Value, land_anonymous, primitive};
        use crate::build_time_evaluation::expression_table;
        use diagnostics::Diagnostic;
        use numerics::bignum::BigRational;
        use numerics::literals::LandedIntegerType;
        use typed_trees::{
            TypedTrees,
            expression::{BinaryOperator, ExpressionHandle, ExpressionNode, MatchPattern},
            types::PrimitiveType,
        };

        #[derive(Clone, Copy)]
        pub(super) struct MatchPlan {
            pub expression: ExpressionHandle,
            pub subject: Shape,
            pub result: Shape,
        }

        pub(super) enum MatchSubject {
            Scalar(Value),
            Anonymous(BigRational),
        }

        impl MatchSubject {
            pub(super) fn new(
                program: &TypedTrees,
                value: Value,
                selected: &[(ExpressionHandle, ExpressionHandle)],
                builtin: impl FnMut(ExpressionHandle) -> bool,
            ) -> Result<Self, String> {
                if let Value::Anonymous(expression) = value {
                    evaluate_anonymous_numeric_expression(program, expression, selected, builtin)
                        .map(Self::Anonymous)
                        .ok_or("constant Match needs a defined anonymous subject".into())
                } else {
                    Ok(Self::Scalar(value))
                }
            }

            pub(super) fn matches(
                &self,
                program: &TypedTrees,
                pattern: Value,
                selected: &[(ExpressionHandle, ExpressionHandle)],
                builtin: impl FnMut(ExpressionHandle) -> bool,
            ) -> Result<bool, String> {
                match (self, pattern) {
                    (Self::Anonymous(subject), Value::Anonymous(expression)) => {
                        let pattern = evaluate_anonymous_numeric_expression(
                            program, expression, selected, builtin,
                        )
                        .ok_or("constant Match needs a defined anonymous pattern")?;
                        Ok(subject.cmp_value(&pattern).is_eq())
                    }
                    (Self::Scalar(subject), pattern) => {
                        let Value::Boolean(matched) = super::apply(
                            typed_trees::expression::BinaryOperator::Equal,
                            *subject,
                            pattern,
                        )?
                        else {
                            return Err("constant Match equality did not produce Boolean".into());
                        };
                        Ok(matched)
                    }
                    _ => Err("constant Match subject and pattern changed scalar types".into()),
                }
            }
        }

        pub(super) fn validate_graph(
            program: &TypedTrees,
            root: ExpressionHandle,
        ) -> Result<(), String> {
            let mut pending = vec![(root, false)];
            let mut active = Vec::new();
            let mut complete = Vec::new();
            while let Some((expression, finish)) = pending.pop() {
                if finish {
                    if active.pop() != Some(expression) {
                        return Err("invalid constant expression traversal".into());
                    }
                    complete.push(expression);
                    continue;
                }
                if !program.expression_table.expression_is_valid(expression)
                    || active.contains(&expression)
                {
                    return Err("invalid or cyclic constant expression".into());
                }
                if complete.contains(&expression) {
                    continue;
                }
                active.push(expression);
                pending.push((expression, true));
                match program.expression_table.expression(expression) {
                    ExpressionNode::Call(call) => {
                        let arguments = program.expression_table.expression_handles(call.arguments);
                        if arguments.len() != call.arguments.len() {
                            return Err("invalid constant call argument span".into());
                        }
                        pending.extend(arguments.iter().rev().map(|argument| (*argument, false)));
                    }
                    ExpressionNode::Match(dispatch) => {
                        let arms = program.expression_table.match_arms(dispatch.arms);
                        if arms.len() != dispatch.arms.len() || arms.is_empty() {
                            return Err("invalid or empty constant Match arm span".into());
                        }
                        for arm in arms.iter().rev() {
                            pending.push((arm.value, false));
                            if let MatchPattern::Value(pattern) = arm.pattern {
                                pending.push((pattern, false));
                            }
                        }
                        pending.push((dispatch.subject, false));
                    }
                    ExpressionNode::Unary(unary) => pending.push((unary.operand, false)),
                    ExpressionNode::Cast(cast) => pending.push((cast.value, false)),
                    ExpressionNode::Binary(binary) => {
                        pending.push((binary.right, false));
                        pending.push((binary.left, false));
                    }
                    ExpressionNode::Member(_) => {
                        pending.push((super::member_field_value(program, expression)?.0, false));
                    }
                    ExpressionNode::Boolean(_)
                    | ExpressionNode::Integer(_)
                    | ExpressionNode::Float(_)
                    | ExpressionNode::UndeterminedNaN(_) => {}
                    _ => return Err("unsupported node in exact scalar constant expression".into()),
                }
            }
            Ok(())
        }

        pub(super) fn validate_join(
            program: &TypedTrees,
            context: super::EvaluationContext<'_>,
            expression: ExpressionHandle,
            shapes: &mut Vec<Shape>,
            warnings: &mut Vec<Diagnostic>,
        ) -> Result<MatchPlan, String> {
            let Some(dispatch) = program.expression_table.match_node(expression) else {
                return Err("constant Match plan lost its dispatch".into());
            };
            let arms = program.expression_table.match_arms(dispatch.arms);
            let child_count = 1 + arms
                .iter()
                .map(|arm| 1 + usize::from(matches!(arm.pattern, MatchPattern::Value(_))))
                .sum::<usize>();
            let start = shapes
                .len()
                .checked_sub(child_count)
                .ok_or("constant Match lost child types")?;
            let children = shapes.split_off(start);
            let mut children = children.into_iter();
            let subject = children.next().ok_or("constant Match lost subject type")?;
            let mut patterns = vec![subject];
            let mut results = Vec::new();
            for arm in arms {
                if matches!(arm.pattern, MatchPattern::Value(_)) {
                    patterns.push(children.next().ok_or("constant Match lost pattern type")?);
                }
                results.push(children.next().ok_or("constant Match lost result type")?);
            }
            let subject = join(program, context, &patterns, warnings)?;
            let result = join(program, context, &results, warnings)?;
            let result = if matches!(result, Shape::Anonymous(_)) {
                Shape::Anonymous(expression)
            } else {
                result
            };
            let mut diagnostics = Vec::new();
            if let super::EvaluationContext::Machine(machine, state) = context {
                validation::validate_match_dispatch(
                    program,
                    machine,
                    state,
                    expression,
                    dispatch,
                    &mut diagnostics,
                );
            } else {
                let wildcard = arms
                    .iter()
                    .any(|arm| matches!(arm.pattern, MatchPattern::Wildcard));
                let covered = [false, true].into_iter().all(|expected| arms.iter().any(|arm| matches!(arm.pattern, MatchPattern::Value(pattern) if matches!(program.expression_table.expression(pattern), ExpressionNode::Boolean(value) if *value == expected))));
                if !wildcard && !(matches!(subject, Shape::Boolean) && covered) {
                    return Err("match requires exhaustive Boolean patterns or a wildcard".into());
                }
            }
            if !diagnostics.is_empty() {
                return Err(diagnostics::join_messages(&diagnostics));
            }
            Ok(MatchPlan {
                expression,
                subject,
                result,
            })
        }

        fn join(
            program: &TypedTrees,
            context: super::EvaluationContext<'_>,
            shapes: &[Shape],
            warnings: &mut Vec<Diagnostic>,
        ) -> Result<Shape, String> {
            let peer = shapes
                .iter()
                .copied()
                .find(|shape| !matches!(shape, Shape::Anonymous(_)))
                .or_else(|| shapes.first().copied())
                .ok_or("empty constant Match type join")?;
            for shape in shapes {
                match (*shape, peer) {
                    (Shape::Boolean, Shape::Boolean) => {}
                    (Shape::Float(left), Shape::Float(right)) if left == right => {}
                    (Shape::Anonymous(expression), Shape::Float(_)) => {
                        validate_anonymous_fragments(program, context, expression)?;
                    }
                    (Shape::Anonymous(expression), Shape::Anonymous(_)) => {
                        validate_anonymous_fragments(program, context, expression)?;
                    }
                    (
                        Shape::Integer(left, _, left_policy),
                        Shape::Integer(right, _, right_policy),
                    ) if left == right && left_policy == right_policy => {}
                    (Shape::Anonymous(expression), Shape::Integer(carrier, _, _)) => {
                        validate_landing(
                            program,
                            context,
                            expression,
                            primitive(carrier)?,
                            warnings,
                        )?;
                    }
                    _ => {
                        return Err(
                            "constant Match has incompatible subject, pattern or result types"
                                .into(),
                        );
                    }
                }
            }
            context.joined_result(program, shapes, peer)
        }

        pub(super) fn validate_landing(
            program: &TypedTrees,
            context: super::EvaluationContext<'_>,
            expression: ExpressionHandle,
            destination: PrimitiveType,
            warnings: &mut Vec<Diagnostic>,
        ) -> Result<LandedIntegerType, String> {
            let carrier = destination
                .landed_landing_fixed()
                .ok_or("anonymous constant destination is not an exact integer carrier")?;
            let mut pending = vec![expression];
            while let Some(expression) = pending.pop() {
                if let Some(dispatch) = program.expression_table.match_node(expression) {
                    pending.extend(
                        expression_table(program)
                            .match_arms(dispatch.arms)
                            .iter()
                            .map(|arm| arm.value),
                    );
                } else if contains_match(program, expression) {
                    validate_anonymous_fragments(program, context, expression)?;
                    let fractional_history = rational_bounds::validate_integer_landing(
                        program,
                        expression,
                        carrier,
                        |operand| context.has_builtin(program, operand),
                    )?;
                    if fractional_history {
                        validate_fractional_landings(
                            program,
                            context,
                            expression,
                            destination,
                            warnings,
                        )?;
                    }
                } else {
                    land_anonymous(program, context, expression, destination, &[], warnings)?;
                }
            }
            Ok(carrier)
        }

        struct LandingSummary {
            value: BigRational,
            fractional: Option<(ExpressionHandle, BigRational)>,
            selection: Vec<(ExpressionHandle, ExpressionHandle)>,
        }

        fn validate_fractional_landings(
            program: &TypedTrees,
            context: super::EvaluationContext<'_>,
            root: ExpressionHandle,
            destination: PrimitiveType,
            warnings: &mut Vec<Diagnostic>,
        ) -> Result<(), String> {
            for summary in landing_summaries(program, context, root)? {
                land_anonymous(
                    program,
                    context,
                    root,
                    destination,
                    &summary.selection,
                    warnings,
                )?;
            }
            Ok(())
        }

        fn landing_summaries(
            program: &TypedTrees,
            context: super::EvaluationContext<'_>,
            root: ExpressionHandle,
        ) -> Result<Vec<LandingSummary>, String> {
            enum Step {
                Enter(ExpressionHandle),
                Leave(ExpressionHandle),
            }
            let mut pending = vec![Step::Enter(root)];
            let mut results: Vec<Vec<LandingSummary>> = Vec::new();
            while let Some(step) = pending.pop() {
                match step {
                    Step::Enter(expression) => {
                        match program.expression_table.expression(expression) {
                            ExpressionNode::Match(dispatch) => {
                                let arms = program.expression_table.match_arms(dispatch.arms);
                                if arms.is_empty() || arms.len() != dispatch.arms.len() {
                                    return Err(
                                        "anonymous constant Match landing requires valid arms"
                                            .into(),
                                    );
                                }
                                pending.push(Step::Leave(expression));
                                pending.extend(arms.iter().rev().map(|arm| Step::Enter(arm.value)));
                            }
                            ExpressionNode::Binary(binary) => {
                                if !context.has_builtin(program, expression) {
                                    return Err(
                            "anonymous rational bounds require selected builtin meaning".into()
                        );
                                }
                                pending.push(Step::Leave(expression));
                                pending.push(Step::Enter(binary.right));
                                pending.push(Step::Enter(binary.left));
                            }
                            _ => {
                                let value = evaluate_anonymous_numeric_expression(
                        program,
                        expression,
                        &[],
                        |operand| context.has_builtin(program, operand),
                    )
                    .ok_or("anonymous constant expression requires defined exact numeric values")?;
                                let fractional = (matches!(
                                    program.expression_table.expression(expression),
                                    ExpressionNode::Float(_)
                                ) && value.to_integer_exact().is_none())
                                .then_some((expression, value.clone()));
                                results.push(vec![LandingSummary {
                                    value,
                                    fractional,
                                    selection: Vec::new(),
                                }]);
                            }
                        }
                    }
                    Step::Leave(expression) => {
                        match program.expression_table.expression(expression) {
                            ExpressionNode::Match(dispatch) => {
                                let arms = program.expression_table.match_arms(dispatch.arms);
                                let count = arms.len();
                                if results.len() < count {
                                    return Err(
                                        "anonymous constant Match lost its arm results".into()
                                    );
                                }
                                let selected = results.split_off(results.len() - count);
                                let mut joined = Vec::new();
                                for (arm, mut arm_results) in arms.iter().zip(selected) {
                                    for summary in &mut arm_results {
                                        summary.selection.push((expression, arm.value));
                                    }
                                    append_unique(&mut joined, arm_results)?;
                                }
                                results.push(joined);
                            }
                            ExpressionNode::Binary(binary) => {
                                let Some(right) = results.pop() else {
                                    return Err(
                                        "anonymous constant operation lost its right operand"
                                            .into(),
                                    );
                                };
                                let Some(left) = results.pop() else {
                                    return Err(
                                        "anonymous constant operation lost its left operand".into(),
                                    );
                                };
                                let mut joined = Vec::new();
                                for left in &left {
                                    for right in &right {
                                        let value = match binary.operator {
                                            BinaryOperator::Add => left.value.add(&right.value),
                                            BinaryOperator::Subtract => {
                                                left.value.sub(&right.value)
                                            }
                                            BinaryOperator::Multiply => {
                                                left.value.mul(&right.value)
                                            }
                                            BinaryOperator::Divide => left
                                                .value
                                                .div(&right.value)
                                                .ok_or("undefined anonymous rational quotient")?,
                                            _ => {
                                                return Err(
                                        "anonymous rational bounds require arithmetic meaning"
                                            .into(),
                                    );
                                            }
                                        };
                                        let fractional = if let Some(fractional) = &left.fractional
                                        {
                                            Some(fractional.clone())
                                        } else if let Some(fractional) = &right.fractional {
                                            Some(fractional.clone())
                                        } else {
                                            (value.to_integer_exact().is_none())
                                                .then_some((expression, value.clone()))
                                        };
                                        let mut selection = left.selection.clone();
                                        selection.extend(right.selection.iter().copied());
                                        push_landing_summary(
                                            &mut joined,
                                            LandingSummary {
                                                value,
                                                fractional,
                                                selection,
                                            },
                                        )?;
                                    }
                                }
                                results.push(joined);
                            }
                            _ => {
                                return Err(
                                    "anonymous constant Match landing lost its traversal".into()
                                );
                            }
                        }
                    }
                }
            }
            if results.len() != 1 {
                return Err("anonymous constant expression did not produce one summary".into());
            }
            results
                .pop()
                .ok_or("anonymous constant expression did not produce one summary".into())
        }

        fn push_landing_summary(
            summaries: &mut Vec<LandingSummary>,
            summary: LandingSummary,
        ) -> Result<(), String> {
            let duplicate = summaries.iter().any(|existing| {
                existing.value.cmp_value(&summary.value).is_eq()
                    && match (&existing.fractional, &summary.fractional) {
                        (None, None) => true,
                        (Some((left_origin, left_value)), Some((right_origin, right_value))) => {
                            left_origin == right_origin && left_value.cmp_value(right_value).is_eq()
                        }
                        _ => false,
                    }
            });
            if !duplicate {
                if summaries.len() >= MAX_LANDING_SUMMARIES {
                    return Err(
                "anonymous constant Match landing has more exact fractional-warning paths than the bounded summary"
                    .into(),
            );
                }
                summaries.push(summary);
            }
            Ok(())
        }

        fn append_unique(
            summaries: &mut Vec<LandingSummary>,
            arm_results: Vec<LandingSummary>,
        ) -> Result<(), String> {
            for summary in arm_results {
                push_landing_summary(summaries, summary)?;
            }
            Ok(())
        }

        const MAX_LANDING_SUMMARIES: usize = 4096;

        fn contains_match(program: &TypedTrees, root: ExpressionHandle) -> bool {
            let mut pending = vec![root];
            while let Some(expression) = pending.pop() {
                match program.expression_table.expression(expression) {
                    ExpressionNode::Match(_) => return true,
                    ExpressionNode::Binary(binary) => {
                        pending.push(binary.right);
                        pending.push(binary.left);
                    }
                    _ => {}
                }
            }
            false
        }

        pub(super) fn validate_anonymous_fragments(
            program: &TypedTrees,
            context: super::EvaluationContext<'_>,
            root: ExpressionHandle,
        ) -> Result<(), String> {
            let mut pending = vec![root];
            while let Some(expression) = pending.pop() {
                if let Some(dispatch) = program.expression_table.match_node(expression) {
                    pending.extend(
                        expression_table(program)
                            .match_arms(dispatch.arms)
                            .iter()
                            .map(|arm| arm.value),
                    );
                } else if contains_match(program, expression) {
                    let Some(binary) = program.expression_table.binary_node(expression) else {
                        return Err("invalid anonymous result graph".into());
                    };
                    if binary.operator == typed_trees::expression::BinaryOperator::Divide {
                        validate_nonzero_divisor(program, context, binary.right)?;
                    }
                    pending.push(binary.right);
                    pending.push(binary.left);
                } else {
                    evaluate_anonymous_numeric_expression(program, expression, &[], |operand| {
                        context.has_builtin(program, operand)
                    })
                    .ok_or("anonymous constant expression requires defined exact numeric values")?;
                }
            }
            Ok(())
        }

        fn validate_nonzero_divisor(
            program: &TypedTrees,
            context: super::EvaluationContext<'_>,
            root: ExpressionHandle,
        ) -> Result<(), String> {
            if !rational_bounds::excludes_zero(program, root, |operand| {
                context.has_builtin(program, operand)
            })? {
                return Err("anonymous constant division requires a nonzero divisor proof; its rational bounds include zero".into());
            }
            Ok(())
        }

        pub(super) fn validate_anonymous_comparison(
            program: &TypedTrees,
            context: super::EvaluationContext<'_>,
            expression: ExpressionHandle,
        ) -> Result<(), String> {
            if contains_match(program, expression) {
                let Some(binary) = program.expression_table.binary_node(expression) else {
                    return Err("anonymous comparison lost operands".into());
                };
                validate_anonymous_fragments(program, context, binary.left)?;
                validate_anonymous_fragments(program, context, binary.right)
            } else {
                super::compare_anonymous(program, context, expression, &[]).map(|_| ())
            }
        }

        pub(super) fn coerce(
            program: &TypedTrees,
            context: super::EvaluationContext<'_>,
            value: Value,
            shape: Shape,
            selected: &[(ExpressionHandle, ExpressionHandle)],
            warnings: &mut Vec<Diagnostic>,
        ) -> Result<Value, String> {
            match (value, shape) {
                (Value::Anonymous(expression), Shape::Float(format)) => {
                    let exact = evaluate_anonymous_numeric_expression(
                        program,
                        expression,
                        selected,
                        |operand| context.has_builtin(program, operand),
                    )
                    .ok_or("floating constant requires a defined exact anonymous value")?;
                    Ok(Value::Float(
                        format,
                        match format {
                            numerics::literals::FloatFormat::F32 => {
                                u64::from(exact.to_f32().to_bits())
                            }
                            numerics::literals::FloatFormat::F64 => exact.to_f64().to_bits(),
                        },
                    ))
                }
                (Value::Anonymous(expression), Shape::Integer(carrier, _, policy)) => {
                    land_anonymous(
                        program,
                        context,
                        expression,
                        primitive(carrier)?,
                        selected,
                        warnings,
                    )
                    .map(|value| super::retag_landed(value, policy))
                }
                (Value::Anonymous(_), Shape::Anonymous(_))
                | (Value::Boolean(_), Shape::Boolean) => Ok(value),
                (Value::Landed(left, _, left_policy), Shape::Integer(right, _, right_policy))
                    if left == right && left_policy == right_policy =>
                {
                    Ok(value)
                }
                (Value::Float(left, _), Shape::Float(right)) if left == right => Ok(value),
                _ => Err("constant Match execution changed its scalar join type".into()),
            }
        }
    }
    use context::EvaluationContext;

    pub(crate) trait ConstantCalls {
        fn result_type(&self, _expression: ExpressionHandle) -> TypeReferenceHandle {
            TypeReferenceHandle::invalid()
        }

        fn validate_call(
            &self,
            expression: ExpressionHandle,
        ) -> Result<(PrimitiveType, Vec<Diagnostic>), String>;

        fn evaluate_call(
            &self,
            expression: ExpressionHandle,
        ) -> Result<(CanonicalConstValue, Vec<Diagnostic>), String>;

        fn evaluate_scalar_call(
            &self,
            expression: ExpressionHandle,
        ) -> Result<(ScalarValue, Vec<Diagnostic>), String> {
            self.evaluate_call(expression)
                .map(|(value, warnings)| (ScalarValue::Index(value), warnings))
        }
    }

    #[derive(Clone, Copy)]
    struct ScalarDestination {
        primitive: PrimitiveType,
        policy: ArithmeticDomain,
    }

    #[derive(Clone, Copy)]
    enum Shape {
        Anonymous(ExpressionHandle),
        Boolean,
        Float(FloatFormat),
        Integer(LandedIntegerType, TypeReferenceHandle, ArithmeticDomain),
    }

    #[derive(Clone, Copy)]
    enum Value {
        Anonymous(ExpressionHandle),
        Boolean(bool),
        Float(FloatFormat, u64),
        UndeterminedNaN(FloatFormat),
        Landed(LandedIntegerType, IntegerValue, ArithmeticDomain),
    }

    pub(crate) enum ScalarValue {
        Index(CanonicalConstValue),
        Float { format: FloatFormat, bits: u64 },
        UndeterminedNaN(FloatFormat),
    }

    impl ScalarValue {
        pub(crate) fn declaration_value(&self) -> language_core::constant_value::EvaluatedConstant {
            use language_core::constant_value::EvaluatedConstant;
            match self {
                Self::UndeterminedNaN(format) => EvaluatedConstant::UndeterminedNaN(*format),
                Self::Index(value) => EvaluatedConstant::Encoded(value.encoding.clone()),
                Self::Float {
                    format: FloatFormat::F32,
                    bits,
                } => EvaluatedConstant::Encoded(format!("float:f32:{bits:08x}")),
                Self::Float {
                    format: FloatFormat::F64,
                    bits,
                } => EvaluatedConstant::Encoded(format!("float:f64:{bits:016x}")),
            }
        }

        pub(crate) fn type_name(&self) -> &str {
            match self {
                Self::Index(value) => &value.type_name,
                Self::Float { format, .. } | Self::UndeterminedNaN(format) => format.name(),
            }
        }

        pub(crate) fn encoding(&self) -> Result<String, String> {
            Ok(match self {
                Self::Index(value) => value.encoding.clone(),
                Self::Float {
                    format: FloatFormat::F32,
                    bits,
                } => format!("float:f32:{bits:08x}"),
                Self::Float {
                    format: FloatFormat::F64,
                    bits,
                } => format!("float:f64:{bits:016x}"),
                Self::UndeterminedNaN(_) => {
                    return Err(
                        "constant result is NaN without an exact raw-NaN realization".into(),
                    );
                }
            })
        }

        pub(crate) fn into_index(self) -> Result<CanonicalConstValue, String> {
            match self {
                Self::Index(value) => Ok(value),
                Self::Float { .. } | Self::UndeterminedNaN(_) => {
                    Err("floating declaration values are not canonical indices".into())
                }
            }
        }
    }

    pub(crate) fn validate(
        program: &TypedTrees,
        machine: &Machine,
        state: &State,
        expression: ExpressionHandle,
        destination: PrimitiveType,
        calls: Option<&dyn ConstantCalls>,
    ) -> Result<Vec<Diagnostic>, String> {
        match_dispatch::validate_graph(program, expression)?;
        Ok(validate_shapes(
            program,
            EvaluationContext::Machine(machine, state),
            expression,
            Some(ScalarDestination {
                primitive: destination,
                policy: ArithmeticDomain::Exact,
            }),
            calls,
        )?
        .warnings)
    }

    pub(crate) fn evaluate_scalar(
        program: &TypedTrees,
        machine: &Machine,
        state: &State,
        expression: ExpressionHandle,
        destination: PrimitiveType,
        calls: Option<&dyn ConstantCalls>,
    ) -> Result<(ScalarValue, Vec<Diagnostic>), String> {
        evaluate_scalar_in(
            program,
            EvaluationContext::Machine(machine, state),
            expression,
            destination,
            ArithmeticDomain::Exact,
            calls,
        )
    }

    pub(crate) fn evaluate_scalar_with_policy(
        program: &TypedTrees,
        machine: &Machine,
        state: &State,
        expression: ExpressionHandle,
        destination: PrimitiveType,
        policy: ArithmeticDomain,
        calls: Option<&dyn ConstantCalls>,
    ) -> Result<(ScalarValue, Vec<Diagnostic>), String> {
        evaluate_scalar_in(
            program,
            EvaluationContext::Machine(machine, state),
            expression,
            destination,
            policy,
            calls,
        )
    }

    fn evaluate_scalar_in(
        program: &TypedTrees,
        context: EvaluationContext<'_>,
        expression: ExpressionHandle,
        destination: PrimitiveType,
        policy: ArithmeticDomain,
        calls: Option<&dyn ConstantCalls>,
    ) -> Result<(ScalarValue, Vec<Diagnostic>), String> {
        let (value, selected_arms, mut warnings) = evaluate_expression(
            program,
            context,
            expression,
            Some(ScalarDestination {
                primitive: destination,
                policy,
            }),
            calls,
        )?;
        let value = match value {
            Value::Anonymous(expression)
                if matches!(destination, PrimitiveType::F32 | PrimitiveType::F64) =>
            {
                let exact = evaluate_anonymous_numeric_expression(
                    program,
                    expression,
                    &selected_arms,
                    |operand| context.has_builtin(program, operand),
                )
                .ok_or("floating constant requires a defined exact anonymous value")?;
                let value = match destination {
                    PrimitiveType::F32 => ScalarValue::Float {
                        format: FloatFormat::F32,
                        bits: u64::from(exact.to_f32().to_bits()),
                    },
                    _ => ScalarValue::Float {
                        format: FloatFormat::F64,
                        bits: exact.to_f64().to_bits(),
                    },
                };
                return Ok((value, warnings));
            }
            Value::Anonymous(expression) => land_anonymous(
                program,
                context,
                expression,
                destination,
                &selected_arms,
                &mut warnings,
            )?,
            value => value,
        };
        if let Value::Float(format, bits) = value {
            return Ok((ScalarValue::Float { format, bits }, warnings));
        }
        if let Value::UndeterminedNaN(format) = value {
            return Ok((ScalarValue::UndeterminedNaN(format), warnings));
        }
        if let Value::Boolean(value) = value {
            return Ok((
                ScalarValue::Index(CanonicalConstValue::boolean(value)),
                warnings,
            ));
        }
        let Value::Landed(carrier, value, _) = value else {
            return Err("constant expression has no integer landing".into());
        };
        let value = match value {
            IntegerValue::Signed(value) => value,
            IntegerValue::Unsigned(value) => i128::try_from(value)
                .map_err(|_| "constant value exceeds canonical integer encoding")?,
        };
        let identity = CanonicalConstIdentity::integer(carrier.name(), value);
        Ok((
            ScalarValue::Index(CanonicalConstValue::new(
                identity.type_name,
                identity.encoding,
                value.to_string(),
            )),
            warnings,
        ))
    }

    fn member_field_value(
        program: &TypedTrees,
        expression: ExpressionHandle,
    ) -> Result<(ExpressionHandle, PrimitiveType), String> {
        validation::closed_record_scalar_projection(program, expression).ok_or_else(|| {
            let ExpressionNode::Member(member) = program.expression_table.expression(expression)
            else {
                return "constant projection has no member selection".to_owned();
            };
            format!(
                "constant member read cannot select closed record field `{}` in its exact carrier",
                member.member
            )
        })
    }

    fn evaluate_expression(
        program: &TypedTrees,
        context: EvaluationContext<'_>,
        expression: ExpressionHandle,
        destination: Option<ScalarDestination>,
        calls: Option<&dyn ConstantCalls>,
    ) -> Result<
        (
            Value,
            Vec<(ExpressionHandle, ExpressionHandle)>,
            Vec<Diagnostic>,
        ),
        String,
    > {
        if !program.expression_table.expression_is_valid(expression) {
            return Err("invalid constant expression".to_owned());
        }
        enum Step {
            Enter(ExpressionHandle),
            Cast(ExpressionHandle, LandedIntegerType),
            Projection(PrimitiveType),
            Unary(ExpressionHandle, UnaryOperator),
            Binary(ExpressionHandle, BinaryOperator),
            LogicalLeft(ExpressionHandle, BinaryOperator, ExpressionHandle),
            LogicalRight(ExpressionHandle),
            MatchSubject(ExpressionHandle),
            MatchNext(ExpressionHandle, usize, match_dispatch::MatchSubject),
            MatchPattern(ExpressionHandle, usize, match_dispatch::MatchSubject),
            MatchResult(ExpressionHandle),
        }
        let mut pending = vec![Step::Enter(expression)];
        let mut active = Vec::new();
        let mut values = Vec::new();
        match_dispatch::validate_graph(program, expression)?;
        let ValidatedShapes {
            mut warnings,
            matches,
            call_shapes,
            ..
        } = validate_shapes(program, context, expression, destination, calls)?;
        let mut selected_arms = Vec::new();
        while let Some(step) = pending.pop() {
            match step {
                Step::Enter(expression) => {
                    if !program.expression_table.expression_is_valid(expression)
                        || active.contains(&expression)
                    {
                        return Err("invalid or cyclic constant expression".into());
                    }
                    match program.expression_table.expression(expression) {
                        ExpressionNode::Call(_) => {
                            let shape = call_shapes
                                .iter()
                                .find(|(source, _)| *source == expression)
                                .map(|(_, shape)| *shape)
                                .ok_or("constant call has no validated scalar carrier")?;
                            let (value, call_warnings) = calls
                                .ok_or("machine calls require constant invocation admission")?
                                .evaluate_scalar_call(expression)?;
                            values.push(match value {
                            ScalarValue::Index(value) => call_value(&value, shape)?,
                            ScalarValue::Float { format, bits }
                                if matches!(shape, Shape::Float(expected) if expected == format) =>
                            {
                                Value::Float(format, bits)
                            }
                            ScalarValue::UndeterminedNaN(format)
                                if matches!(shape, Shape::Float(expected) if expected == format) =>
                            {
                                Value::UndeterminedNaN(format)
                            }
                            _ => return Err("constant call result differs from its validated scalar carrier".into()),
                        });
                            extend_unique(&mut warnings, call_warnings);
                        }
                        ExpressionNode::Match(dispatch) => {
                            active.push(expression);
                            pending.push(Step::MatchSubject(expression));
                            pending.push(Step::Enter(dispatch.subject));
                        }
                        ExpressionNode::Boolean(value) => values.push(Value::Boolean(*value)),
                        ExpressionNode::UndeterminedNaN(format) => {
                            values.push(Value::UndeterminedNaN(*format))
                        }
                        ExpressionNode::Integer(literal) if literal.landing().is_some() => {
                            values.push(landed_literal(literal)?);
                        }
                        ExpressionNode::Integer(_) => values.push(Value::Anonymous(expression)),
                        ExpressionNode::Float(literal) if literal.landing().is_none() => {
                            values.push(Value::Anonymous(expression));
                        }
                        ExpressionNode::Float(literal) => {
                            let format = literal
                                .landing()
                                .ok_or("floating literal lost its format")?;
                            values.push(Value::Float(
                                format,
                                match format {
                                    FloatFormat::F32 => u64::from(literal.f32_bits()),
                                    FloatFormat::F64 => literal.landed_f64().to_bits(),
                                },
                            ));
                        }
                        ExpressionNode::Unary(unary) => {
                            active.push(expression);
                            pending.push(Step::Unary(expression, unary.operator));
                            pending.push(Step::Enter(unary.operand));
                        }
                        ExpressionNode::Cast(cast) => {
                            active.push(expression);
                            pending
                                .push(Step::Cast(expression, cast_integer_carrier(program, cast)?));
                            pending.push(Step::Enter(cast.value));
                        }
                        ExpressionNode::Binary(binary) => {
                            if !context.has_builtin(program, expression) {
                                return Err(
                                    "constant expression has no selected builtin operator meaning"
                                        .into(),
                                );
                            }
                            active.push(expression);
                            if matches!(binary.operator, BinaryOperator::And | BinaryOperator::Or) {
                                pending.push(Step::LogicalLeft(
                                    expression,
                                    binary.operator,
                                    binary.right,
                                ));
                            } else {
                                pending.push(Step::Binary(expression, binary.operator));
                                pending.push(Step::Enter(binary.right));
                            }
                            pending.push(Step::Enter(binary.left));
                        }
                        ExpressionNode::Member(_) => {
                            let (field, primitive) = member_field_value(program, expression)?;
                            pending.push(Step::Projection(primitive));
                            pending.push(Step::Enter(field));
                        }
                        _ => {
                            return Err(
                                "unsupported node in exact integer constant expression".into()
                            );
                        }
                    }
                }
                Step::Projection(primitive) => {
                    let value = values
                        .pop()
                        .ok_or("constant projection lost its scalar value")?;
                    let value = match value {
                        Value::Anonymous(field)
                            if matches!(primitive, PrimitiveType::F32 | PrimitiveType::F64) =>
                        {
                            let format = match primitive {
                                PrimitiveType::F32 => FloatFormat::F32,
                                _ => FloatFormat::F64,
                            };
                            Value::Float(
                                format,
                                land_anonymous_float(
                                    program,
                                    context,
                                    field,
                                    format,
                                    &selected_arms,
                                )?,
                            )
                        }
                        Value::Anonymous(field) => land_anonymous(
                            program,
                            context,
                            field,
                            primitive,
                            &selected_arms,
                            &mut warnings,
                        )?,
                        value => value,
                    };
                    let matches = match (value, scalar_shape(primitive)?) {
                        (Value::Boolean(_), Shape::Boolean) => true,
                        (
                            Value::Float(actual, _) | Value::UndeterminedNaN(actual),
                            Shape::Float(expected),
                        ) => actual == expected,
                        (
                            Value::Landed(actual, _, policy),
                            Shape::Integer(expected, _, expected_policy),
                        ) => actual == expected && policy == expected_policy,
                        _ => false,
                    };
                    if !matches {
                        return Err(
                            "constant projected value differs from its selected field carrier"
                                .into(),
                        );
                    }
                    values.push(value);
                }
                Step::MatchSubject(expression) => {
                    let plan = matches
                        .iter()
                        .find(|plan| plan.expression == expression)
                        .ok_or("missing constant Match plan")?;
                    let subject = values.pop().ok_or("missing constant Match subject")?;
                    let subject = match_dispatch::coerce(
                        program,
                        context,
                        subject,
                        plan.subject,
                        &selected_arms,
                        &mut warnings,
                    )?;
                    let subject = match_dispatch::MatchSubject::new(
                        program,
                        subject,
                        &selected_arms,
                        |operand| context.has_builtin(program, operand),
                    )?;
                    pending.push(Step::MatchNext(expression, 0, subject));
                }
                Step::MatchNext(expression, ordinal, subject) => {
                    let Some(dispatch) = program.expression_table.match_node(expression) else {
                        return Err("constant Match lost dispatch".into());
                    };
                    let arm = expression_table(program)
                        .match_arms(dispatch.arms)
                        .get(ordinal)
                        .ok_or("constant Match has no selected arm")?;
                    match arm.pattern {
                        typed_trees::expression::MatchPattern::Destructure(_) => {
                            return Err(
                            "structural constant Match requires saved-subject value interpretation"
                                .into(),
                        );
                        }
                        typed_trees::expression::MatchPattern::Wildcard => {
                            selected_arms.push((expression, arm.value));
                            pending.push(Step::MatchResult(expression));
                            pending.push(Step::Enter(arm.value));
                        }
                        typed_trees::expression::MatchPattern::Value(pattern) => {
                            pending.push(Step::MatchPattern(expression, ordinal, subject));
                            pending.push(Step::Enter(pattern));
                        }
                    }
                }
                Step::MatchPattern(expression, ordinal, subject) => {
                    let plan = matches
                        .iter()
                        .find(|plan| plan.expression == expression)
                        .ok_or("missing constant Match plan")?;
                    let pattern = values.pop().ok_or("missing constant Match pattern")?;
                    let pattern = match_dispatch::coerce(
                        program,
                        context,
                        pattern,
                        plan.subject,
                        &selected_arms,
                        &mut warnings,
                    )?;
                    let matched = subject.matches(program, pattern, &selected_arms, |operand| {
                        context.has_builtin(program, operand)
                    })?;
                    if matched {
                        let Some(dispatch) = program.expression_table.match_node(expression) else {
                            return Err("constant Match lost dispatch".into());
                        };
                        let arm = expression_table(program)
                            .match_arms(dispatch.arms)
                            .get(ordinal)
                            .ok_or("constant Match lost arm")?;
                        selected_arms.push((expression, arm.value));
                        pending.push(Step::MatchResult(expression));
                        pending.push(Step::Enter(arm.value));
                    } else {
                        pending.push(Step::MatchNext(expression, ordinal + 1, subject));
                    }
                }
                Step::MatchResult(expression) => {
                    if active.pop() != Some(expression) {
                        return Err("invalid constant Match traversal".into());
                    }
                    let plan = matches
                        .iter()
                        .find(|plan| plan.expression == expression)
                        .ok_or("missing constant Match plan")?;
                    let value = values.pop().ok_or("constant Match lost selected result")?;
                    values.push(match_dispatch::coerce(
                        program,
                        context,
                        value,
                        plan.result,
                        &selected_arms,
                        &mut warnings,
                    )?);
                }
                Step::LogicalLeft(expression, operator, right) => {
                    let Some(Value::Boolean(left)) = values.pop() else {
                        return Err("Boolean logic requires a Boolean left operand".into());
                    };
                    if left == (operator == BinaryOperator::And) {
                        pending.push(Step::LogicalRight(expression));
                        pending.push(Step::Enter(right));
                    } else {
                        if active.pop() != Some(expression) {
                            return Err("invalid constant expression traversal".into());
                        }
                        values.push(Value::Boolean(left));
                    }
                }
                Step::LogicalRight(expression) => {
                    if active.pop() != Some(expression) {
                        return Err("invalid constant expression traversal".into());
                    }
                    if !matches!(values.last(), Some(Value::Boolean(_))) {
                        return Err("Boolean logic requires a Boolean right operand".into());
                    }
                }
                Step::Cast(expression, target) => {
                    if active.pop() != Some(expression) {
                        return Err("invalid constant expression traversal".into());
                    }
                    let operand = values.pop().ok_or("missing constant cast operand")?;
                    let result = match operand {
                        /* `as` lands its complete anonymous operand explicitly,
                        so the implicit-landing error is acknowledged and
                        discarded; integrality and range still reject. */
                        Value::Anonymous(operand) => land_anonymous(
                            program,
                            context,
                            operand,
                            primitive(target)?,
                            &selected_arms,
                            &mut Vec::new(),
                        )?,
                        Value::Landed(source, value, _) => Value::Landed(
                            target,
                            integer_type(source)?
                                .exact_cast_value_to(integer_type(target)?, value)
                                .ok_or("constant cast value does not fit its target carrier")?,
                            ArithmeticDomain::Exact,
                        ),
                        _ => return Err("constant cast requires an integer operand".into()),
                    };
                    let Some(cast) = program.expression_table.cast_node(expression) else {
                        return Err("constant cast lost its expression".into());
                    };
                    values.push(retag_landed(result, cast.domain));
                }
                Step::Unary(expression, operator) => {
                    if active.pop() != Some(expression) {
                        return Err("invalid constant expression traversal".into());
                    }
                    let operand = values.pop().ok_or("missing unary constant operand")?;
                    let result = match (operator, operand) {
                        (UnaryOperator::LogicalNot, Value::Boolean(value)) => {
                            Value::Boolean(!value)
                        }
                        (UnaryOperator::BitwiseNot, Value::Landed(carrier, value, policy)) => {
                            let value = integer_type(carrier)?
                                .bitwise_not(value)
                                .ok_or("integer complement operand does not inhabit its carrier")?;
                            Value::Landed(carrier, value, policy)
                        }
                        _ => {
                            return Err(
                                "unary constant operator has incompatible operand type".into()
                            );
                        }
                    };
                    values.push(result);
                }
                Step::Binary(expression, operator) => {
                    if active.pop() != Some(expression) {
                        return Err("invalid constant expression traversal".into());
                    }
                    let right = values.pop().ok_or("missing right constant operand")?;
                    let left = values.pop().ok_or("missing left constant operand")?;
                    let value = match (left, right) {
                        (Value::Anonymous(_), Value::Anonymous(_))
                            if comparison_operator(operator) =>
                        {
                            Value::Boolean(compare_anonymous(
                                program,
                                context,
                                expression,
                                &selected_arms,
                            )?)
                        }
                        (Value::Anonymous(_), Value::Anonymous(_)) => {
                            if !matches!(
                                operator,
                                BinaryOperator::Add
                                    | BinaryOperator::Subtract
                                    | BinaryOperator::Multiply
                                    | BinaryOperator::Divide
                            ) {
                                return Err(
                                "anonymous constant operation requires a selected fixed carrier"
                                    .into(),
                            );
                            }
                            Value::Anonymous(expression)
                        }
                        (Value::Landed(carrier, left, policy), Value::Anonymous(right)) => {
                            let right_destination = if matches!(
                                operator,
                                BinaryOperator::ShiftLeft | BinaryOperator::ShiftRight
                            ) {
                                PrimitiveType::U64
                            } else {
                                primitive(carrier)?
                            };
                            let right = land_anonymous(
                                program,
                                context,
                                right,
                                right_destination,
                                &selected_arms,
                                &mut warnings,
                            )?;
                            apply(
                                operator,
                                Value::Landed(carrier, left, policy),
                                retag_landed(
                                    right,
                                    if matches!(
                                        operator,
                                        BinaryOperator::ShiftLeft | BinaryOperator::ShiftRight
                                    ) {
                                        ArithmeticDomain::Exact
                                    } else {
                                        policy
                                    },
                                ),
                            )?
                        }
                        (Value::Anonymous(left), Value::Landed(carrier, right, policy)) => {
                            if matches!(
                                operator,
                                BinaryOperator::ShiftLeft | BinaryOperator::ShiftRight
                            ) {
                                return Err("shift value requires a selected fixed carrier".into());
                            }
                            let left = land_anonymous(
                                program,
                                context,
                                left,
                                primitive(carrier)?,
                                &selected_arms,
                                &mut warnings,
                            )?;
                            apply(
                                operator,
                                retag_landed(left, policy),
                                Value::Landed(carrier, right, policy),
                            )?
                        }
                        (
                            left @ (Value::Float(format, _) | Value::UndeterminedNaN(format)),
                            Value::Anonymous(right),
                        ) => {
                            let right = land_anonymous_float(
                                program,
                                context,
                                right,
                                format,
                                &selected_arms,
                            )?;
                            apply(operator, left, Value::Float(format, right))?
                        }
                        (
                            Value::Anonymous(left),
                            right @ (Value::Float(format, _) | Value::UndeterminedNaN(format)),
                        ) => {
                            let left = land_anonymous_float(
                                program,
                                context,
                                left,
                                format,
                                &selected_arms,
                            )?;
                            apply(operator, Value::Float(format, left), right)?
                        }
                        (left, right) => apply(operator, left, right)?,
                    };
                    values.push(value);
                }
            }
        }
        if values.len() != 1 {
            return Err("constant expression did not produce one value".into());
        }
        Ok((
            values.pop().ok_or("missing constant expression value")?,
            selected_arms,
            warnings,
        ))
    }

    struct ValidatedShapes {
        warnings: Vec<Diagnostic>,
        matches: Vec<match_dispatch::MatchPlan>,
        call_shapes: Vec<(ExpressionHandle, Shape)>,
    }

    fn validate_shapes(
        program: &TypedTrees,
        context: EvaluationContext<'_>,
        root: ExpressionHandle,
        destination: Option<ScalarDestination>,
        calls: Option<&dyn ConstantCalls>,
    ) -> Result<ValidatedShapes, String> {
        let destination_policy = destination.map(|destination| destination.policy);
        if let Some(policy) = destination_policy {
            require_supported_policy(policy)?;
        }
        let destination = destination.map(|destination| destination.primitive);
        let mut pending = vec![(root, false)];
        let mut active = Vec::new();
        let mut shapes = Vec::new();
        let mut warnings = Vec::new();
        let mut matches = Vec::new();
        let mut call_shapes = Vec::new();
        while let Some((expression, finish)) = pending.pop() {
            if !program.expression_table.expression_is_valid(expression) {
                return Err("invalid constant expression".into());
            }
            if !finish {
                if active.contains(&expression) {
                    return Err("cyclic constant expression".into());
                }
                match program.expression_table.expression(expression) {
                    ExpressionNode::Call(_) => {
                        let (primitive, call_warnings) = calls
                            .ok_or("machine calls require constant invocation admission")?
                            .validate_call(expression)?;
                        let mut shape = scalar_shape(primitive)?;
                        if let Shape::Integer(_, reference, policy) = &mut shape {
                            *reference =
                                calls.expect("validated call owner").result_type(expression);
                            *policy = reference_policy(program, *reference)?;
                        }
                        shapes.push(shape);
                        call_shapes.push((expression, shape));
                        extend_unique(&mut warnings, call_warnings);
                    }
                    ExpressionNode::Match(dispatch) => {
                        active.push(expression);
                        pending.push((expression, true));
                        for arm in expression_table(program)
                            .match_arms(dispatch.arms)
                            .iter()
                            .rev()
                        {
                            pending.push((arm.value, false));
                            if let typed_trees::expression::MatchPattern::Value(pattern) =
                                arm.pattern
                            {
                                pending.push((pattern, false));
                            }
                        }
                        pending.push((dispatch.subject, false));
                    }
                    ExpressionNode::Boolean(_) => shapes.push(Shape::Boolean),
                    ExpressionNode::UndeterminedNaN(format) => shapes.push(Shape::Float(*format)),
                    ExpressionNode::Integer(literal) if literal.landing().is_some() => {
                        let Value::Landed(carrier, _, policy) = landed_literal(literal)? else {
                            unreachable!()
                        };
                        shapes.push(Shape::Integer(
                            carrier,
                            TypeReferenceHandle::invalid(),
                            policy,
                        ));
                    }
                    ExpressionNode::Integer(_) => shapes.push(Shape::Anonymous(expression)),
                    ExpressionNode::Float(literal) if literal.landing().is_none() => {
                        shapes.push(Shape::Anonymous(expression))
                    }
                    ExpressionNode::Float(literal) => {
                        shapes.push(Shape::Float(
                            literal
                                .landing()
                                .ok_or("floating literal lost its format")?,
                        ));
                    }
                    ExpressionNode::Unary(unary) => {
                        active.push(expression);
                        pending.push((expression, true));
                        pending.push((unary.operand, false));
                    }
                    ExpressionNode::Cast(cast) => {
                        active.push(expression);
                        pending.push((expression, true));
                        pending.push((cast.value, false));
                    }
                    ExpressionNode::Binary(binary) => {
                        if !context.has_builtin(program, expression) {
                            return Err(
                                "constant expression has no selected builtin operator meaning"
                                    .into(),
                            );
                        }
                        active.push(expression);
                        pending.push((expression, true));
                        pending.push((binary.right, false));
                        pending.push((binary.left, false));
                    }
                    ExpressionNode::Member(_) => {
                        let (_, primitive) = member_field_value(program, expression)?;
                        shapes.push(scalar_shape(primitive)?);
                    }
                    _ => return Err("unsupported node in exact scalar constant expression".into()),
                }
                continue;
            }
            if active.pop() != Some(expression) {
                return Err("invalid constant expression traversal".into());
            }
            if matches!(
                program.expression_table.expression(expression),
                ExpressionNode::Match(_)
            ) {
                let plan = match_dispatch::validate_join(
                    program,
                    context,
                    expression,
                    &mut shapes,
                    &mut warnings,
                )?;
                shapes.push(plan.result);
                matches.push(plan);
                continue;
            }
            if let Some(unary) = program.expression_table.unary_node(expression) {
                let operand = shapes.pop().ok_or("missing unary constant operand type")?;
                let result = match (unary.operator, operand) {
                    (UnaryOperator::LogicalNot, Shape::Boolean) => Shape::Boolean,
                    (UnaryOperator::BitwiseNot, Shape::Integer(_, _, _)) => {
                        context.arithmetic_result(program, operand)?
                    }
                    _ => return Err("unary constant operator has incompatible operand type".into()),
                };
                shapes.push(result);
                continue;
            }
            if let Some(cast) = program.expression_table.cast_node(expression) {
                let target = cast_integer_carrier(program, cast)?;
                let source = shapes.pop().ok_or("missing constant cast operand type")?;
                match source {
                Shape::Anonymous(operand) => {
                    // Explicit landing: see the matching evaluation step.
                    match_dispatch::validate_landing(
                        program, context, operand, primitive(target)?, &mut Vec::new(),
                    )?;
                }
                Shape::Integer(source, _, _) if source == target
                    || integer_type(source)?.can_widen_to(integer_type(target)?) => {}
                _ => return Err("constant cast needs a total integer conversion or checked partial-conversion evidence".into()),
            }
                shapes.push(Shape::Integer(target, cast.target_type, cast.domain));
                continue;
            }
            let Some(binary) = program.expression_table.binary_node(expression) else {
                unreachable!()
            };
            let operator = binary.operator;
            let right = shapes.pop().ok_or("missing right constant operand type")?;
            let left = shapes.pop().ok_or("missing left constant operand type")?;
            context.require_operator(program, expression, left, right)?;
            let shift = matches!(
                operator,
                BinaryOperator::ShiftLeft | BinaryOperator::ShiftRight
            );
            let (left, right) = match (left, right) {
                (Shape::Anonymous(_), Shape::Anonymous(_)) if comparison_operator(operator) => {
                    match_dispatch::validate_anonymous_comparison(program, context, expression)?;
                    shapes.push(Shape::Boolean);
                    continue;
                }
                (Shape::Anonymous(_), Shape::Anonymous(_)) => {
                    if !matches!(
                        operator,
                        BinaryOperator::Add
                            | BinaryOperator::Subtract
                            | BinaryOperator::Multiply
                            | BinaryOperator::Divide
                    ) {
                        return Err(
                            "anonymous constant operation requires a selected fixed carrier".into(),
                        );
                    }
                    shapes.push(Shape::Anonymous(expression));
                    continue;
                }
                (Shape::Integer(carrier, reference, policy), Shape::Anonymous(right)) => {
                    let destination = if shift {
                        PrimitiveType::U64
                    } else {
                        primitive(carrier)?
                    };
                    let right_carrier = match_dispatch::validate_landing(
                        program,
                        context,
                        right,
                        destination,
                        &mut warnings,
                    )?;
                    (
                        Shape::Integer(carrier, reference, policy),
                        Shape::Integer(
                            right_carrier,
                            TypeReferenceHandle::invalid(),
                            if shift {
                                ArithmeticDomain::Exact
                            } else {
                                policy
                            },
                        ),
                    )
                }
                (Shape::Anonymous(left), Shape::Integer(carrier, reference, policy)) => {
                    if shift {
                        return Err("shift value requires a selected fixed carrier".into());
                    }
                    match_dispatch::validate_landing(
                        program,
                        context,
                        left,
                        primitive(carrier)?,
                        &mut warnings,
                    )?;
                    (
                        Shape::Integer(carrier, TypeReferenceHandle::invalid(), policy),
                        Shape::Integer(carrier, reference, policy),
                    )
                }
                (Shape::Float(format), Shape::Anonymous(right)) => {
                    match_dispatch::validate_anonymous_fragments(program, context, right)?;
                    (Shape::Float(format), Shape::Float(format))
                }
                (Shape::Anonymous(left), Shape::Float(format)) => {
                    match_dispatch::validate_anonymous_fragments(program, context, left)?;
                    (Shape::Float(format), Shape::Float(format))
                }
                operands => operands,
            };
            let result = match (left, right) {
                (Shape::Boolean, Shape::Boolean)
                    if matches!(
                        operator,
                        BinaryOperator::And
                            | BinaryOperator::Or
                            | BinaryOperator::Equal
                            | BinaryOperator::NotEqual
                    ) =>
                {
                    Shape::Boolean
                }
                (
                    Shape::Integer(left, left_reference, left_policy),
                    Shape::Integer(right, right_reference, right_policy),
                ) => {
                    if matches!(operator, BinaryOperator::And | BinaryOperator::Or) {
                        return Err("Boolean logic requires Boolean operands".into());
                    }
                    if !shift
                        && !comparison_operator(operator)
                        && (left != right || left_policy != right_policy)
                    {
                        return Err(
                            "constant operands have incompatible landed integer carriers".into(),
                        );
                    }
                    if comparison_operator(operator) {
                        Shape::Boolean
                    } else {
                        context.arithmetic_result(
                            program,
                            Shape::Integer(right, right_reference, right_policy),
                        )?;
                        context.arithmetic_result(
                            program,
                            Shape::Integer(left, left_reference, left_policy),
                        )?
                    }
                }
                (Shape::Float(left), Shape::Float(right)) => {
                    if left != right {
                        return Err(
                            "constant float operands have incompatible landed formats".into()
                        );
                    }
                    if comparison_operator(operator) {
                        Shape::Boolean
                    } else if matches!(
                        operator,
                        BinaryOperator::Add
                            | BinaryOperator::Subtract
                            | BinaryOperator::Multiply
                            | BinaryOperator::Divide
                    ) {
                        Shape::Float(left)
                    } else {
                        return Err("constant operator has incompatible operand types".into());
                    }
                }
                _ => return Err("constant operator has incompatible operand types".into()),
            };
            shapes.push(result);
        }
        if shapes.len() != 1 {
            return Err("constant expression did not produce one type".into());
        }
        match shapes[0] {
            Shape::Anonymous(_)
                if matches!(
                    destination,
                    None | Some(PrimitiveType::F32 | PrimitiveType::F64)
                ) =>
            {
                match_dispatch::validate_anonymous_fragments(program, context, root)?;
            }
            Shape::Anonymous(_) => {
                match_dispatch::validate_landing(
                    program,
                    context,
                    root,
                    destination.expect("anonymous integer destination"),
                    &mut warnings,
                )?;
            }
            Shape::Boolean if destination.is_none() => {
                return Err(
                    "range endpoint position requires an exact builtin integer carrier".into(),
                );
            }
            Shape::Boolean if calls.is_none() || destination == Some(PrimitiveType::Bool) => {}
            Shape::Float(format) if destination == Some(float_primitive(format)) => {}
            Shape::Integer(carrier, _, _)
                if calls.is_none()
                    || destination
                        .is_none_or(|destination| primitive(carrier).ok() == Some(destination)) => {
            }
            _ => return Err("constant expression differs from its destination carrier".into()),
        }
        if let (Some(expected), Shape::Integer(_, _, actual)) = (destination_policy, shapes[0])
            && expected != actual
        {
            return Err(
                "constant expression cannot implicitly change its arithmetic policy".into(),
            );
        }
        Ok(ValidatedShapes {
            warnings,
            matches,
            call_shapes,
        })
    }

    fn scalar_shape(primitive: PrimitiveType) -> Result<Shape, String> {
        Ok(Shape::Integer(
            match primitive {
                PrimitiveType::Bool => return Ok(Shape::Boolean),
                PrimitiveType::F32 => return Ok(Shape::Float(FloatFormat::F32)),
                PrimitiveType::F64 => return Ok(Shape::Float(FloatFormat::F64)),
                primitive => primitive
                    .landed_landing_fixed()
                    .ok_or("constant call requires an exact integer or Boolean carrier")?,
            },
            TypeReferenceHandle::invalid(),
            ArithmeticDomain::Exact,
        ))
    }

    fn cast_integer_carrier(
        program: &TypedTrees,
        cast: &typed_trees::expression::TableCastExpression,
    ) -> Result<LandedIntegerType, String> {
        if cast.form.is_recast() || !cast.semantic_domain.is_empty() {
            return Err("constant cast requires an ordinary exact integer coercion".into());
        }
        let target = super::exact_probe_destination(program, cast.target_type)
            .ok_or("constant cast target requires an unqualified fixed integer carrier")?;
        let Shape::Integer(carrier, _, ArithmeticDomain::Exact) = scalar_shape(target)? else {
            return Err("constant cast target requires a fixed integer carrier".into());
        };
        Ok(carrier)
    }

    fn float_primitive(format: FloatFormat) -> PrimitiveType {
        match format {
            FloatFormat::F32 => PrimitiveType::F32,
            FloatFormat::F64 => PrimitiveType::F64,
        }
    }

    fn call_value(value: &CanonicalConstValue, shape: Shape) -> Result<Value, String> {
        match (shape, value.decode_encoding()) {
            (Shape::Boolean, Some(DecodedCanonicalConstValue::Boolean(result)))
                if value.identity() == CanonicalConstValue::boolean(result).identity() =>
            {
                Ok(Value::Boolean(result))
            }
            (
                Shape::Integer(carrier, _, policy),
                Some(DecodedCanonicalConstValue::Integer { value: result, .. }),
            ) if value.identity() == CanonicalConstIdentity::integer(carrier.name(), result) => {
                let result = if carrier.is_signed() {
                    IntegerValue::Signed(result)
                } else {
                    IntegerValue::Unsigned(
                        u128::try_from(result)
                            .map_err(|_| "negative unsigned constant call result")?,
                    )
                };
                if !integer_type(carrier)?.admits(result) {
                    return Err("constant call result exceeds its declared carrier".into());
                }
                Ok(Value::Landed(carrier, result, policy))
            }
            _ => Err("constant call result differs from its validated scalar carrier".into()),
        }
    }

    fn compare_anonymous(
        program: &TypedTrees,
        context: EvaluationContext<'_>,
        expression: ExpressionHandle,
        selected_arms: &[(ExpressionHandle, ExpressionHandle)],
    ) -> Result<bool, String> {
        evaluate_anonymous_numeric_comparison(program, expression, selected_arms, |operand| {
        context.has_builtin(program, operand)
    })
    .ok_or_else(|| {
        "anonymous comparison requires defined exact numeric operands and selected builtin meaning"
            .into()
    })
    }

    fn land_anonymous(
        program: &TypedTrees,
        context: EvaluationContext<'_>,
        expression: ExpressionHandle,
        destination: PrimitiveType,
        selected_arms: &[(ExpressionHandle, ExpressionHandle)],
        warnings: &mut Vec<Diagnostic>,
    ) -> Result<Value, String> {
        let (literal, warning) = validation::land_anonymous_integer_expression(
            program,
            expression,
            destination,
            selected_arms,
            |expression| context.has_builtin(program, expression),
        )
        .ok_or_else(|| {
            if let Some(value) = evaluate_anonymous_numeric_expression(
                program,
                expression,
                selected_arms,
                |operand| context.has_builtin(program, operand),
            ) && value.to_integer_exact().is_none()
            {
                return format!(
                    "exact anonymous value `{value}` cannot land in an integer const argument"
                );
            }
            "anonymous constant expression cannot land exactly at the selected integer carrier"
                .to_owned()
        })?;
        if let Some(warning) = warning
            && !warnings.contains(&warning)
        {
            warnings.push(warning);
        }
        landed_literal(&literal)
    }

    fn landed_literal(literal: &IntegerLiteral) -> Result<Value, String> {
        let landing = literal.landing().ok_or("missing integer landing")?;
        require_supported_policy(landing.domain)?;
        let carrier = integer_type(landing.landed_type)?;
        let value = if landing.landed_type.is_signed() {
            IntegerValue::Signed(i128::from(
                literal
                    .value_i64()
                    .ok_or("invalid signed constant literal")?,
            ))
        } else {
            IntegerValue::Unsigned(u128::from(
                literal
                    .value_u64()
                    .ok_or("invalid unsigned constant literal")?,
            ))
        };
        if !carrier.admits(value) {
            return Err("constant literal is outside its selected integer carrier".into());
        }
        Ok(Value::Landed(landing.landed_type, value, landing.domain))
    }

    fn land_anonymous_float(
        program: &TypedTrees,
        context: EvaluationContext<'_>,
        expression: ExpressionHandle,
        format: FloatFormat,
        selected_arms: &[(ExpressionHandle, ExpressionHandle)],
    ) -> Result<u64, String> {
        let exact =
            evaluate_anonymous_numeric_expression(program, expression, selected_arms, |operand| {
                context.has_builtin(program, operand)
            })
            .ok_or("floating operand requires a defined exact anonymous value")?;
        Ok(match format {
            FloatFormat::F32 => u64::from(exact.to_f32().to_bits()),
            FloatFormat::F64 => exact.to_f64().to_bits(),
        })
    }

    fn float_apply(
        operator: BinaryOperator,
        left_format: FloatFormat,
        left: FloatMeaning,
        right_format: FloatFormat,
        right: FloatMeaning,
    ) -> Result<Value, String> {
        if left_format != right_format {
            return Err("constant float operands have incompatible landed formats".into());
        }
        let semantic_format = match left_format {
            FloatFormat::F32 => SemanticFloatFormat::BINARY32,
            FloatFormat::F64 => SemanticFloatFormat::BINARY64,
        };
        match operator {
            BinaryOperator::Equal => {
                return Ok(Value::Boolean(FloatSemantics::equal(&left, &right)));
            }
            BinaryOperator::NotEqual => {
                return Ok(Value::Boolean(FloatSemantics::not_equal(&left, &right)));
            }
            BinaryOperator::Less => {
                return Ok(Value::Boolean(FloatSemantics::less(&left, &right)));
            }
            BinaryOperator::LessOrEqual => {
                return Ok(Value::Boolean(FloatSemantics::less_or_equal(&left, &right)));
            }
            BinaryOperator::Greater => {
                return Ok(Value::Boolean(FloatSemantics::greater(&left, &right)));
            }
            BinaryOperator::GreaterOrEqual => {
                return Ok(Value::Boolean(FloatSemantics::greater_or_equal(
                    &left, &right,
                )));
            }
            _ => {}
        }
        let meaning = match operator {
            BinaryOperator::Add => FloatSemantics::add(semantic_format, &left, &right),
            BinaryOperator::Subtract => FloatSemantics::subtract(semantic_format, &left, &right),
            BinaryOperator::Multiply => FloatSemantics::multiply(semantic_format, &left, &right),
            BinaryOperator::Divide => FloatSemantics::divide(semantic_format, &left, &right),
            _ => return Err("unsupported builtin floating constant operator".into()),
        };
        if meaning.is_nan() {
            return Ok(Value::UndeterminedNaN(left_format));
        }
        Ok(Value::Float(left_format, float_bits(left_format, &meaning)))
    }

    fn float_meaning(format: FloatFormat, bits: u64) -> FloatMeaning {
        match format {
            FloatFormat::F32 => FloatMeaning::from_f32(f32::from_bits(bits as u32)),
            FloatFormat::F64 => FloatMeaning::from_f64(f64::from_bits(bits)),
        }
    }

    fn floating_meaning(value: Value) -> Option<(FloatFormat, FloatMeaning)> {
        match value {
            Value::Float(format, bits) => Some((format, float_meaning(format, bits))),
            Value::UndeterminedNaN(format) => Some((format, FloatMeaning::NaN)),
            _ => None,
        }
    }

    fn float_bits(format: FloatFormat, meaning: &FloatMeaning) -> u64 {
        match format {
            FloatFormat::F32 => u64::from(meaning.to_f32().to_bits()),
            FloatFormat::F64 => meaning.to_f64().to_bits(),
        }
    }

    fn comparison_operator(operator: BinaryOperator) -> bool {
        matches!(
            operator,
            BinaryOperator::Equal
                | BinaryOperator::NotEqual
                | BinaryOperator::Less
                | BinaryOperator::LessOrEqual
                | BinaryOperator::Greater
                | BinaryOperator::GreaterOrEqual
        )
    }

    fn apply(operator: BinaryOperator, left: Value, right: Value) -> Result<Value, String> {
        if let (Value::Boolean(left), Value::Boolean(right)) = (left, right) {
            return match operator {
                BinaryOperator::Equal => Ok(Value::Boolean(left == right)),
                BinaryOperator::NotEqual => Ok(Value::Boolean(left != right)),
                _ => Err("unsupported builtin Boolean constant operator".into()),
            };
        }
        if let (Some((left_format, left)), Some((right_format, right))) =
            (floating_meaning(left), floating_meaning(right))
        {
            return float_apply(operator, left_format, left, right_format, right);
        }
        let (
            Value::Landed(left_carrier, left, policy),
            Value::Landed(right_carrier, right, right_policy),
        ) = (left, right)
        else {
            return Err("integer operation requires landed operands".into());
        };
        let integer = integer_type(left_carrier)?;
        let shifts = matches!(
            operator,
            BinaryOperator::ShiftLeft | BinaryOperator::ShiftRight
        );
        if !shifts
            && !comparison_operator(operator)
            && (left_carrier != right_carrier || policy != right_policy)
        {
            return Err("constant operands have incompatible landed integer carriers".into());
        }
        if matches!(
            operator,
            BinaryOperator::Equal
                | BinaryOperator::NotEqual
                | BinaryOperator::Less
                | BinaryOperator::LessOrEqual
                | BinaryOperator::Greater
                | BinaryOperator::GreaterOrEqual
        ) {
            let ordering = integer
                .compare_with(left, integer_type(right_carrier)?, right)
                .ok_or(
                    "constant comparison operands are outside their selected integer carriers",
                )?;
            let result = match operator {
                BinaryOperator::Equal => ordering.is_eq(),
                BinaryOperator::NotEqual => ordering.is_ne(),
                BinaryOperator::Less => ordering.is_lt(),
                BinaryOperator::LessOrEqual => ordering.is_le(),
                BinaryOperator::Greater => ordering.is_gt(),
                BinaryOperator::GreaterOrEqual => ordering.is_ge(),
                _ => return Err("constant comparison lost its operator".into()),
            };
            return Ok(Value::Boolean(result));
        }
        require_supported_policy(policy)?;
        use ArithmeticDomain::{Exact, Saturating, Wrapping};
        let result = match (operator, policy) {
        (BinaryOperator::Add, Exact) => integer.exact_add(left, right),
        (BinaryOperator::Add, Wrapping) => integer.wrapping_add(left, right),
        (BinaryOperator::Add, Saturating) => integer.saturating_add(left, right),
        (BinaryOperator::Subtract, Exact) => integer.exact_sub(left, right),
        (BinaryOperator::Subtract, Wrapping) => integer.wrapping_sub(left, right),
        (BinaryOperator::Subtract, Saturating) => integer.saturating_sub(left, right),
        (BinaryOperator::Multiply, Exact) => integer.exact_mul(left, right),
        (BinaryOperator::Multiply, Wrapping) => integer.wrapping_mul(left, right),
        (BinaryOperator::Multiply, Saturating) => integer.saturating_mul(left, right),
        (BinaryOperator::Divide, Exact) => integer.exact_div(left, right),
        (BinaryOperator::Divide, Wrapping) => integer.wrapping_div(left, right),
        (BinaryOperator::Divide, Saturating) => integer.saturating_div(left, right),
        (BinaryOperator::Modulo, Exact) => integer.exact_rem(left, right),
        (BinaryOperator::Modulo, Wrapping) => integer.wrapping_rem(left, right),
        (BinaryOperator::Modulo, Saturating) => integer.saturating_rem(left, right),
        (BinaryOperator::BitwiseAnd, _) => integer.bitwise_and(left, right),
        (BinaryOperator::BitwiseOr, _) => integer.bitwise_or(left, right),
        (BinaryOperator::BitwiseXor, _) => integer.bitwise_xor(left, right),
        (BinaryOperator::ShiftLeft, Exact) => {
            integer.exact_shift_left(left, integer_type(right_carrier)?, right)
        }
        (BinaryOperator::ShiftLeft, Wrapping) => {
            integer.wrapping_shift_left(left, integer_type(right_carrier)?, right)
        }
        (BinaryOperator::ShiftRight, Exact | Saturating) => {
            integer.exact_shift_right(left, integer_type(right_carrier)?, right)
        }
        (BinaryOperator::ShiftRight, Wrapping) => {
            integer.wrapping_shift_right(left, integer_type(right_carrier)?, right)
        }
        _ => return Err("unsupported builtin integer constant operation policy".into()),
    }
    .ok_or(format!(
        "{} integer constant operation overflows, divides by zero, or has an invalid shift count",
        policy.name()
    ))?;
        Ok(Value::Landed(left_carrier, result, policy))
    }

    fn require_supported_policy(policy: ArithmeticDomain) -> Result<(), String> {
        if matches!(
            policy,
            ArithmeticDomain::Exact | ArithmeticDomain::Wrapping | ArithmeticDomain::Saturating
        ) {
            Ok(())
        } else {
            Err("constant scalar evaluation requires a nontrapping integer policy".into())
        }
    }

    fn reference_policy(
        program: &TypedTrees,
        mut reference: TypeReferenceHandle,
    ) -> Result<ArithmeticDomain, String> {
        use typed_trees::types::{TypeConstraintNode, TypeReferenceNode};
        if !reference.is_valid() {
            return Ok(ArithmeticDomain::Exact);
        }
        let mut seen = Vec::new();
        let mut policy = None;
        loop {
            if !type_reference_table(program).contains_type_reference(reference)
                || seen.contains(&reference)
            {
                return Err("constant scalar type is invalid or cyclic".into());
            }
            seen.push(reference);
            let TypeReferenceNode::Constrained {
                base_type,
                constraints,
            } = program.type_reference_table.type_reference(reference)
            else {
                break;
            };
            let constraints = type_reference_table(program)
                .constraint_span(*constraints)
                .ok_or("constant scalar type has invalid constraints")?;
            for constraint in constraints {
                if let TypeConstraintNode::ArithmeticDomain(selected) = constraint {
                    require_supported_policy(*selected)?;
                    if policy.replace(*selected).is_some() {
                        return Err("constant scalar type has multiple arithmetic policies".into());
                    }
                }
            }
            reference = *base_type;
        }
        Ok(policy.unwrap_or(ArithmeticDomain::Exact))
    }

    fn retag_landed(value: Value, policy: ArithmeticDomain) -> Value {
        match value {
            Value::Landed(carrier, value, _) => Value::Landed(carrier, value, policy),
            value => value,
        }
    }

    fn integer_type(carrier: LandedIntegerType) -> Result<IntegerType, String> {
        if carrier == LandedIntegerType::Addr {
            return Err("address constant evaluation requires target authority".into());
        }
        let sign = if carrier.is_signed() {
            IntegerSign::Signed
        } else {
            IntegerSign::Unsigned
        };
        IntegerType::new(sign, carrier.bit_width() as u16)
            .map_err(|_| "invalid integer carrier".into())
    }

    fn primitive(carrier: LandedIntegerType) -> Result<PrimitiveType, String> {
        match carrier {
            LandedIntegerType::Addr => {
                Err("address constant evaluation requires target authority".into())
            }
            carrier => Ok(PrimitiveType::from_landed_integer(carrier)),
        }
    }

    pub(crate) fn validate_closed_scalar_with_policy(
        program: &TypedTrees,
        expression: ExpressionHandle,
        destination: PrimitiveType,
        policy: ArithmeticDomain,
        calls: &dyn ConstantCalls,
    ) -> Result<Vec<Diagnostic>, String> {
        match_dispatch::validate_graph(program, expression)?;
        Ok(validate_shapes(
            program,
            EvaluationContext::Closed,
            expression,
            Some(ScalarDestination {
                primitive: destination,
                policy,
            }),
            Some(calls),
        )?
        .warnings)
    }

    pub(crate) fn evaluate_closed_scalar_with_policy(
        program: &TypedTrees,
        expression: ExpressionHandle,
        destination: PrimitiveType,
        policy: ArithmeticDomain,
        calls: &dyn ConstantCalls,
    ) -> Result<(CanonicalConstValue, Vec<Diagnostic>), String> {
        let (value, warnings) = evaluate_scalar_in(
            program,
            EvaluationContext::Closed,
            expression,
            destination,
            policy,
            Some(calls),
        )?;
        Ok((value.into_index()?, warnings))
    }

    pub(crate) fn evaluate_integer_endpoint(
        program: &TypedTrees,
        expression: ExpressionHandle,
        calls: &dyn ConstantCalls,
    ) -> Result<(IntegerLiteral, Vec<Diagnostic>), String> {
        use numerics::literals::{IntegerLanding, IntegerRadix};
        let (value, selected_arms, warnings) = evaluate_expression(
            program,
            EvaluationContext::Closed,
            expression,
            None,
            Some(calls),
        )?;
        let (integer, landing) = match value {
            Value::Anonymous(expression) => {
                let exact = evaluate_anonymous_numeric_expression(
                    program,
                    expression,
                    &selected_arms,
                    |_| true,
                )
                .and_then(|value| value.to_integer_exact())
                .ok_or("range endpoint needs a defined exact integer")?;
                (exact, None)
            }
            Value::Landed(carrier, value, policy) => {
                let integer = match value {
                    IntegerValue::Signed(value) => numerics::bignum::BigInt::from_i128(value),
                    IntegerValue::Unsigned(value) => numerics::bignum::BigInt::from_u128(value),
                };
                (
                    integer,
                    Some(IntegerLanding {
                        landed_type: carrier,
                        domain: policy,
                    }),
                )
            }
            Value::Boolean(_) | Value::Float(_, _) | Value::UndeterminedNaN(_) => {
                return Err(
                    "range endpoint position requires an exact builtin integer carrier".into(),
                );
            }
        };
        let mut literal = IntegerLiteral::from_parts(
            integer.is_negative(),
            IntegerRadix::Decimal,
            &integer.abs().to_string(),
        )
        .map_err(|error| error.to_string())?;
        if let Some(landing) = landing {
            literal = literal.with_landing(landing);
        }
        Ok((literal, warnings))
    }

    pub(crate) fn evaluate_closed_const_argument(
        program: &TypedTrees,
        expression: ExpressionHandle,
        destination: PrimitiveType,
        calls: &dyn ConstantCalls,
    ) -> Result<(CanonicalConstValue, Vec<Diagnostic>), String> {
        let (value, warnings) = evaluate_scalar_in(
            program,
            EvaluationContext::Closed,
            expression,
            destination,
            ArithmeticDomain::Exact,
            Some(calls),
        )?;
        match value {
            ScalarValue::Index(canonical) => Ok((canonical, warnings)),
            ScalarValue::Float { .. } | ScalarValue::UndeterminedNaN(_) => {
                Err("const application needs an exact integer or Boolean carrier".into())
            }
        }
    }
}

use crate::build_time_evaluation::{expression_table, type_reference_table};
use crate::unique::push_unique;
use arena::HandleSpan;
use diagnostics::Diagnostic;
use language_semantics::const_value::DecodedCanonicalConstValue;
use source::SourceSpan;
use syntax_trees::SyntaxTrees;
use syntax_trees::expression::{ExpressionHandle, ExpressionNode};
use syntax_trees::identifier::Identifier;
use syntax_trees::item::{Item, Machine, State};
use syntax_trees::statement::{StatementNode, TableTransition, TransitionTargetNode};
use syntax_trees::types::{ConstArgumentOrigin, TypeReferenceHandle, TypeReferenceNode};

pub(crate) fn evaluate(
    mut syntax: SyntaxTrees,
    context: &crate::BuildTimeSources<'_>,
) -> Result<SyntaxTrees, Vec<Diagnostic>> {
    let sources = context.sources.clone();
    let bindings = context.source_scoped_top_level_bindings;
    let authority = context.selection_authority.as_deref();
    let syntax_trees_to_symbol_resolved_trees::pre_resolution::ClosedConstArguments {
        data_arguments,
        extent_arguments,
        machine_arguments,
    } = syntax_trees_to_symbol_resolved_trees::pre_resolution::closed_const_arguments(
        &mut syntax,
        sources.clone(),
        bindings.to_vec(),
        context.retained_base(),
        context.declaration_syntax,
    )?;
    let mut lexical_arguments = Vec::new();
    let mut aggregate_arguments = Vec::new();
    let mut arguments = data_arguments
        .into_iter()
        .filter(|(argument, destination, _)| {
            matches!(
                syntax.type_references.type_reference(*argument),
                TypeReferenceNode::ConstExpression(_)
            ) && scalar_probe_route(&syntax, *destination)
        })
        .collect::<Vec<_>>();
    let extent_uses = extent_arguments
        .iter()
        .map(|(use_handle, ..)| *use_handle)
        .collect::<Vec<_>>();
    lexical_arguments.extend(extent_uses.iter().copied());
    arguments.extend(extent_arguments);
    for (argument, destination, public) in machine_arguments {
        let scalar = scalar_probe_route(&syntax, destination);
        let original = syntax.type_references.type_reference(argument).clone();
        if let Some(name) = syntax.type_references.type_reference(argument).as_named() {
            let name = name.clone();
            let reference = name.source_span();
            let mut members = HandleSpan::empty();
            for member in name.as_str(&syntax.tables.text).split("::") {
                syntax
                    .tables
                    .expressions
                    .append_identifier_path_member_to_span(
                        &mut members,
                        Identifier::new(member, reference),
                    );
            }
            let expression = syntax.expressions.insert(ExpressionNode::Name(members));
            syntax.expressions.set_source_span(expression, reference);
            syntax
                .type_references
                .replace_type_reference(argument, TypeReferenceNode::ConstExpression(expression));
        }
        if !scalar {
            aggregate_arguments.push((argument, original));
            continue;
        }
        arguments.push((argument, destination, public));
        lexical_arguments.push(argument);
    }
    let pending = arguments
        .iter()
        .filter_map(|(argument, destination, public)| {
            let TypeReferenceNode::ConstExpression(expression) =
                syntax.type_references.type_reference(*argument)
            else {
                return None;
            };
            if crate::const_evaluation::const_generic_calls::contains_call(&syntax, *expression) {
                return None;
            }
            Some((*argument, *expression, *destination, *public))
        })
        .collect::<Vec<_>>();
    let need_aggregate_resolution = aggregate_arguments.iter().any(|(argument, _)| {
        matches!(
            syntax.type_references.type_reference(*argument),
            TypeReferenceNode::ConstExpression(expression)
                if lexical_selection::needs_resolution(&syntax, *expression)
        )
    });
    let need_lexical_resolution = pending.iter().any(|(argument, expression, _, _)| {
        lexical_arguments.contains(argument)
            && lexical_selection::needs_resolution(&syntax, *expression)
    });
    let mut lexical_origins = Vec::new();
    if need_aggregate_resolution || need_lexical_resolution {
        let mut selection_syntax = syntax.clone();
        crate::syntax_probes::retain_pending_const_inputs(
            &mut selection_syntax,
            &arguments
                .iter()
                .map(|(argument, _, _)| *argument)
                .collect::<Vec<_>>(),
        );
        let resolved = syntax_trees_to_symbol_resolved_trees::pre_resolution::resolve_const_argument_selection(
            context.resolution_request(selection_syntax),
        )?;
        for (argument, original) in aggregate_arguments {
            if let TypeReferenceNode::ConstExpression(expression) =
                syntax.type_references.type_reference(argument)
                && lexical_selection::needs_resolution(&syntax, *expression)
            {
                lexical_selection::retain(&syntax, &resolved, *expression).map_err(|reason| {
                    vec![
                        Diagnostic::error(format!("const argument expression: {reason}"))
                            .with_source_span(syntax.expressions.source_span(*expression)),
                    ]
                })?;
            }
            syntax
                .type_references
                .replace_type_reference(argument, original);
        }
        for (argument, expression, _, _) in &pending {
            if lexical_arguments.contains(argument) {
                let origins = if lexical_selection::needs_resolution(&syntax, *expression) {
                    lexical_selection::retain(&syntax, &resolved, *expression).map_err(
                        |reason| {
                            vec![
                                Diagnostic::error(format!("const argument expression: {reason}"))
                                    .with_source_span(syntax.expressions.source_span(*expression)),
                            ]
                        },
                    )?
                } else {
                    Vec::new()
                };
                lexical_origins.push((*argument, origins));
            }
        }
    } else {
        for (argument, original) in aggregate_arguments {
            syntax
                .type_references
                .replace_type_reference(argument, original);
        }
        for (argument, _, _, _) in &pending {
            if lexical_arguments.contains(argument) {
                lexical_origins.push((*argument, Vec::new()));
            }
        }
    }
    if pending.is_empty() {
        return Ok(syntax);
    }

    let mut probe = syntax.clone();
    crate::syntax_probes::retain_pending_const_inputs(
        &mut probe,
        &arguments
            .iter()
            .map(|(argument, _, _)| *argument)
            .collect::<Vec<_>>(),
    );
    let declaration_frontier = probe.root_item_handles().len();
    let owners = pending
        .iter()
        .enumerate()
        .map(|(ordinal, (_, expression, destination, _))| {
            append_probe(&mut probe, ordinal, *expression, *destination)
        })
        .collect::<Vec<_>>();
    let declarations = probe.root_item_handles()[declaration_frontier..].to_vec();
    let probe = crate::syntax_probes::normalize_initializer_probe_data(
        probe,
        sources.clone(),
        bindings,
        context.retained_base(),
        [
            context.declaration_syntax,
            context.retained_declaration_syntax,
        ],
        &declarations,
    )?;
    let frontier = resolve_deferred_initializer_probe(context.resolution_request(probe))?;
    let owner_symbols = owners
        .iter()
        .map(|owner| owner.resolve(frontier.trees()))
        .collect::<Result<Vec<_>, _>>()?;
    let typed =
        prepare_call_free_probe_typing(frontier, &owner_symbols, context.predecessor.clone())?;

    for ((argument, original, _, public), owner) in pending.into_iter().zip(owner_symbols) {
        let reference = syntax.expressions.source_span(original);
        let failure = |reason| {
            vec![
                Diagnostic::error(format!("const argument expression: {reason}"))
                    .with_source_span(reference),
            ]
        };
        let expected = lexical_origins
            .iter()
            .find(|(original, _)| *original == argument)
            .map(|(_, origins)| origins.as_slice());
        let ScalarProbeResult {
            value: result,
            origins,
            operators,
            warnings,
            ..
        } = evaluate_probe_internal(
            &typed,
            reference,
            public,
            authority,
            expected,
            &syntax,
            None,
            Some(owner),
            extent_uses.contains(&argument),
        )
        .map_err(&failure)?;
        let result = result.into_index().map_err(&failure)?;
        let replacement = match result.decode_encoding() {
            Some(DecodedCanonicalConstValue::Integer { value, .. }) => value.to_string(),
            Some(DecodedCanonicalConstValue::Boolean(_)) => result.atom(),
            _ => {
                return Err(failure(
                    "index probe returned an unsupported canonical value".to_owned(),
                ));
            }
        };
        crate::report_evaluation_diagnostics(&warnings)?;
        syntax.type_references.retain_const_argument_normalization(
            argument,
            reference,
            result.encoding,
            origins,
            operators,
        );
        syntax.type_references.replace_type_reference(
            argument,
            TypeReferenceNode::Named(Identifier::generated(replacement)),
        );
    }
    Ok(syntax)
}

fn scalar_probe_route(syntax: &SyntaxTrees, destination: TypeReferenceHandle) -> bool {
    matches!(
        syntax.type_references.type_reference(destination),
        TypeReferenceNode::Named(name)
            if matches!(name.as_str(syntax),
                "i8" | "i16" | "i32" | "i64" | "u8" | "u16" | "u32" | "u64" | "bool")
    )
}

pub(crate) fn exact_probe_destination(
    program: &typed_trees::TypedTrees,
    destination: typed_trees::types::TypeReferenceHandle,
) -> Option<typed_trees::types::PrimitiveType> {
    use symbols::BuiltinTypeAtom;
    use typed_trees::types::PrimitiveType;
    let symbol = &type_reference_table(program)
        .type_reference(destination)
        .as_named_symbol()?;
    Some(match program.symbols.builtin_type_atom(*symbol)? {
        BuiltinTypeAtom::I8 => PrimitiveType::I8,
        BuiltinTypeAtom::I16 => PrimitiveType::I16,
        BuiltinTypeAtom::I32 => PrimitiveType::I32,
        BuiltinTypeAtom::I64 => PrimitiveType::I64,
        BuiltinTypeAtom::U8 => PrimitiveType::U8,
        BuiltinTypeAtom::U16 => PrimitiveType::U16,
        BuiltinTypeAtom::U32 => PrimitiveType::U32,
        BuiltinTypeAtom::U64 => PrimitiveType::U64,
        BuiltinTypeAtom::Bool => PrimitiveType::Bool,
        _ => return None,
    })
}

pub(crate) fn extent_integer_destination(
    program: &typed_trees::TypedTrees,
    result: typed_trees::types::TypeReferenceHandle,
) -> Option<(typed_trees::types::PrimitiveType, ArithmeticDomain)> {
    let result = validation::arithmetic_result_type_reference(program, result)?;
    let primitive = program.type_reference_table.primitive_type(result)?;
    if !matches!(
        primitive,
        typed_trees::types::PrimitiveType::I8
            | typed_trees::types::PrimitiveType::I16
            | typed_trees::types::PrimitiveType::I32
            | typed_trees::types::PrimitiveType::I64
            | typed_trees::types::PrimitiveType::U8
            | typed_trees::types::PrimitiveType::U16
            | typed_trees::types::PrimitiveType::U32
            | typed_trees::types::PrimitiveType::U64
    ) {
        return None;
    }
    Some((
        primitive,
        program.type_reference_table.arithmetic_domain(result),
    ))
}

pub(crate) fn scalar_probe_destination(
    program: &typed_trees::TypedTrees,
    destination: typed_trees::types::TypeReferenceHandle,
) -> Option<typed_trees::types::PrimitiveType> {
    use symbols::BuiltinTypeAtom;
    use typed_trees::types::PrimitiveType;
    exact_probe_destination(program, destination).or_else(|| {
        let symbol = &type_reference_table(program)
            .type_reference(destination)
            .as_named_symbol()?;
        match program.symbols.builtin_type_atom(*symbol)? {
            BuiltinTypeAtom::F32 => Some(PrimitiveType::F32),
            BuiltinTypeAtom::F64 => Some(PrimitiveType::F64),
            _ => None,
        }
    })
}

pub(crate) fn append_probe(
    syntax: &mut SyntaxTrees,
    ordinal: usize,
    expression: ExpressionHandle,
    destination: TypeReferenceHandle,
) -> ProbeOwner {
    let reference = syntax.expressions.source_span(expression);
    append_named_probe(
        syntax,
        Identifier::new(format!("@const-argument-{ordinal}"), reference),
        expression,
        destination,
    )
}

pub(crate) fn append_extent_probe(
    syntax: &mut SyntaxTrees,
    use_handle: TypeReferenceHandle,
    expression: ExpressionHandle,
    destination: TypeReferenceHandle,
    public: bool,
) -> Identifier {
    let reference = syntax.expressions.source_span(expression);
    let name = Identifier::new(
        format!(
            "@array-extent-{}-{}",
            use_handle.generation(),
            use_handle.arena_index()
        ),
        reference,
    );
    append_named_probe(syntax, name.clone(), expression, destination);
    syntax
        .roots
        .constant_extent_probes
        .push((expression, public));
    name
}

pub(crate) fn append_named_probe(
    syntax: &mut SyntaxTrees,
    name: Identifier,
    expression: ExpressionHandle,
    destination: TypeReferenceHandle,
) -> ProbeOwner {
    let reference = syntax.expressions.source_span(expression);
    let target = syntax
        .statements
        .insert_transition_target(TransitionTargetNode::Value(expression));
    let statement = syntax
        .statements
        .insert(StatementNode::Transition(TableTransition {
            target,
            source_span: reference,
            ..Default::default()
        }));
    let statement = syntax.items.append_statement_handle(statement);
    let state = syntax.items.insert_state(&State {
        name: name.clone(),
        parameters: HandleSpan::empty(),
        return_type: destination,
        contracts: HandleSpan::empty(),
        statements: HandleSpan::from_parts(statement, 1),
    });
    let state = syntax.items.append_state_handle(state);
    syntax.push_root_item(Item::Machine(Machine {
        nested_machines: HandleSpan::empty(),
        name: name.clone(),
        generic_data_template: Default::default(),
        default_body_trait: Default::default(),
        attached_data: None,
        spelling: None,
        is_public: false,
        bodyless: false,
        target_family_interface: false,
        target: None,
        authored_target: None,
        boundary: false,
        lifetime_parameters: Vec::new(),
        type_parameters: HandleSpan::empty(),
        satisfies: HandleSpan::empty(),
        conformance_bounds: Vec::new(),
        terminates_guarantee: false,
        ranking_subjects: HandleSpan::empty(),
        ranking_view: HandleSpan::empty(),
        ranking_view_arguments: HandleSpan::empty(),
        ranking_range: ExpressionHandle::invalid(),
        service_reach_keyword_source_spans: Vec::new(),
        service_reaches: HandleSpan::empty(),
        service_reach_is_installation_bound: false,
        invokes: HandleSpan::empty(),
        suspends_keyword_source_spans: Vec::new(),
        blocks_keyword_source_spans: Vec::new(),
        suspends: false,
        blocks: false,
        admitted_same_stack_bytes: 0,
        declares_no_crash_route: false,
        contracts: HandleSpan::empty(),
        states: HandleSpan::from_parts(state, 1),
    }));
    ProbeOwner {
        source_span: name.source_span(),
        spelling: name.into_string(syntax),
    }
}

pub(crate) struct ProbeOwner {
    spelling: String,
    source_span: SourceSpan,
}

impl ProbeOwner {
    pub(crate) fn resolve(
        &self,
        resolved: &symbol_resolved_trees::SymbolResolvedTrees,
    ) -> Result<SymbolHandle, Vec<Diagnostic>> {
        let mut owners = resolved.machines.iter().filter(|machine| {
            resolved.symbols.name(machine.symbol) == self.spelling
                && resolved.symbols.symbol_source_span(machine.symbol) == Some(self.source_span)
        });
        match (owners.next(), owners.next()) {
            (Some(owner), None) => Ok(owner.symbol),
            _ => Err(vec![
                Diagnostic::error("evaluation probe lost its exact private declaration")
                    .with_source_span(self.source_span),
            ]),
        }
    }
}

fn has_builtin_operator_selection(
    program: &typed_trees::TypedTrees,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    expression: typed_trees::expression::ExpressionHandle,
    target: language_semantics::declaration_selection::AuthoredDeclarationSelectionTarget,
) -> bool {
    use language_semantics::declaration_selection::{
        AuthoredDeclarationSelectionIntrinsic as Intrinsic,
        AuthoredDeclarationSelectionLateBinding as Binding,
        AuthoredDeclarationSelectionTarget as Target,
    };
    use typed_trees::expression::ExpressionNode;
    matches!(target, Target::Intrinsic(Intrinsic::BuiltinOperator))
        || match program.expression_table.expression(expression) {
            ExpressionNode::Unary(_) => {
                matches!(target, Target::LateBound(Binding::CheckedOperator))
            }
            ExpressionNode::Binary(_) => {
                has_builtin_binary_expression_meaning(program, machine, Some(state), expression)
            }
            _ => false,
        }
}

pub(crate) fn expression_custody(
    program: &typed_trees::TypedTrees,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    root: typed_trees::expression::ExpressionHandle,
    public: bool,
    syntax: &SyntaxTrees,
    admitted_calls: &std::collections::HashSet<typed_trees::expression::ExpressionHandle>,
) -> Result<(Vec<ConstArgumentOrigin>, Vec<SourceSpan>), String> {
    use language_semantics::declaration_selection::{
        AuthoredDeclarationSelectionKind as Kind, AuthoredDeclarationSelectionTarget as Target,
    };
    use typed_trees::expression::ExpressionNode;
    let mut origins = Vec::new();
    let mut operators = Vec::new();
    let mut pending = vec![root];
    let mut visited = Vec::new();
    while let Some(expression) = crate::worklist::unvisited(&mut pending, &mut visited) {
        for occurrence in expression_table(program).authored_selection_occurrences(expression) {
            let selection = program
                .authored_declaration_selections()
                .get(occurrence)
                .ok_or("evaluated expression lost an authored selection")?;
            if selection.kind() == Kind::Call {
                if !admitted_calls.contains(&expression)
                    && !inherits_normalized_constant_call(program, expression, occurrence)
                {
                    return Err("call-free index expression has no owning normalized constant for its call receipt".into());
                }
                continue;
            }
            if selection.kind() == Kind::Operator {
                let builtin = has_builtin_operator_selection(
                    program,
                    machine,
                    state,
                    expression,
                    selection.target(),
                );
                if !builtin {
                    return Err("evaluated operator has no checked builtin meaning".to_owned());
                }
                if !operators.contains(&selection.source_span()) {
                    operators.push(selection.source_span());
                }
                continue;
            }
            if matches!(
                selection.kind(),
                Kind::TypeReference
                    | Kind::MemberAccess
                    | Kind::StructLiteralType
                    | Kind::StructLiteralCase
                    | Kind::StructLiteralField
                    | Kind::CaseReference
                    | Kind::CaseMembership
                    | Kind::DomainMembership
                    | Kind::DomainIssuerAuthorization
                    | Kind::DomainCleanupSelection
                    | Kind::StaticArgument
                    | Kind::Conformance
            ) {
                continue;
            }
            let Target::Resolved(selected) = selection.target() else {
                return Err(
                    "evaluated constant reference has no exact declaration selection".to_owned(),
                );
            };
            let declaration = program
                .const_declaration_by_symbol(selected.selected_symbol())
                .ok_or("evaluated reference did not select a constant declaration")?;
            let declaration_source = program
                .symbols
                .symbol_source_span(declaration.symbol)
                .ok_or("selected constant lost its declaration source")?;
            if !declaration.is_public
                && ((public
                    && !syntax.constant_initializer_owns_selection(selection.source_span()))
                    || !program
                        .symbols
                        .same_source_package(selection.source_span(), declaration_source))
            {
                return Err(
                    "private constant declaration cannot be selected by this index owner"
                        .to_owned(),
                );
            }
            let origin = ConstArgumentOrigin {
                reference: selection.source_span(),
                declaration: declaration_source,
                initializer: declaration.initializer_source_span,
                evaluated_value: declaration
                    .evaluated_value
                    .clone()
                    .ok_or("selected constant has no canonical value")?,
            };
            push_unique(&mut origins, origin);
        }
        if let Some(binary) = program.expression_table.binary_node(expression) {
            pending.push(binary.right);
            pending.push(binary.left);
        } else if let ExpressionNode::Unary(unary) = program.expression_table.expression(expression)
        {
            pending.push(unary.operand);
        } else if let ExpressionNode::Cast(cast) = program.expression_table.expression(expression) {
            pending.push(cast.value);
        } else if let ExpressionNode::Member(member) =
            program.expression_table.expression(expression)
        {
            pending.push(member.receiver);
        } else if let ExpressionNode::Match(dispatch) =
            program.expression_table.expression(expression)
        {
            pending.extend(program.expression_table.match_pending(dispatch));
        }
    }
    Ok((origins, operators))
}

fn inherits_normalized_constant_call(
    program: &typed_trees::TypedTrees,
    expression: typed_trees::expression::ExpressionHandle,
    call_occurrence: typed_trees::AuthoredDeclarationSelectionOccurrenceId,
) -> bool {
    use language_semantics::declaration_selection::AuthoredDeclarationSelectionTarget as Target;
    if matches!(
        program.expression_table.expression(expression),
        typed_trees::expression::ExpressionNode::Call(_)
    ) {
        return false;
    }
    expression_table(program)
        .authored_selection_occurrences(expression)
        .any(|occurrence| {
            let Some(selection) = program.authored_declaration_selections().get(occurrence) else {
                return false;
            };
            let Target::Resolved(selected) = selection.target() else {
                return false;
            };
            let mut declarations = program
                .const_declarations()
                .iter()
                .filter(|declaration| declaration.symbol == selected.selected_symbol());
            let Some(declaration) = declarations.next() else {
                return false;
            };
            declarations.next().is_none()
                && declaration.evaluated_value.is_some()
                && expression_table(program).expression_is_valid(declaration.authored_initializer)
                && expression_table(program)
                    .expression_is_valid(declaration.materialized_initializer)
                && expression_table(program)
                    .authored_selection_occurrences(declaration.materialized_initializer)
                    .any(|owned| owned == call_occurrence)
        })
}

pub(crate) fn immediate_call_expressions(
    program: &typed_trees::TypedTrees,
    roots: &[typed_trees::expression::ExpressionHandle],
) -> Result<Vec<typed_trees::expression::ExpressionHandle>, String> {
    call_custody::immediate_call_expressions(program, roots)
}

pub(crate) fn pending_leaf_call_custody(
    program: &typed_trees::TypedTrees,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    roots: &[typed_trees::expression::ExpressionHandle],
    public: bool,
    syntax: &SyntaxTrees,
) -> Result<
    (
        Vec<ConstArgumentOrigin>,
        Vec<SourceSpan>,
        Vec<(SourceSpan, SourceSpan)>,
    ),
    String,
> {
    let custody =
        call_custody::collect_pending_roots(program, machine, state, roots, public, syntax)?;
    Ok((custody.origins, custody.operators, custody.calls))
}

pub(crate) fn leaf_call_custody(
    program: &typed_trees::TypedTrees,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    roots: &[typed_trees::expression::ExpressionHandle],
    public: bool,
    syntax: &SyntaxTrees,
) -> Result<
    (
        Vec<ConstArgumentOrigin>,
        Vec<SourceSpan>,
        Vec<(SourceSpan, SourceSpan)>,
    ),
    String,
> {
    let custody = call_custody::collect_roots(program, machine, state, roots, public, syntax)?;
    Ok((custody.origins, custody.operators, custody.calls))
}

pub(crate) struct AuthoredProbeCustody {
    reference: SourceSpan,
    owner: SymbolHandle,
}

impl AuthoredProbeCustody {
    pub(crate) fn rejoin(&self, reference: SourceSpan, owner: SymbolHandle) -> Result<(), String> {
        if self.reference != reference || self.owner != owner {
            return Err(
                "realized initializer differs from its checked authored probe owner".into(),
            );
        }
        Ok(())
    }
}

pub(crate) struct AuthoredProbeObservation {
    reference: SourceSpan,
    owner: SymbolHandle,
    custody: call_custody::Custody,
}

impl AuthoredProbeObservation {
    pub(crate) fn calls(&self) -> &[(SourceSpan, SourceSpan)] {
        &self.custody.calls
    }

    pub(crate) fn validate(
        self,
        expected_origins: &[ConstArgumentOrigin],
        expected_calls: &[(SourceSpan, SourceSpan)],
    ) -> Result<AuthoredProbeCustody, String> {
        if !self.custody.pending_constants.is_empty() {
            return Err("authored probe constants are not ready for invocation custody".into());
        }
        if self.custody.calls.len() != expected_calls.len()
            || self
                .custody
                .calls
                .iter()
                .any(|call| !expected_calls.contains(call))
        {
            return Err(format!(
                "authored probe changed the original call declaration selection: expected {expected_calls:?}, observed {:?}",
                self.custody.calls,
            ));
        }
        if self.custody.origins.len() != expected_origins.len()
            || self
                .custody
                .origins
                .iter()
                .any(|origin| !expected_origins.contains(origin))
        {
            return Err(
                "authored probe changed the original constant declaration selection".into(),
            );
        }
        Ok(AuthoredProbeCustody {
            reference: self.reference,
            owner: self.owner,
        })
    }
}

pub(crate) fn observe_authored_probe(
    program: &typed_trees::TypedTrees,
    reference: SourceSpan,
    syntax: &SyntaxTrees,
) -> Result<AuthoredProbeObservation, String> {
    let mut machines = program
        .machines()
        .iter()
        .filter(|machine| program.symbols.symbol_source_span(machine.symbol) == Some(reference));
    let machine = machines
        .next()
        .ok_or("authored initializer probe lost its source owner")?;
    if machines.next().is_some() {
        return Err("authored initializer probe has ambiguous source owners".into());
    }
    let [state] = program.machine_states(machine) else {
        return Err("authored initializer probe lost its single state".into());
    };
    let body = crate::const_evaluation::initializer_probe::value_probe_body(program, state)?;
    if body.reference != reference {
        return Err("authored initializer probe changed its value source point".into());
    }
    let custody =
        call_custody::collect_authored_roots(program, machine, state, &body.roots, syntax)?;
    Ok(AuthoredProbeObservation {
        reference,
        owner: machine.symbol,
        custody,
    })
}

pub(crate) fn initializer_contains_call(
    program: &typed_trees::TypedTrees,
    root: typed_trees::expression::ExpressionHandle,
) -> Result<bool, String> {
    call_custody::contains_call(program, root)
}

pub(crate) fn validate_retained_initializer_call_custody(
    program: &typed_trees::TypedTrees,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    declaration: &typed_trees::constant::ConstDeclaration,
) -> Result<Vec<DependencyValue>, String> {
    call_custody::validate_retained(program, machine, state, declaration)
}

pub(crate) fn validate_authored_initializer_call_custody(
    program: &typed_trees::TypedTrees,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    declaration: &typed_trees::constant::ConstDeclaration,
) -> Result<Vec<DependencyValue>, String> {
    call_custody::validate_authored(program, machine, state, declaration)
}

pub(crate) fn evaluate_probe(
    typed: &typed_trees::TypedTrees,
    reference: SourceSpan,
    public: bool,
    authority: Option<&dyn crate::BuildTimeSelectionAuthority>,
    expected_origins: Option<&[ConstArgumentOrigin]>,
    syntax: &SyntaxTrees,
) -> Result<ScalarProbeResult, String> {
    evaluate_probe_internal(
        typed,
        reference,
        public,
        authority,
        expected_origins,
        syntax,
        None,
        None,
        false,
    )
}

pub(crate) fn evaluate_probe_with_calls(
    typed: &typed_trees::TypedTrees,
    reference: SourceSpan,
    public: bool,
    authority: Option<&dyn crate::BuildTimeSelectionAuthority>,
    authored: &AuthoredProbeCustody,
    syntax: &SyntaxTrees,
    calls: &dyn value::ConstantCalls,
) -> Result<ScalarProbeResult, String> {
    let mut owners = typed
        .machines()
        .iter()
        .filter(|machine| typed.symbols.symbol_source_span(machine.symbol) == Some(reference));
    let owner = owners
        .next()
        .ok_or("realized initializer lost its source probe")?;
    if owners.next().is_some() {
        return Err("realized initializer has ambiguous source probes".into());
    }
    authored.rejoin(reference, owner.symbol)?;
    evaluate_probe_internal(
        typed,
        reference,
        public,
        authority,
        None,
        syntax,
        Some(calls),
        None,
        false,
    )
}

fn evaluate_probe_internal(
    typed: &typed_trees::TypedTrees,
    reference: SourceSpan,
    public: bool,
    authority: Option<&dyn crate::BuildTimeSelectionAuthority>,
    expected_origins: Option<&[ConstArgumentOrigin]>,
    syntax: &SyntaxTrees,
    calls: Option<&dyn value::ConstantCalls>,
    owner: Option<SymbolHandle>,
    extent: bool,
) -> Result<ScalarProbeResult, String> {
    let mut machines = typed.machines().iter().filter(|machine| {
        owner.map_or_else(
            || typed.symbols.symbol_source_span(machine.symbol) == Some(reference),
            |owner| machine.symbol == owner,
        )
    });
    let machine = machines
        .next()
        .ok_or("typed probe lost its source custody")?;
    if machines.next().is_some() {
        return Err("typed probe source custody is ambiguous".to_owned());
    }
    let [state] = typed.machine_states(machine) else {
        return Err("typed probe lost its single expression state".to_owned());
    };
    let [typed_trees::statement::StatementNode::Transition(transition)] =
        typed.statement_table.statements(state.statement_nodes)
    else {
        return Err("typed probe lost its expression return".to_owned());
    };
    let typed_trees::statement::TransitionTargetNode::Value(expression) =
        typed.statement_table.transition_target(transition.target)
    else {
        return Err("typed probe return is not a value".to_owned());
    };
    let result_type = extent
        .then(|| validation::expression_result_type_reference(typed, machine, state, *expression))
        .flatten();
    let (destination, policy) = if extent {
        if let Some(result) = result_type {
            extent_integer_destination(typed, result)
                .ok_or("array extent requires a supported integer carrier and policy")?
        } else {
            (
                exact_probe_destination(typed, state.return_type)
                    .ok_or("array extent has no integer landing carrier")?,
                ArithmeticDomain::Exact,
            )
        }
    } else {
        (
            scalar_probe_destination(typed, state.return_type)
                .ok_or("constant destination requires an unconstrained builtin scalar carrier")?,
            ArithmeticDomain::Exact,
        )
    };
    require_const_expression_selection(typed, machine, reference, authority)?;
    let (origins, operators, call_selections) = if calls.is_some() {
        let custody = call_custody::collect(typed, machine, state, *expression, public, syntax)?;
        (custody.origins, custody.operators, custody.calls)
    } else {
        let (origins, operators) = expression_custody(
            typed,
            machine,
            state,
            *expression,
            public,
            syntax,
            &std::collections::HashSet::new(),
        )?;
        (origins, operators, Vec::new())
    };
    if expected_origins.is_some_and(|expected| {
        origins.len() != expected.len() || origins.iter().any(|origin| !expected.contains(origin))
    }) {
        return Err(
            "standalone probe changed the original constant selection or selected a pending value"
                .to_owned(),
        );
    }
    let (value, warnings) = value::evaluate_scalar_with_policy(
        typed,
        machine,
        state,
        *expression,
        destination,
        policy,
        calls,
    )
    .map_err(|reason| {
        if extent {
            format!(
                "fixed-array length must evaluate to a nonnegative integer count of `{}`: {reason}",
                destination.name()
            )
        } else {
            reason
        }
    })?;
    if value.type_name() != destination.name() {
        return Err(format!(
            "landed `{}` result cannot initialize `{}`",
            value.type_name(),
            destination.name(),
        ));
    }
    Ok(ScalarProbeResult {
        value,
        origins,
        operators,
        call_selections,
        warnings,
    })
}

pub(crate) struct ScalarProbeResult {
    pub(crate) value: value::ScalarValue,
    pub(crate) origins: Vec<ConstArgumentOrigin>,
    pub(crate) operators: Vec<SourceSpan>,
    pub(crate) call_selections: Vec<(SourceSpan, SourceSpan)>,
    pub(crate) warnings: Vec<Diagnostic>,
}
