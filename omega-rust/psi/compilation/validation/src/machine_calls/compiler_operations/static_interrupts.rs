//! Identify the core installation operations before deriving their arguments.
//! A recognized declaration still owes its exact installed-root association;
//! recognition alone cannot supply callback authority or context backing.

use crate::CheckingInput;
use language_core::static_interrupts::StaticInterruptOperation;
use language_semantics::{MachineSupplyMode, ReferenceAccess};
use symbols::SymbolHandle;
use typed_trees::data::{MachineParameterContract, TypeParameter, TypeParameterKind};
use typed_trees::machine::Machine;
use typed_trees::signature::SignatureContractKind;
use typed_trees::static_applications::StaticMachineBindingValue;
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

const DECLARATIONS: &[u8] = language_core::static_interrupts::CORE_DECLARATIONS;
const INTERRUPT_DECLARATIONS: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../source/library/core/interrupt.omg"
));

#[derive(Debug, Clone, Copy)]
pub struct StaticInterruptDeclaration {
    pub operation: StaticInterruptOperation,
    pub declaration: SymbolHandle,
    pub entry: SymbolHandle,
}

impl StaticInterruptDeclaration {
    /// On the only normal exit, installation has left input descriptors intact.
    /// Private address materialization writes scalar bits; validation borrows
    /// the table shared, and successful Work never returns. This promises no
    /// scalar purity, storage geometry or permission to publish the table.
    pub fn preserves_parameter_descriptors(self) -> bool {
        self.operation == StaticInterruptOperation::Install
    }
}

/// The returned descriptor comes from this invocation's installed context.
/// The acknowledgement limits its lifetime; it does not contain the referent.
/// Installation admission still owes the exact root and live occurrence.
#[derive(Debug, Clone, Copy)]
pub struct StaticInterruptContext {
    pub declaration: SymbolHandle,
    pub entry: SymbolHandle,
    pub referent: TypeReferenceHandle,
    pub acknowledgement: TypeReferenceHandle,
}

pub fn static_interrupt_context(
    program: &dyn CheckingInput,
    target: SymbolHandle,
) -> Result<Option<StaticInterruptContext>, &'static str> {
    let Some(machine) = program
        .machine_by_symbol(target)
        .or_else(|| program.machine_holding_state(target))
    else {
        return Ok(None);
    };
    let Some(declaration) = static_interrupt_declaration(program, machine)? else {
        return Ok(None);
    };
    if declaration.operation == StaticInterruptOperation::Context
        && std::env::var_os("OMEGA_COMPILER_TRACE_INSTALLED_CONTEXT").is_some()
    {
        eprintln!(
            "INSTALLED_CONTEXT query target={:?} machine={:?} declaration={:?} specialization={:?} open_parameters={} return={:?}",
            target,
            machine.symbol,
            declaration.declaration,
            program
                .machine_specialization(machine.symbol)
                .map(|application| (
                    application.template,
                    application.instance,
                    application.binding_application
                )),
            machine.has_specialization_parameters(),
            program.machine_states(machine).first().map(|state| program
                .type_reference_table
                .type_reference(state.return_type))
        );
    }
    if declaration.operation != StaticInterruptOperation::Context
        || program.machine_specialization(machine.symbol).is_none()
    {
        return Ok(None);
    }
    if machine.has_specialization_parameters() {
        return Err("installed context requires a complete closed type application");
    }
    let state = &program.machine_states(machine)[0];
    let (referent, _) = shared_reference(program, state.return_type)?;
    let closed = context_is_closed(program, referent);
    if std::env::var_os("OMEGA_COMPILER_TRACE_INSTALLED_CONTEXT").is_some() {
        eprintln!(
            "INSTALLED_CONTEXT referent={:?} closed={closed}",
            program.type_reference_table.type_reference(referent)
        );
    }
    if !closed {
        return Ok(None);
    }
    Ok(Some(StaticInterruptContext {
        declaration: declaration.declaration,
        entry: state.symbol,
        referent,
        acknowledgement: program.state_parameters(state)[0].type_reference,
    }))
}

