//! An open static operation keeps its resolved syntax and the owner's exact arguments.
//! L4 supplies values for closed substitutions that require general evaluation.
use super::static_template::StaticTemplate;
use crate::declarations::resolved_source_file::ResolvedSourceFile;
use crate::{IntegerIndex, Site, SpecializedIndexValue, TypeError, ValueType};
use num_bigint::BigInt;
use omega_parser::NodeKind;
use omega_resolver::{DeclarationId, Symbol};
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub(super) struct SyntaxKey {
    pub(super) owner: DeclarationId,
    nodes: Vec<(NodeKind, usize, Vec<String>, Option<Symbol>)>,
}
impl SyntaxKey {
    fn new(
        unit: &ResolvedSourceFile<'_>,
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
/// Only fully substituted arguments may index an admitted evaluation result.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub(super) struct ClosedArguments(Vec<ValueType>);

impl TryFrom<Vec<ValueType>> for ClosedArguments {
    type Error = Vec<ValueType>;

    fn try_from(arguments: Vec<ValueType>) -> Result<Self, Self::Error> {
        if arguments.iter().any(ValueType::contains_parameter) {
            Err(arguments)
        } else {
            Ok(Self(arguments))
        }
    }
}
#[derive(Default)]
pub(crate) struct IndexSpecializations {
    values: HashMap<SyntaxKey, Arc<HashMap<ClosedArguments, BigInt>>>,
}
impl IndexSpecializations {
    pub fn build(
        units: &[ResolvedSourceFile<'_>],
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
                .find_map(|i| unit.declaration_at(i))
                .ok_or_else(|| fail("evaluation result has no owning declaration"))?;
            let arguments = ClosedArguments::try_from(receipt.arguments.clone()).map_err(|_| fail("evaluation requires closed arguments"))?;
            let entry = prepared
                .values
                .entry(SyntaxKey::new(unit, n, owner))
                .or_default();
            if Arc::make_mut(entry)
                .insert(arguments, receipt.value.clone())
                .is_some_and(|v| v != receipt.value)
            {
                return Err(fail("conflicting evaluations of the same specialized expression"));
            }
        }
        Ok(prepared)
    }
    pub fn prepare(
        &self,
        unit: &ResolvedSourceFile<'_>,
        n: usize,
        owner: &DeclarationId,
        carrier: &ValueType,
        args: Vec<ValueType>,
    ) -> IntegerIndex {
        let key = SyntaxKey::new(unit, n, owner);
        let values = self.values.get(&key).cloned().unwrap_or_default();
        Arc::new(StaticTemplate {
            key,
            site: Site::at(unit.file, n),
            carrier: carrier.clone(),
            values,
        })
        .apply(args)
    }
}
