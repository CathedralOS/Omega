/*
    Typed-program proof obligation construction, range and guard derivation,
    certificate generation, mathematical declaration elaboration, and derivation
    caching.

    We collect bounded-value, assignment, call, transition, and state-return
    obligations from typed trees (`obligations`), discharge them through integer
    and floating-point range reasoning and kernel-checked certificates
    (`checker`), retain re-verified certificates by canonical obligation
    identity (`derivation_store`), and elaborate authored mathematical
    declarations into kernel-checked signatures (`mathematical_declarations`).
*/

/*
    Keeping proof obligation collection, certificate synthesis, and mathematical
    declaration elaboration in a dedicated target-neutral Psi crate separates
    proof search and source diagnostics from both pipeline stage sequencing and
    the small, search-free `proof-admission` kernel that independently re-decides
    every emitted certificate.
*/

/*
    Start at `obligations::build_proof_plan` to construct a `ProofPlan` from a
    typed program and `checker::check_proof_plan_with_decisions` to discharge
    its obligations. `mathematical_declarations::check_mathematical_declarations`
    elaborates authored mathematical definitions and inductive families;
    `derivation_store` persists certificates by semantic identity; `lemmas` and
    `boundary` define reusable proof vocabulary and boundary contracts; and
    `proof_surface` and `typed_expressions` (in this file) provide surface
    reporting and shared expression queries.
*/

pub mod boundary;
pub mod checker;
pub mod derivation_store;
pub mod lemmas;
pub mod mathematical_declarations;
pub mod obligations;
pub(crate) mod proof_surface {
    use arena::Arena;
    use syntax_trees::SyntaxTrees;
    use syntax_trees::identifier::Identifier;
    use syntax_trees::identifier::JoinedSyntaxPath;
    use syntax_trees::item;
    use syntax_trees::item::PropositionBody;
    use syntax_trees::item::{
        CapabilityContract, CapabilityContractKind, DataMember, Item, Machine, ProofFact,
    };
    use syntax_trees::types::{TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode};