pub fn static_interrupt_declaration(
    program: &dyn CheckingInput,
    machine: &Machine,
) -> Result<Option<StaticInterruptDeclaration>, &'static str> {
    let original = program
        .machine_specialization(machine.symbol)
        .map_or(machine.symbol, |application| application.template);
    if program.symbol_toolchain_relative_path(original)
        != Some(std::path::Path::new("static_interrupt.omg"))
    {
        return Ok(None);
    }
    authenticate_source(program, original, DECLARATIONS)?;
    let identity = program
        .normalized_hermetic_symbol_identity(original)
        .map_err(|_| "static interrupt operation lost its exact core declaration")?;
    let Some(operation) = identity
        .strip_prefix("toolchain::")
        .and_then(|name| StaticInterruptOperation::from_core_declaration(name, DECLARATIONS))
    else {
        return Ok(None);
    };
    let template = program
        .machine_by_symbol(original)
        .ok_or("static interrupt application lost its original declaration")?;
    authenticate_shape(program, template, operation)?;
    if template.symbol != machine.symbol {
        authenticate_shape(program, machine, operation)?;
    }
    Ok(Some(StaticInterruptDeclaration {
        operation,
        declaration: original,
        entry: program.machine_states(machine)[0].symbol,
    }))
}

fn authenticate_shape(
    program: &dyn CheckingInput,
    machine: &Machine,
    operation: StaticInterruptOperation,
) -> Result<(), &'static str> {
    for (invalid, reason) in [
        (
            machine.body_is_present,
            "static interrupt declaration unexpectedly has a body",
        ),
        (
            machine.supply_mode != MachineSupplyMode::Boundary,
            "static interrupt declaration is not a boundary operation",
        ),
        (
            machine.suspends || machine.blocks,
            "static interrupt declaration unexpectedly suspends or blocks",
        ),
        (
            machine.spelling.is_some(),
            "static interrupt declaration unexpectedly has a foreign spelling",
        ),
        (
            machine.target.is_some()
                || machine.authored_target.is_some()
                || machine.target_family_interface,
            "static interrupt declaration unexpectedly has a source target selection",
        ),
        (
            machine.purpose != typed_trees::machine::MachinePurpose::Runtime,
            "static interrupt declaration is not a runtime operation",
        ),
        (
            !machine.owned_data.is_empty(),
            "static interrupt declaration unexpectedly owns local data",
        ),
        (
            !machine.conformance_bounds.is_empty(),
            "static interrupt declaration unexpectedly has conformance bounds",
        ),
        (
            !program.machine_trait_conformances(machine).is_empty(),
            "static interrupt declaration unexpectedly implements a trait",
        ),
        (
            machine.admitted_same_stack_bytes != 0,
            "static interrupt declaration unexpectedly supplies a stack bound",
        ),
        (
            machine.service_reach_is_installation_bound,
            "static interrupt declaration unexpectedly has installation-bound service reach",
        ),
    ] {
        if invalid {
            return Err(reason);
        }
    }
    let [state] = program.machine_states(machine) else {
        return Err("static interrupt operation must retain one entry signature");
    };
    let parameters = program.state_parameters(state);
    if parameters
        .iter()
        .any(|parameter| parameter.is_const || parameter.relevance.is_erased())
    {
        return Err("static interrupt operation must retain all runtime arguments");
    }
    if !state.statement_nodes.is_empty() {
        return Err("static interrupt boundary entry cannot contain executable statements");
    }
    let telescope = template_parameters(program, machine);
    if operation != StaticInterruptOperation::Install
        && (!machine.invokes.is_empty()
            || !program
                .service_reach_rows
                .services(machine.service_reach_row)
                .is_empty()
            || machine.attached_data_symbol.is_valid())
    {
        return Err("installed context and completion helpers must retain their empty effects");
    }
    match operation {
        StaticInterruptOperation::Install => {
            let [table, gdt, task_state, stacks, arrivals, context] = parameters else {
                return Err(
                    "interrupt installation requires table, GDT, task state, stacks, arrivals and context",
                );
            };
            for parameter in [table, gdt, stacks] {
                require_reference(program, parameter.type_reference, ReferenceAccess::Mutable)?;
            }
            for parameter in [task_state, arrivals, context] {
                require_reference(program, parameter.type_reference, ReferenceAccess::Shared)?;
            }
        }
        StaticInterruptOperation::Context | StaticInterruptOperation::BeginCompletion => {
            let [acknowledgement] = parameters else {
                return Err(
                    "installed interrupt context and completion operations require one acknowledgement borrow",
                );
            };
            let (acknowledgement_type, input_lifetime) =
                shared_reference(program, acknowledgement.type_reference)?;
            require_core_data(
                program,
                acknowledgement_type,
                "interrupt.omg",
                "toolchain::InterruptAcknowledgement",
                INTERRUPT_DECLARATIONS,
            )?;
            require_pending(program, machine, state, acknowledgement.symbol)?;
            if operation == StaticInterruptOperation::Context {
                let [context] = telescope else {
                    return Err("installed context requires exactly one type argument");
                };
                if !matches!(context.kind, TypeParameterKind::Type)
                    || machine.lifetime_parameters.len() != 1
                {
                    return Err("installed context changed its type or arrival lifetime binder");
                }
                let (referent, output_lifetime) = shared_reference(program, state.return_type)?;
                let (Some(input), Some(output)) = (input_lifetime, output_lifetime) else {
                    return Err("installed context requires explicit input and result lifetimes");
                };
                let input = typed_trees::type_identity::lifetime_parameter_identity(
                    program.typed_program(),
                    machine.symbol,
                    input.as_str(),
                );
                let output = typed_trees::type_identity::lifetime_parameter_identity(
                    program.typed_program(),
                    machine.symbol,
                    output.as_str(),
                );
                if input.is_none() || input != output {
                    return Err(
                        "installed context must borrow for the acknowledgement's arrival lifetime",
                    );
                }
                require_context_type(program, machine, context, referent)?;
            } else {
                if !machine.lifetime_parameters.is_empty() || input_lifetime.is_some() {
                    return Err(
                        "completion begin changed its acknowledgement lifetime declaration",
                    );
                }
                require_completion_parameter(program, telescope)?;
                require_core_data(
                    program,
                    state.return_type,
                    "static_interrupt.omg",
                    "toolchain::StaticInterruptCompletionPermit",
                    DECLARATIONS,
                )?;
            }
        }
        StaticInterruptOperation::FinishCompletion => {
            let [permit] = parameters else {
                return Err("installed interrupt completion requires one consumed permit");
            };
            if !telescope.is_empty()
                || !machine.lifetime_parameters.is_empty()
                || !machine.contracts.is_empty()
                || !state.contracts.is_empty()
                || !matches!(
                    program
                        .type_reference_table
                        .type_reference(state.return_type),
                    TypeReferenceNode::Unit
                )
            {
                return Err(
                    "completion finish must consume its permit and return Unit without contracts",
                );
            }
            require_core_data(
                program,
                permit.type_reference,
                "static_interrupt.omg",
                "toolchain::StaticInterruptCompletionPermit",
                DECLARATIONS,
            )?;
        }
    }
    Ok(())
}

