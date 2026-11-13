/*
    Fold closed scalar constant initializers before any symbol exists.

    `const LIMIT: u64 = BASE + 1;` names a value that needs no machine to
    compute: its operands are literals and other constants whose values are
    already known, and its operators are the builtin integer ones. Resolution
    canonicalizes such an initializer here, as a syntax-to-syntax rewrite, so
    every later phase reads a literal. An initializer that would have to run a
    machine (a call, a match, a cast, an operator some declaration authors) is
    not this stage's to evaluate and is left exactly as written.

    Three rules keep the fold honest:
    - A name is selected through `ConstantSelection`, from the scope of the
      source that wrote it. Two files that each keep a private `LIMIT` never
      read each other's value, and nothing here consults a name-keyed table.
    - A typed value keeps its type (wiki/spec/language/numeric_values.md): a
      `u32` constant cannot initialize a `u64` one, every typed operation must
      fit its carrier at that node, and only a wholly anonymous calculation
      stays exact until it lands.
    - A fold never invents a value. A cycle, an overflow, a zero divisor or a
      mismatched carrier is a diagnostic at the constant.

    The rewrite leaves the receipt complete resolution already checks for any
    normalized initializer (`constant::initializer_normalization`): the
    authored expression, the value, and the constants and builtin operators
    the value was computed from. Resolution re-derives that inventory from the
    resolved declarations, so a fold that named the wrong dependency fails
    there instead of publishing a value.
*/

use super::anonymous::{
    anonymous_numeric_expression, evaluate_anonymous_integer_argument, has_builtin_const_operator,
};
use super::arguments::{admit_integer_operator, apply_integer_operator, validate_declared_integer};
use super::domains::integer_literal_value;
use super::remainder::validate_anonymous_remainder;
use crate::constant::initializer_normalization::{
    InitializerCustodyPurpose, retained_constant_custody,
};
use crate::preparation::declaration_selection::ConstantSelection;
use diagnostics::Diagnostic;
use language_core::constant_value::EvaluatedConstant;
use language_semantics::const_value::{CanonicalConstIdentity, DecodedCanonicalConstValue};
use numerics::arithmetic::ArithmeticDomain;
use numerics::literals::{IntegerLiteral, IntegerRadix, LandedIntegerType};
use source::{SourceSpan, Span};
use symbols::BuiltinTypeAtom;
use syntax_trees::SyntaxTrees;
use syntax_trees::expression::{ExpressionHandle, ExpressionNode};
use syntax_trees::identifier::{Identifier, JoinedSyntaxPath};
use syntax_trees::item::{
    ConstDefinition, ConstInitializerCustody, ConstInitializerNormalization, Item, ItemHandle,
};
use syntax_trees::types::{ConstArgumentOrigin, TypeReferenceNode};

/* The builtin scalar a foldable constant declares. */
#[derive(Clone, Copy, PartialEq, Eq)]
enum Carrier {
    Integer(LandedIntegerType),
    Boolean,
}

impl Carrier {
    fn name(self) -> &'static str {
        match self {
            Self::Integer(integer) => integer.name(),
            Self::Boolean => "bool",
        }
    }
}

#[derive(Clone, Copy)]
enum Scalar {
    Integer(i128),
    Boolean(bool),
}

/*
    The three inventories a normalization receipt keeps apart: what the
    authored expression names, what preparing the declaration needs, and what
    the materialized value was computed from. A closed scalar has no declared
    type dependency and no call, so the three coincide for a constant folded
    here; a constant from a retained base supplies its own.
*/
#[derive(Clone, Default)]
struct Custodies {
    authored: ConstInitializerCustody,
    declaration: ConstInitializerCustody,
    value: ConstInitializerCustody,
}

impl Custodies {
    fn has_calls(&self) -> bool {
        !self.authored.call_selections.is_empty()
            || !self.declaration.call_selections.is_empty()
            || !self.value.call_selections.is_empty()
    }
}

/* A constant whose value is known, as another initializer may read it. */
#[derive(Clone)]
struct Closed {
    declaration: SourceSpan,
    initializer: SourceSpan,
    evaluated: EvaluatedConstant,
    custodies: Custodies,
}

