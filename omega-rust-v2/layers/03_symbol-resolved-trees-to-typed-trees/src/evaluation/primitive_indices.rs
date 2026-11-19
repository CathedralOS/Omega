//! Form primitive integer indices from literals and bound parameters. Source calls retain their typed occurrence for preparation.
use crate::declarations::resolved_source_file::ResolvedSourceFile;
use crate::evaluation::numeric_literals;
use crate::type_system::type_catalog::TypeCatalog;
use crate::{IntegerIndex, TypeError, ValueType};
use omega_parser::NodeKind;
use omega_resolver::{DeclarationId, Symbol};
use std::collections::HashMap;

// Literal constants can participate in type normalization without running source code.
#[derive(Clone, Default)]
pub(crate) struct LiteralConstants(HashMap<DeclarationId, IntegerOperand>);

#[derive(Clone)]
struct IntegerOperand {
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
            && ty.contains_integer(value.numer())
        {
            return Ok(IntegerIndex::Value(value.to_integer()));
        }
        Err(format!("static operand {} does not land in {ty}", self.ty))
    }
}
enum FormationError {
    RequiresEvaluation,
    Invalid(String),
}
impl From<String> for FormationError {
    fn from(message: String) -> Self {
        Self::Invalid(message)
    }
}
impl LiteralConstants {
    pub(crate) fn collect(
        &mut self,
        units: &[ResolvedSourceFile<'_>],
    ) {
        for unit in units {
            let f = unit.file;
            for (node, id) in unit
                .declaration_nodes()
                .filter(|(_, id)| id.kind == NodeKind::Const)
            {
                let syntax = &f.nodes[node];
                let [_, ty, expr] = *syntax.children.as_slice() else { continue };
                if f.nodes[expr].kind != NodeKind::Literal {
                    continue;
                }
                let Some(Symbol::Builtin(name)) = unit.symbol(ty) else { continue };
                let Some(carrier) = ValueType::integer(name) else { continue };
                if let Ok((value, suffix)) = numeric_literals::literal(f, expr)
                    && suffix.as_ref().is_none_or(|ty| ty == &carrier)
                    && let Ok(term) = (IntegerOperand { ty: value, term: None }).land(&carrier)
                {
                    self.0.insert(id.clone(), IntegerOperand::typed(carrier, term));
                }
            }
        }
    }
}
impl ResolvedSourceFile<'_> {
    pub(crate) fn primitive_index(
        &self,
        n: usize,
        types: &TypeCatalog<'_>,
        carrier: &ValueType,
    ) -> Result<Option<IntegerIndex>, TypeError> {
        let mut intermediate = Vec::new();
        let term = match self.form(n, types, &mut intermediate, 0) {
            Ok(operand) => operand
                .land(carrier)
                .map_err(|m| TypeError::at(self.file, n, m))?,
            Err(FormationError::RequiresEvaluation) => return Ok(None),
            Err(FormationError::Invalid(message)) => return Err(TypeError::at(self.file, n, message)),
        };
        for (node, operation, ty) in intermediate {
            let value = operation.typed(ty);
            value
                .validate_closed_indices()
                .map_err(|message| TypeError::at(self.file, node, message))?;
            if value.contains_parameter() {
                types.formations.index(self, node, value);
            }
        }
        Ok(Some(term))
    }
    fn form(
        &self,
        n: usize,
        types: &TypeCatalog<'_>,
        intermediate: &mut Vec<(usize, IntegerIndex, ValueType)>,
        depth: usize,
    ) -> Result<IntegerOperand, FormationError> {
        use NodeKind::*;
        let f = self.file;
        let node = &f.nodes[n];
        if depth >= 192 {
            return Err(FormationError::Invalid("static expression exceeds the normalization nesting budget".into()));
        }
        let child = |i, intermediate: &mut Vec<(usize, IntegerIndex, ValueType)>| self.form(i, types, intermediate, depth + 1);
        let unresolved = || FormationError::RequiresEvaluation;
        match node.kind {
            Literal => {
                let (value, suffix) = numeric_literals::literal(f, n).map_err(|e| e.message)?;
                let operand = IntegerOperand { ty: value, term: None };
                match suffix {
                    None => Ok(operand),
                    Some(ty) => Ok(IntegerOperand::typed(ty.clone(), operand.land(&ty)?)),
                }
            }
            PathExpression | PathType => match self.name_in_role(n, false) {
                Some(Symbol::Binding { span, .. }) => {
                    let formal = self.parameters.get(span).ok_or_else(unresolved)?;
                    let ValueType::Parameter { owner, index } = formal else {
                        return Err(unresolved());
                    };
                    Ok(IntegerOperand::typed(
                        types.parameter_carrier(formal).ok_or_else(unresolved)?.clone(),
                        IntegerIndex::Binder {
                            owner: owner.clone(),
                            index: *index,
                        },
                    ))
                }
                Some(Symbol::Declaration(id)) if id.kind == Const => types
                    .literal_constants
                    .0
                    .get(id)
                    .cloned()
                    .ok_or_else(unresolved),
                _ => Err(unresolved()),
            },
            Paren => child(self.single_child(n).map_err(|e| e.message)?, intermediate),
            Unary if self.file.operator(n).is_some_and(|op| op.token == "-") => {
                let mut value = child(self.single_child(n).map_err(|e| e.message)?, intermediate)?;
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
                let token = self.file.operator(n).ok_or_else(unresolved)?.token;
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
            _ => Err(unresolved()),
        }
    }
}
