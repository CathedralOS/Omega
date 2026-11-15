//! A Build may choose a provider whose slot and implementation are applied
//! to static arguments: `Build::select_provider<Reading<Tag>,
//! FirstReading<[Holder<u64>; 2], Tag>>(builder, target)`. The request
//! carries each operand as the author wrote it, a graph of names anchored at
//! their declarations, and a selection made inside a generic helper carries
//! the helper's static arguments beside it (its environment). This module
//! forms the application those two describe among the declarations of the
//! program being compiled: the declaration, and one closed argument for each
//! of its parameters.
//!
//! provider_selection.md, "Generic selected requirements": "Keep concrete
//! generic arguments in the closure keys rather than collapsing `Stage<1>`
//! and `Stage<2>` to one trait or provider declaration." generics.md,
//! "Applications and specialization": "Every application validates the
//! complete argument tuple, including kinds, ranges, and constraints of
//! unused parameters. Conflicting, excessive, or incomplete selections
//! reject."
//!
//! Nothing here names a declaration by its spelling. A name in the graph is
//! a compiler type atom or the source position of a declaration, found again
//! among this program's symbols; a name bound by the environment is replaced
//! by the argument the helper was given. Served: type arguments that are a
//! builtin type, plain or applied data, or a fixed array of those with a
//! literal extent; value arguments that are an integer literal or a Boolean.
//! Every other form is refused by name and forms nothing.

use arena::Handle;
use language_semantics::declaration_selection::product_operands::{
    ProductArrayLength, ProductOperandName, ProductOperandType, ProductStaticEnvironment,
    ProductStaticOperand, ProductStaticOperandGraph,
};
use language_semantics::declaration_selection::{ProductDeclarationOperand, ProductOperandRole};
use symbols::{SymbolHandle, SymbolKind};
use typed_trees::TypedTrees;
use typed_trees::data::TypeParameterKind;
use typed_trees::expression::{ExpressionNode, StaticMachineArgument, StaticSymbolApplication};
use typed_trees::name::Identifier;
use typed_trees::product_operand::{ProductApplicationDemand, ProductOperandApplication};
use typed_trees::trait_definition::{AppliedTraitScope, TraitApplicationArguments};
use typed_trees::types::{FixedArrayLength, PrimitiveType, TypeReferenceHandle, TypeReferenceNode};

/// Why an operand formed no application.
pub(super) enum Stop {
    /// The operand names a parameter of the generic helper that wrote it and
    /// no environment gives that parameter a value: the request as authored,
    /// before any call of the helper.
    Open,
    /// A form this producer does not serve yet.
    Unserved(&'static str),
    /// The application is wrong as written.
    Rejected(String),
}

type Formed<T> = Result<T, Stop>;

pub(super) enum OperandSource<'program> {
    Authored(&'program typed_trees::product_operand::ProductSourceLookups<'program>),
    Retained(SymbolHandle),
}

/// A static argument with nothing left to substitute.
enum Closed {
    Type(TypeReferenceHandle),
    Integer(numerics::literals::IntegerLiteral),
    Boolean(bool),
}

/// How many names, types and nested arguments one argument may hold before
/// it is refused as malformed. A graph is a tree of what the author wrote in
/// one static argument, so only a damaged one is larger.
const NODES: usize = 4096;

struct Lower<'a> {
    program: &'a mut TypedTrees,
    source: Option<&'a typed_trees::product_operand::ProductSourceLookups<'a>>,
    graph: &'a ProductStaticOperandGraph,
    /// The helper's static arguments, when the graph is the authored request
    /// of a selection made inside a generic helper. `None` while an
    /// environment's own argument is lowered: stage 04 already closed it.
    environment: Option<&'a ProductStaticEnvironment>,
    /// The generic helper that wrote the request, whose binders a bare name
    /// in it may be; invalid where no such name can occur.
    helper: SymbolHandle,
    /// Nodes read so far for this argument.
    nodes: usize,
}

