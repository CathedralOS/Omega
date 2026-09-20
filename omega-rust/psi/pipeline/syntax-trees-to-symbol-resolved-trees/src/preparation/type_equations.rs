//! Shared structural `where Binder == <type>` equation matching.
//!
//! A declaration template's `where` clause may equate a declared type binder with
//! known type structure (`Length == u64[0..=Capacity]`). Applying the
//! template with fewer arguments than binders recovers the omitted trailing
//! binders by matching each equation's structure against the supplied
//! argument; a complete tuple must still satisfy every equation exactly, and
//! a verified equation is an instantiation obligation the instance no longer
//! carries. A supplied range shell is identified only by its retained
//! canonical `IntegerRangeNormalization`: no rendered spelling and no
//! compatibility interval stands in for it, so `u64[0..257]` and
//! `u64[0..=256]` bind the same capacity, and a larger containing interval
//! does not satisfy the equation.
//!
//! Recovery runs in both directions of one equation. An omitted value binder
//! reads its endpoint out of a supplied shell; an omitted *type* binder is
//! built from the shell once the equation's own endpoints are closed
//! integers, so `Bytes<const Capacity: u64, Length> where Length ==
//! u64[0..=Capacity]` applied as `Bytes<256>` binds Length to `u64[0..=256]`.
//! The constructed argument is an ordinary constrained type reference
//! carrying the same canonical normalization the authored spelling would
//! carry, so every later equation, repeat occurrence and closed identity
//! compares one shape.
//!
//! Fixed arrays, slices, anonymous references and declared generic applications
//! retain ordinary type trees.
//! Their type, integer and Boolean const positions recursively recover binders, or build
//! an omitted type once those binders are known. `type_structure` owns this
//! traversal: nominal heads join by selected declaration, never layout or leaf
//! spelling. Closed leaves still use `closed_argument_identity`.
//! Borrow and singleton-array operands gain type role only opposite a declared
//! type binder. They materialize into that same type tree; ordinary value-borrow
//! equalities remain value facts, never parser guesses about identifier spelling.
//! Runtime contents and compatible storage sizes never supply an argument.
//! Data synthesis and machine-call preparation supply their exact selected
//! declaration telescopes; this solver owns neither call selection nor
//! publication. Machine preparation currently requires closed explicit calls
//! and sends completed type roots back through ordinary data normalization;
//! an equation does not discharge the selected constructor's own obligations.

use crate::preparation::generic_data::ClosedArgumentIdentity;
use crate::preparation::generic_data::closed_argument_identity;
use crate::preparation::generic_data::closed_name_identity;
use crate::preparation::generic_data::constant_selection;
use crate::preparation::generic_data::evaluate_const_fact_expression;
use diagnostics::Diagnostic;
use language_semantics::const_value::{CanonicalConstValue, DecodedCanonicalConstValue};
use numerics::bignum::BigInt;
use numerics::literals::{IntegerLiteral, IntegerRadix};
use source::SourceSpan;
use std::collections::HashMap;
use syntax_trees::SyntaxTrees;
use syntax_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use syntax_trees::identifier::Identifier;
use syntax_trees::item::{ProofFact, TypeParameter, TypeParameterKind};
use syntax_trees::types::{
    IntegerRangeNormalization, TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode,
};

mod type_structure;

/// One declaration-level `where` fact that states a type equation, by its offset in
/// the template's fact span. Instances never carry it: applying the template
/// decides it against the complete argument tuple.
#[derive(Clone)]
pub(crate) struct TypeEquation {
    pub(crate) fact_offset: usize,
    shape: EquationShape,
}

impl TypeEquation {
    pub(crate) fn validate_kind(&self, declaration: &str) -> Result<(), Diagnostic> {
        if let EquationShape::KindMismatch { span, description } = &self.shape {
            return Err(Diagnostic::error(format!(
                "{declaration} where equation mixes type and value kinds: {description}"
            ))
            .with_source_span(*span));
        }
        Ok(())
    }
}

/// Type-reference operands have no runtime expression representation. Syntax
/// retains their equations for every specialization; lowering excludes only
/// these obligations, recognized by the same classifier that synthesis solves.
/// Existing name/range expression facts keep their current lowering, including
/// provisional resolution used to evaluate range endpoints before synthesis.
/// Closed instances must already have discharged equations and take no exemption.
pub(crate) fn template_type_equation_offsets(
    syntax: &SyntaxTrees,
    definition: &syntax_trees::item::DataDefinition,
) -> Result<Vec<usize>, Diagnostic> {
    if definition.type_parameters.is_empty() {
        return Ok(Vec::new());
    }
    let mut offsets = Vec::new();
    for equation in classify_type_equations(
        syntax,
        syntax.items.type_parameters(definition.type_parameters),
        syntax.items.proof_facts(definition.where_facts),
    ) {
        match equation.shape {
            EquationShape::Structural {
                structure: Structure::TypeReference(_) | Structure::TypeOperand(_),
                ..
            } => offsets.push(equation.fact_offset),
            EquationShape::Structural { .. } => {}
            EquationShape::KindMismatch { span, description } => {
                return Err(Diagnostic::error(format!(
                    "generic data `{}` where equation mixes type and value kinds: {description}",
                    definition.name.as_str(),
                ))
                .with_source_span(span));
            }
        }
    }
    Ok(offsets)
}

/// Synthesis may defer containers it cannot specialize (for example, an
/// independently generic attached method). Such a surviving application has
/// not discharged the type-reference equation and cannot use the runtime-fact
/// exemption. Closed instances lower as Named references; only their retained
/// application metadata may still describe the original generic tuple.
pub(crate) fn validate_materialized_type_equations(
    syntax: &SyntaxTrees,
    reference: TypeReferenceHandle,
    selection: Option<&constant_selection::ConstantSelection>,
) -> Result<(), Diagnostic> {
    let TypeReferenceNode::Generic { base_name, .. } =
        syntax.type_references.type_reference(reference)
    else {
        return Ok(());
    };
    let Some(declaration) =
        crate::preparation::generic_data::selected_data_item(syntax, selection, base_name)
    else {
        return Ok(());
    };
    let syntax_trees::item::Item::Data(definition) = syntax.root_item(declaration) else {
        return Ok(());
    };
    if !template_type_equation_offsets(syntax, definition)?.is_empty() {
        return Err(Diagnostic::error(format!(
            "generic data `{}` retains unsolved type equations; its application requires closed instance specialization",
            definition.name.as_str(),
        )).with_source_span(base_name.source_span()));
    }
    Ok(())
}

#[derive(Clone)]
enum EquationShape {
    /// `binder == structure` with a `type` binder on the binder side.
    Structural {
        binder: usize,
        structure: Structure,
        span: SourceSpan,
    },
    /// One side names a range shell or a type binder, but the other side is
    /// a value, a const binder, or a value binder: the sides cannot be
    /// equated as types, and nothing converts a type to a value.
    KindMismatch {
        span: SourceSpan,
        description: String,
    },
}

