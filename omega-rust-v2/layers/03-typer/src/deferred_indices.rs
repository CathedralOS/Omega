//! An open static operation keeps its resolved syntax and the owner's exact arguments.
//! L4 supplies values for closed substitutions that require general evaluation.
use crate::*;
use num_bigint::BigInt;
use std::{
    hash::{Hash, Hasher},
    sync::Arc,
};

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
struct SyntaxKey {
    owner: DeclarationId,
    nodes: Vec<(NodeKind, usize, Vec<String>, Option<Symbol>)>,
}
impl SyntaxKey {
    fn new(
        unit: &Unit<'_>,
        n: usize,
        owner: &DeclarationId,
    ) -> Self {
        Self {
            owner: owner.clone(),
            nodes: (n..unit.file.nodes[n].end)
                .map(|i| {
                    let node = &unit.file.nodes[i];
                    (
                        node.kind,
                        node.children.len(),
                        node.leaves.iter().map(|t| t.text.to_owned()).collect(),
                        unit.symbol(i).cloned(),
                    )
                })
                .collect(),
        }
    }
}
#[derive(Default)]
pub(super) struct Specializations {
    values: HashMap<SyntaxKey, Arc<HashMap<Vec<ValueType>, BigInt>>>,
}
impl Specializations {
    pub fn build(
        units: &[Unit<'_>],
        receipts: &[SpecializedIndexValue],
    ) -> Result<Self, TypeError> {
        let mut prepared = Self::default();
        for receipt in receipts {
            let fail = |message: &str| TypeError {
                package: receipt.expression.package.clone(),
                file: receipt.expression.file.clone(),
                span: receipt.expression.span,
                message: message.into(),
            };
            let unit = units
                .iter()
                .find(|u| u.file.package == receipt.expression.package && u.file.path == receipt.expression.file)
                .ok_or_else(|| fail("evaluation result refers to an unadmitted file"))?;
            let n = (0..unit.file.nodes.len())
                .find(|&i| {
                    unit.file.span(i) == receipt.expression.span
                        && matches!(
                            unit.file.nodes[i].kind,
                            NodeKind::Binary | NodeKind::Unary | NodeKind::Call | NodeKind::Cast | NodeKind::Paren | NodeKind::FieldAccess | NodeKind::Match
                        )
                })
                .ok_or_else(|| fail("evaluation result refers to an unknown static expression"))?;
            let owner = unit
                .file
                .enclosing_contexts(n)
                .find_map(|i| unit.definitions.get(&unit.file.span(i)))
                .ok_or_else(|| fail("evaluation result has no owning declaration"))?;
            if receipt.arguments.iter().any(ValueType::contains_parameter) {
                return Err(fail("evaluation requires closed arguments"));
            }
            let entry = prepared
                .values
                .entry(SyntaxKey::new(unit, n, owner))
                .or_default();
            if Arc::make_mut(entry)
                .insert(receipt.arguments.clone(), receipt.value.clone())
                .is_some_and(|v| v != receipt.value)
            {
                return Err(fail("conflicting evaluations of the same specialized expression"));
            }
        }
        Ok(prepared)
    }
    pub fn prepare(
        &self,
        unit: &Unit<'_>,
        n: usize,
        owner: &DeclarationId,
        carrier: &ValueType,
        args: Vec<ValueType>,
    ) -> IntegerIndex {
        let key = SyntaxKey::new(unit, n, owner);
        let values = self.values.get(&key).cloned().unwrap_or_default();
        Arc::new(StaticTemplate {
            key,
            site: Site::at(&unit.file, n),
            carrier: carrier.clone(),
            values,
        })
        .apply(args)
    }
}
pub struct StaticTemplate {
    key: SyntaxKey,
    pub site: Site,
    carrier: ValueType,
    values: Arc<HashMap<Vec<ValueType>, BigInt>>,
}
impl std::fmt::Debug for StaticTemplate {
    fn fmt(
        &self,
        f: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        self.key.fmt(f)
    }
}
impl PartialEq for StaticTemplate {
    fn eq(
        &self,
        other: &Self,
    ) -> bool {
        self.key == other.key && self.carrier == other.carrier
    }
}
impl Eq for StaticTemplate {}
impl Hash for StaticTemplate {
    fn hash<H: Hasher>(
        &self,
        h: &mut H,
    ) {
        self.key.hash(h);
        self.carrier.hash(h)
    }
}
impl StaticTemplate {
    pub(super) fn apply(
        self: &Arc<Self>,
        arguments: Vec<ValueType>,
    ) -> IntegerIndex {
        if arguments.iter().any(ValueType::contains_parameter) {
            return IntegerIndex::Deferred {
                template: self.clone(),
                arguments,
            };
        }
        match self.values.get(&arguments) {
            Some(v) if crate::integers::fits(v, &self.carrier) => IntegerIndex::Value(v.clone()),
            Some(v) => IntegerIndex::Invalid(format!("static index {v} exceeds {}", self.carrier)),
            None => IntegerIndex::Invalid(format!(
                "specialized static expression in {} needs an admitted evaluation result",
                self.key.owner.path
            )),
        }
    }
}
