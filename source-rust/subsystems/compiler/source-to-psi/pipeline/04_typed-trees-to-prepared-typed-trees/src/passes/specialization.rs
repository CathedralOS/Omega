//! A call such as `relay::<u8>(3u8, 4)` reaches typed preparation naming the
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
//! Before copying a body, a selected callable must satisfy its published
//! binder. The lowered-form checker then judges the generic body under that
//! binder and checks the generated calls at the selected declarations.
//!
//! An application this pass does not serve keeps the call as typing left it
//! and records a deferred refusal on the machine holding the call, so only a
//! program that demands that machine is refused. Served today: type
//! parameters given plain named types, implementation parameters given
//! plain named implementations, structural callable parameters given exact
//! named machines, and value parameters given one integer literal or
//! `true`/`false`, on single-state checked bodies with their lexical children
//! ([`template`]). Each child has no independent static telescope. It retains
//! the original enclosing application without capturing runtime values.
//!
//! A value parameter (`value<const Units: u64>`) is bound like the others
//! and is replaced by its literal, landed in the parameter's type, wherever
//! the copied declaration names it. generics.md, "Value binders and const
//! requirements": the value "must be statically known within the compiled
//! specialization". Only a closed scalar is read here: an integer literal,
//! `true` or `false`. Typing has already replaced a calculation over
//! literals, `(8u64 + 9u64)`, by the literal of its value
//! (`crate::source_state::const_integer::closed_literal`); an argument that a machine
//! would have to compute is unserved.
//! Where the parameter is the width of an integer, `u<N>`, the copied type
//! becomes the builtin integer of that width ([`substitute_widths`]).
//!
//! An implementation parameter (`advance<Selected satisfies Step>`) is bound
//! like a type parameter and differs only in what replaces it. conformances.md,
//! "Implementations are operation records": an implementation is a value
//! holding one selected operation per requirement. generics.md, "Static
//! calls": "Each invocation of a static callable argument becomes a direct
//! selected call after specialization." So in the generated machine a call
//! through the parameter, `Selected::apply(start)`, enters the machine the
//! implementation selected for `apply`, and records which requirement it was
//! written against ([`Selection`]).

use crate::source_state::CopiedCallableBody;
use crate::source_state::data::TypeParameterKind;
use crate::source_state::deferred_refusals::DeferredRequest;
use crate::source_state::domain::ProofFact;
use crate::source_state::expression::{
    ExpressionHandle, ExpressionNode, ExpressionTable, StaticMachineArgument, TableNamePath,
};
use crate::source_state::machine::Machine;
use crate::source_state::name::Identifier;
use crate::source_state::references::{StaticArgumentCall, StaticArgumentRole};
use crate::source_state::state::State;
use crate::source_state::statement::{StatementHandle, StatementNode};
use crate::source_state::static_applications::{
    StaticMachineApplicationHandle, StaticMachineBindingValue,
};
use arena::HandleSpan;
use diagnostics::Diagnostic;
use source_text::SourceSpan;
use symbols::{SymbolHandle, SymbolKind};
use typed_trees::data_applications;
use typed_trees::static_operand_binding::{
    self, Identity, Operand, Served, Stop, Value, bind, named_type, unserved,
};

use crate::source_state::types::{TypeReferenceHandle, TypeReferenceNode};
use crate::source_state::{
    MachineSpecialization, PreparedSymbolRebinding, StaticRequirementDispatch, SubtreeEdges,
    SymbolRebinding, TypedTreeTables, TypedTrees,
};

mod callable_binder;
mod named_conformances;
mod propositions;
mod provider_methods;
mod reflected_fields;

#[cfg(test)]
fn prepare(program: &mut TypedTrees) -> Result<(), Vec<Diagnostic>> {
    prepare_selected(program, &[]).map(|_| ())
}

pub(crate) fn prepare_selected(
    program: &mut TypedTrees,
    selected: &[crate::SelectedDeclaration],
) -> Result<Vec<(crate::EntryApplication, SymbolHandle)>, Vec<Diagnostic>> {
    let first_type = program.type_reference_table.type_reference_count();
    let mut generated = GeneratedMachines::default();
    let mut generated_propositions = propositions::GeneratedPropositions::default();
    // Reflection creates one potential callback branch per declared case. Lowering
    // decides which branches exist on the exact applied record. Keep a rejection
    // with its branch, including machines specialized solely for that branch.
    let mut case_branches = std::collections::HashSet::new();
    let mut roots = Vec::new();
    let mut selected_providers = Vec::new();
    for &selection in selected {
        let result = match selection {
            crate::SelectedDeclaration::Provider(provider) => {
                selected_providers.push(provider);
                named_conformances::prepare_selected(program, provider)
                    .map_err(|reason| ("provider", reason))
            }
            crate::SelectedDeclaration::MachineApplication(root) => {
                close_entry(program, root, &mut generated)
                    .map(|machine| roots.push((root, machine)))
                    .map_err(|reason| ("entry application", reason))
            }
        };
        result.map_err(|(kind, reason)| {
            let detail = match reason {
                Stop::Unserved(reason) => reason.into_owned(),
                Stop::Rejected(reason) => reason,
                Stop::Defect(reason) => reason.to_owned(),
            };
            vec![Diagnostic::error(format!(
                "cannot prepare the selected {kind}: {detail}"
            ))]
        })?;
    }
    let mut captures = crate::source_state::NamedConformanceCaptureIndex::new(program);
    // A generated machine is appended to the machines and read in its turn.
    let mut selected_methods = provider_methods::SelectedMethods::new(program, &selected_providers);
    let mut position = 0;
    loop {
        if position == program.machines().len() {
            selected_methods.prepare_applications(program, &mut generated)?;
            if position == program.machines().len() {
                break;
            }
        }
        if program.machines()[position].body_status
            == crate::source_state::machine::MachineBodyStatus::Deferred
        {
            position += 1;
            continue;
        }
        let holder = program.machines()[position].symbol;
        let conditional = program.machines()[position].purpose
            == crate::source_state::machine::MachinePurpose::ActiveCaseBranch
            || case_branches.contains(&holder);
        let first_child = program.machines().len();
        match reflected_fields::elaborate(program, position) {
            Ok(()) => {}
            Err((Stop::Unserved(detail), span)) => {
                program.record_deferred_refusal_with_detail(
                    holder,
                    DeferredRequest::StaticApplication,
                    span,
                    detail,
                );
            }
            Err((Stop::Rejected(message), span)) if conditional => {
                program.record_deferred_refusal_with_detail(
                    holder,
                    DeferredRequest::InvalidStaticApplication,
                    span,
                    message,
                );
            }
            Err((Stop::Rejected(message), span)) => {
                return Err(vec![Diagnostic::error(message).with_source_span(span)]);
            }
            Err((Stop::Defect(message), _)) => return Err(vec![Diagnostic::error(message)]),
        }
        let mut sites = Vec::new();
        match propositions::close(program, position, &mut generated_propositions) {
            Ok(formulas) => {
                for formula in formulas {
                    sites.extend(clause_sites(program, formula));
                }
            }
            Err((Stop::Unserved(detail), span)) => {
                program.record_deferred_refusal_with_detail(
                    holder,
                    DeferredRequest::StaticApplication,
                    span,
                    detail,
                );
            }
            Err((Stop::Rejected(message), span)) => {
                return Err(vec![Diagnostic::error(message).with_source_span(span)]);
            }
            Err((Stop::Defect(message), _)) => return Err(vec![Diagnostic::error(message)]),
        }
        sites.extend(call_sites(program, position));
        for site in sites {
            let served = named_conformances::serve(program, holder, site, &mut captures)
                .and_then(|handled| {
                    if handled {
                        Ok(())
                    } else {
                        serve(program, site, &mut generated)
                    }
                })
                .and_then(|()| {
                    let Some(call) = site.call(program) else {
                        return Ok(());
                    };
                    let dispatch = match call {
                        StaticArgumentCall::Expression(_, call) => {
                            call.static_requirement_dispatch.clone()
                        }
                        StaticArgumentCall::Statement(call) => {
                            call.static_requirement_dispatch.clone()
                        }
                    };
                    let Some(dispatch) = dispatch else {
                        return Ok(());
                    };
                    let application = call.application();
                    let original = call.registration_operation(program);
                    program
                        .prepare_selected_requirement_application(application, &dispatch, original)
                        .map_err(unserved)
                });
            match served {
                Ok(()) => {}
                Err(Stop::Unserved(detail)) => {
                    let span = site.source_span(program);
                    program.record_deferred_refusal_with_detail(
                        holder,
                        DeferredRequest::StaticApplication,
                        span,
                        detail,
                    );
                }
                Err(Stop::Rejected(message)) if conditional => {
                    program.record_deferred_refusal_with_detail(
                        holder,
                        DeferredRequest::InvalidStaticApplication,
                        site.source_span(program),
                        message,
                    );
                }
                Err(Stop::Rejected(message)) => {
                    return Err(vec![
                        Diagnostic::error(message).with_source_span(site.source_span(program)),
                    ]);
                }
                Err(Stop::Defect(message)) => return Err(vec![Diagnostic::error(message)]),
            }
        }
        if conditional {
            let machines = program.machines();
            case_branches.extend((first_child..machines.len()).map(|index| machines[index].symbol));
        }
        position += 1;
    }
    // Selected callback headers also create substituted types before copying
    // a body. Form their data views along with every other new type from this
    // pass, so later readers see the same exact application and field types.
    data_applications::prepare_from(program, first_type)
        .map_err(|reason| vec![Diagnostic::error(reason)])?;
    Ok(roots)
}

/// Enter the same specialization as an ordinary call without inventing a call
/// site in the authored body. The product later checks the closed entry shape.
fn close_entry(
    program: &mut TypedTrees,
    root: crate::EntryApplication,
    generated: &mut GeneratedMachines,
) -> Served<SymbolHandle> {
    let application = root.in_source(program).ok_or(unserved(
        "the selected static application belongs to another typed source or is missing",
    ))?;
    let row = program
        .expression_table
        .static_machine_application(application);
    if !program
        .symbols
        .symbols()
        .nodes()
        .is_valid(row.lexical_owner)
    {
        return Err(unserved(
            "the selected entry application lost its supplying source scope",
        ));
    }
    let callee = row.registration_operation;
    let machine = program
        .machine_by_symbol(callee)
        .filter(|machine| machine.body_is_present)
        .ok_or(unserved(
            "the selected entry application has no ordinary machine body",
        ))?;
    let parameters = program.machine_callable_parameters(machine);
    let mut arguments = program
        .expression_table
        .static_machine_application_arguments(application)
        .to_vec();
    if parameters.is_empty() || parameters.len() != arguments.len() {
        return Err(unserved(
            "the selected entry application must supply every declared static parameter",
        ));
    }
    specialize_application(
        program,
        callee,
        &parameters,
        application,
        &mut arguments,
        generated,
    )?
    .map(|(machine, _)| machine)
    .ok_or(unserved(
        "the selected entry application must have a closed executable specialization",
    ))
}

