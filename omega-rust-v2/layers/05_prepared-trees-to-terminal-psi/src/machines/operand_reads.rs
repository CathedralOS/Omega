//! Runtime operands read by lowered operations and continuations.
use crate::{CallArgument, CallTarget, CleanupAction, OperationKind, OperationResult, PlaceId, ValueOperand};

/// Dense local keys keep scalar values distinct from structural roots.
pub(super) struct Keys(pub(super) usize);
impl Keys {
    pub(super) fn place(
        &self,
        p: PlaceId,
    ) -> usize {
        self.0 + p.index()
    }
    pub(super) fn result(
        &self,
        v: OperationResult,
    ) -> Option<usize> {
        match v {
            OperationResult::Scalar(v) => Some(v.index()),
            OperationResult::Structural(p) => Some(self.place(p)),
            OperationResult::Unit => None,
        }
    }
    pub(super) fn projected(
        &self,
        place: &crate::ProjectedPlace,
        mut read: impl FnMut(usize),
    ) {
        read(self.place(place.root));
        for step in &place.path {
            match step {
                crate::PlaceProjection::RuntimeIndex(index) => read(index.selector.index()),
                crate::PlaceProjection::ViewElement { index, length } => {
                    read(index.selector.index());
                    read(length.index());
                }
                _ => {}
            }
        }
    }
    pub(super) fn operand(
        &self,
        operand: &ValueOperand,
        mut read: impl FnMut(usize),
    ) {
        match operand {
            ValueOperand::Scalar(x) => read(x.index()),
            ValueOperand::CopyWhole(p) | ValueOperand::MoveWhole(p) => read(self.place(*p)),
            ValueOperand::Borrow { place, .. } => self.projected(place, read),
        }
    }
    pub(super) fn cleanup(
        &self,
        actions: &[CleanupAction],
        mut read: impl FnMut(usize),
    ) {
        for action in actions {
            match action {
                CleanupAction::DiscardRoot(p) | CleanupAction::RetireHome(p) => read(self.place(*p)),
                CleanupAction::DiscardResidual { place, .. } | CleanupAction::Invoke { place, .. } => self.projected(place, &mut read),
            }
        }
    }
    pub(super) fn call(
        &self,
        target: CallTarget,
        args: &[CallArgument],
        reads: &mut Vec<usize>,
    ) {
        if let CallTarget::Runtime { descriptor, .. } = target {
            reads.push(descriptor.index());
        }
        for arg in args {
            self.operand(arg, |key| reads.push(key));
        }
    }
    pub(super) fn operation_reads(
        &self,
        op: &OperationKind,
    ) -> Result<Vec<usize>, &'static str> {
        use OperationKind::*;
        let mut reads = Vec::new();
        match op {
            Constant(_) | ConstructCallableDescriptor { .. } | DescribeMachine(_) | EstablishZeroedStructural => {}
            // An erased citation names its subjects without demanding them at runtime.
            ApplyTheorem(_) => {}
            Boolean { operands, .. } | Integer { operands, .. } | Float { operands, .. } | EstablishScalarArray { elements: operands } => {
                reads.extend(operands.iter().map(|v| v.index()))
            }
            TrappingInteger { left, right, .. } => reads.extend([left.index(), right.index()]),
            ExactNumericCast { operand } | RefineCallableDescriptor { source: operand, .. } | EstablishPrimitiveLocal { value: operand, .. } => {
                reads.push(operand.index())
            }
            Call { callee, arguments, .. } => self.call(*callee, arguments, &mut reads),
            ByteSequenceLength { source } | ElementViewLength { source } | ScalarBitRecast { source, .. } => reads.push(self.place(*source)),
            MutableScalarBitRecast { lender, .. } => reads.push(self.place(*lender)),
            ByteSequenceRead { source, index, length, .. } | ElementViewRead { source, index, length, .. } => {
                reads.extend([self.place(*source), index.index(), length.index()])
            }
            ElementViewWrite {
                destination,
                index,
                value,
                length,
                ..
            } => reads.extend([self.place(*destination), index.index(), value.index(), length.index()]),
            BorrowPlace { source, .. }
            | BorrowedMove { source }
            | CopyStructural { source }
            | MoveStructural { source }
            | StructuralCaseMembership { source, .. }
            | DecomposeRecord { source, .. } => self.projected(source, |key| reads.push(key)),
            StructuralStore {
                destination,
                value,
                disposition,
            } => {
                self.projected(destination, |key| reads.push(key));
                reads.push(self.place(*value));
                if let crate::StructuralStoreDisposition::Dispose(actions) = disposition {
                    self.cleanup(actions, |key| reads.push(key));
                }
            }
            EstablishRecord { fields } | EstablishCase { fields, .. } => {
                for field in fields {
                    self.operand(&field.operand, |key| reads.push(key));
                }
            }
            StructuralScalarFieldStore { destination, value, .. } | ProjectedScalarStore { destination, value } => {
                self.projected(destination, |key| reads.push(key));
                reads.push(value.index());
            }
            ProjectedScalarRead { source } => self.projected(source, |key| reads.push(key)),
            StructuralLeafCopy { source, path } | PrimitiveScalarRead { source, path } => {
                reads.push(self.place(*source));
                reads.extend(path.iter().map(|p| p.selector.index()));
            }
            EstablishStructuralArray { elements } => reads.extend(elements.iter().map(|p| self.place(*p))),
            BeginAssembly(_) | AssemblyInstruction(_) => return Err("suspension needs the assembly register and control frontier"),
        }
        Ok(reads)
    }
}
