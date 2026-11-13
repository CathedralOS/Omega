/*
    Seams the product coordinator reaches after checking: `package_review`
    rederivation queries, `product_pruning` of the checked tree to a selected
    product, and `quotient_eligibility` finished from the same invocation's
    crash facts.
*/

pub(crate) mod package_review {
    /*
        Compiler-internal package-review seams: rederivation queries that let the
        orchestration layer reject drifted retained facts without trusting them.
        These are review joins over the private subsystems, not a public IR
        contract; handles and private coordinates never enter review identity.
    */
    use checked_trees::{CheckFacts, CheckedSemanticDependencies};

    use crate::published::facts;
    use crate::shared::authored_selections;
    use crate::{checks, flow};

    /*
        Conservative pre-check classification used by compiler-run semantic
        evaluation. `true` means the typed expression cannot select an authored
        operator declaration; final checking still validates builtin semantics.
    */
    pub fn typed_operator_has_no_authored_selection(
        program: &dyn ::validation::CheckingInput,
        expression: typed_trees::expression::ExpressionHandle,
    ) -> bool {
        authored_selections::typed_operator_has_no_authored_selection(program, expression)
    }

    /*
        Exact declaration selected by one late-bound member access whose typed
        `member_symbol` is still invalid, derived from the receiver's owner type
        exactly as checked binding derives it. Build-time authority confines that
        declaration's package instead of every same-spelled member in the program.
    */
    pub fn late_bound_member_declaration_from_exact_owner(
        program: &dyn ::validation::CheckingInput,
        expression: typed_trees::expression::ExpressionHandle,
    ) -> Option<symbols::SymbolHandle> {
        authored_selections::exact_owner_member_declaration(program, expression)
    }

    /*
        Conservative declaration candidates for an operator before checked
        selection is final. Build-time authority uses this set only to prove that
        every possible authored meaning is already within the package's admitted
        source graph; ordinary checked lowering still chooses the exact meaning.
    */
    pub fn typed_operator_authored_selection_candidates(
        program: &dyn ::validation::CheckingInput,
        expression: typed_trees::expression::ExpressionHandle,
    ) -> Vec<symbols::SymbolHandle> {
        authored_selections::typed_operator_authored_selection_candidates(program, expression)
    }

    /*
        Rederive the complete, canonically ordered checked semantic-dependency
        table from the final typed program and its checked facts.

        This is the compiler-internal package-review entry point. It does not trust
        or consult the semantic-dependency table already retained in `facts`.
    */
    pub fn derive_checked_semantic_dependencies(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
    ) -> CheckedSemanticDependencies {
        flow::derive_checked_semantic_dependencies(program, facts)
    }

    /*
        Rederive canonical authored crash buckets for an exact typed machine.
        Scalar lowering metadata and private sites are deliberately outside this
        read-only package-policy join; canonical predicate equality is unchanged.
    */
    pub fn derive_authored_machine_crash_buckets(
        program: &dyn ::validation::CheckingInput,
        machine: &typed_trees::machine::Machine,
    ) -> Result<Vec<checked_trees::CrashRouteBucket>, diagnostics::Diagnostic> {
        facts::derive_authored_machine_crash_buckets(program, machine, None)
    }

    /*
        Exact authored crash identity for a retained structural signature, using
        the same canonical owner as its checked contract capsule.
    */
    pub fn derive_authored_signature_crash_buckets(
        program: &dyn ::validation::CheckingInput,
        signature: &typed_trees::signature::StateSignature,
    ) -> Result<Vec<checked_trees::CrashRouteBucket>, diagnostics::Diagnostic> {
        facts::derive_authored_signature_crash_buckets(program, signature, None)
    }

    /*
        Query conservative causes from the checker's closed local crash summary.
        `None` means no complete summary, not a crash-free machine. This read-only
        query exposes no private guard, site, call, or proof coordinates.
    */
    pub fn infer_checked_machine_crash_causes(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        machine: symbols::SymbolHandle,
    ) -> Option<Vec<checked_trees::CrashCause>> {
        facts::infer_checked_machine_crash_causes(program, facts, machine)
    }

    /*
        Read all complete local crash summaries from ordinary checking.
        Missing machine keys mean unknown, not complete empty summaries. Each row
        retains only its exact machine key and conservative canonical cause set.
    */
    pub fn infer_checked_crash_causes(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
    ) -> Vec<(symbols::SymbolHandle, Vec<checked_trees::CrashCause>)> {
        facts::infer_checked_crash_causes(program, facts)
    }

    /*
        Rederive one nominal call target from the final typed program and the exact
        checked owner environments that contain the expression. Package review uses
        this compiler-internal seam for proof-owned attached and path-qualified
        calls whose typed node intentionally carries no direct target symbol.
    */
    pub fn derive_checked_nominal_call_target(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        expression: typed_trees::expression::ExpressionHandle,
        owner_index: Option<&validation::expression_owners::OwnerEnvironmentIndex>,
    ) -> Option<symbols::SymbolHandle> {
        authored_selections::derive_checked_nominal_call_target(
            program,
            facts,
            expression,
            owner_index,
        )
    }

    /*
        Rederive the erased requires lane for one exact proof-expression call
        target from checked contract facts. This is a compiler-internal package
        review seam; handles remain private joins and never enter review identity.
    */
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

    pub fn derive_checked_runtime_contract_expression_evidence_parameters(
        facts: &CheckFacts,
        contract: typed_trees::callable_values::RuntimeCallableContractHandle,
    ) -> Vec<arena::Handle<checked_trees::CheckedEvidenceTerm>> {
        checks::contracts::exact_runtime_evidence_parameters(facts, contract)
    }

    /*
        Freshly instantiate one erased requires proposition against the current
        typed call arguments. Package review uses this after checking to reject a
        coordinated typed-tree edit paired with stale checked evidence custody.
    */
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