impl Lower<'_> {
    fn deeper(&mut self) -> Formed<()> {
        self.nodes += 1;
        if self.nodes > NODES {
            return Err(Stop::Unserved("a static argument too large to be authored"));
        }
        Ok(())
    }

    /// The declaration a graph name stands for among this program's symbols.
    fn symbol(&self, name: &ProductOperandName) -> Formed<SymbolHandle> {
        let symbols = &self.program.symbols;
        if let Some(atom) = name.builtin_type {
            return symbols
                .builtin_atom_symbol(atom)
                .ok_or(Stop::Unserved("a builtin type this program does not hold"));
        }
        let Some(declaration) = name.declaration else {
            // A Build names Product declarations it cannot see while it is
            // typed, so typing leaves the name of one unresolved. It is
            // resolved the way the operand's own path is: as the one data
            // declaration visible from where the name is written. A retained
            // operand has no such position; every name of it is anchored.
            if !self.helper.is_valid() && self.environment.is_none() {
                return Err(Stop::Unserved(
                    "a static argument name with no declaration anchor",
                ));
            }
            return self.source.ok_or(Stop::Unserved("a retained operand has no authored lookup scope"))?.declarations.resolve(
                &name.text,
                name.source_span,
                ProductOperandRole::Data,
            )
            .ok_or_else(|| {
                Stop::Rejected(format!(
                    "static argument `{}` of this provider choice names no data type visible here",
                    name.text
                ))
            });
        };
        let declaration = symbols.rejoin_product_occurrence(declaration);
        let mut found = self
            .program
            .product_declarations()
            .data_definitions()
            .iter()
            .map(|data| data.symbol)
            .filter(|symbol| symbols.symbol_provenance_source_span(*symbol) == Some(declaration));
        match (found.next(), found.next()) {
            (Some(symbol), None) => Ok(symbol),
            (None, _) => Err(Stop::Unserved(
                "a static argument naming something other than a builtin type or data",
            )),
            (Some(_), Some(_)) => Err(Stop::Unserved(
                "a static argument whose declaration anchor is ambiguous",
            )),
        }
    }

    /// The source position of the helper parameter `name` stands for, when
    /// it stands for one. A name typing resolved carries its declaration; a
    /// bare name typing left unresolved is looked up where the language
    /// resolves it, among the binders of the helper that wrote the request,
    /// and nowhere else.
    fn parameter(&self, name: &ProductOperandName) -> Formed<Option<source::SourceSpan>> {
        let symbols = &self.program.symbols;
        if let Some(declaration) = name.declaration {
            let declaration = symbols.rejoin_product_occurrence(declaration);
            let is_parameter = self
                .program
                .data_type_parameters
                .iter()
                .any(|(_, parameter)| {
                    symbols.symbols().nodes().is_valid(parameter.symbol)
                        && symbols.get(parameter.symbol).kind == SymbolKind::TypeParameter
                        && symbols.symbol_provenance_source_span(parameter.symbol)
                            == Some(declaration)
                });
            return Ok(is_parameter.then_some(declaration));
        }
        if name.builtin_type.is_some() || name.builtin_function.is_some() {
            return Ok(None);
        }
        let helper = match self.environment {
            Some(environment) => environment
                .template
                .and_then(|template| self.source?.authors.resolve(template).ok()),
            None => Some(self.helper),
        };
        let Some(machine) = helper.and_then(|helper| self.program.machine_by_symbol(helper)) else {
            return Ok(None);
        };
        let mut binders = self
            .program
            .machine_callable_parameters(machine)
            .into_iter()
            .filter(|parameter| parameter.name.as_str() == name.text);
        match (binders.next(), binders.next()) {
            (Some(binder), None) => Ok(symbols
                .symbol_provenance_source_span(binder.symbol)
                .map(|declaration| symbols.rejoin_product_occurrence(declaration))),
            (None, _) => Ok(None),
            (Some(_), Some(_)) => Err(Stop::Unserved(
                "a static argument naming two binders of its helper",
            )),
        }
    }

    /// The argument the environment gives the helper parameter `name`
    /// stands for, lowered; `None` when `name` is no parameter of the
    /// helper. A parameter with no environment is open.
    fn supplied(&mut self, name: &ProductOperandName) -> Formed<Option<Closed>> {
        let Some(parameter) = self.parameter(name)? else {
            return Ok(None);
        };
        let Some(environment) = self.environment else {
            return Err(Stop::Open);
        };
        let symbols = &self.program.symbols;
        let mut bound = environment
            .bindings
            .iter()
            .map(|(_, binding)| binding)
            .filter(|binding| symbols.rejoin_product_occurrence(binding.parameter) == parameter);
        let (Some(binding), None) = (bound.next(), bound.next()) else {
            return Err(Stop::Unserved(
                "a helper parameter its selection gives no single value",
            ));
        };
        let mut actual = Lower {
            program: self.program,
            source: self.source,
            graph: &environment.graph,
            environment: None,
            helper: SymbolHandle::invalid(),
            nodes: self.nodes,
        };
        actual.argument(binding.argument).map(Some)
    }

    /// One static argument, closed.
    fn argument(&mut self, handle: Handle<ProductStaticOperand>) -> Formed<Closed> {
        self.deeper()?;
        if !self.graph.operands.is_valid(handle) {
            return Err(Stop::Unserved(
                "a static argument its request does not hold",
            ));
        }
        let operand = self.graph.operands.get(handle).clone();
        if operand.projection.is_some() {
            return Err(Stop::Unserved("a projected static argument"));
        }
        if let Some(literal) = operand.integer {
            return Ok(Closed::Integer(literal));
        }
        if operand.structural_type.is_valid() {
            return self.ty(operand.structural_type).map(Closed::Type);
        }
        let names = self.graph.names.span_or_empty(operand.path).to_vec();
        let [name] = names.as_slice() else {
            return Err(Stop::Unserved("a qualified static argument"));
        };
        if !operand.application.is_valid() {
            // `true` and `false` are keywords, so neither is a binder.
            if name.declaration.is_none() && name.builtin_type.is_none() {
                match name.text.as_str() {
                    "true" => return Ok(Closed::Boolean(true)),
                    "false" => return Ok(Closed::Boolean(false)),
                    _ => {}
                }
            }
            if let Some(supplied) = self.supplied(name)? {
                return Ok(supplied);
            }
            let symbol = self.symbol(name)?;
            return Ok(Closed::Type(self.named(symbol, &name.text)));
        }
        // Applied data written as a path, `Holder<u64>`.
        let application = self.graph.applications.get(operand.application).clone();
        if application.member.is_valid() || !application.lifetimes.is_empty() {
            return Err(Stop::Unserved(
                "a static argument applied at lifetimes or naming a member",
            ));
        }
        let symbol = self.symbol(name)?;
        let mut arguments = Vec::new();
        for argument in self
            .graph
            .static_arguments
            .span_or_empty(application.arguments)
            .to_vec()
        {
            let closed = self.argument(argument)?;
            arguments.push(self.reference(closed)?);
        }
        Ok(Closed::Type(self.applied(symbol, &name.text, arguments)))
    }

    fn named(&mut self, symbol: SymbolHandle, text: &str) -> TypeReferenceHandle {
        self.program
            .type_reference_table
            .insert(TypeReferenceNode::Named {
                symbol,
                name: Identifier::generated(text),
            })
    }

    fn applied(
        &mut self,
        symbol: SymbolHandle,
        text: &str,
        arguments: Vec<TypeReferenceHandle>,
    ) -> TypeReferenceHandle {
        let arguments = self
            .program
            .type_reference_table
            .insert_type_reference_handles(arguments);
        self.program
            .type_reference_table
            .insert(TypeReferenceNode::Generic {
                base_symbol: symbol,
                base_name: Identifier::generated(text),
                lifetime_arguments: Vec::new(),
                arguments,
            })
    }

    /// A closed argument as the type reference an application holds it by.
    fn reference(&mut self, closed: Closed) -> Formed<TypeReferenceHandle> {
        match closed {
            Closed::Type(reference) => Ok(reference),
            Closed::Integer(literal) => {
                let value = self
                    .program
                    .expression_table
                    .insert(ExpressionNode::Integer(literal));
                Ok(self
                    .program
                    .type_reference_table
                    .insert(TypeReferenceNode::ConstExpression(value)))
            }
            Closed::Boolean(_) => Err(Stop::Unserved(
                "a Boolean argument of applied data in a provider choice",
            )),
        }
    }

    fn ty(&mut self, handle: Handle<ProductOperandType>) -> Formed<TypeReferenceHandle> {
        self.deeper()?;
        if !self.graph.types.is_valid(handle) {
            return Err(Stop::Unserved("a static type its request does not hold"));
        }
        match self.graph.types.get(handle).clone() {
            ProductOperandType::Unit => Ok(self
                .program
                .type_reference_table
                .insert(TypeReferenceNode::Unit)),
            ProductOperandType::Named(name) => {
                let name = self.graph.names.get(name).clone();
                match self.supplied(&name)? {
                    Some(Closed::Type(reference)) => Ok(reference),
                    Some(other) => self.reference(other),
                    None => {
                        let symbol = self.symbol(&name)?;
                        Ok(self.named(symbol, &name.text))
                    }
                }
            }
            ProductOperandType::Generic {
                name,
                lifetimes,
                arguments,
            } => {
                if !lifetimes.is_empty() {
                    return Err(Stop::Unserved("a static type applied at lifetimes"));
                }
                let name = self.graph.names.get(name).clone();
                let symbol = self.symbol(&name)?;
                let mut lowered = Vec::new();
                for argument in self.graph.type_arguments.span_or_empty(arguments).to_vec() {
                    lowered.push(self.ty(argument)?);
                }
                Ok(self.applied(symbol, &name.text, lowered))
            }
            ProductOperandType::Array { element, length } => {
                let length = match length {
                    ProductArrayLength::Literal(length) => length,
                    ProductArrayLength::Parameter(name) => {
                        let name = self.graph.names.get(name).clone();
                        let Some(Closed::Integer(literal)) = self.supplied(&name)? else {
                            return Err(Stop::Unserved(
                                "an array extent that is not a literal or a helper's value",
                            ));
                        };
                        literal
                            .value_u64()
                            .and_then(|length| usize::try_from(length).ok())
                            .ok_or(Stop::Unserved("an array extent outside the host's range"))?
                    }
                    ProductArrayLength::Expression(_) | ProductArrayLength::Call(_) => {
                        return Err(Stop::Unserved("a computed array extent"));
                    }
                };
                let element_type = self.ty(element)?;
                Ok(self
                    .program
                    .type_reference_table
                    .insert(TypeReferenceNode::FixedArray {
                        element_type,
                        length: FixedArrayLength::Literal(length),
                    }))
            }
            ProductOperandType::CanonicalConst(_)
            | ProductOperandType::ConstExpression(_)
            | ProductOperandType::Reference { .. }
            | ProductOperandType::Slice(_)
            | ProductOperandType::Constrained { .. }
            | ProductOperandType::Dynamic { .. }
            | ProductOperandType::Invalid => Err(Stop::Unserved(
                "a static type that is not a builtin type, data or a fixed array of those",
            )),
        }
    }
}