/*
    Where one constant of this forest stands. `Carried` is a constant this
    stage does not evaluate: it keeps its authored initializer, and so does
    every constant that reads it.
*/
enum Fold {
    Unvisited,
    Active,
    Carried,
    Closed(Closed),
}

struct Dependency {
    reference: SourceSpan,
    name: String,
    closed: Closed,
}

/* A closed initializer's operand names and builtin operator occurrences. */
struct Shape {
    carrier: Carrier,
    references: Vec<(SourceSpan, String)>,
    operators: Vec<SourceSpan>,
}

enum Advance {
    /* This constant of the forest must settle first. */
    Demand(usize),
    Settled(Fold),
    Materialize {
        value: Scalar,
        shape: Shape,
        dependencies: Vec<Dependency>,
    },
}

type SpanKey = (usize, usize, usize);

fn span_key(span: SourceSpan) -> SpanKey {
    (span.source_id.0, span.span.start, span.span.end)
}

fn push_unique<Value: PartialEq>(values: &mut Vec<Value>, value: Value) {
    if !values.contains(&value) {
        values.push(value);
    }
}

fn invalid(syntax: &SyntaxTrees, definition: &ConstDefinition, reason: &str) -> Vec<Diagnostic> {
    vec![
        Diagnostic::error(format!(
            "constant `{}` is invalid: {reason}",
            crate::constant::semantic_const_name(syntax, definition)
        ))
        .with_source_span(definition.name.source_span()),
    ]
}

/*
    Rewrite every closed scalar initializer in the forest to its value.

    Constants settle in dependency order, depth first from each declaration.
    Meeting a constant that is still being settled is a cycle. The walk keeps
    its own stack, so a long alias chain costs heap rather than call depth.
*/
pub(crate) fn fold_closed_scalar_initializers(
    syntax: &mut SyntaxTrees,
    selection: &ConstantSelection,
) -> Result<(), Vec<Diagnostic>> {
    let mut constants = Vec::new();
    let mut demanded = false;
    for item in syntax.root_item_handles() {
        let Item::Const(definition) = syntax.root_item(*item) else {
            continue;
        };
        demanded |= definition.normalization.is_none()
            && crate::constant::requires_const_initializer_evaluation(syntax, definition);
        constants.push((span_key(definition.name.source_span()), *item));
    }
    if !demanded {
        return Ok(());
    }
    constants.sort_unstable_by_key(|(key, _)| *key);
    let mut folds = Vec::new();
    folds.resize_with(constants.len(), || Fold::Unvisited);
    let mut stack = Vec::new();
    for root in 0..constants.len() {
        if !matches!(folds[root], Fold::Unvisited) {
            continue;
        }
        stack.push(root);
        while let Some(&current) = stack.last() {
            folds[current] = Fold::Active;
            let item = constants[current].1;
            match advance(syntax, selection, &constants, &folds, item)? {
                Advance::Demand(dependency) => stack.push(dependency),
                Advance::Settled(fold) => {
                    folds[current] = fold;
                    stack.pop();
                }
                Advance::Materialize {
                    value,
                    shape,
                    dependencies,
                } => {
                    folds[current] = Fold::Closed(materialize(
                        syntax,
                        selection,
                        item,
                        value,
                        &shape,
                        &dependencies,
                    )?);
                    stack.pop();
                }
            }
        }
    }
    Ok(())
}

/* The forest's constant declared at `declaration`, when exactly one is. */
fn forest_constant(constants: &[(SpanKey, ItemHandle)], declaration: SourceSpan) -> Option<usize> {
    let key = span_key(declaration);
    let first = constants.partition_point(|(candidate, _)| *candidate < key);
    let exact = |ordinal: usize| {
        constants
            .get(ordinal)
            .is_some_and(|(candidate, _)| *candidate == key)
    };
    (exact(first) && !exact(first + 1)).then_some(first)
}