    /*
        Derive the checked body-local termination summary for one typed machine.

        Constant and plan positions must run before checked lowering because their
        values refine the typed program. This exposes the checker's pure judgment
        for those admission sites while keeping the proof implementation single-
        sourced with the facts produced by [`crate::lower_typed_trees`].
    */
    pub fn infer_machine_termination_summary(
        program: &dyn ::validation::CheckingInput,
        machine_symbol: symbols::SymbolHandle,
    ) -> Option<language_semantics::TerminationGuarantee> {
        let machine = program.machine_by_symbol(machine_symbol)?;
        Some(checks::termination::infer_machine_checked_summary(
            program, machine, None,
        ))
    }

    /*
        Require a termination proof for the complete call cycle containing this
        body. Preserve the ordinary checker's rejection reasons. Checking one body
        without its cycle cannot prove termination or grant permission to execute.
    */
    pub fn infer_checked_recursive_machine_terminations(
        checked: &checked_trees::CheckedTrees,
        machine: symbols::SymbolHandle,
        body: arena::HandleSpan<typed_trees::state::State>,
    ) -> Result<Vec<checked_trees::CheckedRuntimeRankMember>, Vec<diagnostics::Diagnostic>> {
        checks::termination::infer_checked_recursive_machine_terminations(checked, machine, body)
    }
}
pub(crate) mod product_pruning {
    /*
        Checked-trees optimization phase: exact opt-in whole-product root
        selection and unreachable machine-declaration pruning.

        # Phase contract

        This module is the Psi-side execution seam for the checked-tree
        optimization phase (`OptimizationExecutionPhase::CheckedTrees`). It runs
        strictly AFTER authored checking: the input is a complete, diagnostic-
        cleared [`CheckedTrees`], so pruning can never hide an invalid authored
        declaration -- checking, diagnostics, and all fact construction have
        already committed before this phase sees the program.

        The opt-in plan is [`CheckedTreeProductPruning`]: an exact, explicit set of
        product root machines supplied by the coordinator. Nothing is enabled by
        default, by an optimization level, or by inference; an absent plan is an
        identity execution with no second pipeline route. Each execution returns a
        [`CheckedTreeProductSelection`] evidence record carrying the exact roots,
        the retained/pruned machine rosters in declaration order, and a
        domain-separated SHA-256 identity, so every target/root-selected Psi
        product is independently reproducible and joins to its source selection.

        # Retention bound

        Only `MachineSupplyMode::CheckedBody` machines are pruning candidates.
        Requirement slots, top-level requirements, boundary declarations,
        admission claims, and external realizations are product interface surface:
        Omega-side provider selection, callback admission, and trust reporting
        still consume them, so they are always retained.

        Retention is the transitive closure of the product roots over every
        machine edge the checked representation enumerates:

        - checked call facts (`facts.flow.control`) and the typed statement/expression
          call nodes they join to, including `machine_arguments`,
          `static_requirement_dispatch`, quotient-theorem, and private-layout
          applications;
        - authored `invokes` service targets and `reaches` rows;
        - `satisfies` `via` expressions, conformance-bound selections, owned-data
          initializers, signature/contract proof facts, and const-expression type
          positions reachable from each machine;
        - contract evidence calls, static conformance applications, proof
          recursive-component edges, suspension-crossing targets, specialization
          machine arguments, and
          `AutomaticCleanupMachine`/machine-valued semantic dependencies;
        - conservative whole-surface retention for provider-selected identities a
          root cannot bound: closed-conformance realization machines, boundary
          calling plans, nominal machine-use selections, fact-call-projection
          targets, boundary adapter realizations, access-view policy machines,
          open-index providers, fused service erasures, and every machine
          referenced by a retained non-machine declaration (data, trait,
          conformance, proposition, measure, const, operator, domain, wire).

        # What pruning changes

        Pruning rebuilds the machine declaration arena and every machine-keyed
        `Vec` table in the typed sidecars and `CheckFacts`/`FlowFacts` terminal
        plan families, plus the machine-keyed roster arenas
        (`flow.control.states`, `service_reaches.machines`) that no `Handle`/`HandleSpan` references into.
        Arena-resident rows keyed by non-machine identities (expressions, state
        fact handles, conformances, evidence terms, span-referenced fact rows)
        remain as retained dead evidence: they are valid checked rows that are
        unreachable through the pruned declaration surface, and compacting them
        would remap handles owned by representations outside this crate.

        The retained set is the closure of the edge relation, so no retained
        machine references a pruned one by construction; the phase does not
        re-extract the edges of its own output to confirm it.
    */
    use std::collections::HashSet;
    use std::sync::Arc;

    use checked_trees::CheckedTrees;
    use diagnostics::Diagnostic;
    use symbols::SymbolHandle;
    use typed_trees::TypedTrees;

    pub(crate) mod dependencies {
        /*
            Exact machine dependency extraction and conservative interface retention.
        */
        use std::collections::{HashMap, HashSet};
        use typed_trees::references::TypedReferences;
        use typed_trees::references::visit_closed_conformance;
        use typed_trees::references::visit_declarations;
        use typed_trees::references::visit_machine;
        use typed_trees::references::visit_static_argument;
        use typed_trees::references::{visit_expression, visit_signature};
        use typed_trees::typed_trees::AccessLifecyclePlan;
        use typed_trees::typed_trees::ClosedConformanceApplication;

        use checked_trees::{CheckFacts, ContractProofFactOwner, ProofFacts};
        use symbols::SymbolHandle;
        use typed_trees::{expression::StaticMachineArgument, statement::StatementHandle};

        /*
            Exact declaration indices used to resolve machine references and to key
            machine-owner lookups without scanning the declaration table.
        */
        pub(super) struct MachineIndex {
            /* Membership of every declared machine symbol; order comes from the program. */
            pub(super) machine_symbols: HashSet<SymbolHandle>,
            /* Owning machine for every machine state symbol. */
            pub(super) state_machine: HashMap<SymbolHandle, SymbolHandle>,
            /* Owning machine for every statement handle inside a machine state. */
            pub(super) statement_machine: HashMap<StatementHandle, SymbolHandle>,
        }