#[derive(Clone)]
enum Structure {
    /// Borrow/singleton-array syntax whose type role follows from the opposite
    /// declared type binder, not from parser guesses about identifier spelling.
    TypeOperand(ExpressionHandle),
    /// Type-role syntax retains recursive constructors instead of rendering them
    /// as names or treating their operands as runtime array elements.
    TypeReference(TypeReferenceHandle),
    /// `carrier[minimum..maximum]` or `carrier[minimum..=maximum]`.
    RangeShell {
        carrier: Identifier,
        /// The carrier position names a type binder rather than a closed type.
        carrier_binder: Option<usize>,
        minimum: Endpoint,
        maximum: Endpoint,
        end_inclusive: bool,
    },
    /// A single name: a closed type, or another type binder.
    Name {
        name: Identifier,
        binder: Option<usize>,
    },
}

#[derive(Clone)]
enum Endpoint {
    Literal(BigInt),
    /// A bare const binder of the template.
    Binder(usize),
    /// Any other expression, compared once every binder it mentions is bound.
    Expression(ExpressionHandle),
    Absent,
}

impl Structure {
    fn binder_mentions(&self, syntax: &SyntaxTrees, parameters: &[String]) -> Vec<usize> {
        let mut mentions = Vec::new();
        match self {
            Structure::TypeOperand(expression) => {
                collect_expression_binder_mentions(syntax, *expression, parameters, &mut mentions);
            }
            Structure::TypeReference(reference) => {
                type_structure::collect_binder_mentions(
                    syntax,
                    *reference,
                    parameters,
                    &mut mentions,
                );
            }
            Structure::RangeShell {
                carrier_binder,
                minimum,
                maximum,
                ..
            } => {
                mentions.extend(*carrier_binder);
                for endpoint in [minimum, maximum] {
                    match endpoint {
                        Endpoint::Binder(index) => mentions.push(*index),
                        Endpoint::Expression(expression) => {
                            collect_expression_binder_mentions(
                                syntax,
                                *expression,
                                parameters,
                                &mut mentions,
                            );
                        }
                        Endpoint::Literal(_) | Endpoint::Absent => {}
                    }
                }
            }
            Structure::Name { binder, .. } => mentions.extend(*binder),
        }
        mentions
    }
}

enum Side {
    AmbiguousTypeOperand(ExpressionHandle),
    TypeStructure(TypeReferenceHandle),
    TypeBinder(usize),
    ValueBinder(usize),
    ClosedName(Identifier),
    RangeShell {
        carrier: Identifier,
        start: ExpressionHandle,
        end: ExpressionHandle,
        end_inclusive: bool,
    },
    Other(&'static str),
}

fn binder_index<'parameters>(
    parameters: &'parameters [TypeParameter],
    name: &str,
) -> Option<(usize, &'parameters TypeParameterKind)> {
    parameters
        .iter()
        .position(|parameter| parameter.name.as_str() == name)
        .map(|index| (index, &parameters[index].kind))
}

fn single_name(syntax: &SyntaxTrees, expression: ExpressionHandle) -> Option<&Identifier> {
    let ExpressionNode::Name(path) = syntax.expressions.expression(expression) else {
        return None;
    };
    let [member] = syntax.expressions.identifier_path_members(*path) else {
        return None;
    };
    Some(member)
}

/// Classify every declaration-level `where` fact that equates a type binder or a
/// range shell. Facts over const binders and values alone (`Capacity > 0`)
/// stay ordinary instantiation obligations for the const evaluator.
pub(crate) fn classify_type_equations(
    syntax: &SyntaxTrees,
    parameters: &[TypeParameter],
    where_facts: &[ProofFact],
) -> Vec<TypeEquation> {
    where_facts
        .iter()
        .enumerate()
        .filter_map(|(fact_offset, fact)| {
            let ProofFact::Expression(expression) = fact else {
                return None;
            };
            let ExpressionNode::Binary(binary) = syntax.expressions.expression(*expression) else {
                return None;
            };
            if binary.operator != BinaryOperator::Equal {
                return None;
            }
            let span = syntax.expressions.source_span(*expression);
            let left = classify_side(syntax, parameters, binary.left);
            let right = classify_side(syntax, parameters, binary.right);
            let shape = match (left, right) {
                (Side::TypeBinder(binder), other) | (other, Side::TypeBinder(binder)) => {
                    equation_shape(syntax, parameters, binder, other, span)
                }
                (Side::RangeShell { .. }, other) | (other, Side::RangeShell { .. }) => {
                    EquationShape::KindMismatch {
                        span,
                        description: format!(
                            "{} cannot equal a range shell",
                            describe_side(parameters, &other)
                        ),
                    }
                }
                (Side::TypeStructure(_), other) | (other, Side::TypeStructure(_)) => {
                    EquationShape::KindMismatch {
                        span,
                        description: format!(
                            "{} cannot equal a type structure",
                            describe_side(parameters, &other)
                        ),
                    }
                }
                _ => return None,
            };
            Some(TypeEquation { fact_offset, shape })
        })
        .collect()
}

fn classify_side(
    syntax: &SyntaxTrees,
    parameters: &[TypeParameter],
    expression: ExpressionHandle,
) -> Side {
    match syntax.expressions.expression(expression) {
        ExpressionNode::Borrow(_) | ExpressionNode::ArrayLiteral(_) => {
            Side::AmbiguousTypeOperand(expression)
        }
        ExpressionNode::TypeExpression(reference) => Side::TypeStructure(*reference),
        ExpressionNode::Name(path) => {
            let [member] = syntax.expressions.identifier_path_members(*path) else {
                return Side::Other("a qualified name");
            };
            match binder_index(parameters, member.as_str()) {
                Some((index, TypeParameterKind::Type)) => Side::TypeBinder(index),
                Some((index, _)) => Side::ValueBinder(index),
                None => Side::ClosedName(member.clone()),
            }
        }
        ExpressionNode::Indexed(indexed) => {
            let Some(carrier) = single_name(syntax, indexed.collection) else {
                return Side::Other("an indexed expression");
            };
            let ExpressionNode::Range(range) = syntax.expressions.expression(indexed.index) else {
                return Side::Other("an indexed expression");
            };
            Side::RangeShell {
                carrier: carrier.clone(),
                start: range.start,
                end: range.end,
                end_inclusive: range.end_inclusive,
            }
        }
        _ => Side::Other("a value expression"),
    }
}

fn describe_side(parameters: &[TypeParameter], side: &Side) -> String {
    match side {
        Side::AmbiguousTypeOperand(_) => "a reference or slice operand".to_owned(),
        Side::TypeStructure(_) => "a type structure".to_owned(),
        Side::TypeBinder(index) => format!("type binder `{}`", parameters[*index].name.as_str()),
        Side::ValueBinder(index) => {
            format!("value binder `{}`", parameters[*index].name.as_str())
        }
        Side::ClosedName(name) => format!("`{}`", name.as_str()),
        Side::RangeShell { carrier, .. } => format!("range shell on `{}`", carrier.as_str()),
        Side::Other(what) => (*what).to_owned(),
    }
}

