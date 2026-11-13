//! A call such as `relay<u8>(3u8, 4)` reaches typed preparation naming the
//! generic machine `relay` and carrying `u8` only as written. This pass
//! pairs each static argument with the parameter it is for, and where the
//! arguments are closed it generates the machine the application selects,
//! `relay` with `u8` in place of `T`, and points the call at it.
//!
//! generics.md, "Closed applications and canonical identity": "Substitution
//! is declaration-identity based throughout bodies, contracts, field types,
//! and nested applications." A static argument is therefore bound to its
//! parameter's symbol and replaces that symbol wherever the copied
//! declaration names it; no spelling is compared. One application makes one
//! machine: a second call at the same arguments enters the first call's
//! machine. A generated body is read like any other, so the applications it
//! holds are served in turn (`forward<T>` inside `relay` becomes
//! `forward<u8>` inside `relay` at `u8`).
//!
//! Nothing here checks. The generated machine is the generic declaration's
//! own body and contract at the arguments; whether that body keeps its
//! contract is the lowered-form checker's judgment, made once on the generic
//! form and carried to each generated machine.
//!
//! An application this pass does not serve keeps the call as typing left it
//! and records a deferred refusal on the machine holding the call, so only a
//! program that demands that machine is refused. Served today: type
//! parameters given plain named types, on a machine that is a single state
//! with a body and declares nothing else to specialize ([`template`]).

use arena::HandleSpan;
use diagnostics::Diagnostic;
use source::SourceSpan;
use symbols::{SymbolHandle, SymbolKind};
use typed_trees::deferred_refusals::DeferredRequest;
use typed_trees::domain::ProofFact;
use typed_trees::expression::{
    ExpressionHandle, ExpressionNode, ExpressionTable, StaticMachineArgument,
};
use typed_trees::machine::Machine;
use typed_trees::name::Identifier;
use typed_trees::signature::SignatureContract;
use typed_trees::state::State;
use typed_trees::static_applications::{
    StaticMachineApplicationHandle, StaticMachineBinding, StaticMachineBindingValue,
};
use typed_trees::statement::{StatementHandle, StatementNode};

use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};
use typed_trees::{
    MachineSpecialization, SubtreeEdges, SymbolRebinding, TypedTreeTables, TypedTrees,
};

pub(crate) fn prepare(program: &mut TypedTrees) -> Result<(), Vec<Diagnostic>> {
    let mut generated = Vec::new();
    // A generated machine is appended to the machines and read in its turn.
    let mut position = 0;
    while position < program.machines().len() {
        let holder = program.machines()[position].symbol;
        for site in call_sites(program, position) {
            match serve(program, site, &mut generated) {
                Ok(()) => {}
                Err(Stop::Unserved) => {
                    let span = site.source_span(program);
                    program.record_deferred_refusal(
                        holder,
                        DeferredRequest::StaticApplication,
                        span,
                    );
                }
                Err(Stop::Defect(message)) => return Err(vec![Diagnostic::error(message)]),
            }
        }
        position += 1;
    }
    Ok(())
}