/// The argument a parameter of kind `kind` is given by `closed`, checked
/// against the kind: a type for a type parameter, and for a value parameter
/// a literal of its declared type. A suffixed literal keeps its type
/// (numeric_values.md, "Literal landing and destinations": "An incompatible
/// destination rejects rather than erasing the suffix").
fn bound(
    program: &TypedTrees,
    parameter: &typed_trees::data::TypeParameter,
    closed: Closed,
) -> Formed<StaticMachineArgument> {
    let name = parameter.name.as_str();
    match (&parameter.kind, closed) {
        (TypeParameterKind::Type, Closed::Type(reference)) => Ok(StaticMachineArgument {
            type_reference: reference,
            ..StaticMachineArgument::default()
        }),
        (TypeParameterKind::Const { type_reference }, Closed::Integer(literal)) => {
            let landed_type = program
                .primitive_type_reference(*type_reference)
                .and_then(PrimitiveType::landed_landing_fixed)
                .ok_or(Stop::Unserved(
                    "a value parameter that is not a fixed-width integer or a Boolean",
                ))?;
            let landing = numerics::literals::IntegerLanding {
                landed_type,
                domain: numerics::arithmetic::ArithmeticDomain::Exact,
            };
            let fits = literal
                .value_i64()
                .map(i128::from)
                .or_else(|| literal.value_u64().map(i128::from))
                .is_some_and(|value| landed_type.admits_i128(value));
            let literal = match literal.landing() {
                None if fits => literal.with_landing(landing),
                Some(stated) if stated == landing && fits => literal,
                _ => {
                    return Err(Stop::Rejected(format!(
                        "this application gives the value parameter `{name}` of type `{landed_type}` the literal `{literal}`, which is not a value of that type"
                    )));
                }
            };
            Ok(StaticMachineArgument {
                const_literal: Some(literal),
                ..StaticMachineArgument::default()
            })
        }
        (TypeParameterKind::Const { type_reference }, Closed::Boolean(value))
            if program.primitive_type_reference(*type_reference) == Some(PrimitiveType::Bool) =>
        {
            Ok(StaticMachineArgument {
                path: Box::new([Identifier::generated(if value { "true" } else { "false" })]),
                ..StaticMachineArgument::default()
            })
        }
        (TypeParameterKind::Type, _) => Err(Stop::Rejected(format!(
            "this application gives the type parameter `{name}` a value"
        ))),
        (TypeParameterKind::Const { .. }, _) => Err(Stop::Rejected(format!(
            "this application gives the value parameter `{name}` something that is not a value of its type"
        ))),
        (
            TypeParameterKind::Value { .. }
            | TypeParameterKind::Machine { .. }
            | TypeParameterKind::Implementation
            | TypeParameterKind::Proposition { .. },
            _,
        ) => Err(Stop::Unserved(
            "a slot or provider with a runtime-value, machine, implementation or proposition parameter",
        )),
    }
}

