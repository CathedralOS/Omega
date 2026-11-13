/*
    Symbol table construction and identity assignment.

    `assign_symbols` builds the table from every declaration, then assigns
    symbols to top-level references, type references, propositions, measures,
    contracts, domain facts, and statements. The seeded variant extends a
    retained base table with a later stratum in place. Static module calls
    normalize here once module namespaces exist.
*/

use std::sync::Arc;
use syntax_trees::identifier::JoinedSyntaxPath;

use source::SourceMap;
use symbol_resolved_trees::SymbolResolvedTrees;

#[path = "symbols_runtime_callable_types.rs"]
mod runtime_callable_types;
#[path = "symbols_symbol_table.rs"]
mod symbol_table;

use symbol_table::build_symbol_table;
use type_references::assign_type_reference_symbols;

mod contracts {
    use diagnostics::Diagnostic;
    use symbol_resolved_trees::SymbolResolvedTrees;
    use symbol_resolved_trees::signature::SignatureContractKind;
    use symbols::{SymbolHandle, SymbolTable};

    use super::expression_paths::resolve_expression_table_call_target_symbol;
    use super::expressions::{
        assign_member_symbol, assign_membership_symbol, assign_name_symbol,
        assign_struct_literal_symbols,
    };
    use super::scope::MachineScope;
    use super::targets::assign_static_argument_symbols;

    macro_rules! assign_calls {
        ($symbols:ident, $machine:ident, $parameters:ident, $state_symbol:ident, $expression_table:ident, $child_type_references:ident, $expression:expr) => {
            assign_contract_call_symbols(
                $symbols,
                $machine,
                $parameters,
                $state_symbol,
                $expression_table,
                $child_type_references,
                $expression,
            )
        };
    }