fn advance(
    syntax: &SyntaxTrees,
    selection: &ConstantSelection,
    constants: &[(SpanKey, ItemHandle)],
    folds: &[Fold],
    item: ItemHandle,
) -> Result<Advance, Vec<Diagnostic>> {
    let Item::Const(definition) = syntax.root_item(item) else {
        return Ok(Advance::Settled(Fold::Carried));
    };
    let initializer = syntax.expressions.source_span(definition.value);
    if let Some(receipt) = &definition.normalization {
        return Ok(Advance::Settled(Fold::Closed(Closed {
            declaration: definition.name.source_span(),
            initializer,
            evaluated: receipt.result_value.clone(),
            custodies: Custodies {
                authored: receipt.authored_custody.clone(),
                declaration: receipt.declaration_custody.clone(),
                value: ConstInitializerCustody {
                    selections: receipt.selections.clone(),
                    builtin_operators: receipt.builtin_operators.clone(),
                    call_selections: receipt.call_selections.clone(),
                },
            },
        })));
    }
    if definition.pending_target_observation {
        return Ok(Advance::Settled(Fold::Carried));
    }
    if !crate::constant::requires_const_initializer_evaluation(syntax, definition) {
        /*
            A literal is already its value. One that does not fit its declared
            type has no value to lend; its own declaration check reports it.
        */
        return Ok(Advance::Settled(
            match crate::constant::evaluated_declaration_value(syntax, definition, Some(selection))
            {
                Ok(evaluated) => Fold::Closed(Closed {
                    declaration: definition.name.source_span(),
                    initializer,
                    evaluated,
                    custodies: Custodies::default(),
                }),
                Err(_) => Fold::Carried,
            },
        ));
    }
    let Some(shape) = closed_shape(syntax, selection, definition) else {
        return Ok(Advance::Settled(Fold::Carried));
    };
    let mut dependencies = Vec::with_capacity(shape.references.len());
    for (reference, name) in &shape.references {
        let header = selection
            .const_header(syntax, &Identifier::new(name.clone(), *reference))
            .map_err(|diagnostic| vec![diagnostic])?;
        let Some(header) = header else {
            return Ok(Advance::Settled(Fold::Carried));
        };
        let (closed, is_public) = if let Some(ordinal) = forest_constant(constants, header) {
            let Item::Const(selected) = syntax.root_item(constants[ordinal].1) else {
                return Ok(Advance::Settled(Fold::Carried));
            };
            match &folds[ordinal] {
                Fold::Unvisited => return Ok(Advance::Demand(ordinal)),
                Fold::Active => {
                    return Err(invalid(
                        syntax,
                        definition,
                        &format!("its initializer depends on itself through `{name}`"),
                    ));
                }
                Fold::Carried => return Ok(Advance::Settled(Fold::Carried)),
                Fold::Closed(closed) => (closed.clone(), selected.is_public),
            }
        } else {
            let Some(base) = selection.retained_base() else {
                return Ok(Advance::Settled(Fold::Carried));
            };
            let Some(retained) = base.const_declaration_at_source_span(header) else {
                return Ok(Advance::Settled(Fold::Carried));
            };
            let Some(evaluated) = retained.evaluated_value.clone() else {
                return Ok(Advance::Settled(Fold::Carried));
            };
            let custody = |purpose| {
                retained_constant_custody(base, retained, purpose)
                    .map_err(|diagnostic| vec![diagnostic])
            };
            (
                Closed {
                    declaration: header,
                    initializer: retained.initializer_source_span,
                    evaluated,
                    custodies: Custodies {
                        authored: custody(InitializerCustodyPurpose::Authored)?,
                        declaration: custody(InitializerCustodyPurpose::Declaration)?,
                        value: custody(InitializerCustodyPurpose::Value)?,
                    },
                },
                retained.is_public,
            )
        };
        if !is_public && !selection.same_checked_package(*reference, closed.declaration) {
            return Err(vec![
                Diagnostic::error(format!(
                    "constant `{name}` selects a private declaration in another package"
                ))
                .with_source_span(*reference),
            ]);
        }
        /*
            A value some machine computed carries that call in its custody.
            Reading it is still not this stage's fold: the reader stays as
            written beside the constant it names.
        */
        if closed.custodies.has_calls() {
            return Ok(Advance::Settled(Fold::Carried));
        }
        dependencies.push(Dependency {
            reference: *reference,
            name: name.clone(),
            closed,
        });
    }
    let mut warnings = Vec::new();
    let value = evaluate(syntax, definition, &shape, &dependencies, &mut warnings)
        .map_err(|reason| invalid(syntax, definition, &reason))?;
    if !warnings.is_empty() {
        return Err(warnings);
    }
    Ok(Advance::Materialize {
        value,
        shape,
        dependencies,
    })
}

