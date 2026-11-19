//! An open static operation keeps its resolved syntax and the owner's exact arguments.
//! L4 supplies values for closed substitutions that require general evaluation.
use super::static_template::StaticTemplate;
use crate::declarations::resolved_source_file::ResolvedSourceFile;
use crate::{IntegerIndex, SpecializedIndexValue, StaticIntegerRequest, TypeError, ValueType};
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
                        // Receipts identify the same resolved source before and after L3 selects names.
                        unit.tree.symbol(i).cloned(),
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
    values: HashMap<(SyntaxKey, ValueType), Arc<HashMap<ClosedArguments, BigInt>>>,
}
impl IndexSpecializations {
    pub fn build(
        units: &[ResolvedSourceFile<'_>],
        receipts: &[SpecializedIndexValue],
    ) -> Result<Self, TypeError> {
        let mut prepared = Self::default();
        for receipt in receipts {
            let fail = |message: &str| TypeError::for_site(&receipt.request.expression, message);
            let n = receipt.request.node.node;
            let unit = units
                .get(receipt.request.node.file)
                .filter(|u| u.file.package == receipt.request.expression.package && u.file.path == receipt.request.expression.file)
                .ok_or_else(|| fail("evaluation result refers to an unadmitted file"))?;
            if n >= unit.file.nodes.len() || unit.file.span(n) != receipt.request.expression.span {
                return Err(fail("evaluation result refers to an unknown static expression"));
            }
            let owner = unit
                .specialization_owner(n)
                .ok_or_else(|| fail("evaluation result has no owning declaration"))?;
            let arguments = ClosedArguments::try_from(receipt.arguments.clone()).map_err(|_| fail("evaluation requires closed arguments"))?;
            let entry = prepared
                .values
                .entry((SyntaxKey::new(unit, n, owner), receipt.request.carrier.clone()))
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
        let key = (SyntaxKey::new(unit, n, owner), carrier.clone());
        let values = self.values.get(&key).cloned().unwrap_or_default();
        Arc::new(StaticTemplate {
            key: key.0,
            request: StaticIntegerRequest::at(unit.file, n, carrier),
            values,
        })
        .apply(args)
    }
}