        impl MachineIndex {
            pub(super) fn build(program: &dyn ::validation::CheckingInput) -> Self {
                let mut machine_symbols = HashSet::new();
                let mut state_machine = HashMap::new();
                let mut statement_machine = HashMap::new();
                for machine in program.machines() {
                    machine_symbols.insert(machine.symbol);
                    for state in program.machine_states(machine) {
                        state_machine.insert(state.symbol, machine.symbol);
                        for node in state.statement_nodes.handles() {
                            statement_machine.insert(node, machine.symbol);
                        }
                    }
                }
                Self {
                    machine_symbols,
                    state_machine,
                    statement_machine,
                }
            }

            /*
                Resolve any symbol that names a machine or a state owned by a machine
                to the machine's declaration symbol. Anything else (data, traits,
                parameters, intrinsics, evidence terms) is not a machine edge.
            */
            pub(super) fn machine_of(&self, symbol: SymbolHandle) -> Option<SymbolHandle> {
                if !symbol.is_valid() {
                    return None;
                }
                if self.machine_symbols.contains(&symbol) {
                    return Some(symbol);
                }
                self.state_machine.get(&symbol).copied()
            }
        }

        /*
            Machines that are always retained independent of reachability: every
            non-`CheckedBody` supply mode plus the provider/authority surface whose
            machine references no root can bound.
        */
        pub(super) fn interface_retention_set(
            program: &dyn ::validation::CheckingInput,
            facts: &CheckFacts,
            index: &MachineIndex,
        ) -> HashSet<SymbolHandle> {
            let mut retained = HashSet::new();

            for machine in program.machines() {
                if !machine.supply_mode.is_checked_body() {
                    retained.insert(machine.symbol);
                }
            }

            for conformance in program.conformances() {
                for row in &conformance.rows {
                    retain_symbol(&mut retained, index, row.realization_machine);
                    retain_symbol(&mut retained, index, row.realization_state);
                }
            }

            for nominal_use in facts.nominal_machine_uses.uses.iter().chain(
                facts
                    .installed_interrupt_registrations
                    .iter()
                    .flat_map(|registration| &registration.callbacks.uses),
            ) {
                retain_symbol(&mut retained, index, nominal_use.selected_machine);
                retain_symbol(&mut retained, index, nominal_use.selected_entry);
                retain_symbol(&mut retained, index, nominal_use.registration_operation);
                if let Some(declaration) = nominal_use.registration_requirement_trait {
                    retain_symbol(&mut retained, index, declaration);
                }
                retain_symbol(&mut retained, index, nominal_use.satisfaction_requirement);
                retain_symbol(&mut retained, index, nominal_use.satisfaction_trait);
            }

            for call in &facts.proof.contract_expression_evidence_calls {
                retain_contract_expression_call(program, index, &mut retained, call);
            }
            for application in &facts
                .proof
                .contract_expression_static_conformance_applications
            {
                retain_machine_argument(program, index, &mut retained, &application.application);
            }

            for projection in &facts.fact_call_projections {
                retained.insert(projection.target_machine);
                retain_symbol(&mut retained, index, projection.target_state);
                for argument in projection.machine_arguments.iter() {
                    retain_static_machine_argument(program, &mut retained, index, argument);
                }
            }

            for plan in &program.access_view_plans {
                retain_symbol(&mut retained, index, plan.policy_plan_machine_symbol);
            }
            for (_, plan) in program.access_lifecycle_plans.iter() {
                if let AccessLifecyclePlan::Evaluated { policy_machine, .. } = plan {
                    retain_symbol(&mut retained, index, *policy_machine);
                }
            }
            for input in &facts.access_view_inputs {
                retain_symbol(&mut retained, index, input.policy_plan_machine);
            }

            for operation in program.expression_table.selected_index_operations() {
                retain_symbol(&mut retained, index, operation.provider);
                retain_symbol(&mut retained, index, operation.operator);
            }

            retain_declaration_surface_machines(program, index, &mut retained);

            retained.retain(|symbol| index.machine_symbols.contains(symbol));
            retained
        }

        /* Extract exact machine edges from typed references and checked facts. */
        pub(super) fn collect_machine_edges(
            program: &dyn ::validation::CheckingInput,
            facts: &CheckFacts,
            index: &MachineIndex,
        ) -> HashMap<SymbolHandle, Vec<SymbolHandle>> {
            let mut edges: HashMap<SymbolHandle, Vec<SymbolHandle>> = HashMap::new();
            macro_rules! edge {
                ($from:expr, $to:expr) => {
                    push_edge(&mut edges, $from, index.machine_of($to))
                };
            }

            for (_, state) in facts.flow.control.states.iter() {
                for call in facts.flow.control.calls.span_or_empty(state.calls) {
                    edge!(state.machine_symbol, call.target_symbol);
                }
            }

            for machine in program.machines() {
                retain_machine_references(
                    program,
                    index,
                    edges.entry(machine.symbol).or_default(),
                    machine,
                );
            }

            for specialization in &program.machine_specializations {
                for argument in &specialization.machine_arguments {
                    edge!(specialization.instance, *argument);
                }
                for application in &specialization.conformance_applications {
                    for argument in application.machine_arguments.iter() {
                        edge!(specialization.instance, *argument);
                    }
                    for row in &application.rows {
                        edge!(specialization.instance, row.realization_machine);
                    }
                }
            }

            for call in &facts.proof.contract_expression_evidence_calls {
                if let Some(machine_symbol) = contract_caller(&facts.proof, call.owner) {
                    retain_contract_expression_call(
                        program,
                        index,
                        edges.entry(machine_symbol).or_default(),
                        call,
                    );
                }
            }
            for application in &facts
                .proof
                .contract_expression_static_conformance_applications
            {
                if let Some(machine_symbol) = contract_caller(&facts.proof, application.owner) {
                    for argument in application.application.machine_arguments.iter() {
                        edge!(machine_symbol, *argument);
                    }
                    for row in &application.application.rows {
                        edge!(machine_symbol, row.realization_machine);
                    }
                }
            }

            for component in &facts.termination.proof_recursive_components {
                for edge in &component.edges {
                    edge!(edge.caller, edge.callee);
                }
            }

            for crossing in &facts.carry.suspension_crossings {
                edge!(crossing.machine, crossing.target);
            }

            for row in facts.flow.semantic_dependencies.iter() {
                edge!(row.consumer_machine, row.dependency);
            }

            for targets in edges.values_mut() {
                targets.sort_unstable_by_key(|symbol| (symbol.arena_index(), symbol.generation()));
                targets.dedup();
            }
            edges
        }