    /*
        Resolve contract value references, call targets, and static machine
        arguments in the owning callable's scope. Machine contracts use entry
        parameters; state contracts use only that state's explicit parameters.
        Operator contracts use their own declared operand and generic binders.
        Static-machine signatures use their own parameter children, at every nesting
        depth; their contracts must not remain name-only for downstream consumers.
        Membership domain names retain their separate declaration lookup.
    */
    pub(super) fn assign_contract_reference_symbols(
        program: &mut SymbolResolvedTrees,
        symbols: &SymbolTable,
    ) -> Result<(), Vec<Diagnostic>> {
        let conformance_scopes = super::scope::DeclarationTypeScopes::of(program)?;
        let runtime_scopes = program
            .tables
            .declarations
            .runtime_callable_contracts
            .iter()
            .map(|(handle, declaration)| {
                super::runtime_callable_types::lexical_parameters(
                    program,
                    &conformance_scopes,
                    declaration.lexical_owner,
                )
                .map(|parameters| (handle, parameters))
                .map_err(|error| vec![error])
            })
            .collect::<Result<Vec<_>, Vec<Diagnostic>>>()?;
        let attached_machines = super::scope::attached_machines(program);
        let trait_scopes = super::scope::TraitLexicalScopes::of(program);
        let SymbolResolvedTrees { roots, tables, .. } = program;
        let data_definitions = &roots.data_definitions;
        let data_members = &tables.declarations.data_members;
        let data_type_parameters = &tables.declarations.data_type_parameters;
        let machine_owned_data = &tables.declarations.machine_owned_data;
        let machine_state_handles = &tables.declarations.machine_state_handles;
        let machine_states = &tables.declarations.machine_states;
        let trait_machine_signatures = &tables.declarations.trait_machine_signatures;
        let state_parameters = &tables.declarations.state_parameters;
        let signature_contracts = &tables.declarations.signature_contracts;
        let proof_facts = &tables.declarations.proof_facts;
        let expression_table = &mut tables.bodies.expressions;
        let child_type_references = &mut tables.declarations.child_type_references;
        let scope_context = super::scope::ContractScopeContext {
            attached_machines: &attached_machines,
            resolution_indexes: &program.resolution_indexes,
            data_definitions,
            type_aliases: &roots.type_aliases,
            conformances: &roots.conformances,
            traits: &roots.traits,
            operand_owners: &tables.types.operand_lifetime_owners,
            data_members,
            data_type_parameters,
            data_payload_fields: &tables.declarations.data_payload_fields,
            type_constraints: &tables.types.constraints,
        };

        for machine in roots.machines.iter() {
            let data_definition = data_definitions
                .iter()
                .find(|definition| definition.symbol == machine.attached_data_symbol);
            let invocation_scope = super::scope::declaration_type_scope(
                data_type_parameters.span_or_empty(machine.type_parameters),
                conformance_scopes.parameters(machine.symbol),
            );
            let lexical_scope = super::scope::declaration_type_scope(
                &invocation_scope,
                data_definition.map_or(&[], |definition| {
                    data_type_parameters.span_or_empty(definition.type_parameters)
                }),
            );
            let scope = scope_context.scope(
                machine.symbol,
                trait_scopes.requirements_of(machine.trait_default_origin.declaring_trait),
                &lexical_scope,
                machine.attached_data_symbol,
                data_definition.map(|definition| data_members.span_or_empty(definition.members)),
                machine_owned_data.span_or_empty(machine.owned_data),
            );
            let states = machine_state_handles.span_or_empty(machine.states);
            let entry = states.first().map(|handle| machine_states.get(*handle));
            let entry_symbol = entry.map_or(SymbolHandle::invalid(), |state| state.symbol);
            let entry_parameters = entry
                .map(|state| state_parameters.span_or_empty(state.parameters))
                .unwrap_or_default();
            assign_callable_value_symbols(
                symbols,
                &scope,
                &[],
                entry_parameters,
                None,
                entry_symbol,
                machine.contracts,
                signature_contracts,
                proof_facts,
                expression_table,
                child_type_references,
            );

            for handle in states {
                let state = machine_states.get(*handle);
                assign_callable_value_symbols(
                    symbols,
                    &scope,
                    &[],
                    state_parameters.span_or_empty(state.parameters),
                    state.return_type.as_ref(),
                    state.symbol,
                    state.contracts,
                    signature_contracts,
                    proof_facts,
                    expression_table,
                    child_type_references,
                );
            }
        }

        for trait_definition in roots.traits.iter() {
            let scope = scope_context.scope(
                trait_definition.symbol,
                trait_scopes.requirements_of(trait_definition.symbol),
                data_type_parameters.span_or_empty(trait_definition.type_parameters),
                SymbolHandle::invalid(),
                None,
                &[],
            );
            for signature in trait_machine_signatures
                .span_or_empty(trait_definition.machines)
                .iter()
                .chain(
                    trait_definition
                        .refinement_clauses
                        .iter()
                        .map(|clause| &clause.signature),
                )
            {
                assign_callable_value_symbols(
                    symbols,
                    &scope,
                    data_type_parameters.span_or_empty(signature.type_parameters),
                    state_parameters.span_or_empty(signature.parameters),
                    signature.return_type.as_ref(),
                    signature.symbol,
                    signature.contracts,
                    signature_contracts,
                    proof_facts,
                    expression_table,
                    child_type_references,
                );
            }
        }

        for operator in roots
            .operators
            .iter()
            .chain(roots.domain_definitions.iter().flat_map(|domain| {
                tables
                    .declarations
                    .operator_definitions
                    .span_or_empty(domain.operators)
            }))
        {
            let scope = scope_context.scope(
                operator.symbol,
                super::scope::LexicalRequirements::none(),
                data_type_parameters.span_or_empty(operator.type_parameters),
                SymbolHandle::invalid(),
                None,
                &[],
            );
            assign_callable_value_symbols(
                symbols,
                &scope,
                &[],
                state_parameters.span_or_empty(operator.parameters),
                operator.return_type.as_ref(),
                operator.symbol,
                operator.contracts,
                signature_contracts,
                proof_facts,
                expression_table,
                child_type_references,
            );
        }

        for (_, parameter) in data_type_parameters.iter() {
            let symbol_resolved_trees::data::TypeParameterKind::Machine { contract } =
                &parameter.kind
            else {
                continue;
            };
            let Some(signature) = contract.structural() else {
                continue;
            };
            let scope = scope_context.scope(
                signature.symbol,
                super::scope::LexicalRequirements::none(),
                data_type_parameters.span_or_empty(signature.type_parameters),
                SymbolHandle::invalid(),
                None,
                &[],
            );
            let owner_parameters = roots
                .machines
                .iter()
                .find_map(|machine| {
                    let parameters = data_type_parameters.span_or_empty(machine.type_parameters);
                    parameters
                        .iter()
                        .any(|candidate| {
                            candidate.symbol.is_valid() && candidate.symbol == parameter.symbol
                        })
                        .then(|| parameters.to_vec())
                })
                .unwrap_or_default();
            assign_callable_value_symbols(
                symbols,
                &scope,
                &owner_parameters,
                state_parameters.span_or_empty(signature.parameters),
                signature.return_type.as_ref(),
                signature.symbol,
                signature.contracts,
                signature_contracts,
                proof_facts,
                expression_table,
                child_type_references,
            );
        }
        for (handle, parameters) in &runtime_scopes {
            let declaration = tables.declarations.runtime_callable_contracts.get(*handle);
            let signature = &declaration.signature;
            let scope = scope_context.scope(
                signature.symbol,
                super::scope::LexicalRequirements::none(),
                parameters,
                SymbolHandle::invalid(),
                None,
                &[],
            );
            assign_callable_value_symbols(
                symbols,
                &scope,
                data_type_parameters.span_or_empty(signature.type_parameters),
                state_parameters.span_or_empty(signature.parameters),
                signature.return_type.as_ref(),
                signature.symbol,
                signature.contracts,
                signature_contracts,
                proof_facts,
                expression_table,
                child_type_references,
            );
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn assign_callable_value_symbols(
        symbols: &SymbolTable,
        machine: &MachineScope<'_>,
        signature_type_parameters: &[symbol_resolved_trees::data::TypeParameter],
        parameters: &[symbol_resolved_trees::signature::StateParameter],
        return_type: Option<&symbol_resolved_trees::types::TypeReference>,
        state_symbol: SymbolHandle,
        contracts: arena::HandleSpan<symbol_resolved_trees::signature::SignatureContract>,
        signature_contracts: &arena::Arena<symbol_resolved_trees::signature::SignatureContract>,
        proof_facts: &arena::Arena<symbol_resolved_trees::domain::ProofFact>,
        expression_table: &mut symbol_resolved_trees::expression::ExpressionTable,
        child_type_references: &mut arena::Arena<symbol_resolved_trees::types::TypeReference>,
    ) {
        let type_parameters = super::scope::declaration_type_scope(
            signature_type_parameters,
            machine.type_parameters,
        );
        let contract_scope = &MachineScope {
            type_parameters: &type_parameters,
            ..*machine
        };
        for reference in parameters
            .iter()
            .map(|parameter| &parameter.type_reference)
            .chain(return_type)
        {
            let mut reference = reference.clone();
            crate::symbols::type_references::assign_type_value_expression_symbols(
                symbols,
                contract_scope,
                parameters,
                state_symbol,
                expression_table,
                child_type_references,
                &mut reference,
            );
        }
        for contract in signature_contracts.span_or_empty(contracts) {
            for fact in proof_facts.span_or_empty(contract.facts) {
                let expression = match fact {
                    symbol_resolved_trees::domain::ProofFact::Expression(expression) => *expression,
                    symbol_resolved_trees::domain::ProofFact::Membership(membership) => {
                        crate::symbols::type_references::assign_type_reference_argument_symbols_with_constraints(
                        symbols,
                        child_type_references,
                        machine.type_constraints,
                        contract_scope.type_parameters,
                        membership.domain_arguments,
                    );
                        membership.value
                    }
                };
                assign_calls!(
                    symbols,
                    contract_scope,
                    parameters,
                    state_symbol,
                    expression_table,
                    child_type_references,
                    expression
                );
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn assign_contract_call_symbols(
        symbols: &SymbolTable,
        machine: &MachineScope<'_>,
        parameters: &[symbol_resolved_trees::signature::StateParameter],
        state_symbol: SymbolHandle,
        expression_table: &mut symbol_resolved_trees::expression::ExpressionTable,
        child_type_references: &mut arena::Arena<symbol_resolved_trees::types::TypeReference>,
        expression: symbol_resolved_trees::expression::ExpressionHandle,
    ) {
        use symbol_resolved_trees::expression::ExpressionNode;

        if !expression.is_valid() {
            return;
        }

        macro_rules! assign_calls {
            ($expression:expr) => {
                assign_contract_call_symbols(
                    symbols,
                    machine,
                    parameters,
                    state_symbol,
                    expression_table,
                    child_type_references,
                    $expression,
                )
            };
        }

        match expression_table.expression(expression).clone() {
            ExpressionNode::Match(dispatch) => {
                assign_calls!(dispatch.subject);
                for arm in expression_table.match_arms(dispatch.arms).to_vec() {
                    for pattern in expression_table
                        .match_pattern_dependencies(arm.pattern)
                        .collect::<Vec<_>>()
                    {
                        assign_calls!(pattern);
                    }
                    if let symbol_resolved_trees::expression::MatchPattern::Destructure(pattern) =
                        arm.pattern
                    {
                        super::expression_paths::assign_pattern_field_symbols(
                            symbols,
                            expression_table,
                            pattern,
                        );
                    }
                    assign_calls!(arm.value);
                }
            }
            ExpressionNode::SavedSubject(_) => {}
            ExpressionNode::ArrayLiteral(values) => {
                let values = expression_table.expression_handles(values).to_vec();
                for value in values {
                    assign_calls!(value);
                }
            }
            ExpressionNode::Atomic(atomic) => {
                assign_calls!(atomic.value);
            }
            ExpressionNode::Binary(binary) => {
                for child in [binary.left, binary.right] {
                    assign_calls!(child);
                }
            }
            ExpressionNode::Cast(cast) => {
                assign_calls!(cast.value);
                crate::symbols::type_references::reassign_type_reference_symbols_and_values(
                    symbols,
                    machine,
                    parameters,
                    state_symbol,
                    expression_table,
                    child_type_references,
                    cast.target_type,
                );
                for handle in cast.semantic_domain_arguments.handles() {
                    crate::symbols::type_references::reassign_type_reference_symbols(
                        symbols,
                        child_type_references,
                        machine.type_parameters,
                        handle,
                    );
                }
            }
            ExpressionNode::Call(call) => {
                assign_calls!(call.callee);
                if call.requirement_application.is_valid() {
                    super::type_references::reassign_type_reference_symbols_and_values(
                        symbols,
                        machine,
                        parameters,
                        state_symbol,
                        expression_table,
                        child_type_references,
                        call.requirement_application,
                    );
                }
                assign_calls!(call.receiver);
                let arguments = expression_table.expression_handles(call.arguments).to_vec();
                for argument in arguments {
                    assign_calls!(argument);
                }
                let target_symbol = resolve_expression_table_call_target_symbol(
                    machine,
                    parameters,
                    state_symbol,
                    &call,
                    expression_table,
                    child_type_references,
                    symbols,
                );
                crate::symbols::expressions::assign_static_arguments_type_symbols(
                    symbols,
                    machine,
                    parameters,
                    state_symbol,
                    expression_table,
                    child_type_references,
                    &call.machine_arguments,
                );
                if let ExpressionNode::Call(call) = expression_table.expression_mut(expression) {
                    call.target_symbol = target_symbol;
                    let proof_static =
                        super::targets::proof_static_call_target(symbols, target_symbol);
                    for argument in &mut call.machine_arguments {
                        assign_static_argument_symbols(
                            symbols,
                            machine.symbol,
                            machine.type_parameters,
                            argument,
                            proof_static,
                        );
                    }
                }
            }
            ExpressionNode::Indexed(indexed) => {
                for child in [indexed.collection, indexed.index] {
                    assign_calls!(child);
                }
            }
            ExpressionNode::Member(member) => {
                assign_calls!(member.receiver);
                assign_member_symbol(
                    symbols,
                    machine,
                    state_symbol,
                    expression_table,
                    member.receiver,
                    &member.member,
                    expression,
                );
            }
            ExpressionNode::Membership(membership) => {
                assign_calls!(membership.value);
                assign_membership_symbol(symbols, expression_table, membership.domain, expression);
            }
            ExpressionNode::Borrow(inner) => assign_calls!(inner.target),
            ExpressionNode::Range(range) => {
                for child in [range.start, range.end] {
                    assign_calls!(child);
                }
            }
            ExpressionNode::StructLiteral(literal) => {
                let fields = expression_table.struct_fields(literal.fields).to_vec();
                for field in fields {
                    assign_calls!(field.value);
                }
                assign_struct_literal_symbols(symbols, expression_table, expression);
            }
            ExpressionNode::Unary(unary) => assign_calls!(unary.operand),
            ExpressionNode::TypeExpression(type_reference)
            | ExpressionNode::ZeroValue(type_reference) => {
                crate::symbols::type_references::reassign_type_reference_symbols_and_values(
                    symbols,
                    machine,
                    parameters,
                    state_symbol,
                    expression_table,
                    child_type_references,
                    type_reference,
                );
            }
            ExpressionNode::Name(path) => {
                if matches!(expression_table.name_path_members(path.members), [name]
                if language_core::is_reserved_result(name.as_str()))
                    && language_core::signature_reserves_result(
                        parameters.iter().map(|parameter| parameter.name.as_str()),
                    )
                {
                    // This is the one place the reserved result is told apart
                    // from a parameter or local spelled `result`: the source
                    // rule is a spelling, decided here once. Record the
                    // decision as the contract owner's identity so later
                    // stages need not repeat the spelling test; the symbol
                    // stays invalid, as every existing reader expects.
                    if let ExpressionNode::Name(resolved) =
                        expression_table.expression_mut(expression)
                    {
                        resolved.reserved_result_of = state_symbol;
                    }
                    return;
                }
                assign_name_symbol(
                    symbols,
                    machine,
                    parameters,
                    state_symbol,
                    expression_table,
                    &path,
                    expression,
                )
            }
            ExpressionNode::Boolean(_)
            | ExpressionNode::Float(_)
            | ExpressionNode::UndeterminedNaN(_)
            | ExpressionNode::Integer(_)
            | ExpressionNode::String(_) => {}
        }
    }

    /*
        Bind every pending outcome-specific contract to the result data and case
        it names, once top-level symbols exist.
    */
    pub(crate) fn finalize_outcome_specific_contract_symbols(
        program: &mut symbol_resolved_trees::SymbolResolvedTrees,
        pending: &[crate::resolution::lowerer::PendingOutcomeSpecificContract],
    ) -> Result<(), Diagnostic> {
        for pending in pending {
            let (result_data, result_case) = {
                let data_symbol = program
                    .symbols
                    .find_top_level_by_name_and_kinds_from_source(
                        &pending.result_data_name,
                        &[symbols::SymbolKind::Data],
                        pending.result_data_source_span,
                    )
                    .unwrap_or_else(SymbolHandle::invalid);
                let data = program.data_definition_by_symbol(data_symbol)
                .ok_or_else(|| {
                    Diagnostic::error(format!(
                        "outcome-specific ensures lost declared result sum `{}` during symbol assignment",
                        pending.result_data_name
                    ))
                })?;
                let result_case = program
                .data_members(data.members)
                .iter()
                .find_map(|member| match member {
                    symbol_resolved_trees::data::DataMember::Variant(variant)
                        if variant.name.as_str() == pending.result_case_name =>
                    {
                        Some(variant.symbol)
                    }
                    _ => None,
                })
                .ok_or_else(|| {
                    Diagnostic::error(format!(
                        "outcome-specific ensures lost declared result case `{}::{}` during symbol assignment",
                        pending.result_data_name, pending.result_case_name
                    ))
                })?;
                (data.symbol, result_case)
            };
            let contract = program
                .tables
                .declarations
                .signature_contracts
                .get_mut(pending.contract);
            let SignatureContractKind::EnsuresForResultCase {
                result_data: contract_data,
                result_case: contract_case,
            } = &mut contract.kind
            else {
                return Err(Diagnostic::error(
                    "outcome-specific ensures changed kind before symbol normalization",
                ));
            };
            *contract_data = result_data;
            *contract_case = result_case;
        }
        Ok(())
    }
}
mod domain_facts {
    /*
        Declaration-scoped facts bind lexical subjects before selecting remaining
        closed type names. Complete source-visible paths retain semantic namespace
        ancestry, including declarations physically stored at root, so authored
        selection custody survives generic equation discharge.
    */
    use crate::tables::body_expressions;
    use crate::tables::resolved_declarations;
    use language_semantics::byte_predicates::ByteSequencePredicate;
    use symbol_resolved_trees::SymbolResolvedTrees;
    use symbols::{JoinedPath, SymbolHandle, SymbolKind, SymbolTable};

    use super::expressions::assign_membership_symbol;
    use super::expressions::assign_struct_literal_symbols;
    use super::lookup::diagnostic_path_source_span;
    use super::lookup::domain_name_reaches;
    use super::targets::resolve_free_call_target_symbol;
    use language_semantics::declaration_selection::CollectionMeasure;
    use std::iter::once;
    use symbol_resolved_trees::data::DataDefinition;
    use symbol_resolved_trees::data::DataMember;
    use symbol_resolved_trees::data::TypeParameter;
    use symbol_resolved_trees::data::TypeParameterKind;
    use symbol_resolved_trees::domain::ProofFact;
    use symbol_resolved_trees::expression::BinaryOperator;
    use symbol_resolved_trees::expression::ExpressionHandle;
    use symbol_resolved_trees::expression::ExpressionNode;
    use symbol_resolved_trees::expression::ExpressionTable;
    use symbol_resolved_trees::expression::TableNamePath;
    use symbol_resolved_trees::name::DiagnosticName;
    use symbol_resolved_trees::types::TypeConstraint;
    use symbol_resolved_trees::types::TypeReference;

    mod case_members {
        /*
            Guarded payload names retain the selected declaration, not proof of access.
            Specification formation independently checks the preceding conjunct and its
            exact receiver before admitting a payload projection.
        */

        use symbol_resolved_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
        use symbol_resolved_trees::name::DiagnosticName;
        use symbols::{SymbolHandle, SymbolKind};

        use super::ProofExpressionContext;

        pub(super) struct CaseSelection {
            subject: ExpressionHandle,
            variant: SymbolHandle,
        }

        impl ProofExpressionContext<'_> {
            pub(super) fn collect_case_selections(&mut self, expression: ExpressionHandle) {
                match self.expression_table.expression(expression).clone() {
                    ExpressionNode::Membership(membership) if membership.case_symbol.is_valid() => {
                        self.case_selections.push(CaseSelection {
                            subject: membership.value,
                            variant: membership.case_symbol,
                        });
                    }
                    ExpressionNode::Binary(binary) if binary.operator == BinaryOperator::And => {
                        self.collect_case_selections(binary.left);
                        self.collect_case_selections(binary.right);
                    }
                    _ => {}
                }
            }

            pub(super) fn assign_guarded_case_member(&mut self, expression: ExpressionHandle) {
                let ExpressionNode::Member(member) = self.expression_table.expression(expression)
                else {
                    return;
                };
                if member.member_symbol.is_valid() || member.case_variant.is_some() {
                    return;
                }
                let mut selections = self
                    .case_selections
                    .iter()
                    .filter(|selection| self.same_case_subject(selection.subject, member.receiver));
                let Some(selection) = selections.next() else {
                    return;
                };
                let variant = selection.variant;
                if selections.any(|other| other.variant != variant) {
                    return;
                }
                let Some(children) = self.symbols.child_handles(variant) else {
                    return;
                };
                let mut fields = children.filter(|symbol| {
                    self.symbols.get(*symbol).kind == SymbolKind::Field
                        && self.symbols.name(*symbol) == member.member.as_str()
                });
                let Some(field) = fields.next() else {
                    return;
                };
                if fields.next().is_some() {
                    return;
                }
                let case_name = DiagnosticName::generated(self.symbols.name(variant));
                if let ExpressionNode::Member(member) =
                    self.expression_table.expression_mut(expression)
                {
                    member.member_symbol = field;
                    member.case_variant = Some(case_name);
                }
            }

            /*
                Only exact resolved storage paths share a selection. Equal spellings or
                independently evaluated calls do not identify the same subject.
            */
            fn same_case_subject(&self, left: ExpressionHandle, right: ExpressionHandle) -> bool {
                match (
                    self.expression_table.expression(left),
                    self.expression_table.expression(right),
                ) {
                    (ExpressionNode::Name(left), ExpressionNode::Name(right)) => {
                        let left_members = self
                            .expression_table
                            .name_path_member_symbols(left.member_symbols);
                        let right_members = self
                            .expression_table
                            .name_path_member_symbols(right.member_symbols);
                        left.head_symbol.is_valid()
                            && left.symbol.is_valid()
                            && left.head_symbol == right.head_symbol
                            && left.symbol == right.symbol
                            && left_members.len() == left.members.count() as usize
                            && right_members.len() == right.members.count() as usize
                            && left_members.iter().all(|symbol| symbol.is_valid())
                            && left_members == right_members
                    }
                    (ExpressionNode::Member(left), ExpressionNode::Member(right)) => {
                        left.member_symbol.is_valid()
                            && left.member_symbol == right.member_symbol
                            && self.same_case_subject(left.receiver, right.receiver)
                    }
                    _ => false,
                }
            }
        }
    }

    pub(super) fn assign_domain_fact_symbols(
        program: &mut SymbolResolvedTrees,
        symbols: &SymbolTable,
    ) {
        let domain_symbols = program
            .domain_definitions
            .iter()
            .map(|domain| {
                (
                    domain.name.as_str().to_owned(),
                    domain.symbol,
                    domain.semantic_id,
                )
            })
            .collect::<Vec<_>>();
        let alias_symbols = program
            .domain_definitions
            .iter()
            .map(|domain| {
                domain
                    .alias
                    .as_ref()
                    .map(|alias| {
                        alias
                            .constituents
                            .iter()
                            .map(|constituent| {
                                let members = program.domain_path_members(constituent.domain);
                                let name = members.joined_path();
                                resolve_domain_symbol(
                                    symbols,
                                    &domain_symbols,
                                    &name,
                                    diagnostic_path_source_span(members),
                                )
                            })
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default()
            })
            .collect::<Vec<_>>();
        let mut alias_symbols = alias_symbols.into_iter();
        program.domain_definitions.for_each_mut(|domain| {
            let resolved = alias_symbols
                .next()
                .expect("one alias-resolution set per domain");
            let Some(alias) = domain.alias.as_mut() else {
                return;
            };
            for (constituent, symbol) in alias.constituents.iter_mut().zip(resolved) {
                constituent.domain_symbol = symbol;
            }
        });
        let mut proof_fact_scopes = program
            .domain_definitions
            .iter()
            .map(|domain| {
                let local_symbols = resolved_declarations(program)
                    .data_type_parameters
                    .span_or_empty(domain.type_parameters)
                    .iter()
                    .map(|parameter| (parameter.name.as_str().to_owned(), parameter.symbol))
                    .chain(once((
                        domain.subject_name.as_str().to_owned(),
                        domain.subject_symbol,
                    )))
                    .collect();
                (
                    domain.facts,
                    local_symbols,
                    Some(domain.type_parameters),
                    false,
                )
            })
            .collect::<Vec<_>>();
        proof_fact_scopes.extend(
            resolved_declarations(program)
                .signature_contracts
                .iter()
                .map(|(_, contract)| (contract.facts, Vec::new(), None, false)),
        );
        proof_fact_scopes.extend(program.data_definitions.iter().flat_map(|definition| {
            let descriptor_parameters = program
                .data_type_parameters(definition.type_parameters)
                .iter()
                .filter(|parameter| matches!(parameter.kind, TypeParameterKind::Value { .. }))
                .zip(program.data_members(definition.members))
                .filter_map(|(parameter, member)| match member {
                    DataMember::Field(field) if field.name == parameter.name => {
                        Some(parameter.symbol)
                    }
                    _ => None,
                })
                .collect::<Vec<_>>();
            let mut local_symbols = resolved_declarations(program)
                .data_type_parameters
                .span_or_empty(definition.type_parameters)
                .iter()
                .filter(|parameter| !descriptor_parameters.contains(&parameter.symbol))
                .map(|parameter| (parameter.name.as_str().to_owned(), parameter.symbol))
                .collect::<Vec<_>>();
            local_symbols.extend(
                resolved_declarations(program)
                    .data_members
                    .span_or_empty(definition.members)
                    .iter()
                    .filter_map(|member| match member {
                        DataMember::Field(field) => {
                            Some((field.name.as_str().to_owned(), field.symbol))
                        }
                        DataMember::Variant(_) => None,
                    }),
            );
            if definition.predicate_subject_symbol.is_valid() {
                local_symbols.push(("self".to_owned(), definition.predicate_subject_symbol));
            }
            let variant_scopes = resolved_declarations(program)
                .data_members
                .span_or_empty(definition.members)
                .iter()
                .filter_map(|member| match member {
                    DataMember::Variant(variant) if !variant.requires_facts.is_empty() => {
                        let mut scope = resolved_declarations(program)
                            .data_payload_fields
                            .span_or_empty(variant.payload)
                            .iter()
                            .map(|field| (field.name.as_str().to_owned(), field.symbol))
                            .collect::<Vec<_>>();
                        let inherited = local_symbols
                            .iter()
                            .filter(|(name, _)| {
                                !scope.iter().any(|(payload_name, _)| payload_name == name)
                            })
                            .cloned()
                            .collect::<Vec<_>>();
                        scope.extend(inherited);
                        Some((
                            variant.requires_facts,
                            scope,
                            Some(definition.type_parameters),
                            true,
                        ))
                    }
                    _ => None,
                })
                .collect::<Vec<_>>();
            once((
                definition.requires_facts,
                local_symbols,
                Some(definition.type_parameters),
                true,
            ))
            .chain(variant_scopes)
        }));
        let domain_path_members = &program.tables.declarations.domain_path_members;
        let data_type_parameters = &program.tables.declarations.data_type_parameters;
        let child_type_references = &mut program.tables.declarations.child_type_references;
        let type_constraints = &program.tables.types.constraints;
        let proof_facts = &mut program.tables.declarations.proof_facts;

        for (facts, local_symbols, type_parameters, logical_data_subject) in proof_fact_scopes {
            let local_type_parameters =
                type_parameters.map(|parameters| data_type_parameters.span_or_empty(parameters));
            for fact in proof_facts.span_mut_or_empty(facts) {
                match fact {
                    ProofFact::Membership(membership) => {
                        if let Some(type_parameters) = type_parameters {
                            let local_type_parameters =
                                data_type_parameters.span_or_empty(type_parameters);
                            super::type_references::assign_type_reference_argument_symbols_with_constraints(
                            symbols,
                            child_type_references,
                            type_constraints,
                            local_type_parameters,
                            membership.domain_arguments,
                        );
                        }
                        assign_proof_expression_symbols(
                            symbols,
                            &domain_symbols,
                            child_type_references,
                            type_constraints,
                            local_type_parameters,
                            &local_symbols,
                            logical_data_subject,
                            &mut program.tables.bodies.expressions,
                            membership.value,
                        );
                        let members = domain_path_members.span_or_empty(membership.domain);
                        let name = members.joined_path();
                        membership.domain_symbol = resolve_domain_symbol(
                            symbols,
                            &domain_symbols,
                            &name,
                            diagnostic_path_source_span(members),
                        );
                    }
                    ProofFact::Expression(expression) => {
                        assign_proof_expression_symbols(
                            symbols,
                            &domain_symbols,
                            child_type_references,
                            type_constraints,
                            local_type_parameters,
                            &local_symbols,
                            logical_data_subject,
                            &mut program.tables.bodies.expressions,
                            *expression,
                        );
                    }
                }
            }
        }

        update_data_zero_gates(program);
    }

    /*
        Resolve the conservative declaration gate after lexical subjects and domains
        are bound. Only stored fields are born zero; static binders remain unknown
        until typed application preparation retains their actual arguments.
    */
    fn update_data_zero_gates(program: &mut SymbolResolvedTrees) {
        let membership_results = program
            .data_definitions
            .iter()
            .map(|definition| {
                let facts = resolved_declarations(program)
                    .proof_facts
                    .span_or_empty(definition.requires_facts);
                let mut saw_membership = false;
                let mut memberships_admit_zero = true;
                for fact in facts {
                    let ProofFact::Membership(membership) = fact else {
                        continue;
                    };
                    saw_membership = true;
                    memberships_admit_zero &=
                        resolved_domain_byte_predicate(program, membership.domain_symbol)
                            .is_some_and(|predicate| predicate.holds_for(&[]));
                }
                let expressions_gate_zero = facts
                    .iter()
                    .filter_map(|fact| match fact {
                        ProofFact::Expression(expression) => Some(*expression),
                        ProofFact::Membership(_) => None,
                    })
                    .any(|expression| {
                        zero_fold(program, definition, expression).is_none_or(|value| value == 0)
                    });
                let first_variant_gates_zero = resolved_declarations(program)
                    .data_members
                    .span_or_empty(definition.members)
                    .iter()
                    .filter_map(|member| match member {
                        DataMember::Variant(variant) => Some(variant),
                        DataMember::Field(_) => None,
                    })
                    .next()
                    .is_some_and(|variant| {
                        resolved_declarations(program)
                            .proof_facts
                            .span_or_empty(variant.requires_facts)
                            .iter()
                            .any(|fact| match fact {
                                ProofFact::Expression(expression) => {
                                    fact_expression_is_constant_false(program, *expression)
                                }
                                ProofFact::Membership(_) => false,
                            })
                    });
                (
                    saw_membership,
                    memberships_admit_zero,
                    expressions_gate_zero,
                    first_variant_gates_zero,
                )
            })
            .collect::<Vec<_>>();

        let mut membership_results = membership_results.into_iter();
        program.data_definitions.for_each_mut(|definition| {
            let (
                saw_membership,
                memberships_admit_zero,
                expressions_gate_zero,
                first_variant_gates_zero,
            ) = membership_results
                .next()
                .expect("one zero result per data definition");
            definition.zero_gated = expressions_gate_zero
                || first_variant_gates_zero
                || (saw_membership && !memberships_admit_zero);
        });
    }

    fn zero_fold(
        program: &SymbolResolvedTrees,
        definition: &DataDefinition,
        expression: ExpressionHandle,
    ) -> Option<i128> {
        use symbol_resolved_trees::expression::{BinaryOperator, ExpressionNode};
        match program.tables.bodies.expressions.expression(expression) {
            ExpressionNode::Name(path) => resolved_declarations(program)
                .data_members
                .span_or_empty(definition.members)
                .iter()
                .any(|member| {
                    matches!(member, DataMember::Field(field)
                    if path.symbol.is_valid() && field.symbol == path.symbol)
                })
                .then_some(0),
            ExpressionNode::Member(member)
                if CollectionMeasure::from_authored_spelling(member.member.as_str()).is_some() =>
            {
                (zero_fold(program, definition, member.receiver)? == 0).then_some(0)
            }
            ExpressionNode::Integer(literal) => literal.decimal_value_i128(),
            ExpressionNode::Binary(binary) => {
                let left = zero_fold(program, definition, binary.left)?;
                let right = zero_fold(program, definition, binary.right)?;
                match binary.operator {
                    BinaryOperator::Add => left.checked_add(right),
                    BinaryOperator::Subtract => left.checked_sub(right),
                    BinaryOperator::Multiply => left.checked_mul(right),
                    BinaryOperator::LessOrEqual => Some(i128::from(left <= right)),
                    BinaryOperator::Less => Some(i128::from(left < right)),
                    BinaryOperator::GreaterOrEqual => Some(i128::from(left >= right)),
                    BinaryOperator::Greater => Some(i128::from(left > right)),
                    BinaryOperator::Equal => Some(i128::from(left == right)),
                    BinaryOperator::NotEqual => Some(i128::from(left != right)),
                    BinaryOperator::And => Some(i128::from(left != 0 && right != 0)),
                    BinaryOperator::Or => Some(i128::from(left != 0 || right != 0)),
                    _ => None,
                }
            }
            _ => None,
        }
    }

    /*
        A proof-fact expression that is a literal FALSE constant: the `0` witness
        synthesis leaves on an instance case whose `T == name` refuted, or an
        authored `where false` / `where 0`. Anything requiring evaluation is not
        counted -- an unprovable fact is not a contradiction.
    */
    fn fact_expression_is_constant_false(
        program: &SymbolResolvedTrees,
        expression: ExpressionHandle,
    ) -> bool {
        match program.tables.bodies.expressions.expression(expression) {
            ExpressionNode::Integer(literal) => literal.decimal_value_i128() == Some(0),
            ExpressionNode::Boolean(value) => !*value,
            _ => false,
        }
    }

    fn resolved_domain_byte_predicate(
        program: &SymbolResolvedTrees,
        domain_symbol: SymbolHandle,
    ) -> Option<ByteSequencePredicate> {
        let domain = program.domain_definition_by_symbol(domain_symbol)?;
        let [ProofFact::Expression(expression)] = program.proof_facts(domain.facts) else {
            return None;
        };
        let ExpressionNode::Call(call) = program.tables.bodies.expressions.expression(*expression)
        else {
            return None;
        };
        if call.receiver.is_valid() {
            return None;
        }
        let predicate = ByteSequencePredicate::from_name(call.target.as_str())?;
        let [argument] = body_expressions(program).expression_handles(call.arguments) else {
            return None;
        };
        let ExpressionNode::Name(path) = program.tables.bodies.expressions.expression(*argument)
        else {
            return None;
        };
        let members = body_expressions(program).name_path_members(path.members);
        matches!(members, [member] if member.is_self_receiver()).then_some(predicate)
    }

    /*
        One proof-surface expression walk: declaration predicates (membership
        values and `requires` facts) carry their declaration's generic scope, its
        lexical subjects, and the domain roster through every child visit. The
        walk carries that context instead of re-threading it through each
        recursive call.
    */
    struct ProofExpressionContext<'visit> {
        symbols: &'visit SymbolTable,
        domain_symbols: &'visit [(String, SymbolHandle, language_semantics::SemanticDomainId)],
        child_type_references: &'visit mut arena::Arena<TypeReference>,
        type_constraints: &'visit arena::Arena<TypeConstraint>,
        local_type_parameters: Option<&'visit [TypeParameter]>,
        local_symbols: &'visit [(String, SymbolHandle)],
        logical_data_subject: bool,
        expression_table: &'visit mut ExpressionTable,
        case_selections: Vec<case_members::CaseSelection>,
    }

    fn assign_proof_expression_symbols(
        symbols: &SymbolTable,
        domain_symbols: &[(String, SymbolHandle, language_semantics::SemanticDomainId)],
        child_type_references: &mut arena::Arena<TypeReference>,
        type_constraints: &arena::Arena<TypeConstraint>,
        local_type_parameters: Option<&[TypeParameter]>,
        local_symbols: &[(String, SymbolHandle)],
        logical_data_subject: bool,
        expression_table: &mut ExpressionTable,
        expression: ExpressionHandle,
    ) {
        ProofExpressionContext {
            symbols,
            domain_symbols,
            child_type_references,
            type_constraints,
            local_type_parameters,
            local_symbols,
            logical_data_subject,
            expression_table,
            case_selections: Vec::new(),
        }
        .assign(expression);
    }

    impl ProofExpressionContext<'_> {
        /// Declaration facts own static operands just as callable expressions do.
        /// The shared resolver owns type/static binders and source visibility;
        /// remaining value subjects use this declaration's exact lexical roster.
        fn assign_static_argument(
            &mut self,
            argument: &mut symbol_resolved_trees::expression::StaticMachineArgument,
        ) {
            if argument.type_reference.is_valid() {
                let mut reference = self
                    .child_type_references
                    .get(argument.type_reference)
                    .clone();
                super::type_references::assign_type_reference_symbol_with_locals_and_constraints(
                    self.symbols,
                    self.child_type_references,
                    self.type_constraints,
                    self.local_type_parameters.unwrap_or(&[]),
                    &mut reference,
                );
                *self.child_type_references.get_mut(argument.type_reference) = reference;
                argument.symbol = SymbolHandle::invalid();
            } else if !argument.symbol.is_valid()
                && argument.const_literal.is_none()
                && argument.evidence_projection.is_none()
            {
                super::targets::assign_static_argument_symbol(
                    self.symbols,
                    SymbolHandle::invalid(),
                    self.local_type_parameters.unwrap_or(&[]),
                    argument,
                    true,
                );
                let lexical = if !argument.symbol.is_valid()
                    && let [name] = argument.path.as_ref()
                {
                    let mut matching = self
                        .local_symbols
                        .iter()
                        .filter(|(candidate, _)| candidate == name.as_str());
                    match (matching.next(), matching.next()) {
                        (None, _) => None,
                        (Some((_, symbol)), None) => Some(*symbol),
                        (Some(_), Some(_)) => Some(SymbolHandle::invalid()),
                    }
                } else {
                    None
                };
                if let Some(symbol) = lexical {
                    argument.symbol = symbol;
                }
            }
            if let Some(application) = &mut argument.application {
                for nested in &mut application.arguments {
                    self.assign_static_argument(nested);
                }
            }
        }
        /// Bind the fact's lexical subject: the single local or fact-parameter
        /// whose spelling matches the path's leaf. Several candidates stay
        /// unresolved rather than exposing an outer declaration, and an
        /// already-bound path is left alone.
        fn assign_lexical_subject(
            &mut self,
            members: arena::HandleSpan<DiagnosticName>,
            member_symbols: arena::HandleSpan<SymbolHandle>,
            expression: ExpressionHandle,
        ) -> bool {
            let Some(name) = self.expression_table.name_path_members(members).first() else {
                return false;
            };
            let matching = {
                let mut matches = self
                    .local_symbols
                    .iter()
                    .filter(|(candidate, symbol)| candidate == name.as_str() && symbol.is_valid());
                match (matches.next(), matches.next()) {
                    (Some((_, symbol)), None) => Some(*symbol),
                    _ => None,
                }
            };
            let Some(symbol) = matching else {
                return false;
            };
            if let ExpressionNode::Name(path) = self.expression_table.expression_mut(expression) {
                path.head_symbol = symbol;
                if members.count() == 1 {
                    path.symbol = symbol;
                }
            }
            if member_symbols.count() != 0 {
                self.expression_table.set_name_path_member_symbol_at_offset(
                    member_symbols,
                    0,
                    symbol,
                );
            }
            true
        }

        fn assign(&mut self, expression: ExpressionHandle) {
            let expression_node = self.expression_table.expression(expression).clone();
            match expression_node {
                ExpressionNode::Match(dispatch) => {
                    self.assign(dispatch.subject);
                    for arm in self.expression_table.match_arms(dispatch.arms).to_vec() {
                        for pattern in self
                            .expression_table
                            .match_pattern_dependencies(arm.pattern)
                            .collect::<Vec<_>>()
                        {
                            self.assign(pattern);
                        }
                        if let symbol_resolved_trees::expression::MatchPattern::Destructure(
                            pattern,
                        ) = arm.pattern
                        {
                            super::expression_paths::assign_pattern_field_symbols(
                                self.symbols,
                                self.expression_table,
                                pattern,
                            );
                        }
                        self.assign(arm.value);
                    }
                }
                ExpressionNode::SavedSubject(_) => {}
                ExpressionNode::Atomic(atomic) => {
                    self.assign(atomic.value);
                    if atomic.result.is_valid() {
                        self.assign(atomic.result);
                    }
                }
                ExpressionNode::ArrayLiteral(values) => {
                    for value in self.expression_table.expression_handles(values).to_vec() {
                        self.assign(value);
                    }
                }
                ExpressionNode::Binary(binary) => {
                    self.assign(binary.left);
                    let selection_count = self.case_selections.len();
                    if binary.operator == BinaryOperator::And {
                        self.collect_case_selections(binary.left);
                    }
                    self.assign(binary.right);
                    self.case_selections.truncate(selection_count);
                }
                ExpressionNode::Call(call) => {
                    if call.receiver.is_valid() {
                        self.assign(call.receiver);
                    }
                    for argument in self
                        .expression_table
                        .expression_handles(call.arguments)
                        .to_vec()
                    {
                        self.assign(argument);
                    }
                    if !call.receiver.is_valid() && !call.target_symbol.is_valid() {
                        let mut lexical_matches = self
                            .local_symbols
                            .iter()
                            .filter(|(name, _)| name == call.target.as_str());
                        let target_symbol = match (lexical_matches.next(), lexical_matches.next()) {
                            (Some((_, symbol)), None) => *symbol,
                            (Some(_), Some(_)) => SymbolHandle::invalid(),
                            (None, _) => {
                                resolve_free_call_target_symbol(self.symbols, &call.target)
                            }
                        };
                        if let ExpressionNode::Call(call) =
                            self.expression_table.expression_mut(expression)
                        {
                            call.target_symbol = target_symbol;
                        }
                    }
                    let mut arguments = call.machine_arguments;
                    for argument in &mut arguments {
                        self.assign_static_argument(argument);
                    }
                    if let ExpressionNode::Call(call) =
                        self.expression_table.expression_mut(expression)
                    {
                        call.machine_arguments = arguments;
                    }
                }
                ExpressionNode::Cast(cast) => {
                    if let Some(parameters) = self.local_type_parameters {
                        for reference in
                            once(cast.target_type).chain(cast.semantic_domain_arguments.handles())
                        {
                            let mut target = self.child_type_references.get(reference).clone();
                            super::type_references::assign_type_reference_symbol_with_locals_and_constraints(
                        self.symbols,
                        self.child_type_references,
                        self.type_constraints,
                        parameters,
                        &mut target,
                    );
                            *self.child_type_references.get_mut(reference) = target;
                        }
                    }
                    self.assign(cast.value);
                }
                ExpressionNode::Indexed(indexed) => {
                    self.assign(indexed.collection);
                    self.assign(indexed.index);
                }
                ExpressionNode::Range(range) => {
                    if range.start.is_valid() {
                        self.assign(range.start);
                    }
                    if range.end.is_valid() {
                        self.assign(range.end);
                    }
                }
                ExpressionNode::Membership(membership) => {
                    self.assign(membership.value);
                    let (name, reference_span) = {
                        let members = self.expression_table.name_path_members(membership.domain);
                        (members.joined_path(), diagnostic_path_source_span(members))
                    };
                    assign_membership_symbol(
                        self.symbols,
                        self.expression_table,
                        membership.domain,
                        expression,
                    );
                    let domain_symbol = resolve_domain_symbol(
                        self.symbols,
                        self.domain_symbols,
                        &name,
                        reference_span,
                    );
                    if let ExpressionNode::Membership(membership) =
                        self.expression_table.expression_mut(expression)
                    {
                        membership.domain_symbol = domain_symbol;
                        if domain_symbol.is_valid() {
                            membership.case_type_symbol = SymbolHandle::invalid();
                            membership.case_symbol = SymbolHandle::invalid();
                        }
                    }
                }
                ExpressionNode::Member(member) => {
                    let logical_receiver = self.logical_data_subject
                        && matches!(self.expression_table.expression(member.receiver),
                        ExpressionNode::Name(path)
                        if self.expression_table.name_path_members(path.members).len() == 1
                            && self.expression_table.name_path_members(path.members)[0].as_str() == "self");
                    if logical_receiver {
                        let candidates = self
                            .local_symbols
                            .iter()
                            .filter(|(name, symbol)| {
                                name == member.member.as_str()
                                    && self.symbols.get(*symbol).kind == SymbolKind::Field
                            })
                            .map(|(_, symbol)| *symbol)
                            .collect::<Vec<_>>();
                        if let [symbol] = candidates.as_slice() {
                            let members = self.expression_table.reserve_name_path_members(1);
                            self.expression_table.set_name_path_member_at_offset(
                                members,
                                0,
                                member.member,
                            );
                            let member_symbols =
                                self.expression_table.reserve_name_path_member_symbols(1);
                            self.expression_table.set_name_path_member_symbol_at_offset(
                                member_symbols,
                                0,
                                *symbol,
                            );
                            *self.expression_table.expression_mut(expression) =
                                ExpressionNode::Name(TableNamePath::new(
                                    members,
                                    member_symbols,
                                    *symbol,
                                    *symbol,
                                ));
                            return;
                        }
                    }
                    self.assign(member.receiver);
                    self.assign_guarded_case_member(expression);
                }
                ExpressionNode::Borrow(inner) => {
                    self.assign(inner.target);
                }
                ExpressionNode::Unary(unary) => {
                    self.assign(unary.operand);
                }
                ExpressionNode::StructLiteral(struct_literal) => {
                    for field in self
                        .expression_table
                        .struct_fields(struct_literal.fields)
                        .to_vec()
                    {
                        self.assign(field.value);
                    }
                    assign_struct_literal_symbols(self.symbols, self.expression_table, expression);
                }
                ExpressionNode::Name(path) => {
                    if path.head_symbol.is_valid() || path.symbol.is_valid() {
                        return;
                    }
                    if self.assign_lexical_subject(path.members, path.member_symbols, expression) {
                        return;
                    }
                    if super::expressions::normalize_qualified_case_value(
                        self.symbols,
                        self.expression_table,
                        &path,
                        expression,
                    ) {
                        return;
                    }
                    let members = self.expression_table.name_path_members(path.members);
                    if members.is_empty()
                        || matches!(members, [member] if language_core::is_reserved_result(member.as_str()))
                    {
                        return;
                    }
                    let name = DiagnosticName::from_str(
                        &members.joined_path(),
                        diagnostic_path_source_span(members),
                    );
                    let fixed = symbols::FixedCoreDeclaration::from_name(name.as_str())
                        .and_then(|declaration| {
                            self.symbols.fixed_core_declaration_symbol(declaration)
                        })
                        .unwrap_or_default();
                    let (head, symbol, member_symbols) = if fixed.is_valid() {
                        let mut selected = fixed;
                        let mut member_symbols = Vec::with_capacity(members.len());
                        for _ in members {
                            if !selected.is_valid() || selected == self.symbols.root() {
                                return;
                            }
                            member_symbols.push(selected);
                            selected = self.symbols.namespace_parent(selected);
                        }
                        member_symbols.reverse();
                        (member_symbols[0], fixed, member_symbols)
                    } else {
                        let (head, symbol) = super::scoped_paths::resolve_state_scoped_table_path(
                            self.symbols,
                            SymbolHandle::invalid(),
                            SymbolHandle::invalid(),
                            self.expression_table,
                            &path,
                            None,
                        );
                        let members =
                            super::scoped_paths::resolve_state_scoped_table_path_member_symbols(
                                self.symbols,
                                SymbolHandle::invalid(),
                                SymbolHandle::invalid(),
                                self.expression_table,
                                &path,
                            );
                        (head, symbol, members)
                    };
                    if !symbol.is_valid() {
                        return;
                    }
                    for (offset, selected) in (0u32..).zip(member_symbols) {
                        self.expression_table.set_name_path_member_symbol_at_offset(
                            path.member_symbols,
                            offset,
                            selected,
                        );
                    }
                    if let ExpressionNode::Name(path) =
                        self.expression_table.expression_mut(expression)
                    {
                        path.head_symbol = head;
                        path.symbol = symbol;
                    }
                }
                ExpressionNode::Boolean(_)
                | ExpressionNode::Float(_)
                | ExpressionNode::UndeterminedNaN(_)
                | ExpressionNode::Integer(_)
                | ExpressionNode::String(_)
                | ExpressionNode::TypeExpression(_)
                | ExpressionNode::ZeroValue(_) => {}
            }
        }
    }

    fn resolve_domain_symbol(
        symbols: &SymbolTable,
        domain_symbols: &[(String, SymbolHandle, language_semantics::SemanticDomainId)],
        name: &str,
        reference: source::SourceSpan,
    ) -> SymbolHandle {
        let selected = symbols
            .find_top_level_by_name_and_kinds_from_source(name, &[SymbolKind::Domain], reference)
            .unwrap_or_else(SymbolHandle::invalid);
        let matches = domain_symbols
            .iter()
            .filter(|(candidate, symbol, _)| {
                symbols.source_reference_can_see_symbol(reference, *symbol)
                    && (*symbol == selected
                        || (!(name.contains("::") && symbols.display_path(*symbol, "::") == name)
                            && domain_name_reaches(symbols, *symbol, candidate, name, reference)))
            })
            .map(|(name, symbol, semantic_id)| (name.as_str(), *symbol, *semantic_id))
            .collect::<Vec<_>>();
        let reference_module = if reference.span.start == reference.span.end {
            SymbolHandle::invalid()
        } else {
            symbols.source_module(reference.source_id)
        };
        let local = matches
            .iter()
            .copied()
            .filter(|(_, symbol, _)| {
                reference_module.is_valid() && symbols.symbol_module(*symbol) == reference_module
            })
            .collect::<Vec<_>>();
        let pool = if !local.is_empty() {
            local
        } else if reference.span.start != reference.span.end {
            let own_package = matches
                .iter()
                .copied()
                .filter(|(_, symbol, _)| {
                    symbols
                        .symbol_provenance_source_span(*symbol)
                        .is_some_and(|declaration| {
                            symbols.same_source_package(reference, declaration)
                        })
                })
                .collect::<Vec<_>>();
            if own_package.is_empty() {
                matches
            } else {
                own_package
            }
        } else {
            matches
        };
        if let Some((first_name, symbol, first_semantic_id)) = pool.first() {
            if pool.iter().any(|(candidate, _, semantic_id)| {
                candidate != first_name || semantic_id != first_semantic_id
            }) {
                SymbolHandle::invalid()
            } else {
                *symbol
            }
        } else if name.contains("::") {
            symbols
                .find_top_level_by_name_and_kinds_from_source(
                    name,
                    &[SymbolKind::Domain],
                    reference,
                )
                .unwrap_or_else(SymbolHandle::invalid)
        } else {
            SymbolHandle::invalid()
        }
    }
}
#[path = "symbols_expression_paths.rs"]
mod expression_paths;
#[path = "symbols_expressions.rs"]
mod expressions;
mod lookup {
    use symbol_resolved_trees::name::DiagnosticName;
    use symbols::{BuiltinTypeAtom, SymbolHandle, SymbolKind, SymbolLookup, SymbolTable};

    /*
        The closed builtin type atom installed under `symbol_name`, or `None` for
        a name outside the roster. Every name-based builtin-type lookup reads the
        roster here instead of re-scanning `BuiltinTypeAtom::ALL`.
    */
    pub(crate) fn builtin_type_atom_by_symbol_name(symbol_name: &str) -> Option<BuiltinTypeAtom> {
        BuiltinTypeAtom::ALL
            .into_iter()
            .find(|atom| atom.symbol_name() == symbol_name)
    }

    /*
        Search declaration scopes only. Enclosing invocation parameters, locals and
        live facts are never lexical declaration bindings. Stop at a shadowing cohort,
        including ambiguity; root/module visibility remains the ordinary lookup's job.
    */
    pub(super) fn lexical_declaration_lookup(
        symbols: &SymbolTable,
        mut scope: SymbolHandle,
        name: &str,
        kinds: &[SymbolKind],
    ) -> SymbolLookup {
        while symbols.get(scope).kind == SymbolKind::Machine {
            let mut declarations =
                symbols
                    .child_handles(scope)
                    .into_iter()
                    .flatten()
                    .filter(|child| {
                        kinds.contains(&symbols.get(*child).kind) && symbols.name(*child) == name
                    });
            if let Some(first) = declarations.next() {
                return match declarations.next() {
                    Some(second) => SymbolLookup::Ambiguous { first, second },
                    None => SymbolLookup::Unique(first),
                };
            }
            scope = symbols.get(scope).parent;
        }
        SymbolLookup::NotFound
    }

    pub(super) fn diagnostic_path_source_span(members: &[DiagnosticName]) -> source::SourceSpan {
        let Some(first) = members.first() else {
            return source::SourceSpan::default();
        };
        let Some(last) = members.last() else {
            return first.source_span();
        };
        if first.source_span().source_id != last.source_span().source_id {
            return first.source_span();
        }
        source::SourceSpan::new(
            first.source_span().source_id,
            source::Span::new(first.source_span().span.start, last.source_span().span.end),
        )
    }

    /*
        Whether an authored domain reference may select `domain` from
        `reference` under module name law. The exposure rules live on the symbol
        table (`SymbolTable::domain_name_reaches`) so every stage that pools or
        selects domain declarations applies one law: a fully qualified spelling
        always selects its exact declaration, while a relative spelling reaches a
        module-owned domain inside its own module or through an exposing import.
    */
    pub(crate) fn domain_name_reaches(
        symbols: &SymbolTable,
        domain_symbol: SymbolHandle,
        _domain_name: &str,
        authored: &str,
        reference: source::SourceSpan,
    ) -> bool {
        symbols.domain_name_reaches(domain_symbol, authored, reference)
    }

    /*
        A domain declared in the reference's own module outranks same-spelled
        foreign or root candidates, mirroring local-binding precedence.
    */
    pub(crate) fn prefer_module_local_domain(
        symbols: &SymbolTable,
        candidates: Vec<SymbolHandle>,
        reference: source::SourceSpan,
    ) -> Vec<SymbolHandle> {
        if reference.span.start == reference.span.end {
            return candidates;
        }
        let reference_module = symbols.source_module(reference.source_id);
        if !reference_module.is_valid() {
            return candidates;
        }
        let local = candidates
            .iter()
            .copied()
            .filter(|candidate| symbols.symbol_module(*candidate) == reference_module)
            .collect::<Vec<_>>();
        if local.is_empty() { candidates } else { local }
    }

    pub(super) fn top_level_type_symbol_for_source(
        symbols: &SymbolTable,
        name: &DiagnosticName,
    ) -> SymbolHandle {
        if let Some(symbol) = symbols::FixedCoreDeclaration::from_name(name.as_str())
            .and_then(|declaration| symbols.fixed_core_declaration_symbol(declaration))
        {
            return symbol;
        }
        match symbols.lookup_top_level_by_name_and_kinds_from_source_matching(
            name.as_str(),
            &[
                SymbolKind::BuiltinType,
                SymbolKind::Data,
                SymbolKind::TypeAlias,
                SymbolKind::Machine,
                SymbolKind::Trait,
                SymbolKind::Const,
            ],
            name.source_span(),
            |_| true,
        ) {
            symbols::SymbolLookup::Unique(symbol) => symbol,
            symbols::SymbolLookup::Ambiguous { .. } => SymbolHandle::invalid(),
            symbols::SymbolLookup::NotFound => {
                top_level_symbol_for_source(symbols, SymbolKind::Conformance, name)
            }
        }
    }

    pub(super) fn top_level_symbol_for_source(
        symbols: &SymbolTable,
        kind: SymbolKind,
        name: &DiagnosticName,
    ) -> SymbolHandle {
        symbols
            .find_top_level_by_name_and_kinds_from_source(
                name.as_str(),
                &[kind],
                name.source_span(),
            )
            .unwrap_or_else(SymbolHandle::invalid)
    }

    /*
        Select the carrier of a complete constructor name through ordinary source
        visibility. A full data name and a case-owner prefix are competing meanings,
        never alternatives selected by the number of path segments.
    */
    pub(crate) fn constructor_type<'name>(
        symbols: &SymbolTable,
        name: &'name str,
        reference: source::SourceSpan,
    ) -> Result<Option<(SymbolHandle, Option<&'name str>)>, String> {
        let record = symbols.lookup_top_level_by_name_and_kinds_from_source_matching(
            name,
            &[SymbolKind::Data],
            reference,
            |_| true,
        );
        let (case, case_name) =
            name.rsplit_once("::")
                .map_or((SymbolLookup::NotFound, None), |(owner, case)| {
                    (
                        symbols.lookup_top_level_by_name_and_kinds_from_source_matching(
                            owner,
                            &[SymbolKind::Data],
                            reference,
                            |symbol| {
                                child_symbol_by_kinds(symbols, symbol, &[SymbolKind::Variant], case)
                                    .is_valid()
                            },
                        ),
                        Some(case),
                    )
                });
        let case_symbol = |owner| {
            child_symbol_by_kinds(
                symbols,
                owner,
                &[SymbolKind::Variant],
                case_name.expect("eligible case owner retains its requested case"),
            )
        };
        let ambiguity = |first, second| {
            selection_ambiguity(symbols, name, reference, first, second, "constructor")
        };
        match (record, case) {
            (SymbolLookup::Ambiguous { first, second }, _) => Err(ambiguity(first, second)),
            (_, SymbolLookup::Ambiguous { first, second }) => {
                Err(ambiguity(case_symbol(first), case_symbol(second)))
            }
            (SymbolLookup::Unique(record), SymbolLookup::Unique(case)) => {
                Err(ambiguity(record, case_symbol(case)))
            }
            (SymbolLookup::Unique(symbol), SymbolLookup::NotFound) => Ok(Some((symbol, None))),
            (SymbolLookup::NotFound, SymbolLookup::Unique(symbol)) => Ok(Some((symbol, case_name))),
            (SymbolLookup::NotFound, SymbolLookup::NotFound) => Ok(None),
        }
    }

    fn selection_ambiguity(
        symbols: &SymbolTable,
        name: &str,
        reference: source::SourceSpan,
        first: SymbolHandle,
        second: SymbolHandle,
        role: &str,
    ) -> String {
        let imports = symbols
            .source_module_import_paths(reference.source_id)
            .collect::<Vec<_>>();
        format!(
            "ambiguous {role} `{name}`: competing declarations `{}` and `{}`; source imports: {}",
            symbols.display_path(first, "::"),
            symbols.display_path(second, "::"),
            if imports.is_empty() {
                "(none)".to_owned()
            } else {
                imports.join(", ")
            },
        )
    }

    pub(crate) fn constructor_prefix_is_value(
        symbols: &SymbolTable,
        name: &str,
        reference: source::SourceSpan,
    ) -> Result<bool, String> {
        let Some((prefix, _)) = name.split_once("::") else {
            return Ok(false);
        };
        match symbols.lookup_top_level_by_name_and_kinds_from_source_matching(
            prefix,
            &[SymbolKind::Const],
            reference,
            |_| true,
        ) {
            SymbolLookup::Unique(_) => return Ok(true),
            SymbolLookup::Ambiguous { first, second } => {
                return Err(selection_ambiguity(
                    symbols,
                    name,
                    reference,
                    first,
                    second,
                    "constructor",
                ));
            }
            SymbolLookup::NotFound => {}
        }
        Ok(false)
    }

    /*
        A carrier-qualified path may construct only a payload-free case. The caller
        resolves lexical binders first; named constant prefixes remain values,
        including an ambiguous prefix that must never be reinterpreted as a namespace.
    */
    pub(crate) fn qualified_payload_free_case_type<'name>(
        symbols: &SymbolTable,
        name: &'name str,
        reference: source::SourceSpan,
    ) -> Result<Option<(SymbolHandle, SymbolHandle, &'name str)>, String> {
        if !name.contains("::") {
            return Ok(None);
        }
        if constructor_prefix_is_value(symbols, name, reference)? {
            return Ok(None);
        }
        let Some((owner, Some(case))) = constructor_type(symbols, name, reference)? else {
            return Ok(None);
        };
        let case_symbol = child_symbol_by_kinds(symbols, owner, &[SymbolKind::Variant], case);
        if !case_symbol.is_valid()
            || symbols
                .child_handles(case_symbol)
                .is_none_or(|mut children| {
                    children.any(|child| symbols.get(child).kind == SymbolKind::Field)
                })
        {
            return Ok(None);
        }
        Ok(Some((owner, case_symbol, case)))
    }

    pub(super) fn top_level_symbol_by_kinds(
        symbols: &SymbolTable,
        kinds: &[SymbolKind],
        name: &str,
    ) -> SymbolHandle {
        child_symbol_by_kinds(symbols, symbols.root(), kinds, name)
    }

    pub(super) fn child_symbol_by_kinds(
        symbols: &SymbolTable,
        parent: SymbolHandle,
        kinds: &[SymbolKind],
        name: &str,
    ) -> SymbolHandle {
        if parent == symbols.root() && symbols.get(parent).kind != SymbolKind::Module {
            return symbols
                .root_children_named(name)
                .find(|child| kinds.contains(&symbols.get(*child).kind))
                .unwrap_or_else(SymbolHandle::invalid);
        }
        child_symbol_by_kinds_matching(symbols, parent, kinds, |symbol_name| symbol_name == name)
    }

    pub(super) fn child_or_attached_data_child_symbol_by_kinds(
        symbols: &SymbolTable,
        parent: SymbolHandle,
        kinds: &[SymbolKind],
        name: &str,
    ) -> SymbolHandle {
        let child = child_symbol_by_kinds(symbols, parent, kinds, name);
        if !child.is_valid() && symbols.get(parent).kind == SymbolKind::Variant {
            return child_symbol_by_kinds(symbols, symbols.get(parent).parent, kinds, name);
        }
        if child.is_valid() || symbols.get(parent).kind != SymbolKind::Machine {
            return child;
        }

        let Some(attached_data_name) = symbols.name(parent).split_once("::").map(|(data, _)| data)
        else {
            return SymbolHandle::invalid();
        };
        let reference = symbols
            .symbol_provenance_source_span(parent)
            .unwrap_or_default();
        let attached_data = symbols
            .find_top_level_by_name_and_kinds_from_source(
                attached_data_name,
                &[SymbolKind::Data],
                reference,
            )
            .unwrap_or_else(SymbolHandle::invalid);
        if !attached_data.is_valid() {
            return SymbolHandle::invalid();
        }

        child_symbol_by_kinds(symbols, attached_data, kinds, name)
    }

    pub(super) fn child_indexed_symbol_by_kinds(
        symbols: &SymbolTable,
        parent: SymbolHandle,
        kinds: &[SymbolKind],
        name: &str,
        index: i64,
    ) -> SymbolHandle {
        child_symbol_by_kinds_matching(symbols, parent, kinds, |symbol_name| {
            symbol_name_matches_indexed_member(symbol_name, name, index)
        })
    }

    pub(super) fn call_target_for_attached_data(
        symbols: &SymbolTable,
        attached_data: &str,
        target_name: &str,
        reference: source::SourceSpan,
    ) -> SymbolHandle {
        let machine_name = format!("{attached_data}::{target_name}");
        let machine_symbol = symbols
            .find_top_level_by_name_and_kinds_from_source(
                &machine_name,
                &[SymbolKind::Machine],
                reference,
            )
            .unwrap_or_else(SymbolHandle::invalid);
        if !machine_symbol.is_valid() {
            return SymbolHandle::invalid();
        }

        child_symbol_by_kinds(symbols, machine_symbol, &[SymbolKind::State], target_name)
    }

    fn child_symbol_by_kinds_matching(
        symbols: &SymbolTable,
        parent: SymbolHandle,
        kinds: &[SymbolKind],
        mut matches_name: impl FnMut(&str) -> bool,
    ) -> SymbolHandle {
        let Some(children) = symbols.child_handles(parent) else {
            return SymbolHandle::invalid();
        };

        let mut matched = SymbolHandle::invalid();
        for child in children {
            let symbol = symbols.get(child);
            if matches_name(symbols.name(child))
                && (kinds.contains(&symbol.kind) || symbols.get(parent).kind == SymbolKind::Module)
            {
                if symbols.get(parent).kind != SymbolKind::Module {
                    return child;
                }
                if matched.is_valid() {
                    return SymbolHandle::invalid();
                }
                matched = child;
            }
        }

        matched
    }

    fn symbol_name_matches_indexed_member(symbol_name: &str, member: &str, index: i64) -> bool {
        let Some(suffix) = symbol_name.strip_prefix(member) else {
            return false;
        };
        let Some(suffix) = suffix.strip_prefix('[') else {
            return false;
        };
        let Some(index_text) = suffix.strip_suffix(']') else {
            return false;
        };

        index_text.parse::<i64>().ok() == Some(index)
    }

    /*
        Membership selects either a declared domain or an actual case of an exact
        carrier. Both meanings share the member namespace; ambiguity is not absence.
        Unlike construction, observing a case permits common fields and payloads.
    */
    pub(crate) enum MembershipSelection {
        Domain(SymbolHandle),
        Case {
            owner: SymbolHandle,
            case: SymbolHandle,
        },
    }

    /* The domain side of a membership selection. */
    enum DomainMembership {
        NotFound,
        Unique(SymbolHandle),
        /*
            The ordinary path law found competing exact selections — an authored
            ambiguity that rejects immediately.
        */
        Ambiguous {
            first: SymbolHandle,
            second: SymbolHandle,
        },
        /*
            The attached-domain exposure pool claims the spelling under more than
            one distinct owner. Like the proof-fact resolver's pooled decline,
            this retains the membership as a checked obligation rather than
            reporting ambiguity here: downstream validation owns the diagnostic,
            and no other meaning may claim the contested spelling.
        */
        Contested,
    }

    /*
        Pool every domain candidate an authored membership name can select: the
        ordinary qualified/import path lookup unioned with the attached-domain
        exposure law (`domain_name_reaches`), which admits a carrier-qualified or
        leaf spelling through a declaring-module or exact-domain import.
        Module-local owners outrank every other tier, and declarations sharing one
        complete logical path are one semantic candidate.
    */
    fn membership_domain_lookup(
        symbols: &SymbolTable,
        name: &str,
        reference: source::SourceSpan,
    ) -> DomainMembership {
        let mut candidates = match symbols.lookup_top_level_by_name_and_kinds_from_source_matching(
            name,
            &[SymbolKind::Domain],
            reference,
            |_| true,
        ) {
            SymbolLookup::Unique(symbol) => vec![symbol],
            SymbolLookup::Ambiguous { first, second } => {
                return DomainMembership::Ambiguous { first, second };
            }
            SymbolLookup::NotFound => Vec::new(),
        };
        if let Some(children) = symbols.child_handles(symbols.root()) {
            for symbol in children {
                if symbols.get(symbol).kind != SymbolKind::Domain
                    || candidates.contains(&symbol)
                    || !symbols.source_reference_can_see_symbol(reference, symbol)
                    || !domain_name_reaches(symbols, symbol, symbols.name(symbol), name, reference)
                {
                    continue;
                }
                candidates.push(symbol);
            }
        }
        let candidates = prefer_module_local_domain(symbols, candidates, reference);
        let mut unique: Vec<(String, SymbolHandle)> = Vec::new();
        'next: for candidate in candidates {
            let path = symbols.display_path(candidate, "::");
            for (existing, _) in &unique {
                if *existing == path {
                    continue 'next;
                }
            }
            unique.push((path, candidate));
        }
        match unique.as_slice() {
            [] => DomainMembership::NotFound,
            [(_, symbol)] => DomainMembership::Unique(*symbol),
            _ => DomainMembership::Contested,
        }
    }

    pub(crate) fn membership_selection(
        symbols: &SymbolTable,
        name: &str,
        reference: source::SourceSpan,
    ) -> Result<Option<MembershipSelection>, String> {
        let domain = membership_domain_lookup(symbols, name, reference);
        let (case_owner, case_name) =
            name.rsplit_once("::")
                .map_or((SymbolLookup::NotFound, ""), |(owner, case)| {
                    (
                        symbols.lookup_top_level_by_name_and_kinds_from_source_matching(
                            owner,
                            &[SymbolKind::Data],
                            reference,
                            |owner| {
                                child_symbol_by_kinds(symbols, owner, &[SymbolKind::Variant], case)
                                    .is_valid()
                            },
                        ),
                        case,
                    )
                });
        let case = |owner| child_symbol_by_kinds(symbols, owner, &[SymbolKind::Variant], case_name);
        let ambiguous = |first, second| {
            selection_ambiguity(symbols, name, reference, first, second, "membership")
        };
        match (domain, case_owner) {
            (DomainMembership::Ambiguous { first, second }, _) => Err(ambiguous(first, second)),
            (_, SymbolLookup::Ambiguous { first, second }) => {
                Err(ambiguous(case(first), case(second)))
            }
            (DomainMembership::Contested, _) => Ok(None),
            (DomainMembership::Unique(domain), SymbolLookup::Unique(owner)) => {
                Err(ambiguous(domain, case(owner)))
            }
            (DomainMembership::Unique(domain), SymbolLookup::NotFound) => {
                Ok(Some(MembershipSelection::Domain(domain)))
            }
            (DomainMembership::NotFound, SymbolLookup::Unique(owner)) => {
                Ok(Some(MembershipSelection::Case {
                    owner,
                    case: case(owner),
                }))
            }
            (DomainMembership::NotFound, SymbolLookup::NotFound) => Ok(None),
        }
    }
}
mod mathematical {
    /*
        Mathematical `let`/`boundary let` declarations: name resolution inside
        their signature surfaces and transparent bodies.

        Every declaration contributes its binder/parameter children as the local
        scope. `Ordinary` mathematical type arms resolve through the shared type
        pass with the binders as local type parameters — so `u: core::Level` binds
        inside `A: core::Type<u>` — while `Application` argument expressions and
        `Definition` bodies share the proposition walk and the common callable
        declaration resolver. Arrow binders are retained as
        authored names only; the typed-tree telescope owns their scoping.
    */