fn template_parameters<'a>(
    program: &'a dyn CheckingInput,
    machine: &'a Machine,
) -> &'a [TypeParameter] {
    program
        .machine_specialization(machine.symbol)
        .map(|application| {
            program
                .data_type_parameters
                .span_or_empty(application.template_parameters)
        })
        .unwrap_or_else(|| program.machine_type_parameters(machine))
}

fn context_is_closed(program: &dyn CheckingInput, referent: TypeReferenceHandle) -> bool {
    let mut pending = vec![referent];
    let mut visited = Vec::new();
    while let Some(reference) = pending.pop() {
        if visited.contains(&reference) {
            continue;
        }
        visited.push(reference);
        for symbol in typed_trees::references::collect_type_symbol_dependencies(
            program.typed_program(),
            reference,
        ) {
            if !symbol.is_valid() {
                continue;
            }
            if matches!(
                program.symbols.get(symbol).kind,
                symbols::SymbolKind::TypeParameter
                    | symbols::SymbolKind::MachineParameter
                    | symbols::SymbolKind::ConformanceParameter
                    | symbols::SymbolKind::Parameter
                    | symbols::SymbolKind::Local
            ) {
                return false;
            }
            if let Some(application) = program
                .data_definition_by_symbol(symbol)
                .and_then(|data| data.generic_instance)
            {
                pending.push(application);
            }
        }
    }
    true
}

