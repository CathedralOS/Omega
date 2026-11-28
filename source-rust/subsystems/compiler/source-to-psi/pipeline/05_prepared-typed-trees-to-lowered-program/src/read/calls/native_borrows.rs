//! An exact Core declaration supplies the native trust assertion. Its ordinary
//! applied signature determines the argument, referent, lifetime and access.
use super::*;
use crate::source_state::expression::ExpressionNode;
use language_core::sealed_intrinsics::{CompilerPrimitive, NativeBorrow, SealedIntrinsic};

#[derive(Clone, Copy)]
enum Window {
    Bytes { capacity: Option<u64> },
    Elements(ValueType),
}

#[derive(Clone, Copy)]
enum BorrowedStorage {
    Value(ValueType),
    Window(Window, crate::body::Qualification),
}

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
        let Some((destination, access)) = self.native_borrow_storage(id, declared)? else {
            return Err(unavailable("native borrow result has no reference carrier"));
        };
        let expected_access = if operation == NativeBorrow::SharedAt {
            StructuralAccess::SharedBorrow
        } else {
            StructuralAccess::MutableBorrow
        };
        if access != expected_access {
            return Err(unavailable("native borrow changed its result access"));
        }
        let (operand, mut regions) = match operation {
            NativeBorrow::SharedAt | NativeBorrow::MutableAt => {
                if !matches!(destination, BorrowedStorage::Value(_)) {
                    return Err(unavailable("a raw address supplies no slice length"));
                }
                let operand = self.expression(id, slots, *argument, Some(formal_type))?;
                if !formal_type
                    .scalar()
                    .is_some_and(|ty| ty.as_integer().is_some_and(|i| i.is_address()))
                    || !self.same_type(operand.result, formal_type)
                {
                    return Err(unavailable("native borrow needs an address operand"));
                }
                (operand, self.region_tuple(declared.regions()).to_vec())
            }
            NativeBorrow::Upgrade => {
                if !self.shared_upgrade_input(id, formal_type, destination)? {
                    return Err(unavailable(
                        "upgrade changes the reference or its view geometry",
                    ));
                }
                // A sized formal is represented by its referent in the body.
                // A slice formal already holds its window, including its length.
                let direct_parameter = matches!(destination, BorrowedStorage::Value(_))
                    && matches!(self.node(id, *argument)?, ExpressionNode::Name(path)
                        if self.slot_of(id, slots, path).is_ok_and(|i| slots[i].borrowed.is_some()));
                let operand = if direct_parameter {
                    self.borrow_value(id, slots, *argument, formal_type, source)?
                } else {
                    self.expression(id, slots, *argument, Some(formal_type))?
                };
                let actual = operand
                    .result
                    .ok_or_else(|| unavailable("upgrade operand has no reference"))?;
                if !self.shared_upgrade_input(id, actual, destination)? {
                    return Err(unavailable(
                        "upgrade operand must be a shared borrow of the same storage type",
                    ));
                }
                (operand, self.region_tuple(actual.regions()).to_vec())
            }
        };
        let outer = regions
            .first_mut()
            .ok_or_else(|| unavailable("native borrow has no backing lifetime"))?;
        *outer = if operation == NativeBorrow::Upgrade {
            self.loan_region(*outer)
        } else {
            self.fresh_region()
        };
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
    fn shared_upgrade_input(
        &mut self,
        id: MachineId,
        source: ValueType,
        destination: BorrowedStorage,
    ) -> Checked<bool> {
        let Some((source, StructuralAccess::SharedBorrow)) =
            self.native_borrow_storage(id, source)?
        else {
            return Ok(false);
        };
        Ok(match (source, destination) {
            (BorrowedStorage::Value(source), BorrowedStorage::Value(destination)) => {
                self.same_type(Some(source), destination)
            }
            (
                BorrowedStorage::Window(source, source_qualification),
                BorrowedStorage::Window(destination, destination_qualification),
            ) => {
                source_qualification == destination_qualification
                    && match (source, destination) {
                        (Window::Bytes { capacity: a }, Window::Bytes { capacity: b }) => a == b,
                        (Window::Elements(a), Window::Elements(b)) => self.same_type(Some(a), b),
                        _ => false,
                    }
            }
            _ => false,
        })
    }

    /// The shape and qualification the incoming borrow actually carries.
    /// An owned sequence has no lender and cannot be upgraded in place.
    fn native_borrow_storage(
        &mut self,
        id: MachineId,
        value: ValueType,
    ) -> Checked<Option<(BorrowedStorage, StructuralAccess)>> {
        use crate::StorageShape;
        use terminal_psi::ByteSequenceCarrier;
        if let Some((referent, access)) = self.reference_referent(id, value)? {
            return Ok(Some((BorrowedStorage::Value(referent), access)));
        }
        let ValueType::Record(row, qualification, regions) = value else {
            return Ok(None);
        };
        let (window, access) = match self.data_type(id, row)?.shape {
            StorageShape::ByteSequence(ByteSequenceCarrier::BorrowedView {
                access: Some(access),
            }) => (Window::Bytes { capacity: None }, access),
            StorageShape::ByteSequence(ByteSequenceCarrier::BoundedView {
                access: Some(access),
                capacity,
            }) => (
                Window::Bytes {
                    capacity: Some(capacity),
                },
                access,
            ),
            StorageShape::ElementView {
                element,
                qualification,
                access: Some(access),
            } => {
                let (_, nested) = self.region_tuple(regions).split_first().ok_or_else(|| {
                    refuse_at(
                        id,
                        Unserved::Type("element view without its backing lifetime"),
                    )
                })?;
                let element =
                    ValueType::Record(element, qualification, self.intern_regions(nested.to_vec()));
                (Window::Elements(element), access)
            }
            _ => return Ok(None),
        };
        Ok(Some((
            BorrowedStorage::Window(window, qualification),
            access,
        )))
    }
}
