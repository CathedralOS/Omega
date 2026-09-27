//! Receiving checks must reject forged current IR after derived metadata is refreshed.

use crate::tests::fixtures::dominance::refresh_function_derivatives;
use crate::tests::fixtures::scalar_units::write_only_store_unit;
use crate::tests::support::id;
use crate::validate_psi_optimization_unit;
use abstract_operations::{AbstractOperation as O, AbstractResult};
use semantic_vocabulary::{OperationId, ValueId};
use terminal_psi::StructuralAccess;

#[test]
fn incoming_primitive_reads_require_readable_access() {
    for access in [
        StructuralAccess::SharedBorrow,
        StructuralAccess::MutableBorrow,
        StructuralAccess::WriteOnlyBorrow,
    ] {
        let mut unit = write_only_store_unit();
        let source = unit.functions[0].structural_parameters[0].place;
        unit.functions[0].structural_parameters[0].access = access;
        let O::WriteOnlyPrimitiveStore { value, .. } =
            unit.functions[0].blocks[0].nodes[1].operation
        else {
            panic!("store")
        };
        unit.functions[0].blocks[0].nodes[1].operation = O::PrimitiveScalarRead {
            path: Vec::new(),
            psi_operation: id(57, OperationId::new),
            source,
            result: AbstractResult {
                value: id(60, ValueId::new),
                scalar_type: value.scalar_type,
            },
        };
        refresh_function_derivatives(&mut unit, 0);
        assert_eq!(
            validate_psi_optimization_unit(&unit).is_ok(),
            access != StructuralAccess::WriteOnlyBorrow
        );
    }
}