/// Why one application was left as typing wrote it.
enum Stop {
    /// A request this pass does not serve yet; nothing was changed for it.
    Unserved,
    /// The trees broke an invariant while a machine was being generated.
    Defect(&'static str),
}

type Served<T> = Result<T, Stop>;

/// One generated machine: the generic machine, the declaration each of its
/// parameters was bound to, and the generated machine and its one state.
struct Generated {
    template: SymbolHandle,
    arguments: Vec<SymbolHandle>,
    machine: SymbolHandle,
    state: SymbolHandle,
}

/// A call as the trees hold one: an expression, or a statement that
/// discards the result.
#[derive(Clone, Copy)]
enum Site {
    Expression(ExpressionHandle),
    Statement(StatementHandle),
}

/// What a call applies: its target, its static application, and whether the
/// call is selected some other way (a requirement dispatch, a Build
/// selection), which is not this pass's request.
struct Applied {
    target: SymbolHandle,
    application: StaticMachineApplicationHandle,
    selected_otherwise: bool,
}

impl Site {
    fn applied(self, program: &TypedTrees) -> Option<Applied> {
        match self {
            Self::Expression(handle) => match program.expression_table.expression(handle) {
                ExpressionNode::Call(call) => Some(Applied {
                    target: call.target_symbol,
                    application: call.authored_machine_application,
                    selected_otherwise: call.static_requirement_dispatch.is_some()
                        || call.build_selection.is_valid(),
                }),
                _ => None,
            },
            Self::Statement(handle) => {
                program
                    .statement_table
                    .statement(handle)
                    .as_call()
                    .map(|call| Applied {
                        target: call.target_symbol,
                        application: call.authored_machine_application,
                        selected_otherwise: call.static_requirement_dispatch.is_some(),
                    })
            }
        }
    }

    fn source_span(self, program: &TypedTrees) -> SourceSpan {
        match self {
            Self::Expression(handle) => program.expression_table.source_span(handle),
            Self::Statement(handle) => program
                .statement_table
                .statement(handle)
                .as_call()
                .map(|call| call.source_span)
                .unwrap_or_default(),
        }
    }

