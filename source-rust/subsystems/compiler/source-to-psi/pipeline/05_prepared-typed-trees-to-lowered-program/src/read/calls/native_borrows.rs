//! An exact Core declaration supplies the native trust assertion. Its ordinary
//! applied signature determines the argument, referent, lifetime and access.
use super::*;
use crate::source_state::expression::ExpressionNode;
use language_core::sealed_intrinsics::{CompilerPrimitive, NativeBorrow, SealedIntrinsic};

impl Program<'_> {
    pub(super) fn native_borrow(
        &mut self,
        id: MachineId,
        slots: &[Slot],
        call: &StaticInvocation<'_>,
        source: SourceOrigin,
    ) -> Checked<Option<Expression>> {
        let typed = self.typed;
        let Some(declaration) = self
            .entered_machine(call.target)
            .and_then(|symbol| typed.machine_by_symbol(symbol))
        else {
            return Ok(None);
        };
        let Some(SealedIntrinsic::CompilerPrimitive(CompilerPrimitive::NativeBorrow(operation))) =
            typed
                .symbols
                .sealed_declaration(declaration.symbol, declaration.name.as_str())
        else {
            return Ok(None);
        };
        let unavailable = |reason| refuse_at(id, Unserved::Call(reason));
        if call.unsupported || call.runtime_receiver {
            return Err(unavailable("native borrow has unsupported call operands"));
        }
        self.visible_to(id, declaration.symbol)?;
        self.direct_selection(id, call.target, call.selection, call.binding)?;
        let header = typed
            .bodyless_callable_signature(call.application, call.target)
            .map_err(unavailable)?
            .ok_or_else(|| unavailable("native borrow lacks its checked applied declaration"))?;
        if !self.owns_call_arguments(
            id,
            call.application,
            declaration.symbol,
            call.authored_arguments,
        ) {
            return Err(unavailable(
                "native borrow operands belong to another application",
            ));
        }
        let [argument] = call.arguments else {
            return Err(unavailable("native borrow needs one argument"));
        };
        let Some([formal]) = typed.state_signature_parameters(header).as_array() else {
            return Err(unavailable("native borrow lost its applied formal"));
        };
        let formal_type = self.local_type(id, formal.type_reference)?;
        let declared = self.local_type(id, header.return_type)?;
        let Some((referent, access)) = self.reference_referent(id, declared)? else {
            return Err(unavailable("native borrow needs a sized reference result"));
        };
        let expected_access = if operation == NativeBorrow::SharedAt {
            StructuralAccess::SharedBorrow
        } else {
            StructuralAccess::MutableBorrow
        };
        if access != expected_access {
            return Err(unavailable("native borrow changed its result access"));
        }
        let (operand, outer) = match operation {
            NativeBorrow::SharedAt | NativeBorrow::MutableAt => {
                let operand = self.expression(id, slots, *argument, Some(formal_type))?;
                if !formal_type
                    .scalar()
                    .is_some_and(|ty| ty.as_integer().is_some_and(|i| i.is_address()))
                    || !self.same_type(operand.result, formal_type)
                {
                    return Err(unavailable("native borrow needs an address operand"));
                }
                (operand, self.fresh_region())
            }
            NativeBorrow::Upgrade => {
                let Some((original, StructuralAccess::SharedBorrow)) =
                    self.reference_referent(id, formal_type)?
                else {
                    return Err(unavailable("upgrade requires a shared reference"));
                };
                if !self.same_type(Some(original), referent) {
                    return Err(unavailable("upgrade changes the referent"));
                }
                // A directly borrowed parameter is represented by its referent in the body.
                // Materialize its descriptor before applying the same upgrade as a held @T.
                let direct_parameter = matches!(self.node(id, *argument)?, ExpressionNode::Name(path)
                    if self.slot_of(id, slots, path).is_ok_and(|i| slots[i].borrowed.is_some()));
                let operand = if direct_parameter {
                    self.borrow_value(id, slots, *argument, formal_type, source)?
                } else {
                    self.expression(id, slots, *argument, Some(formal_type))?
                };
                let actual = operand
                    .result
                    .ok_or_else(|| unavailable("upgrade operand has no reference"))?;
                let Some((original, StructuralAccess::SharedBorrow)) =
                    self.reference_referent(id, actual)?
                else {
                    return Err(unavailable("upgrade operand must be a shared reference"));
                };
                if !self.same_type(Some(original), referent) {
                    return Err(unavailable("upgrade operand has a different referent"));
                }
                let outer = *self
                    .region_tuple(actual.regions())
                    .first()
                    .ok_or_else(|| unavailable("upgrade operand has no backing lifetime"))?;
                (operand, self.loan_region(outer))
            }
        };
        let mut regions = vec![outer];
        regions.extend_from_slice(self.region_tuple(referent.regions()));
        let result = super::super::places::with_regions(declared, self.intern_regions(regions));
        let services = super::super::services::finite_reach(typed, id, header.service_reach_row)?;
        for &service in &services {
            let scope = typed
                .service_reach_scope(service)
                .ok_or_else(|| unavailable("native borrow lost its declared reach"))?;
            self.service(id, scope)?;
        }
        Ok(Some(Expression {
            result: Some(result),
            kind: ExpressionKind::NativeBorrow {
                operation,
                operand: Box::new(operand),
                services,
            },
            call_context: None,
            source,
        }))
    }
}
