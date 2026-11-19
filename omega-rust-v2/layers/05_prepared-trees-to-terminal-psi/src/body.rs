use std::num::NonZeroUsize;

use num_bigint::BigInt;
use omega_parser::NodeKind;
use omega_preparation::{PreparedTrees, PreparedValue};
use omega_resolver::{DeclarationId, IndexedSource, Symbol};
use omega_typer::{NodeSemantics, Site, TypedSignature, TypedTree, ValueType};

use crate::{LoweringError, TypeId};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BlockId(NonZeroUsize);

impl BlockId {
    pub fn get(self) -> usize {
        self.0.get()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EdgeId(NonZeroUsize);

impl EdgeId {
    pub fn get(self) -> usize {
        self.0.get()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ValueId(NonZeroUsize);

impl ValueId {
    pub fn get(self) -> usize {
        self.0.get()
    }

    pub fn index(self) -> usize {
        self.get() - 1
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PlaceId(NonZeroUsize);

impl PlaceId {
    pub fn get(self) -> usize {
        self.0.get()
    }

    pub fn index(self) -> usize {
        self.get() - 1
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OperationId(NonZeroUsize);

impl OperationId {
    pub fn get(self) -> usize {
        self.0.get()
    }

    pub fn index(self) -> usize {
        self.get() - 1
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Parameter {
    Scalar(ValueId),
    Structural(PlaceId),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Value {
    pub ty: TypeId,
    pub origin: ValueOrigin,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ValueOrigin {
    Parameter,
    Operation(OperationId),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConstantOperation {
    pub id: OperationId,
    pub result: ValueId,
    pub literal: ScalarLiteral,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ScalarLiteral {
    Integer(BigInt),
    Boolean(bool),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Place {
    pub ty: TypeId,
    pub origin: PlaceOrigin,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlaceOrigin {
    Parameter,
    Result,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReturnEdge {
    Unit,
    Scalar(ValueId),
    Structural { source: PlaceId, result: PlaceId },
}

#[derive(Debug, Eq, PartialEq)]
pub struct EntryBlock {
    pub id: BlockId,
    pub return_edge_id: EdgeId,
    pub operations: Vec<ConstantOperation>,
    pub parameters: Vec<Parameter>,
    pub places: Vec<Place>,
    pub values: Vec<Value>,
    pub return_edge: ReturnEdge,
    pub diagnostics: EntryDiagnostics,
}

#[derive(Debug, Eq, PartialEq)]
pub struct EntryDiagnostics {
    pub body: Site,
    pub parameters: Vec<Site>,
    pub operations: Vec<Site>,
    pub return_edge: Site,
}

pub(crate) fn lower_body(
    prepared: &PreparedTrees<'_>,
    signature: &TypedSignature,
    parameter_types: &[TypeId],
    result_type: Option<TypeId>,
) -> Result<EntryBlock, LoweringError> {
    let tree = prepared
        .typed()
        .trees()
        .nth(signature.node.file)
        .ok_or_else(|| LoweringError::new(&signature.site, "machine signature has no defining tree"))?;
    let resolved = tree.resolved();
    let file = resolved.file;
    let declaration = signature.node.node;
    if resolved.declaration_at(declaration) != Some(&signature.machine) {
        return Err(LoweringError::new(&signature.site, "machine signature disagrees with its defining tree"));
    }

    let body = file
        .child(declaration, NodeKind::Body)
        .ok_or_else(|| LoweringError::new(&signature.site, "machine has no executable body"))?;
    let params = file
        .child(declaration, NodeKind::Params)
        .ok_or_else(|| LoweringError::new(&signature.site, "machine has no parameter list"))?;
    let binders = &file.nodes[params].children;
    if binders.len() != signature.parameters.len() || binders.len() != parameter_types.len() {
        return Err(LoweringError::new(&site_at(file, params), "parameter list disagrees with the typed signature"));
    }

    let body_site = site_at(file, body);
    let mut block = EntryBlock {
        id: BlockId(NonZeroUsize::MIN),
        return_edge_id: EdgeId(NonZeroUsize::MIN),
        operations: Vec::new(),
        parameters: Vec::with_capacity(binders.len()),
        places: Vec::new(),
        values: Vec::new(),
        return_edge: ReturnEdge::Unit,
        diagnostics: EntryDiagnostics {
            body: body_site.clone(),
            parameters: Vec::with_capacity(binders.len()),
            operations: Vec::new(),
            return_edge: body_site,
        },
    };
    for ((&binder, (span, carrier)), &ty) in binders.iter().zip(&signature.parameters).zip(parameter_types) {
        let node = &file.nodes[binder];
        let site = site_at(file, binder);
        if node.binder_name().is_none_or(|name| name.span != *span) || node.kind != NodeKind::Param || node.children.len() != 1 {
            return Err(LoweringError::new(&site, "parameter binder disagrees with the typed signature"));
        }
        let parameter = match carrier {
            ValueType::Array(..) => {
                let place = PlaceId(next_id(block.places.len()));
                block.places.push(Place {
                    ty,
                    origin: PlaceOrigin::Parameter,
                });
                Parameter::Structural(place)
            }
            _ => Parameter::Scalar(block.push_value(ty, ValueOrigin::Parameter)),
        };
        block.parameters.push(parameter);
        block.diagnostics.parameters.push(site);
    }

    // The return is the only executable edge in this slice. Every extra
    // statement or declaration needs lowering of its own.
    check_container(&tree, body)?;
    let statements = file.nodes[body].children.as_slice();
    let tail = match statements {
        [] if result_type.is_none() && signature.result == ValueType::NoResult => return Ok(block),
        [] => return Err(LoweringError::new(&block.diagnostics.body, "machine body has no result")),
        [tail] if file.nodes[*tail].kind == NodeKind::Tail => *tail,
        [tail, next, ..] if file.nodes[*tail].kind == NodeKind::Tail => return Err(unsupported_node(file, *next, "unsupported statement")),
        [first, ..] => return Err(unsupported_node(file, *first, "unsupported statement")),
    };
    check_container(&tree, tail)?;
    let [expression] = *file.nodes[tail].children.as_slice() else {
        return Err(unsupported_node(file, tail, "unsupported return statement"));
    };
    let Some(result_type) = result_type else {
        return Err(unsupported_node(file, expression, "unsupported unit return expression"));
    };
    block.diagnostics.return_edge = site_at(file, expression);
    block.return_edge = lower_return(prepared, &tree, signature, binders, expression, result_type, &mut block)?;
    Ok(block)
}

impl EntryBlock {
    fn push_value(
        &mut self,
        ty: TypeId,
        origin: ValueOrigin,
    ) -> ValueId {
        let value = ValueId(next_id(self.values.len()));
        self.values.push(Value { ty, origin });
        value
    }

    fn push_constant(
        &mut self,
        ty: TypeId,
        literal: ScalarLiteral,
        site: Site,
    ) -> ValueId {
        let id = OperationId(next_id(self.operations.len()));
        let result = self.push_value(ty, ValueOrigin::Operation(id));
        self.operations.push(ConstantOperation { id, result, literal });
        self.diagnostics.operations.push(site);
        result
    }
}

fn next_id(current_count: usize) -> NonZeroUsize {
    NonZeroUsize::new(current_count + 1).expect("vector length leaves room for a one-based identity")
}

fn lower_return(
    prepared: &PreparedTrees<'_>,
    tree: &TypedTree<'_>,
    signature: &TypedSignature,
    binders: &[usize],
    mut expression: usize,
    ty: TypeId,
    block: &mut EntryBlock,
) -> Result<ReturnEdge, LoweringError> {
    let resolved = tree.resolved();
    let file = resolved.file;
    let carrier = &signature.result;
    let mut integer = None;
    loop {
        let node = &file.nodes[expression];
        if !matches!(node.kind, NodeKind::Paren | NodeKind::Literal | NodeKind::PathExpression) {
            return Err(unsupported_node(file, expression, "unsupported return expression"));
        }
        let semantics = return_semantics(tree, expression, carrier)?;
        if let Some(value) = semantics.integer {
            if integer.is_some_and(|previous| previous != value) {
                return Err(LoweringError::new(&site_at(file, expression), "conflicting exact numeric landings"));
            }
            integer = Some(value);
        }

        match (node.kind, node.children.as_slice()) {
            (NodeKind::Paren, [inner]) if semantics.constant.is_none() => expression = *inner,
            (NodeKind::Literal, []) if semantics.constant.is_none() => {
                let Some(value) = integer else {
                    return Err(LoweringError::new(&site_at(file, expression), "literal has no exact integer landing"));
                };
                return Ok(ReturnEdge::Scalar(block.push_constant(
                    ty,
                    ScalarLiteral::Integer(value.clone()),
                    site_at(file, expression),
                )));
            }
            (NodeKind::PathExpression, []) if integer.is_none() => {
                let literal = if let Some(constant) = semantics.constant {
                    if !matches!(resolved.symbol(expression), Some(Symbol::Declaration(id)) if id == constant && id.kind == NodeKind::Const) {
                        return Err(LoweringError::new(
                            &site_at(file, expression),
                            "typed constant disagrees with the resolved return binding",
                        ));
                    }
                    frozen_literal(prepared, carrier, constant, &site_at(file, expression))?
                } else {
                    match resolved.symbol(expression) {
                        Some(Symbol::Builtin(name)) if *carrier == ValueType::Bool && matches!(name.as_str(), "true" | "false") => {
                            ScalarLiteral::Boolean(name == "true")
                        }
                        Some(Symbol::Binding {
                            binder_node,
                            kind: NodeKind::Param,
                            ..
                        }) => {
                            let position = binders
                                .iter()
                                .position(|binder| binder == binder_node)
                                .filter(|&position| &signature.parameters[position].1 == carrier)
                                .ok_or_else(|| LoweringError::new(&site_at(file, expression), "return does not name a compatible parameter of this machine"))?;
                            return Ok(match block.parameters[position] {
                                Parameter::Scalar(value) => ReturnEdge::Scalar(value),
                                Parameter::Structural(source) => {
                                    // An unrestricted array can be returned from its parameter
                                    // place. The caller receives it in the separately declared result.
                                    let result = PlaceId(next_id(block.places.len()));
                                    block.places.push(Place {
                                        ty,
                                        origin: PlaceOrigin::Result,
                                    });
                                    ReturnEdge::Structural { source, result }
                                }
                            });
                        }
                        _ => return Err(unsupported_node(file, expression, "unsupported return binding")),
                    }
                };
                return Ok(ReturnEdge::Scalar(block.push_constant(ty, literal, site_at(file, expression))));
            }
            _ => return Err(unsupported_node(file, expression, "unsupported return expression")),
        }
    }
}

#[derive(Default)]
struct ReturnSemantics<'a> {
    integer: Option<&'a BigInt>,
    constant: Option<&'a DeclarationId>,
}

fn return_semantics<'a>(
    tree: &'a TypedTree<'_>,
    node: usize,
    carrier: &ValueType,
) -> Result<ReturnSemantics<'a>, LoweringError> {
    let file = tree.resolved().file;
    let mut semantic = ReturnSemantics::default();
    let mut typed = false;
    for record in tree.semantics_at(node) {
        match record {
            NodeSemantics::Expression(expression)
                if expression.static_argument_slot.is_none()
                    && (&expression.ty == carrier
                        || (matches!(file.nodes[node].kind, NodeKind::Literal | NodeKind::Paren) && matches!(expression.ty, ValueType::Exact { .. }))) =>
            {
                typed = true;
            }
            NodeSemantics::Landing(landing)
                if &landing.ty == carrier
                    && landing.value.is_integer()
                    && carrier.contains_integer(landing.value.numer())
                    && semantic
                        .integer
                        .is_none_or(|value| value == landing.value.numer()) =>
            {
                semantic.integer = Some(landing.value.numer());
            }
            NodeSemantics::Constant(id) if semantic.constant.is_none_or(|previous| previous == id) => {
                semantic.constant = Some(id);
            }
            NodeSemantics::Obligation(obligation) => {
                return Err(LoweringError::new(
                    &obligation.site,
                    format!("unsupported return obligation: {:?}", obligation.kind),
                ));
            }
            _ => return Err(unsupported_node(file, node, "unsupported typed return record")),
        }
    }
    if !typed {
        return Err(LoweringError::new(&site_at(file, node), "return expression has no compatible typed carrier"));
    }
    Ok(semantic)
}

fn frozen_literal(
    prepared: &PreparedTrees<'_>,
    carrier: &ValueType,
    constant: &DeclarationId,
    site: &Site,
) -> Result<ScalarLiteral, LoweringError> {
    match prepared.constant(constant) {
        Some(PreparedValue::Bool(value)) if *carrier == ValueType::Bool => Ok(ScalarLiteral::Boolean(*value)),
        Some(PreparedValue::Integer(value)) if carrier.contains_integer(value) => Ok(ScalarLiteral::Integer(value.clone())),
        Some(_) => Err(LoweringError::new(site, "frozen constant does not match the supported return carrier")),
        None => Err(LoweringError::new(site, "return constant has no frozen value")),
    }
}

fn check_container(
    tree: &TypedTree<'_>,
    node: usize,
) -> Result<(), LoweringError> {
    if tree.semantics_at(node).next().is_some() {
        return Err(unsupported_node(tree.resolved().file, node, "unsupported typed body record"));
    }
    Ok(())
}

fn unsupported_node(
    file: &IndexedSource<'_>,
    node: usize,
    reason: &str,
) -> LoweringError {
    LoweringError::new(&site_at(file, node), format!("{reason}: {:?}", file.nodes[node].kind))
}

fn site_at(
    file: &IndexedSource<'_>,
    node: usize,
) -> Site {
    Site {
        package: file.package.to_owned(),
        file: file.path.to_owned(),
        span: file.span(node),
    }
}