/* The source range and spelling of one authored name path. */
fn name_reference(
    syntax: &SyntaxTrees,
    expression: ExpressionHandle,
) -> Option<(SourceSpan, String)> {
    let ExpressionNode::Name {
        members,
        application,
    } = syntax.expressions.expression(expression)
    else {
        return None;
    };
    if !application.is_none() {
        return None;
    }
    let members = syntax
        .expressions
        .identifier_path_members(members.to_owned());
    let (first, last) = (members.first()?, members.last()?);
    Some((
        SourceSpan::new(
            first.source_span().source_id,
            Span::new(first.source_span().span.start, last.source_span().span.end),
        ),
        members.joined_path_in(syntax),
    ))
}

/*
    Whether an initializer is one this stage folds: a plain builtin integer or
    Boolean destination, and an expression built only from integer literals,
    names, and Exact integer operators no declaration authors. Anything else
    asks for a judgment resolution does not own, so there is no shape.
*/
fn closed_shape(
    syntax: &SyntaxTrees,
    selection: &ConstantSelection,
    definition: &ConstDefinition,
) -> Option<Shape> {
    if definition.qualification.is_some() || definition.pending_qualification.is_some() {
        return None;
    }
    let TypeReferenceNode::Named(type_name) = syntax
        .type_references
        .type_reference(definition.type_reference)
    else {
        return None;
    };
    let carrier = match selection.builtin_type(syntax, type_name)? {
        BuiltinTypeAtom::I8 => Carrier::Integer(LandedIntegerType::I8),
        BuiltinTypeAtom::I16 => Carrier::Integer(LandedIntegerType::I16),
        BuiltinTypeAtom::I32 => Carrier::Integer(LandedIntegerType::I32),
        BuiltinTypeAtom::I64 => Carrier::Integer(LandedIntegerType::I64),
        BuiltinTypeAtom::U8 => Carrier::Integer(LandedIntegerType::U8),
        BuiltinTypeAtom::U16 => Carrier::Integer(LandedIntegerType::U16),
        BuiltinTypeAtom::U32 => Carrier::Integer(LandedIntegerType::U32),
        BuiltinTypeAtom::U64 => Carrier::Integer(LandedIntegerType::U64),
        BuiltinTypeAtom::Bool => Carrier::Boolean,
        _ => return None,
    };
    let mut shape = Shape {
        carrier,
        references: Vec::new(),
        operators: Vec::new(),
    };
    let mut pending = vec![definition.value];
    while let Some(expression) = pending.pop() {
        match syntax.expressions.expression(expression) {
            ExpressionNode::Integer(_) if matches!(carrier, Carrier::Integer(_)) => {}
            ExpressionNode::Name { .. } => {
                push_unique(&mut shape.references, name_reference(syntax, expression)?);
            }
            ExpressionNode::Binary(binary)
                if matches!(carrier, Carrier::Integer(_))
                    && binary.policy == ArithmeticDomain::Exact
                    && admit_integer_operator(binary.operator, None).is_ok()
                    && has_builtin_const_operator(syntax, binary) =>
            {
                shape
                    .operators
                    .push(syntax.expressions.source_span(expression));
                pending.push(binary.right);
                pending.push(binary.left);
            }
            _ => return None,
        }
    }
    Some(shape)
}

/* The scalar a completed constant value denotes, with its carrier. */
fn scalar(evaluated: &EvaluatedConstant) -> Option<(Carrier, Scalar)> {
    let identity = CanonicalConstIdentity {
        type_name: String::new(),
        encoding: evaluated.encoding()?.to_owned(),
    };
    match identity.decode_declaration_encoding()? {
        DecodedCanonicalConstValue::Integer { type_name, value } => Some((
            Carrier::Integer(LandedIntegerType::from_name(&type_name)?),
            Scalar::Integer(value),
        )),
        DecodedCanonicalConstValue::Boolean(value) => {
            Some((Carrier::Boolean, Scalar::Boolean(value)))
        }
        _ => None,
    }
}