fn require_context_type(
    program: &dyn CheckingInput,
    machine: &Machine,
    context: &TypeParameter,
    referent: TypeReferenceHandle,
) -> Result<(), &'static str> {
    if let Some(application) = program.machine_specialization(machine.symbol) {
        let Some(StaticMachineBindingValue::Type(actual)) =
            program.specialization_binding(application, context.symbol)
        else {
            return Err("installed context lost its exact retained type argument");
        };
        let bindings = program
            .expression_table
            .static_machine_application_bindings(application.binding_application);
        if bindings.len() != 1 || application.instance != machine.symbol {
            return Err("installed context has an incomplete or foreign specialization tuple");
        }
        let owner = program
            .expression_table
            .static_machine_application(application.binding_application)
            .lexical_owner;
        let expected = typed_trees::type_identity::applied_type_identity(
            program.typed_program(),
            *actual,
            owner,
            &[],
        );
        let observed = typed_trees::type_identity::applied_type_identity(
            program.typed_program(),
            referent,
            owner,
            &[],
        );
        if expected.is_none() || expected != observed {
            return Err("installed context result differs from its exact type argument");
        }
    } else if !matches!(program.type_reference_table.type_reference(referent), TypeReferenceNode::Named { symbol, .. } if *symbol == context.symbol)
    {
        return Err("installed context result must refer to its declared Context binder");
    }
    Ok(())
}

fn shared_reference(
    program: &dyn CheckingInput,
    reference: TypeReferenceHandle,
) -> Result<(TypeReferenceHandle, Option<&typed_trees::name::Identifier>), &'static str> {
    match program.type_reference_table.type_reference(reference) {
        TypeReferenceNode::Reference {
            referee,
            access: ReferenceAccess::Shared,
            lifetime,
        } => Ok((*referee, lifetime.as_ref())),
        _ => Err("installed helper requires an unqualified shared reference"),
    }
}

fn authenticate_source(
    program: &dyn CheckingInput,
    symbol: SymbolHandle,
    bytes: &[u8],
) -> Result<(), &'static str> {
    let source = program
        .symbols
        .symbol_source_span(symbol)
        .and_then(|span| program.symbols.source_file(span))
        .ok_or("static interrupt operation lost its core source")?;
    if source.origin != source::SourceOrigin::Toolchain
        || source.package_identity.is_some()
        || source.source.as_bytes() != bytes
    {
        return Err("static interrupt operation does not match the compiler's core declarations");
    }
    Ok(())
}

fn require_core_data(
    program: &dyn CheckingInput,
    reference: TypeReferenceHandle,
    path: &str,
    identity: &str,
    bytes: &[u8],
) -> Result<(), &'static str> {
    let TypeReferenceNode::Named { symbol, .. } =
        program.type_reference_table.type_reference(reference)
    else {
        return Err("installed helper requires its exact unqualified core token type");
    };
    require_core_symbol(program, *symbol, path, identity)?;
    authenticate_source(program, *symbol, bytes)?;
    let data = program
        .data_definition_by_symbol(*symbol)
        .ok_or("installed helper token lost its data declaration")?;
    if data.supply_mode != language_semantics::DataSupplyMode::BoundaryOpaque
        || data.properties.multiplicity != language_semantics::Multiplicity::Linear
        || !data.members.is_empty()
        || !data.type_parameters.is_empty()
        || !data.lifetime_parameters.is_empty()
    {
        return Err("installed helper token must retain its opaque linear core declaration");
    }
    Ok(())
}