        /*
            Append one resolved machine edge; unresolved symbols are non-machine
            identities and contribute nothing.
        */
        fn push_edge(
            edges: &mut HashMap<SymbolHandle, Vec<SymbolHandle>>,
            from: SymbolHandle,
            to: Option<SymbolHandle>,
        ) {
            if let Some(to) = to {
                edges.entry(from).or_default().push(to);
            }
        }

        struct MachineReferences<'index, 'output, Output> {
            index: &'index MachineIndex,
            output: &'output mut Output,
        }
        impl<Output: Extend<SymbolHandle>> Extend<SymbolHandle> for MachineReferences<'_, '_, Output> {
            fn extend<T: IntoIterator<Item = SymbolHandle>>(&mut self, symbols: T) {
                self.output.extend(
                    symbols
                        .into_iter()
                        .filter_map(|symbol| self.index.machine_of(symbol)),
                );
            }
        }
        impl<Output: Extend<SymbolHandle>> TypedReferences for MachineReferences<'_, '_, Output> {}

        fn retain_contract_expression_call(
            program: &dyn ::validation::CheckingInput,
            index: &MachineIndex,
            output: &mut impl Extend<SymbolHandle>,
            call: &checked_trees::ContractExpressionEvidenceCallFact,
        ) {
            if let Some(contract) = call.runtime_contract {
                let row = program.runtime_callable_contracts.get(contract);
                let mut references = MachineReferences { index, output };
                visit_signature(program, &mut references, &row.signature);
                visit_expression(program, &mut references, call.expression);
            } else {
                retain_symbol(output, index, call.target_machine_symbol);
                retain_symbol(output, index, call.target_state_symbol);
            }
        }

        pub(super) fn validate_contract_expression_call_sources(
            program: &dyn ::validation::CheckingInput,
            facts: &CheckFacts,
        ) -> Result<(), Vec<diagnostics::Diagnostic>> {
            for call in &facts.proof.contract_expression_evidence_calls {
                let Some(contract) = call.runtime_contract else {
                    continue;
                };
                let valid = (|| {
                    if !program.runtime_callable_contracts.is_valid(contract)
                        || call.target_machine_symbol.is_valid()
                        || call.target_state_symbol.is_valid()
                        || !program
                            .expression_table
                            .expression_is_valid(call.expression)
                    {
                        return None;
                    }
                    let typed_trees::expression::ExpressionNode::Call(source) =
                        program.expression_table.expression(call.expression)
                    else {
                        return None;
                    };
                    let row = program.runtime_callable_contracts.get(contract);
                    let site = validation::CallSite::Expression {
                        expression: call.expression,
                        call: source,
                    };
                    match validation::call_contract_source(program, row.lexical_owner, &site)
                        .ok()??
                    {
                        validation::CallContractSource::Runtime { binding, .. }
                            if binding.contract == contract =>
                        {
                            Some(())
                        }
                        _ => None,
                    }
                })();
                if valid.is_none() {
                    return Err(vec![diagnostics::Diagnostic::error(
                        "checked runtime proof-expression call lost its exact contract source during product selection",
                    )]);
                }
            }
            Ok(())
        }

        fn retain_declaration_surface_machines(
            program: &dyn ::validation::CheckingInput,
            index: &MachineIndex,
            output: &mut impl Extend<SymbolHandle>,
        ) {
            visit_declarations(program, &mut MachineReferences { index, output });
        }
        fn retain_machine_references(
            program: &dyn ::validation::CheckingInput,
            index: &MachineIndex,
            output: &mut impl Extend<SymbolHandle>,
            machine: &typed_trees::machine::Machine,
        ) {
            visit_machine(program, &mut MachineReferences { index, output }, machine);
        }
        fn retain_static_machine_argument(
            program: &dyn ::validation::CheckingInput,
            output: &mut impl Extend<SymbolHandle>,
            index: &MachineIndex,
            argument: &StaticMachineArgument,
        ) {
            visit_static_argument(program, &mut MachineReferences { index, output }, argument);
        }
        fn retain_machine_argument(
            program: &dyn ::validation::CheckingInput,
            index: &MachineIndex,
            output: &mut impl Extend<SymbolHandle>,
            application: &ClosedConformanceApplication,
        ) {
            visit_closed_conformance(
                program,
                &mut MachineReferences { index, output },
                application,
            );
        }

        fn retain_symbol(
            output: &mut impl Extend<SymbolHandle>,
            index: &MachineIndex,
            symbol: SymbolHandle,
        ) {
            if let Some(machine) = index.machine_of(symbol) {
                output.extend([machine]);
            }
        }

        fn contract_caller(
            proof: &ProofFacts,
            owner: ContractProofFactOwner,
        ) -> Option<SymbolHandle> {
            match owner {
                ContractProofFactOwner::Machine { machine_symbol }
                | ContractProofFactOwner::MachineState { machine_symbol, .. } => {
                    Some(machine_symbol)
                }
                ContractProofFactOwner::CallSignature { application, .. } => {
                    (crate::published::proof::contracts::call_signatures::owner(
                        &proof.call_contract_signatures,
                        application,
                    ) == Some(owner))
                    .then(|| {
                        proof
                            .call_contract_signatures
                            .get(application)
                            .caller_machine_symbol
                    })
                }
                ContractProofFactOwner::RuntimeCallableContract { .. } => None,
                _ => None,
            }
        }
    }
    pub(crate) mod rewrite {
        /*
            Product roster rewriting.
        */
        use std::collections::HashSet;

        use checked_trees::CheckedTrees;
        use symbols::SymbolHandle;

        use super::dependencies::MachineIndex;