fn equation_shape(
    syntax: &SyntaxTrees,
    parameters: &[TypeParameter],
    binder: usize,
    other: Side,
    span: SourceSpan,
) -> EquationShape {
    let binder_name = parameters[binder].name.as_str();
    let mismatch = |description: String| EquationShape::KindMismatch { span, description };
    let structure = match other {
        Side::AmbiguousTypeOperand(expression) => {
            if let Some(reason) = type_operand_kind_error(syntax, parameters, expression) {
                return mismatch(reason.to_owned());
            }
            Structure::TypeOperand(expression)
        }
        Side::TypeStructure(reference) => Structure::TypeReference(reference),
        Side::TypeBinder(index) => Structure::Name {
            name: parameters[index].name.clone(),
            binder: Some(index),
        },
        Side::ClosedName(name) => Structure::Name { name, binder: None },
        Side::ValueBinder(index) => {
            return mismatch(format!(
                "type binder `{binder_name}` cannot equal value binder `{}`",
                parameters[index].name.as_str()
            ));
        }
        Side::Other(what) => {
            return mismatch(format!("type binder `{binder_name}` cannot equal {what}"));
        }
        Side::RangeShell {
            carrier,
            start,
            end,
            end_inclusive,
        } => {
            let carrier_binder = match binder_index(parameters, carrier.as_str()) {
                Some((index, TypeParameterKind::Type)) => Some(index),
                Some(_) => {
                    return mismatch(format!(
                        "value binder `{}` cannot carry a range shell",
                        carrier.as_str()
                    ));
                }
                None => None,
            };
            let (minimum, maximum) = match (
                classify_endpoint(syntax, parameters, start),
                classify_endpoint(syntax, parameters, end),
            ) {
                (Ok(minimum), Ok(maximum)) => (minimum, maximum),
                (Err(description), _) | (_, Err(description)) => return mismatch(description),
            };
            Structure::RangeShell {
                carrier,
                carrier_binder,
                minimum,
                maximum,
                end_inclusive,
            }
        }
    };
    EquationShape::Structural {
        binder,
        structure,
        span,
    }
}

fn classify_endpoint(
    syntax: &SyntaxTrees,
    parameters: &[TypeParameter],
    expression: ExpressionHandle,
) -> Result<Endpoint, String> {
    if !expression.is_valid() {
        return Ok(Endpoint::Absent);
    }
    match syntax.expressions.expression(expression) {
        ExpressionNode::Integer(literal) => Ok(literal
            .value_bignum()
            .map_or(Endpoint::Expression(expression), Endpoint::Literal)),
        ExpressionNode::Name(_) => match single_name(syntax, expression)
            .and_then(|name| binder_index(parameters, name.as_str()))
        {
            Some((_, TypeParameterKind::Type)) => Err(format!(
                "type binder `{}` cannot be a range endpoint",
                single_name(syntax, expression)
                    .expect("a single-segment name was matched")
                    .as_str()
            )),
            Some((index, _)) => Ok(Endpoint::Binder(index)),
            None => Ok(Endpoint::Expression(expression)),
        },
        _ => Ok(Endpoint::Expression(expression)),
    }
}

/// Resolve the operand's role only after the opposite side selected a declared
/// type binder. Ordinary borrow/array equalities remain value propositions.
fn type_operand_kind_error(
    syntax: &SyntaxTrees,
    parameters: &[TypeParameter],
    expression: ExpressionHandle,
) -> Option<&'static str> {
    match syntax.expressions.expression(expression) {
        ExpressionNode::Name(path) => {
            if let [name] = syntax.expressions.identifier_path_members(*path)
                && binder_index(parameters, name.as_str())
                    .is_some_and(|(_, kind)| !matches!(kind, TypeParameterKind::Type))
            {
                return Some("a value binder cannot supply a reference or slice element type");
            }
            None
        }
        ExpressionNode::Borrow(borrow) => {
            type_operand_kind_error(syntax, parameters, borrow.target)
        }
        ExpressionNode::ArrayLiteral(elements) => {
            let [element] = syntax.expressions.expression_handles(*elements) else {
                return Some("a slice type requires exactly one element type");
            };
            type_operand_kind_error(syntax, parameters, *element)
        }
        ExpressionNode::TypeExpression(_) => None,
        _ => Some("a value expression cannot supply a reference or slice element type"),
    }
}

fn collect_expression_binder_mentions(
    syntax: &SyntaxTrees,
    expression: ExpressionHandle,
    parameters: &[String],
    mentions: &mut Vec<usize>,
) {
    if !expression.is_valid() {
        return;
    }
    match syntax.expressions.expression(expression) {
        ExpressionNode::Borrow(borrow) => {
            collect_expression_binder_mentions(syntax, borrow.target, parameters, mentions);
        }
        ExpressionNode::ArrayLiteral(elements) => {
            for element in syntax.expressions.expression_handles(*elements) {
                collect_expression_binder_mentions(syntax, *element, parameters, mentions);
            }
        }
        ExpressionNode::TypeExpression(reference) => {
            type_structure::collect_binder_mentions(syntax, *reference, parameters, mentions);
        }
        ExpressionNode::Name(path) => {
            if let [member] = syntax.expressions.identifier_path_members(*path)
                && let Some(index) = parameters
                    .iter()
                    .position(|parameter| parameter == member.as_str())
            {
                mentions.push(index);
            }
        }
        ExpressionNode::Binary(binary) => {
            collect_expression_binder_mentions(syntax, binary.left, parameters, mentions);
            collect_expression_binder_mentions(syntax, binary.right, parameters, mentions);
        }
        ExpressionNode::Unary(unary) => {
            collect_expression_binder_mentions(syntax, unary.operand, parameters, mentions);
        }
        ExpressionNode::Indexed(indexed) => {
            collect_expression_binder_mentions(syntax, indexed.collection, parameters, mentions);
            collect_expression_binder_mentions(syntax, indexed.index, parameters, mentions);
        }
        ExpressionNode::Range(range) => {
            collect_expression_binder_mentions(syntax, range.start, parameters, mentions);
            collect_expression_binder_mentions(syntax, range.end, parameters, mentions);
        }
        ExpressionNode::Member(member) => {
            collect_expression_binder_mentions(syntax, member.receiver, parameters, mentions);
        }
        ExpressionNode::Call(call) => {
            for argument in syntax.expressions.expression_handles(call.arguments) {
                collect_expression_binder_mentions(syntax, *argument, parameters, mentions);
            }
        }
        _ => {}
    }
}

