//! Closed integer indices can use pure Omega definitions. Compile the admitted
//! first-order integer operations once, keeping the selected declaration on
//! every open application. Evaluating a closed application uses that program.
use crate::declarations::resolved_source_file::ResolvedSourceFile;
use crate::evaluation::{integer_arithmetic, numeric_literals};
use crate::type_system::type_catalog::TypeCatalog;
use crate::{IntegerIndex, TypeError, ValueType};
use omega_parser::NodeKind;
use omega_resolver::{DeclarationId, Symbol};
use std::{
    collections::{HashMap, HashSet, VecDeque},
    sync::Arc,
};

use super::integer_routine::IntegerRoutine;

#[derive(Default)]
pub(crate) struct IntegerProgram {
    routines: HashMap<DeclarationId, Arc<IntegerRoutine>>,
}
#[derive(Clone)]
pub(crate) struct IntegerOperand {
    pub ty: ValueType,
    pub term: Option<IntegerIndex>,
}
impl IntegerOperand {
    fn typed(
        ty: ValueType,
        term: IntegerIndex,
    ) -> Self {
        Self { ty, term: Some(term) }
    }
    fn land(
        &self,
        ty: &ValueType,
    ) -> Result<IntegerIndex, String> {
        if &self.ty == ty
            && let Some(t) = &self.term
        {
            return Ok(t.clone());
        }
        if let ValueType::Exact {
            value,
            fractional_origin: None,
        } = &self.ty
            && value.is_integer()
            && integer_arithmetic::fits(value.numer(), ty)
        {
            return Ok(IntegerIndex::Value(value.to_integer()));
        }
        Err(format!("static operand {} does not land in {ty}", self.ty))
    }
}
impl IntegerProgram {
    pub(crate) fn build(
        units: &[ResolvedSourceFile<'_>],
        evaluated: &[crate::EvaluatedIntegerCall],
    ) -> Result<Self, TypeError> {
        let mut program = Self::default();
        let mut pending = Vec::new();
        for unit in units {
            let f = unit.file;
            for (n, node) in f
                .nodes
                .iter()
                .enumerate()
                .filter(|(_, n)| matches!(n.kind, NodeKind::Machine | NodeKind::Const))
            {
                let Ok(id) = unit.declaration(n) else { continue };
                let integer = |i| match unit.symbol(i) {
                    Some(Symbol::Builtin(name)) if f.nodes[i].children.is_empty() => ValueType::integer(name),
                    _ => None,
                };
                let (params, result, body) =
                    if node.kind == NodeKind::Const {
                        let [_, ty, expr] = *node.children.as_slice() else { continue };
                        let Some(result) = integer(ty) else { continue };
                        (vec![], result, expr)
                    } else {
                        // Explicit effect/proof clauses need an admitted evaluation
                        // frame. Only a completely local integer body can run here.
                        let executable = !node.children.iter().any(|&c| {
                        if matches!(f.nodes[c].kind, NodeKind::Reaches | NodeKind::Crashes) {
                            return !matches!(f.nodes[c].children.as_slice(), [arg] if matches!(unit.symbol(*arg), Some(Symbol::Builtin(name)) if name=="none"));
                        }
                        !matches!(
                            f.nodes[c].kind,
                            NodeKind::Path | NodeKind::Params | NodeKind::Result | NodeKind::Body | NodeKind::Terminates
                        )
                    }) && !f.child(n, NodeKind::Terminates).is_some_and(|t| !f.nodes[t].children.is_empty());
                        if f.child(n, NodeKind::GenericParams).is_some() {
                            continue;
                        }
                        let Some(params) = f.child(n, NodeKind::Params).and_then(|s| {
                            f.nodes[s]
                                .children
                                .iter()
                                .map(|&p| Some((f.nodes[p].words().next()?.span, integer(unit.single_child(p).ok()?)?)))
                                .collect::<Option<Vec<_>>>()
                        }) else {
                            continue;
                        };
                        let Some(result) = f
                            .child(n, NodeKind::Result)
                            .and_then(|r| integer(unit.single_child(r).ok()?))
                        else {
                            continue;
                        };
                        program.routines.insert(
                            id.clone(),
                            Arc::new(IntegerRoutine {
                                declaration: id.clone(),
                                parameters: params.iter().map(|(_, ty)| ty.clone()).collect(),
                                result: result.clone(),
                                body: None,
                                values: HashMap::new(),
                                intermediate: vec![],
                            }),
                        );
                        let Some(body) = f.child(n, NodeKind::Body).filter(|_| executable) else {
                            continue;
                        };
                        (params, result, body)
                    };
                pending.push((unit, id, params, result, body));
            }
        }
        for receipt in evaluated {
            let routine = program
                .routines
                .get_mut(&receipt.machine)
                .and_then(Arc::get_mut);
            let error = |message: &str| TypeError {
                package: receipt.machine.package.clone(),
                file: receipt.machine.path.clone(),
                span: omega_lexer::SourceSpan::new(0, 0),
                message: message.into(),
            };
            let routine = routine.ok_or_else(|| error("evaluation result has no matching integer signature"))?;
            if receipt.inputs.len() != routine.parameters.len()
                || !receipt
                    .inputs
                    .iter()
                    .zip(&routine.parameters)
                    .all(|(n, ty)| integer_arithmetic::fits(n, ty))
                || !integer_arithmetic::fits(&receipt.result, &routine.result)
            {
                return Err(error("evaluation result is outside its declared integer signature"));
            }
            let value = IntegerIndex::Value(receipt.result.clone());
            if routine
                .values
                .insert(receipt.inputs.clone(), value.clone())
                .is_some_and(|prior| prior != value)
            {
                return Err(error("conflicting evaluation results for a closed static call"));
            }
        }
        // A definition only becomes executable after every declaration in its
        // body is available. Cycles cannot acquire a termination receipt here.
        let ids: HashMap<_, _> = pending
            .iter()
            .enumerate()
            .map(|(i, (_, id, ..))| (*id, i))
            .collect();
        let mut users = vec![Vec::new(); pending.len()];
        let mut missing = vec![0usize; pending.len()];
        let mut ready = VecDeque::new();
        for (index, (unit, _, _, _, body)) in pending.iter().enumerate() {
            let mut dependencies = HashSet::new();
            for part in *body..unit.file.nodes[*body].end {
                match unit.symbol(part) {
                    Some(Symbol::Declaration(id)) => {
                        if let Some(dep) = ids.get(id) {
                            dependencies.insert(*dep);
                        }
                    }
                    Some(Symbol::Candidates(options)) => dependencies.extend(options.iter().filter_map(|id| ids.get(id).copied())),
                    _ => {}
                }
            }
            missing[index] = dependencies.len();
            if dependencies.is_empty() {
                ready.push_back(index);
            }
            for dep in dependencies {
                users[dep].push(index);
            }
        }
        let mut depth = vec![0usize; pending.len()];
        while let Some(index) = ready.pop_front() {
            let (unit, id, params, result, body) = &pending[index];
            if depth[index] > 96 {
                continue;
            }
            let locals = params
                .iter()
                .enumerate()
                .map(|(index, (span, ty))| {
                    (
                        *span,
                        IntegerOperand::typed(
                            ty.clone(),
                            IntegerIndex::Binder {
                                owner: Box::new((*id).clone()),
                                index,
                            },
                        ),
                    )
                })
                .collect();
            let mut intermediate = Vec::new();
            let Ok(value) = program.compile(unit, *body, &locals, None, &mut intermediate, 0) else {
                continue;
            };
            let Ok(term) = value.land(result) else { continue };
            program.routines.insert(
                (*id).clone(),
                Arc::new(IntegerRoutine {
                    declaration: (*id).clone(),
                    parameters: params.iter().map(|(_, t)| t.clone()).collect(),
                    result: result.clone(),
                    body: Some(term),
                    intermediate,
                    values: HashMap::new(),
                }),
            );
            for &user in &users[index] {
                depth[user] = depth[user].max(depth[index] + 1);
                missing[user] -= 1;
                if missing[user] == 0 {
                    ready.push_back(user);
                }
            }
        }
        Ok(program)
    }
    pub(crate) fn expression(
        &self,
        unit: &ResolvedSourceFile<'_>,
        n: usize,
        types: &TypeCatalog<'_>,
        carrier: &ValueType,
    ) -> Result<IntegerIndex, TypeError> {
        let mut intermediate = Vec::new();
        let term = self
            .compile(unit, n, &HashMap::new(), Some(types), &mut intermediate, 0)
            .and_then(|v| v.land(carrier))
            .map_err(|m| TypeError::at(unit.file, n, m))?;
        if let Some(owner) = unit
            .file
            .enclosing_contexts(n)
            .find_map(|a| unit.declaration_at(a))
        {
            let mut all = types.formations.borrow_mut();
            let checks = all.entry((*owner).clone()).or_default();
            for (node, operation, ty) in intermediate {
                let value = operation.typed(ty);
                value
                    .validate_closed_indices()
                    .map_err(|message| TypeError::at(unit.file, node, message))?;
                checks.insert(crate::Site::at(unit.file, node), value);
            }
        }
        Ok(term)
    }
    fn compile(
        &self,
        unit: &ResolvedSourceFile<'_>,
        n: usize,
        locals: &HashMap<omega_lexer::SourceSpan, IntegerOperand>,
        types: Option<&TypeCatalog<'_>>,
        intermediate: &mut Vec<(usize, IntegerIndex, ValueType)>,
        depth: usize,
    ) -> Result<IntegerOperand, String> {
        use NodeKind::*;
        let f = unit.file;
        let node = &f.nodes[n];
        if depth >= 192 {
            return Err("static expression exceeds the preparation nesting budget".into());
        }
        let child = |i, intermediate: &mut Vec<(usize, IntegerIndex, ValueType)>| self.compile(unit, i, locals, types, intermediate, depth + 1);
        let unresolved = || "static expression requires an admitted evaluator for this declaration".to_string();
        match node.kind {
            Body => {
                let (tail, statements) = node.children.split_last().ok_or_else(unresolved)?;
                if f.nodes[*tail].kind != Tail {
                    return Err(unresolved());
                }
                let mut bindings = locals.clone();
                for &statement in statements {
                    let binding = &f.nodes[statement];
                    if binding.kind != Let || !binding.leaves.iter().any(|token| token.text == "=") {
                        return Err(unresolved());
                    }
                    let (pattern, annotation, expr) = match *binding.children.as_slice() {
                        [p, e] => (p, None, e),
                        [p, t, e] => (p, Some(t), e),
                        _ => return Err(unresolved()),
                    };
                    if f.nodes[pattern].kind != BindingPattern {
                        return Err(unresolved());
                    }
                    let value = self.compile(unit, expr, &bindings, types, intermediate, depth + 1)?;
                    let value = if let Some(ty) = annotation {
                        let Some(Symbol::Builtin(name)) = unit.symbol(ty) else {
                            return Err(unresolved());
                        };
                        let ty = ValueType::integer(name).ok_or_else(unresolved)?;
                        IntegerOperand::typed(ty.clone(), value.land(&ty)?)
                    } else {
                        value
                    };
                    bindings.insert(f.span(pattern), value);
                }
                self.compile(
                    unit,
                    unit.single_child(*tail).map_err(|e| e.message)?,
                    &bindings,
                    types,
                    intermediate,
                    depth + 1,
                )
            }
            Literal => {
                let (value, suffix) = numeric_literals::literal(f, n).map_err(|e| e.message)?;
                let operand = IntegerOperand { ty: value, term: None };
                match suffix {
                    None => Ok(operand),
                    Some(ty) => Ok(IntegerOperand::typed(ty.clone(), operand.land(&ty)?)),
                }
            }
            PathExpression | PathType => match unit.symbol(n) {
                Some(Symbol::Binding { span, .. }) => locals
                    .get(span)
                    .cloned()
                    .or_else(|| {
                        let formal = unit.parameters.get(span)?;
                        let ValueType::Parameter { owner, index } = formal else { return None };
                        Some(IntegerOperand::typed(
                            types?.parameter_carrier(formal)?.clone(),
                            IntegerIndex::Binder {
                                owner: owner.clone(),
                                index: *index,
                            },
                        ))
                    })
                    .ok_or_else(unresolved),
                Some(Symbol::Declaration(id)) if id.kind == Const => {
                    let routine = self.routines.get(id).ok_or_else(unresolved)?;
                    Ok(IntegerOperand::typed(routine.result.clone(), routine.apply(vec![])))
                }
                _ => Err(unresolved()),
            },
            Paren => child(unit.single_child(n).map_err(|e| e.message)?, intermediate),
            Unary if unit.operator(n).is_some_and(|op| op.token == "-") => {
                let mut value = child(unit.single_child(n).map_err(|e| e.message)?, intermediate)?;
                match &mut value.ty {
                    ValueType::Exact { value, .. } => *value = -value.clone(),
                    ty => {
                        let t = IntegerIndex::primitive(
                            "-",
                            ty,
                            vec![
                                IntegerIndex::Value(0.into()),
                                value.term.take().ok_or_else(unresolved)?,
                            ],
                        );
                        intermediate.push((n, t.clone(), ty.clone()));
                        value.term = Some(t);
                    }
                }
                Ok(value)
            }
            Binary => {
                let [a, b] = *node.children.as_slice() else { return Err(unresolved()) };
                let left = child(a, intermediate)?;
                let right = child(b, intermediate)?;
                let token = unit.operator(n).ok_or_else(unresolved)?.token.as_str();
                if let (
                    ValueType::Exact {
                        value: a,
                        fractional_origin: af,
                    },
                    ValueType::Exact {
                        value: b,
                        fractional_origin: bf,
                    },
                ) = (&left.ty, &right.ty)
                {
                    return Ok(IntegerOperand {
                        ty: numeric_literals::exact_binary(token, a, b, af.or(*bf), f.span(n)).map_err(str::to_owned)?,
                        term: None,
                    });
                }
                if !matches!(
                    token,
                    "+" | "-"
                        | "*"
                        | "/"
                        | "%"
                        | "+%"
                        | "-%"
                        | "*%"
                        | "/%"
                        | "%%"
                        | "+|"
                        | "-|"
                        | "*|"
                        | "/|"
                        | "%|"
                        | "&"
                        | "|"
                        | "^"
                        | "<<"
                        | ">>"
                        | "<<%"
                        | ">>%"
                        | "<<|"
                        | ">>|"
                ) {
                    return Err(unresolved());
                }
                let ty = if left.term.is_some() { left.ty.clone() } else { right.ty.clone() };
                let count_type = if token.starts_with("<<") || token.starts_with(">>") {
                    match &right.ty {
                        ValueType::Integer { .. } => &right.ty,
                        _ => &ty,
                    }
                } else {
                    &ty
                };
                let term = IntegerIndex::primitive(token, &ty, vec![left.land(&ty)?, right.land(count_type)?]);
                intermediate.push((n, term.clone(), ty.clone()));
                Ok(IntegerOperand::typed(ty, term))
            }
            Call => {
                let [callee, args] = *node.children.as_slice() else {
                    return Err(unresolved());
                };
                if !f.nodes[callee].children.is_empty() {
                    return Err(unresolved());
                }
                let values = f.nodes[args]
                    .children
                    .iter()
                    .map(|&a| child(a, intermediate))
                    .collect::<std::result::Result<Vec<_>, _>>()?;
                let ids = match unit.symbol(callee) {
                    Some(Symbol::Declaration(id)) => std::slice::from_ref(id),
                    Some(Symbol::Candidates(ids)) => ids.as_slice(),
                    _ => return Err(unresolved()),
                };
                let mut selected = None;
                for id in ids {
                    let routine = self.routines.get(id).ok_or_else(unresolved)?;
                    if routine.parameters.len() != values.len()
                        || !routine
                            .parameters
                            .iter()
                            .zip(&values)
                            .all(|(p, v)| p.admits(&v.ty))
                    {
                        continue;
                    }
                    if selected.replace(routine).is_some() {
                        return Err("ambiguous static call".into());
                    }
                }
                let routine = selected.ok_or_else(unresolved)?;
                let args = routine
                    .parameters
                    .iter()
                    .zip(&values)
                    .map(|(p, v)| v.land(p))
                    .collect::<std::result::Result<Vec<_>, _>>()?;
                let term = routine.apply(args);
                intermediate.push((n, term.clone(), routine.result.clone()));
                Ok(IntegerOperand::typed(routine.result.clone(), term))
            }
            _ => Err(unresolved()),
        }
    }
}