    /// Points the call at `target`, the machine its application selects.
    /// The static arguments the call listed are spent: the generated machine
    /// has no parameter left to apply.
    fn enter(self, program: &mut TypedTrees, target: SymbolHandle) {
        match self {
            Self::Expression(handle) => {
                if let ExpressionNode::Call(call) =
                    program.expression_table.expression_mut(handle)
                {
                    call.target_symbol = target;
                    call.machine_arguments = Box::new([]);
                }
            }
            Self::Statement(handle) => {
                if let StatementNode::Call(call) = program.statement_table.statement_mut(handle) {
                    call.target_symbol = target;
                    call.machine_arguments = Box::new([]);
                }
            }
        }
    }
}

/// Every call written in the bodies of the machine at `position`.
fn call_sites(program: &TypedTrees, position: usize) -> Vec<Site> {
    let machine = &program.machines()[position];
    let mut sites = Vec::new();
    for state in program.machine_states(machine) {
        for handle in state.statement_nodes.handles() {
            let statement = program.statement_table.statement(handle);
            if statement.as_call().is_some() {
                sites.push(Site::Statement(handle));
            }
            for (_, root) in program.tables.expression_roots(statement) {
                if !root.is_valid() {
                    continue;
                }
                program.subtree_for_each(root, SubtreeEdges::default(), |handle, node| {
                    if matches!(node, ExpressionNode::Call(_)) {
                        sites.push(Site::Expression(handle));
                    }
                });
            }
        }
    }
    sites
}

/// The machine a call target enters, and whether the call named the
/// machine's state (as an authored call does) rather than the machine.
fn entered_machine(program: &TypedTrees, target: SymbolHandle) -> Option<(SymbolHandle, bool)> {
    if !target.is_valid() || !program.symbols.symbols().nodes().is_valid(target) {
        return None;
    }
    let symbol = program.symbols.get(target);
    match symbol.kind {
        SymbolKind::Machine => Some((target, false)),
        SymbolKind::State => Some((symbol.parent, true)),
        _ => None,
    }
}

fn serve(program: &mut TypedTrees, site: Site, generated: &mut Vec<Generated>) -> Served<()> {
    let Some(applied) = site.applied(program) else {
        return Ok(());
    };
    if !applied.application.is_valid() || applied.selected_otherwise {
        return Ok(());
    }
    let arguments = program
        .expression_table
        .static_machine_application_arguments(applied.application)
        .to_vec();
    if arguments.is_empty() {
        return Ok(());
    }
    // Only an application of a declared machine with parameters is a
    // request to specialize. Static arguments given to anything else (a
    // type query, a requirement of a trait) belong to whoever reads that
    // call, and a call already entering a generated machine was served.
    let Some((callee, names_state)) = entered_machine(program, applied.target) else {
        return Ok(());
    };
    if program.symbols.get(applied.target).generated_from.is_valid() {
        return Ok(());
    }
    let Some(machine) = program.machine_by_symbol(callee) else {
        return Ok(());
    };
    let parameters = program
        .machine_type_parameters(machine)
        .iter()
        .map(|parameter| (parameter.symbol, parameter.kind.clone()))
        .collect::<Vec<_>>();
    if parameters.is_empty() {
        return Ok(());
    }
    if parameters.len() != arguments.len() {
        return Err(Stop::Unserved);
    }
    let mut operands = Vec::with_capacity(parameters.len());
    for ((parameter, kind), argument) in parameters.iter().zip(&arguments) {
        operands.push(Operand::read(program, *parameter, kind, argument)?);
    }
    bind(program, applied.application, &operands);
    // An argument that names a parameter of the enclosing generic machine
    // leaves the application open: it is closed in each machine generated
    // from that one, where the parameter is replaced.
    if operands.iter().any(|operand| operand.is_open(program)) {
        return Ok(());
    }
    let key = operands
        .iter()
        .map(|operand| operand.declaration)
        .collect::<Vec<_>>();
    let entry = match generated
        .iter()
        .find(|existing| existing.template == callee && existing.arguments == key)
    {
        Some(existing) => existing,
        None => {
            let made = generate(program, callee, applied.application, &operands)?;
            generated.push(made);
            generated.last().expect("the machine just generated")
        }
    };
    let target = if names_state {
        entry.state
    } else {
        entry.machine
    };
    site.enter(program, target);
    Ok(())
}

/// One static argument read for the parameter it is given to. Every kind of
/// parameter is bound through this one shape; a kind with no reading yet is
/// unserved.
struct Operand {
    parameter: SymbolHandle,
    /// The declaration the argument names: the identity an application is
    /// keyed by and the one substituted for the parameter.
    declaration: SymbolHandle,
    /// The argument as the author wrote it, which replaces the parameter
    /// where a copied body passes it on as a static argument.
    argument: StaticMachineArgument,
    value: StaticMachineBindingValue,
}

impl Operand {
    fn read(
        program: &mut TypedTrees,
        parameter: SymbolHandle,
        kind: &typed_trees::data::TypeParameterKind,
        argument: &StaticMachineArgument,
    ) -> Served<Self> {
        use typed_trees::data::TypeParameterKind;
        let value = match kind {
            TypeParameterKind::Type => {
                StaticMachineBindingValue::Type(named_type(program, argument)?)
            }
            // A value, a machine, an implementation and a proposition each
            // need their own reading of the argument; none is built yet.
            TypeParameterKind::Implementation
            | TypeParameterKind::Const { .. }
            | TypeParameterKind::Value { .. }
            | TypeParameterKind::Machine { .. }
            | TypeParameterKind::Proposition { .. } => return Err(Stop::Unserved),
        };
        Ok(Self {
            parameter,
            declaration: argument.symbol,
            argument: argument.clone(),
            value,
        })
    }