/* One selected constant read at the destination's carrier. */
fn operand(
    dependencies: &[Dependency],
    reference: SourceSpan,
    carrier: Carrier,
) -> Result<Scalar, String> {
    let dependency = dependencies
        .iter()
        .find(|dependency| dependency.reference == reference)
        .ok_or("an operand lost its selected constant")?;
    let name = &dependency.name;
    let (actual, value) = scalar(&dependency.closed.evaluated).ok_or_else(|| {
        format!(
            "`{name}` is not a `{}` value; a constant keeps the type it was declared with",
            carrier.name()
        )
    })?;
    if actual != carrier {
        return Err(format!(
            "`{name}` is a `{}` constant and this constant declares `{}`; a typed value keeps \
             its type, so convert it explicitly",
            actual.name(),
            carrier.name()
        ));
    }
    Ok(value)
}

/*
    Compute a closed initializer. Typed operands compute in the destination's
    carrier and each result must fit it; a wholly anonymous subtree stays an
    exact rational until the typed operation or the destination lands it.
*/
fn evaluate(
    syntax: &SyntaxTrees,
    definition: &ConstDefinition,
    shape: &Shape,
    dependencies: &[Dependency],
    warnings: &mut Vec<Diagnostic>,
) -> Result<Scalar, String> {
    let integer = match shape.carrier {
        Carrier::Integer(integer) => integer,
        Carrier::Boolean => {
            let (reference, _) = name_reference(syntax, definition.value)
                .ok_or("a Boolean constant folds only another Boolean constant")?;
            return operand(dependencies, reference, shape.carrier);
        }
    };
    enum Step {
        Enter(ExpressionHandle),
        Apply(ExpressionHandle),
    }
    #[derive(Clone, Copy)]
    enum Operand {
        Anonymous(ExpressionHandle),
        Landed(i128),
    }
    let mut land = |operand: Operand| -> Result<i128, String> {
        let value = match operand {
            Operand::Landed(value) => return Ok(value),
            Operand::Anonymous(expression) => {
                evaluate_anonymous_integer_argument(syntax, expression, warnings)?
                    .ok_or("an anonymous operand lost its exact value")?
            }
        };
        validate_declared_integer(integer, value)?;
        Ok(value)
    };
    let mut pending = vec![Step::Enter(definition.value)];
    let mut operands = Vec::new();
    while let Some(step) = pending.pop() {
        match step {
            Step::Enter(expression) => match syntax.expressions.expression(expression) {
                ExpressionNode::Integer(literal) => match literal.landing() {
                    None => operands.push(Operand::Anonymous(expression)),
                    Some(landing) => {
                        if landing.landed_type != integer
                            || landing.domain != ArithmeticDomain::Exact
                        {
                            return Err(format!(
                                "an integer literal lands as `{}` and this constant declares \
                                 `{}`",
                                landing.landed_type.name(),
                                integer.name()
                            ));
                        }
                        let value = integer_literal_value(literal)
                            .ok_or("an integer literal exceeds the const-value envelope")?;
                        validate_declared_integer(integer, value)?;
                        operands.push(Operand::Landed(value));
                    }
                },
                ExpressionNode::Name { .. } => {
                    let (reference, _) = name_reference(syntax, expression)
                        .ok_or("an operand lost its authored name")?;
                    let Scalar::Integer(value) = operand(dependencies, reference, shape.carrier)?
                    else {
                        return Err("an integer operand selected a Boolean value".to_owned());
                    };
                    operands.push(Operand::Landed(value));
                }
                ExpressionNode::Binary(binary) => {
                    pending.push(Step::Apply(expression));
                    pending.push(Step::Enter(binary.right));
                    pending.push(Step::Enter(binary.left));
                }
                _ => return Err("the initializer changed while it was folded".to_owned()),
            },
            Step::Apply(expression) => {
                let ExpressionNode::Binary(binary) = syntax.expressions.expression(expression)
                else {
                    return Err("the initializer changed while it was folded".to_owned());
                };
                let (Some(right), Some(left)) = (operands.pop(), operands.pop()) else {
                    return Err("an operator lost an operand".to_owned());
                };
                validate_anonymous_remainder(syntax, binary)?;
                if anonymous_numeric_expression(syntax, expression) {
                    operands.push(Operand::Anonymous(expression));
                    continue;
                }
                let (left, right) = (land(left)?, land(right)?);
                admit_integer_operator(binary.operator, Some(right))?;
                let value = apply_integer_operator(binary.operator, left, right, Some(integer))?;
                validate_declared_integer(integer, value)?;
                operands.push(Operand::Landed(value));
            }
        }
    }
    let (Some(value), None) = (operands.pop(), operands.pop()) else {
        return Err("the initializer has no single value".to_owned());
    };
    land(value).map(Scalar::Integer)
}