/// Declaration-neutral inputs for structural equation completion. Data and
/// machine applications retain their own selection and publication owners.
#[derive(Clone, Copy)]
pub(crate) struct EquationTemplate<'template> {
    pub(crate) kind: &'static str,
    pub(crate) name: &'template str,
    pub(crate) parameters: arena::HandleSpan<TypeParameter>,
    pub(crate) parameter_names: &'template [String],
    pub(crate) const_parameter_types: &'template [Option<TypeReferenceHandle>],
    pub(crate) type_equations: &'template [TypeEquation],
}

pub(crate) fn complete_equation_arguments(
    syntax: &mut SyntaxTrees,
    base_info: EquationTemplate<'_>,
    base_name: &Identifier,
    supplied: &[TypeReferenceHandle],
    const_values: &HashMap<String, i128>,
    selection: Option<&constant_selection::ConstantSelection>,
    warnings: &mut Vec<Diagnostic>,
) -> Result<Vec<TypeReferenceHandle>, Diagnostic> {
    let bindings = {
        let mut solver = Solver::new(
            &mut *syntax,
            base_info,
            base_name,
            supplied,
            const_values,
            selection,
        );
        solver.solve(warnings)?;
        solver.bindings
    };
    let mut tuple = supplied.to_vec();
    for binding in bindings.into_iter().skip(supplied.len()) {
        let node = match binding {
            Some(Binding::Type(handle)) => {
                tuple.push(handle);
                continue;
            }
            Some(Binding::NamedType(name)) => {
                TypeReferenceNode::Named(Identifier::new(name.as_str(), name.source_span()))
            }
            Some(Binding::Integer(value)) => {
                TypeReferenceNode::Named(Identifier::generated(value.to_string()))
            }
            Some(Binding::Boolean(value)) => TypeReferenceNode::Named(Identifier::generated(
                CanonicalConstValue::boolean(value).atom(),
            )),
            Some(Binding::Opaque) | None => {
                unreachable!("solving binds every omitted binder or rejects")
            }
        };
        tuple.push(syntax.tables.type_references.insert(node));
    }
    Ok(tuple)
}

#[derive(Clone)]
enum Binding {
    /// A supplied or forwarded type argument.
    Type(TypeReferenceHandle),
    /// A type binder recovered from a closed name equation; materialized as a
    /// fresh `Named` leaf once solving completes.
    NamedType(Identifier),
    Integer(BigInt),
    /// Boolean index identity never participates in integer endpoint arithmetic.
    Boolean(bool),
    /// A supplied const argument that is not an integer literal (a canonical
    /// structured atom). Equations cannot relate it to an endpoint.
    Opaque,
}

enum Outcome {
    Settled,
    Deferred,
}

/// The carrier of a range shell being constructed for an omitted type binder:
/// an already bound type argument, or the equation's own closed carrier name.
enum ConstructedCarrier {
    Bound(TypeReferenceHandle),
    Name(Identifier),
}

struct Solver<'a, 's> {
    /// Constructing an omitted type binder's range shell appends its carrier,
    /// endpoints and canonical normalization to these tables while solving.
    syntax: &'a mut SyntaxTrees,
    base_info: EquationTemplate<'a>,
    base_name: &'a Identifier,
    const_values: &'a HashMap<String, i128>,
    selection: Option<&'a constant_selection::ConstantSelection<'s>>,
    supplied_count: usize,
    bindings: Vec<Option<Binding>>,
    settled: Vec<bool>,
}

impl<'a, 's> Solver<'a, 's> {
    fn new(
        syntax: &'a mut SyntaxTrees,
        base_info: EquationTemplate<'a>,
        base_name: &'a Identifier,
        supplied: &[TypeReferenceHandle],
        const_values: &'a HashMap<String, i128>,
        selection: Option<&'a constant_selection::ConstantSelection<'s>>,
    ) -> Self {
        let mut bindings = vec![None; base_info.parameter_names.len()];
        for (index, argument) in supplied.iter().enumerate() {
            bindings[index] = Some(if base_info.const_parameter_types[index].is_some() {
                match syntax.tables.type_references.type_reference(*argument) {
                    TypeReferenceNode::Named(value) => {
                        if let Some(value) = normalized_boolean_argument(value.as_str()) {
                            Binding::Boolean(value)
                        } else {
                            value
                                .as_str()
                                .parse::<i128>()
                                .map_or(Binding::Opaque, |value| {
                                    Binding::Integer(BigInt::from_i128(value))
                                })
                        }
                    }
                    _ => Binding::Opaque,
                }
            } else {
                Binding::Type(*argument)
            });
        }
        Self {
            syntax,
            base_info,
            base_name,
            const_values,
            selection,
            supplied_count: supplied.len(),
            bindings,
            settled: vec![false; base_info.type_equations.len()],
        }
    }

    fn reject(&self, message: String) -> Diagnostic {
        Diagnostic::error(format!(
            "generic {} `{}` {message}",
            self.base_info.kind, self.base_info.name
        ))
        .with_source_span(self.base_name.source_span())
    }

    fn binder_name(&self, index: usize) -> &str {
        self.base_info.parameter_names[index].as_str()
    }

    fn solve(&mut self, warnings: &mut Vec<Diagnostic>) -> Result<(), Diagnostic> {
        loop {
            let mut progress = false;
            for index in 0..self.base_info.type_equations.len() {
                if self.settled[index] {
                    continue;
                }
                let equation = self.base_info.type_equations[index].clone();
                if let Outcome::Settled = self.apply(&equation, warnings)? {
                    self.settled[index] = true;
                    progress = true;
                }
            }
            if !progress {
                break;
            }
        }
        if let Some(index) = self.bindings.iter().position(Option::is_none) {
            let binder = self.binder_name(index);
            if self.defined_through_cycle(index) {
                return Err(self.reject(format!(
                    "where equations define `{binder}` through itself; supply the argument explicitly"
                )));
            }
            if let Some(owner) = self.expression_endpoint_mentioning(index) {
                return Err(self.reject(format!(
                    "where equation on `{}` cannot bind `{binder}`: structural inference binds a bare const binder, not an expression over it; supply the argument explicitly",
                    self.binder_name(owner)
                )));
            }
            if self.has_range_shell_equation(index) {
                return Err(self.reject(format!(
                    "cannot construct type binder `{binder}` from its range-shell equation; supply the argument explicitly"
                )));
            }
            return Err(self.reject(format!(
                "cannot determine `{binder}` from its where equations; supply the argument explicitly"
            )));
        }
        if let Some(index) = self.settled.iter().position(|settled| !settled) {
            let EquationShape::Structural { binder, span, .. } =
                &self.base_info.type_equations[index].shape
            else {
                unreachable!("kind mismatches reject when applied");
            };
            return Err(self
                .reject(format!(
                    "where equation on `{}` cannot be decided from the supplied arguments",
                    self.binder_name(*binder)
                ))
                .with_source_span(*span));
        }
        Ok(())
    }