    use symbol_resolved_trees::data::TypeParameterKind;
    use symbol_resolved_trees::mathematical::{MathematicalBody, MathematicalType};
    use symbols::SymbolTable;

    use super::propositions::assign_expression_symbols;
    use super::type_references::assign_type_reference_symbol_with_locals;

    pub(super) fn assign_mathematical_expression_symbols(
        program: &mut symbol_resolved_trees::SymbolResolvedTrees,
        symbols: &SymbolTable,
    ) {
        let data_type_parameters = &mut program.tables.declarations.data_type_parameters;
        let mathematical_parameters = &program.tables.declarations.mathematical_parameters;
        let mathematical_types = &mut program.tables.declarations.mathematical_types;
        let child_type_references = &mut program.tables.declarations.child_type_references;
        let expressions = &mut program.tables.bodies.expressions;
        let definitions = &program.roots.mathematical_definitions;

        for definition in definitions {
            let local_type_parameters = data_type_parameters
                .span_or_empty(definition.binders)
                .to_vec();

            for binder in data_type_parameters.span_mut_or_empty(definition.binders) {
                let type_reference = match &mut binder.kind {
                    TypeParameterKind::Const { type_reference }
                    | TypeParameterKind::Value { type_reference } => type_reference,
                    _ => continue,
                };
                assign_type_reference_symbol_with_locals(
                    symbols,
                    child_type_references,
                    &local_type_parameters,
                    type_reference,
                );
            }

            let mut worklist = vec![definition.result];
            for parameter in mathematical_parameters.span_or_empty(definition.parameters) {
                worklist.push(parameter.ty);
            }
            while let Some(handle) = worklist.pop() {
                let mut node = mathematical_types.get(handle).clone();
                match &mut node {
                    MathematicalType::Ordinary(reference) => {
                        assign_type_reference_symbol_with_locals(
                            symbols,
                            child_type_references,
                            &local_type_parameters,
                            reference,
                        );
                    }
                    MathematicalType::Arrow {
                        domain, codomain, ..
                    } => {
                        worklist.push(*domain);
                        worklist.push(*codomain);
                    }
                    MathematicalType::Application { callee, arguments } => {
                        worklist.push(*callee);
                        for argument in expressions.expression_handles(*arguments).to_vec() {
                            assign_expression_symbols(
                                expressions,
                                child_type_references,
                                symbols,
                                definition.symbol,
                                &local_type_parameters,
                                argument,
                            );
                        }
                    }
                }
                *mathematical_types.get_mut(handle) = node;
            }

            let MathematicalBody::Definition(term) = definition.body else {
                continue;
            };
            assign_expression_symbols(
                expressions,
                child_type_references,
                symbols,
                definition.symbol,
                &local_type_parameters,
                term,
            );
        }
    }
}
mod measures {
    use symbol_resolved_trees::SymbolResolvedTrees;
    use symbols::{SymbolHandle, SymbolTable};

