//! An entry context is an ordinary local owned by a generated caller.
//!
//! Generate before checking so zero establishment, the exclusive loan, call
//! preconditions and normal cleanup share the authored-code implementation.
//! Neither this body nor its origin grants installation or backing authority;
//! native entry settlement must join it to the exact selected source occurrence.

use arena::HandleSpan;
use diagnostics::Diagnostic;
use symbols::{SymbolHandle, SymbolKind};
use typed_trees::TypedTrees;
use typed_trees::expression::{
    ExpressionHandle, ExpressionNode, TableBorrowExpression, TableCallExpression, TableNamePath,
};
use typed_trees::machine::{EntryContextOrigin, Machine};
use typed_trees::name::Identifier;
use typed_trees::state::State;
use typed_trees::statement::{StatementNode, TableLocalData};
use typed_trees::types::{TypeReferenceNode, TypeReferenceTable};

pub(super) fn prepare(
    program: &mut TypedTrees,
    requests: &[EntryContextOrigin],
) -> Result<(), Vec<Diagnostic>> {
    for request in requests {
        prepare_entry(program, *request).map_err(|diagnostic| vec![diagnostic])?;
    }
    Ok(())
}

/// Field applications must have their final substituted views before zero
/// admission. The generated caller is formed earlier so it participates in
/// the same specialization and checking as every authored body.
pub(super) fn validate_zero_contexts(
    program: &TypedTrees,
    requests: &[EntryContextOrigin],
) -> Result<(), Vec<Diagnostic>> {
    for origin in requests {
        let entry = program.machine_by_symbol(origin.entry).ok_or_else(|| {
            vec![Diagnostic::error(
                "entry context request has no exact source machine",
            )]
        })?;
        let parameter = program
            .machine_states(entry)
            .first()
            .and_then(|state| {
                program
                    .state_parameters(state)
                    .iter()
                    .find(|parameter| parameter.symbol == origin.context_parameter)
            })
            .ok_or_else(|| {
                vec![Diagnostic::error(
                    "entry context request names a foreign parameter",
                )]
            })?;
        let owned = program
            .type_reference_table
            .peeled_type_reference(parameter.type_reference);
        let definition = program.closed_data_definition(owned).ok_or_else(|| {
            vec![Diagnostic::error(
                "entry context requires a closed record type",
            )]
        })?;
        if validation::data_requires_establishment(program, definition) {
            return Err(vec![Diagnostic::error(format!(
                "entry machine `{}` requests a provisioned `{}` context, but its all-zero image is not a valid value; use a free entry and construct the state explicitly",
                entry.name.as_str(),
                definition.name.as_str()
            ))]);
        }
    }
    Ok(())
}

