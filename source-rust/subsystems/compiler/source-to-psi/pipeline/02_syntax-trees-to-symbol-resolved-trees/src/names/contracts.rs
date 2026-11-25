use crate::source_resolved::SymbolResolvedTrees;
use crate::source_resolved::signature::SignatureContractKind;
use crate::source_symbols::{JoinedPath, SymbolTable};
use diagnostics::Diagnostic;
use symbols::SymbolHandle;

use super::expression_paths::resolve_expression_table_call_target_symbol;
use super::expressions::{
    assign_member_symbol, assign_membership_symbol, assign_name_symbol,
    assign_struct_literal_symbols,
};
use super::scopes::MachineScope;

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
    let conformance_scopes = super::scopes::DeclarationTypeScopes::of(program)?;
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
    let attached_machines = super::scopes::attached_machines(program);
    let binder_roles = super::scopes::BinderRoles::of(program, symbols);
    let trait_scopes = super::scopes::TraitLexicalScopes::of(program);
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
    let domain_paths = &tables.declarations.domain_path_members;
    let expression_table = &mut tables.bodies.expressions;
    let child_type_references = &mut tables.declarations.child_type_references;
    let declared_type_equations = &mut tables.declarations.declared_type_equations;
    let scope_context = super::scopes::ContractScopeContext {
        attached_machines: &attached_machines,
        binder_roles: &binder_roles,
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
        let invocation_scope = super::scopes::declaration_type_scope(
            data_type_parameters.span_or_empty(machine.type_parameters),
            conformance_scopes.parameters(machine.symbol),
        );
        let lexical_scope = super::scopes::declaration_type_scope(
            &invocation_scope,
            data_definition.map_or(&[], |definition| {
                data_type_parameters.span_or_empty(definition.type_parameters)
            }),
        );
        let scope = scope_context.scope(
            machine.symbol,
            trait_scopes.machine_requirements(machine.symbol),
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
        // Retained type equations use this same entry scope as the authored
        // requirement. Constructor arguments take their selected binder role:
        // Buffer<Element, N> reads Element as a type and N as a value.
        for equation in declared_type_equations.span_mut_or_empty(machine.declared_type_equations) {
            for reference in [&mut equation.parameter, &mut equation.definition] {
                super::type_references::assign_type_value_expression_symbols(
                    symbols,
                    &scope,
                    entry_parameters,
                    entry_symbol,
                    expression_table,
                    child_type_references,
                    reference,
                );
            }
        }
        assign_callable_value_symbols(
            symbols,
            &scope,
            &[],
            entry_parameters,
            entry.and_then(|state| state.return_type.as_ref()),
            entry_symbol,
            machine.contracts,
            signature_contracts,
            proof_facts,
            domain_paths,
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
                domain_paths,
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
                domain_paths,
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
            super::scopes::LexicalRequirements::none(),
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
            domain_paths,
            expression_table,
            child_type_references,
        );
    }

    for (_, parameter) in data_type_parameters.iter() {
        let Some(signature) = parameter.kind.contract_signature() else {
            continue;
        };
        let scope = scope_context.scope(
            signature.symbol,
            super::scopes::LexicalRequirements::none(),
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
            domain_paths,
            expression_table,
            child_type_references,
        );
    }
    for (handle, parameters) in &runtime_scopes {
        let declaration = tables.declarations.runtime_callable_contracts.get(*handle);
        let signature = &declaration.signature;
        let scope = scope_context.scope(
            signature.symbol,
            super::scopes::LexicalRequirements::none(),
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
            domain_paths,
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
    signature_type_parameters: &[crate::source_resolved::data::TypeParameter],
    parameters: &[crate::source_resolved::signature::StateParameter],
    return_type: Option<&crate::source_resolved::types::TypeReference>,
    state_symbol: SymbolHandle,
    contracts: arena::HandleSpan<crate::source_resolved::signature::SignatureContract>,
    signature_contracts: &arena::Arena<crate::source_resolved::signature::SignatureContract>,
    proof_facts: &arena::Arena<crate::source_resolved::domain::ProofFact>,
    domain_paths: &arena::Arena<DiagnosticName>,
    expression_table: &mut crate::source_resolved::expression::ExpressionTable,
    child_type_references: &mut crate::source_resolved::types::TypeReferenceArena,
) {
    let type_parameters =
        super::scopes::declaration_type_scope(signature_type_parameters, machine.type_parameters);
    let contract_scope = &MachineScope {
        type_parameters: &type_parameters,
        contract_result_data: return_type
            .and_then(|reference| {
                super::expression_paths::projected_receivers::declared_nominal(
                    reference,
                    child_type_references,
                )
            })
            .unwrap_or_default(),
        ..*machine
    };
    for reference in parameters
        .iter()
        .map(|parameter| &parameter.type_reference)
        .chain(return_type)
    {
        let mut reference = reference.clone();
        crate::resolution::names::type_references::assign_type_value_expression_symbols(
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
                crate::source_resolved::domain::ProofFact::Expression(expression) => *expression,
                crate::source_resolved::domain::ProofFact::Membership(membership) => {
                    assign_contract_domain_arguments(
                        symbols,
                        contract_scope,
                        parameters,
                        domain_paths.span_or_empty(membership.domain),
                        membership.domain_arguments,
                        child_type_references,
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

/// A contract uses the declaration's slot kind to select a const binder or
/// a value after resolving its ordinary type arguments in the callable scope.
fn assign_contract_domain_arguments(
    symbols: &SymbolTable,
    machine: &MachineScope<'_>,
    parameters: &[crate::source_resolved::signature::StateParameter],
    path: &[DiagnosticName],
    arguments: arena::HandleSpan<crate::source_resolved::types::TypeReference>,
    child_types: &mut crate::source_resolved::types::TypeReferenceArena,
) {
    if arguments.is_empty() {
        return;
    }
    super::type_references::assign_type_reference_argument_symbols_with_constraints(
        symbols,
        child_types,
        machine.type_constraints,
        machine.type_parameters,
        arguments,
    );
    if let Some(last) = path.last() {
        super::type_references::assign_domain_index_values(
            symbols,
            machine,
            parameters,
            &DiagnosticName::from_str(&path.joined_path(), last.source_span()),
            arguments,
            child_types,
            &[],
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn assign_contract_call_symbols(
    symbols: &SymbolTable,
    machine: &MachineScope<'_>,
    parameters: &[crate::source_resolved::signature::StateParameter],
    state_symbol: SymbolHandle,
    expression_table: &mut crate::source_resolved::expression::ExpressionTable,
    child_type_references: &mut crate::source_resolved::types::TypeReferenceArena,
    expression: crate::source_resolved::expression::ExpressionHandle,
) {
    use crate::source_resolved::expression::ExpressionNode;

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
        // Typing refuses a block value in a contract.
        ExpressionNode::Block(_) => {}
        ExpressionNode::Match(dispatch) => {
            assign_calls!(dispatch.subject);
            for arm in expression_table.match_arms(dispatch.arms).to_vec() {
                for pattern in expression_table
                    .match_pattern_dependencies(arm.pattern)
                    .collect::<Vec<_>>()
                {
                    assign_calls!(pattern);
                }
                if let crate::source_resolved::expression::MatchPattern::Destructure(pattern) =
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
            crate::resolution::names::type_references::reassign_type_reference_symbols_and_values(
                symbols,
                machine,
                parameters,
                state_symbol,
                expression_table,
                child_type_references,
                cast.target_type,
            );
            assign_contract_domain_arguments(
                symbols,
                machine,
                parameters,
                expression_table.name_path_members(cast.semantic_domain),
                cast.semantic_domain_arguments,
                child_type_references,
            );
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
            // A contract call takes the same static qualification as a
            // body call, so a fixed `core::` name such as
            // `core::Identity::<T>(left, right)` and a module-qualified
            // machine select their declarations here too.
            let target_symbol = super::targets::normalize_expression_module_call(
                symbols,
                expression_table,
                expression,
            )
            .unwrap_or_else(|| {
                resolve_expression_table_call_target_symbol(
                    machine,
                    parameters,
                    state_symbol,
                    &call,
                    expression_table,
                    child_type_references,
                    symbols,
                )
            });
            crate::resolution::names::expressions::assign_static_arguments_type_symbols(
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
                for (position, argument) in call.machine_arguments.iter_mut().enumerate() {
                    super::targets::assign_call_static_argument_symbol(
                        symbols,
                        machine,
                        state_symbol,
                        target_symbol,
                        position,
                        argument,
                    );
                }
            }
            super::expressions::retain_unresolved_call(
                symbols,
                machine,
                parameters,
                expression_table,
                &call,
                target_symbol,
                expression,
            );
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
            assign_contract_domain_arguments(
                symbols,
                machine,
                parameters,
                expression_table.name_path_members(membership.domain),
                membership.domain_arguments,
                child_type_references,
            );
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
            crate::resolution::names::type_references::reassign_type_reference_symbols_and_values(
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
                if let ExpressionNode::Name(resolved) = expression_table.expression_mut(expression)
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
    program: &mut crate::source_resolved::SymbolResolvedTrees,
    pending: &[crate::resolution::lowering::lowerer::PendingOutcomeSpecificContract],
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
                crate::source_resolved::data::DataMember::Variant(variant)
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