/// Forms the application `operand` describes and keeps it with the program.
/// `Ok(None)`: the request as authored names a helper parameter and has no
/// environment yet, so there is no application to keep; its shape was
/// checked.
///
/// `retained` is the declaration itself when the operand was handed over by
/// the package that made the choice: its path was resolved there, and here
/// the declaration is the one its anchor found again.
pub(super) fn form(
    program: &mut TypedTrees,
    source: OperandSource<'_>,
    operand: &ProductDeclarationOperand,
    role: ProductOperandRole,
    author: SymbolHandle,
    environment: Option<&ProductStaticEnvironment>,
    demand: ProductApplicationDemand,
) -> Formed<Option<Handle<ProductOperandApplication>>> {
    let (source, retained) = match source {
        OperandSource::Authored(source) => (Some(source), None),
        OperandSource::Retained(declaration) => (None, Some(declaration)),
    };
    let Some(graph) = operand.application.as_deref() else {
        return Ok(None);
    };
    if !matches!(
        role,
        ProductOperandRole::BoundarySubject | ProductOperandRole::Provider
    ) {
        return Err(Stop::Unserved(
            "an applied operand that is not a slot or a provider",
        ));
    }
    if let Some(existing) = program.prepared_product_operand(operand, environment, author, role) {
        return Ok(Some(existing));
    }
    if !graph.operands.is_valid(graph.root) {
        return Err(Stop::Unserved("an applied operand with no root"));
    }
    let root = graph.operands.get(graph.root).clone();
    if !graph.applications.is_valid(root.application) {
        return Err(Stop::Unserved("an applied operand with no argument list"));
    }
    let application = graph.applications.get(root.application).clone();
    if application.member.is_valid() || !application.lifetimes.is_empty() {
        return Err(Stop::Unserved(
            "a slot or provider applied at lifetimes or naming a member",
        ));
    }
    let declaration = match retained {
        Some(declaration) => Some(declaration),
        None => source.ok_or(Stop::Unserved("an authored operand has no declaration lookup scope"))?.declarations.resolve(
            &operand.path,
            operand.source_span,
            role,
        ),
    }
    .ok_or_else(|| {
        Stop::Rejected(format!(
            "cannot select provider operand `{}` here; it must name one visible declaration of the required kind",
            operand.path
        ))
    })?;
    let (declaration_parameters, lifetimes) = match role {
        ProductOperandRole::BoundarySubject => program
            .trait_definition_by_symbol(declaration)
            .map(|definition| {
                (
                    definition.type_parameters,
                    definition.lifetime_parameters.len(),
                )
            })
            .ok_or(Stop::Unserved(
                "an applied slot that is not a boundary trait",
            ))?,
        _ => program
            .conformance_by_symbol(declaration)
            .map(|definition| {
                (
                    definition.type_parameters,
                    definition.lifetime_parameters.len(),
                )
            })
            .ok_or_else(|| {
                Stop::Rejected(format!(
                    "provider selection `{}` is not a static operation record from a named implementation; a data type is not a provider value",
                    operand.path
                ))
            })?,
    };
    if lifetimes != 0 {
        return Err(Stop::Unserved(
            "an applied slot or provider with lifetime parameters",
        ));
    }
    let parameters = program
        .data_type_parameters
        .span_or_empty(declaration_parameters)
        .to_vec();
    let arguments = graph
        .static_arguments
        .span_or_empty(application.arguments)
        .to_vec();
    if parameters.len() != arguments.len() {
        return Err(Stop::Rejected(format!(
            "`{}` takes {} static argument(s) and this provider choice gives it {}: an application supplies its complete argument tuple",
            operand.path,
            parameters.len(),
            arguments.len()
        )));
    }
    let mut closed = Vec::with_capacity(arguments.len());
    for (parameter, argument) in parameters.iter().zip(arguments) {
        let mut lower = Lower {
            program,
            source,
            graph,
            environment,
            helper: if retained.is_some() {
                SymbolHandle::invalid()
            } else {
                author
            },
            nodes: 0,
        };
        let argument = lower.argument(argument)?;
        closed.push(bound(program, parameter, argument)?);
    }
    let whole = StaticMachineArgument {
        symbol: declaration,
        path: Box::new([Identifier::generated(&operand.path)]),
        application: Some(Box::new(StaticSymbolApplication {
            member: Handle::invalid(),
            lifetime_arguments: Box::default(),
            arguments: closed.clone().into_boxed_slice(),
        })),
        ..StaticMachineArgument::default()
    };
    let unformed = |_| Stop::Unserved("an application whose arguments have no type form");
    let references = closed
        .iter()
        .map(|argument| program.static_operand_type_reference_in_scope(argument, author))
        .collect::<Result<Vec<_>, _>>()
        .map_err(unformed)?;
    let incomplete = Stop::Unserved("an application with no complete trait scope");
    let scope = match role {
        ProductOperandRole::BoundarySubject => {
            let definition =
                program
                    .trait_definition_by_symbol(declaration)
                    .ok_or(Stop::Unserved(
                        "an applied slot that is not a boundary trait",
                    ))?;
            AppliedTraitScope::new(
                program,
                definition,
                TraitApplicationArguments {
                    lexical_owner: author,
                    enclosing_data_applications: &[],
                    arguments: &references,
                    lifetimes: &[],
                },
            )
            .map(|scope| scope.environment())
        }
        _ => {
            let definition = program
                .conformance_by_symbol(declaration)
                .ok_or(Stop::Unserved("an applied provider with no declaration"))?;
            AppliedTraitScope::from_conformance_application(
                program,
                definition,
                TraitApplicationArguments {
                    lexical_owner: author,
                    enclosing_data_applications: &[],
                    arguments: &references,
                    lifetimes: &[],
                },
            )
            .map(|scope| scope.environment())
        }
    }
    .ok_or(incomplete)?;
    let scope = scope.retain(program).map_err(Stop::Unserved)?;
    let supplying = program.expression_table.retain_static_machine_application(
        declaration,
        Identifier::generated(&operand.path),
        std::slice::from_ref(&whole),
    );
    let retained = program
        .expression_table
        .static_machine_application_mut(supplying);
    retained.binding_declaration = declaration;
    retained.lexical_owner = author;
    Ok(Some(program.retain_product_operand_application(
        ProductOperandApplication {
            operand: operand.clone(),
            environment: environment.cloned().map(std::sync::Arc::new),
            owner: author,
            role: Some(role),
            declaration,
            declaration_parameters,
            declaration_lifetime_count: 0,
            demand,
            argument: whole,
            input_selections: Default::default(),
            scope,
            supplying_application: supplying,
            implementation: None,
        },
    )))
}