fn prepare_entry(program: &mut TypedTrees, origin: EntryContextOrigin) -> Result<(), Diagnostic> {
    if program
        .machines()
        .iter()
        .any(|machine| machine.entry_context_origin == Some(origin))
    {
        return Ok(());
    }
    let entry = program
        .machine_by_symbol(origin.entry)
        .cloned()
        .ok_or_else(|| Diagnostic::error("entry context request has no exact source machine"))?;
    let state = program
        .machine_states(&entry)
        .first()
        .cloned()
        .ok_or_else(|| Diagnostic::error("entry context request has no source entry state"))?;
    let parameters = program.state_parameters(&state).to_vec();
    let context = parameters
        .iter()
        .position(|parameter| parameter.symbol == origin.context_parameter)
        .ok_or_else(|| Diagnostic::error("entry context request names a foreign parameter"))?;
    if entry.has_specialization_parameters() {
        return Err(Diagnostic::error(
            "entry context requires a closed source machine",
        ));
    }
    if parameters[context].is_const
        || parameters[context].relevance.is_erased()
        || program
            .placed_view_plan_for_input_type_reference(parameters[context].type_reference)
            .is_some()
        || !matches!(program.type_reference_table.type_reference(parameters[context].type_reference),
            TypeReferenceNode::Reference { access: language_core::ReferenceAccess::Mutable, referee, .. }
                if program.closed_data_definition(*referee).is_some())
    {
        return Err(Diagnostic::error(
            "entry context request requires an ordinary mutable record parameter",
        ));
    }
    let helper_name = format!("{}$entry_context", entry.symbol_spelling());
    let helper_symbol =
        program
            .symbols
            .insert_generated_root_from(entry.symbol, SymbolKind::Machine, &helper_name);
    let state_symbol = program
        .symbols
        .insert_generated_children(helper_symbol, [(SymbolKind::State, "entry")])
        .handle_at(0);
    let symbols = program.symbols.insert_generated_children_from(
        state_symbol,
        parameters.iter().enumerate().map(|(position, parameter)| {
            (
                parameter.symbol,
                if position == context {
                    SymbolKind::Local
                } else {
                    SymbolKind::Parameter
                },
                parameter.name.as_str(),
            )
        }),
    );
    let mapping: Vec<_> = parameters
        .iter()
        .zip(symbols.handles())
        .map(|(parameter, symbol)| (parameter.symbol, symbol))
        .collect();

    // Copy just the signature, not the whole program. Its constraints may refer
    // to earlier parameters; remap every copied occurrence into the new scope.
    let mut signature_types = TypeReferenceTable::default();
    let mut signature_expressions = typed_trees::expression::ExpressionTable::default();
    let copied_types: Vec<_> = parameters
        .iter()
        .map(|parameter| parameter.type_reference)
        .chain(std::iter::once(state.return_type))
        .map(|reference| {
            signature_types.copy_from(
                &program.type_reference_table,
                &program.expression_table,
                &mut signature_expressions,
                reference,
            )
        })
        .collect();
    let mut parameter_types = Vec::new();
    for reference in copied_types {
        let tables = &mut program.tables;
        let reference = tables.type_reference_table.copy_from(
            &signature_types,
            &signature_expressions,
            &mut tables.expression_table,
            reference,
        );
        tables.type_reference_table.remap_symbols_in(
            reference,
            &mut tables.expression_table,
            &mapping,
        );
        parameter_types.push(reference);
    }
    let TypeReferenceNode::Reference {
        referee: context_type,
        ..
    } = program
        .type_reference_table
        .type_reference(parameter_types[context])
        .clone()
    else {
        return Err(Diagnostic::error(
            "entry context signature copy lost its reference type",
        ));
    };
    let mut helper_state = State {
        symbol: state_symbol,
        name: Identifier::generated("entry"),
        return_type: parameter_types[parameters.len()],
        ..Default::default()
    };
    // The selected source root's static premises remain obligations of the
    // same external root after inserting this caller. Runtime parameter facts
    // do not move: the ordinary call must prove them from actual arrival
    // arguments and the context's zero establishment.
    retain_static_entry_contracts(program, &entry, &state, &parameters, &mut helper_state);
    let mut arguments = Vec::new();
    for (position, parameter) in parameters.iter().enumerate() {
        let symbol = mapping[position].1;
        let value = name(program, symbol);
        if position == context {
            arguments.push(program.expression_table.insert(ExpressionNode::Borrow(
                TableBorrowExpression {
                    target: value,
                    access: language_core::ReferenceAccess::Mutable,
                },
            )));
        } else {
            let mut parameter = parameter.clone();
            parameter.symbol = symbol;
            parameter.type_reference = parameter_types[position];
            // The ordinary parameter declaration owns these membership rows.
            // Rebuild them from its copied type. The static root premises above
            // supply no fact about a runtime argument or the new local context.
            for (domain_symbol, domain_name, domain_arguments, semantic_domain) in
                validation::parameter_domain_constraints(program, parameter.type_reference, &[])
            {
                let contract = program.parameter_domain_membership_contract(
                    parameter.symbol,
                    parameter.name.clone(),
                    domain_symbol,
                    &domain_name,
                    domain_arguments,
                    semantic_domain,
                );
                program.push_state_contract(&mut helper_state, contract);
            }
            program.push_state_parameter(&mut helper_state, parameter);
            arguments.push(value);
        }
    }
    let arguments = program
        .expression_table
        .insert_expression_handles(arguments);
    // Forward the ordinary result, including its custody, through normal return
    // cleanup. A result borrowing this local must fail the same escape check as
    // authored code; silently dropping a non-Unit result would change semantics.
    let call = program
        .expression_table
        .insert(ExpressionNode::Call(TableCallExpression {
            authored_machine_application: Default::default(),
            exact_coercion_target: Default::default(),
            requirement_application: Default::default(),
            receiver: Default::default(),
            target_symbol: state.symbol,
            static_machine_parameter: Default::default(),
            target: entry.name.clone(),
            static_requirement_dispatch: None,
            machine_arguments: Box::default(),
            quotient_operation: None,
            private_layout_operation: None,
            layout_query: None,
            native_code_layout: None,
            root_binding: Default::default(),
            arguments,
            evidence_arguments: Box::default(),
            operational_acknowledgement: language_semantics::CallOperationalAcknowledgement {
                origin:
                    language_semantics::CallOperationalAcknowledgementOrigin::CompilerSynthesized,
                ..Default::default()
            },
        }));
    helper_state.statement_nodes = program.statement_table.insert_statements([
        StatementNode::LocalData(TableLocalData {
            symbol: mapping[context].1,
            name: parameters[context].name.clone(),
            type_reference: context_type,
            is_mutable: true,
            ..Default::default()
        }),
        StatementNode::Expression(call),
    ]);
    let mut helper = Machine {
        symbol: helper_symbol,
        name: Identifier::generated(helper_name),
        entry_context_origin: Some(origin),
        lifetime_parameters: entry.lifetime_parameters.clone(),
        // These are provenance markers, not a new target-family selection.
        target: entry.target.clone(),
        authored_target: entry.authored_target.clone(),
        ..Default::default()
    };
    program.push_machine_state(&mut helper, helper_state);
    program.push_machine(helper);
    Ok(())
}