/* One of a folded constant's three inventories, closed over its operands. */
fn custody(
    shape: &Shape,
    dependencies: &[Dependency],
    inventory: impl Fn(&Custodies) -> &ConstInitializerCustody,
) -> ConstInitializerCustody {
    let mut custody = ConstInitializerCustody {
        builtin_operators: shape.operators.clone(),
        ..Default::default()
    };
    for dependency in dependencies {
        push_unique(
            &mut custody.selections,
            ConstArgumentOrigin {
                reference: dependency.reference,
                declaration: dependency.closed.declaration,
                initializer: dependency.closed.initializer,
                evaluated_value: dependency.closed.evaluated.clone(),
            },
        );
        let inherited = inventory(&dependency.closed.custodies);
        for origin in &inherited.selections {
            push_unique(&mut custody.selections, origin.clone());
        }
        for operator in &inherited.builtin_operators {
            push_unique(&mut custody.builtin_operators, *operator);
        }
    }
    custody
}

/*
    Replace the initializer with its value and leave the receipt. The literal
    takes the authored expression's source span, and the authored expression
    stays in the forest under the receipt for resolution to read back.
*/
fn materialize(
    syntax: &mut SyntaxTrees,
    selection: &ConstantSelection,
    item: ItemHandle,
    value: Scalar,
    shape: &Shape,
    dependencies: &[Dependency],
) -> Result<Closed, Vec<Diagnostic>> {
    let Item::Const(definition) = syntax.root_item(item) else {
        return Err(vec![Diagnostic::error(
            "a folded constant lost its declaration",
        )]);
    };
    let mut folded = definition.clone();
    let authored = folded.value;
    let initializer = syntax.expressions.source_span(authored);
    let literal = match value {
        Scalar::Boolean(value) => ExpressionNode::Boolean(value),
        Scalar::Integer(value) => {
            let digits = value.unsigned_abs().to_string();
            ExpressionNode::Integer(
                IntegerLiteral::from_parts(value < 0, IntegerRadix::Decimal, &digits)
                    .map_err(|reason| invalid(syntax, &folded, reason))?,
            )
        }
    };
    folded.value = syntax.expressions.insert(literal);
    syntax
        .expressions
        .set_source_span(folded.value, initializer);
    let evaluated = crate::constant::evaluated_declaration_value(syntax, &folded, Some(selection))
        .map_err(|reason| invalid(syntax, &folded, &reason))?;
    let custodies = Custodies {
        authored: custody(shape, dependencies, |custodies| &custodies.authored),
        declaration: custody(shape, dependencies, |custodies| &custodies.declaration),
        value: custody(shape, dependencies, |custodies| &custodies.value),
    };
    folded.normalization = Some(ConstInitializerNormalization {
        authored_custody: custodies.authored.clone(),
        declaration_custody: custodies.declaration.clone(),
        authored_expression: authored,
        result_value: evaluated.clone(),
        selections: custodies.value.selections.clone(),
        builtin_operators: custodies.value.builtin_operators.clone(),
        call_selections: Vec::new(),
    });
    let declaration = folded.name.source_span();
    syntax.items.replace_item(item, Item::Const(folded));
    Ok(Closed {
        declaration,
        initializer,
        evaluated,
        custodies,
    })
}

#[cfg(test)]
mod tests {
    /*
        Each test resolves a whole forest and reads the declaration resolution
        published, so the fold is judged together with the receipt check that
        complete resolution runs on it. Every accepted program has a rejected
        twin that differs in the one thing the fold must refuse.
    */

