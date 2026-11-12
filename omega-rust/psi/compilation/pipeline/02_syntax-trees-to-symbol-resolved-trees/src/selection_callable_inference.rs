/*
    Callable parameter types originate at exact selected static-argument uses.

    Declaration inference reads ordinary resolved calls, never current callers'
    implementations. A finite work list propagates an originating nominal route
    through forwarding binders. Every call remains in the ordinary body checker:
    its substituted argument contract must satisfy that published bound. Authored
    bounds are never rewritten, and an origin-free cycle publishes nothing.
*/

use arena::{Handle, HandleSpan};
use diagnostics::Diagnostic;
use source::SourceSpan;
use symbol_resolved_trees::SymbolResolvedTrees;
use symbol_resolved_trees::data::{MachineParameterContract, TypeParameter, TypeParameterKind};
use symbol_resolved_trees::expression::{ExpressionNode, StaticMachineArgument};
use symbol_resolved_trees::statement::Statement;
use symbol_resolved_trees::types::{FixedArrayLength, TypeReference};
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
    let mut diagnostics = Vec::new();
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
        let trait_arguments = program.copy_type_arguments(trait_arguments, &usage.arguments);
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
                        trait_arguments: previous_arguments,
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
                diagnostics.push(error(
                    usage.source,
                    format!(
                        "inferred callable parameter `{}` has uses requiring different nominal requirement types; one parameter carries one nominal route",
                        destination.name
                    ),
                ));
                continue;
            }
            match same_type_arguments(program, *previous_arguments, trait_arguments, usage.source) {
                Ok(true) => {}
                Ok(false) => diagnostics.push(error(usage.source, format!(
                    "inferred callable parameter `{}` has conflicting substituted nominal arguments for `{}`: published [{}], required by this use [{}]",
                    destination.name,
                    program.symbols.display_path(requirement, "::"),
                    argument_names(program, *previous_arguments),
                    argument_names(program, trait_arguments),
                ))),
                Err(reason) => diagnostics.push(reason),
            }
            continue;
        }
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
    diagnostics.extend(program.tables.declarations.data_type_parameters.iter()
        .filter(|(_, parameter)| pending(parameter))
        .map(|(_, parameter)| error(parameter.name.source_span(), format!(
            "inferred callable parameter `{}` has no exact originating callable type; bare calls and forwarding cycles cannot supply one",
            parameter.name
        ))));
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
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