    fn equations_defining(&self, binder: usize) -> impl Iterator<Item = &Structure> {
        self.base_info
            .type_equations
            .iter()
            .filter_map(move |equation| match &equation.shape {
                EquationShape::Structural {
                    binder: defined,
                    structure,
                    ..
                } if *defined == binder => Some(structure),
                _ => None,
            })
    }

    /// The binder side of an equation whose endpoint expression mentions
    /// `binder` without being that bare binder: `0..=N * 2` cannot bind N.
    fn expression_endpoint_mentioning(&self, binder: usize) -> Option<usize> {
        self.base_info
            .type_equations
            .iter()
            .find_map(|equation| match &equation.shape {
                EquationShape::Structural {
                    binder: owner,
                    structure:
                        Structure::RangeShell {
                            minimum, maximum, ..
                        },
                    ..
                } => [minimum, maximum].into_iter().find_map(|endpoint| {
                    let Endpoint::Expression(expression) = endpoint else {
                        return None;
                    };
                    let mut mentions = Vec::new();
                    collect_expression_binder_mentions(
                        self.syntax,
                        *expression,
                        self.base_info.parameter_names,
                        &mut mentions,
                    );
                    mentions.contains(&binder).then_some(*owner)
                }),
                _ => None,
            })
    }

    fn has_range_shell_equation(&self, binder: usize) -> bool {
        self.equations_defining(binder)
            .any(|structure| matches!(structure, Structure::RangeShell { .. }))
    }

    /// Whether following `binder == structure` edges from `start` returns to
    /// `start`: the binder is defined through itself.
    fn defined_through_cycle(&self, start: usize) -> bool {
        let mut stack = vec![start];
        let mut visited = Vec::new();
        while let Some(node) = stack.pop() {
            for structure in self.equations_defining(node) {
                for mention in
                    structure.binder_mentions(self.syntax, self.base_info.parameter_names)
                {
                    if mention == start {
                        return true;
                    }
                    if !visited.contains(&mention) {
                        visited.push(mention);
                        stack.push(mention);
                    }
                }
            }
        }
        false
    }

    fn apply(
        &mut self,
        equation: &TypeEquation,
        warnings: &mut Vec<Diagnostic>,
    ) -> Result<Outcome, Diagnostic> {
        match &equation.shape {
            EquationShape::KindMismatch { span, description } => Err(self
                .reject(format!(
                    "where equation mixes type and value kinds: {description}"
                ))
                .with_source_span(*span)),
            EquationShape::Structural {
                binder,
                structure,
                span,
            } => {
                if structure
                    .binder_mentions(self.syntax, self.base_info.parameter_names)
                    .contains(binder)
                {
                    return Err(self
                        .reject(format!(
                            "where equations define `{}` through itself; supply the argument explicitly",
                            self.binder_name(*binder)
                        ))
                        .with_source_span(*span));
                }
                match self.bindings[*binder].clone() {
                    Some(Binding::Type(handle)) => {
                        self.match_structure(*binder, handle, structure, *span, warnings)
                    }
                    Some(Binding::NamedType(name)) => {
                        let Structure::Name {
                            name: other,
                            binder: other_binder,
                        } = structure
                        else {
                            return Err(self
                                .reject(format!(
                                    "cannot construct type binder `{}` from its range-shell equation; supply the argument explicitly",
                                    self.binder_name(*binder)
                                ))
                                .with_source_span(*span));
                        };
                        let expected = closed_name_identity(self.syntax, self.selection, &name);
                        self.match_name(*binder, expected, other, *other_binder, *span)
                    }
                    Some(Binding::Integer(_) | Binding::Boolean(_) | Binding::Opaque) => Err(self
                        .reject(format!(
                            "where equation mixes type and value kinds: type binder `{}` received a value argument",
                            self.binder_name(*binder)
                        ))
                        .with_source_span(*span)),
                    None => self.recover_binder(*binder, structure, *span, warnings),
                }
            }
        }
    }

    /// The binder side is unbound: a closed name, an already bound type
    /// binder, or the equation's own range shell defines it.
    fn recover_binder(
        &mut self,
        binder: usize,
        structure: &Structure,
        span: SourceSpan,
        warnings: &mut Vec<Diagnostic>,
    ) -> Result<Outcome, Diagnostic> {
        let (name, other) = match structure {
            Structure::TypeOperand(expression) => {
                let pattern = self.type_operand_reference(*expression, span)?;
                let Some(reference) = self.construct_type_structure(pattern, span)? else {
                    return Ok(Outcome::Deferred);
                };
                self.bindings[binder] = Some(Binding::Type(reference));
                return Ok(Outcome::Settled);
            }
            Structure::TypeReference(reference) => {
                let Some(reference) = self.construct_type_structure(*reference, span)? else {
                    return Ok(Outcome::Deferred);
                };
                self.bindings[binder] = Some(Binding::Type(reference));
                return Ok(Outcome::Settled);
            }
            Structure::Name {
                name,
                binder: other,
            } => (name, other),
            Structure::RangeShell {
                carrier,
                carrier_binder,
                minimum,
                maximum,
                end_inclusive,
            } => {
                return self.construct_range_shell(
                    binder,
                    carrier,
                    *carrier_binder,
                    minimum,
                    maximum,
                    *end_inclusive,
                    span,
                    warnings,
                );
            }
        };
        let binding = match other {
            None => Binding::NamedType(name.clone()),
            Some(other) => match self.bindings[*other].clone() {
                Some(binding @ (Binding::Type(_) | Binding::NamedType(_))) => binding,
                Some(Binding::Integer(_) | Binding::Boolean(_) | Binding::Opaque) => {
                    unreachable!("a type binder holds a type binding")
                }
                None => return Ok(Outcome::Deferred),
            },
        };
        self.bindings[binder] = Some(binding);
        Ok(Outcome::Settled)
    }

    /// Build the omitted type binder's argument out of the equation's own
    /// range shell. Every endpoint must already be a closed integer; while one
    /// is still open the equation defers, and the unsolved-binder report then
    /// asks for an explicit argument. The materialized node is an ordinary
    /// constrained type reference with the canonical inclusive interval
    /// retained beside it, which is the only thing that identifies a shell.
    #[allow(clippy::too_many_arguments)]
    fn construct_range_shell(
        &mut self,
        binder: usize,
        carrier: &Identifier,
        carrier_binder: Option<usize>,
        minimum: &Endpoint,
        maximum: &Endpoint,
        end_inclusive: bool,
        span: SourceSpan,
        warnings: &mut Vec<Diagnostic>,
    ) -> Result<Outcome, Diagnostic> {
        let binder_name = self.binder_name(binder).to_owned();
        let carrier = match carrier_binder {
            None => ConstructedCarrier::Name(carrier.clone()),
            Some(index) => match self.bindings[index].clone() {
                Some(Binding::Type(handle)) => ConstructedCarrier::Bound(handle),
                Some(Binding::NamedType(name)) => ConstructedCarrier::Name(name),
                Some(Binding::Integer(_) | Binding::Boolean(_) | Binding::Opaque) => {
                    unreachable!("a type binder holds a type binding")
                }
                None => return Ok(Outcome::Deferred),
            },
        };
        let Some(minimum) =
            self.constructed_endpoint(minimum, &binder_name, "minimum", span, warnings)?
        else {
            return Ok(Outcome::Deferred);
        };
        let Some(maximum) =
            self.constructed_endpoint(maximum, &binder_name, "maximum", span, warnings)?
        else {
            return Ok(Outcome::Deferred);
        };
        // An exclusive end lands as its proof-integer predecessor, exactly the
        // interval an authored spelling of the same shell would retain.
        let inclusive_maximum = if end_inclusive {
            maximum.clone()
        } else {
            maximum.sub(&BigInt::from_u64(1))
        };
        let handle = self.materialize_range_shell(
            carrier,
            &minimum,
            &maximum,
            &inclusive_maximum,
            end_inclusive,
            span,
            &binder_name,
        )?;
        self.bindings[binder] = Some(Binding::Type(handle));
        Ok(Outcome::Settled)
    }