        /*
            Rebuild the machine declaration surface and every machine-keyed `Vec`
            table so the product's iteration-visible rosters match the selection.
            Machine-keyed roster arenas with no inbound `Handle`/`HandleSpan`
            references are rebuilt the same way; arena rows referenced by spans or
            handles from other retained rows remain as valid dead evidence and are
            unreachable through the pruned declaration surface.
        */
        pub(super) fn apply_pruning(
            checked: CheckedTrees,
            retained: &HashSet<SymbolHandle>,
            pruned: &HashSet<SymbolHandle>,
            index: &MachineIndex,
        ) -> CheckedTrees {
            let mut typed = checked.typed.into_inner();
            let mut facts = checked.facts;
            validation::execution_projection::retain_machine_roster(&mut typed, retained, pruned);
            super::retain_live_family_members(&mut typed);
            apply_fact_pruning(&mut facts, retained, pruned, index);
            CheckedTrees::with_roots(typed, facts)
        }

        fn apply_fact_pruning(
            facts: &mut checked_trees::CheckFacts,
            retained: &HashSet<SymbolHandle>,
            pruned: &HashSet<SymbolHandle>,
            index: &MachineIndex,
        ) {
            facts
                .proof_applications
                .retain_product_machines(|machine| retained.contains(&machine));
            facts
                .mutation
                .machines
                .retain(|row| retained.contains(&row.machine));
            facts
                .suspensions
                .machines
                .retain(|row| retained.contains(&row.machine));
            facts
                .blocking
                .machines
                .retain(|row| retained.contains(&row.machine));
            facts
                .synchronous_invocations
                .machines
                .retain(|row| retained.contains(&row.machine));
            facts
                .termination
                .machines
                .retain(|row| retained.contains(&row.machine));
            facts
                .termination
                .proof_recursive_components
                .retain(|component| {
                    component
                        .members
                        .iter()
                        .all(|member| retained.contains(&member.machine))
                });
            facts
                .contract_plans
                .machines_mut()
                .retain(|row| retained.contains(&row.machine));
            facts.contract_plans.index_machines();
            facts
                .contract_plans
                .realized_envelopes
                .retain(|row| retained.contains(&row.machine));
            facts
                .qualifications
                .machines
                .retain(|row| retained.contains(&row.machine));
            facts
                .qualifications
                .vacuous_uses
                .retain(|row| retained.contains(&row.machine));
            facts
                .qualifications
                .content
                .identity_reshuffles
                .retain(|row| retained.contains(&row.machine_symbol));
            facts
                .qualifications
                .content
                .partition_compositions
                .retain(|row| retained.contains(&row.machine_symbol));
            let transformations = facts
                .qualifications
                .content
                .body_transformations
                .iter()
                .filter(|(_, row)| retained.contains(&row.machine_symbol))
                .map(|(_, row)| row.clone())
                .collect::<Vec<_>>();
            let mut retained_transformations = arena::Arena::default();
            retained_transformations.insert_many(transformations);
            facts.qualifications.content.body_transformations = retained_transformations;
            facts
                .qualifications
                .content
                .partition_returns
                .retain(|row| retained.contains(&row.plan.owner));
            facts
                .dynamic_conformances
                .selections
                .retain(|row| retained.contains(&row.machine));
            facts
                .carry
                .suspension_crossings
                .retain(|row| retained.contains(&row.machine));
            facts
                .carry
                .activation_wide_carry
                .retain(|row| retained.contains(&row.machine));
            facts
                .access_view_inputs
                .retain(|row| retained.contains(&row.machine));
            facts
                .provider_closed_requirements
                .retain(|row| retained.contains(&row.provider));
            let removed = facts
                .provider_instance_publication
                .requirements
                .iter()
                .filter_map(|(handle, row)| {
                    let owner = if row.provider.is_valid() {
                        row.provider
                    } else {
                        row.template
                    };
                    (!retained.contains(&owner)).then_some(handle)
                })
                .collect::<Vec<_>>();
            for handle in removed {
                facts
                    .provider_instance_publication
                    .requirements
                    .free(handle);
            }
            facts
                .operators
                .symbolic_boundary_applications
                .retain(|row| retained.contains(&row.machine_symbol));
            let retained_template = |origin: checked_trees::CheckedValueOrigin| {
                origin
                    .machine_symbol()
                    .is_some_and(|machine| retained.contains(&machine))
            };
            facts.operators.uses.for_each_mut(|_, operator_use| {
                operator_use.is_generic_template &= retained_template(operator_use.origin);
            });
            facts.operators.named_uses.for_each_mut(|_, operator_use| {
                operator_use.is_generic_template &= retained_template(operator_use.origin);
            });
            let mut requirements = arena::Arena::new();
            requirements.insert_many(
                facts
                    .operators
                    .provider_requirements
                    .iter()
                    .filter(|&(_, requirement)| {
                        matches!(
                            requirement.kind,
                            checked_trees::CheckedProviderRequirementKind::Operator { .. }
                        ) || retained.contains(&requirement.symbol)
                    })
                    .map(|(_, requirement)| requirement.clone()),
            );
            facts.operators.provider_requirements = requirements;
            facts
                .nominal_machine_uses
                .uses
                .retain(|row| match row.site {
                    checked_trees::NominalMachineUseSite::Statement(statement) => index
                        .statement_machine
                        .get(&statement)
                        .is_some_and(|owner| retained.contains(owner)),
                    checked_trees::NominalMachineUseSite::Expression(_) => true,
                });

            facts
                .installed_interrupt_registrations
                .retain(|row| match row.site {
                    checked_trees::NominalMachineUseSite::Statement(statement) => index
                        .statement_machine
                        .get(&statement)
                        .is_some_and(|owner| retained.contains(owner)),
                    checked_trees::NominalMachineUseSite::Expression(_) => true,
                });

            facts
                .proof
                .contract_expression_evidence_calls
                .retain(|row| {
                    contract_owner_retained(
                        row.owner,
                        pruned,
                        &facts.proof.call_contract_signatures,
                    )
                });
            facts
                .proof
                .contract_expression_static_conformance_applications
                .retain(|row| {
                    contract_owner_retained(
                        row.owner,
                        pruned,
                        &facts.proof.call_contract_signatures,
                    )
                });

            {
                let kept: Vec<checked_trees::FlowStateFact> = facts
                    .flow
                    .control
                    .states
                    .iter()
                    .map(|(_, state)| state)
                    .filter(|state| retained.contains(&state.machine_symbol))
                    .cloned()
                    .collect();
                let mut states = arena::PagedArenaSet::default();
                states.insert_many(kept);
                facts.flow.control.states = states.into();
            }
            {
                let kept: Vec<checked_trees::MachineServiceReachRows> = facts
                    .service_reaches
                    .machines
                    .iter()
                    .map(|(_, row)| row)
                    .filter(|row| retained.contains(&row.machine))
                    .cloned()
                    .collect();
                let mut machines = arena::Arena::default();
                facts.service_reaches.root_machines = machines.insert_many(kept);
                facts.service_reaches.machines = machines;
            }

            facts.flow.semantic_dependencies.rows.retain(|row| {
                retained.contains(&row.consumer_machine) && !pruned.contains(&row.dependency)
            });

            facts
                .flow
                .terminal_machines
                .machines
                .retain(|row| retained.contains(&row.machine));
            facts
                .flow
                .terminal_debug
                .machines
                .retain(|row| retained.contains(&row.machine));
            facts.flow.guarded_exits.tails.retain(|row| {
                index
                    .state_machine
                    .get(&row.state)
                    .is_none_or(|machine| retained.contains(machine))
            });
            facts
                .flow
                .terminal_unit_effects
                .machines
                .retain(|row| retained.contains(&row.machine));
            facts
                .flow
                .terminal_unit_effects
                .composed_machines
                .retain(|row| retained.contains(&row.machine));
            facts
                .flow
                .terminal_unit_effects
                .boundary_machines
                .retain(|row| !pruned.contains(&row.machine));
            facts
                .flow
                .terminal_partial_affine_unit_cleanups
                .machines
                .retain(|row| retained.contains(&row.machine.machine));
            facts
                .flow
                .terminal_nominal_affine_unit_cleanups
                .machines
                .retain(|row| retained.contains(&row.machine));
            facts
                .flow
                .terminal_structural_control_cleanups
                .states
                .retain(|row| retained.contains(&row.machine));
        }