    use super::expressions::assign_expression_span_symbols;
    use super::scope::MachineScope;

    pub(super) fn assign_measure_expression_symbols(
        program: &mut SymbolResolvedTrees,
        symbols: &SymbolTable,
    ) {
        let attached_machines = super::scope::attached_machines(program);
        let declarations = &mut program.tables.declarations;
        let expressions = &mut program.tables.bodies.expressions;
        for measure in &program.roots.measures {
            let Some(parameter) = &measure.parameter else {
                continue;
            };
            let scope = MachineScope {
                attached_machines: &attached_machines,
                resolution_indexes: &program.resolution_indexes,
                symbol: SymbolHandle::invalid(),
                lexical_requirements: super::scope::LexicalRequirements::none(),
                type_parameters: &[],
                attached_data_symbol: SymbolHandle::invalid(),
                inherited_data_members: None,
                owned_data: &[],
                prior_statements: &[],
                data_definitions: &program.roots.data_definitions,
                type_aliases: &program.roots.type_aliases,
                conformances: &program.roots.conformances,
                traits: &program.roots.traits,
                operand_owners: &program.tables.types.operand_lifetime_owners,
                data_members: &declarations.data_members,
                data_type_parameters: &declarations.data_type_parameters,
                data_payload_fields: &declarations.data_payload_fields,
                type_constraints: &program.tables.types.constraints,
            };
            assign_expression_span_symbols(
                symbols,
                &scope,
                std::slice::from_ref(parameter),
                measure.symbol,
                expressions,
                &mut declarations.child_type_references,
                measure.body,
            );
        }
    }
}
mod repeated_locals {
    /*
        Refuse a `let` that declares a name its body already declares.

        A local name is declared at most once in a machine body, and parameters
        and locals share that namespace (`wiki/spec/language/state_contracts.md`,
        "Bindings"): "Refusal compares declared binder identities, not spellings
        rendered as text." This is where each binder gets its identity, so the
        rule is applied here, once, for both checkers: a body that breaks it
        never reaches typing. Each binder is compared by the name its
        declaration gave its symbol, so two binders are never merged and no
        expression rendering is read. The binders are the state's parameters
        and the runtime bindings among its statements, the same set the rule
        covered when Psi 05 applied it.
    */

    use diagnostics::Diagnostic;
    use symbol_resolved_trees::SymbolResolvedTrees;
    use symbol_resolved_trees::statement::Statement;
    use symbols::SymbolHandle;

    pub(super) fn reject_repeated_local_names(
        program: &SymbolResolvedTrees,
    ) -> Result<(), Vec<Diagnostic>> {
        let symbols = &program.symbols;
        let declarations = &program.tables.declarations;
        let mut diagnostics = Vec::new();
        for (_, state) in declarations.machine_states.iter() {
            if !state.symbol.is_valid() {
                continue;
            }
            let parameters: Vec<SymbolHandle> = declarations
                .state_parameters
                .span_or_empty(state.parameters)
                .iter()
                .map(|parameter| parameter.symbol)
                .filter(|symbol| symbol.is_valid())
                .collect();
            let mut locals: Vec<SymbolHandle> = Vec::new();
            for statement in declarations
                .state_statements
                .span_or_empty(state.statements)
            {
                let Statement::LocalData(local) = statement else {
                    continue;
                };
                if !local.role.is_runtime_binding() || !local.symbol.is_valid() {
                    continue;
                }
                let name = symbols.name(local.symbol);
                let rule = if parameters
                    .iter()
                    .any(|parameter| symbols.name(*parameter) == name)
                {
                    Some("conflicts with an existing name")
                } else if locals.iter().any(|earlier| symbols.name(*earlier) == name) {
                    Some("duplicate")
                } else {
                    None
                };
                locals.push(local.symbol);
                let Some(rule) = rule else {
                    continue;
                };
                let machine = symbols.name(symbols.get(state.symbol).parent);
                let state_name = symbols.name(state.symbol);
                let message = if rule == "duplicate" {
                    format!(
                        "machine `{machine}` state `{state_name}` has duplicate local data `{name}`: \
                         a local name is declared at most once in a machine body"
                    )
                } else {
                    format!(
                        "machine `{machine}` state `{state_name}` local data `{name}` conflicts \
                         with an existing name: parameters and locals share one namespace"
                    )
                };
                let diagnostic = Diagnostic::error(message);
                diagnostics.push(match symbols.symbol_source_span(local.symbol) {
                    Some(span) => diagnostic.with_source_span(span),
                    None => diagnostic,
                });
            }
        }
        if diagnostics.is_empty() {
            Ok(())
        } else {
            Err(diagnostics)
        }
    }
}

mod private_nested_calls {
    /*
        Refuse a qualified call into another machine's nested helper.

        A nested machine is unavailable outside the machine whose scope declares it,
        and qualification cannot bypass that rule
        (`wiki/spec/language/machines.md#nested-machines-and-identity`). Call
        resolution already leaves `outer::helper(..)` unselected: a machine receiver
        names no methods. Left alone, that absence surfaced wherever a later stage
        first needed the callee (a dispatch-subject storage type, a generic static
        argument, a receiver-method hint), never naming the rule. This scan runs once
        the bodies' symbols are assigned and reports the rule at the call, for every
        carrier: expression calls, statement calls and terminal transition targets.
        The expression and statement arenas are flat, so no body walk is needed.
    */

    use diagnostics::Diagnostic;
    use symbol_resolved_trees::SymbolResolvedTrees;
    use symbol_resolved_trees::expression::ExpressionNode;
    use symbol_resolved_trees::name::DiagnosticName;
    use symbol_resolved_trees::statement::{Statement, TransitionTarget};
    use symbols::{SymbolHandle, SymbolKind, SymbolTable};

    pub(super) fn reject_private_nested_calls(
        program: &SymbolResolvedTrees,
    ) -> Result<(), Vec<Diagnostic>> {
        let symbols = &program.symbols;
        let expressions = &program.tables.bodies.expressions;
        let declarations = &program.tables.declarations;
        let mut diagnostics = Vec::new();
        for (_, node) in expressions.iter_expressions() {
            let ExpressionNode::Call(call) = node else {
                continue;
            };
            if call.target_symbol.is_valid() || !call.receiver.is_valid() {
                continue;
            }
            let ExpressionNode::Name(path) = expressions.expression(call.receiver) else {
                continue;
            };
            let receiver = if path.symbol.is_valid() {
                path.symbol
            } else {
                path.head_symbol
            };
            report(symbols, receiver, &call.target, &mut diagnostics);
        }
        for (_, statement) in declarations.state_statements.iter() {
            match statement {
                Statement::Call(call) if !call.target_symbol.is_valid() => {
                    report(
                        symbols,
                        call.receiver_symbol,
                        &call.target,
                        &mut diagnostics,
                    );
                }
                Statement::Transition(transition) => {
                    for target in
                        std::iter::once(&transition.target).chain(transition.continuation.as_ref())
                    {
                        let TransitionTarget::Named(named) = target else {
                            continue;
                        };
                        let path = declarations
                            .statement_path_members
                            .span_or_empty(named.path);
                        if let [_, .., leaf] = path {
                            report(symbols, named.receiver_symbol, leaf, &mut diagnostics);
                        }
                    }
                }
                _ => {}
            }
        }
        if diagnostics.is_empty() {
            Ok(())
        } else {
            Err(diagnostics)
        }
    }

    fn report(
        symbols: &SymbolTable,
        receiver: SymbolHandle,
        target: &DiagnosticName,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        if !receiver.is_valid()
            || symbols.get(receiver).kind != SymbolKind::Machine
            || symbols
                .find_child_by_name_and_kind(receiver, target.as_str(), SymbolKind::Machine)
                .is_none()
        {
            return;
        }
        let owner = symbols.name(receiver);
        diagnostics.push(
        Diagnostic::error(format!(
            "call `{owner}::{}` names a nested machine private to `{owner}`; a nested machine is \
             called by its bare name inside its enclosing machine and is unavailable outside it",
            target.as_str(),
        ))
        .with_source_span(target.source_span()),
    );
    }
}
mod propositions {
    use arena::{Arena, HandleSpan};
    use symbol_resolved_trees::data::{TypeParameter, TypeParameterKind};
    use symbol_resolved_trees::expression::{
        ExpressionHandle, ExpressionNode, ExpressionTable, MatchPattern,
    };
    use symbol_resolved_trees::proposition::PropositionBody;
    use symbol_resolved_trees::types::TypeReference;
    use symbols::{FixedCoreDeclaration, JoinedPath, SymbolHandle, SymbolKind, SymbolTable};

    use super::expressions::{NameRole, assign_struct_literal_symbols};
    use super::lookup::child_symbol_by_kinds;
    use super::scoped_paths::{
        resolve_state_scoped_table_path, resolve_state_scoped_table_path_member_symbols,
    };
    use super::targets::{assign_static_argument_symbols, resolve_free_call_target_symbol};

    pub(super) fn assign_proposition_expression_symbols(
        program: &mut symbol_resolved_trees::SymbolResolvedTrees,
        symbols: &SymbolTable,
    ) {
        let proposition_binders = &program.tables.declarations.proposition_binders;
        let child_type_references = &mut program.tables.declarations.child_type_references;
        let expressions = &mut program.tables.bodies.expressions;
        let propositions = &program.roots.propositions;
        for proposition in propositions {
            let PropositionBody::Transparent { proposition: body } = proposition.body else {
                continue;
            };
            let local_type_parameters = proposition_binders
                .span_or_empty(proposition.binders)
                .iter()
                .map(|binder| TypeParameter {
                    symbol: binder.symbol,
                    name: binder.name.clone(),
                    kind: TypeParameterKind::Type,
                    bounds: binder.bounds,
                })
                .collect::<Vec<_>>();
            assign_expression_symbols(
                expressions,
                child_type_references,
                symbols,
                proposition.symbol,
                &local_type_parameters,
                body,
            );
        }
    }

    fn assign_expression_span_symbols(
        expressions: &mut ExpressionTable,
        child_type_references: &mut Arena<TypeReference>,
        symbols: &SymbolTable,
        owner_symbol: SymbolHandle,
        local_type_parameters: &[TypeParameter],
        span: HandleSpan<ExpressionHandle>,
    ) {
        let children = expressions.expression_handles(span).to_vec();
        for child in children {
            assign_expression_symbols(
                expressions,
                child_type_references,
                symbols,
                owner_symbol,
                local_type_parameters,
                child,
            );
        }
    }

    /*
        Resolve the authored names inside one proof-surface expression.
        `owner_symbol` is the declaration whose binder/parameter children are in
        scope; callable declarations use the shared source target resolver.
        Exact lexical binders precede authored declarations, which precede the
        outermost compiler builtins. Ambiguous declarations stay unresolved.
        Shared by proposition formulas and mathematical `let`/`boundary let`
        terms (PROOF-CONTRACT-MIGRATION).
    */
    pub(super) fn assign_expression_symbols(
        expressions: &mut ExpressionTable,
        child_type_references: &mut Arena<TypeReference>,
        symbols: &SymbolTable,
        owner_symbol: SymbolHandle,
        local_type_parameters: &[TypeParameter],
        expression: ExpressionHandle,
    ) {
        assign_expression_symbols_in_role(
            expressions,
            child_type_references,
            symbols,
            owner_symbol,
            local_type_parameters,
            expression,
            NameRole::Value,
        );
    }

    fn assign_expression_symbols_in_role(
        expressions: &mut ExpressionTable,
        child_type_references: &mut Arena<TypeReference>,
        symbols: &SymbolTable,
        owner_symbol: SymbolHandle,
        local_type_parameters: &[TypeParameter],
        expression: ExpressionHandle,
        role: NameRole,
    ) {
        macro_rules! recurse {
            ($expression:expr) => {
                assign_expression_symbols(
                    expressions,
                    child_type_references,
                    symbols,
                    owner_symbol,
                    local_type_parameters,
                    $expression,
                )
            };
        }
        match expressions.expression(expression).clone() {
            ExpressionNode::Match(dispatch) => {
                recurse!(dispatch.subject);
                for arm in expressions.match_arms(dispatch.arms).to_vec() {
                    let head = match arm.pattern {
                        MatchPattern::Value(head) => head,
                        MatchPattern::Destructure(pattern) => pattern.head,
                        MatchPattern::Wildcard => ExpressionHandle::invalid(),
                    };
                    for pattern in expressions
                        .match_pattern_dependencies(arm.pattern)
                        .collect::<Vec<_>>()
                    {
                        assign_expression_symbols_in_role(
                            expressions,
                            child_type_references,
                            symbols,
                            owner_symbol,
                            local_type_parameters,
                            pattern,
                            if pattern == head {
                                NameRole::PatternHead
                            } else {
                                NameRole::Value
                            },
                        );
                    }
                    recurse!(arm.value);
                }
            }
            ExpressionNode::SavedSubject(_) => {}
            ExpressionNode::ArrayLiteral(values) => assign_expression_span_symbols(
                expressions,
                child_type_references,
                symbols,
                owner_symbol,
                local_type_parameters,
                values,
            ),
            ExpressionNode::Atomic(atomic) => {
                recurse!(atomic.value);
                if atomic.result.is_valid() {
                    recurse!(atomic.result);
                }
            }
            ExpressionNode::Binary(binary) => {
                recurse!(binary.left);
                recurse!(binary.right);
            }
            ExpressionNode::Cast(cast) => {
                recurse!(cast.value);
                crate::symbols::type_references::reassign_type_reference_symbols(
                    symbols,
                    child_type_references,
                    local_type_parameters,
                    cast.target_type,
                );
                for handle in cast.semantic_domain_arguments.handles() {
                    crate::symbols::type_references::reassign_type_reference_symbols(
                        symbols,
                        child_type_references,
                        local_type_parameters,
                        handle,
                    );
                }
            }
            ExpressionNode::Call(call) => {
                if call.receiver.is_valid() {
                    recurse!(call.receiver);
                }
                assign_expression_span_symbols(
                    expressions,
                    child_type_references,
                    symbols,
                    owner_symbol,
                    local_type_parameters,
                    call.arguments,
                );
                let target_symbol = if call.receiver.is_valid() {
                    SymbolHandle::invalid()
                } else {
                    let parameter = child_symbol_by_kinds(
                        symbols,
                        owner_symbol,
                        &[
                            SymbolKind::Parameter,
                            SymbolKind::PropositionParameter,
                            SymbolKind::MachineParameter,
                        ],
                        call.target.as_str(),
                    );
                    if parameter.is_valid() {
                        parameter
                    } else {
                        resolve_free_call_target_symbol(symbols, &call.target)
                    }
                };
                if let ExpressionNode::Call(call) = expressions.expression_mut(expression) {
                    call.target_symbol = target_symbol;
                    for argument in &mut call.machine_arguments {
                        assign_static_argument_symbols(symbols, owner_symbol, &[], argument, true);
                    }
                }
            }
            ExpressionNode::Indexed(indexed) => {
                recurse!(indexed.collection);
                recurse!(indexed.index);
            }
            ExpressionNode::Membership(membership) => recurse!(membership.value),
            ExpressionNode::Member(member) => recurse!(member.receiver),
            ExpressionNode::Borrow(inner) => recurse!(inner.target),
            ExpressionNode::Name(path) => {
                let members = expressions.name_path_members(path.members);
                let (head_symbol, symbol) = if let [name] = members {
                    let symbol = child_symbol_by_kinds(
                        symbols,
                        owner_symbol,
                        &[
                            SymbolKind::Parameter,
                            SymbolKind::TypeParameter,
                            SymbolKind::PropositionMachineParameter,
                        ],
                        name.as_str(),
                    );
                    (symbol, symbol)
                } else if let Some(symbol) = FixedCoreDeclaration::from_name(&members.joined_path())
                    .and_then(|declaration| symbols.fixed_core_declaration_symbol(declaration))
                {
                    (SymbolHandle::invalid(), symbol)
                } else {
                    resolve_state_scoped_table_path(
                        symbols,
                        owner_symbol,
                        SymbolHandle::invalid(),
                        expressions,
                        &path,
                        None,
                    )
                };
                let member_count = members.len();
                // Mathematical bodies and proof formulas construct the same case
                // values as machine expressions. Resolve the lexical head first so
                // a value binding cannot be reinterpreted as a constructor namespace.
                if role == NameRole::Value
                    && (!head_symbol.is_valid()
                        || matches!(
                            symbols.get(head_symbol).kind,
                            SymbolKind::Data | SymbolKind::Module
                        ))
                    && super::expressions::normalize_qualified_case_value(
                        symbols,
                        expressions,
                        &path,
                        expression,
                    )
                {
                    return;
                }
                let member_symbols = if member_count == 1 && head_symbol.is_valid() {
                    vec![head_symbol]
                } else if head_symbol.is_valid() {
                    resolve_state_scoped_table_path_member_symbols(
                        symbols,
                        owner_symbol,
                        SymbolHandle::invalid(),
                        expressions,
                        &path,
                    )
                } else {
                    Vec::new()
                };
                for (offset, member_symbol) in (0u32..).zip(member_symbols) {
                    expressions.set_name_path_member_symbol_at_offset(
                        path.member_symbols,
                        offset,
                        member_symbol,
                    );
                }
                if let ExpressionNode::Name(path) = expressions.expression_mut(expression) {
                    path.head_symbol = head_symbol;
                    path.symbol = symbol;
                }
            }
            ExpressionNode::Range(range) => {
                if range.start.is_valid() {
                    recurse!(range.start);
                }
                if range.end.is_valid() {
                    recurse!(range.end);
                }
            }
            ExpressionNode::StructLiteral(struct_literal) => {
                let fields = expressions.struct_fields(struct_literal.fields).to_vec();
                for field in fields {
                    recurse!(field.value);
                }
                assign_struct_literal_symbols(symbols, expressions, expression);
            }
            ExpressionNode::Unary(unary) => recurse!(unary.operand),
            ExpressionNode::TypeExpression(type_reference)
            | ExpressionNode::ZeroValue(type_reference) => {
                crate::symbols::type_references::reassign_type_reference_symbols(
                    symbols,
                    child_type_references,
                    local_type_parameters,
                    type_reference,
                );
            }
            ExpressionNode::Boolean(_)
            | ExpressionNode::Float(_)
            | ExpressionNode::UndeterminedNaN(_)
            | ExpressionNode::Integer(_)
            | ExpressionNode::String(_) => {}
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use crate::resolution::{ResolutionRequest, resolve};
        use source_files_to_tokens::Lexer;
        use symbol_resolved_trees::mathematical::MathematicalBody;

        #[test]
        fn mathematical_match_keeps_case_selector_separate_from_constructed_value() {
            let text = r#"
                data Choice { case First; case Other; }
                let select(value: Choice): Choice = match value {
                    Choice::First {} -> Choice::First
                    _ -> Choice::Other
                };
            "#;
            let mut sources = source::SourceMap::default();
            let source_id = sources
                .add("mathematical-match.omg".into(), text.into())
                .source_id;
            let tokens = Lexer::new(text).tokenize().unwrap();
            let syntax =
                tokens_to_syntax_trees::parse_syntax_trees_with_id(source_id, &tokens).unwrap();
            let program = resolve(ResolutionRequest {
                syntax,
                sources: Some(std::sync::Arc::new(sources)),
                top_level_bindings: Vec::new(),
                retained_base: None,
            })
            .unwrap();
            let MathematicalBody::Definition(body) = program.mathematical_definitions[0].body
            else {
                panic!("the mathematical match must retain its definition");
            };
            let expressions = &program.tables.bodies.expressions;
            let ExpressionNode::Match(dispatch) = expressions.expression(body) else {
                panic!("the mathematical body must retain its match");
            };
            let arm = &expressions.match_arms(dispatch.arms)[0];
            let MatchPattern::Destructure(pattern) = arm.pattern else {
                panic!("the case pattern must retain its destructure selector");
            };
            let ExpressionNode::Name(selector) = expressions.expression(pattern.head) else {
                panic!("a pattern selector must retain declaration identity");
            };
            assert_eq!(
                program.symbols.get(selector.symbol).kind,
                SymbolKind::Variant
            );
            let ExpressionNode::StructLiteral(value) = expressions.expression(arm.value) else {
                panic!("the arm result must construct its case value");
            };
            assert_eq!(value.case_symbol, Some(selector.symbol));
        }
    }
}
pub(crate) mod recursive_tail_calls {
    /*
        Exact recursive tail calls retain invocation admission on constant-stack edges.

        Run once after all declaration/conformance/target binding, on authoritative
        statement nodes. Earlier ordinary resolution can still name a target-family
        interface, not this body; normalizing then misses legal recursive transfers.
        Re-running name assignment here would undo final binding. Legacy statement
        slices are no longer the consumer's input and must not be rebuilt over these
        nodes. Ranking remains independent of tail position; later checking admits
        operands, invocation contracts, ownership and exact authored call markers.
    */