fn name(program: &mut TypedTrees, symbol: SymbolHandle) -> ExpressionHandle {
    let identifier = Identifier::generated(program.symbols.name(symbol));
    let mut members = HandleSpan::empty();
    program
        .expression_table
        .push_name_path_member(&mut members, identifier);
    program
        .expression_table
        .insert(ExpressionNode::Name(TableNamePath::new(
            members,
            HandleSpan::empty(),
            symbol,
            symbol,
        )))
}

/// Preserve root premises that do not observe any source runtime parameter.
/// Shared fact traversal retains declaration identities through nested terms;
/// neither source spelling nor a recognized comparison shape selects a premise.
fn retain_static_entry_contracts(
    program: &mut TypedTrees,
    entry: &Machine,
    state: &State,
    parameters: &[typed_trees::signature::StateParameter],
    helper: &mut State,
) {
    struct Dependencies(Vec<SymbolHandle>);
    impl Extend<SymbolHandle> for Dependencies {
        fn extend<I: IntoIterator<Item = SymbolHandle>>(&mut self, symbols: I) {
            self.0.extend(symbols);
        }
    }
    impl typed_trees::references::TypedReferences for Dependencies {}

    let contracts: Vec<_> = program
        .machine_contracts(entry)
        .iter()
        .chain(program.state_contracts(state))
        .cloned()
        .collect();
    for mut contract in contracts {
        if contract.kind != typed_trees::signature::SignatureContractKind::Requires {
            continue;
        }
        let mut retained = HandleSpan::empty();
        let mut facts = program
            .proof_facts
            .span_or_empty(contract.facts)
            .iter()
            .rev()
            .cloned()
            .collect::<Vec<_>>();
        while let Some(fact) = facts.pop() {
            // Independent conjuncts keep their own dependency closures. A
            // context predicate beside a static premise neither erases the
            // latter nor becomes a premise of the generated caller.
            if let typed_trees::domain::ProofFact::Expression(expression) = &fact
                && let Some(binary) = program.expression_table.binary_node(*expression)
                && binary.operator == typed_trees::expression::BinaryOperator::And
                && validation::has_builtin_binary_expression_meaning(
                    program,
                    entry,
                    Some(state),
                    *expression,
                )
            {
                facts.push(typed_trees::domain::ProofFact::Expression(binary.right));
                facts.push(typed_trees::domain::ProofFact::Expression(binary.left));
                continue;
            }
            let mut dependencies = Dependencies(Vec::new());
            typed_trees::references::visit_proof_fact(program, &mut dependencies, &fact);
            if dependencies.0.iter().any(|symbol| {
                parameters
                    .iter()
                    .any(|parameter| parameter.symbol == *symbol)
            }) {
                continue;
            }
            program.proof_facts.append_to_span(&mut retained, fact);
        }
        if !retained.is_empty() {
            contract.facts = retained;
            program.push_state_contract(helper, contract);
        }
    }
}