    #[derive(Debug, Clone, PartialEq, Eq, Default)]
    pub struct ProofSurfaceReport {
        pub domains: Arena<DomainSurface>,
        pub propositions: Arena<PropositionSurface>,
        pub contracts: Arena<ContractSurface>,
        pub bounded_sites: Arena<BoundedTypeSite>,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq)]
    pub struct PropositionSurface {
        pub name: String,
        pub binder_count: usize,
        pub parameter_count: usize,
        pub body: PropositionBodySurface,
    }

    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
    pub enum PropositionBodySurface {
        #[default]
        Primitive,
        Witness,
        Transparent,
    }

    #[derive(Debug, Clone, PartialEq, Eq, Default)]
    pub struct DomainSurface {
        pub name: String,
        pub target_type: String,
        pub predicate_presence: language_semantics::DomainPredicatePresence,
        pub fact_count: usize,
        pub membership_fact_count: usize,
        pub semantic_clause_token_count: usize,
    }

    #[derive(Debug, Clone, PartialEq, Eq, Default)]
    pub struct ContractSurface {
        pub owner: String,
        pub kind: ContractKindSurface,
        pub fact_count: usize,
        pub membership_fact_count: usize,
        pub token_count: usize,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
    pub enum ContractKindSurface {
        #[default]
        Requires,
        Ensures,
        Crashes,
    }

    #[derive(Debug, Clone, PartialEq, Eq, Default)]
    pub struct BoundedTypeSite {
        pub owner: String,
        pub base_type: String,
        pub constraints: String,
    }

    pub fn build_proof_surface_report(syntax_trees: &SyntaxTrees) -> ProofSurfaceReport {
        let mut report = ProofSurfaceReport::default();

        for item in syntax_trees.root_items() {
            match item {
                Item::Capability(_) => {}
                Item::Const(_) => {}
                Item::TypeAlias(alias) => {
                    collect_bounded_type_site(
                        &mut report,
                        syntax_trees,
                        alias.expansion,
                        &format!("type alias `{}` expansion", alias.name.as_str(syntax_trees)),
                    );
                    for parameter in syntax_trees.items.type_parameters(alias.type_parameters) {
                        if let syntax_trees::item::TypeParameterKind::Const { type_reference }
                        | syntax_trees::item::TypeParameterKind::Value { type_reference } =
                            parameter.kind
                        {
                            collect_bounded_type_site(
                                &mut report,
                                syntax_trees,
                                type_reference,
                                &format!(
                                    "type alias `{}` parameter `{}`",
                                    alias.name.as_str(syntax_trees),
                                    parameter.name.as_str(syntax_trees)
                                ),
                            );
                        }
                    }
                }
                Item::Data(data_definition) => {
                    for member in syntax_trees.items.data_members(data_definition.members) {
                        if let DataMember::Field(field) = member {
                            collect_bounded_type_site(
                                &mut report,
                                syntax_trees,
                                field.type_reference,
                                &format!(
                                    "data `{}` field `{}`",
                                    data_definition.name.as_str(syntax_trees),
                                    field.name.as_str(syntax_trees)
                                ),
                            );
                        }
                    }
                }
                Item::Domain(domain) => {
                    report.domains.insert(DomainSurface {
                        name: domain.name.as_str(syntax_trees).to_owned(),
                        target_type: type_reference_name(syntax_trees, domain.target_type),
                        predicate_presence: domain.predicate_presence,
                        fact_count: syntax_trees.items.proof_facts(domain.facts).len(),
                        membership_fact_count: syntax_trees
                            .items
                            .proof_facts(domain.facts)
                            .iter()
                            .filter(|fact| matches!(fact, ProofFact::Membership(_)))
                            .count(),
                        semantic_clause_token_count: domain.semantic_clause_token_count,
                    });
                    collect_bounded_type_site(
                        &mut report,
                        syntax_trees,
                        domain.target_type,
                        &format!("domain `{}` target type", domain.name.as_str(syntax_trees)),
                    );
                    for operator in syntax_trees.items.operators(domain.operators) {
                        collect_operator(
                            &mut report,
                            syntax_trees,
                            operator,
                            &format!(
                                "domain `{}` operator `{}`",
                                domain.name.as_str(syntax_trees),
                                operator_name(syntax_trees, operator.name)
                            ),
                        );
                    }
                }
                Item::Use(_)
                | Item::Measure(_)
                | Item::Conformance(_)
                | Item::Module(_)
                | Item::Package(_) => {}
                Item::Operator(operator) => {
                    collect_operator(
                        &mut report,
                        syntax_trees,
                        operator,
                        &format!("operator `{}`", operator_name(syntax_trees, operator.name)),
                    );
                }
                Item::Proposition(proposition) => {
                    let body = match proposition.body {
                        PropositionBody::Primitive => PropositionBodySurface::Primitive,
                        PropositionBody::Witness { evidence } => {
                            collect_bounded_type_site(
                                &mut report,
                                syntax_trees,
                                evidence,
                                &format!(
                                    "proposition `{}` evidence",
                                    proposition.name.as_str(syntax_trees)
                                ),
                            );
                            PropositionBodySurface::Witness
                        }
                        PropositionBody::Transparent { .. } => PropositionBodySurface::Transparent,
                    };
                    for parameter in syntax_trees.items.state_parameters(proposition.parameters) {
                        let parameter = syntax_trees.items.state_parameter(*parameter);
                        collect_bounded_type_site(
                            &mut report,
                            syntax_trees,
                            parameter.type_reference,
                            &format!(
                                "proposition `{}` parameter `{}`",
                                proposition.name.as_str(syntax_trees),
                                parameter.name.as_str(syntax_trees)
                            ),
                        );
                    }
                    report.propositions.insert(PropositionSurface {
                        name: proposition.name.as_str(syntax_trees).to_owned(),
                        binder_count: proposition.type_parameters.len(),
                        parameter_count: proposition.parameters.len(),
                        body,
                    });
                }
                Item::Machine(machine) => collect_machine(&mut report, syntax_trees, machine),
                Item::Trait(trait_definition) => {
                    collect_trait_definition(&mut report, syntax_trees, trait_definition)
                }
            }
        }

        report
    }

    fn collect_machine(
        report: &mut ProofSurfaceReport,
        syntax_trees: &SyntaxTrees,
        machine: &Machine,
    ) {
        collect_contracts(
            report,
            syntax_trees,
            machine.contracts,
            &format!("machine `{}`", machine.name.as_str(syntax_trees)),
        );

        for state in syntax_trees.items.state_handles(machine.states) {
            let state = syntax_trees.items.state(*state);
            collect_state_node(report, syntax_trees, machine, state);
        }
    }

    fn collect_operator(
        report: &mut ProofSurfaceReport,
        syntax_trees: &SyntaxTrees,
        operator: &item::OperatorDefinition,
        owner: &str,
    ) {
        collect_contracts(report, syntax_trees, operator.contracts, owner);
        collect_signature_parts(
            report,
            syntax_trees,
            operator.parameters,
            operator.return_type,
            owner,
        );
    }

    fn collect_state_node(
        report: &mut ProofSurfaceReport,
        syntax_trees: &SyntaxTrees,
        machine: &Machine,
        state: &item::State,
    ) {
        collect_contracts(
            report,
            syntax_trees,
            state.contracts,
            &format!(
                "machine `{}` state `{}`",
                machine.name.as_str(syntax_trees),
                state.name.as_str(syntax_trees)
            ),
        );
        collect_signature_parts(
            report,
            syntax_trees,
            state.parameters,
            state.return_type,
            &format!(
                "machine `{}` state `{}`",
                machine.name.as_str(syntax_trees),
                state.name.as_str(syntax_trees)
            ),
        );

        for statement in syntax_trees.items.statements(state.statements) {
            let syntax_trees::statement::StatementNode::LocalData(local_data) =
                syntax_trees.statements.statement(*statement)
            else {
                continue;
            };

            collect_bounded_type_site(
                report,
                syntax_trees,
                local_data.type_reference,
                &format!(
                    "machine `{}` state `{}` local `{}`",
                    machine.name.as_str(syntax_trees),
                    state.name.as_str(syntax_trees),
                    local_data.name.as_str(syntax_trees)
                ),
            );
        }
    }

    fn collect_trait_definition(
        report: &mut ProofSurfaceReport,
        syntax_trees: &SyntaxTrees,
        trait_definition: &item::TraitDefinition,
    ) {
        for machine in syntax_trees
            .items
            .state_signatures(trait_definition.machines)
        {
            let machine = syntax_trees.items.state_signature(*machine);
            collect_state_signature_node(
                report,
                syntax_trees,
                machine,
                &format!(
                    "trait `{}` machine `{}`",
                    trait_definition.name.as_str(syntax_trees),
                    machine.name.as_str(syntax_trees)
                ),
            );
        }
    }

    fn collect_state_signature_node(
        report: &mut ProofSurfaceReport,
        syntax_trees: &SyntaxTrees,
        state: &item::StateSignature,
        owner: &str,
    ) {
        for parameter in syntax_trees.items.type_parameters(state.type_parameters) {
            use item::{MachineParameterContract, TypeParameterKind};
            let parameter_owner = format!(
                "{owner} static parameter `{}`",
                parameter.name.as_str(syntax_trees)
            );
            match &parameter.kind {
                TypeParameterKind::Const { type_reference }
                | TypeParameterKind::Value { type_reference } => {
                    collect_bounded_type_site(
                        report,
                        syntax_trees,
                        *type_reference,
                        &parameter_owner,
                    );
                }
                TypeParameterKind::Machine {
                    contract: Some(MachineParameterContract::Structural(signature)),
                } => {
                    collect_state_signature_node(report, syntax_trees, signature, &parameter_owner);
                }
                TypeParameterKind::Machine {
                    contract:
                        Some(MachineParameterContract::Nominal {
                            trait_arguments, ..
                        }),
                } => {
                    for argument in syntax_trees
                        .type_references
                        .type_reference_handles(*trait_arguments)
                    {
                        collect_bounded_type_site(
                            report,
                            syntax_trees,
                            *argument,
                            &parameter_owner,
                        );
                    }
                }
                TypeParameterKind::Proposition {
                    contract: Some(signature),
                } => {
                    for formal in syntax_trees.items.state_parameters(signature.parameters) {
                        let formal = syntax_trees.items.state_parameter(*formal);
                        collect_bounded_type_site(
                            report,
                            syntax_trees,
                            formal.type_reference,
                            &parameter_owner,
                        );
                    }
                }
                TypeParameterKind::Type
                | TypeParameterKind::Implementation
                | TypeParameterKind::Machine {
                    contract:
                        None
                        | Some(
                            MachineParameterContract::Inferred
                            | MachineParameterContract::RequirementIdentity,
                        ),
                }
                | TypeParameterKind::Proposition { contract: None } => {}
            }
        }
        for bound in &state.conformance_bounds {
            for argument in syntax_trees
                .type_references
                .type_reference_handles(bound.arguments)
            {
                collect_bounded_type_site(report, syntax_trees, *argument, owner);
            }
        }
        collect_contracts(report, syntax_trees, state.contracts, owner);
        collect_signature_parts(
            report,
            syntax_trees,
            state.parameters,
            state.return_type,
            owner,
        );
    }

    fn collect_signature_parts(
        report: &mut ProofSurfaceReport,
        syntax_trees: &SyntaxTrees,
        parameters: arena::HandleSpan<item::StateParameterHandle>,
        return_type: TypeReferenceHandle,
        owner: &str,
    ) {
        for parameter in syntax_trees.items.state_parameters(parameters) {
            let parameter = syntax_trees.items.state_parameter(*parameter);
            collect_bounded_type_site(
                report,
                syntax_trees,
                parameter.type_reference,
                &format!(
                    "{owner} parameter `{}`",
                    parameter.name.as_str(syntax_trees)
                ),
            );
        }

        if return_type.is_valid() {
            collect_bounded_type_site(
                report,
                syntax_trees,
                return_type,
                &format!("{owner} return type"),
            );
        }
    }

    fn collect_contracts(
        report: &mut ProofSurfaceReport,
        syntax_trees: &SyntaxTrees,
        contracts: arena::HandleSpan<CapabilityContract>,
        owner: &str,
    ) {
        for contract in syntax_trees.items.capability_contracts(contracts) {
            report.contracts.insert(ContractSurface {
                owner: owner.to_owned(),
                kind: match &contract.kind {
                    CapabilityContractKind::Requires => ContractKindSurface::Requires,
                    CapabilityContractKind::Ensures => ContractKindSurface::Ensures,
                    CapabilityContractKind::EnsuresForResultCase { .. } => {
                        ContractKindSurface::Ensures
                    }
                    CapabilityContractKind::Crashes { .. } => ContractKindSurface::Crashes,
                },
                fact_count: syntax_trees.items.proof_facts(contract.facts).len(),
                membership_fact_count: syntax_trees
                    .items
                    .proof_facts(contract.facts)
                    .iter()
                    .filter(|fact| matches!(fact, ProofFact::Membership(_)))
                    .count(),
                token_count: contract.token_count,
            });
        }
    }

    fn collect_bounded_type_site(
        report: &mut ProofSurfaceReport,
        syntax_trees: &SyntaxTrees,
        type_reference: TypeReferenceHandle,
        owner: &str,
    ) {
        match syntax_trees.type_references.type_reference(type_reference) {
            TypeReferenceNode::Callable { signature } => {
                let signature = syntax_trees.items.state_signature(*signature);
                collect_state_signature_node(
                    report,
                    syntax_trees,
                    signature,
                    &format!("{owner} callable signature"),
                );
            }
            TypeReferenceNode::Reference { referee, .. } => {
                collect_bounded_type_site(report, syntax_trees, *referee, owner);
            }
            TypeReferenceNode::Constrained {
                base_type,
                constraints,
            } => {
                report.bounded_sites.insert(BoundedTypeSite {
                    owner: owner.to_owned(),
                    base_type: type_reference_name(syntax_trees, *base_type),
                    constraints: constraint_handle_name(syntax_trees, *constraints),
                });
                collect_bounded_type_site(report, syntax_trees, *base_type, owner);
            }
            TypeReferenceNode::FixedArray { element_type, .. } => {
                collect_bounded_type_site(report, syntax_trees, *element_type, owner);
            }
            TypeReferenceNode::Slice { element_type } => {
                collect_bounded_type_site(report, syntax_trees, *element_type, owner);
            }
            TypeReferenceNode::Generic { arguments, .. } => {
                for argument in syntax_trees
                    .type_references
                    .type_reference_handles(*arguments)
                {
                    collect_bounded_type_site(report, syntax_trees, *argument, owner);
                }
            }
            TypeReferenceNode::StaticSelection(selection) => {
                collect_bounded_static_selection(report, syntax_trees, selection, owner);
            }
            TypeReferenceNode::ConstExpression(_) | TypeReferenceNode::DynamicTrait { .. } => {}
            TypeReferenceNode::Named(_) | TypeReferenceNode::Unit => {}
        }
    }

    fn collect_bounded_static_selection(
        report: &mut ProofSurfaceReport,
        syntax: &SyntaxTrees,
        selection: &syntax_trees::expression::StaticMachineArgument,
        owner: &str,
    ) {
        if selection.type_reference.is_valid() {
            collect_bounded_type_site(report, syntax, selection.type_reference, owner);
        }
        if let Some(application) = &selection.application {
            for argument in &application.arguments {
                collect_bounded_static_selection(report, syntax, argument, owner);
            }
            if let Some(member) = syntax.expressions.static_member(application.member) {
                collect_bounded_static_selection(report, syntax, member, owner);
            }
        }
    }

    fn static_selection_name(
        syntax: &SyntaxTrees,
        selection: &syntax_trees::expression::StaticMachineArgument,
    ) -> String {
        if selection.type_reference.is_valid() {
            return type_reference_name(syntax, selection.type_reference);
        }
        if let Some(literal) = &selection.const_literal {
            return literal.to_string();
        }
        if let Some(projection) = &selection.evidence_projection {
            return format!(
                "{}.{}",
                projection.term.as_str(syntax),
                projection.member.as_str(syntax)
            );
        }
        let mut name = selection
            .path
            .iter()
            .map(|part| part.as_str(syntax))
            .collect::<Vec<_>>()
            .join("::");
        if let Some(application) = &selection.application {
            let arguments = application
                .lifetime_arguments
                .iter()
                .map(|region| format!("'{}", region.as_str(syntax)))
                .chain(
                    application
                        .arguments
                        .iter()
                        .map(|argument| static_selection_name(syntax, argument)),
                )
                .collect::<Vec<_>>()
                .join(", ");
            name.push_str(&format!("<{arguments}>"));
            if let Some(member) = syntax.expressions.static_member(application.member) {
                name.push_str("::");
                name.push_str(&static_selection_name(syntax, member));
            }
        }
        name
    }

    fn type_reference_name(
        syntax_trees: &SyntaxTrees,
        type_reference: TypeReferenceHandle,
    ) -> String {
        match syntax_trees.type_references.type_reference(type_reference) {
            TypeReferenceNode::Callable { signature } => {
                let signature = syntax_trees.items.state_signature(*signature);
                let parameters = syntax_trees
                    .items
                    .state_parameters(signature.parameters)
                    .iter()
                    .map(|parameter| {
                        let parameter = syntax_trees.items.state_parameter(*parameter);
                        format!(
                            "{}: {}",
                            parameter.name.as_str(syntax_trees),
                            type_reference_name(syntax_trees, parameter.type_reference)
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                format!(
                    "machine({parameters}) -> {}",
                    type_reference_name(syntax_trees, signature.return_type)
                )
            }
            TypeReferenceNode::Reference {
                referee, access, ..
            } => {
                let qualifier = access.borrow_prefix();
                format!("{qualifier}{}", type_reference_name(syntax_trees, *referee))
            }
            TypeReferenceNode::Constrained { base_type, .. } => {
                type_reference_name(syntax_trees, *base_type)
            }
            TypeReferenceNode::FixedArray {
                element_type,
                length,
            } => {
                format!(
                    "[{}; {}]",
                    type_reference_name(syntax_trees, *element_type),
                    length.display_name(syntax_trees)
                )
            }
            TypeReferenceNode::Slice { element_type } => {
                format!("[{}]", type_reference_name(syntax_trees, *element_type))
            }
            TypeReferenceNode::Generic {
                base_name,
                arguments,
                ..
            } => {
                let arguments = syntax_trees
                    .type_references
                    .type_reference_handles(*arguments)
                    .iter()
                    .map(|argument| type_reference_name(syntax_trees, *argument))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!(
                    "{base_name}<{arguments}>",
                    base_name = base_name.as_str(syntax_trees)
                )
            }
            TypeReferenceNode::StaticSelection(selection) => {
                static_selection_name(syntax_trees, selection)
            }
            TypeReferenceNode::ConstExpression(expression) => {
                format!(
                    "const {}",
                    syntax_trees.display_expression_name(*expression)
                )
            }
            TypeReferenceNode::DynamicTrait {
                trait_name,
                trait_arguments,
            } => {
                let arguments = syntax_trees
                    .type_references
                    .type_reference_handles(*trait_arguments)
                    .iter()
                    .map(|argument| type_reference_name(syntax_trees, *argument))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!(
                    "dyn {trait_name}<{arguments}>",
                    trait_name = trait_name.as_str(syntax_trees)
                )
            }
            TypeReferenceNode::Named(name) => name.as_str(syntax_trees).to_owned(),
            TypeReferenceNode::Unit => "()".to_owned(),
        }
    }

    fn constraint_handle_name(
        syntax_trees: &SyntaxTrees,
        constraints: arena::HandleSpan<TypeConstraintNode>,
    ) -> String {
        let constraints = syntax_trees.type_references.constraints(constraints);
        if constraints.is_empty() {
            return "[]".to_owned();
        }

        let mut output = String::new();
        output.push('[');

        for (index, constraint) in constraints.iter().enumerate() {
            if index > 0 {
                output.push_str(", ");
            }

            output.push_str(&match constraint {
                TypeConstraintNode::Named(name) => name.as_str(syntax_trees).to_owned(),
                TypeConstraintNode::Domain(name) => {
                    format!("in {}", name.name.as_str(syntax_trees))
                }

                TypeConstraintNode::ArithmeticDomain(domain) => domain.name().to_owned(),
            });
        }

        output.push(']');
        output
    }

    fn operator_name(syntax_trees: &SyntaxTrees, name: arena::HandleSpan<Identifier>) -> String {
        syntax_trees
            .items
            .identifier_path_members(name)
            .joined_path_in(syntax_trees)
    }
}
pub(crate) mod typed_expressions {
    use symbols::SymbolHandle;
    use typed_trees::TypedTreeTables;
    use typed_trees::data::TypeParameterKind;
    use typed_trees::expression;
    use typed_trees::expression::{ExpressionHandle, ExpressionNode, ExpressionTable};
    use typed_trees::machine::Machine;
    use typed_trees::name::Identifier;
    use typed_trees::state::State;
    use typed_trees::statement::StatementTable;
    use typed_trees::types::TypeReferenceNode;
    use typed_trees::types::{TypeReferenceHandle, TypeReferenceTable};

    pub(crate) fn expression_table(tables: &TypedTreeTables) -> &ExpressionTable {
        &tables.expression_table
    }

    pub(crate) fn statement_table(tables: &TypedTreeTables) -> &StatementTable {
        &tables.statement_table
    }

    pub(crate) fn type_reference_table(tables: &TypedTreeTables) -> &TypeReferenceTable {
        &tables.type_reference_table
    }

    pub(crate) fn expression_type_reference(
        program: &dyn ::validation::CheckingInput,
        machine: &Machine,
        state: &State,
        expression: ExpressionHandle,
    ) -> Option<TypeReferenceHandle> {
        if let Some(field_type) =
            ::validation::field_domains::attached_data_field_type(program, machine, expression)
        {
            return Some(field_type);
        }
        match program.expression_table.expression(expression) {
            ExpressionNode::Borrow(inner) => {
                expression_type_reference(program, machine, state, inner.target)
            }
            ExpressionNode::Name(path) if path.symbol.is_valid() => {
                type_reference_for_symbol(program, machine, state, path.symbol)
            }
            ExpressionNode::Member(member) => {
                expression_type_reference(program, machine, state, member.receiver)
                    .and_then(|receiver_type| {
                        data_field_type_reference(
                            program,
                            receiver_type,
                            member.member_symbol,
                            &member.member,
                        )
                    })
                    .or_else(|| {
                        type_reference_for_symbol(program, machine, state, member.member_symbol)
                    })
            }
            ExpressionNode::Indexed(indexed) => {
                let collection_type =
                    expression_type_reference(program, machine, state, indexed.collection)?;
                element_type_reference(program, collection_type)
            }
            ExpressionNode::ZeroValue(type_reference) => Some(*type_reference),
            _ => None,
        }
    }

    fn element_type_reference(
        program: &dyn ::validation::CheckingInput,
        type_reference: TypeReferenceHandle,
    ) -> Option<TypeReferenceHandle> {
        if !type_reference.is_valid() {
            return None;
        }
        match program.type_reference_table.type_reference(type_reference) {
            TypeReferenceNode::Reference { referee, .. } => {
                element_type_reference(program, *referee)
            }
            TypeReferenceNode::Constrained { base_type, .. } => {
                element_type_reference(program, *base_type)
            }
            TypeReferenceNode::FixedArray { element_type, .. } => Some(*element_type),
            TypeReferenceNode::Slice { element_type } => Some(*element_type),
            _ => None,
        }
    }

    pub(crate) fn dehoisted_operand(
        program: &dyn ::validation::CheckingInput,
        state: &State,
        operand: ExpressionHandle,
    ) -> ExpressionHandle {
        match dehoisted_initializer(program, state, operand) {
            Some(initializer)
                if matches!(
                    program.expression_table.expression(initializer),
                    ExpressionNode::Indexed(_)
                        | ExpressionNode::Member(_)
                        | ExpressionNode::Name(_)
                ) =>
            {
                initializer
            }
            _ => operand,
        }
    }

    pub(crate) fn dehoisted_condition(
        program: &dyn ::validation::CheckingInput,
        state: &State,
        condition: ExpressionHandle,
    ) -> ExpressionHandle {
        dehoisted_initializer(program, state, condition).unwrap_or(condition)
    }

    fn dehoisted_initializer(
        program: &dyn ::validation::CheckingInput,
        state: &State,
        operand: ExpressionHandle,
    ) -> Option<ExpressionHandle> {
        let path = program.expression_table.name_path_node(operand)?;
        if !path.symbol.is_valid() {
            return None;
        }
        let (_, local) = program.state_local_declaration(state, path.symbol)?;
        if !local.initial_value.is_valid()
            || expression_contains_call_node(program, local.initial_value)
        {
            return None;
        }
        let mut initializer = local.initial_value;
        initializer = expression_table(program).peeled_expression_handle(initializer);
        Some(initializer)
    }

    fn type_reference_for_symbol(
        program: &dyn ::validation::CheckingInput,
        machine: &Machine,
        state: &State,
        symbol: SymbolHandle,
    ) -> Option<TypeReferenceHandle> {
        program
            .state_parameter_by_symbol(state, symbol)
            .map(|parameter| parameter.type_reference)
            .or_else(|| {
                program
                    .machine_type_parameters(machine)
                    .iter()
                    .find(|parameter| parameter.symbol == symbol)
                    .and_then(|parameter| match parameter.kind {
                        TypeParameterKind::Const { type_reference }
                        | TypeParameterKind::Value { type_reference } => Some(type_reference),
                        _ => None,
                    })
            })
            .or_else(|| {
                program
                    .state_local_declaration(state, symbol)
                    .map(|(_, local_data)| local_data.type_reference)
            })
            .or_else(|| {
                program
                    .machine_owned_data_by_symbol(machine, symbol)
                    .map(|owned_data| owned_data.type_reference)
            })
            .or_else(|| {
                program
                    .data_definitions()
                    .iter()
                    .find_map(|data_definition| {
                        program
                            .data_members(data_definition)
                            .iter()
                            .find_map(|member| {
                                let field = member.as_field()?;

                                (field.symbol == symbol).then_some(field.type_reference)
                            })
                    })
            })
    }

    fn data_field_type_reference(
        program: &dyn ::validation::CheckingInput,
        type_reference: TypeReferenceHandle,
        member_symbol: SymbolHandle,
        member_name: &Identifier,
    ) -> Option<TypeReferenceHandle> {
        ::validation::field_domains::data_field_type_reference_by_symbol_or_name(
            program,
            type_reference,
            member_symbol,
            member_name,
            ::validation::field_domains::data_definition_by_symbol_or_name,
        )
    }

    pub(crate) fn call_expression_return_type(
        program: &dyn ::validation::CheckingInput,
        _machine: &Machine,
        _state: &State,
        call: &expression::TableCallExpression,
    ) -> Option<TypeReferenceHandle> {
        callable_return_type_by_symbol(program, call.target_symbol)
    }

    fn callable_return_type_by_symbol(
        program: &dyn ::validation::CheckingInput,
        target_symbol: SymbolHandle,
    ) -> Option<TypeReferenceHandle> {
        if !target_symbol.is_valid() {
            return None;
        }

        program
            .machines()
            .iter()
            .flat_map(|machine| program.machine_states(machine).iter())
            .find(|candidate| candidate.symbol == target_symbol)
            .and_then(|candidate| {
                candidate
                    .return_type
                    .is_valid()
                    .then_some(candidate.return_type)
            })
            .or_else(|| {
                program
                    .machine_parameter_signature(target_symbol)
                    .and_then(|(_, candidate)| {
                        candidate
                            .return_type
                            .is_valid()
                            .then_some(candidate.return_type)
                    })
            })
    }

    pub(crate) fn expression_contains_call_node(
        program: &dyn ::validation::CheckingInput,
        expression: ExpressionHandle,
    ) -> bool {
        if !expression.is_valid() {
            return false;
        }
        match program.expression_table.expression(expression) {
            ExpressionNode::Match(dispatch) => {
                program.expression_table.match_deep_any(dispatch, |handle| {
                    expression_contains_call_node(program, handle)
                })
            }
            ExpressionNode::Call(_) => true,
            ExpressionNode::Binary(binary) => {
                expression_contains_call_node(program, binary.left)
                    || expression_contains_call_node(program, binary.right)
            }
            ExpressionNode::Unary(unary) => expression_contains_call_node(program, unary.operand),
            ExpressionNode::Cast(cast) => expression_contains_call_node(program, cast.value),
            ExpressionNode::Borrow(inner) => expression_contains_call_node(program, inner.target),
            ExpressionNode::Indexed(indexed) => {
                expression_contains_call_node(program, indexed.collection)
                    || expression_contains_call_node(program, indexed.index)
            }
            ExpressionNode::Member(member) => {
                expression_contains_call_node(program, member.receiver)
            }
            _ => false,
        }
    }
}

pub use proof_surface::{
    BoundedTypeSite, ContractKindSurface, ContractSurface, DomainSurface, ProofSurfaceReport,
    PropositionBodySurface, PropositionSurface, build_proof_surface_report,
};

#[cfg(test)]
pub(crate) fn integer_range(minimum: i64, maximum: i64) -> obligations::IntegerRange {
    obligations::IntegerRange {
        minimum: numerics::bignum::BigInt::from_i64(minimum),
        maximum: numerics::bignum::BigInt::from_i64(maximum),
    }
}

#[cfg(test)]
mod test_program {
    use crate::obligations::{
        BoundedValueObligation, ProofConstraint, ProofObligation, ProofObligationOwner,
    };
    use symbols::{SymbolHandle, SymbolKind, SymbolNameRef, SymbolTableBuilder};
    use typed_trees::TypedTrees;
    use typed_trees::name::Identifier;
    use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

    pub(crate) struct Program {
        pub(crate) typed_trees: TypedTrees,
        pub(crate) machine: SymbolHandle,
        pub(crate) data: [SymbolHandle; 2],
        pub(crate) int_type: SymbolHandle,
    }

    pub(crate) fn program() -> Program {
        let mut builder = SymbolTableBuilder::new();
        let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
        let machine = SymbolTableBuilder::child_handles(
            builder.insert_children(root, [(SymbolKind::Machine, SymbolNameRef::Static("Main"))]),
        )
        .next()
        .expect("machine");
        let members = SymbolTableBuilder::child_handles(builder.insert_children(
            machine,
            [
                (SymbolKind::Data, SymbolNameRef::Static("count")),
                (SymbolKind::Data, SymbolNameRef::Static("total")),
                (SymbolKind::BuiltinType, SymbolNameRef::Static("Int")),
            ],
        ))
        .collect::<Vec<_>>();
        Program {
            typed_trees: TypedTrees {
                symbols: builder.finish(),
                ..TypedTrees::default()
            },
            machine,
            data: [members[0], members[1]],
            int_type: members[2],
        }
    }

    impl Program {
        pub(crate) fn count_value(&mut self) -> (TypeReferenceHandle, ProofObligation) {
            let base = self.int_reference();
            let obligation = main_owned_value(self.machine, self.data[0], "count", base);
            (base, obligation)
        }

        pub(crate) fn data_value(
            &self,
            data: usize,
            name: &str,
            base: TypeReferenceHandle,
        ) -> ProofObligation {
            main_owned_value(self.machine, self.data[data], name, base)
        }

        pub(crate) fn int_reference(&mut self) -> TypeReferenceHandle {
            self.typed_trees
                .type_reference_table
                .insert(TypeReferenceNode::Named {
                    symbol: self.int_type,
                    name: Identifier::generated("Int"),
                })
        }
    }

    pub(crate) fn main_owned_value(
        machine: SymbolHandle,
        data_symbol: SymbolHandle,
        data_name: &str,
        base: TypeReferenceHandle,
    ) -> ProofObligation {
        bounded_value(
            machine,
            "Main",
            data_symbol,
            data_name,
            base,
            arena::HandleSpan::empty(),
        )
    }

    pub(crate) fn bounded_value(
        machine: SymbolHandle,
        machine_name: &str,
        data_symbol: SymbolHandle,
        data_name: &str,
        base: TypeReferenceHandle,
        constraints: arena::HandleSpan<ProofConstraint>,
    ) -> ProofObligation {
        ProofObligation::BoundedValue(BoundedValueObligation {
            owner: ProofObligationOwner::MachineOwnedData {
                machine_symbol: machine,
                machine: Identifier::generated(machine_name),
                data_symbol,
                data: Identifier::generated(data_name),
            },
            base_type: base,
            constraints,
        })
    }
}

#[cfg(test)]
mod tests {
    use arena::HandleSpan;
    use syntax_trees::SyntaxTrees;
    use syntax_trees::identifier::Identifier;
    use syntax_trees::item::{
        CapabilityContract, CapabilityContractKind, DomainDefinition, Item, Machine,
        OperatorDefinition, State, StateParameterNode,
    };
    use syntax_trees::types::{TypeConstraintNode, TypeReferenceNode};

    use crate::build_proof_surface_report;
    use language_semantics::DomainPredicatePresence;

    #[test]
    fn collects_domain_surface() {
        let mut syntax_trees = SyntaxTrees::new(Default::default());
        let target_type = syntax_trees
            .type_references
            .insert(TypeReferenceNode::Named(Identifier::generated("String")));

        syntax_trees.push_root_item(Item::Domain(DomainDefinition {
            name: Identifier::generated("NonEmpty"),
            type_parameters: HandleSpan::empty(),
            target_type,
            index_arguments: HandleSpan::empty(),
            is_public: false,
            alias: None,
            cleanup_selection: None,
            authored_routes: Vec::new(),
            classification: None,
            predicate_presence: DomainPredicatePresence::Absent,
            facts: HandleSpan::empty(),
            operators: HandleSpan::empty(),
            semantic_clause_token_count: 3,
        }));

        let report = build_proof_surface_report(&syntax_trees);

        assert_eq!(report.domains.len(), 1);
        let (_, domain) = report.domains.iter().next().expect("domain surface");
        assert_eq!(domain.name, "NonEmpty");
        assert_eq!(domain.target_type, "String");
        assert_eq!(domain.predicate_presence, DomainPredicatePresence::Absent);
        assert_eq!(domain.fact_count, 0);
        assert_eq!(domain.membership_fact_count, 0);
        assert_eq!(domain.semantic_clause_token_count, 3);
    }

    #[test]
    fn collects_operator_contract_surface() {
        let mut syntax_trees = SyntaxTrees::new(Default::default());
        let operator_name = syntax_trees.items.insert_identifier_path_members([
            Identifier::generated("Slice"),
            Identifier::generated("index"),
        ]);
        let requires = syntax_trees
            .items
            .append_capability_contract(CapabilityContract {
                kind: CapabilityContractKind::Requires,
                keyword_source_span: None,
                binding: None,
                facts: HandleSpan::empty(),
                token_count: 3,
            });
        let ensures = syntax_trees
            .items
            .append_capability_contract(CapabilityContract {
                kind: CapabilityContractKind::Ensures,
                keyword_source_span: None,
                binding: None,
                facts: HandleSpan::empty(),
                token_count: 3,
            });
        let return_type = syntax_trees
            .type_references
            .insert_named(Identifier::generated("T"));
        syntax_trees.push_root_item(Item::Operator(OperatorDefinition {
            is_public: false,
            is_boundary: false,
            name: operator_name,
            lifetime_parameters: Vec::new(),
            type_parameters: HandleSpan::empty(),
            parameters: HandleSpan::empty(),
            return_type,
            contracts: HandleSpan::from_parts(
                requires,
                ensures
                    .arena_index()
                    .checked_sub(requires.arena_index())
                    .expect("contracts should be contiguous")
                    + 1,
            ),
            spelling: None,
            token_count: 1,
        }));

        let report = build_proof_surface_report(&syntax_trees);

        assert_eq!(report.contracts.len(), 2);
        assert!(
            report
                .contracts
                .iter()
                .all(|(_, contract)| { contract.owner == "operator `Slice::index`" })
        );
    }

    #[test]
    fn collects_bounded_type_sites() {
        let mut syntax_trees = SyntaxTrees::new(Default::default());
        let base_type = syntax_trees
            .type_references
            .insert_named(Identifier::generated("f32"));
        let constraint = syntax_trees
            .type_references
            .append_constraint(TypeConstraintNode::Named(Identifier::generated(
                "speed_range",
            )));
        let parameter_type = syntax_trees
            .type_references
            .insert_constrained(base_type, HandleSpan::from_parts(constraint, 1));
        let parameter = syntax_trees
            .items
            .insert_state_parameter_node(StateParameterNode {
                name: Identifier::generated("speed"),
                type_reference: parameter_type,
                is_const: false,
                is_mutable: false,
                relevance: Default::default(),
            });
        let parameter_handle = syntax_trees.items.append_state_parameter_handle(parameter);
        let state = syntax_trees.items.insert_state(&State {
            name: Identifier::generated("entry"),
            parameters: HandleSpan::from_parts(parameter_handle, 1),
            return_type: syntax_trees::types::TypeReferenceHandle::invalid(),
            contracts: HandleSpan::empty(),
            statements: HandleSpan::empty(),
        });
        let state_handle = syntax_trees.items.append_state_handle(state);
        syntax_trees.push_root_item(Item::Machine(Machine {
            nested_machines: HandleSpan::empty(),
            name: Identifier::generated("main"),
            generic_data_template: Default::default(),
            default_body_trait: Default::default(),
            attached_data: None,
            spelling: None,
            is_public: false,
            target: None,
            authored_target: None,
            boundary: false,
            bodyless: false,
            target_family_interface: false,
            lifetime_parameters: Vec::new(),
            type_parameters: HandleSpan::empty(),
            satisfies: HandleSpan::empty(),
            conformance_bounds: Vec::new(),
            terminates_guarantee: false,
            ranking_subjects: HandleSpan::empty(),
            ranking_view: HandleSpan::empty(),
            ranking_view_arguments: HandleSpan::empty(),
            ranking_range: syntax_trees::expression::ExpressionHandle::invalid(),
            service_reach_keyword_source_spans: Vec::new(),
            service_reach_is_installation_bound: false,
            service_reaches: HandleSpan::empty(),
            invokes: HandleSpan::empty(),
            suspends_keyword_source_spans: Vec::new(),
            blocks_keyword_source_spans: Vec::new(),
            suspends: false,
            blocks: false,
            admitted_same_stack_bytes: 0,
            declares_no_crash_route: false,
            contracts: HandleSpan::empty(),
            states: HandleSpan::from_parts(state_handle, 1),
        }));

        let report = build_proof_surface_report(&syntax_trees);

        assert_eq!(report.bounded_sites.len(), 1);

        let (_, bounded_site) = report
            .bounded_sites
            .iter()
            .next()
            .expect("bounded site should be collected");
        assert_eq!(bounded_site.base_type, "f32");
        assert_eq!(bounded_site.constraints, "[speed_range]");
    }
}