    use crate::tables::body_statements;
    use arena::HandleSpan;
    use symbol_resolved_trees::SymbolResolvedTrees;
    use symbol_resolved_trees::expression::{ExpressionHandle, ExpressionNode};
    use symbol_resolved_trees::machine::Machine;
    use symbol_resolved_trees::statement::{
        StatementNode, TableNamePath, TableTransition, TransitionTargetHandle, TransitionTargetNode,
    };
    use symbols::SymbolHandle;

    use super::scope::{AttachedMachines, attached_machines};

    #[derive(Clone, Copy)]
    struct RecursiveEntry {
        machine: SymbolHandle,
        entry: SymbolHandle,
        owner: SymbolHandle,
    }

    /*
        Attached machines ordered by owner, then entry-state spelling, then roster
        position, so one owner's same-named entries form one contiguous range.

        Every machine of one data type is attached to the same owner. Filtering
        that owner's whole roster by name for each machine made this pass
        quadratic in the number of methods a type has.
    */
    struct EntryNames {
        rows: Vec<((u32, u32), String, SymbolHandle)>,
    }

    impl EntryNames {
        fn of(program: &SymbolResolvedTrees, attached: &AttachedMachines) -> Self {
            let mut rows = attached
                .list
                .iter()
                .map(|candidate| {
                    (
                        symbol_key(candidate.owner),
                        program.symbols.name(candidate.entry).to_owned(),
                        candidate.machine,
                    )
                })
                .collect::<Vec<_>>();
            rows.sort_by(|left, right| (left.0, &left.1).cmp(&(right.0, &right.1)));
            Self { rows }
        }

        /* Machines attached to `owner` whose entry is spelled `name`, in roster order. */
        fn candidates<'a>(
            &'a self,
            owner: SymbolHandle,
            name: &'a str,
        ) -> impl Iterator<Item = SymbolHandle> + 'a {
            let key = symbol_key(owner);
            let start = self.rows.partition_point(|(row_owner, row_name, _)| {
                (*row_owner, row_name.as_str()) < (key, name)
            });
            self.rows[start..]
                .iter()
                .take_while(move |(row_owner, row_name, _)| *row_owner == key && row_name == name)
                .map(|(_, _, machine)| *machine)
        }
    }

    fn symbol_key(symbol: SymbolHandle) -> (u32, u32) {
        (symbol.arena_index(), symbol.generation())
    }

    pub(crate) fn normalize(program: &mut SymbolResolvedTrees, work: &[SymbolHandle]) {
        let attached = attached_machines(program);
        let entry_names = EntryNames::of(program, &attached);
        let mut work = work.iter().copied().map(symbol_key).collect::<Vec<_>>();
        work.sort_unstable();
        for machine_position in 0..program.machines.len() {
            let machine = &program.machines[machine_position];
            if work.binary_search(&symbol_key(machine.symbol)).is_err()
                || !unambiguous_canonical_entry(program, &entry_names, machine)
            {
                continue;
            }
            let states = machine.states;
            let Some(&entry_handle) = program.machine_state_handles(states).first() else {
                continue;
            };
            let entry = program.machine_state(entry_handle);
            let identity = RecursiveEntry {
                machine: machine.symbol,
                entry: entry.symbol,
                owner: machine.attached_data_symbol,
            };
            let result = entry.return_type.clone();
            for state_position in 0..program.machine_state_handles(states).len() {
                let state_handle = program.machine_state_handles(states)[state_position];
                let state = program.machine_state(state_handle);
                if state.return_type == result {
                    normalize_state(program, identity, state.statement_nodes);
                }
            }
        }
    }

    fn unambiguous_canonical_entry(
        program: &SymbolResolvedTrees,
        entry_names: &EntryNames,
        machine: &Machine,
    ) -> bool {
        if !machine.attached_data_symbol.is_valid() {
            return true;
        }
        let canonical_symbol = if machine.target_family.is_valid() {
            machine.target_family
        } else {
            machine.symbol
        };
        let canonical = if canonical_symbol == machine.symbol {
            Some(machine)
        } else {
            program.machine(canonical_symbol)
        };
        let Some(canonical) = canonical else {
            return false;
        };
        let Some(&entry) = program.machine_state_handles(canonical.states).first() else {
            return false;
        };
        let entry_name = program.symbols.name(program.machine_state(entry).symbol);
        let mut candidates = entry_names.candidates(canonical.attached_data_symbol, entry_name);
        candidates.next() == Some(canonical_symbol) && candidates.next().is_none()
    }

    fn normalize_state(
        program: &mut SymbolResolvedTrees,
        identity: RecursiveEntry,
        statements: HandleSpan<StatementNode>,
    ) {
        let final_position = statements.len().checked_sub(1);
        for (position, statement_handle) in statements.handles().enumerate() {
            match body_statements(program).statement(statement_handle).clone() {
                StatementNode::Expression(expression) if Some(position) == final_position => {
                    if let Some(target) = recursive_target(program, identity, expression) {
                        let source_span = program.tables.bodies.expressions.source_span(expression);
                        let target = program
                            .tables
                            .bodies
                            .statements
                            .insert_transition_target(target);
                        *program
                            .tables
                            .bodies
                            .statements
                            .statement_mut(statement_handle) =
                            StatementNode::Transition(TableTransition {
                                target,
                                source_span,
                                ..Default::default()
                            });
                    }
                }
                StatementNode::Transition(transition) => {
                    normalize_target(program, identity, transition.target);
                    if transition.continuation.is_valid() {
                        normalize_target(program, identity, transition.continuation);
                    }
                }
                _ => {}
            }
        }
    }

    fn normalize_target(
        program: &mut SymbolResolvedTrees,
        identity: RecursiveEntry,
        target: TransitionTargetHandle,
    ) {
        let TransitionTargetNode::Value(expression) =
            *program.tables.bodies.statements.transition_target(target)
        else {
            return;
        };
        if let Some(named) = recursive_target(program, identity, expression) {
            *program
                .tables
                .bodies
                .statements
                .transition_target_mut(target) = named;
        }
    }

    fn recursive_target(
        program: &mut SymbolResolvedTrees,
        identity: RecursiveEntry,
        expression: ExpressionHandle,
    ) -> Option<TransitionTargetNode> {
        let expressions = &program.tables.bodies.expressions;
        let call = expressions.call_node(expression)?;
        if !identity.entry.is_valid()
            || (call.target_symbol != identity.entry && call.target_symbol != identity.machine)
            || !call.machine_arguments.is_empty()
        {
            return None;
        }
        if call.receiver.is_valid()
            && !matches!(expressions.expression(call.receiver), ExpressionNode::Name(receiver)
            if identity.owner.is_valid() && receiver.symbol == identity.owner)
        {
            return None;
        }
        let mut occurrences = expressions.authored_selection_occurrences(expression);
        let authored_call_selection = occurrences.next();
        if occurrences.next().is_some() {
            return None;
        }
        let statements = &mut program.tables.bodies.statements;
        let arguments = expressions.expression_handles(call.arguments);
        let retained = statements.reserve_expression_handles(u32::try_from(arguments.len()).ok()?);
        for (position, &argument) in arguments.iter().enumerate() {
            statements.set_expression_handle_at_offset(
                retained,
                u32::try_from(position).ok()?,
                argument,
            );
        }
        Some(TransitionTargetNode::Named {
            operational_acknowledgement: call.operational_acknowledgement,
            path: TableNamePath {
                receiver_symbol: Default::default(),
                members: statements.insert_name_path_members(std::slice::from_ref(&call.target)),
                head_symbol: identity.machine,
                symbol: identity.machine,
            },
            arguments: retained,
            evidence_arguments: call.evidence_arguments.clone(),
            source_span: call.target.source_span(),
            authored_call_selection,
        })
    }
}
pub(crate) mod requirement_namespaces {
    /*
        Keep an evidence binder's local methods aligned with their declarations.

        Symbol creation lists the public methods in declaration order. Constant
        evaluation later uses that same list to find the declaration represented by
        a selected local child, while retaining the child's exact binder and target.
    */

    use crate::tables::resolved_declarations;
    use source::SourceMap;
    use symbol_resolved_trees::{
        SymbolResolvedTrees, machine::GenericConformanceBound, name::DiagnosticName,
        signature::StateSignature, trait_definition::TraitDefinition,
    };
    use symbols::{SymbolHandle, SymbolKind};

    pub(crate) fn visible_requirements<'program>(
        program: &'program SymbolResolvedTrees,
        bound: &GenericConformanceBound,
        sources: Option<&SourceMap>,
    ) -> Vec<&'program StateSignature> {
        let Some(declaration) = select_visible_trait(program, &bound.trait_name, sources) else {
            return Vec::new();
        };
        let mut requirements = Vec::new();
        collect_requirements(
            program,
            declaration,
            &mut |reference| Ok(select_visible_trait(program, &reference.name, sources)),
            &mut Vec::new(),
            &mut requirements,
        )
        .unwrap_or_else(|never: std::convert::Infallible| match never {});
        requirements
    }

    pub(crate) fn evidence_requirement(
        program: &SymbolResolvedTrees,
        target: SymbolHandle,
    ) -> Result<Option<&StateSignature>, String> {
        let selected = program.symbols.get(target);
        if selected.kind != SymbolKind::State
            || !selected.parent.is_valid()
            || program.symbols.get(selected.parent).kind != SymbolKind::ConformanceParameter
        {
            return Ok(None);
        }
        let binder = selected.parent;
        let owner = program.symbols.get(binder).parent;
        let bounds = declaration_conformance_bounds(program, owner)
            .ok_or("constant evidence child lost its declaring bound owner")?;
        let mut matches = bounds.iter().filter(|bound| bound.binder == binder);
        let bound = matches
            .next()
            .ok_or("constant evidence child lost its exact authored bound")?;
        if matches.next().is_some() {
            return Err("constant evidence child has multiple authored bounds".into());
        }
        let declaration = exact_trait(program, bound.trait_symbol)?;
        let mut requirements = Vec::new();
        collect_requirements(
            program,
            declaration,
            &mut |reference| exact_trait(program, reference.symbol).map(Some),
            &mut Vec::new(),
            &mut requirements,
        )?;
        let children = program
            .symbols
            .child_handles(binder)
            .ok_or("constant evidence binder lost its requirement children")?
            .collect::<Vec<_>>();
        if children.len() != requirements.len()
            || children
                .iter()
                .any(|child| program.symbols.get(*child).kind != SymbolKind::State)
        {
            return Err(
                "constant evidence namespace disagrees with its declared requirements".into(),
            );
        }
        let ordinal = children
            .iter()
            .position(|child| *child == target)
            .ok_or("constant evidence requirement is not an exact binder child")?;
        Ok(Some(requirements[ordinal]))
    }

    fn declaration_conformance_bounds(
        program: &SymbolResolvedTrees,
        owner: SymbolHandle,
    ) -> Option<&[GenericConformanceBound]> {
        program
            .machines
            .iter()
            .find(|declaration| declaration.symbol == owner)
            .map(|declaration| declaration.conformance_bounds.as_slice())
            .or_else(|| {
                program
                    .traits
                    .iter()
                    .find(|declaration| declaration.symbol == owner)
                    .map(|declaration| declaration.conformance_bounds.as_slice())
            })
            .or_else(|| {
                program
                    .data_definition_by_symbol(owner)
                    .map(|declaration| declaration.conformance_bounds.as_slice())
            })
            .or_else(|| {
                program
                    .type_aliases
                    .iter()
                    .find(|declaration| declaration.symbol == owner)
                    .map(|declaration| declaration.conformance_bounds.as_slice())
            })
            .or_else(|| {
                program
                    .traits
                    .iter()
                    .flat_map(|declaration| program.trait_machine_signatures(declaration.machines))
                    .find(|signature| signature.symbol == owner)
                    .map(|signature| signature.conformance_bounds.as_slice())
            })
            .or_else(|| {
                resolved_declarations(program)
                    .data_type_parameters
                    .iter()
                    .find_map(|(_, parameter)| {
                        if parameter.symbol != owner {
                            return None;
                        }
                        let symbol_resolved_trees::data::TypeParameterKind::Machine { contract } =
                            &parameter.kind
                        else {
                            return None;
                        };
                        contract
                            .structural()
                            .map(|signature| signature.conformance_bounds.as_slice())
                    })
            })
    }

    fn exact_trait(
        program: &SymbolResolvedTrees,
        symbol: SymbolHandle,
    ) -> Result<&TraitDefinition, String> {
        let mut declarations = program
            .traits
            .iter()
            .filter(|declaration| symbol.is_valid() && declaration.symbol == symbol);
        let declaration = declarations
            .next()
            .ok_or("constant evidence namespace lost a resolved trait declaration")?;
        if declarations.next().is_some() {
            return Err("constant evidence namespace has ambiguous trait identity".into());
        }
        Ok(declaration)
    }

    fn collect_requirements<'program, Error>(
        program: &'program SymbolResolvedTrees,
        declaration: &'program TraitDefinition,
        resolve_parent: &mut impl FnMut(
            &symbol_resolved_trees::trait_definition::TraitRequirement,
        ) -> Result<Option<&'program TraitDefinition>, Error>,
        visited: &mut Vec<&'program TraitDefinition>,
        output: &mut Vec<&'program StateSignature>,
    ) -> Result<(), Error> {
        if visited
            .iter()
            .any(|known| std::ptr::eq(*known, declaration))
        {
            return Ok(());
        }
        visited.push(declaration);
        output.extend(program.trait_machine_signatures(declaration.machines));
        for reference in declaration
            .refines
            .iter()
            .chain(program.trait_requirements(declaration.requires))
        {
            if let Some(parent) = resolve_parent(reference)? {
                collect_requirements(program, parent, resolve_parent, visited, output)?;
            }
        }
        Ok(())
    }

    fn select_visible_trait<'program>(
        program: &'program SymbolResolvedTrees,
        reference_name: &DiagnosticName,
        sources: Option<&SourceMap>,
    ) -> Option<&'program TraitDefinition> {
        let reference = reference_name.source_span();
        let candidates = program
            .traits
            .iter()
            .filter(|candidate| {
                candidate.name == *reference_name
                    && sources.is_none_or(|sources| {
                        sources
                            .reference_can_see_declaration(reference, candidate.name.source_span())
                    })
            })
            .collect::<Vec<_>>();
        candidates
            .iter()
            .copied()
            .find(|candidate| candidate.name.source_span().source_id == reference.source_id)
            .or_else(|| {
                sources.and_then(|sources| {
                    candidates.iter().copied().find(|candidate| {
                        !sources.resolution_strata_separate(reference, candidate.name.source_span())
                    })
                })
            })
            .or_else(|| candidates.first().copied())
    }
}
mod scope {
    use super::lookup::child_symbol_by_kinds;
    use arena::{Arena, OrderedRootArena};
    use language_core::is_self_receiver;
    use symbol_resolved_trees::SymbolResolvedTrees;
    use symbol_resolved_trees::data::{DataDefinition, DataMember, TypeParameter};
    use symbol_resolved_trees::lookup_indexes::ResolvedIndexes;
    use symbol_resolved_trees::type_aliases::TypeAliasDefinition;
    use symbol_resolved_trees::types::TypeReference;
    use symbols::{SymbolHandle, SymbolLookup, SymbolTable};

    /*
        Owner binders retain their declaration identities in attached signatures
        and bodies. Method-local names shadow them without rebinding the owner's
        telescope. This supplies a lexical type scope, not an implicit receiver,
        value argument, or new machine-local generic parameter.
    */
    pub(super) fn declaration_type_scope(
        local_parameters: &[TypeParameter],
        owner_parameters: &[TypeParameter],
    ) -> Vec<TypeParameter> {
        let mut parameters = local_parameters.to_vec();
        parameters.extend(
            owner_parameters
                .iter()
                .filter(|owner| {
                    !local_parameters
                        .iter()
                        .any(|local| local.name == owner.name)
                })
                .cloned(),
        );
        parameters
    }