    /// One endpoint of a shell being constructed, as a closed integer.
    /// `Ok(None)` defers while a binder it mentions is still unbound.
    fn constructed_endpoint(
        &self,
        endpoint: &Endpoint,
        binder_name: &str,
        position: &str,
        span: SourceSpan,
        warnings: &mut Vec<Diagnostic>,
    ) -> Result<Option<BigInt>, Diagnostic> {
        match endpoint {
            Endpoint::Absent => Err(self
                .reject(format!(
                    "cannot construct type binder `{binder_name}` from its range-shell equation: it has no {position} endpoint; supply the argument explicitly"
                ))
                .with_source_span(span)),
            Endpoint::Literal(value) => Ok(Some(value.clone())),
            Endpoint::Binder(index) => match &self.bindings[*index] {
                Some(Binding::Integer(value)) => Ok(Some(value.clone())),
                Some(Binding::Boolean(_) | Binding::Opaque) => Err(self
                    .reject(format!(
                        "where equation mixes type and value kinds: const binder `{}` holds a non-integer argument",
                        self.binder_name(*index)
                    ))
                    .with_source_span(span)),
                Some(Binding::Type(_) | Binding::NamedType(_)) => {
                    unreachable!("classification admits only value binders as endpoints")
                }
                None => Ok(None),
            },
            Endpoint::Expression(expression) => {
                self.expression_endpoint_value(*expression, binder_name, position, span, warnings)
            }
        }
    }

    /// Append the constructed shell: its carrier, one range constraint over
    /// freshly landed decimal endpoints, and the canonical normalization that
    /// gives the argument its closed identity.
    #[allow(clippy::too_many_arguments)]
    fn materialize_range_shell(
        &mut self,
        carrier: ConstructedCarrier,
        minimum: &BigInt,
        maximum: &BigInt,
        inclusive_maximum: &BigInt,
        end_inclusive: bool,
        span: SourceSpan,
        binder_name: &str,
    ) -> Result<TypeReferenceHandle, Diagnostic> {
        let base_type = match carrier {
            ConstructedCarrier::Bound(handle) => handle,
            ConstructedCarrier::Name(name) => self
                .syntax
                .tables
                .type_references
                .insert(TypeReferenceNode::Named(name)),
        };
        let minimum_expression = self.insert_integer_expression(minimum, span, binder_name)?;
        let maximum_expression = self.insert_integer_expression(maximum, span, binder_name)?;
        let constraints =
            self.syntax
                .tables
                .type_references
                .insert_constraints([TypeConstraintNode::Range {
                    minimum: minimum_expression,
                    maximum: maximum_expression,
                    end_inclusive,
                }]);
        let handle = self
            .syntax
            .tables
            .type_references
            .insert(TypeReferenceNode::Constrained {
                base_type,
                constraints,
            });
        self.syntax
            .tables
            .type_references
            .retain_integer_range_normalization(
                handle,
                0,
                IntegerRangeNormalization {
                    minimum: minimum.clone(),
                    maximum: inclusive_maximum.clone(),
                },
            );
        Ok(handle)
    }

    fn insert_integer_expression(
        &mut self,
        value: &BigInt,
        span: SourceSpan,
        binder_name: &str,
    ) -> Result<ExpressionHandle, Diagnostic> {
        let literal = IntegerLiteral::from_parts(
            value.is_negative(),
            IntegerRadix::Decimal,
            value.abs().to_string().as_str(),
        )
        .map_err(|reason| {
            self.reject(format!(
                "cannot construct type binder `{binder_name}` from its range-shell equation: {reason}"
            ))
            .with_source_span(span)
        })?;
        let handle = self
            .syntax
            .expressions
            .insert(ExpressionNode::Integer(literal));
        self.syntax.expressions.set_source_span(handle, span);
        Ok(handle)
    }

    /// Evaluate an endpoint expression once every template binder it mentions
    /// is a bound integer. `Ok(None)` means one mention is still unbound, so
    /// the caller defers; a closed expression that is not an integer rejects.
    fn expression_endpoint_value(
        &self,
        expression: ExpressionHandle,
        binder_name: &str,
        position: &str,
        span: SourceSpan,
        warnings: &mut Vec<Diagnostic>,
    ) -> Result<Option<BigInt>, Diagnostic> {
        let mut mentions = Vec::new();
        collect_expression_binder_mentions(
            self.syntax,
            expression,
            self.base_info.parameter_names,
            &mut mentions,
        );
        let mut parameter_values = HashMap::new();
        for mention in mentions {
            match &self.bindings[mention] {
                Some(Binding::Integer(value)) => {
                    let Some(value) = integer_to_i128(value) else {
                        return Err(self.reject(format!(
                            "where equation on `{binder_name}` cannot be decided: `{}` exceeds the endpoint evaluator's 64-bit envelope",
                            self.binder_name(mention)
                        )).with_source_span(span));
                    };
                    parameter_values.insert(self.binder_name(mention).to_owned(), value);
                }
                Some(_) => {
                    return Err(self
                        .reject(format!(
                            "where equation mixes type and value kinds: `{}` is not an integer const binder",
                            self.binder_name(mention)
                        ))
                        .with_source_span(span));
                }
                None => return Ok(None),
            }
        }
        let value = evaluate_const_fact_expression(
            self.syntax,
            expression,
            self.const_values,
            &parameter_values,
            None,
            warnings,
        )
        .and_then(|value| match value {
            Some(value) => value.into_integer(self.syntax, warnings),
            None => Ok(None),
        })
        .map_err(|reason| {
            self.reject(format!(
                "where equation on `{binder_name}` has an invalid {position} endpoint: {reason}"
            ))
            .with_source_span(span)
        })?;
        let Some(value) = value else {
            return Err(self.reject(format!(
                "where equation on `{binder_name}` cannot be decided: its {position} endpoint is not a closed integer; structural inference binds a bare const binder, not an expression over it"
            )).with_source_span(span));
        };
        Ok(Some(BigInt::from_i128(value)))
    }