// Newer slots sort first so ascending applications need fewer comparisons within each tree node.
#[derive(PartialEq, Eq, PartialOrd, Ord)]
struct ApplicationKey {
    template: (u32, u32),
    // Each selected body implements the contract of one closed public method.
    requirement: Option<(u32, u32)>,
    arguments: Vec<Identity>,
}

struct Generated {
    machine: SymbolHandle,
    state: SymbolHandle,
}

/// A closed application generates its machine on first use. The program records
/// machines in that encounter order. This lookup only answers repeated calls.
#[derive(Default)]
struct GeneratedMachines {
    applications: std::collections::BTreeMap<std::cmp::Reverse<ApplicationKey>, Generated>,
}

impl GeneratedMachines {
    fn get_or_generate(
        &mut self,
        template: SymbolHandle,
        requirement: Option<SymbolHandle>,
        arguments: Vec<Identity>,
        generate: impl FnOnce() -> Served<Generated>,
    ) -> Served<&Generated> {
        use std::collections::btree_map::Entry;
        let key = std::cmp::Reverse(ApplicationKey {
            template: symbol_order(template),
            requirement: requirement.map(symbol_order),
            arguments,
        });
        match self.applications.entry(key) {
            Entry::Occupied(entry) => Ok(entry.into_mut()),
            Entry::Vacant(entry) => Ok(entry.insert(generate()?)),
        }
    }
}

/// A total order on symbols for sorted lookups: by arena slot, then by
/// generation, exactly the parts symbol equality compares.
fn symbol_order(symbol: SymbolHandle) -> (u32, u32) {
    (symbol.arena_index(), symbol.generation())
}

/// A call as the trees hold one: an expression, or a statement that
/// discards the result.
#[derive(Clone, Copy)]
enum Site {
    Expression(ExpressionHandle),
    Statement(StatementHandle),
}

/// What a call applies: its target, its static application, and whether the
/// call is selected some other way, which is not this pass's request: a
/// Build selection, or a requirement dispatch that does not enter the
/// machine realizing the requirement. A dispatched call that does enter it
/// (`Selected::value::<7>(7)` once `Selected` is known) applies that machine
/// like any authored call. Declaration-query ownership is read before
/// specializing.
struct Applied {
    target: SymbolHandle,
    application: StaticMachineApplicationHandle,
    selected_otherwise: bool,
}