    /*
        Machine evidence has its own durable binder roster. Reuse these exact
        handles wherever source signatures need the complete lexical type scope.
    */
    pub(super) fn evidence_type_parameters(
        bounds: &[symbol_resolved_trees::machine::GenericConformanceBound],
    ) -> impl Iterator<Item = TypeParameter> + '_ {
        bounds.iter().map(|bound| TypeParameter {
            symbol: bound.binder,
            name: bound.binder_name.clone(),
            kind: symbol_resolved_trees::data::TypeParameterKind::Implementation,
            bounds: Default::default(),
        })
    }

    /*
        Resolve the construction ordinal and finalized declaration through the same
        exact row owner. Inline member machines are root declarations; their symbol
        parent is not the conformance's lexical ownership roster.
    */
    pub(crate) fn owned_conformance_realization(
        program: &SymbolResolvedTrees,
        row: &symbol_resolved_trees::trait_definition::ConformanceRow,
    ) -> Result<SymbolHandle, &'static str> {
        if !row.source.owns_realization() {
            return Err("referenced conformance row has no owned realization");
        }
        let provisional = row
            .provisional_realization_ordinal
            .filter(|ordinal| *ordinal < program.machines.len())
            .map(|ordinal| &program.machines[ordinal])
            .map(|machine| machine.symbol);
        if row.provisional_realization_ordinal.is_some() && provisional.is_none() {
            return Err("owned conformance realization has an invalid construction ordinal");
        }
        let retained = row.realization_machine;
        let selected = if retained.is_valid() {
            provisional
                .is_none_or(|symbol| symbol == retained)
                .then_some(retained)
        } else {
            provisional.filter(|symbol| symbol.is_valid())
        }
        .ok_or("owned conformance realization lost its exact enclosing capture scope")?;
        if program.machines.unique_by_symbol(selected).is_none() {
            return Err("owned conformance realization has no unique retained machine declaration");
        }
        Ok(selected)
    }

    /*
        Publish authored containment when declaration symbols first exist. This is
        independent of requirement-overload normalization: a provisional row still
        has to agree with any retained realization. Every later scope consumer reads
        this same inverse relationship instead of reconstructing it from all maps.
    */
    pub(super) fn record_declaration_owners(
        program: &SymbolResolvedTrees,
        symbols: &mut SymbolTable,
    ) -> Result<(), Vec<diagnostics::Diagnostic>> {
        let mut ownership = Vec::new();
        let mut diagnostics = Vec::new();
        for conformance in program.conformances.iter() {
            for row in conformance
                .rows
                .iter()
                .filter(|row| row.source.owns_realization())
            {
                let machine = match owned_conformance_realization(program, row) {
                    Ok(machine) => machine,
                    Err(reason) => {
                        diagnostics.push(diagnostics::Diagnostic::error(reason));
                        continue;
                    }
                };
                if ownership
                    .iter()
                    .any(|(declaration, _)| *declaration == machine)
                {
                    diagnostics.push(diagnostics::Diagnostic::error(
                        "owned conformance realization has ambiguous enclosing capture scope",
                    ));
                } else {
                    ownership.push((machine, conformance.symbol));
                }
            }
        }
        if !diagnostics.is_empty() {
            return Err(diagnostics);
        }
        for (declaration, owner) in ownership {
            symbols
                .record_declaration_owner(declaration, owner)
                .map_err(|reason| vec![diagnostics::Diagnostic::error(reason)])?;
        }
        Ok(())
    }

    /*
        Exact namespace owning this machine, if it is a retained map member.
        A second row cannot lend its capture or normalization domain to the first.
    */
    pub(crate) fn owned_conformance_namespace(
        program: &SymbolResolvedTrees,
        machine: SymbolHandle,
    ) -> Result<SymbolHandle, &'static str> {
        let owner = program.symbols.get(machine).declaration_owner;
        if !owner.is_valid() {
            return Ok(owner);
        }
        for conformance in program
            .conformances
            .iter()
            .filter(|map| map.symbol == owner)
        {
            for row in conformance
                .rows
                .iter()
                .filter(|row| row.source.owns_realization())
            {
                if owned_conformance_realization(program, row)? == machine {
                    return Ok(owner);
                }
            }
        }
        Ok(SymbolHandle::invalid())
    }

    /*
        Static declaration context of owned map rows and nested machines. These
        exact binders are visible during formation without becoming runtime captures
        or fresh invocation parameters. Referenced map rows capture nothing.
    */
    pub(super) struct DeclarationTypeScopes {
        owned: Vec<(SymbolHandle, Vec<TypeParameter>)>,
    }

    impl DeclarationTypeScopes {
        pub(super) fn of(
            program: &SymbolResolvedTrees,
        ) -> Result<Self, Vec<diagnostics::Diagnostic>> {
            let mut owned = Vec::new();
            let mut diagnostics = Vec::new();
            for conformance in program.conformances.iter() {
                for row in conformance
                    .rows
                    .iter()
                    .filter(|row| row.source.owns_realization())
                {
                    let machine = match owned_conformance_realization(program, row) {
                        Ok(machine) => machine,
                        Err(reason) => {
                            diagnostics.push(diagnostics::Diagnostic::error(reason));
                            continue;
                        }
                    };
                    if owned.iter().any(|(owner, _)| *owner == machine) {
                        diagnostics.push(diagnostics::Diagnostic::error(
                            "owned conformance realization has ambiguous enclosing capture scope",
                        ));
                        continue;
                    }
                    owned.push((
                        machine,
                        program
                            .data_type_parameters(conformance.type_parameters)
                            .to_vec(),
                    ));
                }
            }
            for machine in program
                .machines
                .iter()
                .filter(|machine| machine.lexical_parent.is_valid())
            {
                let parent = program.machines.get(machine.lexical_parent);
                let mut parameters = program.machine_type_parameters(parent).to_vec();
                parameters.extend(evidence_type_parameters(&parent.conformance_bounds));
                if let Some((_, inherited)) =
                    owned.iter().find(|(owner, _)| *owner == parent.symbol)
                {
                    parameters = declaration_type_scope(&parameters, inherited);
                }
                if let Some(data) = program
                    .data_definitions
                    .iter()
                    .find(|data| data.symbol == parent.attached_data_symbol)
                {
                    parameters = declaration_type_scope(
                        &parameters,
                        program.data_type_parameters(data.type_parameters),
                    );
                }
                if let Some((_, inherited)) =
                    owned.iter_mut().find(|(owner, _)| *owner == machine.symbol)
                {
                    *inherited = declaration_type_scope(inherited, &parameters);
                } else {
                    owned.push((machine.symbol, parameters));
                }
            }
            if diagnostics.is_empty() {
                Ok(Self { owned })
            } else {
                Err(diagnostics)
            }
        }

        pub(super) fn parameters(&self, machine: SymbolHandle) -> &[TypeParameter] {
            let mut owners = self.owned.iter().filter(|(owner, _)| *owner == machine);
            let Some((_, parameters)) = owners.next() else {
                return &[];
            };
            if owners.next().is_some() {
                return &[];
            }
            parameters
        }
    }

    #[derive(Clone, Copy)]
    pub(super) struct AttachedMachine {
        pub(super) owner: SymbolHandle,
        pub(super) machine: SymbolHandle,
        pub(super) entry: SymbolHandle,
    }

    /*
        The attached-machine roster together with the by-owner index every
        ambient-method lookup re-derives: the walk used to bucket the roster
        through a thread-local keyed on the slice address; carrying the index in
        the roster makes the built-once index a value the caller owns.
    */
    pub(super) struct AttachedMachines {
        pub(super) list: Vec<AttachedMachine>,
        by_owner: symbols::SymbolKeyMap<SymbolHandle, Vec<usize>>,
    }

    impl AttachedMachines {
        /* Roster positions of machines attached to `owner`, in roster order. */
        pub(super) fn positions_for_owner(&self, owner: SymbolHandle) -> &[usize] {
            self.by_owner.get(&owner).map(Vec::as_slice).unwrap_or(&[])
        }
    }

    pub(super) fn attached_machines(program: &SymbolResolvedTrees) -> AttachedMachines {
        let list = attached_machine_list(program);
        let mut by_owner: symbols::SymbolKeyMap<SymbolHandle, Vec<usize>> = Default::default();
        for (index, entry) in list.iter().enumerate() {
            by_owner.entry(entry.owner).or_default().push(index);
        }
        AttachedMachines { list, by_owner }
    }

    fn attached_machine_list(program: &SymbolResolvedTrees) -> Vec<AttachedMachine> {
        program
            .machines
            .iter()
            .enumerate()
            .filter_map(|(ordinal, machine)| {
                let closed_realization = program.conformances.iter().any(|conformance| {
                    let rows = &conformance.rows;
                    rows.iter().any(|row| {
                        row.source.owns_realization()
                            && row.provisional_realization_ordinal.map_or_else(
                                || {
                                    row.realization_machine.is_valid()
                                        && row.realization_machine == machine.symbol
                                },
                                |selected| selected == ordinal,
                            )
                    })
                });
                (!closed_realization
                    && machine.symbol.is_valid()
                    && machine.attached_data_symbol.is_valid()
                    && machine.target.is_none())
                .then_some(AttachedMachine {
                    owner: machine.attached_data_symbol,
                    machine: machine.symbol,
                    entry: program
                        .machine_state_handles(machine.states)
                        .first()
                        .map(|state| program.machine_state(*state).symbol)
                        .unwrap_or_else(SymbolHandle::invalid),
                })
            })
            .collect()
    }

    /*
        For every trait, the requirements it inherits through `requires` and
        header parents, built once per assignment batch from declaration
        identities. A trait's own requirements are its symbol children and need no
        roster.
    */
    pub(super) struct TraitLexicalScopes {
        inherited: Vec<(SymbolHandle, Vec<SymbolHandle>)>,
    }

    impl TraitLexicalScopes {
        pub(super) fn of(program: &SymbolResolvedTrees) -> Self {
            let inherited = program
                .traits
                .iter()
                .map(|trait_definition| {
                    let mut visited = vec![trait_definition.symbol];
                    let mut requirements = Vec::new();
                    for parent in program.trait_requirements(trait_definition.requires) {
                        collect_trait_requirements(
                            program,
                            parent.symbol,
                            &mut visited,
                            &mut requirements,
                        );
                    }
                    (trait_definition.symbol, requirements)
                })
                .collect();
            Self { inherited }
        }

        /*
            The bare requirement names visible in `owner`'s lexical scope; an
            invalid owner sees none.
        */
        pub(super) fn requirements_of(&self, owner: SymbolHandle) -> LexicalRequirements<'_> {
            LexicalRequirements {
                owner,
                inherited: self
                    .inherited
                    .iter()
                    .find(|(trait_symbol, _)| owner.is_valid() && *trait_symbol == owner)
                    .map_or(&[], |(_, requirements)| requirements.as_slice()),
            }
        }
    }

    fn collect_trait_requirements(
        program: &SymbolResolvedTrees,
        trait_symbol: SymbolHandle,
        visited: &mut Vec<SymbolHandle>,
        requirements: &mut Vec<SymbolHandle>,
    ) {
        if !trait_symbol.is_valid() || visited.contains(&trait_symbol) {
            return;
        }
        visited.push(trait_symbol);
        let Some(trait_definition) = program.trait_definition(trait_symbol) else {
            return;
        };
        for requirement in program.trait_machine_signatures(trait_definition.machines) {
            if requirement.symbol.is_valid() && !requirements.contains(&requirement.symbol) {
                requirements.push(requirement.symbol);
            }
        }
        for parent in program.trait_requirements(trait_definition.requires) {
            collect_trait_requirements(program, parent.symbol, visited, requirements);
        }
    }

    /*
        Requirements a receiverless call may name inside one trait's lexical scope:
        a requirement contract written in the trait, or a default body the trait
        authored. Default synthesis instantiates that body onto each conforming
        carrier, but the body stays the trait's text, so its bare names keep the
        trait's scope rather than the carrier's
        ([complete rows](../../../../../../../wiki/spec/language/conformances.md#complete-row-identity):
        "Default bodies call other requirements through this same selected map").
        Selection names the requirement declaration itself; each instantiation's
        conformance row is chosen later, after the authored-selection ledger
        records the requirement as the trait author's own selection.
    */
    #[derive(Clone, Copy)]
    pub(super) struct LexicalRequirements<'program> {
        /* The trait whose own requirements are visible; invalid outside a trait. */
        owner: SymbolHandle,
        /* Requirements `owner` inherits, each once. */
        inherited: &'program [SymbolHandle],
    }

    impl LexicalRequirements<'_> {
        pub(super) fn none() -> Self {
            Self {
                owner: SymbolHandle::invalid(),
                inherited: &[],
            }
        }

        /*
            The trait's own requirement wins, as its own declaration shadows an
            inherited one of the same name. An inherited name must select one
            requirement: a short name is not a row identity, so a name several
            parents declare stays with ordinary resolution.
        */
        pub(super) fn select(&self, symbols: &SymbolTable, name: &str) -> SymbolHandle {
            if !self.owner.is_valid() {
                return SymbolHandle::invalid();
            }
            let own =
                child_symbol_by_kinds(symbols, self.owner, &[symbols::SymbolKind::State], name);
            if own.is_valid() {
                return own;
            }
            let mut matching = self
                .inherited
                .iter()
                .copied()
                .filter(|requirement| symbols.name(*requirement) == name);
            match (matching.next(), matching.next()) {
                (Some(selected), None) => selected,
                _ => SymbolHandle::invalid(),
            }
        }
    }

    pub(super) struct MachineScope<'program> {
        pub(super) symbol: SymbolHandle,
        /* Bare requirement names of the trait this scope is lexically inside. */
        pub(super) lexical_requirements: LexicalRequirements<'program>,
        pub(super) attached_machines: &'program AttachedMachines,
        /*
            The program's owned position indexes — local-statement, parameter,
            and data-definition positions resolve through these instead of
            rescanning (or a thread-local side table).
        */
        pub(super) resolution_indexes: &'program ResolvedIndexes,
        pub(super) type_parameters: &'program [symbol_resolved_trees::data::TypeParameter],
        pub(super) attached_data_symbol: SymbolHandle,
        pub(super) inherited_data_members:
            Option<&'program [symbol_resolved_trees::data::DataMember]>,
        pub(super) owned_data: &'program [symbol_resolved_trees::machine::OwnedData],
        /*
            Only the already-stamped prefix of the current state's statements.
            Local receiver types cannot come from a later declaration or from the
            binding whose initializer is currently being resolved.
        */
        pub(super) prior_statements: &'program [symbol_resolved_trees::statement::Statement],
        /*
            All top-level data definitions and the shared member arena -- lets the
            receiver walk resolve a NESTED member chain's declared field types
            (`self.p.a` -> `p: PairD` -> `a: BoxI`). Empty for scopes built outside
            body resolution (field-initializer resolution), where nested method
            receivers do not occur.
        */
        pub(super) data_definitions: &'program OrderedRootArena<DataDefinition>,
        /* Alias headers supply argument roles, never runtime receiver members. */
        pub(super) type_aliases: &'program OrderedRootArena<TypeAliasDefinition>,
        pub(super) conformances:
            &'program OrderedRootArena<symbol_resolved_trees::trait_definition::Conformance>,
        pub(super) traits:
            &'program OrderedRootArena<symbol_resolved_trees::trait_definition::TraitDefinition>,
        pub(super) operand_owners:
            &'program Arena<symbol_resolved_trees::types::OperandLifetimeOwner>,
        pub(super) data_members: &'program Arena<DataMember>,
        pub(super) data_type_parameters: &'program Arena<TypeParameter>,
        pub(super) data_payload_fields: &'program Arena<symbol_resolved_trees::data::DataField>,
        pub(super) type_constraints: &'program Arena<symbol_resolved_trees::types::TypeConstraint>,
    }

    /*
        The reference bundle every contract-time `MachineScope` shares. The four
        scope literals in `symbols/contracts` differ only in the fields this
        constructor takes; the rest are constants.
    */
    pub(super) struct ContractScopeContext<'program> {
        pub(super) attached_machines: &'program AttachedMachines,
        pub(super) resolution_indexes: &'program ResolvedIndexes,
        pub(super) data_definitions: &'program OrderedRootArena<DataDefinition>,
        pub(super) type_aliases: &'program OrderedRootArena<TypeAliasDefinition>,
        pub(super) conformances:
            &'program OrderedRootArena<symbol_resolved_trees::trait_definition::Conformance>,
        pub(super) traits:
            &'program OrderedRootArena<symbol_resolved_trees::trait_definition::TraitDefinition>,
        pub(super) operand_owners:
            &'program Arena<symbol_resolved_trees::types::OperandLifetimeOwner>,
        pub(super) data_members: &'program Arena<DataMember>,
        pub(super) data_type_parameters: &'program Arena<TypeParameter>,
        pub(super) data_payload_fields: &'program Arena<symbol_resolved_trees::data::DataField>,
        pub(super) type_constraints: &'program Arena<symbol_resolved_trees::types::TypeConstraint>,
    }

    impl<'program> ContractScopeContext<'program> {
        pub(super) fn scope(
            &self,
            symbol: SymbolHandle,
            lexical_requirements: LexicalRequirements<'program>,
            type_parameters: &'program [symbol_resolved_trees::data::TypeParameter],
            attached_data_symbol: SymbolHandle,
            inherited_data_members: Option<&'program [DataMember]>,
            owned_data: &'program [symbol_resolved_trees::machine::OwnedData],
        ) -> MachineScope<'program> {
            MachineScope {
                symbol,
                lexical_requirements,
                attached_machines: self.attached_machines,
                resolution_indexes: self.resolution_indexes,
                type_parameters,
                attached_data_symbol,
                inherited_data_members,
                owned_data,
                prior_statements: &[],
                data_definitions: self.data_definitions,
                type_aliases: self.type_aliases,
                conformances: self.conformances,
                traits: self.traits,
                operand_owners: self.operand_owners,
                data_members: self.data_members,
                data_type_parameters: self.data_type_parameters,
                data_payload_fields: self.data_payload_fields,
                type_constraints: self.type_constraints,
            }
        }
    }

    impl<'program> MachineScope<'program> {
        pub(super) fn declaration_scope(
            &self,
            declaration: SymbolHandle,
            parameters: &'program [TypeParameter],
        ) -> Self {
            Self {
                symbol: declaration,
                lexical_requirements: LexicalRequirements::none(),
                type_parameters: parameters,
                attached_data_symbol: SymbolHandle::invalid(),
                inherited_data_members: None,
                owned_data: &[],
                prior_statements: &[],
                ..*self
            }
        }

        /*
            A copied type operand's existing input receipt owns its names. An
            omitted/ambiguous header or malformed span supplies no lexical context.
        */
        pub(super) fn supplied_scope(
            &self,
            owner: symbol_resolved_trees::types::OperandLifetimeOwnerHandle,
        ) -> Option<Self> {
            let declaration = self.operand_owners.get(owner).declaration;
            if !declaration.is_valid() {
                return None;
            }
            let mut declarations = self
                .conformances
                .iter()
                .filter(|header| header.symbol == declaration)
                .map(|header| header.type_parameters)
                .chain(
                    self.traits
                        .iter()
                        .filter(|header| header.symbol == declaration)
                        .map(|header| header.type_parameters),
                );
            let type_parameters = declarations.next()?;
            if declarations.next().is_some() {
                return None;
            }
            let parameters = self.data_type_parameters.span_or_empty(type_parameters);
            if parameters.len() != type_parameters.count() as usize {
                return None;
            }
            Some(self.declaration_scope(declaration, parameters))
        }
    }

    impl MachineScope<'_> {
        pub(super) fn attached_call_target(
            &self,
            symbols: &SymbolTable,
            owner: SymbolHandle,
            target: &symbol_resolved_trees::name::DiagnosticName,
        ) -> SymbolHandle {
            if !owner.is_valid() || symbols.get(owner).kind != symbols::SymbolKind::Data {
                return SymbolHandle::invalid();
            }
            let owner_attachments = self.attached_machines.positions_for_owner(owner);
            let owner_machines: std::collections::HashSet<SymbolHandle> = owner_attachments
                .iter()
                .map(|index| self.attached_machines.list[*index].machine)
                .collect();
            let mut selected = SymbolHandle::invalid();
            for attachment in owner_attachments
                .iter()
                .map(|index| &self.attached_machines.list[*index])
            {
                let state = attachment.entry;
                if !state.is_valid() || symbols.name(state) != target.as_str() {
                    continue;
                }
                let path = symbols.display_path(attachment.machine, "::");
                let selects = |path: &str| match symbols
                    .lookup_top_level_by_name_and_kinds_from_source_matching(
                        path,
                        &[symbols::SymbolKind::Machine],
                        target.source_span(),
                        |candidate| owner_machines.contains(&candidate),
                    ) {
                    SymbolLookup::Unique(candidate) => candidate == attachment.machine,
                    SymbolLookup::Ambiguous { first, second } => {
                        attachment.machine == first || attachment.machine == second
                    }
                    SymbolLookup::NotFound => false,
                };
                let mut visible = selects(&path);
                if !visible {
                    for import in symbols.source_module_import_paths(target.source_span().source_id)
                    {
                        let mut prefix_end = 0;
                        for member in import.split("::") {
                            prefix_end += member.len();
                            if selects(&format!("{}::{path}", &import[..prefix_end])) {
                                visible = true;
                                break;
                            }
                            prefix_end += 2;
                        }
                        if visible {
                            break;
                        }
                    }
                }
                if !visible {
                    continue;
                }
                if selected.is_valid() {
                    continue;
                }
                selected = state;
            }
            selected
        }

        pub(super) fn field_type_reference(
            &self,
            symbols: &SymbolTable,
            field_symbol: SymbolHandle,
        ) -> Option<&symbol_resolved_trees::types::TypeReference> {
            if symbols.get(self.symbol).kind == symbols::SymbolKind::Variant
                && symbols.get(field_symbol).parent == self.symbol
            {
                let payload =
                    self.inherited_data_members?
                        .iter()
                        .find_map(|member| match member {
                            DataMember::Variant(variant) if variant.symbol == self.symbol => {
                                Some(variant.payload)
                            }
                            _ => None,
                        })?;
                return self
                    .data_payload_fields
                    .span_or_empty(payload)
                    .iter()
                    .find(|field| field.symbol == field_symbol)
                    .map(|field| &field.type_reference);
            }
            if let Some(data_members) = self.inherited_data_members {
                for member in data_members {
                    let symbol_resolved_trees::data::DataMember::Field(field) = member else {
                        continue;
                    };
                    if field.symbol == field_symbol
                        || (field_symbol.is_valid()
                            && field.name.as_str() == symbols.name(field_symbol))
                    {
                        return Some(&field.type_reference);
                    }
                }
            }

            self.owned_data
                .iter()
                .find(|owned_data| owned_data.symbol == field_symbol)
                .map(|owned_data| &owned_data.type_reference)
        }

        /*
            Walk a `self`-rooted member chain of SPELLED names (`["self", "p", "a"]`)
            through the declared field types and return the exact type symbol AFTER the last
            segment (`"BoxI"` for `self.p.a` where `a: BoxI`). Each hop's field type
            must be a plain `Named` type: shell-wrapped intermediates (`&mut`,
            constrained, arrays) return `None` -- conservative, so an unsupported
            nested receiver keeps the existing loud unresolved-call error rather
            than silently binding 0. `None` unless the root is `self` and every hop
            resolves. Used by the nested-receiver symbol stamping (rung 2b of the
            receiver-place staircase).
        */
        pub(super) fn nested_self_chain_type(&self, chain: &[&str]) -> Option<SymbolHandle> {
            let (root, hops) = chain.split_first()?;
            if !is_self_receiver(root) {
                return None;
            }
            let mut current_symbol = self.attached_data_symbol;
            if !current_symbol.is_valid() {
                return None;
            }
            for hop in hops {
                let definition = &self.data_definitions[self
                    .resolution_indexes
                    .data_definition_position(self.data_definitions, current_symbol)?
                    as usize];
                let field = self
                    .data_members
                    .span_or_empty(definition.storage.members)
                    .iter()
                    .find_map(|member| match member {
                        DataMember::Field(field) if field.name.as_str() == *hop => Some(field),
                        _ => None,
                    })?;
                current_symbol = match &field.type_reference {
                    TypeReference::Named { symbol, .. } if symbol.is_valid() => *symbol,
                    _ => return None,
                };
            }
            Some(current_symbol)
        }
    }
}
pub(crate) use scope::{owned_conformance_namespace, owned_conformance_realization};
mod scoped_paths {
    use symbols::{JoinedPath, SymbolHandle, SymbolKind, SymbolLookup, SymbolTable};

    mod base {
        use symbols::{SymbolHandle, SymbolKind, SymbolLookup, SymbolTable};

        use super::super::lookup::{
            child_indexed_symbol_by_kinds, child_or_attached_data_child_symbol_by_kinds,
            child_symbol_by_kinds,
        };

        /*
            Preserve absence versus ambiguity for compound namespace selection. Only
            absence permits a longer certified address; ambiguity cannot be resolved by
            choosing the sole declaration that happens to have the requested descendant.
        */
        pub(super) fn resolve_base_lookup(
            symbols: &SymbolTable,
            machine_symbol: SymbolHandle,
            state_symbol: SymbolHandle,
            member: &symbol_resolved_trees::name::DiagnosticName,
        ) -> SymbolLookup {
            if state_symbol.is_valid() {
                let lexical_symbol = child_symbol_by_kinds(
                    symbols,
                    state_symbol,
                    &[SymbolKind::Parameter, SymbolKind::Local],
                    member.as_str(),
                );
                if lexical_symbol.is_valid() {
                    return SymbolLookup::Unique(lexical_symbol);
                }
            }

            let machine_child = child_or_attached_data_child_symbol_by_kinds(
                symbols,
                machine_symbol,
                &[
                    SymbolKind::Field,
                    SymbolKind::State,
                    SymbolKind::ConformanceParameter,
                ],
                member.as_str(),
            );
            if machine_child.is_valid() {
                return SymbolLookup::Unique(machine_child);
            }

            let lexical = super::super::lookup::lexical_declaration_lookup(
                symbols,
                machine_symbol,
                member.as_str(),
                &[
                    SymbolKind::Machine,
                    SymbolKind::TypeParameter,
                    SymbolKind::MachineParameter,
                    SymbolKind::ConformanceParameter,
                    SymbolKind::PropositionParameter,
                    SymbolKind::PropositionMachineParameter,
                ],
            );
            if !matches!(lexical, SymbolLookup::NotFound) {
                return lexical;
            }

            symbols.lookup_top_level_by_name_and_kinds_from_source_matching(
                member.as_str(),
                &[
                    SymbolKind::BuiltinType,
                    SymbolKind::Const,
                    SymbolKind::Data,
                    SymbolKind::Machine,
                    SymbolKind::Trait,
                    SymbolKind::Conformance,
                    SymbolKind::Module,
                ],
                member.source_span(),
                |_| true,
            )
        }

        pub(super) fn resolve_base_indexed_symbol(
            symbols: &SymbolTable,
            machine_symbol: SymbolHandle,
            state_symbol: SymbolHandle,
            member: &str,
            index: i64,
        ) -> SymbolHandle {
            if state_symbol.is_valid() {
                let parameter_symbol = child_indexed_symbol_by_kinds(
                    symbols,
                    state_symbol,
                    &[SymbolKind::Parameter],
                    member,
                    index,
                );
                if parameter_symbol.is_valid() {
                    return parameter_symbol;
                }
            }

            child_indexed_symbol_by_kinds(
                symbols,
                machine_symbol,
                &[SymbolKind::Field, SymbolKind::State],
                member,
                index,
            )
        }
    }

    use self::base::resolve_base_indexed_symbol;
    use super::lookup::{
        child_indexed_symbol_by_kinds, child_or_attached_data_child_symbol_by_kinds,
    };

    /*
        The symbol kinds a scoped path may resolve through at each member:
        fields, states, parameters, variants and the mathematical/constant
        declarations — the same roster every path-walk step consults.
    */
    const SCOPED_PATH_MEMBER_KINDS: &[SymbolKind] = &[
        SymbolKind::Field,
        SymbolKind::State,
        SymbolKind::Parameter,
        SymbolKind::Variant,
        SymbolKind::MathematicalDefinition,
        SymbolKind::Const,
    ];

    /*
        Resolve a state-scoped table path to its lexical head and selected leaf.
        `indexed_last_member` carries the path's possible indexed tail
        (`items[index]`): one walk serves both, and only the final member's lookup
        consults the indexed declaration form.
    */
    pub(super) fn resolve_state_scoped_table_path(
        symbols: &SymbolTable,
        machine_symbol: SymbolHandle,
        state_symbol: SymbolHandle,
        expression_table: &symbol_resolved_trees::expression::ExpressionTable,
        path: &symbol_resolved_trees::expression::TableNamePath,
        indexed_last_member: Option<i64>,
    ) -> (SymbolHandle, SymbolHandle) {
        let members = expression_table.name_path_members(path.members);
        resolve_state_scoped_members(
            symbols,
            machine_symbol,
            state_symbol,
            members,
            indexed_last_member,
        )
    }

