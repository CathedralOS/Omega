use std::ops::Deref;

use omega_typed::{
    MachineApplication, NodeSemantics, ObligationKind, TypedCall, TypedExpression, TypedNumericLanding, TypedOperation, TypedTree, TypingObligation,
};

use crate::{LoweringError, site_at};

/// An application substitutes the types on the checked occurrences of one body.
/// The original tree still owns its symbols, positions and selected operations.
pub(crate) struct LoweringTree<'a> {
    template: &'a TypedTree<'a>,
    records: Option<Vec<(usize, AppliedRecord<'a>)>>,
}

impl<'a> LoweringTree<'a> {
    pub(crate) fn plain(template: &'a TypedTree<'a>) -> Self {
        Self { template, records: None }
    }

    pub(crate) fn for_application(
        template: &'a TypedTree<'a>,
        signature: &omega_typed::TypedSignature,
        application: &MachineApplication,
        callbacks: &[super::callbacks::Callback<'_>],
    ) -> Result<Self, LoweringError> {
        if application.arguments.is_empty() && application.enclosing_arguments.is_empty() {
            return Ok(Self::plain(template));
        }
        let declaration = signature.node.node;
        let file = template.resolved().file;
        let mut records = Vec::new();
        let mut node = declaration;
        while node < file.nodes[declaration].end {
            if node != declaration && file.nodes[node].kind == omega_syntax::NodeKind::Machine {
                node = file.nodes[node].end;
                continue;
            }
            for record in template.semantics_at(node) {
                // An inline implementation also carries its enclosing conformance row.
                // The package publishes that declaration-wide obligation once for every specialization.
                if node == declaration
                    && match record {
                        NodeSemantics::RequirementBinding(row) => row.implementation == signature.machine,
                        NodeSemantics::Obligation(o) => {
                            matches!(&o.kind, ObligationKind::RefineRequirement { implementation, .. } if implementation == &signature.machine)
                        }
                        _ => false,
                    }
                {
                    records.push((node, AppliedRecord::Borrowed(record)));
                    continue;
                }
                // Generic result types can carry a loan. A result made only of scalar bits
                // cannot transport any storage identity across this return.
                if let NodeSemantics::Obligation(o) = record
                    && matches!(&o.kind,ObligationKind::ReturnBorrow {machine} if machine==&signature.machine)
                    && super::types::has_only_scalar_payload(&signature.result)
                {
                    continue;
                }
                if let NodeSemantics::IndirectCall(call) = record
                    && let omega_typed::InvocationTarget::StaticParameter { owner, index } = &call.entry
                    && owner == application.machine.as_ref()
                    && let Some(binding) = callbacks.iter().find(|b| b.slot == *index)
                {
                    let selected = binding
                        .lower_call(call, application)
                        .ok_or_else(|| LoweringError::new(&call.site, "static call disagrees with its selected callable contract"))?;
                    records.push((node, AppliedRecord::SelectedCallback(Box::new(selected))));
                    continue;
                }
                records.push((
                    node,
                    AppliedRecord::instantiate(record, application)
                        .ok_or_else(|| LoweringError::new(&site_at(template, node), "machine application has an unsupported typed record"))?,
                ));
            }
            node += 1;
        }
        Ok(Self {
            template,
            records: Some(records),
        })
    }

    pub(crate) fn has_acknowledged_call(
        &self,
        mut node: usize,
    ) -> bool {
        let file = self.resolved().file;
        while let (omega_syntax::NodeKind::CallMarker | omega_syntax::NodeKind::Paren, [inner]) = (file.nodes[node].kind, file.nodes[node].children.as_slice())
        {
            node = *inner;
        }
        self.semantics_at(node).any(|row| match row {
            NodeSemantics::Call(c) => c.acknowledgements != Default::default(),
            NodeSemantics::IndirectCall(c) => c.acknowledgements != Default::default(),
            _ => false,
        })
    }
    pub(crate) fn semantics_at(
        &self,
        node: usize,
    ) -> impl Iterator<Item = NodeSemantics<'_>> {
        let plain = self
            .records
            .is_none()
            .then(|| self.template.semantics_at(node))
            .into_iter()
            .flatten();
        let applied = self.records.iter().flat_map(move |rows| {
            let first = rows.partition_point(|(index, _)| *index < node);
            rows[first..]
                .iter()
                .take_while(move |(index, _)| *index == node)
                .map(|(_, record)| record.as_record())
        });
        plain.chain(applied)
    }
}
impl<'a> Deref for LoweringTree<'a> {
    type Target = TypedTree<'a>;
    fn deref(&self) -> &Self::Target {
        self.template
    }
}

enum AppliedRecord<'a> {
    Expression(TypedExpression),
    Landing(TypedNumericLanding),
    Operation(TypedOperation),
    Call(Box<TypedCall>),
    SelectedCallback(Box<TypedCall>),
    Obligation(Box<TypingObligation>),
    Borrowed(NodeSemantics<'a>),
}

impl<'a> AppliedRecord<'a> {
    fn instantiate(
        record: NodeSemantics<'a>,
        app: &MachineApplication,
    ) -> Option<Self> {
        use NodeSemantics::*;
        Some(match record {
            Expression(e) => Self::Expression(TypedExpression {
                node: e.node,
                site: e.site.clone(),
                static_argument_slot: e.static_argument_slot,
                ty: app.instantiate(&e.ty),
            }),
            Landing(l) => Self::Landing(TypedNumericLanding {
                node: l.node,
                site: l.site.clone(),
                ty: app.instantiate(&l.ty),
                value: l.value.clone(),
            }),
            Operation(o) => Self::Operation(TypedOperation {
                node: o.node,
                site: o.site.clone(),
                token: o.token.clone(),
                result: app.instantiate(&o.result),
                operands: o.operands.iter().map(|p| app.instantiate(p)).collect(),
            }),
            Call(c) => Self::Call(Box::new(c.with_substitutions(|v| app.instantiate(v)))),
            Obligation(o) => {
                let kind = match &o.kind {
                    ObligationKind::OperationContract { token, operands } => ObligationKind::OperationContract {
                        token: token.clone(),
                        operands: operands.iter().map(|p| app.instantiate(p)).collect(),
                    },
                    ObligationKind::ExactCast { source, destination } => ObligationKind::ExactCast {
                        source: app.instantiate(source),
                        destination: app.instantiate(destination),
                    },
                    ObligationKind::ProveGuarantee { .. }
                    | ObligationKind::CallPrecondition(_)
                    | ObligationKind::Termination(_)
                    | ObligationKind::ReturnBorrow { .. }
                    | ObligationKind::Borrow { .. }
                    // Crash records name only a cause, a caller and their exact site.
                    | ObligationKind::TotalCrashGuard
                    | ObligationKind::CrashCoverage { .. } => return Some(Self::Borrowed(record)),
                    _ => return None,
                };
                Self::Obligation(Box::new(TypingObligation {
                    node: o.node,
                    site: o.site.clone(),
                    kind,
                }))
            }
            Constant(_) | Field(_) => Self::Borrowed(record),
            _ => return None,
        })
    }
    fn as_record(&self) -> NodeSemantics<'_> {
        match self {
            Self::Expression(v) => NodeSemantics::Expression(v),
            Self::Landing(v) => NodeSemantics::Landing(v),
            Self::Operation(v) => NodeSemantics::Operation(v),
            Self::Call(v) | Self::SelectedCallback(v) => NodeSemantics::Call(v),
            Self::Obligation(v) => NodeSemantics::Obligation(v),
            Self::Borrowed(v) => *v,
        }
    }
}
