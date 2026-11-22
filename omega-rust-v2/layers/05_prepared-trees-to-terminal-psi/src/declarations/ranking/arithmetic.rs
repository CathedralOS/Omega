//! Build the entry expression in operand-first order. Locations identify each
//! intermediate formation question without entering the witness's semantic identity.
use crate::lowering::applied_tree::LoweringTree;
use crate::machines::arithmetic::{LoweredInteger, folded_exact_operator, lower_integer};
use crate::{ArithmeticPolicy, IntegerOperation, IntegerPrimitive, LoweringError, RankArithmeticStep, RankExpression, RankOperand, ScalarType, Site, site_at};
use omega_syntax::NodeKind;
use omega_typed::{NodeSemantics, TypedSignature, ValueType};

pub(super) fn subject(
    tree: &LoweringTree<'_>,
    signature: &TypedSignature,
    root: usize,
    result: &ValueType,
    operand: &impl Fn(usize, &ValueType) -> Result<RankOperand<usize>, LoweringError>,
    sites: &mut Vec<Site>,
) -> Result<RankExpression<usize>, LoweringError> {
    enum Visit<'a> {
        Read(usize, &'a ValueType),
        Apply(usize, IntegerOperation),
    }
    let file = tree.resolved().file;
    let mut pending = vec![Visit::Read(root, result)];
    let mut steps = Vec::new();
    while let Some(visit) = pending.pop() {
        let (raw, expected) = match visit {
            Visit::Read(node, expected) => (node, expected),
            Visit::Apply(node, operation) => {
                steps.push(RankArithmeticStep::Apply(operation));
                sites.push(site_at(tree, node));
                continue;
            }
        };
        let node = file.ungroup(raw);
        let refuse = |message| LoweringError::new(&site_at(tree, node), message);
        let ValueType::Integer { signed, width: Some(width) } = expected else {
            return Err(refuse("arithmetic rank operand requires a fixed-width integer"));
        };
        let carrier = ScalarType::Integer {
            signed: *signed,
            width: *width,
        };
        if !matches!(file.nodes[node].kind, NodeKind::Binary | NodeKind::Unary) {
            steps.push(RankArithmeticStep::Operand {
                operand: operand(node, expected)?,
                carrier,
            });
            continue;
        }

        // Untyped literal arithmetic may have been folded and landed by typing.
        // A typed carrier operation, even on constants, still owes its own formation.
        let inherited = tree.semantics_at(raw).find_map(|record| match record {
            NodeSemantics::Landing(landing) if &landing.ty == expected && landing.value.is_integer() => Some(landing.value.numer()),
            _ => None,
        });
        if let Some(value) = folded_exact_operator(tree, node, expected, inherited)? {
            steps.push(RankArithmeticStep::Operand {
                operand: RankOperand::Constant(value.clone()),
                carrier,
            });
            continue;
        }

        let (mut typed, mut selected) = (false, None);
        let mut obligations = Vec::new();
        for record in tree.semantics_at(node) {
            match record {
                NodeSemantics::Expression(e) if !typed && e.static_argument_slot.is_none() && &e.ty == expected => typed = true,
                NodeSemantics::Operation(op) if selected.is_none() && &op.result == expected => selected = Some(op),
                NodeSemantics::Obligation(obligation) => obligations.push(obligation),
                _ => return Err(refuse("unsupported typed arithmetic rank record")),
            }
        }
        let (true, Some(operation)) = (typed, selected) else {
            return Err(refuse("arithmetic rank has no selected operation of its carrier"));
        };
        let LoweredInteger::Ordinary(lowered) = lower_integer(operation, &obligations, &signature.machine)? else {
            return Err(refuse("a trapping operation cannot form a ranking witness"));
        };
        let children = &file.nodes[node].children;
        if children.len() != operation.operands.len() {
            return Err(refuse("arithmetic rank disagrees with its selected operand count"));
        }
        pending.push(Visit::Apply(node, lowered));
        pending.extend(
            children
                .iter()
                .zip(&operation.operands)
                .rev()
                .map(|(&child, ty)| Visit::Read(child, ty)),
        );
    }
    // Preserve the canonical encoding of existing packages.
    match steps.as_slice() {
        [RankArithmeticStep::Operand { operand, .. }] => Ok(RankExpression::Operand(operand.clone())),
        [
            RankArithmeticStep::Operand { operand: minuend, .. },
            RankArithmeticStep::Operand { operand: subtrahend, .. },
            RankArithmeticStep::Apply(op),
        ] if op.primitive == IntegerPrimitive::Subtract && op.policy == ArithmeticPolicy::Exact => Ok(RankExpression::ExactDifference {
            minuend: minuend.clone(),
            subtrahend: subtrahend.clone(),
        }),
        _ => Ok(RankExpression::Arithmetic(steps)),
    }
}
