/*
    Callable parameter types originate at exact selected static-argument uses.

    Declaration inference reads ordinary resolved calls, never current callers'
    implementations. A finite work list propagates an originating nominal route
    through forwarding binders. Every call remains in the ordinary body checker:
    its substituted argument contract must satisfy that published bound. Authored
    bounds are never rewritten, and an origin-free cycle publishes nothing.
*/

use arena::Handle;
use diagnostics::Diagnostic;
use source::SourceSpan;
use symbol_resolved_trees::SymbolResolvedTrees;
use symbol_resolved_trees::data::{MachineParameterContract, TypeParameter, TypeParameterKind};
use symbol_resolved_trees::expression::{ExpressionNode, StaticMachineArgument};
use symbol_resolved_trees::statement::Statement;
use symbol_resolved_trees::types::TypeReference;
use symbols::{SymbolHandle, SymbolKind};

pub(crate) fn pending(parameter: &TypeParameter) -> bool {
    matches!(
        parameter.kind,
        TypeParameterKind::Machine {
            contract: MachineParameterContract::Inferred
        }
    )
}

struct Use {
    receiving: Handle<TypeParameter>,
    forwarded: Handle<TypeParameter>,
    arguments: Vec<(SymbolHandle, TypeReference)>,
    source: SourceSpan,
}

struct Parameters {
    by_symbol: Vec<Option<(SymbolHandle, Handle<TypeParameter>)>>,
    inferred: Vec<bool>,
}

impl Parameters {
    fn new(program: &SymbolResolvedTrees) -> Self {
        let mut by_symbol = Vec::new();
        let mut inferred = vec![false; program.tables.declarations.data_type_parameters.len()];
        for (handle, parameter) in program.tables.declarations.data_type_parameters.iter() {
            if !parameter.symbol.is_valid() {
                continue;
            }
            let index = parameter.symbol.arena_index() as usize;
            if by_symbol.len() <= index {
                by_symbol.resize(index + 1, None);
            }
            by_symbol[index] = Some((parameter.symbol, handle));
            inferred[handle.arena_index() as usize] = pending(parameter);
        }
        Self {
            by_symbol,
            inferred,
        }
    }

    fn parameter(&self, symbol: SymbolHandle) -> Option<Handle<TypeParameter>> {
        self.by_symbol
            .get(symbol.arena_index() as usize)
            .copied()
            .flatten()
            .filter(|(selected, _)| *selected == symbol)
            .map(|(_, handle)| handle)
    }

    fn inferred(&self, symbol: SymbolHandle) -> Option<Handle<TypeParameter>> {
        let handle = self.parameter(symbol)?;
        self.inferred[handle.arena_index() as usize].then_some(handle)
    }
}

pub(crate) fn infer(program: &mut SymbolResolvedTrees) -> Result<(), Vec<Diagnostic>> {
    if !program
        .tables
        .declarations
        .data_type_parameters
        .iter()
        .any(|(_, parameter)| pending(parameter))
    {
        return Ok(());
    }
    let parameters = Parameters::new(program);
    let mut uses = Vec::new();
    for (_, expression) in program.tables.bodies.expressions.iter_expressions() {
        if let ExpressionNode::Call(call) = expression {
            collect_use(
                program,
                &parameters,
                call.target_symbol,
                &call.machine_arguments,
                call.target.source_span(),
                &mut uses,
            )
            .map_err(|error| vec![error])?;
        }
    }
    for (_, statement) in program.tables.declarations.state_statements.iter() {
        if let Statement::Call(call) = statement {
            collect_use(
                program,
                &parameters,
                call.target_symbol,
                &call.machine_arguments,
                call.target.source_span(),
                &mut uses,
            )
            .map_err(|error| vec![error])?;
        }
    }

    let mut dependents = vec![Vec::new(); parameters.inferred.len()];
    let mut ready = Vec::new();
    for (index, usage) in uses.iter().enumerate() {
        let source = usage.receiving.arena_index() as usize;
        if parameters.inferred[source] {
            dependents[source].push(index);
        } else {
            ready.push(index);
        }
    }
    let mut cursor = 0;
    while cursor < ready.len() {
        let usage = &uses[ready[cursor]];
        cursor += 1;
        let receiving = program
            .tables
            .declarations
            .data_type_parameters
            .get(usage.receiving);
        let TypeParameterKind::Machine { contract } = &receiving.kind else {
            return Err(vec![error(
                usage.source,
                "a forwarded callable requires a callable parameter type at its selected use",
            )]);
        };
        let MachineParameterContract::Nominal {
            trait_definition,
            requirement,
            authored_path,
            trait_arguments,
        } = contract
        else {
            let message = match contract {
                MachineParameterContract::Structural(_) => {
                    "inference from structural callable parameter types is not implemented"
                }
                MachineParameterContract::RequirementIdentity => {
                    "a CallableInfo identity is not an originating callable parameter type"
                }
                _ => "a forwarded callable use lost its originating parameter type",
            };
            return Err(vec![error(usage.source, message)]);
        };
        let (trait_definition, requirement, authored_path, trait_arguments) = (
            *trait_definition,
            *requirement,
            authored_path.clone(),
            *trait_arguments,
        );
        let destination = program
            .tables
            .declarations
            .data_type_parameters
            .get(usage.forwarded);
        if !pending(destination) {
            let TypeParameterKind::Machine {
                contract:
                    MachineParameterContract::Nominal {
                        trait_definition: previous_trait,
                        requirement: previous_requirement,
                        ..
                    },
            } = &destination.kind
            else {
                return Err(vec![error(
                    usage.source,
                    "an inferred callable lost its published nominal route",
                )]);
            };
            if *previous_trait != trait_definition || *previous_requirement != requirement {
                return Err(vec![error(
                    usage.source,
                    format!(
                        "inferred callable parameter `{}` has uses requiring different nominal requirement types; one parameter carries one nominal route",
                        destination.name
                    ),
                )]);
            }
            // Ordinary argument checking compares every substituted trait tuple
            // against this bound, including additional uses of the same route.
            continue;
        }
        let trait_arguments = program.copy_type_arguments(trait_arguments, &usage.arguments);
        program
            .tables
            .declarations
            .data_type_parameters
            .get_mut(usage.forwarded)
            .kind = TypeParameterKind::Machine {
            contract: MachineParameterContract::Nominal {
                trait_definition,
                requirement,
                authored_path,
                trait_arguments,
            },
        };
        ready.extend_from_slice(&dependents[usage.forwarded.arena_index() as usize]);
    }
    let unresolved = program.tables.declarations.data_type_parameters.iter()
        .filter(|(_, parameter)| pending(parameter))
        .map(|(_, parameter)| error(parameter.name.source_span(), format!(
            "inferred callable parameter `{}` has no exact originating callable type; bare calls and forwarding cycles cannot supply one",
            parameter.name
        ))).collect::<Vec<_>>();
    if unresolved.is_empty() {
        Ok(())
    } else {
        Err(unresolved)
    }
}