        /*
            Whether a contract-fact owner survives pruning: machine owners must be
            retained; non-machine owners (trait/operator machinery) are always
            retained.
        */
        fn contract_owner_retained(
            owner: checked_trees::ContractProofFactOwner,
            pruned: &HashSet<SymbolHandle>,
            signatures: &arena::Arena<checked_trees::CallContractSignature>,
        ) -> bool {
            match owner {
                checked_trees::ContractProofFactOwner::Machine { machine_symbol }
                | checked_trees::ContractProofFactOwner::MachineState { machine_symbol, .. } => {
                    !pruned.contains(&machine_symbol)
                }
                checked_trees::ContractProofFactOwner::CallSignature { application, .. } => {
                    crate::published::proof::contracts::call_signatures::owner(
                        signatures,
                        application,
                    ) == Some(owner)
                        && !pruned.contains(&signatures.get(application).caller_machine_symbol)
                }
                checked_trees::ContractProofFactOwner::RuntimeCallableContract { .. } => true,
                _ => true,
            }
        }
    }

    use dependencies::{MachineIndex, collect_machine_edges, interface_retention_set};
    use rewrite::apply_pruning;

    const PRODUCT_SELECTION_IDENTITY_DOMAIN: &[u8] =
        b"omega.psi.checked-tree-product-selection.v1\0";

    /*
        Exact product roots for one checked-tree product selection. Roots are
        machine symbols chosen by the coordinator (entry/product surface); the
        phase validates each against the checked program and never invents or
        infers an additional root.
    */
    #[derive(Debug, Clone, Default, PartialEq, Eq)]
    pub struct CheckedTreeProductRoots {
        machines: Vec<SymbolHandle>,
    }

    impl CheckedTreeProductRoots {
        /*
            Bind an exact root set. Duplicates are rejected rather than
            canonicalized so the selection identity never differs from the
            coordinator's input.
        */
        pub fn new(
            machines: impl IntoIterator<Item = SymbolHandle>,
        ) -> Result<Self, CheckedTreeProductRootsError> {
            let mut machines: Vec<SymbolHandle> = machines.into_iter().collect();
            machines.sort_unstable_by_key(|symbol| symbol.arena_index());
            if let Some(duplicate) = machines
                .windows(2)
                .find(|pair| pair[0] == pair[1])
                .map(|pair| pair[0])
            {
                return Err(CheckedTreeProductRootsError::DuplicateRoot(duplicate));
            }
            Ok(Self { machines })
        }

        pub fn machines(&self) -> &[SymbolHandle] {
            &self.machines
        }
    }

    /* A product root selection that cannot be a canonical input. */
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum CheckedTreeProductRootsError {
        DuplicateRoot(SymbolHandle),
    }

    /*
        The exact opt-in plan for the checked-tree product-pruning phase.

        This is the phase-local projection a coordinator supplies when the unified
        build selection names this optimization for the `CheckedTrees` phase. The
        plan owns no defaults: every retained root is explicit.
    */
    #[derive(Debug, Clone, Default, PartialEq, Eq)]
    pub struct CheckedTreeProductPruning {
        roots: CheckedTreeProductRoots,
    }

    impl CheckedTreeProductPruning {
        pub fn new(roots: CheckedTreeProductRoots) -> Self {
            Self { roots }
        }

        pub const fn roots(&self) -> &CheckedTreeProductRoots {
            &self.roots
        }
    }

    /*
        Domain-separated commitment to one executed product selection. The digest
        binds the phase identity, the exact roots, and the retained/pruned machine
        rosters in declaration order.
    */
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct CheckedTreeProductSelectionIdentity([u8; 32]);

    impl CheckedTreeProductSelectionIdentity {
        pub const fn from_digest(digest: [u8; 32]) -> Self {
            Self(digest)
        }

        pub const fn as_bytes(self) -> [u8; 32] {
            self.0
        }
    }