    use crate::resolution::{ResolutionRequest, resolve};
    use language_core::constant_value::EvaluatedConstant;
    use language_semantics::const_value::CanonicalConstIdentity;
    use std::sync::Arc;
    use symbol_resolved_trees::SymbolResolvedTrees;

    /* Resolve files that each belong to the package their directory names. */
    fn resolve_files(
        units: &[(&str, &str)],
        base: Option<(Arc<SymbolResolvedTrees>, source::SourceMap)>,
    ) -> Result<(SymbolResolvedTrees, source::SourceMap), Vec<diagnostics::Diagnostic>> {
        let (retained_base, mut sources) = match base {
            Some((base, sources)) => (Some(base), sources),
            None => (None, source::SourceMap::default()),
        };
        let mut syntax = syntax_trees::SyntaxTrees::default();
        for (path, text) in units {
            let path = std::path::PathBuf::from(path);
            let root = path.parent().unwrap().to_path_buf();
            let source = sources
                .add_checked_instance(
                    path,
                    *text,
                    root,
                    None,
                    source::SourceOrigin::User,
                    source::SourceResolutionStratum::Base,
                    source::DependencyScope::Product,
                )
                .source_id;
            let tokens = source_files_to_tokens::Lexer::new(text).tokenize().unwrap();
            let unit = tokens_to_syntax_trees::parse_syntax_trees_with_id(source, &tokens).unwrap();
            syntax.extend_from(&unit);
        }
        resolve(ResolutionRequest {
            syntax,
            sources: Some(Arc::new(sources.clone())),
            top_level_bindings: Vec::new(),
            retained_base,
        })
        .map(|program| (program, sources))
    }

    fn resolve_one(text: &str) -> Result<SymbolResolvedTrees, Vec<diagnostics::Diagnostic>> {
        resolve_files(&[("app/main.omg", text)], None).map(|(program, _)| program)
    }

    /* The value resolution published for the one constant named `name`. */
    fn value(program: &SymbolResolvedTrees, name: &str) -> EvaluatedConstant {
        let mut declarations = program
            .const_declarations
            .iter()
            .filter(|declaration| program.symbols.name(declaration.symbol) == name);
        let declaration = declarations.next().expect("declared constant");
        assert!(declarations.next().is_none(), "one constant named {name}");
        declaration
            .evaluated_value
            .clone()
            .expect("the constant has a value")
    }

    fn integer(type_name: &str, value: i128) -> EvaluatedConstant {
        EvaluatedConstant::Encoded(CanonicalConstIdentity::integer(type_name, value).encoding)
    }

    /* The single refusal, which must point at the constant named `name`. */
    fn refusal(text: &str, name: &str) -> String {
        let diagnostics = resolve_one(text).expect_err("the program is refused");
        let [diagnostic] = diagnostics.as_slice() else {
            panic!("one refusal, found {diagnostics:?}");
        };
        let span = diagnostic
            .source_span
            .expect("the refusal names its source");
        assert_eq!(&text[span.span.start..span.span.end], name);
        diagnostic.message.clone()
    }

    #[test]
    fn an_alias_chain_takes_the_value_at_its_end() {
        let program = resolve_one(
            "const FIRST: u64 = 0x8000000000000002;\n\
             public const SECOND: u64 = FIRST;\n\
             public const THIRD: u64 = SECOND;\n\
             const ENABLED: bool = true;\n\
             public const ALSO_ENABLED: bool = ENABLED;",
        )
        .unwrap();
        let expected = integer("u64", 0x8000_0000_0000_0002);
        assert_eq!(value(&program, "SECOND"), expected);
        assert_eq!(value(&program, "THIRD"), expected);
        assert_eq!(
            value(&program, "ALSO_ENABLED"),
            value(&program, "ENABLED"),
            "a Boolean alias reads the Boolean it names"
        );
    }

    #[test]
    fn an_alias_of_another_carrier_is_refused_at_the_constant() {
        let message = refusal(
            "const NARROW: u32 = 7;\npublic const WIDE: u64 = NARROW;",
            "WIDE",
        );
        assert!(
            message.contains("`NARROW` is a `u32` constant and this constant declares `u64`"),
            "{message}"
        );
    }

