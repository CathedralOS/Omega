//! A Build may choose a provider whose slot and implementation are applied
//! to static arguments: `Build::select_provider::<Reading<Tag>,
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
//! A retained name rejoins a builtin atom or a declaration anchor. An authored
//! Product data path unresolved in Build is looked up from that exact source
//! occurrence. Helper binders are replaced by their supplied arguments. Served: type arguments that are a
//! builtin type, plain or applied data, a fixed array with a literal extent, or a type with declared domain qualifications; value arguments that are an integer literal or a Boolean.
//! Every other form is refused by name and forms nothing.

mod aliases;
mod qualifications;

use crate::declaration_selection::product_operands::{
    ProductArrayLength, ProductOperandName, ProductOperandType, ProductStaticEnvironment,
    ProductStaticOperand, ProductStaticOperandGraph,
};
use crate::declaration_selection::{ProductDeclarationOperand, ProductOperandRole};
use crate::source_state::TypedTrees;
use crate::source_state::data::TypeParameterKind;
use crate::source_state::expression::{StaticMachineArgument, StaticSymbolApplication};
use crate::source_state::name::Identifier;
use crate::source_state::product_operand::{ProductApplicationDemand, ProductOperandApplication};
use crate::source_state::trait_definition::{AppliedTraitScope, TraitApplicationArguments};
use crate::source_state::types::{
    FixedArrayLength, PrimitiveType, TypeReferenceHandle, TypeReferenceNode,
};
use arena::Handle;
use std::borrow::Cow;
use symbols::{SymbolHandle, SymbolKind};

/// Why an operand formed no application.
pub(super) enum Stop {
    /// The operand names a parameter of the generic helper that wrote it and
    /// no environment gives that parameter a value: the request as authored,
    /// before any call of the helper.
    Open,
    /// A form this producer does not serve yet.
    Unserved(Cow<'static, str>),
    /// The application is wrong as written.
    Rejected(String),
}

type Formed<T> = Result<T, Stop>;

fn unserved(reason: impl Into<Cow<'static, str>>) -> Stop {
    Stop::Unserved(reason.into())
}

pub(super) enum OperandSource<'program> {
    Authored(&'program crate::source_state::product_operand::ProductSourceLookups<'program>),
    Retained(SymbolHandle),
}

/// A static argument with nothing left to substitute.
enum Closed {
    Type(TypeReferenceHandle),
    Integer(numeric_literals::IntegerLiteral),
    Boolean(bool),
}

/// How many names, types and nested arguments one argument may hold before
/// it is refused as malformed. A graph is a tree of what the author wrote in
/// one static argument, so only a damaged one is larger.
const NODES: usize = 4096;

struct Lower<'a> {
    program: &'a mut TypedTrees,
    source: Option<&'a crate::source_state::product_operand::ProductSourceLookups<'a>>,
    graph: &'a ProductStaticOperandGraph,
    /// The helper's static arguments, when the graph is the authored request
    /// of a selection made inside a generic helper. `None` while an
    /// environment's own argument is lowered: stage 04 already closed it.
    environment: Option<&'a ProductStaticEnvironment>,
    /// The generic helper that wrote the request, whose binders a bare name
    /// in it may be; invalid where no such name can occur.
    helper: SymbolHandle,
    /// The authored selection that owns the normalized nominal types.
    selecting_operand: source_text::SourceSpan,
    /// Nodes read so far for this argument.
    nodes: usize,
}