    /*
        Identity-bearing evidence for one executed checked-tree product
        selection. The rosters partition the source program's machine table: the
        same plan against the same checked program always reproduces the same
        record.
    */
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct CheckedTreeProductSelection {
        roots: CheckedTreeProductRoots,
        retained_machines: Vec<SymbolHandle>,
        pruned_machines: Vec<SymbolHandle>,
        identity: CheckedTreeProductSelectionIdentity,
    }

    impl CheckedTreeProductSelection {
        pub const fn roots(&self) -> &CheckedTreeProductRoots {
            &self.roots
        }

        /* Retained machine symbols in source declaration order. */
        pub fn retained_machines(&self) -> &[SymbolHandle] {
            &self.retained_machines
        }

        /* Pruned machine symbols in source declaration order. */
        pub fn pruned_machines(&self) -> &[SymbolHandle] {
            &self.pruned_machines
        }

        pub const fn identity(&self) -> CheckedTreeProductSelectionIdentity {
            self.identity
        }
    }

    /* The pruned checked product plus its identity-bearing selection evidence. */
    #[derive(Debug)]
    pub struct CheckedTreeProductPruningOutcome {
        pub checked: CheckedTrees,
        pub selection: CheckedTreeProductSelection,
    }

    /*
        Execute the optional checked-product selection after authored checking.

        An absent root selection is the identity operation, retaining the same
        shared program and no selection evidence. A requested selection must name
        at least one canonical root; it cannot silently prune the whole product.
        The lower-level pruning transform also supports interface-only products,
        but that is not an executable product selection.
    */
    pub fn select_checked_tree_product(
        checked: Arc<CheckedTrees>,
        product_root_machines: Option<Vec<SymbolHandle>>,
    ) -> Result<(Arc<CheckedTrees>, Option<CheckedTreeProductSelection>), Vec<Diagnostic>> {
        let Some(product_root_machines) = product_root_machines else {
            return Ok((checked, None));
        };
        let roots = CheckedTreeProductRoots::new(product_root_machines).map_err(|error| {
            vec![Diagnostic::error(format!(
                "checked-tree product root selection is not canonical: {error:?}"
            ))]
        })?;
        if roots.machines().is_empty() {
            return Err(vec![Diagnostic::error(
                "CheckedTreeProductPruning requires at least one bound product root; this compilation's build bound none",
            )]);
        }
        let program = Arc::try_unwrap(checked).unwrap_or_else(|shared| (*shared).clone());
        let outcome = prune_checked_tree_product(program, &CheckedTreeProductPruning::new(roots))?;
        Ok((Arc::new(outcome.checked), Some(outcome.selection)))
    }

    /*
        Run the checked-tree product-pruning phase.

        The input must be a fully checked program: this phase runs only after all
        authored source has been parsed, resolved, typed, and checked, and it
        returns diagnostics rather than ever relaxing that contract. A root that
        does not name a declared machine is rejected before any mutation.
    */
    pub fn prune_checked_tree_product(
        mut checked: CheckedTrees,
        plan: &CheckedTreeProductPruning,
    ) -> Result<CheckedTreeProductPruningOutcome, Vec<Diagnostic>> {
        let index = MachineIndex::build(&checked.typed);

        let mut diagnostics = Vec::new();
        for root in plan.roots().machines() {
            if !index.machine_symbols.contains(root) {
                diagnostics.push(Diagnostic::error(format!(
                    "checked-tree product pruning root does not name a declared machine (symbol #{})",
                    root.arena_index()
                )));
            }
        }
        if !diagnostics.is_empty() {
            return Err(diagnostics);
        }

        dependencies::validate_contract_expression_call_sources(&checked.typed, &checked.facts)?;
        let edges = collect_machine_edges(&checked.typed, &checked.facts, &index);
        let mut retained = interface_retention_set(&checked.typed, &checked.facts, &index);
        retained.extend(plan.roots().machines().iter().copied());
        retained.extend(
            checked
                .typed
                .constant_recipes
                .iter()
                .filter(|(_, recipe)| {
                    !matches!(
                        recipe.position,
                        typed_trees::constant_recipes::RetainedConstantPosition::ContractExpression(
                            _
                        )
                    )
                })
                .map(|(_, recipe)| recipe.machine)
                .filter(|machine| machine.is_valid()),
        );
        retained.extend(
            checked
                .typed
                .target_machine_families
                .iter()
                .flat_map(|family| family.members.iter().map(|member| member.machine)),
        );

        let mut frontier: Vec<SymbolHandle> = retained.iter().copied().collect();
        while let Some(machine) = frontier.pop() {
            let Some(targets) = edges.get(&machine) else {
                continue;
            };
            for target in targets {
                if retained.insert(*target) {
                    frontier.push(*target);
                }
            }
        }

        let pruned: HashSet<SymbolHandle> = index
            .machine_symbols
            .iter()
            .copied()
            .filter(|symbol| !retained.contains(symbol))
            .collect();

        checked = apply_pruning(checked, &retained, &pruned, &index);

        let retained_machines: Vec<SymbolHandle> = checked
            .typed
            .machines()
            .iter()
            .map(|machine| machine.symbol)
            .collect();
        let mut pruned_machines: Vec<SymbolHandle> = pruned.into_iter().collect();
        pruned_machines.sort_unstable_by_key(|symbol| symbol.arena_index());

        let identity = selection_identity(
            plan.roots().machines(),
            &retained_machines,
            &pruned_machines,
        );
        let selection = CheckedTreeProductSelection {
            roots: plan.roots().clone(),
            retained_machines,
            pruned_machines,
            identity,
        };
        Ok(CheckedTreeProductPruningOutcome { checked, selection })
    }

    /*
        Keep the target-machine family roster consistent with the pruned machine
        table: `members` names only bodies this product still carries, and a
        family whose members all left keeps no row. `canonical` is retained
        regardless — it is the family's contract-head identity, a symbol that
        survives machine pruning even when its own body did not.
    */
    fn retain_live_family_members(typed: &mut TypedTrees) {
        if typed.target_machine_families.is_empty() {
            return;
        }
        let live: HashSet<SymbolHandle> = typed
            .machines()
            .iter()
            .map(|machine| machine.symbol)
            .collect();
        for family in &mut typed.target_machine_families {
            family
                .members
                .retain(|member| live.contains(&member.machine));
        }
        typed
            .target_machine_families
            .retain(|family| !family.members.is_empty());
    }