    /*
        Retain each authored segment's declaration where it has one. Package-alias
        segments have zero identities; their complete prefix selects one exact
        declaration. An empty result means that selection or a descendant failed,
        so callers must not retain a partially selected suffix.
    */
    pub(super) fn resolve_state_scoped_table_path_member_symbols(
        symbols: &SymbolTable,
        machine_symbol: SymbolHandle,
        state_symbol: SymbolHandle,
        expression_table: &symbol_resolved_trees::expression::ExpressionTable,
        path: &symbol_resolved_trees::expression::TableNamePath,
    ) -> Vec<SymbolHandle> {
        let members = expression_table.name_path_members(path.members);
        if members.is_empty() {
            return Vec::new();
        }

        let (head, prefix_length) =
            resolve_path_head(symbols, machine_symbol, state_symbol, members);
        if !head.is_valid() {
            return Vec::new();
        }
        let suffix = resolve_path_members_from_head(symbols, head, &members[prefix_length..]);
        let mut resolved = vec![SymbolHandle::invalid(); prefix_length - 1];
        resolved.extend(suffix);

        if resolved.len() == members.len() {
            resolved
        } else {
            Default::default()
        }
    }

    /*
        A failed suffix does not invalidate the caller's independently selected
        lexical head, but it cannot publish a complete member-selection chain.
    */
    pub(super) fn resolve_path_members_from_head(
        symbols: &SymbolTable,
        head: SymbolHandle,
        suffix: &[symbol_resolved_trees::name::DiagnosticName],
    ) -> Vec<SymbolHandle> {
        let mut current = head;
        let mut resolved = vec![head];
        for member in suffix {
            current = child_or_attached_data_child_symbol_by_kinds(
                symbols,
                current,
                SCOPED_PATH_MEMBER_KINDS,
                member.as_str(),
            );
            if !current.is_valid() {
                return Vec::new();
            }
            resolved.push(current);
        }

        resolved
    }

    /*
        Resolve a run of authored path members to the lexical head and the selected
        leaf. `indexed_last_member` is the walk's only variation: when present, the
        final member's declaration is looked up in its indexed form (`name[index]`)
        instead of its plain form, and only at that final member.
    */
    pub(super) fn resolve_state_scoped_members(
        symbols: &SymbolTable,
        machine_symbol: SymbolHandle,
        state_symbol: SymbolHandle,
        members: &[symbol_resolved_trees::name::DiagnosticName],
        indexed_last_member: Option<i64>,
    ) -> (SymbolHandle, SymbolHandle) {
        if members.is_empty() {
            return invalid_symbol_pair();
        }

        let (head, prefix_length) =
            if let Some(last_index) = indexed_last_member.filter(|_| members.len() == 1) {
                (
                    resolve_base_indexed_symbol(
                        symbols,
                        machine_symbol,
                        state_symbol,
                        members[0].as_str(),
                        last_index,
                    ),
                    1,
                )
            } else {
                resolve_path_head(symbols, machine_symbol, state_symbol, members)
            };
        if !head.is_valid() {
            return invalid_symbol_pair();
        }
        let mut current = head;

        for (offset, member) in members[prefix_length..].iter().enumerate() {
            let is_last = offset + prefix_length + 1 == members.len();
            current = if let Some(last_index) = indexed_last_member.filter(|_| is_last) {
                child_indexed_symbol_by_kinds(
                    symbols,
                    current,
                    SCOPED_PATH_MEMBER_KINDS,
                    member.as_str(),
                    last_index,
                )
            } else {
                child_or_attached_data_child_symbol_by_kinds(
                    symbols,
                    current,
                    SCOPED_PATH_MEMBER_KINDS,
                    member.as_str(),
                )
            };
            if !current.is_valid() {
                return (head, SymbolHandle::invalid());
            }
        }

        (head, current)
    }

    /*
        Lexical values own their suffix before static-address lookup. When the
        first segment is not a declaration (a loader-certified package alias),
        select the longest exact declaration prefix through ordinary name law.
        This is the same operation for a call receiver and an ordinary name path;
        aliases never become runtime values or grant visibility to another package.
    */
    fn resolve_path_head(
        symbols: &SymbolTable,
        machine_symbol: SymbolHandle,
        state_symbol: SymbolHandle,
        members: &[symbol_resolved_trees::name::DiagnosticName],
    ) -> (SymbolHandle, usize) {
        match base::resolve_base_lookup(symbols, machine_symbol, state_symbol, &members[0]) {
            SymbolLookup::Unique(head) => return (head, 1),
            SymbolLookup::Ambiguous { .. } => return (SymbolHandle::invalid(), 1),
            SymbolLookup::NotFound => {}
        }
        for prefix_length in (2..=members.len()).rev() {
            let prefix = &members[..prefix_length];
            let name = symbol_resolved_trees::name::DiagnosticName::new(
                prefix.joined_path(),
                super::lookup::diagnostic_path_source_span(prefix),
            );
            match base::resolve_base_lookup(symbols, machine_symbol, state_symbol, &name) {
                SymbolLookup::Unique(selected) => return (selected, prefix_length),
                SymbolLookup::Ambiguous { .. } => return (SymbolHandle::invalid(), 1),
                SymbolLookup::NotFound => {}
            }
        }
        (SymbolHandle::invalid(), 1)
    }

    pub(super) fn invalid_symbol_pair() -> (SymbolHandle, SymbolHandle) {
        (SymbolHandle::invalid(), SymbolHandle::invalid())
    }
}
#[path = "symbols_statements.rs"]
mod statements;
pub(crate) mod target_machine_families {
    /*
        Resolve authored product target families once, while declaration names
        still participate in resolution. Later stages join bodies by these handles.
    */

    use diagnostics::Diagnostic;
    use source::DependencyScope;
    use symbol_resolved_trees::SymbolResolvedTrees;

    mod contracts {
        /*
            Prepare family premises in the interface's own resolved telescope.

            A portable caller must meet every possible member's premises, not those of
            a lexical prototype. Copying clauses without rebasing their operands would
            leave them rooted in private member formals. This pass uses positional
            correspondence and exact symbols; it neither chooses a target nor imports
            inferred body guarantees. Typing and checking subsequently join the full
            effect and crash envelope; execution selects a member without reopening
            a private contract as a portable guarantee.
        */

        use arena::HandleSpan;
        use diagnostics::Diagnostic;
        use symbol_resolved_trees::SymbolResolvedTrees;
        use symbol_resolved_trees::machine::Machine;
        use symbol_resolved_trees::signature::{SignatureContract, SignatureContractKind};
        use symbols::SymbolHandle;

        pub(crate) fn prepare_premises(
            program: &mut SymbolResolvedTrees,
        ) -> Result<(), Vec<Diagnostic>> {
            let interfaces = program
                .machines
                .iter()
                .enumerate()
                .filter_map(|(position, machine)| {
                    (machine.target_family_interface && !machine.target_family_contracts_prepared)
                        .then_some(position)
                })
                .collect::<Vec<_>>();
            for position in interfaces {
                let mut interface = program.machines[position].clone();
                let members = program
                    .machines
                    .iter()
                    .filter(|machine| {
                        machine.target_family == interface.symbol
                            && machine.authored_target.is_some()
                    })
                    .cloned()
                    .collect::<Vec<_>>();
                if members.is_empty() {
                    return Err(vec![Diagnostic::error(format!(
                        "target-family interface `{}` has no retained implementations",
                        interface.name,
                    ))]);
                }
                interface.type_parameters = join_property_demands(program, &interface, &members)
                    .map_err(|diagnostic| vec![diagnostic])?;
                program.machines[position].type_parameters = interface.type_parameters;
                let mut contracts = Vec::new();
                for member in &members {
                    let symbols = telescope_correspondence(program, &interface, member)
                        .map_err(|diagnostic| vec![diagnostic])?;
                    let mut member_contracts = program.machine_contracts(member).to_vec();
                    if let Some(entry) = program.machine_state_handles(member.states).first() {
                        member_contracts.extend_from_slice(
                            program.signature_contracts(program.machine_state(*entry).contracts),
                        );
                    }
                    for contract in member_contracts {
                        if !matches!(
                            contract.kind,
                            SignatureContractKind::Requires | SignatureContractKind::Crashes { .. }
                        ) {
                            continue;
                        }
                        contracts.push(copy_contract(program, contract, &symbols));
                    }
                }
                program.machines[position].contracts = program
                    .tables
                    .declarations
                    .signature_contracts
                    .insert_many(contracts);
                program.machines[position].target_family_contracts_prepared = true;
            }
            Ok(())
        }

        /*
            Affine/no-carry bounds impose no generic property demand. A portable
            application must establish every member's non-neutral demand; retaining
            the first member's weaker bound would check calls the other body cannot
            accept. The existing property carrier cannot encode conflicting demands.
        */
        fn join_property_demands(
            program: &mut SymbolResolvedTrees,
            interface: &Machine,
            members: &[Machine],
        ) -> Result<HandleSpan<symbol_resolved_trees::data::TypeParameter>, Diagnostic> {
            use language_semantics::Multiplicity;

            let mut parameters = program.machine_type_parameters(interface).to_vec();
            for member in members {
                let member_parameters = program.machine_type_parameters(member);
                if parameters.len() != member_parameters.len() {
                    return Err(Diagnostic::error(format!(
                        "machine `{}` has an incompatible target-family generic telescope",
                        member.name,
                    )));
                }
                for (required, actual) in parameters.iter_mut().zip(member_parameters) {
                    if actual.bounds.multiplicity != Multiplicity::Affine {
                        if required.bounds.multiplicity != Multiplicity::Affine
                            && required.bounds.multiplicity != actual.bounds.multiplicity
                        {
                            return Err(Diagnostic::error(format!(
                                "unimplemented: target-family generic multiplicity conjunction for `{}`",
                                interface.name,
                            )));
                        }
                        required.bounds.multiplicity = actual.bounds.multiplicity;
                    }
                    if let Some(carry) = actual.bounds.carry {
                        if required
                            .bounds
                            .carry
                            .is_some_and(|required| required != carry)
                        {
                            return Err(Diagnostic::error(format!(
                                "unimplemented: target-family generic carry conjunction for `{}`",
                                interface.name,
                            )));
                        }
                        required.bounds.carry = Some(carry);
                    }
                }
            }
            Ok(program
                .tables
                .declarations
                .data_type_parameters
                .insert_many(parameters))
        }

        fn telescope_correspondence(
            program: &SymbolResolvedTrees,
            interface: &Machine,
            member: &Machine,
        ) -> Result<Vec<(SymbolHandle, SymbolHandle)>, Diagnostic> {
            let interface_entry = program
                .machine_state_handles(interface.states)
                .first()
                .map(|state| program.machine_state(*state));
            let member_entry = program
                .machine_state_handles(member.states)
                .first()
                .map(|state| program.machine_state(*state));
            let (Some(interface_entry), Some(member_entry)) = (interface_entry, member_entry)
            else {
                return Err(Diagnostic::error(
                    "target family has no complete entry telescope",
                ));
            };
            let interface_parameters = program.state_parameters(interface_entry.parameters);
            let member_parameters = program.state_parameters(member_entry.parameters);
            let interface_generics = program.machine_type_parameters(interface);
            let member_generics = program.machine_type_parameters(member);
            if interface_parameters.len() != member_parameters.len()
                || interface_generics.len() != member_generics.len()
                || interface.conformance_bounds.len() != member.conformance_bounds.len()
            {
                return Err(Diagnostic::error(format!(
                    "machine `{}` has an incompatible target-family telescope",
                    member.name,
                )));
            }
            let mut symbols = member_parameters
                .iter()
                .zip(interface_parameters)
                .map(|(member, interface)| (member.symbol, interface.symbol))
                .chain(
                    member_generics
                        .iter()
                        .zip(interface_generics)
                        .map(|(member, interface)| (member.symbol, interface.symbol)),
                )
                .chain(
                    member
                        .conformance_bounds
                        .iter()
                        .zip(&interface.conformance_bounds)
                        .map(|(member, interface)| (member.binder, interface.binder)),
                )
                .collect::<Vec<_>>();
            symbols.push((member_entry.symbol, interface_entry.symbol));
            Ok(symbols)
        }

        fn copy_contract(
            program: &mut SymbolResolvedTrees,
            mut contract: SignatureContract,
            symbols: &[(SymbolHandle, SymbolHandle)],
        ) -> SignatureContract {
            let mut first = arena::Handle::invalid();
            for offset in 0..contract.facts.count() {
                let copied =
                    program.copy_proof_fact_with_symbols(contract.facts.handle_at(offset), symbols);
                if !first.is_valid() {
                    first = copied;
                }
            }
            contract.facts = HandleSpan::from_parts(first, contract.facts.count());
            contract
        }
    }

    pub(crate) use contracts::prepare_premises;

    pub(crate) fn bind(program: &mut SymbolResolvedTrees) -> Result<(), Vec<Diagnostic>> {
        let mut declarations = program
            .machines
            .iter()
            .enumerate()
            .filter(|(_, machine)| {
                machine.authored_target.is_some()
                    && program
                        .symbols
                        .source_file(machine.name.source_span())
                        .is_none_or(|source| source.dependency_scope == DependencyScope::Product)
            })
            .map(|(machine_index, _)| machine_index)
            .collect::<Vec<_>>();
        declarations.sort_by(|left, right| {
            program.machines[*left]
                .name
                .as_str()
                .cmp(program.machines[*right].name.as_str())
        });
        let mut remaining = declarations.as_slice();
        while let Some(&first) = remaining.first() {
            let family_length = remaining
                .iter()
                .take_while(|&&member| {
                    program.machines[member].name.as_str() == program.machines[first].name.as_str()
                })
                .count();
            let (family, rest) = remaining.split_at(family_length);
            for &member in family {
                let declaration = &program.machines[member];
                let members = family
                    .iter()
                    .map(|&candidate| &program.machines[candidate])
                    .filter(|candidate| {
                        program.symbols.symbol_module(candidate.symbol)
                            == program.symbols.symbol_module(declaration.symbol)
                            && candidate.attached_data_symbol == declaration.attached_data_symbol
                            && program.symbols.same_source_package(
                                candidate.name.source_span(),
                                declaration.name.source_span(),
                            )
                    });
                if members
                    .clone()
                    .filter(|candidate| {
                        candidate
                            .authored_target
                            .as_ref()
                            .map(|target| target.as_str())
                            == declaration
                                .authored_target
                                .as_ref()
                                .map(|target| target.as_str())
                    })
                    .count()
                    > 1
                {
                    return Err(vec![Diagnostic::error(format!(
                    "machine `{}` is implemented twice for `{}` -- a target supplies exactly one implementation of a contract machine",
                    declaration.name,
                    declaration.authored_target.as_ref().map_or("", |target| target.as_str()),
                )).with_source_span(declaration.name.source_span())]);
                }
                let mut interfaces = program.machines.iter().filter(|candidate| {
                    candidate.target_family_interface
                        && candidate.name.as_str() == declaration.name.as_str()
                        && program.symbols.symbol_module(candidate.symbol)
                            == program.symbols.symbol_module(declaration.symbol)
                        && candidate.attached_data_symbol == declaration.attached_data_symbol
                        && program.symbols.same_source_package(
                            candidate.name.source_span(),
                            declaration.name.source_span(),
                        )
                });
                let Some(interface) = interfaces.next() else {
                    return Err(vec![
                        Diagnostic::error(format!(
                            "target machine `{}` has no portable family interface",
                            declaration.name,
                        ))
                        .with_source_span(declaration.name.source_span()),
                    ]);
                };
                let canonical = interface.symbol;
                if interfaces.next().is_some() {
                    return Err(vec![
                        Diagnostic::error(format!(
                            "target machine `{}` has multiple portable family interfaces",
                            declaration.name,
                        ))
                        .with_source_span(declaration.name.source_span()),
                    ]);
                }
                if declaration.target_family.is_valid() && declaration.target_family != canonical {
                    return Err(vec![
                        Diagnostic::error("resolution changed a retained target-machine family")
                            .with_source_span(declaration.name.source_span()),
                    ]);
                }
                program.machines[member].target_family = canonical;
            }
            remaining = rest;
        }
        if let Some(machine) = program.machines.iter().find(|machine| {
            machine.target_family_interface
                && machine.target_family.is_valid()
                && machine.target_family != machine.symbol
        }) {
            return Err(vec![
                Diagnostic::error("resolution changed a retained target-machine interface")
                    .with_source_span(machine.name.source_span()),
            ]);
        }
        program.machines.for_each_mut(|machine| {
            if machine.target_family_interface {
                machine.target_family = machine.symbol;
            }
        });
        Ok(())
    }
}
pub(crate) mod target_siblings {
    /*
        Bind each target-authored body's calls to that target's declarations.
        The canonical member is a body too, not a portable caller. Ordinary
        resolution sees only canonical declarations: a sibling's symbol is spelled
        `<path>::<target>` and excluded from attachment lookups, so a
        `windows_x86_64` body that calls a
        `windows_x86_64`-only helper would otherwise stay unresolved, and one that
        calls a contract every target implements would bind to the selected
        target's body instead of its own target's.
    */

    use crate::selection::body_calls::{CallSite, retarget_machine_calls};
    use crate::symbols::lookup::child_symbol_by_kinds;
    use symbol_resolved_trees::SymbolResolvedTrees;
    use symbols::{SymbolHandle, SymbolKind, SymbolTable};

    struct Sibling {
        machine: SymbolHandle,
        family: SymbolHandle,
        target: String,
        owner: Option<String>,
    }

    /*
        Runs after every other symbol assignment and selection pass: later passes
        re-derive call targets from names, which cannot see a sibling.
    */
    pub(crate) fn bind_sibling_calls(
        program: &mut SymbolResolvedTrees,
        inputs: &[crate::selection::body_calls::CallSelectionInput],
        work: &[SymbolHandle],
    ) {
        let symbols = std::mem::take(&mut program.symbols);
        bind_sibling_calls_with(program, &symbols, inputs, work);
        program.symbols = symbols;
    }

    fn bind_sibling_calls_with(
        program: &mut SymbolResolvedTrees,
        symbols: &SymbolTable,
        inputs: &[crate::selection::body_calls::CallSelectionInput],
        work: &[SymbolHandle],
    ) {
        let machine_families = program
            .machines
            .iter()
            .filter(|machine| machine.target_family.is_valid())
            .map(|machine| (machine.symbol, machine.target_family))
            .collect::<Vec<_>>();
        let siblings = program
            .machines
            .iter()
            .filter_map(|machine| {
                let target = machine
                    .authored_target
                    .as_ref()
                    .or(machine.target.as_ref())?;
                Some(Sibling {
                    machine: machine.symbol,
                    family: machine.target_family,
                    target: target.as_str().to_owned(),
                    owner: machine
                        .attached_data
                        .as_ref()
                        .map(|owner| owner.as_str().to_owned()),
                })
            })
            .collect::<Vec<_>>();
        let bodies = program
            .machines
            .iter()
            .filter_map(|machine| {
                let body = machine.symbol;
                let mut owner = machine;
                loop {
                    if let Some(sibling) = siblings
                        .iter()
                        .find(|sibling| sibling.machine == owner.symbol)
                    {
                        return Some((body, sibling));
                    }
                    if !owner.lexical_parent.is_valid() {
                        return None;
                    }
                    owner = program.machines.get(owner.lexical_parent);
                }
            })
            .collect::<Vec<_>>();
        for (body, sibling) in bodies {
            let same_target = siblings
                .iter()
                .filter(|candidate| candidate.target == sibling.target)
                .collect::<Vec<_>>();
            let mut decide = |site: &CallSite<'_>| -> Option<SymbolHandle> {
                if site.target_symbol.is_valid() {
                    let target = symbols.get(site.target_symbol);
                    let callee = match target.kind {
                        SymbolKind::State => target.parent,
                        SymbolKind::Machine => site.target_symbol,
                        _ => return None,
                    };
                    if same_target
                        .iter()
                        .any(|candidate| candidate.machine == callee)
                    {
                        return None;
                    }
                    let (_, family) = machine_families
                        .iter()
                        .find(|(symbol, _)| *symbol == callee)?;
                    let replacement = same_target
                        .iter()
                        .find(|candidate| candidate.family == *family)?;
                    if target.kind == SymbolKind::Machine {
                        return Some(replacement.machine);
                    }
                    let state = child_symbol_by_kinds(
                        symbols,
                        replacement.machine,
                        &[SymbolKind::State],
                        symbols.name(site.target_symbol),
                    );
                    return state.is_valid().then_some(state);
                }
                let spelled = site.target.as_str();
                let leaf = spelled.rsplit_once("::").map_or(spelled, |(_, leaf)| leaf);
                let lookup = |path: &str| {
                    symbols.find_top_level_by_name_and_kinds_from_source(
                        &format!("{path}::{}", sibling.target),
                        &[SymbolKind::Machine],
                        site.target.source_span(),
                    )
                };
                let candidate = lookup(spelled).or_else(|| {
                    if !site.local_receiver {
                        return None;
                    }
                    let owner = sibling.owner.as_deref()?;
                    lookup(&format!("{owner}::{leaf}"))
                })?;
                if !same_target
                    .iter()
                    .any(|sibling| sibling.machine == candidate)
                {
                    return None;
                }
                let named = child_symbol_by_kinds(symbols, candidate, &[SymbolKind::State], leaf);
                let state = if named.is_valid() {
                    named
                } else {
                    child_symbol_by_kinds(symbols, candidate, &[SymbolKind::State], "entry")
                };
                state.is_valid().then_some(state)
            };
            retarget_machine_calls(program, body, inputs, work.contains(&body), &mut decide);
        }
    }
}
#[path = "symbols_targets.rs"]
mod targets;
#[path = "symbols_top_level.rs"]
mod top_level;
pub(crate) mod type_predicates {
    /*
        Give equality operands their selected type role before value preparation.
        A spelling never supplies this role: ordinary lexical selection has already
        distinguished data/type binders from constants, locals and value binders.
    */

    use crate::tables::body_expressions;
    use diagnostics::Diagnostic;
    use language_semantics::declaration_selection::AuthoredDeclarationSelectionOccurrenceId;
    use symbol_resolved_trees::data::TypeParameterKind;
    use symbol_resolved_trees::name::DiagnosticName;
    use symbol_resolved_trees::{
        SymbolResolvedTrees,
        expression::{BinaryOperator, ExpressionHandle, ExpressionNode},
        types::TypeReference,
    };
    use symbols::SymbolKind;

    pub(crate) fn retain_operands(
        program: &mut SymbolResolvedTrees,
        expressions: &[ExpressionHandle],
    ) -> Result<(), Vec<Diagnostic>> {
        for &expression in expressions {
            let ExpressionNode::Binary(binary) =
                program.tables.bodies.expressions.expression(expression)
            else {
                continue;
            };
            if !matches!(
                binary.operator,
                BinaryOperator::Equal | BinaryOperator::NotEqual
            ) {
                continue;
            }
            let (left, right) = (binary.left, binary.right);
            let operands = [left, right].map(|operand| has_type_role(program, operand, 0));
            match operands {
                [false, false] => continue,
                [true, true] => {
                    for operand in [left, right] {
                        if matches!(
                            program.tables.bodies.expressions.expression(operand),
                            ExpressionNode::TypeExpression(_)
                        ) {
                            continue;
                        }
                        let (reference, occurrences) = materialize_type_operand(program, operand);
                        let reference = program
                            .tables
                            .declarations
                            .child_type_references
                            .append(reference);
                        program
                            .tables
                            .bodies
                            .expressions
                            .attach_authored_selection_occurrences(operand, occurrences);
                        *program.tables.bodies.expressions.expression_mut(operand) =
                            ExpressionNode::TypeExpression(reference);
                    }
                }
                _ => {
                    return Err(vec![
                        Diagnostic::error(
                            "static type equality cannot mix a type operand with a runtime value",
                        )
                        .with_source_span(
                            program.tables.bodies.expressions.source_span(expression),
                        ),
                    ]);
                }
            }
        }
        Ok(())
    }