impl Site {
    fn call(self, program: &TypedTrees) -> Option<StaticArgumentCall<'_>> {
        match self {
            Self::Expression(handle) => {
                let ExpressionNode::Call(call) = program.expression_table.expression(handle) else {
                    return None;
                };
                Some(StaticArgumentCall::Expression(handle, call))
            }
            Self::Statement(handle) => Some(StaticArgumentCall::Statement(
                program.statement_table.statement(handle).as_call()?,
            )),
        }
    }

    fn applied(self, program: &TypedTrees) -> Option<Applied> {
        let call = self.call(program)?;
        let elsewhere = |dispatch: &Option<StaticRequirementDispatch>, target| {
            dispatch
                .as_ref()
                .is_some_and(|dispatch| dispatch.realization_state != target)
        };
        let (target, selected_otherwise) = match &call {
            StaticArgumentCall::Expression(_, call) => (
                call.target_symbol,
                elsewhere(&call.static_requirement_dispatch, call.target_symbol),
            ),
            StaticArgumentCall::Statement(call) => (
                call.target_symbol,
                elsewhere(&call.static_requirement_dispatch, call.target_symbol),
            ),
        };
        Some(Applied {
            target,
            application: call.application(),
            selected_otherwise,
        })
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

    fn requirement_application(self, program: &TypedTrees) -> TypeReferenceHandle {
        match self.call(program) {
            Some(StaticArgumentCall::Expression(_, call)) => call.requirement_application,
            Some(StaticArgumentCall::Statement(_)) => TypeReferenceHandle::invalid(),
            None => TypeReferenceHandle::invalid(),
        }
    }

    /// Points the call at `target`, the machine its application selects.
    /// The static arguments the call listed are spent: the generated machine
    /// has no parameter left to apply.
    fn enter(self, program: &mut TypedTrees, target: SymbolHandle) {
        match self {
            Self::Expression(handle) => {
                if let ExpressionNode::Call(call) = program.expression_table.expression_mut(handle)
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
        // A statement call inside a block value is a site of its own. The
        // subtree walk below reaches a block's expressions from the
        // statement that holds it.
        for handle in program.body_statement_handles(state.statement_nodes) {
            if program
                .statement_table
                .statement(handle)
                .as_call()
                .is_some()
            {
                sites.push(Site::Statement(handle));
            }
        }
        for handle in state.statement_nodes.handles() {
            let statement = program.statement_table.statement(handle);
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
        // A clause names the closed application of a machine generic over
        // types it is about, as a body does
        // (`ensures |proof|::Identity::<Nat>(count::<u64>(nil), zero)`). A
        // clause applying a machine generic over machines keeps naming the
        // template, which the mathematical signature reads as one term.
        for fact in program
            .machine_contracts(machine)
            .iter()
            .chain(program.state_contracts(state))
            .flat_map(|contract| program.proof_facts.span_or_empty(contract.facts))
        {
            let ProofFact::Expression(root) = fact else {
                continue;
            };
            if !root.is_valid() {
                continue;
            }
            sites.extend(clause_sites(program, *root));
        }
    }
    sites
}

/// The calls under `root`, a clause, that apply a machine generic over
/// types only.
fn clause_sites(program: &TypedTrees, root: ExpressionHandle) -> Vec<Site> {
    let mut sites = Vec::new();
    program.subtree_for_each(root, SubtreeEdges::default(), |handle, node| {
        if matches!(node, ExpressionNode::Call(_)) {
            sites.push(Site::Expression(handle));
        }
    });
    sites.retain(|site| {
        site.applied(program)
            .and_then(|applied| entered_machine(program, applied.target))
            .and_then(|(machine, _)| program.machine_by_symbol(machine))
            .is_some_and(|machine| {
                let parameters = program.machine_type_parameters(machine);
                !parameters.is_empty()
                    && parameters
                        .iter()
                        .all(|parameter| matches!(parameter.kind, TypeParameterKind::Type))
            })
    });
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

fn serve(program: &mut TypedTrees, site: Site, generated: &mut GeneratedMachines) -> Served<()> {
    let Some(applied) = site.applied(program) else {
        return Ok(());
    };
    if !applied.application.is_valid() || applied.selected_otherwise {
        return Ok(());
    }
    if program
        .closed_requirement_application(applied.target)
        .is_some()
    {
        program
            .require_closed_requirement_call_application(applied.target, applied.application)
            .map_err(unserved)?;
        let reference = site.requirement_application(program);
        if reference.is_valid() {
            let owner = program
                .expression_table
                .static_machine_application(applied.application)
                .lexical_owner;
            let scope = program
                .applied_requirement_scope(owner, reference)
                .ok_or(unserved(
                    "closed requirement call lost its trait application",
                ))?;
            if !program.requirement_application_matches_contract_scope(applied.target, &scope) {
                return Err(unserved(
                    "closed requirement call changed its trait application",
                ));
            }
        }
        return Ok(());
    }
    let mut arguments = program
        .expression_table
        .static_machine_application_arguments(applied.application)
        .to_vec();
    if let Some((declaration, _)) = entered_machine(program, applied.target)
        && program
            .machine_by_symbol(declaration)
            .is_some_and(|machine| {
                machine.supply_mode == source_declarations::MachineSupplyMode::Boundary
                    && machine.has_bodyless_signature()
            })
    {
        program
            .prepare_bodyless_callable_application(applied.application, applied.target)
            .map_err(unserved)?;
        return Ok(());
    }
    if program.trait_machine_signature(applied.target).is_some() {
        return apply_requirement(program, site, &applied, &arguments);
    }
    if arguments.is_empty() {
        if let Some((callee, _)) = entered_machine(program, applied.target)
            && let Some(machine) = program.machine_by_symbol(callee)
            && let Some(proof) =
                program
                    .machine_callable_parameters(machine)
                    .iter()
                    .find(|parameter| {
                        matches!(parameter.kind, TypeParameterKind::Proof { .. })
                            && program.symbols.symbols().nodes().is_valid(parameter.symbol)
                            && program.symbols.get(parameter.symbol).parent == machine.symbol
                    })
        {
            return Err(Stop::Rejected(format!(
                "call to `{}` must explicitly supply the proof argument `{}`",
                machine.name.as_str(),
                proof.name.as_str()
            )));
        }
        return Ok(());
    }
    // Only an application of a declared machine with parameters is a
    // request to specialize. Static arguments given to anything else (a
    // type query, a requirement of a trait) belong to whoever reads that
    // call, and a call already entering a generated machine was served.
    let Some((callee, names_state)) = entered_machine(program, applied.target) else {
        return Ok(());
    };
    if program
        .executable_application_template(callee)
        .map_err(Stop::Defect)?
        .is_some()
        && program
            .machine_by_symbol(callee)
            .is_some_and(|machine| program.machine_callable_parameters(machine).is_empty())
    {
        return enter_generated(program, site, &applied, callee, &arguments);
    }
    let Some(machine) = program.machine_by_symbol(callee) else {
        if program
            .evidence_requirement_declaration(applied.target)
            .is_ok_and(|requirement| requirement.is_some())
        {
            return bind_requirement(program, &applied, &arguments);
        }
        return apply_requirement(program, site, &applied, &arguments);
    };
    // Its type and value parameters, then each implementation parameter
    // its bounds declare: the order a call's static arguments are given in.
    let parameters = program.machine_callable_parameters(machine);
    if parameters.is_empty() {
        return Ok(());
    }
    // A declaration query reads its actual type or callable operand; it
    // does not execute this generic declaration's placeholder body. Check
    // that ownership only for requests that would otherwise specialize.
    if let Some(call) = site.call(program)
        && crate::source_state::reflection::construction::declaration_query_argument_role(
            program, call,
        ) == StaticArgumentRole::DeclarationDescription
    {
        return Ok(());
    }
    if parameters.len() != arguments.len() {
        arguments = remaining_arguments(program, callee, &arguments)?;
        program
            .expression_table
            .set_static_machine_application_arguments(applied.application, &arguments);
        match site {
            Site::Expression(handle) => {
                if let ExpressionNode::Call(call) = program.expression_table.expression_mut(handle)
                {
                    call.machine_arguments = arguments.clone().into_boxed_slice();
                }
            }
            Site::Statement(handle) => {
                if let StatementNode::Call(call) = program.statement_table.statement_mut(handle) {
                    call.machine_arguments = arguments.clone().into_boxed_slice();
                }
            }
        }
        if parameters.len() != arguments.len() {
            return Err(unserved(
                "a static call needs one argument for each remaining binder",
            ));
        }
    }
    match specialize_application(
        program,
        callee,
        &parameters,
        applied.application,
        &mut arguments,
        generated,
    )? {
        Some((machine, state)) => site.enter(program, if names_state { state } else { machine }),
        None => match site {
            Site::Expression(handle) => {
                if let ExpressionNode::Call(call) = program.expression_table.expression_mut(handle)
                {
                    call.machine_arguments = arguments.into_boxed_slice();
                }
            }
            Site::Statement(handle) => {
                if let StatementNode::Call(call) = program.statement_table.statement_mut(handle) {
                    call.machine_arguments = arguments.into_boxed_slice();
                }
            }
        },
    }
    Ok(())
}

/// Named callback applications and ordinary calls close through the same
/// binding, contract and executable-entry receipts. No runtime environment is
/// introduced when a callback supplies its own static arguments.
fn specialize_application(
    program: &mut TypedTrees,
    callee: SymbolHandle,
    parameters: &[crate::source_state::data::TypeParameter],
    application: StaticMachineApplicationHandle,
    arguments: &mut [StaticMachineArgument],
    generated: &mut GeneratedMachines,
) -> Served<Option<(SymbolHandle, SymbolHandle)>> {
    let mut open_callback = false;
    for (parameter, argument) in parameters.iter().zip(arguments.iter_mut()) {
        // An instantiated theorem citation, such as `above::<0u8>`, names the
        // machine generated for those arguments, exactly as a callback does.
        // Its contract is then the one checked against the alias; a citation
        // left open here is still refused when stage 05 reads it.
        let closes = match parameter.kind {
            TypeParameterKind::Machine { .. } => true,
            TypeParameterKind::Proof { .. } => {
                argument.const_literal.is_none() && argument.evidence_projection.is_none()
            }
            _ => false,
        };
        if closes
            && argument.application.is_some()
            && !close_machine_argument(program, application, argument, generated)?
            && matches!(parameter.kind, TypeParameterKind::Machine { .. })
        {
            open_callback = true;
        }
    }
    program
        .expression_table
        .set_static_machine_application_arguments(application, arguments);
    let supplying_owner = program
        .expression_table
        .static_machine_application(application)
        .lexical_owner;
    let mut operands = Vec::with_capacity(parameters.len());
    for (parameter, argument) in parameters.iter().zip(arguments.iter()) {
        if matches!(parameter.kind, TypeParameterKind::Proof { .. }) {
            continue;
        }
        operands.push(Operand::read(
            program,
            parameter,
            argument,
            supplying_owner,
        )?);
    }
    Operand::require_nominal_callbacks(program, parameters, &operands, supplying_owner)?;
    // Explicit groups keep the original callable's formals and the caller's
    // supplying scope. Authenticate that relation before copying; absent groups
    // still leave ordinary invocation lifetime inference to the call checker.
    program
        .static_application_lifetimes_for_declaration(application, callee)
        .map_err(unserved)?;
    program
        .expression_table
        .static_machine_application_mut(application)
        .binding_declaration = callee;
    if parameters
        .iter()
        .any(|parameter| matches!(parameter.kind, TypeParameterKind::Proof { .. }))
    {
        // Evidence keeps its declared position and actual citation. It never
        // chooses a generated executable body or enters a specialization key.
        let mut computational = operands.iter();
        let bindings = parameters
            .iter()
            .zip(arguments.iter())
            .map(|(parameter, argument)| {
                let value = if matches!(parameter.kind, TypeParameterKind::Proof { .. }) {
                    StaticMachineBindingValue::Proof(argument.clone())
                } else {
                    computational
                        .next()
                        .expect("every computational parameter was read above")
                        .supplied_value(program, supplying_owner)?
                };
                Ok(
                    crate::source_state::static_applications::StaticMachineBinding {
                        parameter: parameter.symbol,
                        value,
                    },
                )
            })
            .collect::<Served<Vec<_>>>()?;
        program
            .expression_table
            .set_static_machine_application_bindings(application, &bindings);
        return Ok(None);
    }
    bind(program, application, &operands)?;
    if program
        .compiler_primitive_schema(callee)
        .map_err(|_| unserved("compiler primitive has no complete authenticated schema"))?
        .is_some()
    {
        return Ok(None);
    }
    // An argument that names a parameter of the enclosing generic machine
    // leaves the application open: it is closed in each machine generated
    // from that one, where the parameter is replaced.
    // A helper can mention a map binder in its body or signature even
    // when its own arguments are all concrete. Close it in the selected tree.
    let open_map = program
        .enclosing_source_conformance(callee)
        .is_some_and(|map| !map.type_parameters.is_empty() || !map.lifetime_parameters.is_empty());
    if open_map || open_callback || operands.iter().any(|operand| operand.is_open(program)) {
        return Ok(None);
    }
    program
        .prepare_static_callback_requirements(application)
        .map_err(unserved)?;
    let key = operands
        .iter()
        .map(|operand| operand.identity.clone())
        .collect::<Vec<_>>();
    let registration = program
        .expression_table
        .static_machine_application(application)
        .registration_operation;
    let requirement = program
        .closed_requirement_application(registration)
        .map(|closed| closed.signature.symbol);
    let entry = generated.get_or_generate(callee, requirement, key, || {
        generate(program, callee, application, &operands)
    })?;
    Ok(Some((entry.machine, entry.state)))
}

fn close_machine_argument(
    program: &mut TypedTrees,
    supplying: StaticMachineApplicationHandle,
    argument: &mut StaticMachineArgument,
    generated: &mut GeneratedMachines,
) -> Served<bool> {
    let Some(selected) = argument.application.as_ref() else {
        return Ok(true);
    };
    if selected.member.is_valid()
        || !selected.lifetime_arguments.is_empty()
        || argument.evidence_projection.is_some()
        || argument.const_literal.is_some()
        || (argument.type_reference.is_valid()
            && !matches!(program.type_reference_table.type_reference(argument.type_reference),
                TypeReferenceNode::Named { symbol, .. } if *symbol == argument.symbol))
    {
        return Err(unserved(
            "a static callback application needs an exact named machine and its static arguments",
        ));
    }
    let Some((callee, names_state)) = entered_machine(program, argument.symbol) else {
        return Err(unserved(
            "a static callback application has no declared machine",
        ));
    };
    let machine = program.machine_by_symbol(callee).ok_or(unserved(
        "a static callback application has no body-bearing declaration",
    ))?;
    if !machine.lifetime_parameters.is_empty() {
        return Err(unserved(
            "an applied callback needs its exact own lifetime application",
        ));
    }
    let parameters = program.machine_callable_parameters(machine);
    let selected_name = machine.name.clone();
    let mut arguments = selected.arguments.to_vec();
    if parameters.len() != arguments.len() {
        arguments = remaining_arguments(program, callee, &arguments)?;
    }
    if parameters.len() != arguments.len() {
        return Err(unserved(
            "a static callback needs one argument for each declared binder",
        ));
    }
    if parameters.is_empty() {
        if program
            .executable_application_template(callee)
            .map_err(Stop::Defect)?
            .is_none()
        {
            return Err(unserved(
                "a static callback application binds no declared parameter",
            ));
        }
        argument.type_reference = TypeReferenceHandle::invalid();
        argument.application = None;
        argument.authored_machine_application = StaticMachineApplicationHandle::invalid();
        return Ok(true);
    }
    let owner = program
        .expression_table
        .static_machine_application(supplying)
        .lexical_owner;
    let target = argument.path.last().cloned().unwrap_or(selected_name);
    let application = program.expression_table.retain_static_machine_application(
        argument.symbol,
        target,
        &arguments,
    );
    program
        .expression_table
        .static_machine_application_mut(application)
        .lexical_owner = owner;
    let Some((machine, state)) = specialize_application(
        program,
        callee,
        &parameters,
        application,
        &mut arguments,
        generated,
    )?
    else {
        argument.authored_machine_application = application;
        // Nested arguments may themselves have prepared open applications.
        // Keep the exact tuple that produced the retained bindings.
        argument
            .application
            .as_mut()
            .ok_or(Stop::Defect("open callback lost its source application"))?
            .arguments = arguments.into_boxed_slice();
        return Ok(false);
    };
    // Its receipt now retains the complete application. The operand selects
    // that exact executable entry and keeps the original authored occurrence.
    argument.symbol = if names_state { state } else { machine };
    argument.type_reference = TypeReferenceHandle::invalid();
    argument.application = None;
    argument.authored_machine_application = StaticMachineApplicationHandle::invalid();
    Ok(true)
}

/// A copied nested declaration has already received its enclosing arguments.
/// Authenticate those arguments against its actual selection before removing
/// them from this new call. Its independent binders keep their own arguments;
/// the two applications retain the complete context through their receipts.
fn remaining_arguments(
    program: &mut TypedTrees,
    callee: SymbolHandle,
    arguments: &[StaticMachineArgument],
) -> Served<Vec<StaticMachineArgument>> {
    let (template, template_parameters, binding_application) = {
        let mut rows = program.machine_specializations.for_instance(callee);
        match (rows.next(), rows.next()) {
            (Some(row), None) => (
                row.template,
                row.template_parameters,
                row.binding_application,
            ),
            _ => {
                return Err(unserved(
                    "a static call has no unique enclosing application",
                ));
            }
        }
    };
    let original = program.machine_by_symbol(template).ok_or(unserved(
        "a nested application lost its original declaration",
    ))?;
    let original_parameters = program.machine_callable_parameters(original);
    if original_parameters.len() != arguments.len() {
        return Err(unserved(
            "a nested application lost its complete source argument tuple",
        ));
    }
    let application = program
        .expression_table
        .static_machine_application(binding_application);
    let (owner, _) = entered_machine(program, application.registration_operation).ok_or(
        unserved("a nested application lost its enclosing declaration"),
    )?;
    let source = program.machine_by_symbol(owner).ok_or(unserved(
        "a nested application lost its enclosing declaration",
    ))?;
    let captured = program
        .data_type_parameters
        .span(template_parameters)
        .ok_or(unserved(
            "a nested application lost its captured binder roster",
        ))?;
    if !same_declared_binders(
        program.machine_type_parameters(source).iter(),
        captured.iter(),
    ) {
        return Err(unserved(
            "a nested application changed its enclosing binder roster",
        ));
    }
    let enclosing = program.machine_callable_parameters(source);
    let mut positions = original_parameters
        .iter()
        .enumerate()
        .map(|(position, parameter)| (symbol_order(parameter.symbol), position))
        .collect::<Vec<_>>();
    positions.sort_unstable_by_key(|(symbol, _)| *symbol);
    if positions.windows(2).any(|pair| pair[0].0 == pair[1].0) {
        return Err(unserved("a nested application repeats a source binder"));
    }
    let position_for = |symbol| {
        positions
            .binary_search_by_key(&symbol_order(symbol), |(symbol, _)| *symbol)
            .map(|position| positions[position].1)
            .map_err(|_| unserved("a nested application has no argument for its binder"))
    };
    let enclosing_arguments = enclosing
        .iter()
        .map(|parameter| position_for(parameter.symbol).map(|position| arguments[position].clone()))
        .collect::<Served<Vec<_>>>()?;
    let machine = program
        .machine_by_symbol(callee)
        .ok_or(unserved("a nested application lost its copied declaration"))?;
    let remaining = program
        .machine_callable_parameters(machine)
        .iter()
        .map(|parameter| {
            let position = position_for(program.symbols.get(parameter.symbol).generated_from)?;
            let original = &original_parameters[position];
            if parameter.kind != original.kind || parameter.bounds != original.bounds {
                return Err(unserved(
                    "a nested application changed its independent binder's properties",
                ));
            }
            Ok(arguments[position].clone())
        })
        .collect::<Served<Vec<_>>>()?;
    bound_arguments(
        program,
        &enclosing,
        &enclosing,
        binding_application,
        &enclosing_arguments,
    )?;
    Ok(remaining)
}

/// A lexical child has already been copied at its enclosing application's
/// arguments, while typing retained those arguments on the call too. Spend
/// them only when they select that exact existing copy. The source application
/// stays authored; no new specialization or caller fact is manufactured.
fn enter_generated(
    program: &mut TypedTrees,
    site: Site,
    applied: &Applied,
    callee: SymbolHandle,
    arguments: &[StaticMachineArgument],
) -> Served<()> {
    // The receipt table indexes instances; a duplicate receipt stays unserved.
    let row = {
        let mut receipts = program.machine_specializations.for_instance(callee);
        match (receipts.next(), receipts.next()) {
            (Some(row), None) => row,
            _ => {
                return Err(unserved(
                    "a copied callable needs exactly one specialization receipt",
                ));
            }
        }
    };
    let Some(template) = program.machine_by_symbol(row.template) else {
        return Err(unserved(
            "a copied callable receipt has no template machine",
        ));
    };
    let parameters = program.machine_callable_parameters(template);
    let table = &program.expression_table;
    let selection = table.static_machine_application(row.binding_application);
    let Some((owner, _)) = entered_machine(program, selection.registration_operation) else {
        return Err(unserved(
            "a copied callable selection does not enter an enclosing machine",
        ));
    };
    let Some(source) = program.machine_by_symbol(owner) else {
        return Err(unserved(
            "a copied callable selection has no enclosing machine declaration",
        ));
    };
    // The selection belongs to the original enclosing declaration, not an
    // unrelated application with equal rendered arguments.
    let captured = program
        .data_type_parameters
        .span(row.template_parameters)
        .ok_or(unserved(
            "a copied application lost its captured binder roster",
        ))?;
    if !same_declared_binders(
        program.machine_type_parameters(source).iter(),
        captured.iter(),
    ) || program.symbols.get(callee).generated_from != row.template
    {
        return Err(unserved(
            "a copied callable changed its captured binders or template origin",
        ));
    }
    let mut lexical = row.template;
    while lexical.is_valid() && lexical != owner {
        lexical = program.symbols.get(lexical).parent;
    }
    if lexical != owner {
        return Err(unserved(
            "a copied callable template is outside its selected lexical owner",
        ));
    }
    let source_parameters = program.machine_callable_parameters(source);
    let operands = bound_arguments(
        program,
        &parameters,
        &source_parameters,
        row.binding_application,
        arguments,
    )?;
    bind(program, applied.application, &operands)?;
    site.enter(program, applied.target);
    Ok(())
}

/// Capturing a binder preserves its declaration. Copying carrier and contract
/// type references does not create another binder or another application.
fn same_declared_binders<'a>(
    left: impl Iterator<Item = &'a crate::source_state::data::TypeParameter>,
    right: impl Iterator<Item = &'a crate::source_state::data::TypeParameter>,
) -> bool {
    left.map(|parameter| parameter.symbol)
        .eq(right.map(|parameter| parameter.symbol))
}

/// Compare a requested tuple with the complete original selection, including
/// unused binders. This only acknowledges an already supplied application;
/// the caller separately authenticates its source association.
fn bound_arguments(
    program: &mut TypedTrees,
    parameters: &[crate::source_state::data::TypeParameter],
    source_parameters: &[crate::source_state::data::TypeParameter],
    selection: StaticMachineApplicationHandle,
    arguments: &[StaticMachineArgument],
) -> Served<Vec<Operand>> {
    // A copied signature keeps these binder declarations even when its
    // type-reference handles differ. The operand reads below still check the
    // supplied and retained arguments under each receiving binder's kind.
    if parameters.len() != arguments.len()
        || !same_declared_binders(parameters.iter(), source_parameters.iter())
    {
        return Err(unserved(
            "a captured static argument tuple does not match its declared binders",
        ));
    }
    let table = &program.expression_table;
    let actual_arguments = table.static_machine_application_arguments(selection);
    let bindings = table.static_machine_application_bindings(selection);
    if actual_arguments.len() != bindings.len() || bindings.len() != source_parameters.len() {
        return Err(unserved(
            "a captured selection has incomplete argument bindings",
        ));
    }
    let mut selected = bindings
        .iter()
        .zip(actual_arguments)
        .map(|(binding, argument)| (symbol_order(binding.parameter), argument.clone()))
        .collect::<Vec<_>>();
    selected.sort_unstable_by_key(|(symbol, _)| *symbol);
    if selected.windows(2).any(|pair| pair[0].0 == pair[1].0) {
        return Err(unserved("a captured selection repeats a static binder"));
    }
    let supplying_owner = program
        .expression_table
        .static_machine_application(selection)
        .lexical_owner;
    let mut operands = Vec::with_capacity(parameters.len());
    for (parameter, argument) in parameters.iter().zip(arguments) {
        let position = selected
            .binary_search_by_key(&symbol_order(parameter.symbol), |(symbol, _)| *symbol)
            .map_err(|_| {
                unserved("a captured selection has no argument for the requested binder")
            })?;
        let actual = &selected[position].1;
        let operand = Operand::read(program, parameter, argument, supplying_owner)?;
        let existing = Operand::read(program, parameter, actual, supplying_owner)?;
        if operand.is_open(program) || operand.identity != existing.identity {
            return Err(unserved(
                "a captured static argument is open or differs from its selected value",
            ));
        }
        operands.push(operand);
    }
    Operand::require_nominal_callbacks(program, parameters, &operands, supplying_owner)?;
    Ok(operands)
}

/// A call through an implementation parameter to a requirement that takes
/// value parameters, `Implementation::value::<7>(7)`: nothing is generated for
/// it, because the machine it enters is known only once the implementation
/// is. Its arguments are bound to the requirement's parameters, so the
/// requirement's contract can be read at them, and the call is served as an
/// application when a generated machine points it at a realizing machine.
/// A requirement with any other kind of parameter is left as typing wrote
/// it.
fn bind_requirement(
    program: &mut TypedTrees,
    applied: &Applied,
    arguments: &[StaticMachineArgument],
) -> Served<()> {
    let Ok(Some(requirement)) = program.evidence_requirement_declaration(applied.target) else {
        return Ok(());
    };
    let declaration = requirement.symbol;
    let parameters = program
        .data_type_parameters
        .span_or_empty(requirement.type_parameters)
        .iter()
        .cloned()
        .collect::<Vec<_>>();
    if !parameters.iter().all(|parameter| {
        matches!(
            parameter.kind,
            crate::source_state::data::TypeParameterKind::Const { .. }
                | crate::source_state::data::TypeParameterKind::Value { .. }
        )
    }) {
        return Ok(());
    }
    if parameters.len() != arguments.len() {
        return Err(unserved(
            "a requirement call needs one argument for each declared binder",
        ));
    }
    let supplying_owner = program
        .expression_table
        .static_machine_application(applied.application)
        .lexical_owner;
    let mut operands = Vec::with_capacity(parameters.len());
    for (parameter, argument) in parameters.iter().zip(arguments) {
        operands.push(Operand::read(
            program,
            parameter,
            argument,
            supplying_owner,
        )?);
    }
    program
        .expression_table
        .static_machine_application_mut(applied.application)
        .binding_declaration = declaration;
    bind(program, applied.application, &operands)?;
    Ok(())
}

/// A call that names a trait's requirement and applies the requirement's own
/// parameters, `Probe::take::<Reading>(reading)`. A requirement has no body to
/// copy, so what the application selects is the requirement's signature and
/// contract with each parameter replaced: one closed requirement per
/// application, which the call then names. generics.md, "Closed applications
/// and canonical identity": two calls at the same arguments name the same
/// closed requirement (`TypedTrees::instantiate_requirement_application`
/// keys it by the application's identity).
///
/// An explicit trait application or a named implementation supplies the
/// declaring trait's arguments. Requirements of a trait with no parameters
/// can use its declared scope. Each supplied operand is read by [`Operand::read`].
fn apply_requirement(
    program: &mut TypedTrees,
    site: Site,
    applied: &Applied,
    arguments: &[StaticMachineArgument],
) -> Served<()> {
    let reference = site.requirement_application(program);
    let receiver = match site.call(program) {
        Some(StaticArgumentCall::Expression(_, call)) => program
            .expression_table
            .name_path_node(call.receiver)
            .map(|name| name.symbol),
        Some(StaticArgumentCall::Statement(call)) => Some(call.receiver_symbol),
        None => None,
    };
    let lexical_owner = program
        .expression_table
        .static_machine_application(applied.application)
        .lexical_owner;
    let environment = if reference.is_valid() {
        Some(
            program
                .applied_requirement_scope(lexical_owner, reference)
                .ok_or(unserved("requirement call lost its applied trait scope"))?
                .environment(),
        )
    } else if let Some(receiver) = receiver
        && let Some(member) = program
            .static_record_bound_requirement(lexical_owner, receiver, applied.target)
            .map_err(|_| unserved("static record call lost its declared requirement application"))?
    {
        Some(member.application.environment())
    } else if program
        .trait_machine_signature(applied.target)
        .is_some_and(|(owner, _)| {
            !owner.type_parameters.is_empty() || !owner.lifetime_parameters.is_empty()
        })
    {
        // A named, closed implementation supplies its trait arguments
        // even when the call does not write a type application.
        let member = match site.call(program) {
            Some(StaticArgumentCall::Expression(_, call)) => {
                program.named_conformance_call_member(call)
            }
            Some(StaticArgumentCall::Statement(call))
                if call.static_requirement_dispatch.is_none() =>
            {
                program.named_conformance_member(call.receiver_symbol, applied.target)
            }
            _ => Ok(None),
        }
        .map_err(unserved)?;
        member
            .map(|(conformance, row, _)| {
                crate::source_state::trait_definition::AppliedTraitScope::from_conformance(
                    program,
                    conformance,
                )
                .and_then(|scope| scope.declaring_scope(program, row.declaring_trait))
                .map(|scope| scope.environment())
                .ok_or(unserved(
                    "selected requirement lost its declaring trait application",
                ))
            })
            .transpose()?
    } else {
        None
    };
    let selected = if let Some(environment) = environment {
        static_operand_binding::prepare_requirement_in_scope(
            program,
            &environment,
            applied.target,
            applied.application,
            arguments,
        )?
    } else {
        static_operand_binding::prepare_requirement_application(
            program,
            applied.target,
            applied.application,
            arguments,
        )?
    };
    if let Some(closed) = selected {
        site.enter(program, closed);
    }
    Ok(())
}

/// One requirement of an implementation parameter as a generated machine
/// calls it: the symbol a call through the parameter targets in the generic
/// body, and the selected operation that call enters instead.
struct Selection {
    placeholder: SymbolHandle,
    dispatch: StaticRequirementDispatch,
}

/// The operation each implementation operand selects for every requirement
/// its parameter offers. A requirement the implementation gives no row for,
/// or one with parameters of its own other than value parameters, leaves the
/// application unserved.
fn selections(program: &TypedTrees, operands: &[Operand]) -> Served<Vec<Selection>> {
    let mut selected = Vec::new();
    for operand in operands {
        if !matches!(operand.value, StaticMachineBindingValue::Conformance(_)) {
            continue;
        }
        let Some(implementation) = operand
            .declaration()
            .and_then(|declaration| program.conformance_by_symbol(declaration))
        else {
            return Err(unserved(
                "a selected implementation operand has no conformance declaration",
            ));
        };
        let placeholders = program
            .symbols
            .child_handles(operand.parameter)
            .ok_or(unserved(
                "an implementation binder has no requirement placeholders",
            ))?;
        for placeholder in placeholders {
            let Some(requirement) = program
                .evidence_requirement_declaration(placeholder)
                .map_err(unserved)?
            else {
                return Err(unserved(
                    "an implementation placeholder has no exact requirement declaration",
                ));
            };
            // A requirement's own value parameters are bound at each call
            // through it (`bind_requirement`); any other parameter of its
            // own is not carried.
            let only_values = program
                .data_type_parameters
                .span_or_empty(requirement.type_parameters)
                .iter()
                .all(|parameter| {
                    matches!(
                        parameter.kind,
                        crate::source_state::data::TypeParameterKind::Const { .. }
                            | crate::source_state::data::TypeParameterKind::Value { .. }
                    )
                });
            if !only_values
                || !requirement.lifetime_parameters.is_empty()
                || !requirement.conformance_bounds.is_empty()
            {
                return Err(unserved(
                    "an implementation requirement needs only value parameters without lifetimes or conformance bounds",
                ));
            }
            let mut rows = implementation
                .rows
                .iter()
                .filter(|row| row.requirement == requirement.symbol);
            let (Some(row), None) = (rows.next(), rows.next()) else {
                return Err(unserved(
                    "an implementation needs exactly one row for each selected requirement",
                ));
            };
            if !row.realization_machine.is_valid() || !row.realization_state.is_valid() {
                return Err(unserved(
                    "a selected implementation row has no realization machine or state",
                ));
            }
            selected.push(Selection {
                placeholder,
                dispatch: StaticRequirementDispatch {
                    evidence_parameter: operand.parameter,
                    declaring_trait: row.declaring_trait,
                    requirement: row.requirement,
                    realization_machine: row.realization_machine,
                    realization_state: row.realization_state,
                },
            });
        }
    }
    Ok(selected)
}

/// A declaration tree is copied as one unit: every child keeps its lexical
/// owner, and calls between siblings resolve only after all fresh symbols
/// exist. Runtime values remain explicit parameters; this supplies only the
/// enclosing declaration's already selected static context.
fn template(program: &TypedTrees, symbol: SymbolHandle) -> Served<Vec<(Machine, State)>> {
    let root = program
        .machine_by_symbol(symbol)
        .ok_or_else(|| unserved("a declaration template has no exact machine"))?;
    // Cloning a signature can give a captured binder's carrier or contract
    // fresh type-reference handles. Its declaration symbol still identifies
    // the same enclosing binder, whose application was checked at the owner.
    let mut parameters = program
        .machine_callable_parameters(root)
        .into_iter()
        .map(|parameter| parameter.symbol)
        .collect::<Vec<_>>();
    parameters.sort_unstable_by_key(|symbol| symbol_order(*symbol));
    if parameters.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(unserved("a declaration template repeats a callable binder"));
    }
    let mut pending = vec![symbol];
    let mut parts = Vec::new();
    let mut position = 0;
    while position < pending.len() {
        let declaration = pending[position];
        let (machine, state, types) = copied_parts(program, declaration)?;
        // Enclosing binders use this application. Independent type binders
        // remain on the copied child and receive their own later application.
        // Other independent binder kinds need their full interface copied.
        if position != 0
            && program
                .machine_callable_parameters(&machine)
                .iter()
                .any(|parameter| {
                    parameters
                        .binary_search_by_key(&symbol_order(parameter.symbol), |symbol| {
                            symbol_order(*symbol)
                        })
                        .is_err()
                        && parameter.kind != TypeParameterKind::Type
                })
        {
            return Err(unserved(
                "a nested declaration has static binders different from its enclosing template",
            ));
        }
        if has_unserved_type_expression(program, &types) {
            return Err(unserved(
                "a declaration template has an embedded type expression without a retained static operand",
            ));
        }
        for owner in [machine.symbol, state.symbol] {
            for child in program
                .symbols
                .child_handles(owner)
                .ok_or_else(|| unserved("a declaration template lost its owned child roster"))?
            {
                if program.symbols.get(child).kind == SymbolKind::Machine {
                    pending.push(child);
                }
            }
        }
        parts.push((machine, state));
        position += 1;
    }
    Ok(parts)
}

/// The machine and its state, with every type the copy will have to copy:
/// the signature's and the ones the contract and body expressions hold.
fn copied_parts(
    program: &TypedTrees,
    symbol: SymbolHandle,
) -> Served<(Machine, State, Vec<TypeReferenceHandle>)> {
    let Some(machine) = program.machine_by_symbol(symbol) else {
        return Err(unserved("a copied declaration has no machine"));
    };
    let Some([state]) = program.machine_states(machine).as_array() else {
        return Err(unserved("a copied declaration needs exactly one entry"));
    };
    let plan = &machine.termination_plan;
    // Lifetimes remain binders of each copied declaration. The shared type
    // copier preserves their supplying owners, and the specialization receipt
    // identifies the original declaration; invocation regions do not select a
    // separate executable body.
    let plain = machine.body_is_present
        && machine.supply_mode == source_declarations::MachineSupplyMode::CheckedBody
        && machine.owned_data.is_empty()
        && machine.satisfies.is_empty()
        && program.signature_invokes.span(machine.invokes).is_some()
        // The written prefix and operator spelling are diagnostic labels on
        // this same callable. Actual data attachment still needs its copier.
        && !machine.attached_data_symbol.is_valid()
        && !machine.attached_data_application.is_valid()
        && !machine.generic_data_template.is_valid()
        && machine.target.is_none()
        && !machine.target_family_interface
        && !machine.structural_type_equations_pending
        && !machine.service_reach_is_installation_bound
        && plan.checked_summary == source_declarations::TerminationGuarantee::NoGuarantee
        && program.authored_service_reach_rows_for(symbol).all(|row| {
            !row.installation_bound
                && row
                    .targets
                    .iter()
                    .all(|target| target.service.symbol(program).is_some_and(|symbol| program.symbols.symbols().nodes().is_valid(symbol)))
        })
        && !program
            .proof_output_calls
            .iter()
            .any(|call| call.machine_symbol == symbol)
        && !program
            .evidence_forwardings
            .iter()
            .any(|forwarding| forwarding.machine_symbol == symbol);
    if !plain {
        return Err(unserved(
            "the declaration copier does not yet retain this machine's complete interface",
        ));
    }
    let facts = program
        .machine_contracts(machine)
        .iter()
        .chain(program.state_contracts(state))
        .flat_map(|contract| program.proof_facts.span_or_empty(contract.facts));
    let mut roots = Vec::new();
    let mut fact_types = program
        .declared_type_equations
        .span_or_empty(machine.declared_type_equations)
        .iter()
        .flat_map(|equation| [equation.parameter, equation.definition])
        .collect::<Vec<_>>();
    match (
        &plan.implementation_witness,
        program.ranking_expression_custody_for(symbol),
    ) {
        (None, None) => {}
        (Some(witness), Some(custody))
            if witness.subjects.len() == custody.subjects.len()
                && witness.view_arguments.len() == custody.view_arguments.len()
                && witness.rank_range.is_some() == custody.rank_range.is_some() =>
        {
            roots.extend(custody.subjects.iter().copied());
            roots.extend(custody.view_arguments.iter().copied());
            roots.extend(custody.rank_range);
        }
        _ => {
            return Err(unserved(
                "a copied termination witness needs its original expression custody",
            ));
        }
    }
    for fact in facts {
        match fact {
            ProofFact::Expression(expression) => roots.push(*expression),
            ProofFact::Membership(membership) => {
                roots.push(membership.value);
                fact_types.extend_from_slice(
                    program
                        .type_reference_table
                        .type_reference_handles(membership.domain_arguments),
                );
            }
            ProofFact::Proposition(application) => roots.extend_from_slice(
                program
                    .expression_table
                    .expression_handles(application.arguments),
            ),
        }
    }
    for handle in state.statement_nodes.handles() {
        let statement = program.statement_table.statement(handle);
        let copied = match statement {
            StatementNode::Assembly(_) | StatementNode::AssemblyFact(_) => false,
            StatementNode::Call(call) => {
                call.installed_interrupt_operation.is_none()
                    && call.reflected_field_origin.is_none()
            }
            StatementNode::LocalData(local) => local.reflected_query_origin.is_none(),
            StatementNode::Assignment(_)
            | StatementNode::Expression(_)
            | StatementNode::Evaluate { .. }
            | StatementNode::Transition(_) => true,
        };
        if !copied {
            return Err(unserved(
                "a copied statement has unserved retained metadata",
            ));
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
    types.extend(fact_types);
    for root in roots.into_iter().filter(|root| root.is_valid()) {
        types.extend(
            crate::source_state::references::collect_expression_references(program, root)
                .type_references,
        );
        program.subtree_for_each(root, SubtreeEdges::default(), |_handle, node| {
            copied &= match node {
                ExpressionNode::Call(call) => {
                    // The shared expression copier retains a layout query's
                    // operand and view; callable application substitutes their
                    // types with every other copied expression.
                    call.native_code_layout.is_none()
                        && call.quotient_operation.is_none()
                        && call.private_layout_operation.is_none()
                        && call.installed_interrupt_operation.is_none()
                }
                ExpressionNode::Cast(cast) => cast.semantic_domain_arguments.is_empty(),
                _ => true,
            };
        });
    }
    if !copied {
        return Err(unserved(
            "a copied expression has unserved retained metadata",
        ));
    }
    Ok((machine.clone(), state.clone(), types))
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

/// Canonical value operands are carried by the shared type copier, not
/// evaluated again here. Tagged contract/callee graphs keep their copied
/// owners and ordinary checks; only constant type roots need this evaluator.
fn has_unserved_type_expression(program: &TypedTrees, references: &[TypeReferenceHandle]) -> bool {
    let mut staging = TypedTreeTables::default();
    let mut embedded = Vec::new();
    for reference in references.iter().filter(|reference| reference.is_valid()) {
        staging.copy_type_reference_with_expressions(&program.tables, *reference, &mut embedded);
    }
    embedded.iter().any(|copied| {
        !copied.contract_expression
            && program
                .static_operand_from_type_reference(copied.source)
                .is_err()
    })
}

/// Structural result of the one declaration copier. This mapping names
/// actual fresh declarations; it grants no conformance or execution receipt.
struct CopiedDeclarations {
    machine: SymbolHandle,
    state: SymbolHandle,
    symbols: PreparedSymbolRebinding,
    originals: Vec<(Machine, State)>,
}

/// A copied body must prove the same progress promise using its own formals.
/// Copy the original witness expressions with the body; rendered witness names
/// cannot identify the fresh parameters, and no checked summary is inherited.
fn copy_progress(
    program: &mut TypedTrees,
    source: &Machine,
    machine: &mut Machine,
    symbols: SymbolRebinding<'_>,
) -> Served<()> {
    if let source_declarations::TerminationInterface::Published(
        source_declarations::TerminationGuarantee::Terminates { premises },
    ) = &mut machine.termination_plan.interface
    {
        for premise in premises {
            premise.subject.root = symbols.lookup(premise.subject.root).ok_or(Stop::Defect(
                "a copied progress premise lost its original parameter",
            ))?;
            for field in &mut premise.subject.projections {
                *field = symbols.lookup(*field).unwrap_or(*field);
            }
        }
    }
    if let Some(mut custody) = program
        .ranking_expression_custody_for(source.symbol)
        .cloned()
    {
        custody.machine = machine.symbol;
        for expression in custody
            .subjects
            .iter_mut()
            .chain(&mut custody.view_arguments)
            .chain(custody.rank_range.iter_mut())
        {
            *expression = copy_expression(program, *expression, symbols);
        }
        program.ranking_expression_custody.push(custody);
    }
    Ok(())
}

/// Copies one validated declaration tree at the supplied static bindings.
/// The caller retains its actual application/selection association separately.
fn copy_declarations(
    program: &mut TypedTrees,
    template_symbol: SymbolHandle,
    operands: &[Operand],
) -> Served<CopiedDeclarations> {
    let parts = template(program, template_symbol)?;
    let mut bound_parameters = operands
        .iter()
        .map(|operand| symbol_order(operand.parameter))
        .collect::<Vec<_>>();
    bound_parameters.sort_unstable();
    let is_bound = |symbol| {
        bound_parameters
            .binary_search(&symbol_order(symbol))
            .is_ok()
    };
    let selections = selections(program, operands)?;
    // Validate and stage the whole tree before adding any generated symbols.
    let mut staging = TypedTreeTables::default();
    let mut embedded = Vec::new();
    let mut staged = Vec::with_capacity(parts.len());
    for (_, state) in &parts {
        staged.push(staging.copy_statement_nodes_with_type_expressions(
            &program.tables,
            state.statement_nodes,
            &mut embedded,
        ));
    }
    if embedded.iter().any(|copied| {
        !copied.contract_expression
            && program
                .static_operand_from_type_reference(copied.source)
                .is_err()
    }) {
        return Err(unserved(
            "a copied body has an embedded type expression without a retained static operand",
        ));
    }
    // Type-row iteration takes a storage offset; expression iteration takes
    // an arena identity. Keep the first copied type in the prepared range.
    let type_start = program.type_reference_table.type_reference_count();
    let expression_start = program.expression_table.expression_count() + 1;
    let spelled = operands
        .iter()
        .map(|operand| operand.spelled(program))
        .collect::<Vec<_>>()
        .join("$");
    let root_name = format!(
        "{}$specialized${spelled}${}",
        parts[0].0.name.as_str(),
        program.symbols.symbols().len() + 1
    );
    let machine_symbol = program.symbols.insert_generated_root_from(
        template_symbol,
        SymbolKind::Machine,
        &root_name,
    );
    let mut renamed = PreparedSymbolRebinding::default();
    record_symbol(&mut renamed, template_symbol, machine_symbol, false)?;
    // Allocate the complete declaration graph before rebinding any body.
    // Forward sibling calls and recursive calls then use the same exact map.
    for (source, source_state) in &parts {
        let owner = renamed.execution_symbol(source.symbol).ok_or(Stop::Defect(
            "a copied nested declaration lost its lexical owner",
        ))?;
        let nested = |parent| {
            program
                .symbols
                .child_handles(parent)
                .into_iter()
                .flatten()
                .filter(|symbol| program.symbols.get(*symbol).kind == SymbolKind::Machine)
                .map(|symbol| {
                    (
                        symbol,
                        program
                            .machine_by_symbol(symbol)
                            .expect("preflighted child")
                            .name
                            .clone(),
                    )
                })
                .collect::<Vec<_>>()
        };
        let machine_children = nested(source.symbol);
        let state_children = nested(source_state.symbol);
        let type_children = program
            .machine_type_parameters(source)
            .iter()
            .filter(|parameter| {
                program.symbols.get(parameter.symbol).parent == source.symbol
                    && !is_bound(parameter.symbol)
            })
            .cloned()
            .collect::<Vec<_>>();
        // Hierarchy arenas allocate one complete child roster per owner.
        // Include nested declarations with the state/formals rather than
        // appending another range after those original children exist.
        let children = program.symbols.insert_generated_children_from(
            owner,
            std::iter::once((
                source_state.symbol,
                SymbolKind::State,
                source_state.name.as_str(),
            ))
            .chain(
                machine_children
                    .iter()
                    .map(|(symbol, name)| (*symbol, SymbolKind::Machine, name.as_str())),
            )
            .chain(type_children.iter().map(|parameter| {
                (
                    parameter.symbol,
                    SymbolKind::TypeParameter,
                    parameter.name.as_str(),
                )
            })),
        );
        let state = children.handle_at(0);
        record_symbol(&mut renamed, source_state.symbol, state, false)?;
        for (original, fresh) in machine_children
            .iter()
            .map(|(symbol, _)| *symbol)
            .zip(children.handles().skip(1))
        {
            record_symbol(&mut renamed, original, fresh, false)?;
        }
        for (parameter, fresh) in type_children
            .iter()
            .zip(children.handles().skip(1 + machine_children.len()))
        {
            record_symbol(&mut renamed, parameter.symbol, fresh, true)?;
        }
        let parameters = program.state_parameters(source_state).to_vec();
        let locals = program
            .body_statement_spans(source_state.statement_nodes)
            .into_iter()
            .flat_map(|span| program.statement_table.statements(span))
            .filter_map(|statement| statement.as_local_data())
            .map(|local| (local.symbol, local.name.clone()))
            .filter(|(symbol, _)| symbol.is_valid())
            .collect::<Vec<_>>();
        let children = program.symbols.insert_generated_children_from(
            state,
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
                )
                .chain(
                    state_children
                        .iter()
                        .map(|(symbol, name)| (*symbol, SymbolKind::Machine, name.as_str())),
                ),
        );
        let local_count = parameters.len() + locals.len();
        for (original, fresh) in parameters
            .iter()
            .map(|parameter| parameter.symbol)
            .chain(locals.iter().map(|(symbol, _)| *symbol))
            .zip(children.handles())
        {
            record_symbol(&mut renamed, original, fresh, true)?;
        }
        for (original, fresh) in state_children
            .iter()
            .map(|(symbol, _)| *symbol)
            .zip(children.handles().skip(local_count))
        {
            record_symbol(&mut renamed, original, fresh, false)?;
        }
    }
    let state_symbol = copied_symbol(&renamed, parts[0].1.symbol)?;
    let symbols = renamed.rebinding();
    let contract_substitutions = operands
        .iter()
        .filter_map(|operand| match operand.value {
            StaticMachineBindingValue::Type(actual) | StaticMachineBindingValue::Const(actual) => {
                Some((operand.parameter, actual))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    let mut copied = Vec::with_capacity(parts.len());
    for ((source, source_state), staged) in parts.iter().zip(staged) {
        let first_expression = program.expression_table.expression_count() + 1;
        let mut machine = source.clone();
        machine.symbol = copied_symbol(&renamed, source.symbol)?;
        if source.symbol == template_symbol {
            machine.name = Identifier::generated(root_name.clone());
        }
        machine.is_public = false;
        copy_progress(program, source, &mut machine, symbols)?;
        let remaining = program
            .machine_type_parameters(source)
            .iter()
            .filter(|parameter| !is_bound(parameter.symbol))
            .cloned()
            .map(|mut parameter| {
                parameter.symbol = copied_symbol(&renamed, parameter.symbol)?;
                Ok(parameter)
            })
            .collect::<Served<Vec<_>>>()?;
        machine.type_parameters = program.tables.data_type_parameters.insert_many(remaining);
        machine.conformance_bounds.clear();
        machine.contracts = HandleSpan::empty();
        machine.states = HandleSpan::empty();
        for contract in program.machine_contracts(source).to_vec() {
            let contract = program
                .copy_callable_contract(&contract, symbols, &contract_substitutions, source.symbol)
                .map_err(unserved)?;
            program.push_machine_contract(&mut machine, contract);
        }
        let mut state = source_state.clone();
        state.symbol = copied_symbol(&renamed, source_state.symbol)?;
        state.parameters = HandleSpan::empty();
        state.contracts = HandleSpan::empty();
        state.statement_nodes = program.tables.copy_statement_nodes_with_type_expressions(
            &staging,
            staged,
            &mut embedded,
        );
        program
            .tables
            .remap_statement_symbols(state.statement_nodes, symbols);
        for mut parameter in program.state_parameters(source_state).to_vec() {
            parameter.symbol = copied_symbol(&renamed, parameter.symbol)?;
            program.push_state_parameter(&mut state, parameter);
        }
        for contract in program.state_contracts(source_state).to_vec() {
            let contract = program
                .copy_callable_contract(&contract, symbols, &contract_substitutions, source.symbol)
                .map_err(unserved)?;
            program.push_state_contract(&mut state, contract);
        }
        let expression_count = program.expression_table.expression_count() + 1 - first_expression;
        let body = CopiedCallableBody {
            first_expression,
            expression_count,
            statements: program
                .body_statement_handles(state.statement_nodes)
                .collect(),
        };
        copied.push((machine, state, body));
    }
    let substitutions = operands
        .iter()
        .filter_map(|operand| match operand.value {
            StaticMachineBindingValue::Type(actual) | StaticMachineBindingValue::Const(actual) => {
                Some((operand.parameter, actual))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    let record_receivers = copied
        .iter()
        .flat_map(|(_, _, body)| copied_record_receivers(program, body, operands))
        .collect::<Vec<_>>();
    let entries = copied
        .iter_mut()
        .zip(&parts)
        .map(|((machine, state, body), (source, _))| (source.symbol, machine, state, &*body));
    program
        .apply_copied_callable_interface(entries, symbols, &substitutions)
        .map_err(Stop::Defect)?;
    substitute_record_receivers(program, &record_receivers)?;
    substitute_widths(program, type_start, operands)?;
    substitute_values(program, expression_start, operands);
    settle_const_expressions(program, type_start, operands)?;
    for (mut machine, state, body) in copied {
        select_requirements(program, &body, &selections);
        substitute_arguments(program, &body, machine.symbol, operands)?;
        program.push_machine_state(&mut machine, state);
        program.push_machine(machine);
    }
    // Copied references are new receiving occurrences, even when their
    // canonical identity equals an existing application. Retain their own
    // substituted data views before a later reader asks about their fields or
    // type identity. Open child applications keep their unresolved binders.
    data_applications::prepare_from(program, type_start).map_err(Stop::Rejected)?;
    Ok(CopiedDeclarations {
        machine: machine_symbol,
        state: state_symbol,
        symbols: renamed,
        originals: parts,
    })
}

/// Retains the genuine enclosing application for every declaration copied
/// from it. Other structural callers use the same copier and retain their
/// own source associations, without manufacturing this specialization row.
fn generate(
    program: &mut TypedTrees,
    template_symbol: SymbolHandle,
    application: StaticMachineApplicationHandle,
    operands: &[Operand],
) -> Served<Generated> {
    let bound = operands
        .iter()
        .filter_map(|operand| match operand.value {
            StaticMachineBindingValue::Type(reference) => {
                Some((operand.parameter, callable_binder::Bound::Type(reference)))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    for operand in operands {
        let StaticMachineBindingValue::Machine(argument) = &operand.value else {
            continue;
        };
        let binder = program
            .retained_static_machine_contract(operand.parameter)
            .ok_or(Stop::Defect("a callable operand lost its published binder"))?
            .signature();
        let machine = program
            .machine_by_symbol(argument.symbol)
            .or_else(|| {
                program
                    .unique_machine_by_entry_state_symbol(argument.symbol)
                    .map(|(machine, _)| machine)
            })
            .ok_or(unserved("a static callable argument has no machine entry"))?;
        callable_binder::binder_shape(
            program,
            binder,
            machine,
            &format!(
                "`{}::{}`",
                program.symbols.name(template_symbol),
                program.symbols.name(operand.parameter)
            ),
            &bound,
            None,
        )?;
    }
    let copied = copy_declarations(program, template_symbol, operands)?;
    // Each nested declaration is specialized by the actual enclosing
    // application's bindings, not by an invented call or runtime capture.
    for (source, state) in &copied.originals {
        // The application consumes the same declaring binders, but a child's
        // copied signature owns its carrier references. Retain that exact
        // prefix so receiving source correspondence can check it strictly.
        let bound_count = copied.originals[0].0.type_parameters.count();
        if source.type_parameters.count() < bound_count {
            return Err(Stop::Defect("a copied child lost its enclosing binders"));
        }
        let template_parameters =
            HandleSpan::from_parts_or_empty(source.type_parameters.start(), bound_count);
        let of_kind = |implementation: bool| {
            operands.iter().filter(move |operand| {
                matches!(operand.value, StaticMachineBindingValue::Conformance(_)) == implementation
            })
        };
        program.machine_specializations.push(MachineSpecialization {
            template: source.symbol,
            instance: copied_symbol(&copied.symbols, source.symbol)?,
            template_parameters,
            binding_application: application,
            states: program.captured_machine_states.insert_many([
                crate::source_state::CapturedMachineState {
                    template: state.symbol,
                    instance: copied_symbol(&copied.symbols, state.symbol)?,
                },
            ]),
            conformance_arguments: of_kind(true).filter_map(Operand::declaration).collect(),
            ..MachineSpecialization::default()
        });
    }
    Ok(Generated {
        machine: copied.machine,
        state: copied.state,
    })
}

fn record_symbol(
    symbols: &mut PreparedSymbolRebinding,
    source: SymbolHandle,
    fresh: SymbolHandle,
    authored: bool,
) -> Served<()> {
    if symbols.insert_execution(source, fresh).is_some()
        || (authored && symbols.insert_authored(source, fresh).is_some())
    {
        return Err(Stop::Defect(
            "a copied declaration has repeated source coordinates",
        ));
    }
    Ok(())
}

fn copied_symbol(symbols: &PreparedSymbolRebinding, source: SymbolHandle) -> Served<SymbolHandle> {
    symbols
        .execution_symbol(source)
        .ok_or(Stop::Defect("a copied declaration lost its fresh symbol"))
}

/// Closes each integer whose width is a value parameter, `u<N>` or `i<N>`,
/// in every type written since `type_start`. numeric_values.md,
/// "Width-indexed integers": `u13` and `u<13>` "are exactly the same type",
/// so with `13` for `N` the application is replaced by the builtin type of
/// that carrier, the very symbol the shorthand names; no later stage sees a
/// width that is still a name. A carrier no source spelled has no symbol
/// yet and is installed here.
///
/// "Zero widths ... reject": an application that gives such a parameter
/// zero, or a width no carrier has, is rejected at the call that applies it.
fn substitute_widths(
    program: &mut TypedTrees,
    type_start: usize,
    operands: &[Operand],
) -> Served<()> {
    for operand in operands {
        let Some(Value::Integer(literal)) = &operand.scalar else {
            continue;
        };
        let table = &program.type_reference_table;
        let occurrences = table
            .type_reference_rows_from(type_start)
            .filter_map(|(handle, node)| {
                let TypeReferenceNode::Generic {
                    base_symbol,
                    arguments,
                    ..
                } = node
                else {
                    return None;
                };
                let sign = program.symbols.integer_family(*base_symbol)?;
                let [argument] = table.type_reference_handles(*arguments) else {
                    return None;
                };
                // Copying the signature may already have replaced the binder
                // with this operand's own value reference; that exact handle
                // is the same width (`apply_copied_callable_interface`).
                let substituted = matches!(
                    operand.value,
                    StaticMachineBindingValue::Const(actual) if actual == *argument
                );
                (substituted
                    || matches!(
                        table.type_reference(*argument),
                        TypeReferenceNode::Named { symbol, .. } if *symbol == operand.parameter
                    ))
                .then_some((handle, sign))
            })
            .collect::<Vec<_>>();
        for (occurrence, sign) in occurrences {
            let letter = match sign {
                numerics::integer_carrier::IntegerSign::Unsigned => 'u',
                numerics::integer_carrier::IntegerSign::Signed => 'i',
            };
            let width = literal.canonical_spelling().to_string();
            let Some(carrier) = literal
                .value_u64()
                .filter(|width| *width > 0)
                .and_then(|width| numerics::integer_carrier::FixedInteger::new(sign, width))
            else {
                return Err(Stop::Rejected(format!(
                    "this application gives `{letter}<{}>` the width {width}: an integer width \
                     is a positive `u64`",
                    program.symbols.name(operand.parameter),
                )));
            };
            let atom = symbols::BuiltinTypeAtom::Integer(carrier);
            program.symbols.install_integer_types([carrier]);
            let symbol = program
                .symbols
                .builtin_atom_symbol(atom)
                .ok_or(Stop::Defect("an installed integer carrier has no symbol"))?;
            let replacement = program
                .type_reference_table
                .insert(TypeReferenceNode::Named {
                    symbol,
                    name: Identifier::generated(carrier.to_string()),
                });
            program
                .type_reference_table
                .substitute_from(occurrence, replacement)
                .map_err(Stop::Defect)?;
        }
    }
    Ok(())
}

/// Replaces each value parameter by its operand's closed value in every
/// expression written since `expression_start`: the generated machine's own
/// contract and body. A name that is the parameter and nothing more becomes
/// the value. An integer literal carries the parameter's type as its landing
/// and a Boolean has only the one type, so either is typed as the parameter
/// was.
fn substitute_values(program: &mut TypedTrees, expression_start: usize, operands: &[Operand]) {
    let value_of = |symbol: SymbolHandle| {
        operands
            .iter()
            .find(|operand| operand.parameter == symbol)
            .and_then(|operand| operand.scalar.clone())
    };
    let named = program
        .expression_table
        .iter_expressions_from(expression_start)
        .filter_map(|(handle, node)| match node {
            ExpressionNode::Name(path)
                if path.symbol.is_valid()
                    && !path.static_application.is_valid()
                    && path.members.count() == 1 =>
            {
                value_of(path.symbol).map(|value| (handle, value))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    for (handle, value) in named {
        *program.expression_table.expression_mut(handle) = value.expression();
    }
}

/// Evaluates closed integer `ConstExpression` type references copied into a
/// specialized machine (`Index<Count + 1>` after `Count = 3` -> `Index<4>`)
/// and settles them in place so type identity and downstream readers see the
/// canonical literal form. A closed expression with no value (`Count + 1`
/// after `Count = 255` with `Count: u8`, or a division by zero) refuses the
/// application rather than settling a number its carrier does not hold.
fn settle_const_expressions(
    program: &mut TypedTrees,
    type_start: usize,
    operands: &[Operand],
) -> Served<()> {
    let mut candidates = Vec::new();
    for (handle, node) in program
        .type_reference_table
        .type_reference_rows_from(type_start)
    {
        if let TypeReferenceNode::ConstExpression(expression) = node
            && let Some(value) = substituted_const_int(program, *expression, operands)?
        {
            candidates.push((handle, value));
        }
    }
    for (handle, value) in candidates {
        data_applications::settle_const_argument(program, handle, &value)
            .map_err(Stop::Rejected)?;
    }
    Ok(())
}

/// The decimal value of a const expression once each value parameter is
/// its operand's literal, which carries the parameter's type as its landing.
/// The fold is exact (`const_integer::exact_value`): `None` leaves an
/// expression it cannot decide as written, and a closed expression with no
/// value is refused.
fn substituted_const_int(
    program: &TypedTrees,
    expression: ExpressionHandle,
    operands: &[Operand],
) -> Served<Option<String>> {
    use crate::source_state::const_integer::{ExactInteger, Operators, exact_value};
    let operand_literal = |path: &TableNamePath| {
        if !path.symbol.is_valid()
            || path.static_application.is_valid()
            || path.members.count() != 1
        {
            return None;
        }
        operands
            .iter()
            .find(|operand| operand.parameter == path.symbol)
            .and_then(|operand| match &operand.scalar {
                Some(Value::Integer(literal)) => Some(literal.clone()),
                _ => None,
            })
    };
    match exact_value(
        &program.expression_table,
        expression,
        Operators::Ring,
        &operand_literal,
    ) {
        ExactInteger::Value { value, .. } => Ok(Some(value.to_string())),
        ExactInteger::Undecided => Ok(None),
        ExactInteger::Refused(refusal) => Err(Stop::Rejected(format!(
            "the constant argument `{}` of this application has no value: {refusal}",
            program.expression_table.display_name(expression)
        ))),
    }
}

/// Points each copied call through an implementation parameter at the
/// operation the implementation selected for that requirement. The call
/// keeps the binding the author wrote, which still names the parameter's
/// requirement, and drops the qualifier that named the parameter: the
/// generated machine has no parameter to qualify by.
fn select_requirements(
    program: &mut TypedTrees,
    body: &CopiedCallableBody,
    selections: &[Selection],
) {
    if selections.is_empty() {
        return;
    }
    let selected = |target: SymbolHandle| {
        selections
            .iter()
            .find(|selection| selection.placeholder == target)
            .map(|selection| selection.dispatch.clone())
    };
    let calls = program
        .expression_table
        .iter_expressions_from(body.first_expression)
        .take(body.expression_count)
        .filter_map(|(handle, node)| match node {
            ExpressionNode::Call(call) => selected(call.target_symbol).map(|found| (handle, found)),
            _ => None,
        })
        .collect::<Vec<_>>();
    for (handle, dispatch) in calls {
        if let ExpressionNode::Call(call) = program.expression_table.expression_mut(handle) {
            call.target_symbol = dispatch.realization_state;
            call.receiver = ExpressionHandle::invalid();
            call.static_requirement_dispatch = Some(dispatch);
        }
    }
    for &handle in &body.statements {
        if let StatementNode::Call(call) = program.statement_table.statement_mut(handle)
            && let Some(dispatch) = selected(call.target_symbol)
        {
            call.target_symbol = dispatch.realization_state;
            call.receiver_root_symbol = SymbolHandle::invalid();
            call.receiver_symbol = SymbolHandle::invalid();
            call.receiver = HandleSpan::empty();
            call.static_requirement_dispatch = Some(dispatch);
        }
    }
}

/// Classify operation-record receivers while they still name their authored
/// binders. The operands already retain the exact schema-checked application.
/// General const expression substitution can then replace ordinary value uses
/// without erasing receiver provenance. Member checks wait for the copier to
/// apply the enclosing arguments to the call's complete target metadata.
fn copied_record_receivers(
    program: &TypedTrees,
    body: &CopiedCallableBody,
    operands: &[Operand],
) -> Vec<(Site, SymbolHandle, SymbolHandle, TypeReferenceHandle)> {
    let mut records = operands
        .iter()
        .filter_map(|operand| {
            let StaticMachineBindingValue::Const(reference) = operand.value else {
                return None;
            };
            let symbol = operand.argument.symbol;
            program
                .conformance_by_symbol(symbol)
                .map(|_| (operand.parameter, symbol, reference))
        })
        .collect::<Vec<_>>();
    records.sort_unstable_by_key(|(parameter, _, _)| symbol_order(*parameter));
    let selected_record = |parameter| {
        records
            .binary_search_by_key(&symbol_order(parameter), |(parameter, _, _)| {
                symbol_order(*parameter)
            })
            .ok()
            .map(|position| {
                (
                    records[position].0,
                    records[position].1,
                    records[position].2,
                )
            })
    };
    let mut receivers = program
        .expression_table
        .iter_expressions_from(body.first_expression)
        .take(body.expression_count)
        .filter_map(|(handle, node)| match node {
            ExpressionNode::Call(call) if !call.build_selection.is_valid() => program
                .expression_table
                .name_path_node(call.receiver)
                .and_then(|path| selected_record(path.symbol))
                .map(|(binder, selected, reference)| {
                    (Site::Expression(handle), binder, selected, reference)
                }),
            _ => None,
        })
        .collect::<Vec<_>>();
    for &handle in &body.statements {
        let record = program
            .statement_table
            .statement(handle)
            .as_call()
            .filter(|call| call.receiver_root_symbol == call.receiver_symbol)
            .and_then(|call| {
                selected_record(call.receiver_symbol).map(|(binder, selected, reference)| {
                    (Site::Statement(handle), binder, selected, reference)
                })
            });
        if let Some(record) = record {
            receivers.push(record);
        }
    }
    receivers
}

fn substitute_record_receivers(
    program: &mut TypedTrees,
    receivers: &[(Site, SymbolHandle, SymbolHandle, TypeReferenceHandle)],
) -> Served<()> {
    for &(site, binder, selected, reference) in receivers {
        let call = site.call(program).ok_or(Stop::Defect(
            "a copied static record call changed its statement or expression kind",
        ))?;
        let target = match call {
            StaticArgumentCall::Expression(_, call) => call.target_symbol,
            StaticArgumentCall::Statement(call) => call.target_symbol,
        };
        program
            .static_record_bound_requirement(program.symbols.get(binder).parent, binder, target)
            .map_err(Stop::Rejected)?
            .ok_or(unserved("record receiver lost its declared schema"))?;
        let application = call.application();
        if !program
            .expression_table
            .static_machine_application_is_valid(application)
        {
            return Err(Stop::Defect("record receiver lost its source application"));
        }
        let source = program
            .expression_table
            .static_machine_application_mut(application);
        if source
            .source_record_receiver
            .is_some_and(|known| known != binder)
        {
            return Err(Stop::Defect("record call changed its original receiver"));
        }
        source.source_record_receiver = Some(binder);
        program
            .named_conformance_member(selected, target)
            .map_err(unserved)?
            .ok_or(unserved(
                "a static record call lost its exact declared requirement",
            ))?;
        match site {
            Site::Expression(handle) => {
                let ExpressionNode::Call(call) = program.expression_table.expression(handle) else {
                    return Err(Stop::Defect("a copied static record lost its exact call"));
                };
                let receiver = call.receiver;
                let ExpressionNode::Name(path) = program.expression_table.expression_mut(receiver)
                else {
                    return Err(Stop::Defect(
                        "a static record receiver changed its expression kind",
                    ));
                };
                path.head_symbol = selected;
                path.symbol = selected;
                if let ExpressionNode::Call(call) = program.expression_table.expression_mut(handle)
                {
                    call.requirement_application = reference;
                }
            }
            Site::Statement(handle) => {
                let StatementNode::Call(call) = program.statement_table.statement_mut(handle)
                else {
                    return Err(Stop::Defect("a copied static record lost its exact call"));
                };
                call.receiver_root_symbol = selected;
                call.receiver_symbol = selected;
            }
        }
    }
    Ok(())
}

/// Replaces each parameter a copied call passes on as a static argument
/// (`forward<T>` in the body of `relay`) by the operand's own argument. The
/// copied application is then unbound again, with this machine as its
/// holder, so it is bound and served from its new arguments like an
/// authored one.
///
/// A Build selection, `Build::select_provider::<Reading<Tag>, Provider<Tag>>(..)`
/// in a helper generic in `Tag`, is left exactly as written. Its operands
/// are not static arguments of a machine to generate: they are the request
/// the call was authored with, which names the helper's parameters by their
/// declarations. provider_selection.md, "Declaration and choice": "Helpers
/// forward that argument", and a selection keeps the declaration that wrote
/// it as its author. So the generated machine holds the authored request,
/// and whoever reads the selection takes the value of each parameter from
/// this machine's application (`MachineSpecialization::binding_application`,
/// read by `crate::source_state::product_operand::capture_static_environment`).
fn substitute_arguments(
    program: &mut TypedTrees,
    body: &CopiedCallableBody,
    holder: SymbolHandle,
    operands: &[Operand],
) -> Served<()> {
    let replace = |program: &mut TypedTrees, arguments: &mut [StaticMachineArgument]| {
        for argument in arguments {
            argument.authored_machine_application = StaticMachineApplicationHandle::invalid();
            if let Some(operand) = operands
                .iter()
                .find(|operand| operand.parameter == argument.symbol)
            {
                *argument = operand.argument.clone();
            } else if argument.type_reference.is_valid()
                && let TypeReferenceNode::ConstExpression(expression) = *program
                    .type_reference_table
                    .type_reference(argument.type_reference)
                && let Some(spelling) = substituted_const_int(program, expression, operands)?
            {
                let fresh = program
                    .type_reference_table
                    .insert(TypeReferenceNode::Named {
                        symbol: SymbolHandle::invalid(),
                        name: Identifier::generated(spelling.clone()),
                    });
                argument.type_reference = fresh;
                argument.const_literal = None;
                argument.symbol = SymbolHandle::invalid();
                argument.path = Box::new([Identifier::generated(spelling)]);
            }
        }
        Served::Ok(())
    };
    let mut callables = operands
        .iter()
        .filter_map(|operand| match &operand.value {
            StaticMachineBindingValue::Machine(argument) => {
                Some((operand.parameter, argument.symbol))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    callables.sort_unstable_by_key(|(binder, _)| symbol_order(*binder));
    let selected_callable = |target| {
        callables
            .binary_search_by_key(&symbol_order(target), |(binder, _)| symbol_order(*binder))
            .ok()
            .map(|position| callables[position].1)
    };
    let mut applications = Vec::new();
    let calls = program
        .expression_table
        .iter_expressions_from(body.first_expression)
        .take(body.expression_count)
        .filter(|(_, node)| matches!(node, ExpressionNode::Call(_)))
        .map(|(handle, _)| handle)
        .collect::<Vec<_>>();
    for handle in calls {
        let (mut machine_arguments, closed_template) =
            match program.expression_table.expression(handle) {
                ExpressionNode::Call(call) if !call.build_selection.is_valid() => (
                    Some(call.machine_arguments.clone()),
                    program
                        .closed_requirement_application(call.target_symbol)
                        .map(|closed| closed.template),
                ),
                _ => (None, None),
            };
        if let Some(arguments) = machine_arguments.as_mut() {
            replace(program, arguments)?;
        }
        if let ExpressionNode::Call(call) = program.expression_table.expression_mut(handle)
            && !call.build_selection.is_valid()
        {
            if let Some(arguments) = machine_arguments {
                call.machine_arguments = arguments;
            }
            if let Some(template) = closed_template {
                call.target_symbol = template;
            }
            if let Some(target) = selected_callable(call.target_symbol) {
                call.static_machine_parameter = call.target_symbol;
                call.target_symbol = target;
            }
            applications.push(call.authored_machine_application);
        }
    }
    for &handle in &body.statements {
        let (mut machine_arguments, closed_template) =
            match program.statement_table.statement(handle) {
                StatementNode::Call(call) => (
                    Some(call.machine_arguments.clone()),
                    program
                        .closed_requirement_application(call.target_symbol)
                        .map(|closed| closed.template),
                ),
                _ => (None, None),
            };
        if let Some(arguments) = machine_arguments.as_mut() {
            replace(program, arguments)?;
        }
        let statement = program.statement_table.statement_mut(handle);
        match statement {
            StatementNode::Call(call) => {
                if let Some(arguments) = machine_arguments {
                    call.machine_arguments = arguments;
                }
                if let Some(template) = closed_template {
                    call.target_symbol = template;
                }
                if let Some(target) = selected_callable(call.target_symbol) {
                    call.static_machine_parameter = call.target_symbol;
                    call.target_symbol = target;
                }
                applications.push(call.authored_machine_application);
            }
            StatementNode::Transition(transition) => {
                let targets = transition.targets();
                for handle in targets {
                    if let crate::source_state::statement::TransitionTargetNode::Named {
                        path,
                        static_machine_parameter,
                        ..
                    } = program.statement_table.transition_target_mut(handle)
                        && let Some(target) = selected_callable(path.symbol)
                    {
                        *static_machine_parameter = path.symbol;
                        path.symbol = target;
                    }
                }
            }
            _ => {}
        }
    }
    for application in applications {
        if !application.is_valid() {
            continue;
        }
        let mut arguments = program
            .expression_table
            .static_machine_application_arguments_mut(application)
            .to_vec();
        replace(program, &mut arguments)?;
        let table = &mut program.expression_table;
        table
            .static_machine_application_arguments_mut(application)
            .clone_from_slice(&arguments);
        table.set_static_machine_application_bindings(application, &[]);
        table
            .static_machine_application_mut(application)
            .lexical_owner = holder;
    }
    Ok(())
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod implementation_tests;

#[cfg(test)]
mod lifetime_tests;