    /* Domain-separated SHA-256 identity of one executed product selection. */
    fn selection_identity(
        roots: &[SymbolHandle],
        retained: &[SymbolHandle],
        pruned: &[SymbolHandle],
    ) -> CheckedTreeProductSelectionIdentity {
        let mut hasher =
            crate::digest_encoder::DomainDigestEncoder::new(PRODUCT_SELECTION_IDENTITY_DOMAIN);
        let mut encode = |symbols: &[SymbolHandle]| {
            hasher.u64(symbols.len() as u64);
            for symbol in symbols {
                hasher.raw(&symbol.arena_index().to_le_bytes());
                hasher.raw(&symbol.generation().to_le_bytes());
            }
        };
        encode(roots);
        encode(retained);
        encode(pruned);
        CheckedTreeProductSelectionIdentity(hasher.finish())
    }
}
pub(crate) mod quotient_eligibility {
    /*
        Finish quotient eligibility from the same checking invocation's crash facts.
        Schema and termination admission precede body checking; no new effect walk
        or crash inference runs here. Missing invocation evidence is not purity.
    */
    use crate::flow::expressions;
    use checked_trees::CheckFacts;
    use diagnostics::Diagnostic;

    pub(crate) fn check(
        program: &dyn validation::CheckingInput,
        facts: &CheckFacts,
    ) -> Result<(), Vec<Diagnostic>> {
        let mut diagnostics = Vec::new();
        for operation in &facts.quotient_operations {
            if let Err(reason) = check_operation(program, facts, operation) {
                diagnostics.push(Diagnostic::error(format!(
                    "quotient operation `{}` lacks checked normal-result eligibility: {reason}",
                    program.symbols.display_path(operation.public_machine, "::"),
                )));
            }
        }
        if diagnostics.is_empty() {
            Ok(())
        } else {
            Err(diagnostics)
        }
    }

    fn check_operation(
        program: &dyn validation::CheckingInput,
        facts: &CheckFacts,
        operation: &checked_trees::CheckedQuotientOperation,
    ) -> Result<(), &'static str> {
        let state = facts
            .flow
            .control
            .state_by_symbol(operation.public_state)
            .ok_or("public flow state is absent")?;
        let mut calls = crate::flow::call_flows(&facts.flow.control, state.calls)
            .iter()
            .filter(|call| call.authored_expression == operation.request);
        let call = calls.next().ok_or("selected invocation is absent")?;
        if calls.next().is_some() {
            return Err("selected invocation is ambiguous");
        }
        let (representative, _) =
            validation::find_machine_by_entry_state(program, operation.representative_state)
                .ok_or("selected representative is absent")?;
        let plan = facts
            .contract_plans
            .for_machine(operation.public_machine)
            .ok_or("public checked contract is absent")?;
        let checked = plan
            .crash
            .checked_call_at(
                operation.public_state,
                u32::try_from(call.statement_index).map_err(|_| "statement coordinate overflow")?,
                u32::try_from(call.call_ordinal).map_err(|_| "call coordinate overflow")?,
            )
            .ok_or("selected representative crash evidence is unresolved")?;
        if checked.target_machine() != representative.symbol
            || checked.target_state() != operation.representative_state
        {
            checked.executable_contract_identity(
                &facts.contract_plans,
                &facts.proof,
                operation.public_machine,
                representative.symbol,
                operation.representative_state,
            )?;
        }
        if !checked.surviving_buckets().is_empty() {
            return Err(
                "selected representative retains a possible crash in the public invocation",
            );
        }
        let request = expressions(program)
            .call_node(operation.request)
            .and_then(|call| call.quotient_operation.as_ref())
            .ok_or("selected request is absent")?;
        for theorem in &request.theorem_evidence {
            let (machine, _) =
                validation::find_machine_by_entry_state(program, theorem.application.symbol)
                    .ok_or("selected theorem body is absent")?;
            let plan = facts
                .contract_plans
                .for_machine(machine.symbol)
                .ok_or("selected theorem checked contract is absent")?;
            if !plan.crash.checked_sites().is_empty()
                || !plan.crash.trapping_sites().is_empty()
                || plan
                    .crash
                    .checked_calls()
                    .iter()
                    .any(|call| !call.surviving_buckets().is_empty())
                || plan
                    .crash
                    .checked_operators()
                    .iter()
                    .any(|operator| !operator.surviving().is_empty())
            {
                return Err("selected theorem retains checked crash behavior");
            }
            for (_, state) in facts
                .flow
                .control
                .machine_states(machine.symbol)
                .into_iter()
                .map(|state| ((), state))
            {
                for call in facts.flow.control.calls.span_or_empty(state.calls) {
                    let site = validation::find_call_site(
                        program,
                        state.machine_symbol,
                        state.state_symbol,
                        call.statement_index,
                        call.call_ordinal,
                    )
                    .ok_or("selected theorem call site is unresolved")?;
                    if matches!(site, validation::CallSite::TransitionNamed { .. }) {
                        continue;
                    }
                    if let Some((target_machine, target_state)) =
                        crate::published::facts::selected_invocation_target(
                            program,
                            site,
                            call.target_symbol,
                        )
                    {
                        let checked = plan
                            .crash
                            .checked_call_at(
                                state.state_symbol,
                                u32::try_from(call.statement_index)
                                    .map_err(|_| "theorem statement coordinate overflow")?,
                                u32::try_from(call.call_ordinal)
                                    .map_err(|_| "theorem call coordinate overflow")?,
                            )
                            .ok_or("selected theorem invocation crash evidence is unresolved")?;
                        if checked.target_machine() != target_machine
                            || checked.target_state() != target_state
                        {
                            checked.executable_contract_identity(
                                &facts.contract_plans,
                                &facts.proof,
                                plan.machine,
                                target_machine,
                                target_state,
                            )?;
                        }
                    }
                }
            }
        }
        Ok(())
    }
}
