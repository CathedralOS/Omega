//! Named public calls retain the same original application as boundary tokens.
//!
//! Inferred destinations cannot use a declaration-local result binder. Retain
//! the original occurrence and tuple, then apply the complete public contract in that
//! caller's scope. Only closed tuples enter the public signature producer. Runtime
//! operands propose only declaration-backed substitutions; implementation
//! bodies, guarantees and provider selection do not participate.

use diagnostics::Diagnostic;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::statement::StatementNode;
use typed_trees::static_applications::StaticApplicationDemand;

use super::CallSite;

pub(super) fn bind(program: &mut TypedTrees) -> Result<Vec<SymbolHandle>, Vec<Diagnostic>> {
    let mut sites = Vec::new();
    let mut expressions = Vec::new();
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            for (statement_handle, statement) in program
                .statement_table
                .iter_statements(state.statement_nodes)
            {
                if matches!(statement, StatementNode::Call(_)) {
                    sites.push((
                        machine.symbol,
                        state.symbol,
                        CallSite::Statement(statement_handle),
                    ));
                }
                expressions.clear();
                validation::collect_statement_expression_trees(
                    program,
                    statement,
                    &mut expressions,
                );
                for expression in expressions.iter().copied() {
                    if program
                        .expression_table
                        .source_call_node(expression)
                        .is_some()
                    {
                        let site = (
                            machine.symbol,
                            state.symbol,
                            CallSite::Expression(expression),
                        );
                        if !sites.contains(&site) {
                            sites.push(site);
                        }
                    }
                }
            }
        }
    }
    let mut applications = Vec::new();
    let mut results = Vec::new();
    let mut changed = Vec::new();
    for (owner, state_symbol, site) in sites {
        let (target, selected, arguments) = match site {
            CallSite::Expression(expression) => {
                let Some(call) = program.expression_table.source_call_node(expression) else {
                    continue;
                };
                (
                    call.target_symbol,
                    call.machine_arguments.to_vec(),
                    program
                        .expression_table
                        .expression_handles(call.arguments)
                        .to_vec(),
                )
            }
            CallSite::Statement(statement) => {
                let Some(call) = program.statement_table.statement(statement).as_call() else {
                    continue;
                };
                (
                    call.target_symbol,
                    call.machine_arguments.to_vec(),
                    program
                        .statement_table
                        .expression_handles(call.arguments)
                        .to_vec(),
                )
            }
        };
        // The target may be a source-declared finite signature. Its original
        // caller application remains the only complete receiving telescope.
        let Some(public) = program.public_trait_requirement_signature(target) else {
            continue;
        };
        let declaration = public.declaration.symbol;
        if !public.owner.is_boundary {
            continue;
        }
        let original = super::applications::retain_authored_call(program, site, owner);
        let header = program
            .instantiated_requirement_signatures
            .iter()
            .find(|(_, row)| program.trait_machine_signatures.get(row.signature).symbol == target);
        if header.is_some()
            && program
                .selected_trait_requirement_signature(target, original)
                .is_some()
        {
            continue;
        }
        if program.closed_requirement_application(target).is_some()
            && !program.closed_requirement_application_matches_call(target, original)
        {
            return Err(vec![Diagnostic::error(
                "public requirement call lost its exact closed source application",
            )]);
        }
        let application = program
            .expression_table
            .static_machine_application(original);
        if application.registration_operation != declaration
            || (application.binding_declaration.is_valid()
                && application.binding_declaration != declaration)
        {
            return Err(vec![Diagnostic::error(
                "public requirement call lost its original declaration application",
            )]);
        }
        // A freshly captured call owns operands before binding derivation.
        // Its resolved registration operation is the original declaration;
        // generated target/header identities never supply this coordinate.
        program
            .expression_table
            .static_machine_application_mut(original)
            .binding_declaration = declaration;
        let application = program
            .expression_table
            .static_machine_application(original);
        let schema =
            typed_trees::static_applications::StaticApplicationDeclaration::for_application(
                program,
                application,
            )
            .map_err(|message| vec![Diagnostic::error(message)])?;
        if schema.parameters().is_empty() && schema.conformance_bounds().is_empty() {
            continue;
        }
        let Some(machine) = program.machine_by_symbol(owner) else {
            continue;
        };
        let Some(state) = program
            .machine_states(machine)
            .iter()
            .find(|state| state.symbol == state_symbol)
        else {
            continue;
        };
        let operand_types = arguments
            .iter()
            .map(|argument| {
                validation::expression_result_type_reference(program, machine, state, *argument)
            })
            .collect::<Vec<_>>();
        let previous_result = program
            .expression_table
            .static_machine_application(original)
            .public_result_type;
        let application = super::requirement_signatures::retain_requirement_application(
            program,
            declaration,
            owner,
            original,
            &selected,
            &operand_types,
        )
        .map_err(|diagnostic| vec![diagnostic])?;
        if application.is_valid() {
            let prepared = super::requirement_signatures::prepare_public_contract(
                program,
                declaration,
                application,
            )
            .map_err(|diagnostic| vec![diagnostic])?;
            results.push((owner, application, previous_result, prepared));
            // A bound caller tuple can expose a result while remaining open.
            // Only closed applications own generated public headers and their
            // provider candidates; ordinary caller specialization revisits the
            // original occurrence with its completed tuple.
            if super::application_arguments::static_application_is_closed(program, application) {
                applications.push((owner, site, declaration, application));
            }
        }
    }
    if applications.is_empty() && results.is_empty() {
        return Ok(changed);
    }
    // One frozen original cohort issues every application in this pass. Header
    // appends cannot alter another call's original tuple or captured scope.
    let source = std::sync::Arc::new(typed_trees::FrozenTypedTrees::from(program.clone()));
    for (owner, application, previous, prepared) in results {
        let result = super::requirement_signatures::copy_public_contract(
            &source,
            program,
            application,
            &prepared,
        )
        .map_err(|diagnostic| vec![diagnostic])?;
        if result.is_valid()
            && (!previous.is_valid()
                || program.package_qualified_type_identity(previous)
                    != program.package_qualified_type_identity(result))
        {
            changed.push(owner);
        }
    }
    for (owner, site, declaration, application) in applications {
        let demand = StaticApplicationDemand {
            declaration,
            owner: source.clone(),
            application,
        };
        super::demanded_applications::instantiate_one(program, &demand)?;
        let instance = program
            .instantiated_static_applications()
            .find(|(_, row)| {
                std::sync::Arc::ptr_eq(&row.demand.owner, &source)
                    && row.demand.application == application
            })
            .map(|(_, row)| row.instance)
            .ok_or_else(|| {
                vec![Diagnostic::error(
                    "public requirement call lost its original application header",
                )]
            })?;
        let (_, signature) = program.trait_machine_signature(instance).ok_or_else(|| {
            vec![Diagnostic::error(
                "public requirement call lost its produced signature",
            )]
        })?;
        let name = signature.name.clone();
        match site {
            CallSite::Expression(expression) => {
                if let Some(call) = program.expression_table.source_call_node_mut(expression) {
                    call.target_symbol = instance;
                    call.target = name;
                    call.authored_machine_application = application;
                    call.machine_arguments = Default::default();
                }
            }
            CallSite::Statement(statement) => {
                if let StatementNode::Call(call) = program.statement_table.statement_mut(statement)
                {
                    call.target_symbol = instance;
                    call.target = name;
                    call.authored_machine_application = application;
                    call.machine_arguments = Default::default();
                }
            }
        }
        changed.push(owner);
    }
    Ok(changed)
}
