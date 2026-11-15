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
//! parameters given plain named types, implementation parameters given
//! plain named implementations and value parameters given one integer
//! literal or `true`/`false`, on single-state checked bodies with their lexical children
//! ([`template`]). Each child has no independent static telescope; it retains
//! the original enclosing application without capturing runtime values.
//!
//! A value parameter (`value<const Units: u64>`) is bound like the others
//! and is replaced by its literal, landed in the parameter's type, wherever
//! the copied declaration names it. generics.md, "Value binders and const
//! requirements": the value "must be statically known within the compiled
//! specialization". Only a closed scalar is read here: an integer literal,
//! `true` or `false`. Typing has already replaced a calculation over
//! literals, `(8u64 + 9u64)`, by the literal of its value
//! (`typed_trees::const_integer::closed_literal`); an argument that a machine
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
use typed_trees::references::{StaticArgumentCall, StaticArgumentRole};
use typed_trees::state::State;
use typed_trees::statement::{StatementHandle, StatementNode};
use typed_trees::static_applications::{
    StaticMachineApplicationHandle, StaticMachineBinding, StaticMachineBindingValue,
};

use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};
use typed_trees::{
    MachineSpecialization, PreparedSymbolRebinding, StaticRequirementDispatch, SubtreeEdges,
    SymbolRebinding, TypedTreeTables, TypedTrees,
};

mod named_conformances;

