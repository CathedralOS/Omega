use std::ops::Deref;

use omega_typed::{
    MachineApplication, NodeSemantics, ObligationKind, TypedCall, TypedExpression, TypedNumericLanding, TypedOperation, TypedTree, TypingObligation, ValueType,
};

use crate::{LoweringError, site_at};

/// An application substitutes the types on the checked occurrences of one body.
/// The original tree still owns its symbols, positions and selected operations.
pub(crate) struct LoweringTree<'a> {
    template: &'a TypedTree<'a>,
    records: Option<Vec<(usize, AppliedRecord<'a>)>>,
    pub(super) positive_widths: Vec<crate::PositiveWidth>,
    /// Resolved binder node, ordered argument slot, and checked static integer.
    integer_binders: Vec<(usize, usize, ValueType)>,
    pub(crate) callback_routes: Vec<(&'a omega_typed::IndirectCall, crate::CallbackCrashRoutes)>,
    pub(crate) guarded_rows: Vec<usize>,
    /// Call node, invoked binder and the number of clauses its bound owes.
    callback_invocations: Vec<(usize, usize, usize)>,
}

impl<'a> LoweringTree<'a> {
    pub(crate) fn plain(template: &'a TypedTree<'a>) -> Self {
        Self {
            template,
            records: None,
            positive_widths: Vec::new(),
            integer_binders: Vec::new(),
            callback_routes: Vec::new(),
            guarded_rows: Vec::new(),
            callback_invocations: Vec::new(),
        }
    }

    /// The closed static binder invoked at `node` and the count of its bound's
    /// preconditions, when this instance selected that binder.
    pub(crate) fn callback_invocation(&self, node: usize) -> Option<(usize, usize)> {
        self.callback_invocations.iter().find(|(at, _, _)| *at == node).map(|&(_, binder, count)| (binder, count))
    }

    pub(crate) fn integer_binding(&self, binder: usize) -> Option<(usize, &ValueType)> {
        self.integer_binders.iter().find(|(node, _, _)| *node == binder).map(|(_, slot, arg)| (*slot, arg))
    }

    pub(crate) fn for_application(
        typed: &omega_typed::TypedStage,
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
        let integer_binders = file.generic_parameters(declaration)
            .zip(&application.arguments)
            .enumerate()
            .filter(|(_, ((_, _, kind), arg))| *kind == omega_syntax::NodeKind::ConstParam && matches!(arg, ValueType::StaticInteger { .. }))
            .map(|(slot, ((_, binder, _), arg))| (binder, slot, arg.clone()))
            .collect();
        let mut positive_widths = Vec::new();
        let mut records = Vec::new();
        let mut callback_routes = Vec::new();
        let mut guarded_rows = Vec::new();
        let mut callback_invocations = Vec::new();
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
                if let NodeSemantics::Obligation(o) = record
                    && let ObligationKind::PositiveWidth(width) = &o.kind
                {
                    let ValueType::Parameter { owner, index } = width else {
                        return Err(LoweringError::new(&o.site, "computed machine width needs a closed formation operand"));
                    };
                    if owner != &application.machine || !matches!(application.arguments.get(*index), Some(ValueType::StaticInteger { carrier, .. })
                        if matches!(carrier.as_ref(), ValueType::Integer { signed: false, width: Some(64) })) {
                        return Err(LoweringError::new(&o.site, "machine width lacks its selected integer argument"));
                    }
                    positive_widths.push(crate::PositiveWidth { argument: *index, site: o.site.clone() });
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
                    if call.use_site == omega_typed::CallUse::StaticArgument && !binding.requires.is_empty() {
                        return Err(LoweringError::new(&call.site, "static argument callback needs its bound premises"));
                    }
                    let selected = binding
                        .lower_call(call, application)
                        .ok_or_else(|| LoweringError::new(&call.site, "static call disagrees with its selected callable contract"))?;
                    if binding.selection.crash_ceiling.as_ref().is_some_and(|c| !c.is_empty()) {
                        let (routes, rows) = crate::declarations::crash_routes::closed_callback_routes(typed, application, binding, call)?;
                        let operand_formals = crate::declarations::machine_templates::tail_binder_formals(template, signature, node, *index, binding.bound)?;
                        callback_routes.push((call, crate::CallbackCrashRoutes {
                            binder: *index, routes, operand_formals, invocation: call.site.clone(),
                        }));
                        guarded_rows.extend(rows);
                    }
                    callback_invocations.push((node, *index, binding.requires.len()));
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
            integer_binders,
            positive_widths,
            callback_routes,
            guarded_rows,
            callback_invocations,
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