    pub(crate) fn is_type_predicate(
        program: &SymbolResolvedTrees,
        expression: ExpressionHandle,
    ) -> bool {
        matches!(program.tables.bodies.expressions.expression(expression), ExpressionNode::Binary(binary)
        if matches!(binary.operator, BinaryOperator::Equal | BinaryOperator::NotEqual)
        && has_type_role(program, binary.left, 0)
        && has_type_role(program, binary.right, 0))
    }

    fn named_type_operand(
        program: &SymbolResolvedTrees,
        expression: ExpressionHandle,
    ) -> Option<TypeReference> {
        match program.tables.bodies.expressions.expression(expression) {
            ExpressionNode::TypeExpression(reference) => {
                Some(program.child_type_reference(*reference).clone())
            }
            ExpressionNode::Name(path) if path.symbol.is_valid() => {
                let kind = program.symbols.get(path.symbol).kind;
                let type_binder = kind == SymbolKind::TypeParameter
                    && program.tables.declarations.data_type_parameters.iter().any(
                        |(_, parameter)| {
                            parameter.symbol == path.symbol
                                && matches!(parameter.kind, TypeParameterKind::Type)
                        },
                    );
                if !type_binder
                    && !matches!(
                        kind,
                        SymbolKind::BuiltinType | SymbolKind::Data | SymbolKind::Domain
                    )
                {
                    return None;
                }
                let name = body_expressions(program)
                    .name_path_members(path.members)
                    .iter()
                    .map(|member| member.as_str())
                    .collect::<Vec<_>>()
                    .join("::");
                Some(TypeReference::Named {
                    supplied_operand_owner: Default::default(),
                    symbol: path.symbol,
                    name: DiagnosticName::new(
                        name,
                        program.tables.bodies.expressions.source_span(expression),
                    ),
                })
            }
            _ => None,
        }
    }

    /*
        Value borrow/array syntax receives type role only when every nested name
        already selects a type. No spelling or compatible storage chooses the role.
    */
    fn has_type_role(
        program: &SymbolResolvedTrees,
        expression: ExpressionHandle,
        depth: usize,
    ) -> bool {
        if depth >= 128 || !expression.is_valid() {
            return false;
        }
        if named_type_operand(program, expression).is_some() {
            return true;
        }
        match program.tables.bodies.expressions.expression(expression) {
            ExpressionNode::Borrow(borrow) => has_type_role(program, borrow.target, depth + 1),
            ExpressionNode::ArrayLiteral(elements) => {
                match body_expressions(program).expression_handles(*elements) {
                    [element] => has_type_role(program, *element, depth + 1),
                    _ => false,
                }
            }

            _ => false,
        }
    }

    fn materialize_type_operand(
        program: &mut SymbolResolvedTrees,
        expression: ExpressionHandle,
    ) -> (TypeReference, Vec<AuthoredDeclarationSelectionOccurrenceId>) {
        use symbol_resolved_trees::types::{
            ReferenceTypeReference, ReferenceTypeReferenceStorage, SliceTypeReference,
            SliceTypeReferenceStorage,
        };
        let mut occurrences = body_expressions(program)
            .authored_selection_occurrences(expression)
            .collect::<Vec<_>>();
        if let Some(reference) = named_type_operand(program, expression) {
            return (reference, occurrences);
        }
        let reference = match body_expressions(program).expression(expression).clone() {
            ExpressionNode::Borrow(borrow) => {
                let (reference, child_occurrences) =
                    materialize_type_operand(program, borrow.target);
                occurrences.extend(child_occurrences);
                let referee = program
                    .tables
                    .declarations
                    .child_type_references
                    .append(reference);
                TypeReference::Reference(ReferenceTypeReference {
                    storage: ReferenceTypeReferenceStorage {
                        supplied_operand_owner: Default::default(),
                        referee,
                        access: borrow.access,
                        lifetime: None,
                    },
                })
            }
            ExpressionNode::ArrayLiteral(elements) => {
                let element = body_expressions(program).expression_handles(elements)[0];
                let (reference, child_occurrences) = materialize_type_operand(program, element);
                occurrences.extend(child_occurrences);
                let element_type = program
                    .tables
                    .declarations
                    .child_type_references
                    .append(reference);
                TypeReference::Slice(SliceTypeReference {
                    storage: SliceTypeReferenceStorage {
                        supplied_operand_owner: Default::default(),
                        element_type,
                    },
                })
            }

            _ => unreachable!("classified structural type operand"),
        };
        (reference, occurrences)
    }

    /*
        The source-role producer identifies type syntax before value-operator
        occurrences are issued. Bounds remain ordinary value expressions.
    */
    pub(crate) fn type_operand_nodes(
        program: &SymbolResolvedTrees,
        expression: ExpressionHandle,
        output: &mut Vec<ExpressionHandle>,
    ) {
        if output.contains(&expression) || !has_type_role(program, expression, 0) {
            return;
        }
        output.push(expression);
        match program.tables.bodies.expressions.expression(expression) {
            ExpressionNode::Borrow(borrow) => type_operand_nodes(program, borrow.target, output),
            ExpressionNode::ArrayLiteral(elements) => {
                for &element in body_expressions(program).expression_handles(*elements) {
                    type_operand_nodes(program, element, output);
                }
            }

            _ => {}
        }
    }
}
#[path = "symbols_type_references.rs"]
mod type_references;

use contracts::assign_contract_reference_symbols;
use domain_facts::assign_domain_fact_symbols;
use statements::{assign_machine_signature_reference_symbols, assign_statement_reference_symbols};
use top_level::assign_top_level_symbols;

pub(crate) use targets::normalize_static_module_calls;

#[derive(Default)]
pub(crate) struct NamespaceDeclarations {
    text: syntax_trees::SyntaxTextOwner,
    modules: Vec<Vec<syntax_trees::identifier::Identifier>>,
    imports: Vec<Vec<syntax_trees::identifier::Identifier>>,
}

impl syntax_trees::SyntaxTextContext for NamespaceDeclarations {
    fn source_text(&self, source: arena::Handle<syntax_trees::RetainedSource>) -> &str {
        self.text.source_text(source)
    }
}

impl NamespaceDeclarations {
    pub(crate) fn module_paths(&self) -> &[Vec<syntax_trees::identifier::Identifier>] {
        &self.modules
    }

    pub(crate) fn import_paths(&self) -> &[Vec<syntax_trees::identifier::Identifier>] {
        &self.imports
    }

    pub(crate) fn record_module(
        &mut self,
        syntax: &syntax_trees::SyntaxTrees,
        path: arena::HandleSpan<syntax_trees::identifier::Identifier>,
    ) {
        self.text.retain_from(&syntax.text);
        self.modules
            .push(syntax.items.identifier_path_members(path).to_vec());
    }

    pub(crate) fn record_import(
        &mut self,
        syntax: &syntax_trees::SyntaxTrees,
        path: arena::HandleSpan<syntax_trees::identifier::Identifier>,
    ) {
        self.text.retain_from(&syntax.text);
        self.imports
            .push(syntax.items.identifier_path_members(path).to_vec());
    }

    fn record_import_selections(
        &self,
        program: &mut SymbolResolvedTrees,
    ) -> Result<(), Vec<Diagnostic>> {
        use language_semantics::declaration_selection::{
            AuthoredDeclarationSelectionExposure, AuthoredDeclarationSelectionKind,
        };
        for path in &self.imports {
            let (Some(first), Some(last)) = (path.first(), path.last()) else {
                continue;
            };
            let name = path.joined_path_in(&self.text);
            let targets = program
                .symbols
                .source_module_import_targets(first.source_span().source_id, &name);
            let span = source::SourceSpan::new(
                first.source_span().source_id,
                source::Span::new(first.source_span().span.start, last.source_span().span.end),
            );
            for target in targets {
                program
                    .record_resolved_authored_declaration_selection(
                        span,
                        AuthoredDeclarationSelectionExposure::PrivateImplementation,
                        AuthoredDeclarationSelectionKind::StaticPathSegment,
                        target,
                    )
                    .map_err(|error| {
                        vec![
                            Diagnostic::error(format!(
                                "failed to retain module import selection: {error:?}"
                            ))
                            .with_source_span(span),
                        ]
                    })?;
            }
        }
        Ok(())
    }

    /*
        Register namespace custody before all nonconstant headers are available.
        Import-target validation still runs when the complete table is installed.
    */
    pub(crate) fn register(&self, symbols: &mut SymbolTable) -> Result<(), Vec<Diagnostic>> {
        for path in &self.modules {
            let Some(first) = path.first() else {
                continue;
            };
            symbols
                .register_source_module(
                    first.source_span().source_id,
                    path.iter()
                        .map(|member| (member.as_str(&self.text), member.source_span())),
                )
                .map_err(|message| {
                    vec![Diagnostic::error(message).with_source_span(first.source_span())]
                })?;
        }
        for path in &self.imports {
            let Some(first) = path.first() else {
                continue;
            };
            let name = path.joined_path_in(&self.text);
            symbols.register_source_import(first.source_span().source_id, &name);
        }
        Ok(())
    }

    fn install(&self, symbols: &mut SymbolTable) -> Result<(), Vec<Diagnostic>> {
        self.register(symbols)?;
        for path in &self.imports {
            let Some(first) = path.first() else {
                continue;
            };
            let name = path.joined_path_in(&self.text);
            symbols
                .validate_source_module_import(first.source_span().source_id, &name)
                .map_err(|message| {
                    vec![Diagnostic::error(message).with_source_span(first.source_span())]
                })?;
        }
        Ok(())
    }
}

/*
    The full symbol-assignment orchestration over a ready table: top-level
    identity first, then generic origins, then the typed reference,
    proposition, mathematical, measure, contract, domain-fact and signature
    passes in dependency order. Both the initial and extension drivers run
    the same sequence.
*/
fn assign_program_symbols(
    program: &mut SymbolResolvedTrees,
    symbols: &SymbolTable,
    generic_origin_frontier: usize,
    runtime_callable_scopes: &mut crate::runtime_callable_scopes::RuntimeCallableScopes,
    runtime_callable_contracts: &[symbol_resolved_trees::callable_values::RuntimeCallableContractHandle],
) -> Vec<Diagnostic> {
    let mut diagnostics = assign_top_level_symbols(program, symbols);
    if let Err(error) = runtime_callable_scopes.bind_program(program) {
        diagnostics.push(error);
        return diagnostics;
    }
    if let Err(errors) =
        runtime_callable_types::assign_signatures(program, symbols, runtime_callable_contracts)
    {
        diagnostics.extend(errors);
        return diagnostics;
    }
    assign_generic_application_origin_symbols(program, symbols, generic_origin_frontier);
    assign_type_reference_symbols(program, symbols);
    type_references::assign_declaration_type_value_expression_symbols(program, symbols);
    propositions::assign_proposition_expression_symbols(program, symbols);
    mathematical::assign_mathematical_expression_symbols(program, symbols);
    measures::assign_measure_expression_symbols(program, symbols);
    if let Err(errors) = assign_contract_reference_symbols(program, symbols) {
        diagnostics.extend(errors);
    }
    assign_domain_fact_symbols(program, symbols);
    if let Err(errors) = assign_machine_signature_reference_symbols(program, symbols) {
        diagnostics.extend(errors);
    }
    diagnostics
}

pub(crate) fn assign_symbols(
    program: &mut SymbolResolvedTrees,
    sources: Option<Arc<SourceMap>>,
    source_scoped_top_level_bindings: Vec<symbols::SourceScopedTopLevelBinding>,
    const_declarations: &[crate::resolution::lowerer::PendingConstDeclaration],
    namespace_declarations: &NamespaceDeclarations,
    generic_origin_frontier: usize,
    operand_bindings: &[(
        symbol_resolved_trees::types::OperandLifetimeOwnerHandle,
        crate::resolution::operand_owners::DeclarationInput,
    )],
    runtime_callable_scopes: &mut crate::runtime_callable_scopes::RuntimeCallableScopes,
) -> Result<(), Vec<Diagnostic>> {
    let mut symbols = build_symbol_table(
        program,
        sources.clone(),
        source_scoped_top_level_bindings,
        const_declarations,
        !namespace_declarations.modules.is_empty(),
    );
    namespace_declarations.install(&mut symbols)?;
    let has_sources = sources.is_some() || !namespace_declarations.modules.is_empty();
    let mut symbols = insert_selected_machine_children(program, symbols, sources, 0, has_sources);
    scope::record_declaration_owners(program, &mut symbols)?;
    program.symbols = symbols;
    let runtime_callable_contracts = symbol_table::append_runtime_callable_symbols(program);
    crate::resolution::operand_owners::bind_program(program, operand_bindings)?;
    let symbols = program.symbols.clone();
    let diagnostics = assign_program_symbols(
        program,
        &symbols,
        generic_origin_frontier,
        runtime_callable_scopes,
        &runtime_callable_contracts,
    );
    namespace_declarations.record_import_selections(program)?;
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

pub(crate) fn assign_symbols_against_resolved_base(
    program: &mut SymbolResolvedTrees,
    sources: Arc<SourceMap>,
    source_scoped_top_level_bindings: Vec<symbols::SourceScopedTopLevelBinding>,
    roots: crate::resolution::lowerer::RootWatermarks,
    const_declarations: &[crate::resolution::lowerer::PendingConstDeclaration],
    namespace_declarations: &NamespaceDeclarations,
    generic_origin_frontier: usize,
    operand_bindings: &[(
        symbol_resolved_trees::types::OperandLifetimeOwnerHandle,
        crate::resolution::operand_owners::DeclarationInput,
    )],
    runtime_callable_scopes: &mut crate::runtime_callable_scopes::RuntimeCallableScopes,
) -> Result<(), Vec<Diagnostic>> {
    symbol_table::extend_symbol_table(
        program,
        sources.clone(),
        source_scoped_top_level_bindings,
        roots,
        const_declarations,
    );
    namespace_declarations.install(&mut program.symbols)?;
    let table = std::mem::take(&mut program.symbols);
    program.symbols =
        insert_selected_machine_children(program, table, Some(sources), roots.machines, true);
    let mut symbols = program.symbols.clone();
    scope::record_declaration_owners(program, &mut symbols)?;
    program.symbols = symbols;
    let runtime_callable_contracts = symbol_table::append_runtime_callable_symbols(program);
    crate::resolution::operand_owners::bind_program(program, operand_bindings)?;
    let symbols = program.symbols.clone();
    let diagnostics = assign_program_symbols(
        program,
        &symbols,
        generic_origin_frontier,
        runtime_callable_scopes,
        &runtime_callable_contracts,
    );
    namespace_declarations.record_import_selections(program)?;
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

/*
    Constant initializers have declaration-source lookup but no caller locals or
    receiver. Reuse the ordinary expression resolver before copying a selected
    initializer into any body; use-site namespaces never resolve its constructors.
*/
pub(crate) fn assign_constant_expression_symbols(
    program: &mut SymbolResolvedTrees,
    initializers: impl IntoIterator<Item = symbol_resolved_trees::expression::ExpressionHandle>,
) {
    let attached_machines = scope::attached_machines(program);
    let declarations = &mut program.tables.declarations;
    let mut scope = scope::MachineScope {
        attached_machines: &attached_machines,
        resolution_indexes: &program.resolution_indexes,
        symbol: symbols::SymbolHandle::invalid(),
        lexical_requirements: scope::LexicalRequirements::none(),
        type_parameters: &[],
        attached_data_symbol: symbols::SymbolHandle::invalid(),
        inherited_data_members: None,
        owned_data: &[],
        prior_statements: &[],
        data_definitions: &program.roots.data_definitions,
        type_aliases: &program.roots.type_aliases,
        conformances: &program.roots.conformances,
        traits: &program.roots.traits,
        operand_owners: &program.tables.types.operand_lifetime_owners,
        data_members: &declarations.data_members,
        data_type_parameters: &declarations.data_type_parameters,
        data_payload_fields: &declarations.data_payload_fields,
        type_constraints: &program.tables.types.constraints,
    };
    for initializer in initializers {
        scope.symbol = program
            .roots
            .const_declarations
            .iter()
            .find(|declaration| {
                declaration.initializer == initializer
                    || declaration.authored_initializer == initializer
            })
            .map_or_else(symbols::SymbolHandle::invalid, |declaration| {
                declaration.symbol
            });
        expressions::assign_expression_table_symbols(
            &program.symbols,
            &scope,
            &[],
            symbols::SymbolHandle::invalid(),
            &mut program.tables.bodies.expressions,
            &mut declarations.child_type_references,
            initializer,
        );
    }
}

pub(crate) use expressions::reject_unqualified_case_values;
pub(crate) use lookup::{
    MembershipSelection, builtin_type_atom_by_symbol_name, constructor_prefix_is_value,
    constructor_type, domain_name_reaches, membership_selection, prefer_module_local_domain,
    qualified_payload_free_case_type,
};

/*
    Build the symbol table and give every reference its identity, then settle
    the static module calls and outcome-specific contracts that need it. A
    seeded lowerer extends its base's table in place.
*/
pub(crate) fn assign(lowerer: &mut Lowerer) -> Result<(), Vec<Diagnostic>> {
    assign_declarations(lowerer)?;
    let machines = lowerer
        .symbol_resolved_trees
        .machines
        .iter()
        .map(|machine| machine.symbol)
        .collect::<Vec<_>>();
    assign_body_symbols(lowerer, &machines)
}

/*
    Allocate and resolve declaration identities before any body-local symbol
    batch. Both eager resolution and retained-body completion use this boundary.
*/
pub(crate) fn assign_declarations(lowerer: &mut Lowerer) -> Result<(), Vec<Diagnostic>> {
    let operand_bindings = crate::resolution::operand_owners::bindings(lowerer)?;
    let seeded_roots = lowerer.seed.as_ref().map(|seed| seed.roots);
    match seeded_roots {
        None => assign_symbols(
            &mut lowerer.symbol_resolved_trees,
            lowerer.sources.take(),
            std::mem::take(&mut lowerer.source_scoped_top_level_bindings),
            &lowerer.pending_const_declarations,
            &lowerer.namespace_declarations,
            lowerer.bound_generic_application_origins,
            &operand_bindings,
            &mut lowerer.runtime_callable_scopes,
        )?,
        Some(roots) => {
            let sources = lowerer.sources.take().ok_or_else(|| {
                vec![Diagnostic::error(
                    "seeded symbol resolution requires retained source custody",
                )]
            })?;
            assign_symbols_against_resolved_base(
                &mut lowerer.symbol_resolved_trees,
                sources,
                std::mem::take(&mut lowerer.source_scoped_top_level_bindings),
                roots,
                &lowerer.pending_const_declarations,
                &lowerer.namespace_declarations,
                lowerer.bound_generic_application_origins,
                &operand_bindings,
                &mut lowerer.runtime_callable_scopes,
            )?;
        }
    }
    lowerer.bound_generic_application_origins = lowerer
        .symbol_resolved_trees
        .tables
        .types
        .generic_application_origins
        .len();
    normalize_static_module_calls(
        &mut lowerer.symbol_resolved_trees,
        &lowerer.pending_static_module_calls,
        &lowerer.pending_static_module_statement_calls,
    );
    contracts::finalize_outcome_specific_contract_symbols(
        &mut lowerer.symbol_resolved_trees,
        &lowerer.pending_outcome_specific_contracts,
    )
    .map_err(|diagnostic| vec![diagnostic])?;
    Ok(())
}

/*
    Complete selected bodies against their existing declaration symbols. New
    locals and synthetic state signatures append; retained bodies and headers
    never have their symbols reallocated.
*/
pub(crate) fn assign_body_symbols(
    lowerer: &mut Lowerer,
    selected_machines: &[symbols::SymbolHandle],
) -> Result<(), Vec<Diagnostic>> {
    symbol_table::append_body_symbols(&mut lowerer.symbol_resolved_trees, selected_machines)?;
    let runtime_callable_contracts =
        symbol_table::append_runtime_callable_symbols(&mut lowerer.symbol_resolved_trees);
    lowerer
        .runtime_callable_scopes
        .bind_program(&mut lowerer.symbol_resolved_trees)
        .map_err(|error| vec![error])?;
    let symbols = lowerer.symbol_resolved_trees.symbols.clone();
    runtime_callable_types::assign_signatures(
        &mut lowerer.symbol_resolved_trees,
        &symbols,
        &runtime_callable_contracts,
    )?;
    assign_generic_application_origin_symbols(
        &mut lowerer.symbol_resolved_trees,
        &symbols,
        lowerer.bound_generic_application_origins,
    );
    lowerer.bound_generic_application_origins = lowerer
        .symbol_resolved_trees
        .tables
        .types
        .generic_application_origins
        .len();
    assign_statement_reference_symbols(
        &mut lowerer.symbol_resolved_trees,
        &symbols,
        selected_machines,
    )?;
    assign_contract_reference_symbols(&mut lowerer.symbol_resolved_trees, &symbols)?;
    normalize_static_module_calls(
        &mut lowerer.symbol_resolved_trees,
        &lowerer.pending_static_module_calls,
        &lowerer.pending_static_module_statement_calls,
    );
    contracts::finalize_outcome_specific_contract_symbols(
        &mut lowerer.symbol_resolved_trees,
        &lowerer.pending_outcome_specific_contracts,
    )
    .map_err(|diagnostic| vec![diagnostic])?;
    private_nested_calls::reject_private_nested_calls(&lowerer.symbol_resolved_trees)?;
    repeated_locals::reject_repeated_local_names(&lowerer.symbol_resolved_trees)
}

pub(crate) fn assign_lexical_inputs(
    lowerer: &mut Lowerer,
    syntax: &syntax_trees::SyntaxTrees,
    demand: crate::lowering::state::LexicalInputDemand,
) -> Result<Vec<crate::selection::body_calls::CallSelectionInput>, Vec<Diagnostic>> {
    let mut inputs =
        crate::lowering::machine::lower_pending_lexical_inputs(lowerer, syntax, demand)
            .map_err(|diagnostic| vec![diagnostic])?;
    for input in &mut inputs {
        let state = lowerer
            .symbol_resolved_trees
            .machine_state(input.state)
            .symbol;
        symbol_table::append_local_symbols(
            &mut lowerer.symbol_resolved_trees.symbols,
            state,
            &mut input.locals,
        );
    }
    let runtime_callable_contracts =
        symbol_table::append_runtime_callable_symbols(&mut lowerer.symbol_resolved_trees);
    lowerer
        .runtime_callable_scopes
        .bind_program(&mut lowerer.symbol_resolved_trees)
        .map_err(|error| vec![error])?;
    let symbols = lowerer.symbol_resolved_trees.symbols.clone();
    runtime_callable_types::assign_signatures(
        &mut lowerer.symbol_resolved_trees,
        &symbols,
        &runtime_callable_contracts,
    )?;
    statements::assign_lexical_input_reference_symbols(
        &mut lowerer.symbol_resolved_trees,
        &symbols,
        &mut inputs,
    )?;
    normalize_static_module_calls(
        &mut lowerer.symbol_resolved_trees,
        &lowerer.pending_static_module_calls,
        &lowerer.pending_static_module_statement_calls,
    );
    let mut calls = Vec::new();
    for input in &mut inputs {
        for (_, expression) in input.calls.drain(..) {
            calls.push(crate::selection::body_calls::CallSelectionInput {
                machine: input.machine,
                expression,
            });
        }
    }
    lowerer.resolved_lexical_inputs.extend(inputs);
    Ok(calls)
}

use crate::resolution::lowerer::Lowerer;
use diagnostics::Diagnostic;
use symbol_table::insert_selected_machine_children;
use symbols::SymbolTable;
use type_references::assign_generic_application_origin_symbols;
pub(crate) use type_references::assign_nominal_machine_requirement_arguments;