    fn match_structure(
        &mut self,
        binder: usize,
        handle: TypeReferenceHandle,
        structure: &Structure,
        span: SourceSpan,
        warnings: &mut Vec<Diagnostic>,
    ) -> Result<Outcome, Diagnostic> {
        match structure {
            Structure::TypeOperand(expression) => {
                let pattern = self.type_operand_reference(*expression, span)?;
                self.match_type_structure(pattern, handle, span)?;
                Ok(Outcome::Settled)
            }
            Structure::TypeReference(reference) => {
                self.match_type_structure(*reference, handle, span)?;
                Ok(Outcome::Settled)
            }
            Structure::Name {
                name,
                binder: other,
            } => {
                self.require_lifetime_free_argument(handle, span)?;
                let actual = closed_argument_identity(self.syntax, self.selection, handle, false);
                self.match_name(binder, actual, name, *other, span)
            }
            Structure::RangeShell {
                carrier,
                carrier_binder,
                minimum,
                maximum,
                end_inclusive,
            } => {
                let binder_name = self.binder_name(binder).to_owned();
                let TypeReferenceNode::Constrained {
                    base_type,
                    constraints,
                } = self.syntax.tables.type_references.type_reference(handle)
                else {
                    return Err(self.reject(format!(
                        "cannot match the where equation on `{binder_name}`: the supplied `{binder_name}` argument lacks a declared range shell; supply the omitted arguments explicitly"
                    )).with_source_span(span));
                };
                let constraints = self.syntax.tables.type_references.constraints(*constraints);
                let ranges = constraints
                    .iter()
                    .enumerate()
                    .filter(|(_, constraint)| {
                        matches!(constraint, TypeConstraintNode::Range { .. })
                    })
                    .map(|(ordinal, _)| ordinal)
                    .collect::<Vec<_>>();
                let ordinal = match ranges.as_slice() {
                    [] => {
                        return Err(self.reject(format!(
                            "cannot match the where equation on `{binder_name}`: the supplied `{binder_name}` argument lacks a declared range shell; supply the omitted arguments explicitly"
                        )).with_source_span(span));
                    }
                    [ordinal] => *ordinal,
                    _ => {
                        return Err(self.reject(format!(
                            "cannot match the where equation on `{binder_name}`: the supplied `{binder_name}` argument carries {} range constraints, so its endpoint is ambiguous",
                            ranges.len()
                        )).with_source_span(span));
                    }
                };
                if constraints.len() != 1 {
                    return Err(self.reject(format!(
                        "where equation on `{binder_name}` does not hold: the supplied `{binder_name}` argument carries qualification beyond the equation's range shell"
                    )).with_source_span(span));
                }
                let base_type = *base_type;
                let actual_carrier =
                    closed_argument_identity(self.syntax, self.selection, base_type, false);
                match carrier_binder {
                    Some(index) => match self.bindings[*index].clone() {
                        None => self.bindings[*index] = Some(Binding::Type(base_type)),
                        Some(Binding::Type(bound)) => {
                            let expected =
                                closed_argument_identity(self.syntax, self.selection, bound, false);
                            self.require_same_carrier(
                                &binder_name,
                                carrier,
                                expected,
                                actual_carrier,
                                span,
                            )?;
                        }
                        Some(Binding::NamedType(name)) => {
                            let expected = closed_name_identity(self.syntax, self.selection, &name);
                            self.require_same_carrier(
                                &binder_name,
                                carrier,
                                expected,
                                actual_carrier,
                                span,
                            )?;
                        }
                        Some(Binding::Integer(_) | Binding::Boolean(_) | Binding::Opaque) => {
                            unreachable!("a type binder holds a type binding")
                        }
                    },
                    None => {
                        let expected = closed_name_identity(self.syntax, self.selection, carrier);
                        self.require_same_carrier(
                            &binder_name,
                            carrier,
                            expected,
                            actual_carrier,
                            span,
                        )?;
                    }
                }
                let Some(normalization) = self
                    .syntax
                    .type_references
                    .integer_range_normalization(handle, ordinal)
                else {
                    return Err(self.reject(format!(
                        "cannot match the where equation on `{binder_name}`: the supplied `{binder_name}` argument has no canonical range endpoint; supply the omitted arguments explicitly"
                    )).with_source_span(span));
                };
                let actual_minimum = normalization.minimum.clone();
                let actual_maximum = if *end_inclusive {
                    normalization.maximum.clone()
                } else {
                    normalization.maximum.add(&BigInt::from_u64(1))
                };
                let mut outcome = Outcome::Settled;
                for (endpoint, actual, position) in [
                    (minimum, actual_minimum, "minimum"),
                    (maximum, actual_maximum, "maximum"),
                ] {
                    if let Outcome::Deferred = self.match_endpoint(
                        &binder_name,
                        endpoint,
                        actual,
                        position,
                        span,
                        warnings,
                    )? {
                        outcome = Outcome::Deferred;
                    }
                }
                Ok(outcome)
            }
        }
    }

    fn require_same_carrier(
        &self,
        binder_name: &str,
        carrier: &Identifier,
        expected: Option<ClosedArgumentIdentity>,
        actual: Option<ClosedArgumentIdentity>,
        span: SourceSpan,
    ) -> Result<(), Diagnostic> {
        match (expected, actual) {
            (Some(expected), Some(actual)) if expected == actual => Ok(()),
            (Some(_), Some(_)) => Err(self
                .reject(format!(
                    "where equation on `{binder_name}` does not hold: the supplied range carrier is not `{}`",
                    carrier.as_str()
                ))
                .with_source_span(span)),
            _ => Err(self
                .reject(format!(
                    "where equation on `{binder_name}` cannot be decided: `{}` or the supplied carrier does not name a closed type",
                    carrier.as_str()
                ))
                .with_source_span(span)),
        }
    }

    fn match_name(
        &mut self,
        binder: usize,
        actual: Option<ClosedArgumentIdentity>,
        name: &Identifier,
        other: Option<usize>,
        span: SourceSpan,
    ) -> Result<Outcome, Diagnostic> {
        let binder_name = self.binder_name(binder).to_owned();
        let expected = match other {
            None => closed_name_identity(self.syntax, self.selection, name),
            Some(index) => match self.bindings[index].clone() {
                None => {
                    self.bindings[index] = Some(match self.bindings[binder].clone() {
                        Some(binding) => binding,
                        None => unreachable!("the binder side is bound before matching"),
                    });
                    return Ok(Outcome::Settled);
                }
                Some(Binding::Type(bound)) => {
                    self.require_lifetime_free_argument(bound, span)?;
                    closed_argument_identity(self.syntax, self.selection, bound, false)
                }
                Some(Binding::NamedType(bound)) => {
                    closed_name_identity(self.syntax, self.selection, &bound)
                }
                Some(Binding::Integer(_) | Binding::Boolean(_) | Binding::Opaque) => {
                    unreachable!("a type binder holds a type binding")
                }
            },
        };
        match (expected, actual) {
            (Some(expected), Some(actual)) if expected == actual => Ok(Outcome::Settled),
            (Some(_), Some(_)) => Err(self
                .reject(format!(
                    "where equation on `{binder_name}` does not hold: the supplied `{binder_name}` argument is not `{}`",
                    name.as_str()
                ))
                .with_source_span(span)),
            _ => Err(self
                .reject(format!(
                    "where equation on `{binder_name}` cannot be decided: `{}` or the supplied `{binder_name}` argument does not name a closed type",
                    name.as_str()
                ))
                .with_source_span(span)),
        }
    }