/*
    Compare the selected type graph, ignoring copied arena coordinates and
    diagnostic spellings. Unelaborated domain/value/lifetime forms need their
    typed identity owner; this early nominal inference route refuses them instead
    of publishing whichever use happened to be visited first.
*/
fn same_type_arguments(
    program: &SymbolResolvedTrees,
    left: HandleSpan<TypeReference>,
    right: HandleSpan<TypeReference>,
    source: SourceSpan,
) -> Result<bool, Diagnostic> {
    let mut pending = Vec::new();
    fn span(
        pending: &mut Vec<(TypeReference, TypeReference)>,
        program: &SymbolResolvedTrees,
        left: HandleSpan<TypeReference>,
        right: HandleSpan<TypeReference>,
    ) -> bool {
        let left = program.child_type_references(left);
        let right = program.child_type_references(right);
        if left.len() != right.len() {
            return false;
        }
        pending.extend(left.iter().cloned().zip(right.iter().cloned()));
        true
    }
    if !span(&mut pending, program, left, right) {
        return Ok(false);
    }
    while let Some((left, right)) = pending.pop() {
        let owner = |reference: &TypeReference| -> Result<Option<SymbolHandle>, Diagnostic> {
            let owner = reference.supplied_operand_owner();
            if !owner.is_valid() {
                return Ok(None);
            }
            let declaration = program
                .tables
                .types
                .operand_lifetime_owners
                .get(owner)
                .declaration;
            if !declaration.is_valid() {
                return Err(error(
                    source,
                    "an inferred nominal type operand lost its supplied lifetime owner",
                ));
            }
            Ok(Some(declaration))
        };
        if owner(&left)? != owner(&right)? {
            return Err(error(
                source,
                "comparing distinct supplied lifetime contexts during callable type inference is not implemented",
            ));
        }
        match (left, right) {
            (TypeReference::AliasUse { expansion, .. }, right) => pending.push((*expansion, right)),
            (left, TypeReference::AliasUse { expansion, .. }) => pending.push((left, *expansion)),
            (
                TypeReference::Named { symbol: left, .. },
                TypeReference::Named { symbol: right, .. },
            ) => {
                if !left.is_valid() || !right.is_valid() {
                    return Err(error(
                        source,
                        "an inferred nominal type argument has no exact selected declaration",
                    ));
                }
                if left != right {
                    return Ok(false);
                }
            }
            (TypeReference::Unit, TypeReference::Unit) => {}
            (TypeReference::Generic(left), TypeReference::Generic(right)) => {
                if !left.base_symbol.is_valid() || !right.base_symbol.is_valid() {
                    return Err(error(
                        source,
                        "an inferred nominal type application has no exact selected declaration",
                    ));
                }
                if !left.lifetime_arguments.is_empty() || !right.lifetime_arguments.is_empty() {
                    return Err(error(
                        source,
                        "comparing invocation lifetimes during callable type inference is not implemented",
                    ));
                }
                if left.base_symbol != right.base_symbol
                    || !span(&mut pending, program, left.arguments, right.arguments)
                {
                    return Ok(false);
                }
            }
            (
                TypeReference::DynamicTrait {
                    symbol: left,
                    arguments: left_arguments,
                    ..
                },
                TypeReference::DynamicTrait {
                    symbol: right,
                    arguments: right_arguments,
                    ..
                },
            ) => {
                if !left.is_valid() || !right.is_valid() {
                    return Err(error(
                        source,
                        "an inferred dynamic type argument has no exact selected trait",
                    ));
                }
                if left != right || !span(&mut pending, program, left_arguments, right_arguments) {
                    return Ok(false);
                }
            }
            (TypeReference::Slice(left), TypeReference::Slice(right)) => pending.push((
                program
                    .declarations()
                    .child_type_references
                    .get(left.element_type)
                    .clone(),
                program
                    .declarations()
                    .child_type_references
                    .get(right.element_type)
                    .clone(),
            )),
            (TypeReference::Reference(left), TypeReference::Reference(right))
                if left.lifetime.is_none() && right.lifetime.is_none() =>
            {
                if left.access != right.access {
                    return Ok(false);
                }
                pending.push((
                    program
                        .declarations()
                        .child_type_references
                        .get(left.referee)
                        .clone(),
                    program
                        .declarations()
                        .child_type_references
                        .get(right.referee)
                        .clone(),
                ));
            }
            (TypeReference::FixedArray(left), TypeReference::FixedArray(right)) => {
                let (
                    FixedArrayLength::Literal(left_length),
                    FixedArrayLength::Literal(right_length),
                ) = (&left.length, &right.length)
                else {
                    return Err(error(
                        source,
                        "comparing dependent extents during callable type inference is not implemented",
                    ));
                };
                if left_length != right_length {
                    return Ok(false);
                }
                pending.push((
                    program
                        .declarations()
                        .child_type_references
                        .get(left.element_type)
                        .clone(),
                    program
                        .declarations()
                        .child_type_references
                        .get(right.element_type)
                        .clone(),
                ));
            }
            (
                TypeReference::Constrained(_)
                | TypeReference::ConstExpression(_)
                | TypeReference::Reference(_),
                _,
            )
            | (
                _,
                TypeReference::Constrained(_)
                | TypeReference::ConstExpression(_)
                | TypeReference::Reference(_),
            ) => {
                return Err(error(
                    source,
                    "comparing this deferred type operand during callable type inference is not implemented",
                ));
            }
            _ => return Ok(false),
        }
    }
    Ok(true)
}

fn argument_names(program: &SymbolResolvedTrees, arguments: HandleSpan<TypeReference>) -> String {
    program
        .child_type_references(arguments)
        .iter()
        .map(|argument| match argument {
            TypeReference::Named { symbol, .. } => program.symbols.display_path(*symbol, "::"),
            TypeReference::Unit => "()".to_owned(),
            _ => "<compound type>".to_owned(),
        })
        .collect::<Vec<_>>()
        .join(", ")
}