impl Lower<'_> {
    fn builtin_type(&mut self, name: &ProductOperandName) -> Formed<Option<SymbolHandle>> {
        if let Some(atom) = name.builtin_type {
            if let Some(carrier) = atom.fixed_integer() {
                self.program.symbols.install_integer_types([carrier]);
            }
            return self
                .program
                .symbols
                .builtin_atom_symbol(atom)
                .map(Some)
                .ok_or(unserved(
                    "a retained builtin type this program does not hold",
                ));
        }
        if !crate::source_symbols::is_builtin_type_spelling(&name.text) {
            return Ok(None);
        }
        // The request came from Build's source map, so its source position
        // cannot scope a lookup in the product. Builtins have the same
        // compiler identity in both maps and need no authored import.
        let atom = numerics::integer_carrier::FixedInteger::from_decimal_spelling(&name.text)
            .map(symbols::BuiltinTypeAtom::Integer)
            .or_else(|| {
                symbols::BuiltinTypeAtom::ALL
                    .into_iter()
                    .find(|atom| atom.symbol_name() == name.text)
            })
            .ok_or_else(|| {
                Stop::Rejected(format!(
                    "invalid builtin type `{}` in provider application",
                    name.text
                ))
            })?;
        if let Some(carrier) = atom.fixed_integer() {
            self.program.symbols.install_integer_types([carrier]);
        }
        self.program
            .symbols
            .builtin_atom_symbol(atom)
            .map(Some)
            .ok_or(unserved("a builtin type this program does not hold"))
    }

    fn deeper(&mut self) -> Formed<()> {
        self.nodes += 1;
        if self.nodes > NODES {
            return Err(unserved("a static argument too large to be authored"));
        }
        Ok(())
    }

    /// The declaration a graph name stands for among this program's symbols.
    fn symbol(
        &self,
        name: &ProductOperandName,
        path: &str,
        kind: SymbolKind,
    ) -> Formed<SymbolHandle> {
        let symbols = &self.program.symbols;
        let Some(declaration) = name.declaration else {
            // A Build names Product declarations it cannot see while it is
            // typed, so typing leaves the name of one unresolved. It is
            // resolved the way the operand's own path is: as the one type or domain
            // declaration visible from where the name is written. A retained
            // operand has no such position; every name of it is anchored.
            if !self.helper.is_valid() && self.environment.is_none() {
                return Err(unserved(
                    "a static argument name with no declaration anchor",
                ));
            }
            let source = self
                .source
                .ok_or(unserved("a retained operand has no authored lookup scope"))?;
            let (declaration, description) = if kind == SymbolKind::Domain {
                (source.declarations.resolve(path, name.source_span, ProductOperandRole::Domain), "domain")
            } else {
                (source.declarations.resolve_candidates(path, name.source_span, &source.types, &[]), "type")
            };
            return declaration
                .ok_or_else(|| Stop::Rejected(format!(
                    "static argument `{path}` of this provider choice names no {description} visible here"
                )));
        };
        let declaration = symbols.rejoin_product_occurrence(declaration);
        let at_declaration = |symbol: &SymbolHandle| {
            symbols.symbol_provenance_source_span(*symbol) == Some(declaration)
        };
        let mut domains = self
            .program
            .domain_definitions()
            .iter()
            .map(|domain| domain.symbol)
            .filter(at_declaration);
        let mut types = self
            .program
            .data_definitions()
            .iter()
            .map(|data| data.symbol)
            .chain(self.program.type_aliases().iter().map(|alias| alias.symbol))
            .filter(at_declaration);
        let matches = if kind == SymbolKind::Domain {
            (domains.next(), domains.next())
        } else {
            (types.next(), types.next())
        };
        match matches {
            (Some(symbol), None) => Ok(symbol),
            (None, _) => Err(unserved(
                "a retained static argument whose declaration has the wrong kind",
            )),
            (Some(_), Some(_)) => Err(unserved(
                "a static argument whose declaration anchor is ambiguous",
            )),
        }
    }

    /// The source position of the helper parameter `name` stands for, when
    /// it stands for one. A name typing resolved carries its declaration; a
    /// bare name typing left unresolved is looked up where the language
    /// resolves it, among the binders of the helper that wrote the request,
    /// and nowhere else.
    fn parameter(&self, name: &ProductOperandName) -> Formed<Option<source_text::SourceSpan>> {
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
            (Some(_), Some(_)) => Err(unserved(
                "a static argument naming two binders of its helper",
            )),
        }
    }

    /// The argument the environment gives the helper parameter `name`
    /// stands for, lowered; `None` when `name` is no parameter of the
    /// helper. A parameter with no environment is open.
    fn supplied(
        &mut self,
        name: &ProductOperandName,
        expects_type: bool,
    ) -> Formed<Option<Closed>> {
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
            return Err(unserved(
                "a helper parameter its selection gives no single value",
            ));
        };
        let mut actual = Lower {
            program: self.program,
            source: self.source,
            graph: &environment.graph,
            environment: None,
            helper: SymbolHandle::invalid(),
            selecting_operand: self.selecting_operand,
            nodes: self.nodes,
        };
        actual.argument(binding.argument, expects_type).map(Some)
    }

    /// One static argument, closed.
    fn argument(
        &mut self,
        handle: Handle<ProductStaticOperand>,
        expects_type: bool,
    ) -> Formed<Closed> {
        self.deeper()?;
        if !self.graph.operands.is_valid(handle) {
            return Err(unserved("a static argument its request does not hold"));
        }
        let operand = self.graph.operands.get(handle).clone();
        if operand.projection.is_some() {
            return Err(unserved("a projected static argument"));
        }
        if let Some(literal) = operand.integer {
            return Ok(Closed::Integer(literal));
        }
        if operand.structural_type.is_valid() {
            return self.ty(operand.structural_type, expects_type);
        }
        let names = self.graph.names.span_or_empty(operand.path).to_vec();
        let name = names
            .last()
            .ok_or(unserved("an empty static argument path"))?;
        if !operand.application.is_valid() && names.len() == 1 {
            return self.bare_name(name, expects_type);
        }
        let path = names
            .iter()
            .map(|part| part.text.as_str())
            .collect::<Vec<_>>()
            .join("::");
        if !operand.application.is_valid() {
            let symbol = self.symbol(name, &path, SymbolKind::Data)?;
            return self.type_application(symbol, name, Vec::new());
        }
        // Applied data written as a path, `Holder<u64>`.
        let application = self.graph.applications.get(operand.application).clone();
        if application.member.is_valid() || !application.lifetimes.is_empty() {
            return Err(unserved(
                "a static argument applied at lifetimes or naming a member",
            ));
        }
        let symbol = self.symbol(name, &path, SymbolKind::Data)?;
        let inputs = self
            .graph
            .static_arguments
            .span_or_empty(application.arguments)
            .to_vec();
        self.applied_arguments(symbol, name, &inputs, Self::argument)
    }

    fn bare_name(&mut self, name: &ProductOperandName, expects_type: bool) -> Formed<Closed> {
        if expects_type && let Some(symbol) = self.builtin_type(name)? {
            return Ok(Closed::Type(self.named(symbol, name)));
        }
        if name.declaration.is_none() && name.builtin_type.is_none() {
            match name.text.as_str() {
                "true" => return Ok(Closed::Boolean(true)),
                "false" => return Ok(Closed::Boolean(false)),
                _ => {}
            }
        }
        if let Some(supplied) = self.supplied(name, expects_type)? {
            return Ok(supplied);
        }
        let symbol = self.symbol(name, &name.text, SymbolKind::Data)?;
        self.type_application(symbol, name, Vec::new())
    }

    /// The data declaration's own telescope gives each nested operand its role.
    /// Both path applications and structural type arguments use this judgment.
    fn applied_arguments<T: Copy>(
        &mut self,
        symbol: SymbolHandle,
        name: &ProductOperandName,
        inputs: &[T],
        read: fn(&mut Self, T, bool) -> Formed<Closed>,
    ) -> Formed<Closed> {
        let parameters = self.program.declaration_type_parameters(symbol).to_vec();
        let arguments = self.closed_arguments(&parameters, name, inputs, read)?;
        self.type_application(symbol, name, arguments)
    }

    fn closed_arguments<T: Copy>(
        &mut self,
        parameters: &[crate::source_state::data::TypeParameter],
        name: &ProductOperandName,
        inputs: &[T],
        read: fn(&mut Self, T, bool) -> Formed<Closed>,
    ) -> Formed<Vec<TypeReferenceHandle>> {
        if parameters.len() != inputs.len() {
            return Err(Stop::Rejected(format!(
                "static type `{}` needs {} arguments, received {}",
                name.text, parameters.len(), inputs.len()
            )));
        }
        let mut arguments = Vec::with_capacity(inputs.len());
        for (&input, parameter) in inputs.iter().zip(parameters) {
            let closed = read(self, input, parameter.kind == TypeParameterKind::Type)?;
            let argument = bound(self.program, parameter, closed)?;
            arguments.push(
                self.program
                    .static_operand_type_reference(&argument)
                    .map_err(unserved)?,
            );
        }
        Ok(arguments)
    }

    fn nominal(
        &mut self,
        node: TypeReferenceNode,
        symbol: SymbolHandle,
        name: &ProductOperandName,
    ) -> TypeReferenceHandle {
        use crate::source_state::types::{ProductTypeInput, TypeOperandOrigin};
        let reference = self.program.type_reference_table.insert(node);
        if matches!(self.program.symbols.get(symbol).kind, SymbolKind::Data | SymbolKind::TypeAlias) {
            self.program.type_reference_table.set_operand_origin(
                reference,
                TypeOperandOrigin::ProductInput(std::sync::Arc::new(ProductTypeInput {
                    operand: self.selecting_operand,
                    occurrence: name.source_span,
                    declaration: symbol,
                })),
            );
        }
        reference
    }

    fn named(&mut self, symbol: SymbolHandle, name: &ProductOperandName) -> TypeReferenceHandle {
        self.nominal(
            TypeReferenceNode::Named {
                symbol,
                name: Identifier::generated(&name.text),
            },
            symbol,
            name,
        )
    }

    fn applied(
        &mut self,
        symbol: SymbolHandle,
        name: &ProductOperandName,
        arguments: Vec<TypeReferenceHandle>,
    ) -> TypeReferenceHandle {
        let arguments = self
            .program
            .type_reference_table
            .insert_type_reference_handles(arguments);
        self.nominal(
            TypeReferenceNode::Generic {
                base_symbol: symbol,
                base_name: Identifier::generated(&name.text),
                lifetime_arguments: Vec::new(),
                arguments,
            },
            symbol,
            name,
        )
    }

    fn ty(&mut self, handle: Handle<ProductOperandType>, expects_type: bool) -> Formed<Closed> {
        self.deeper()?;
        if !self.graph.types.is_valid(handle) {
            return Err(unserved("a static type its request does not hold"));
        }
        match self.graph.types.get(handle).clone() {
            ProductOperandType::Unit => Ok(Closed::Type(self
                .program
                .type_reference_table
                .insert(TypeReferenceNode::Unit))),
            ProductOperandType::Named(name) => {
                let name = self.graph.names.get(name).clone();
                self.bare_name(&name, expects_type)
            }
            ProductOperandType::Generic {
                name,
                lifetimes,
                arguments,
            } => {
                if !lifetimes.is_empty() {
                    return Err(unserved("a static type applied at lifetimes"));
                }
                let name = self.graph.names.get(name).clone();
                let symbol = self.symbol(&name, &name.text, SymbolKind::Data)?;
                let inputs = self.graph.type_arguments.span_or_empty(arguments).to_vec();
                self.applied_arguments(symbol, &name, &inputs, Self::ty)
            }
            ProductOperandType::Array { element, length } => {
                let length = match length {
                    ProductArrayLength::Literal(length) => length,
                    ProductArrayLength::Parameter(name) => {
                        let name = self.graph.names.get(name).clone();
                        let Some(Closed::Integer(literal)) = self.supplied(&name, false)? else {
                            return Err(unserved(
                                "an array extent that is not a literal or a helper's value",
                            ));
                        };
                        literal
                            .value_u64()
                            .and_then(|length| usize::try_from(length).ok())
                            .ok_or(unserved("an array extent outside the host's range"))?
                    }
                    ProductArrayLength::Expression(_) | ProductArrayLength::Call(_) => {
                        return Err(unserved("a computed array extent"));
                    }
                };
                let Closed::Type(element_type) = self.ty(element, true)? else {
                    return Err(Stop::Rejected("an array element in a Product operand must be a type".into()));
                };
                Ok(Closed::Type(self
                    .program
                    .type_reference_table
                    .insert(TypeReferenceNode::FixedArray {
                        element_type,
                        length: FixedArrayLength::Literal(length),
                    })))
            }
            ProductOperandType::Constrained { base, constraints } => {
                self.qualified_type(base, constraints)
            }
            ProductOperandType::CanonicalConst(_)
            | ProductOperandType::ConstExpression(_)
            | ProductOperandType::Reference { .. }
            | ProductOperandType::Slice(_)
            | ProductOperandType::Dynamic { .. }
            | ProductOperandType::Invalid => Err(unserved(
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
    parameter: &crate::source_state::data::TypeParameter,
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
                .ok_or(unserved(
                    "a value parameter that is not a fixed-width integer or a Boolean",
                ))?;
            let landing = numeric_literals::IntegerLanding {
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
            | TypeParameterKind::Proof { .. }
            | TypeParameterKind::Machine { .. }
            | TypeParameterKind::Implementation
            | TypeParameterKind::Proposition { .. },
            _,
        ) => Err(unserved(
            "a slot or provider with a runtime-value, proof, machine, implementation or proposition parameter",
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
        ProductOperandRole::BoundarySubject
            | ProductOperandRole::RootGrantSubject
            | ProductOperandRole::Provider
            | ProductOperandRole::Requirement
            | ProductOperandRole::TestRequirement
    ) {
        return Err(unserved(
            "an applied operand that is not a slot, provider or requirement",
        ));
    }
    if let Some(existing) = program.prepared_product_operand(operand, environment, author, role) {
        return Ok(Some(existing));
    }
    if !graph.operands.is_valid(graph.root) {
        return Err(unserved("an applied operand with no root"));
    }
    let root = graph.operands.get(graph.root).clone();
    if !graph.applications.is_valid(root.application) {
        return Err(unserved("an applied operand with no argument list"));
    }
    let application = graph.applications.get(root.application).clone();
    if !application.lifetimes.is_empty()
        || application.member.is_valid() != (role == ProductOperandRole::TestRequirement)
    {
        return Err(unserved(if role == ProductOperandRole::TestRequirement {
            "a test group without its exact member of a concrete trait application"
        } else {
            "a slot or provider applied at lifetimes or naming a member"
        }));
    }
    let declaration = match retained {
        Some(declaration) => Some(declaration),
        None => source.ok_or(unserved("an authored operand has no declaration lookup scope"))?.declarations.resolve(
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
    let designated_role = role;
    let role = program
        .product_operand_declaration_role(Some(role), declaration)
        .map_err(unserved)?
        .ok_or(unserved("a Product application with no declaration role"))?;
    let (declaration_parameters, lifetimes) = match role {
        ProductOperandRole::TestRequirement => {
            let (owner, requirement) = program
                .trait_machine_signature(declaration)
                .filter(|(owner, requirement)| !owner.is_boundary && requirement.symbol == declaration)
                .ok_or(unserved("a test group without an exact ordinary trait requirement"))?;
            if !requirement.type_parameters.is_empty() || !requirement.lifetime_parameters.is_empty() {
                return Err(unserved("a test group requirement with open machine parameters"));
            }
            (owner.type_parameters, owner.lifetime_parameters.len())
        }
        ProductOperandRole::Requirement => program
            .trait_machine_signature(declaration)
            .filter(|(owner, requirement)| owner.is_boundary && requirement.symbol == declaration)
            .ok_or(unserved("an applied requirement with no exact boundary declaration"))
            .and_then(|(owner, requirement)| {
                if !owner.type_parameters.is_empty() || !owner.lifetime_parameters.is_empty() {
                    return Err(unserved("a requirement without its complete declaring trait application"));
                }
                Ok((requirement.type_parameters, requirement.lifetime_parameters.len()))
            })?,
        ProductOperandRole::BoundarySubject => program
            .trait_definition_by_symbol(declaration)
            .map(|definition| {
                (
                    definition.type_parameters,
                    definition.lifetime_parameters.len(),
                )
            })
            .ok_or(unserved("an applied slot that is not a boundary trait"))?,
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
        return Err(unserved(
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
            selecting_operand: operand.source_span,
            nodes: 0,
        };
        let argument = lower.argument(argument, parameter.kind == TypeParameterKind::Type)?;
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
    let scope = if role == ProductOperandRole::Requirement {
        let (owner, _) = program
            .trait_machine_signature(declaration)
            .ok_or(unserved("an applied requirement lost its declaring trait"))?;
        AppliedTraitScope::abstract_scope(owner).environment()
    } else {
        let unformed = |_| unserved("an application whose arguments have no type form");
        let references = closed
            .iter()
            .map(|argument| program.static_operand_type_reference_in_scope(argument, author))
            .collect::<Result<Vec<_>, _>>()
            .map_err(unformed)?;
        let incomplete = unserved("an application with no complete trait scope");
        match role {
            ProductOperandRole::BoundarySubject | ProductOperandRole::TestRequirement => {
                let definition = if role == ProductOperandRole::TestRequirement {
                    program
                        .trait_machine_signature(declaration)
                        .map(|(owner, _)| owner)
                } else {
                    program.trait_definition_by_symbol(declaration)
                }
                .ok_or(unserved("a Product application with no declaring trait"))?;
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
                    .ok_or(unserved("an applied provider with no declaration"))?;
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
        .ok_or(incomplete)?
    };
    if role == ProductOperandRole::BoundarySubject {
        prepare_boundary_requirements(program, &scope, author)?;
    }
    let requirement = matches!(
        role,
        ProductOperandRole::Requirement | ProductOperandRole::TestRequirement
    );
    let local_arguments = if role == ProductOperandRole::Requirement {
        closed.as_slice()
    } else {
        &[]
    };
    let supplying = program.expression_table.retain_static_machine_application(
        declaration,
        Identifier::generated(&operand.path),
        if requirement {
            local_arguments
        } else {
            std::slice::from_ref(&whole)
        },
    );
    let retained = program
        .expression_table
        .static_machine_application_mut(supplying);
    retained.binding_declaration = declaration;
    retained.lexical_owner = author;
    let selected = if requirement {
        use crate::static_operand_binding::{
            Stop as RequirementStop, prepare_requirement_in_scope,
        };
        prepare_requirement_in_scope(program, &scope, declaration, supplying, local_arguments)
            .map_err(|stop| match stop {
                RequirementStop::Unserved(reason) => Stop::Unserved(reason),
                RequirementStop::Rejected(reason) => Stop::Rejected(reason),
                RequirementStop::Defect(reason) => {
                    Stop::Rejected(format!("Product requirement preparation defect: {reason}"))
                }
            })?
            .ok_or(unserved(
                "a requirement application with no canonical closed header",
            ))?
    } else {
        declaration
    };
    let scope = scope.retain(program).map_err(unserved)?;
    Ok(Some(program.retain_product_operand_application(
        ProductOperandApplication {
            operand: operand.clone(),
            environment: environment.cloned().map(std::sync::Arc::new),
            owner: author,
            role: Some(designated_role),
            declaration: selected,
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

/// A selected service owes every requirement in its complete parent graph.
/// Prepare concrete receiving headers here; the ordinary schema and body
/// judgments still check all rows, including locally generic requirements.
fn prepare_boundary_requirements(
    program: &mut TypedTrees,
    root: &crate::source_state::trait_definition::TraitApplicationEnvironment,
    author: SymbolHandle,
) -> Formed<()> {
    let mut pending = vec![root.clone()];
    let mut visited = std::collections::BTreeSet::new();
    while let Some(environment) = pending.pop() {
        let scope = environment.scope(program).ok_or(unserved(
            "selected service lost its complete trait application",
        ))?;
        let identity = scope.application_identity(program).ok_or(unserved(
            "selected service has no canonical trait application",
        ))?;
        if !visited.insert(identity) {
            continue;
        }
        let requirements = program
            .trait_machine_signatures(scope.declaration)
            .iter()
            .filter(|signature| {
                program
                    .state_signature_type_parameters(signature)
                    .is_empty()
            })
            .map(|signature| (signature.symbol, signature.name.clone()))
            .collect::<Vec<_>>();
        pending.extend(
            scope
                .parent_scopes(program)
                .map_err(unserved)?
                .into_iter()
                .map(|parent| parent.environment()),
        );
        for (requirement, name) in requirements {
            let application =
                program
                    .expression_table
                    .retain_static_machine_application(requirement, name, &[]);
            let row = program
                .expression_table
                .static_machine_application_mut(application);
            row.lexical_owner = author;
            row.binding_declaration = requirement;
            crate::static_operand_binding::prepare_requirement_in_scope(
                program,
                &environment,
                requirement,
                application,
                &[],
            )
            .map_err(|stop| match stop {
                crate::static_operand_binding::Stop::Unserved(reason) => {
                    Stop::Unserved(reason)
                }
                crate::static_operand_binding::Stop::Rejected(reason) => {
                    Stop::Rejected(reason)
                }
                crate::static_operand_binding::Stop::Defect(reason) => {
                    Stop::Rejected(reason.into())
                }
            })?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deferred_builtin_type_names_keep_the_selected_binders_type_role() {
        let mut builder = crate::source_symbols::SymbolTableBuilder::new();
        let root = builder.insert_root(SymbolKind::Root, symbols::SymbolNameRef::Static("root"));
        builder.insert_children(root, crate::source_symbols::builtin_type_symbols());
        let mut program = TypedTrees {
            symbols: builder.finish(),
            ..Default::default()
        };
        for name in ["u32", "u13", "u0"] {
            let mut graph = ProductStaticOperandGraph::default();
            let path = graph.names.insert_many([ProductOperandName {
                text: name.into(),
                ..Default::default()
            }]);
            let operand = graph.operands.insert(ProductStaticOperand {
                path,
                ..Default::default()
            });
            let mut lower = Lower {
                program: &mut program,
                source: None,
                graph: &graph,
                environment: None,
                helper: SymbolHandle::invalid(),
                selecting_operand: Default::default(),
                nodes: 0,
            };
            match lower.argument(operand, true) {
                Ok(Closed::Type(reference)) if name != "u0" => {
                    let TypeReferenceNode::Named { symbol, .. } =
                        lower.program.type_reference_table.type_reference(reference)
                    else {
                        panic!("a builtin retains its resolved type symbol");
                    };
                    assert_eq!(lower.program.symbols.name(*symbol), name);
                    assert!(lower.program.symbols.builtin_type_atom(*symbol).is_some());
                }
                Err(Stop::Rejected(reason)) if name == "u0" => {
                    assert!(reason.contains("invalid builtin type"), "{reason}");
                }
                _ => panic!("unexpected static type outcome for {name}"),
            }
        }
    }
}