    fn is_open(&self, program: &TypedTrees) -> bool {
        program.symbols.get(self.declaration).kind == SymbolKind::TypeParameter
    }
}

/// The type a static argument names when it is only a name: a builtin type,
/// a type parameter in scope, or a data declaration with no parameters of
/// its own. An applied type (`Pair<u8>`), a projection and a literal are
/// unserved here.
fn named_type(program: &mut TypedTrees, argument: &StaticMachineArgument) -> Served<TypeReferenceHandle> {
    let only_a_name = argument.application.is_none()
        && argument.const_literal.is_none()
        && argument.evidence_projection.is_none()
        && !argument.type_reference.is_valid();
    let symbol = argument.symbol;
    if !only_a_name || !symbol.is_valid() || !program.symbols.symbols().nodes().is_valid(symbol) {
        return Err(Stop::Unserved);
    }
    let plain = match program.symbols.get(symbol).kind {
        SymbolKind::BuiltinType | SymbolKind::TypeParameter => true,
        SymbolKind::Data => program
            .unique_data_definition_by_symbol(symbol)
            .is_some_and(|data| {
                program.data_type_parameters(data).is_empty() && data.lifetime_parameters.is_empty()
            }),
        _ => false,
    };
    let Some(name) = argument.path.last().filter(|_| plain) else {
        return Err(Stop::Unserved);
    };
    Ok(program
        .type_reference_table
        .insert(TypeReferenceNode::Named {
            symbol,
            name: name.clone(),
        }))
}

/// Records, on the application, which parameter each argument is for.
fn bind(
    program: &mut TypedTrees,
    application: StaticMachineApplicationHandle,
    operands: &[Operand],
) {
    let bindings = operands
        .iter()
        .map(|operand| StaticMachineBinding {
            parameter: operand.parameter,
            value: operand.value.clone(),
        })
        .collect::<Vec<_>>();
    program
        .expression_table
        .set_static_machine_application_bindings(application, &bindings);
}

/// The generic machine `symbol` when this pass can copy it whole: one state
/// with a body, and nothing else that would have to be specialized with it.
/// What is excluded is not judged wrong, only not copied yet; each exclusion
/// is a part of a machine the copy below does not carry.
fn template(program: &TypedTrees, symbol: SymbolHandle) -> Served<(Machine, State)> {
    let (machine, state, types) = copied_parts(program, symbol)?;
    // A type holding an expression of its own (an array extent, say) is not
    // copied: the expression would need the context it is evaluated in.
    if holds_expression(program, &types) {
        return Err(Stop::Unserved);
    }
    Ok((machine, state))
}

/// The machine and its state, with every type the copy will have to copy:
/// the signature's and the ones the contract and body expressions hold.
fn copied_parts(
    program: &TypedTrees,
    symbol: SymbolHandle,
) -> Served<(Machine, State, Vec<TypeReferenceHandle>)> {
    let unserved = Err(Stop::Unserved);
    let Some(machine) = program.machine_by_symbol(symbol) else {
        return unserved;
    };
    let [state] = program.machine_states(machine) else {
        return unserved;
    };
    let plan = &machine.termination_plan;
    let plain = machine.body_is_present
        && machine.supply_mode == language_semantics::MachineSupplyMode::CheckedBody
        && machine.lifetime_parameters.is_empty()
        && machine.conformance_bounds.is_empty()
        && machine.declared_type_equations.is_empty()
        && machine.owned_data.is_empty()
        && machine.satisfies.is_empty()
        && machine.invokes.is_empty()
        && machine.attached_data.is_none()
        && !machine.attached_data_symbol.is_valid()
        && !machine.generic_data_template.is_valid()
        && machine.target.is_none()
        && !machine.target_family_interface
        && machine.spelling.is_none()
        && !machine.structural_type_equations_pending
        && !machine.service_reach_is_installation_bound
        && matches!(
            plan.interface,
            language_semantics::TerminationInterface::InternalDerived
        )
        && plan.checked_summary == language_semantics::TerminationGuarantee::NoGuarantee
        && plan.implementation_witness.is_none()
        && program.ranking_expression_custody_for(symbol).is_none()
        && program.authored_service_reach_rows_for(symbol).next().is_none()
        && !program
            .proof_output_calls
            .iter()
            .any(|call| call.machine_symbol == symbol)
        && !program
            .evidence_forwardings
            .iter()
            .any(|forwarding| forwarding.machine_symbol == symbol)
        // A machine declared inside this one would be copied with it.
        && !program.machines().iter().any(|nested| {
            let parent = program.symbols.get(nested.symbol).parent;
            parent == symbol || parent == state.symbol
        });
    if !plain {
        return unserved;
    }
    let facts = program
        .machine_contracts(machine)
        .iter()
        .chain(program.state_contracts(state))
        .flat_map(|contract| program.proof_facts.span_or_empty(contract.facts));
    let mut roots = Vec::new();
    for fact in facts {
        let ProofFact::Expression(expression) = fact else {
            return unserved;
        };
        roots.push(*expression);
    }
    for handle in state.statement_nodes.handles() {
        let statement = program.statement_table.statement(handle);
        let copied = match statement {
            StatementNode::Assembly(_) | StatementNode::AssemblyFact(_) => false,
            StatementNode::Call(call) => {
                call.installed_interrupt_operation.is_none()
                    && call.reflected_field_origin.is_none()
                    && call.machine_arguments.iter().all(only_names)
            }
            StatementNode::LocalData(local) => local.reflected_query_origin.is_none(),
            StatementNode::Assignment(_)
            | StatementNode::Expression(_)
            | StatementNode::Transition(_) => true,
        };
        if !copied {
            return unserved;
        }
        roots.extend(
            program
                .tables
                .expression_roots(statement)
                .into_iter()
                .map(|(_, root)| root),
        );
    }
    let mut copied = true;
    let mut types = program
        .state_parameters(state)
        .iter()
        .map(|parameter| parameter.type_reference)
        .chain([state.return_type])
        .collect::<Vec<_>>();
    for root in roots.into_iter().filter(|root| root.is_valid()) {
        program.subtree_for_each(root, SubtreeEdges::default(), |handle, node| {
            types.extend(held_types(node));
            copied &= program
                .expression_table
                .index_expression_authority(handle)
                .is_none();
            copied &= match node {
                ExpressionNode::Call(call) => {
                    call.layout_query.is_none()
                        && call.native_code_layout.is_none()
                        && call.quotient_operation.is_none()
                        && call.private_layout_operation.is_none()
                        && call.installed_interrupt_operation.is_none()
                        && !call.build_selection.is_valid()
                        && call.machine_arguments.iter().all(only_names)
                }
                ExpressionNode::Cast(cast) => cast.semantic_domain_arguments.is_empty(),
                _ => true,
            };
        });
    }
    if !copied {
        return unserved;
    }
    Ok((machine.clone(), state.clone(), types))
}

/// A static argument that is a name and nothing else, so replacing the
/// declaration it names replaces all of it.
fn only_names(argument: &StaticMachineArgument) -> bool {
    argument.application.is_none()
        && argument.evidence_projection.is_none()
        && !argument.type_reference.is_valid()
}

/// Copies `expression` into fresh rows of the same program and renames the
/// declarations it mentions. The copy passes through a table of its own
/// because the expression table copies only from another table.
fn copy_expression(
    program: &mut TypedTrees,
    expression: ExpressionHandle,
    symbols: SymbolRebinding<'_>,
) -> ExpressionHandle {
    if !expression.is_valid() {
        return expression;
    }
    let mut staging = ExpressionTable::default();
    let staged = staging.copy_from(&program.expression_table, expression);
    let copied = program.expression_table.copy_from(&staging, staged);
    program.expression_table.remap_symbols_in(copied, symbols);
    copied
}

/// Copies a type into fresh rows and renames the declarations it mentions.
/// `template` admitted no type that holds an expression of its own.
fn copy_type(
    program: &mut TypedTrees,
    reference: TypeReferenceHandle,
    symbols: SymbolRebinding<'_>,
) -> Served<TypeReferenceHandle> {
    if !reference.is_valid() {
        return Ok(reference);
    }
    let mut staging = TypedTreeTables::default();
    let mut embedded = Vec::new();
    let staged = staging.type_reference_table.copy_with_expressions(
        &program.type_reference_table,
        &program.expression_table,
        &mut staging.expression_table,
        reference,
        &mut embedded,
    );
    if !embedded.is_empty() {
        return Err(Stop::Defect(
            "a type copied for a generated machine holds an expression",
        ));
    }
    let tables = &mut program.tables;
    let copied = tables.type_reference_table.copy_with_expressions(
        &staging.type_reference_table,
        &staging.expression_table,
        &mut tables.expression_table,
        staged,
        &mut embedded,
    );
    tables
        .type_reference_table
        .remap_symbols_in(copied, &mut tables.expression_table, symbols);
    Ok(copied)
}

fn copy_contract(
    program: &mut TypedTrees,
    contract: &SignatureContract,
    symbols: SymbolRebinding<'_>,
) -> SignatureContract {
    let mut copied = contract.clone();
    copied.facts = HandleSpan::empty();
    for source in contract.facts.handles() {
        // `template` admitted only facts that are one expression.
        let ProofFact::Expression(expression) = *program.proof_facts.get(source) else {
            continue;
        };
        let span = program.proof_fact_source_span(source);
        let expression = copy_expression(program, expression, symbols);
        let fact = program
            .proof_facts
            .append_to_span(&mut copied.facts, ProofFact::Expression(expression));
        if let Some(span) = span {
            program.set_proof_fact_source_span(fact, span);
        }
    }
    copied
}

/// Whether any type among `references` holds an expression, which
/// `copy_type` does not carry.
fn holds_expression(program: &TypedTrees, references: &[TypeReferenceHandle]) -> bool {
    let mut staging = TypedTreeTables::default();
    let mut embedded = Vec::new();
    for reference in references.iter().filter(|reference| reference.is_valid()) {
        staging.type_reference_table.copy_with_expressions(
            &program.type_reference_table,
            &program.expression_table,
            &mut staging.expression_table,
            *reference,
            &mut embedded,
        );
    }
    !embedded.is_empty()
}

/// Generates the machine `application` selects from the generic machine
/// `template_symbol`: its state, formals, contract and body copied under
/// fresh symbols, with each parameter replaced by its operand.
fn generate(
    program: &mut TypedTrees,
    template_symbol: SymbolHandle,
    application: StaticMachineApplicationHandle,
    operands: &[Operand],
) -> Served<Generated> {
    let (source, source_state) = template(program, template_symbol)?;
    let parameters = program.state_parameters(&source_state).to_vec();
    // The body is copied before anything is written to the program, so a
    // body this pass cannot copy leaves no trace.
    let mut staging = TypedTreeTables::default();
    let mut embedded = Vec::new();
    let staged = staging.copy_statement_nodes_with_type_expressions(
        &program.tables,
        source_state.statement_nodes,
        &mut embedded,
    );
    if !embedded.is_empty() {
        return Err(Stop::Unserved);
    }

    let type_start = program.type_reference_table.type_reference_count() + 1;
    let expression_start = program.expression_table.expression_count() + 1;

    let locals = program
        .statement_table
        .statements(source_state.statement_nodes)
        .iter()
        .filter_map(|statement| statement.as_local_data())
        .map(|local| (local.symbol, local.name.clone()))
        .filter(|(symbol, _)| symbol.is_valid())
        .collect::<Vec<_>>();
    let spelled = operands
        .iter()
        .map(|operand| program.symbols.name(operand.declaration).to_owned())
        .collect::<Vec<_>>()
        .join("$");
    let name = format!(
        "{}$specialized${spelled}${}",
        source.name.as_str(),
        program.symbols.symbols().len() + 1,
    );
    let machine_symbol =
        program
            .symbols
            .insert_generated_root_from(source.symbol, SymbolKind::Machine, &name);
    let state_symbol = program
        .symbols
        .insert_generated_children_from(
            machine_symbol,
            [(
                source_state.symbol,
                SymbolKind::State,
                source_state.name.as_str(),
            )],
        )
        .handle_at(0);
    let children = program.symbols.insert_generated_children_from(
        state_symbol,
        parameters
            .iter()
            .map(|parameter| {
                (
                    parameter.symbol,
                    SymbolKind::Parameter,
                    parameter.name.as_str(),
                )
            })
            .chain(
                locals
                    .iter()
                    .map(|(symbol, name)| (*symbol, SymbolKind::Local, name.as_str())),
            ),
    );
    let mut renamed = vec![
        (source.symbol, machine_symbol),
        (source_state.symbol, state_symbol),
    ];
    let authored = parameters
        .iter()
        .map(|parameter| parameter.symbol)
        .chain(locals.iter().map(|(symbol, _)| *symbol))
        .zip(children.handles())
        .collect::<Vec<_>>();
    renamed.extend(authored.iter().copied());
    let symbols = SymbolRebinding::new(&renamed, &authored);

    let mut machine = source.clone();
    machine.symbol = machine_symbol;
    machine.name = Identifier::generated(name);
    machine.is_public = false;
    machine.type_parameters = HandleSpan::empty();
    machine.contracts = HandleSpan::empty();
    machine.states = HandleSpan::empty();
    for contract in program.machine_contracts(&source).to_vec() {
        let contract = copy_contract(program, &contract, symbols);
        program.push_machine_contract(&mut machine, contract);
    }

    let mut state = source_state.clone();
    state.symbol = state_symbol;
    state.parameters = HandleSpan::empty();
    state.contracts = HandleSpan::empty();
    state.return_type = copy_type(program, source_state.return_type, symbols)?;
    state.statement_nodes = program.tables.copy_statement_nodes_with_type_expressions(
        &staging,
        staged,
        &mut embedded,
    );
    program
        .tables
        .remap_statement_symbols(state.statement_nodes, symbols);
    for mut parameter in parameters {
        parameter.symbol = renamed_symbol(&renamed, parameter.symbol);
        parameter.type_reference = copy_type(program, parameter.type_reference, symbols)?;
        program.push_state_parameter(&mut state, parameter);
    }
    for contract in program.state_contracts(&source_state).to_vec() {
        let contract = copy_contract(program, &contract, symbols);
        program.push_state_contract(&mut state, contract);
    }
    copy_expression_types(program, expression_start, symbols)?;
    substitute_types(program, type_start, operands)?;
    substitute_arguments(
        program,
        expression_start,
        state.statement_nodes,
        machine_symbol,
        operands,
    );
    program.push_machine_state(&mut machine, state);
    program.push_machine(machine);

    let spellings = operands
        .iter()
        .map(|operand| program.symbols.display_path(operand.declaration, "::"))
        .collect::<Vec<_>>();
    program.machine_specializations.push(MachineSpecialization {
        template: template_symbol,
        instance: machine_symbol,
        template_parameters: source.type_parameters,
        binding_application: application,
        type_arguments: spellings.clone(),
        type_argument_identities: spellings,
        normalized_template_identity: program.symbols.display_path(template_symbol, "::"),
        ..MachineSpecialization::default()
    });
    Ok(Generated {
        template: template_symbol,
        arguments: operands
            .iter()
            .map(|operand| operand.declaration)
            .collect(),
        machine: machine_symbol,
        state: state_symbol,
    })
}

fn renamed_symbol(renamed: &[(SymbolHandle, SymbolHandle)], symbol: SymbolHandle) -> SymbolHandle {
    renamed
        .iter()
        .find(|(original, _)| *original == symbol)
        .map_or(symbol, |(_, fresh)| *fresh)
}

/// The types an expression node holds beside its operands.
fn held_types(node: &ExpressionNode) -> [TypeReferenceHandle; 2] {
    let none = TypeReferenceHandle::invalid();
    match node {
        ExpressionNode::Call(call) => [call.requirement_application, call.exact_coercion_target],
        ExpressionNode::Cast(cast) => [cast.target_type, cast.result_type],
        ExpressionNode::StructLiteral(literal) => [literal.application_type, none],
        ExpressionNode::ZeroValue(reference) | ExpressionNode::TypeExpression(reference) => {
            [*reference, none]
        }
        _ => [none, none],
    }
}

/// An expression copy shares the types its nodes hold with the expression
/// it was copied from. Each copied node receives types of its own, so that
/// replacing a parameter in the generated machine leaves the generic
/// machine's types as they were.
fn copy_expression_types(
    program: &mut TypedTrees,
    expression_start: usize,
    symbols: SymbolRebinding<'_>,
) -> Served<()> {
    let copied = program
        .expression_table
        .iter_expressions_from(expression_start)
        .filter_map(|(handle, node)| {
            let types = held_types(node);
            types
                .iter()
                .any(|reference| reference.is_valid())
                .then_some((handle, types))
        })
        .collect::<Vec<_>>();
    for (handle, [first, second]) in copied {
        let first = copy_type(program, first, symbols)?;
        let second = copy_type(program, second, symbols)?;
        match program.expression_table.expression_mut(handle) {
            ExpressionNode::Call(call) => {
                call.requirement_application = first;
                call.exact_coercion_target = second;
            }
            ExpressionNode::Cast(cast) => {
                cast.target_type = first;
                cast.result_type = second;
            }
            ExpressionNode::StructLiteral(literal) => literal.application_type = first,
            ExpressionNode::ZeroValue(reference) | ExpressionNode::TypeExpression(reference) => {
                *reference = first;
            }
            _ => {}
        }
    }
    Ok(())
}

/// Replaces each type parameter by its operand in every type written since
/// `type_start`: the generated machine's own types.
fn substitute_types(
    program: &mut TypedTrees,
    type_start: usize,
    operands: &[Operand],
) -> Served<()> {
    for operand in operands {
        let StaticMachineBindingValue::Type(actual) = &operand.value else {
            continue;
        };
        let occurrences = program
            .type_reference_table
            .named_references_from(type_start)
            .filter(|(_, symbol, _)| *symbol == operand.parameter)
            .map(|(handle, _, _)| handle)
            .collect::<Vec<_>>();
        let replacement = copy_type(program, *actual, SymbolRebinding::new(&[], &[]))?;
        for occurrence in occurrences {
            program
                .type_reference_table
                .substitute_from(occurrence, replacement)
                .map_err(Stop::Defect)?;
        }
        program.type_reference_table.substitute_shared_children(
            type_start,
            operand.parameter,
            replacement,
        );
    }
    Ok(())
}

/// Replaces each parameter a copied call passes on as a static argument
/// (`forward<T>` in the body of `relay`) by the operand's own argument. The
/// copied application is then unbound again, with this machine as its
/// holder, so it is bound and served from its new arguments like an
/// authored one.
fn substitute_arguments(
    program: &mut TypedTrees,
    expression_start: usize,
    statements: HandleSpan<StatementNode>,
    holder: SymbolHandle,
    operands: &[Operand],
) {
    let replace = |arguments: &mut [StaticMachineArgument]| {
        for argument in arguments {
            if let Some(operand) = operands
                .iter()
                .find(|operand| operand.parameter == argument.symbol)
            {
                *argument = operand.argument.clone();
            }
        }
    };
    let mut applications = Vec::new();
    let calls = program
        .expression_table
        .iter_expressions_from(expression_start)
        .filter(|(_, node)| matches!(node, ExpressionNode::Call(_)))
        .map(|(handle, _)| handle)
        .collect::<Vec<_>>();
    for handle in calls {
        if let ExpressionNode::Call(call) = program.expression_table.expression_mut(handle) {
            replace(&mut call.machine_arguments);
            applications.push(call.authored_machine_application);
        }
    }
    for handle in statements.handles() {
        if let StatementNode::Call(call) = program.statement_table.statement_mut(handle) {
            replace(&mut call.machine_arguments);
            applications.push(call.authored_machine_application);
        }
    }
    for application in applications {
        if !application.is_valid() {
            continue;
        }
        let table = &mut program.expression_table;
        replace(table.static_machine_application_arguments_mut(application));
        table.set_static_machine_application_bindings(application, &[]);
        table.static_machine_application_mut(application).lexical_owner = holder;
    }
}

#[cfg(test)]
mod tests;