fn collect_use(
    program: &SymbolResolvedTrees,
    parameters: &Parameters,
    target: SymbolHandle,
    arguments: &[StaticMachineArgument],
    source: SourceSpan,
    uses: &mut Vec<Use>,
) -> Result<(), Diagnostic> {
    if !arguments
        .iter()
        .any(|argument| parameters.inferred(argument.symbol).is_some())
    {
        return Ok(());
    }
    let selected = target.is_valid().then(|| program.symbols.get(target));
    let owner = selected
        .filter(|symbol| symbol.kind == SymbolKind::State)
        .map_or(target, |symbol| symbol.parent);
    let telescope = program
        .machine(owner)
        .map(|machine| machine.type_parameters)
        .or_else(|| {
            parameters.parameter(target).and_then(|handle| {
                let TypeParameterKind::Machine { contract } = &program
                    .tables
                    .declarations
                    .data_type_parameters
                    .get(handle)
                    .kind
                else {
                    return None;
                };
                program
                    .machine_parameter_contract_view(contract)
                    .map(|view| view.signature().type_parameters)
            })
        })
        .ok_or_else(|| {
            error(
                source,
                "an inferred callable use requires an exact selected callee telescope",
            )
        })?;
    let formals = program.data_type_parameters(telescope);
    if formals.len() != arguments.len() {
        return Err(error(
            source,
            "an inferred callable use requires the selected callee's complete static argument tuple",
        ));
    }
    let mut substitutions = Vec::new();
    for (formal, actual) in formals.iter().zip(arguments) {
        match formal.kind {
            TypeParameterKind::Type => {
                substitutions.push((formal.symbol, argument_type(program, actual, source)?))
            }
            TypeParameterKind::Const { .. } | TypeParameterKind::Value { .. } => {
                return Err(error(
                    source,
                    "inferred callable types with value-parameter substitutions are not implemented",
                ));
            }
            _ => {}
        }
    }
    for (formal, actual) in formals.iter().zip(arguments) {
        let Some(forwarded) = parameters.inferred(actual.symbol) else {
            continue;
        };
        if actual.application.is_some()
            || actual.evidence_projection.is_some()
            || actual.type_reference.is_valid()
        {
            return Err(error(
                source,
                "callable type inference requires a forwarded parameter identity, not an applied selection",
            ));
        }
        let receiving = parameters.parameter(formal.symbol).ok_or_else(|| {
            error(
                source,
                "an inferred callable use lost its exact receiving parameter",
            )
        })?;
        uses.push(Use {
            receiving,
            forwarded,
            arguments: substitutions.clone(),
            source,
        });
    }
    Ok(())
}

fn argument_type(
    program: &SymbolResolvedTrees,
    argument: &StaticMachineArgument,
    source: SourceSpan,
) -> Result<TypeReference, Diagnostic> {
    if argument.type_reference.is_valid() {
        let reference = program
            .tables
            .declarations
            .child_type_references
            .get(argument.type_reference);
        if matches!(reference, TypeReference::Named { symbol, .. } if symbol.is_valid()) {
            return Ok(reference.clone());
        }
        return Err(error(
            source,
            "substituting a compound type into an inferred callable contract is not implemented",
        ));
    }
    if argument.application.is_none()
        && argument.const_literal.is_none()
        && argument.evidence_projection.is_none()
        && argument.symbol.is_valid()
        && let Some(name) = argument.path.last()
    {
        return Ok(TypeReference::Named {
            symbol: argument.symbol,
            name: name.clone(),
            supplied_operand_owner: Default::default(),
        });
    }
    Err(error(
        source,
        "substituting this static operand into an inferred nominal callable type is not implemented",
    ))
}

fn error(source: SourceSpan, message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(message).with_source_span(source)
}