    #[test]
    fn closed_arithmetic_folds_over_literals_and_constants() {
        let program = resolve_one(
            "const BASE: u64 = 6;\n\
             public const SCALED: u64 = BASE * 7 - 2;\n\
             public const SHIFTED: u64 = SCALED << 2;\n\
             public const ANONYMOUS: u32 = (5 + 7) / 4;\n\
             public const SIGNED: i8 = 0i8 - 127i8 - 1i8;",
        )
        .unwrap();
        assert_eq!(value(&program, "SCALED"), integer("u64", 40));
        assert_eq!(value(&program, "SHIFTED"), integer("u64", 160));
        assert_eq!(value(&program, "ANONYMOUS"), integer("u32", 3));
        assert_eq!(value(&program, "SIGNED"), integer("i8", -128));
    }

    #[test]
    fn an_overflow_is_refused_at_the_constant() {
        let message = refusal(
            "const NEAR: u8 = 200;\npublic const OVER: u8 = NEAR + 100;",
            "OVER",
        );
        assert!(
            message.contains("integer value `300` is outside the declared `u8` range"),
            "{message}"
        );
        /*
            The final value fits, but the first typed operation does not: a
            safe result cannot excuse an intermediate outside its carrier.
        */
        let message = refusal(
            "const NEAR: u8 = 200;\npublic const BACK: u8 = NEAR + 100 - 100;",
            "BACK",
        );
        assert!(
            message.contains("integer value `300` is outside the declared `u8` range"),
            "{message}"
        );
        let message = refusal(
            "const NONE: u64 = 0;\npublic const QUOTIENT: u64 = 9 / NONE;",
            "QUOTIENT",
        );
        assert!(message.contains("division by zero is invalid"), "{message}");
    }

    #[test]
    fn a_cycle_is_refused_and_no_value_is_invented() {
        let message = refusal(
            "const FIRST: u64 = SECOND;\nconst SECOND: u64 = THIRD + 1;\nconst THIRD: u64 = FIRST;",
            "THIRD",
        );
        assert!(
            message.contains("its initializer depends on itself through `FIRST`"),
            "{message}"
        );
        let message = refusal("const ALONE: u64 = ALONE;", "ALONE");
        assert!(message.contains("depends on itself"), "{message}");
    }

    #[test]
    fn same_named_private_constants_in_two_files_do_not_collide() {
        let first = "const LIMIT: u64 = 3;\npublic const FIRST_LIMIT: u64 = LIMIT + 1;";
        let second = "const LIMIT: u64 = 9;\npublic const SECOND_LIMIT: u64 = LIMIT + 1;";
        let (program, _) =
            resolve_files(&[("first/a.omg", first), ("second/b.omg", second)], None).unwrap();
        assert_eq!(value(&program, "FIRST_LIMIT"), integer("u64", 4));
        assert_eq!(value(&program, "SECOND_LIMIT"), integer("u64", 10));
        /*
            Neither private constant is visible from another package, so a
            file without its own `LIMIT` reads no value at all.
        */
        let stranger = "public const STRANGER: u64 = LIMIT + 1;";
        assert!(resolve_files(&[("first/a.omg", first), ("third/c.omg", stranger)], None).is_err());
    }

    #[test]
    fn a_constant_folds_over_a_retained_base_constant() {
        let base_text = "const SEED: u64 = 5;\npublic const BASE: u64 = SEED * 2;";
        let (base, sources) = resolve_files(&[("library/base.omg", base_text)], None).unwrap();
        assert_eq!(value(&base, "BASE"), integer("u64", 10));
        let (program, _) = resolve_files(
            &[("library/next.omg", "public const NEXT: u64 = BASE + 1;")],
            Some((Arc::new(base), sources)),
        )
        .unwrap();
        assert_eq!(value(&program, "NEXT"), integer("u64", 11));
    }

    #[test]
    fn an_initializer_that_needs_a_machine_is_not_folded() {
        /*
            A call is not a closed scalar. Resolution has no carrier for an
            unevaluated constant yet, so the declaration is still refused,
            and it is refused as written rather than given a value.
        */
        let diagnostics =
            resolve_one("machine seven() -> u64 { 7u64 }\npublic const COMPUTED: u64 = seven();")
                .expect_err("a machine-call initializer has no route yet");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("expected an integer literal")),
            "{diagnostics:?}"
        );
    }
}