fn require_core_symbol(
    program: &dyn CheckingInput,
    symbol: SymbolHandle,
    path: &str,
    identity: &str,
) -> Result<(), &'static str> {
    if program.symbol_toolchain_relative_path(symbol) != Some(std::path::Path::new(path))
        || program
            .normalized_hermetic_symbol_identity(symbol)
            .ok()
            .as_deref()
            != Some(identity)
    {
        return Err("installed helper selected a different core declaration");
    }
    Ok(())
}

fn require_pending(
    program: &dyn CheckingInput,
    machine: &Machine,
    state: &typed_trees::state::State,
    acknowledgement: SymbolHandle,
) -> Result<(), &'static str> {
    let mut facts = program
        .machine_contracts(machine)
        .iter()
        .chain(program.state_contracts(state))
        .map(|contract| {
            if !matches!(contract.kind, SignatureContractKind::Requires)
                || contract.binding.is_some()
            {
                return Err("installed helper changed its Pending-only entry contract");
            }
            Ok(program.proof_facts.span_or_empty(contract.facts))
        });
    let mut count = 0;
    for contract in &mut facts {
        for fact in contract? {
            let typed_trees::domain::ProofFact::Membership(membership) = fact else {
                return Err(
                    "installed helper requires only its acknowledgement's Pending membership",
                );
            };
            require_core_symbol(
                program,
                membership.domain_symbol,
                "interrupt.omg",
                "toolchain::InterruptAcknowledgement::Pending",
            )?;
            authenticate_source(program, membership.domain_symbol, INTERRUPT_DECLARATIONS)?;
            let typed_trees::expression::ExpressionNode::Name(subject) =
                program.expression_table.expression(membership.value)
            else {
                return Err("installed helper Pending contract must name its acknowledgement");
            };
            if subject.resolved_symbol() != Some(acknowledgement)
                || !membership.domain_arguments.is_empty()
            {
                return Err("installed helper Pending contract names another value or application");
            }
            count += 1;
        }
    }
    if count != 1 {
        return Err("installed helper must retain exactly one Pending precondition");
    }
    Ok(())
}

fn require_completion_parameter(
    program: &dyn CheckingInput,
    parameters: &[TypeParameter],
) -> Result<(), &'static str> {
    let [completion] = parameters else {
        return Err("completion begin requires exactly one completion function");
    };
    let TypeParameterKind::Machine {
        contract:
            MachineParameterContract::Nominal {
                receiving_trait,
                receiving_arguments,
                trait_definition,
                requirement,
                trait_arguments,
                instantiated,
            },
    } = &completion.kind
    else {
        return Err("completion begin requires the core nominal completion contract");
    };
    require_core_symbol(
        program,
        *trait_definition,
        "interrupt.omg",
        "toolchain::InterruptAcknowledgementCompletion",
    )?;
    require_core_symbol(
        program,
        *requirement,
        "interrupt.omg",
        "toolchain::InterruptAcknowledgementCompletion::complete",
    )?;
    if receiving_trait != trait_definition
        || !receiving_arguments.is_empty()
        || !trait_arguments.is_empty()
        || instantiated.is_some()
    {
        return Err("completion begin changed its complete nominal completion requirement");
    }
    Ok(())
}

fn require_reference(
    program: &dyn CheckingInput,
    reference: typed_trees::types::TypeReferenceHandle,
    expected: ReferenceAccess,
) -> Result<(), &'static str> {
    let reference =
        typed_trees::operator::unconstrained_type_reference(program.typed_program(), reference)
            .ok_or("static interrupt argument lost its reference carrier")?;
    match program.type_reference_table.type_reference(reference) {
        TypeReferenceNode::Reference { access, .. } if *access == expected => Ok(()),
        _ => Err("static interrupt argument changed its required borrowed access"),
    }
}