    fn match_endpoint(
        &mut self,
        binder_name: &str,
        endpoint: &Endpoint,
        actual: BigInt,
        position: &str,
        span: SourceSpan,
        warnings: &mut Vec<Diagnostic>,
    ) -> Result<Outcome, Diagnostic> {
        match endpoint {
            Endpoint::Absent => Err(self
                .reject(format!(
                    "where equation on `{binder_name}` has no {position} endpoint to match"
                ))
                .with_source_span(span)),
            Endpoint::Literal(expected) => {
                if *expected == actual {
                    Ok(Outcome::Settled)
                } else {
                    Err(self
                        .reject(format!(
                            "where equation on `{binder_name}` does not hold: its {position} endpoint {expected} differs from the supplied {actual}"
                        ))
                        .with_source_span(span))
                }
            }
            Endpoint::Binder(index) => {
                self.bind_integer(*index, actual, span)?;
                Ok(Outcome::Settled)
            }
            Endpoint::Expression(expression) => {
                let Some(value) = self.expression_endpoint_value(
                    *expression,
                    binder_name,
                    position,
                    span,
                    warnings,
                )?
                else {
                    return Ok(Outcome::Deferred);
                };
                if value == actual {
                    Ok(Outcome::Settled)
                } else {
                    Err(self
                        .reject(format!(
                            "where equation on `{binder_name}` does not hold: its {position} endpoint {value} differs from the supplied {actual}"
                        ))
                        .with_source_span(span))
                }
            }
        }
    }

    fn bind_integer(
        &mut self,
        index: usize,
        value: BigInt,
        span: SourceSpan,
    ) -> Result<(), Diagnostic> {
        let name = self.binder_name(index).to_owned();
        match &self.bindings[index] {
            Some(Binding::Integer(existing)) if *existing == value => Ok(()),
            Some(Binding::Integer(existing)) => Err(self
                .reject(if index < self.supplied_count {
                    format!(
                        "where equation binds `{name}` to {value} but its explicit argument is {existing}"
                    )
                } else {
                    format!("where equations bind `{name}` to both {existing} and {value}")
                })
                .with_source_span(span)),
            Some(Binding::Boolean(_) | Binding::Opaque) => Err(self
                .reject(format!(
                    "where equation mixes type and value kinds: const binder `{name}` holds a non-integer argument"
                ))
                .with_source_span(span)),
            Some(Binding::Type(_) | Binding::NamedType(_)) => {
                unreachable!("classification admits only value binders as endpoints")
            }
            None => {
                let Some(parameter_type) = self.base_info.const_parameter_types[index] else {
                    return Err(self
                        .reject(format!(
                            "where equation cannot recover `{name}` statically: it is a runtime-capable value binder, not a `const` binder"
                        ))
                        .with_source_span(span));
                };
                let TypeReferenceNode::Named(type_name) = self
                    .syntax
                    .tables
                    .type_references
                    .type_reference(parameter_type)
                else {
                    return Err(self
                        .reject(format!(
                            "where equation mixes type and value kinds: const binder `{name}` is not a builtin integer"
                        ))
                        .with_source_span(span));
                };
                let Some((minimum, maximum)) = const_binder_envelope(type_name.as_str()) else {
                    return Err(self
                        .reject(format!(
                            "where equation mixes type and value kinds: const binder `{name}` of type `{}` cannot hold a range endpoint",
                            type_name.as_str()
                        ))
                        .with_source_span(span));
                };
                if value < minimum || value > maximum {
                    return Err(self
                        .reject(format!(
                            "where equation binds `{name}` to {value}, outside its declared `{}`",
                            type_name.as_str()
                        ))
                        .with_source_span(span));
                }
                self.bindings[index] = Some(Binding::Integer(value));
                Ok(())
            }
        }
    }
}

fn integer_to_i128(value: &BigInt) -> Option<i128> {
    value
        .to_i64()
        .map(i128::from)
        .or_else(|| value.to_u64().map(i128::from))
}

/// Only already closed Boolean spelling or the shared canonical value atom.
/// Constructor matching does not evaluate expressions or reinterpret integers.
pub(super) fn normalized_boolean_argument(text: &str) -> Option<bool> {
    match text {
        "true" => Some(true),
        "false" => Some(false),
        _ => {
            let value = CanonicalConstValue::from_atom(text)?;
            if value.type_name != "bool" {
                return None;
            }
            match value.decode_encoding()? {
                DecodedCanonicalConstValue::Boolean(value) => Some(value),
                _ => None,
            }
        }
    }
}

pub(super) fn const_binder_envelope(type_name: &str) -> Option<(BigInt, BigInt)> {
    let (minimum, maximum) = match type_name {
        "i8" => (i128::from(i8::MIN), i128::from(i8::MAX)),
        "i16" => (i128::from(i16::MIN), i128::from(i16::MAX)),
        "i32" => (i128::from(i32::MIN), i128::from(i32::MAX)),
        "i64" => (i128::from(i64::MIN), i128::from(i64::MAX)),
        "u8" => (0, i128::from(u8::MAX)),
        "u16" => (0, i128::from(u16::MAX)),
        "u32" => (0, i128::from(u32::MAX)),
        "u64" | "addr" => (0, i128::from(u64::MAX)),
        _ => return None,
    };
    Some((BigInt::from_i128(minimum), BigInt::from_i128(maximum)))
}

#[cfg(test)]
mod boolean_argument_tests {
    use super::normalized_boolean_argument;
    use language_semantics::const_value::CanonicalConstIdentity;
    use language_semantics::const_value::CanonicalConstValue;

    #[test]
    fn boolean_atom_requires_its_exact_carrier_not_display_text() {
        let mut value = CanonicalConstValue::boolean(true);
        value.display = "false".to_owned();
        assert_eq!(normalized_boolean_argument(&value.atom()), Some(true));
        value.type_name = "u64".to_owned();
        assert_eq!(normalized_boolean_argument(&value.atom()), None);
        let integer = CanonicalConstIdentity::integer("bool", 1);
        let value = CanonicalConstValue::new(integer.type_name, integer.encoding, "1");
        assert_eq!(normalized_boolean_argument(&value.atom()), None);
    }
}