pub(crate) fn prepare(program: &mut TypedTrees) -> Result<(), Vec<Diagnostic>> {
    let mut generated = GeneratedMachines::default();
    let mut captures = typed_trees::NamedConformanceCaptureIndex::new(program);
    // A generated machine is appended to the machines and read in its turn.
    let mut position = 0;
    while position < program.machines().len() {
        let holder = program.machines()[position].symbol;
        for site in call_sites(program, position) {
            let served = named_conformances::serve(program, site, &mut captures)
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
                        .map_err(|reason| Stop::Unserved(Some(reason)))
                });
            match served {
                Ok(()) => {}
                Err(Stop::Unserved(detail)) => {
                    let span = site.source_span(program);
                    program.record_deferred_refusal_with_detail(
                        holder,
                        DeferredRequest::StaticApplication,
                        span,
                        detail.unwrap_or(""),
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
        position += 1;
    }
    Ok(())
}

/// Why one application was left as typing wrote it.
enum Stop {
    /// A request this pass does not serve yet; nothing was changed for it.
    Unserved(Option<&'static str>),
    /// The application is wrong as the author wrote it.
    Rejected(String),
    /// The trees broke an invariant while a machine was being generated.
    Defect(&'static str),
}

type Served<T> = Result<T, Stop>;

/// One generated machine: the generic machine, the declaration each of its
/// parameters was bound to, and the generated machine and its one state.
struct Generated {
    template: SymbolHandle,
    arguments: Vec<Identity>,
    machine: SymbolHandle,
    state: SymbolHandle,
}

/// Every machine this pass generated, in the order the applications were
/// first met, and an index answering "has this application already been
/// generated?" without reading rows of other applications.
///
/// One application makes one machine, so a closed application is looked up
/// before it is generated. Scanning `rows` for that made the pass quadratic
/// in the number of distinct closed applications, across unrelated
/// templates too. `by_application` holds row numbers ordered by template,
/// then by the complete argument key; a lookup is a binary search comparing
/// whole keys, never a digest. Rows stay in encounter order: nothing reads a
/// row's position as anything but a stable name.
#[derive(Default)]
struct GeneratedMachines {
    rows: Vec<Generated>,
    by_application: Vec<u32>,
}

impl GeneratedMachines {
    /// The row generated for `template` at `arguments`, or the place in
    /// `by_application` where that application belongs.
    fn search(&self, template: SymbolHandle, arguments: &[Identity]) -> Result<usize, usize> {
        let wanted = (symbol_order(template), arguments);
        self.by_application
            .binary_search_by(|&row| {
                let row = &self.rows[row as usize];
                (symbol_order(row.template), row.arguments.as_slice()).cmp(&wanted)
            })
            .map(|slot| self.by_application[slot] as usize)
    }

    /// Append `made` and index it at `slot`, the place [`Self::search`]
    /// reported for its application.
    fn insert(&mut self, slot: usize, made: Generated) -> &Generated {
        let row = u32::try_from(self.rows.len()).expect("generated machines fit a u32 row");
        self.rows.push(made);
        self.by_application.insert(slot, row);
        &self.rows[row as usize]
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
/// (`Selected::value<7>(7)` once `Selected` is known) applies that machine
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

fn serve(program: &mut TypedTrees, site: Site, generated: &mut GeneratedMachines) -> Served<()> {
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
    if let Some((declaration, _)) = entered_machine(program, applied.target)
        && program
            .machine_by_symbol(declaration)
            .is_some_and(|machine| {
                machine.supply_mode == language_semantics::MachineSupplyMode::Boundary
                    && machine.has_bodyless_signature()
            })
    {
        program
            .prepare_bodyless_callable_application(applied.application, applied.target)
            .map_err(|reason| Stop::Unserved(Some(reason)))?;
        return Ok(());
    }
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
    if program
        .executable_application_template(callee)
        .map_err(Stop::Defect)?
        .is_some()
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
    let parameters = program
        .machine_callable_parameters(machine)
        .into_iter()
        .map(|parameter| (parameter.symbol, parameter.kind))
        .collect::<Vec<_>>();
    if parameters.is_empty() {
        return Ok(());
    }
    // A declaration query reads its actual type or callable operand; it
    // does not execute this generic declaration's placeholder body. Check
    // that ownership only for requests that would otherwise specialize.
    if let Some(call) = site.call(program)
        && typed_trees::reflection::construction::declaration_query_argument_role(program, call)
            == StaticArgumentRole::DeclarationDescription
    {
        return Ok(());
    }
    if parameters.len() != arguments.len() {
        return Err(Stop::Unserved(None));
    }
    let mut operands = Vec::with_capacity(parameters.len());
    for ((parameter, kind), argument) in parameters.iter().zip(&arguments) {
        operands.push(Operand::read(program, *parameter, kind, argument)?);
    }
    program
        .expression_table
        .static_machine_application_mut(applied.application)
        .binding_declaration = callee;
    bind(program, applied.application, &operands);
    // An argument that names a parameter of the enclosing generic machine
    // leaves the application open: it is closed in each machine generated
    // from that one, where the parameter is replaced.
    if operands.iter().any(|operand| operand.is_open(program)) {
        return Ok(());
    }
    let key = operands
        .iter()
        .map(|operand| operand.identity.clone())
        .collect::<Vec<_>>();
    let entry = match generated.search(callee, &key) {
        Ok(row) => &generated.rows[row],
        Err(slot) => {
            let made = generate(program, callee, applied.application, &operands)?;
            if made.template != callee || made.arguments != key {
                return Err(Stop::Defect(
                    "a generated machine changed the application it was generated for",
                ));
            }
            generated.insert(slot, made)
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
            _ => return Err(Stop::Unserved(None)),
        }
    };
    let Some(template) = program.machine_by_symbol(row.template) else {
        return Err(Stop::Unserved(None));
    };
    let parameters = program.machine_callable_parameters(template);
    let table = &program.expression_table;
    let selection = table.static_machine_application(row.binding_application);
    let Some((owner, _)) = entered_machine(program, selection.registration_operation) else {
        return Err(Stop::Unserved(None));
    };
    let Some(source) = program.machine_by_symbol(owner) else {
        return Err(Stop::Unserved(None));
    };
    // The selection belongs to the original enclosing declaration, not an
    // unrelated application with equal rendered arguments.
    if row.template_parameters != source.type_parameters
        || program.symbols.get(callee).generated_from != row.template
    {
        return Err(Stop::Unserved(None));
    }
    let mut lexical = row.template;
    while lexical.is_valid() && lexical != owner {
        lexical = program.symbols.get(lexical).parent;
    }
    if lexical != owner {
        return Err(Stop::Unserved(None));
    }
    let source_parameters = program.machine_callable_parameters(source);
    let operands = bound_arguments(
        program,
        &parameters,
        &source_parameters,
        row.binding_application,
        arguments,
    )?;
    bind(program, applied.application, &operands);
    site.enter(program, applied.target);
    Ok(())
}

/// Compare a requested tuple with the complete original selection, including
/// unused binders. This only acknowledges an already supplied application;
/// the caller separately authenticates its source association.
fn bound_arguments(
    program: &mut TypedTrees,
    parameters: &[typed_trees::data::TypeParameter],
    source_parameters: &[typed_trees::data::TypeParameter],
    selection: StaticMachineApplicationHandle,
    arguments: &[StaticMachineArgument],
) -> Served<Vec<Operand>> {
    if parameters.len() != arguments.len() || parameters != source_parameters {
        return Err(Stop::Unserved(None));
    }
    let table = &program.expression_table;
    let actual_arguments = table.static_machine_application_arguments(selection);
    let bindings = table.static_machine_application_bindings(selection);
    if actual_arguments.len() != bindings.len() || bindings.len() != source_parameters.len() {
        return Err(Stop::Unserved(None));
    }
    let mut selected = symbols::SymbolMap::default();
    for (binding, argument) in bindings.iter().zip(actual_arguments) {
        if selected
            .insert(binding.parameter, argument.clone())
            .is_some()
        {
            return Err(Stop::Unserved(None));
        }
    }
    for parameter in source_parameters {
        if !selected.contains_key(&parameter.symbol) {
            return Err(Stop::Unserved(None));
        }
    }
    let mut operands = Vec::with_capacity(parameters.len());
    for (parameter, argument) in parameters.iter().zip(arguments) {
        let Some(actual) = selected.get(&parameter.symbol) else {
            return Err(Stop::Unserved(None));
        };
        let operand = Operand::read(program, parameter.symbol, &parameter.kind, argument)?;
        let existing = Operand::read(program, parameter.symbol, &parameter.kind, actual)?;
        if operand.is_open(program) || operand.identity != existing.identity {
            return Err(Stop::Unserved(None));
        }
        operands.push(operand);
    }
    Ok(operands)
}

/// A call through an implementation parameter to a requirement that takes
/// value parameters, `Implementation::value<7>(7)`: nothing is generated for
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
        .map(|parameter| (parameter.symbol, parameter.kind.clone()))
        .collect::<Vec<_>>();
    if !parameters
        .iter()
        .all(|(_, kind)| matches!(kind, typed_trees::data::TypeParameterKind::Const { .. }))
    {
        return Ok(());
    }
    if parameters.len() != arguments.len() {
        return Err(Stop::Unserved(None));
    }
    let mut operands = Vec::with_capacity(parameters.len());
    for ((parameter, kind), argument) in parameters.iter().zip(arguments) {
        operands.push(Operand::read(program, *parameter, kind, argument)?);
    }
    program
        .expression_table
        .static_machine_application_mut(applied.application)
        .binding_declaration = declaration;
    bind(program, applied.application, &operands);
    Ok(())
}

/// A call that names a trait's requirement and applies the requirement's own
/// parameters, `Probe::take<Reading>(reading)`. A requirement has no body to
/// copy, so what the application selects is the requirement's signature and
/// contract with each parameter replaced: one closed requirement per
/// application, which the call then names. generics.md, "Closed applications
/// and canonical identity": two calls at the same arguments name the same
/// closed requirement (`TypedTrees::instantiate_requirement_application`
/// keys it by the application's identity).
///
/// Served for a requirement of a trait with no parameters of its own, given
/// the operands every application here reads ([`Operand::read`]). A trait
/// applied at arguments, and any operand that reading does not serve, leave
/// the call as typing wrote it.
fn apply_requirement(
    program: &mut TypedTrees,
    site: Site,
    applied: &Applied,
    arguments: &[StaticMachineArgument],
) -> Served<()> {
    let Some((owner, requirement)) = program.trait_machine_signature(applied.target) else {
        return Ok(());
    };
    if requirement.symbol != applied.target {
        return Ok(());
    }
    let parameters = program
        .state_signature_type_parameters(requirement)
        .iter()
        .map(|parameter| (parameter.symbol, parameter.kind.clone()))
        .collect::<Vec<_>>();
    if parameters.is_empty() {
        return Ok(());
    }
    if !owner.type_parameters.is_empty()
        || !owner.lifetime_parameters.is_empty()
        || !requirement.lifetime_parameters.is_empty()
        || parameters.len() != arguments.len()
    {
        return Err(Stop::Unserved(None));
    }
    let environment =
        typed_trees::trait_definition::AppliedTraitScope::abstract_scope(owner).environment();
    let mut operands = Vec::with_capacity(parameters.len());
    for ((parameter, kind), argument) in parameters.iter().zip(arguments) {
        operands.push(Operand::read(program, *parameter, kind, argument)?);
    }
    bind(program, applied.application, &operands);
    // Open at a parameter of the enclosing generic machine: closed in each
    // machine generated from that one.
    if operands.iter().any(|operand| operand.is_open(program)) {
        return Ok(());
    }
    let table = &mut program.expression_table;
    if table
        .static_machine_application(applied.application)
        .registration_operation
        != applied.target
    {
        return Err(Stop::Unserved(None));
    }
    table
        .static_machine_application_mut(applied.application)
        .binding_declaration = applied.target;
    table.set_static_machine_application_lifetimes(applied.application, applied.target, &[]);
    let closed = program
        .instantiate_requirement_application(&environment, applied.target, &[], applied.application)
        .map_err(|_| Stop::Unserved(None))?;
    site.enter(program, closed);
    Ok(())
}

/// What a static argument is, as the identity an application is keyed by:
/// two applications of one machine are the same application when each
/// parameter is given the same declaration or the same value.
#[derive(Clone, PartialEq, Eq)]
enum Identity {
    Type(typed_trees::type_identity::TypeIdentityId),
    /// The declaration the argument names: a type, an implementation, or a
    /// parameter of the enclosing generic machine.
    Declaration(SymbolHandle),
    /// A closed scalar value, by its canonical spelling. The parameter it is
    /// given to fixes its type.
    Scalar(String),
}

/// Applications are indexed in sorted order ([`GeneratedMachines`]). The
/// order only has to agree with equality: same variant and same payload.
impl Ord for Identity {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        match (self, other) {
            (Self::Type(left), Self::Type(right)) => left.cmp(right),
            (Self::Declaration(left), Self::Declaration(right)) => {
                symbol_order(*left).cmp(&symbol_order(*right))
            }
            (Self::Scalar(left), Self::Scalar(right)) => left.cmp(right),
            _ => self.rank().cmp(&other.rank()),
        }
    }
}

impl PartialOrd for Identity {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Identity {
    fn rank(&self) -> u8 {
        match self {
            Self::Type(_) => 0,
            Self::Declaration(_) => 1,
            Self::Scalar(_) => 2,
        }
    }
}

/// One static argument read for the parameter it is given to. Every kind of
/// parameter is bound through this one shape; a kind with no reading yet is
/// unserved.
struct Operand {
    parameter: SymbolHandle,
    /// What an application is keyed by and what is substituted for the
    /// parameter.
    identity: Identity,
    /// The argument as the author wrote it, which replaces the parameter
    /// where a copied body passes it on as a static argument.
    argument: StaticMachineArgument,
    value: StaticMachineBindingValue,
    /// A value parameter's closed value, an integer landed in the
    /// parameter's type or a Boolean: what replaces the parameter where a
    /// copied expression names it.
    scalar: Option<Value>,
}

impl Operand {
    fn read(
        program: &mut TypedTrees,
        parameter: SymbolHandle,
        kind: &typed_trees::data::TypeParameterKind,
        argument: &StaticMachineArgument,
    ) -> Served<Self> {
        use typed_trees::data::TypeParameterKind;
        let mut scalar = None;
        let (identity, value) = match kind {
            TypeParameterKind::Type => {
                let reference = named_type(program, argument)?;
                let identity = if argument.symbol.is_valid()
                    && program.symbols.get(argument.symbol).kind == SymbolKind::TypeParameter
                {
                    Identity::Declaration(argument.symbol)
                } else {
                    Identity::Type(program.normalized_type_identity(reference))
                };
                (identity, StaticMachineBindingValue::Type(reference))
            }
            TypeParameterKind::Implementation => (
                Identity::Declaration(named_implementation(program, parameter, argument)?),
                StaticMachineBindingValue::Conformance(argument.clone()),
            ),
            TypeParameterKind::Const { type_reference } => {
                let identity = match closed_scalar(program, *type_reference, argument)? {
                    Scalar::Closed(value) => {
                        let identity = Identity::Scalar(value.spelled());
                        scalar = Some(value);
                        identity
                    }
                    Scalar::Parameter(symbol) => Identity::Declaration(symbol),
                };
                let reference = if let Some(value) = &scalar {
                    let expression = program.expression_table.insert(value.expression());
                    program
                        .type_reference_table
                        .insert(TypeReferenceNode::ConstExpression(expression))
                } else {
                    program
                        .static_operand_type_reference(argument)
                        .map_err(|_| Stop::Unserved(None))?
                };
                (identity, StaticMachineBindingValue::Const(reference))
            }
            // A runtime value, a machine and a proposition each need their
            // own reading of the argument; none is built yet.
            TypeParameterKind::Value { .. }
            | TypeParameterKind::Machine { .. }
            | TypeParameterKind::Proposition { .. } => return Err(Stop::Unserved(None)),
        };
        Ok(Self {
            parameter,
            identity,
            argument: argument.clone(),
            value,
            scalar,
        })
    }

    /// The declaration this operand names, if it names one.
    fn declaration(&self) -> Option<SymbolHandle> {
        match &self.identity {
            Identity::Declaration(symbol) => Some(*symbol),
            Identity::Scalar(_) | Identity::Type(_) => None,
        }
    }

    /// Whether the argument names a parameter of the enclosing generic
    /// machine, of any kind.
    fn is_open(&self, program: &TypedTrees) -> bool {
        self.declaration().is_some_and(|declaration| {
            matches!(
                program.symbols.get(declaration).kind,
                SymbolKind::TypeParameter | SymbolKind::ConformanceParameter
            )
        })
    }

    /// How this operand is spelled in a generated machine's name.
    fn spelled(&self, program: &TypedTrees) -> String {
        match &self.identity {
            Identity::Declaration(symbol) => program.symbols.name(*symbol).to_owned(),
            Identity::Scalar(value) => value.clone(),
            Identity::Type(identity) => program.type_identity_text(*identity).unwrap_or_default(),
        }
    }
}

/// A closed value of a builtin scalar type: what a value parameter is given.
#[derive(Clone)]
enum Value {
    /// An integer literal, landed in the parameter's type.
    Integer(numerics::literals::IntegerLiteral),
    Boolean(bool),
}

impl Value {
    /// The canonical spelling an application is keyed by. The parameter's
    /// type tells `1` from `true`.
    fn spelled(&self) -> String {
        match self {
            Self::Integer(literal) => literal.canonical_spelling().to_string(),
            Self::Boolean(value) => value.to_string(),
        }
    }

    fn expression(&self) -> ExpressionNode {
        match self {
            Self::Integer(literal) => ExpressionNode::Integer(literal.clone()),
            Self::Boolean(value) => ExpressionNode::Boolean(*value),
        }
    }
}

/// A value argument as this pass reads one.
enum Scalar {
    /// A closed value of the parameter's type.
    Closed(Value),
    /// A value parameter of the enclosing generic machine, passed on.
    Parameter(SymbolHandle),
}

/// The value a static argument gives a value parameter of type
/// `parameter_type`, when the argument is closed: one integer literal and
/// nothing else, for a parameter of a fixed-width builtin integer type;
/// `true` or `false` and nothing else, for a parameter of type `bool`; or
/// only the name of a parameter in scope. A literal that states another type
/// than the parameter's is not the parameter's value. An argument that
/// names a completed constant uses its canonical value; no machine runs here.
fn closed_scalar(
    program: &TypedTrees,
    parameter_type: TypeReferenceHandle,
    argument: &StaticMachineArgument,
) -> Served<Scalar> {
    if let Some(primitive) = program.primitive_type_reference(parameter_type)
        && let Ok(value) = program.prepare_callable_scalar_argument(primitive, argument)
    {
        return match value {
            ExpressionNode::Integer(literal) => Ok(Scalar::Closed(Value::Integer(literal))),
            ExpressionNode::Boolean(value) => Ok(Scalar::Closed(Value::Boolean(value))),
            _ => Err(Stop::Defect("prepared callable scalar is not a scalar")),
        };
    }
    let symbol = argument.symbol;
    if only_a_name(program, argument)
        && symbol.is_valid()
        && program.symbols.symbols().nodes().is_valid(symbol)
        && program.symbols.get(symbol).kind == SymbolKind::TypeParameter
    {
        return Ok(Scalar::Parameter(symbol));
    }
    Err(Stop::Unserved(None))
}

/// Whether a static argument is a name and nothing else, so that replacing
/// the declaration it names replaces all of it. Typing may keep the same
/// name a second time as a type; that says nothing more.
fn only_a_name(program: &TypedTrees, argument: &StaticMachineArgument) -> bool {
    let named_again = !argument.type_reference.is_valid()
        || matches!(
            program.type_reference_table.type_reference(argument.type_reference),
            TypeReferenceNode::Named { symbol, .. } if *symbol == argument.symbol
        );
    argument.application.is_none() && argument.evidence_projection.is_none() && named_again
}

/// The implementation a static argument names when it is only a name, as
/// the declaration that stands for its value: an implementation parameter
/// in scope, or a named implementation with no parameters of its own, of
/// exactly the trait `parameter` is bound by. An applied implementation, a
/// projection and a bound applied at arguments are unserved here.
///
/// conformances.md, "Resulting-value identity": "A name or unused
/// constructor argument creates no additional identity", so two names that
/// construct one value are one argument, and the application is keyed by the
/// first declaration constructing it (`implementation_value_head`).
fn named_implementation(
    program: &TypedTrees,
    parameter: SymbolHandle,
    argument: &StaticMachineArgument,
) -> Served<SymbolHandle> {
    let symbol = argument.symbol;
    if !only_a_name(program, argument)
        || argument.const_literal.is_some()
        || !symbol.is_valid()
        || !program.symbols.symbols().nodes().is_valid(symbol)
    {
        return Err(Stop::Unserved(None));
    }
    match program.symbols.get(symbol).kind {
        SymbolKind::ConformanceParameter => Ok(symbol),
        SymbolKind::Conformance => {
            let owner = program.symbols.get(parameter).parent;
            let mut bounds = program
                .callable_conformance_bounds(owner)
                .unwrap_or_default()
                .iter()
                .filter(|bound| bound.binder == parameter);
            let (Some(bound), None) = (bounds.next(), bounds.next()) else {
                return Err(Stop::Unserved(None));
            };
            let plain = program.conformance_by_symbol(symbol).is_some_and(|named| {
                named.trait_symbol == bound.trait_symbol
                    && bound.arguments.is_empty()
                    && bound.lifetime_arguments.is_empty()
                    && named.arguments.is_empty()
                    && named.trait_lifetime_arguments.is_empty()
                    && named.lifetime_parameters.is_empty()
                    && program.conformance_type_parameters(named).is_empty()
            });
            if !plain {
                return Err(Stop::Unserved(None));
            }
            Ok(program.implementation_value_head(symbol))
        }
        _ => Err(Stop::Unserved(None)),
    }
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
            return Err(Stop::Unserved(None));
        };
        let placeholders = program
            .symbols
            .child_handles(operand.parameter)
            .ok_or(Stop::Unserved(None))?;
        for placeholder in placeholders {
            let Ok(Some(requirement)) = program.evidence_requirement_declaration(placeholder)
            else {
                return Err(Stop::Unserved(None));
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
                        typed_trees::data::TypeParameterKind::Const { .. }
                    )
                });
            if !only_values
                || !requirement.lifetime_parameters.is_empty()
                || !requirement.conformance_bounds.is_empty()
            {
                return Err(Stop::Unserved(None));
            }
            let mut rows = implementation
                .rows
                .iter()
                .filter(|row| row.requirement == requirement.symbol);
            let (Some(row), None) = (rows.next(), rows.next()) else {
                return Err(Stop::Unserved(None));
            };
            if !row.realization_machine.is_valid() || !row.realization_state.is_valid() {
                return Err(Stop::Unserved(None));
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

/// A type retained by typing is used with its complete applied data view.
/// Region-bearing applications remain refused until specialization retains
/// their supplying scope. A plain authored name still admits a builtin type,
/// an open type parameter, or data with no parameters of its own.
fn named_type(
    program: &mut TypedTrees,
    argument: &StaticMachineArgument,
) -> Served<TypeReferenceHandle> {
    if argument.type_reference.is_valid() {
        let reference = argument.type_reference;
        if program
            .type_reference_table
            .contains_type_reference(reference)
            && program
                .region_free_scoped_type_identity(
                    typed_trees::type_identity::TypeIdentityRequest::ordinary(reference),
                    &[],
                )
                .is_some()
            && !matches!(
                program.type_reference_table.type_reference(reference),
                TypeReferenceNode::ConstExpression(_)
            )
            && program
                .prepare_data_application_view(reference)
                .map_err(|_| Stop::Unserved(None))?
        {
            return Ok(reference);
        }
        return Err(Stop::Unserved(None));
    }
    let only_a_name = only_a_name(program, argument) && argument.const_literal.is_none();
    let symbol = argument.symbol;
    if !only_a_name || !symbol.is_valid() || !program.symbols.symbols().nodes().is_valid(symbol) {
        return Err(Stop::Unserved(None));
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
        return Err(Stop::Unserved(None));
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

/// A declaration tree is copied as one unit: every child keeps its lexical
/// owner, and calls between siblings resolve only after all fresh symbols
/// exist. Runtime values remain explicit parameters; this supplies only the
/// enclosing declaration's already selected static context.
fn template(program: &TypedTrees, symbol: SymbolHandle) -> Served<Vec<(Machine, State)>> {
    let root = program
        .machine_by_symbol(symbol)
        .ok_or(Stop::Unserved(None))?;
    let parameters = program
        .machine_callable_parameters(root)
        .into_iter()
        .map(|parameter| (parameter.symbol, parameter))
        .collect::<symbols::SymbolMap<_>>();
    let mut pending = vec![symbol];
    let mut parts = Vec::new();
    let mut position = 0;
    while position < pending.len() {
        let declaration = pending[position];
        let (machine, state, types) = copied_parts(program, declaration)?;
        // Typing retains the enclosing binders on the child's callable
        // signature too. Those exact rows use this application; a child's
        // independent binder still needs an application of its own.
        if position != 0
            && program
                .machine_callable_parameters(&machine)
                .iter()
                .any(|parameter| parameters.get(&parameter.symbol) != Some(parameter))
        {
            return Err(Stop::Unserved(None));
        }
        if has_unserved_type_expression(program, &types) {
            return Err(Stop::Unserved(None));
        }
        for owner in [machine.symbol, state.symbol] {
            for child in program
                .symbols
                .child_handles(owner)
                .ok_or(Stop::Unserved(None))?
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
        return Err(Stop::Unserved(Some("a copied declaration has no machine")));
    };
    let [state] = program.machine_states(machine) else {
        return Err(Stop::Unserved(Some(
            "a copied declaration needs exactly one entry",
        )));
    };
    let plan = &machine.termination_plan;
    let plain = machine.body_is_present
        && machine.supply_mode == language_semantics::MachineSupplyMode::CheckedBody
        && machine.lifetime_parameters.is_empty()
        && machine.declared_type_equations.is_empty()
        && machine.owned_data.is_empty()
        && machine.satisfies.is_empty()
        && machine.invokes.is_empty()
        // The written prefix and operator spelling are diagnostic labels on
        // this same callable. Actual data attachment still needs its copier.
        && !machine.attached_data_symbol.is_valid()
        && !machine.attached_data_application.is_valid()
        && !machine.generic_data_template.is_valid()
        && machine.target.is_none()
        && !machine.target_family_interface
        && !machine.structural_type_equations_pending
        && !machine.service_reach_is_installation_bound
        && plan.checked_summary == language_semantics::TerminationGuarantee::NoGuarantee
        && program.authored_service_reach_rows_for(symbol).all(|row| {
            !row.installation_bound
                && row
                    .targets
                    .iter()
                    .all(|target| program.symbols.symbols().nodes().is_valid(target.service))
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
        return Err(Stop::Unserved(Some(
            "the declaration copier does not yet retain this machine's complete interface",
        )));
    }
    let facts = program
        .machine_contracts(machine)
        .iter()
        .chain(program.state_contracts(state))
        .flat_map(|contract| program.proof_facts.span_or_empty(contract.facts));
    let mut roots = Vec::new();
    let mut fact_types = Vec::new();
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
            return Err(Stop::Unserved(Some(
                "a copied termination witness needs its original expression custody",
            )));
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
                    && call
                        .machine_arguments
                        .iter()
                        .all(|argument| only_a_name(program, argument))
            }
            StatementNode::LocalData(local) => local.reflected_query_origin.is_none(),
            StatementNode::Assignment(_)
            | StatementNode::Expression(_)
            | StatementNode::Transition(_) => true,
        };
        if !copied {
            return Err(Stop::Unserved(Some(
                "a copied statement has unserved retained metadata",
            )));
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
            typed_trees::references::collect_expression_references(program, root).type_references,
        );
        program.subtree_for_each(root, SubtreeEdges::default(), |_handle, node| {
            copied &= match node {
                ExpressionNode::Call(call) => {
                    call.layout_query.is_none()
                        && call.native_code_layout.is_none()
                        && call.quotient_operation.is_none()
                        && call.private_layout_operation.is_none()
                        && call.installed_interrupt_operation.is_none()
                        // A Build selection is copied as written
                        // (`substitute_arguments`), whatever its operands
                        // apply.
                        && (call.build_selection.is_valid()
                            || call
                                .machine_arguments
                                .iter()
                                .all(|argument| only_a_name(program, argument)))
                }
                ExpressionNode::Cast(cast) => cast.semantic_domain_arguments.is_empty(),
                _ => true,
            };
        });
    }
    if !copied {
        return Err(Stop::Unserved(Some(
            "a copied expression has unserved retained metadata",
        )));
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
/// evaluated again here. Other embedded expressions still need an evaluator.
fn has_unserved_type_expression(program: &TypedTrees, references: &[TypeReferenceHandle]) -> bool {
    let mut staging = TypedTreeTables::default();
    let mut embedded = Vec::new();
    for reference in references.iter().filter(|reference| reference.is_valid()) {
        staging.copy_type_reference_with_expressions(&program.tables, *reference, &mut embedded);
    }
    embedded.iter().any(|copied| {
        program
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
    if let language_semantics::TerminationInterface::Published(
        language_semantics::TerminationGuarantee::Terminates { premises },
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
        program
            .static_operand_from_type_reference(copied.source)
            .is_err()
    }) {
        return Err(Stop::Unserved(None));
    }
    let type_start = program.type_reference_table.type_reference_count() + 1;
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
            ),
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
        let parameters = program.state_parameters(source_state).to_vec();
        let locals = program
            .statement_table
            .statements(source_state.statement_nodes)
            .iter()
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
        // A written empty ceiling differs from an omitted one. Retain each
        // authored row on its actual copied owner; never turn it into inference.
        let reach_rows = program
            .authored_service_reach_rows_for(source.symbol)
            .cloned()
            .map(|mut row| {
                row.owner = machine.symbol;
                for target in &mut row.targets {
                    target.service = symbols.lookup(target.service).unwrap_or(target.service);
                }
                row
            })
            .collect::<Vec<_>>();
        program.authored_service_reach_rows.extend(reach_rows);
        machine.type_parameters = HandleSpan::empty();
        machine.conformance_bounds.clear();
        machine.contracts = HandleSpan::empty();
        machine.states = HandleSpan::empty();
        for contract in program.machine_contracts(source).to_vec() {
            let contract = program
                .copy_callable_contract(&contract, symbols, &contract_substitutions, source.symbol)
                .map_err(|reason| Stop::Unserved(Some(reason)))?;
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
                .map_err(|reason| Stop::Unserved(Some(reason)))?;
            program.push_state_contract(&mut state, contract);
        }
        let expression_count = program.expression_table.expression_count() + 1 - first_expression;
        copied.push((machine, state, first_expression, expression_count));
    }
    let substitutions = operands
        .iter()
        .filter_map(|operand| match operand.value {
            StaticMachineBindingValue::Type(actual) => Some((operand.parameter, actual)),
            _ => None,
        })
        .collect::<Vec<_>>();
    let entries = copied
        .iter_mut()
        .zip(&parts)
        .map(|((_, state, first, count), (source, _))| (source.symbol, state, *first, *count));
    program
        .apply_copied_callable_types(entries, symbols, &substitutions)
        .map_err(Stop::Defect)?;
    substitute_widths(program, type_start, operands)?;
    substitute_values(program, expression_start, operands);
    for (mut machine, state, first_expression, expression_count) in copied {
        select_requirements(
            program,
            first_expression,
            expression_count,
            state.statement_nodes,
            &selections,
        );
        substitute_arguments(
            program,
            first_expression,
            expression_count,
            state.statement_nodes,
            machine.symbol,
            operands,
        );
        program.push_machine_state(&mut machine, state);
        program.push_machine(machine);
    }
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
    let copied = copy_declarations(program, template_symbol, operands)?;
    // Each nested declaration is specialized by the actual enclosing
    // application's bindings, not by an invented call or runtime capture.
    for (source, state) in &copied.originals {
        let of_kind = |implementation: bool| {
            operands.iter().filter(move |operand| {
                matches!(operand.value, StaticMachineBindingValue::Conformance(_)) == implementation
            })
        };
        program.machine_specializations.push(MachineSpecialization {
            template: source.symbol,
            instance: copied_symbol(&copied.symbols, source.symbol)?,
            template_parameters: copied.originals[0].0.type_parameters,
            binding_application: application,
            states: program.captured_machine_states.insert_many([
                typed_trees::CapturedMachineState {
                    template: state.symbol,
                    instance: copied_symbol(&copied.symbols, state.symbol)?,
                },
            ]),
            conformance_arguments: of_kind(true).filter_map(Operand::declaration).collect(),
            ..MachineSpecialization::default()
        });
    }
    Ok(Generated {
        template: template_symbol,
        arguments: operands
            .iter()
            .map(|operand| operand.identity.clone())
            .collect(),
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
                matches!(
                    table.type_reference(*argument),
                    TypeReferenceNode::Named { symbol, .. } if *symbol == operand.parameter
                )
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

/// Points each copied call through an implementation parameter at the
/// operation the implementation selected for that requirement. The call
/// keeps the binding the author wrote, which still names the parameter's
/// requirement, and drops the qualifier that named the parameter: the
/// generated machine has no parameter to qualify by.
fn select_requirements(
    program: &mut TypedTrees,
    expression_start: usize,
    expression_count: usize,
    statements: HandleSpan<StatementNode>,
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
        .iter_expressions_from(expression_start)
        .take(expression_count)
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
    for handle in statements.handles() {
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

/// Replaces each parameter a copied call passes on as a static argument
/// (`forward<T>` in the body of `relay`) by the operand's own argument. The
/// copied application is then unbound again, with this machine as its
/// holder, so it is bound and served from its new arguments like an
/// authored one.
///
/// A Build selection, `Build::select_provider<Reading<Tag>, Provider<Tag>>(..)`
/// in a helper generic in `Tag`, is left exactly as written. Its operands
/// are not static arguments of a machine to generate: they are the request
/// the call was authored with, which names the helper's parameters by their
/// declarations. provider_selection.md, "Declaration and choice": "Helpers
/// forward that argument", and a selection keeps the declaration that wrote
/// it as its author. So the generated machine holds the authored request,
/// and whoever reads the selection takes the value of each parameter from
/// this machine's application (`MachineSpecialization::binding_application`,
/// read by `typed_trees::product_operand::capture_static_environment`).
fn substitute_arguments(
    program: &mut TypedTrees,
    expression_start: usize,
    expression_count: usize,
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
        .take(expression_count)
        .filter(|(_, node)| matches!(node, ExpressionNode::Call(_)))
        .map(|(handle, _)| handle)
        .collect::<Vec<_>>();
    for handle in calls {
        if let ExpressionNode::Call(call) = program.expression_table.expression_mut(handle)
            && !call.build_selection.is_valid()
        {
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
        table
            .static_machine_application_mut(application)
            .lexical_owner = holder;
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod implementation_tests;
