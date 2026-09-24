use crate::legalization::scalar_graph_input::exact_cast_has_native_carriers;

use super::{IntegerSign, IntegerType};
#[test]
fn exact_cast_carriers_admit_every_fixed_pair_without_specializing_byte_input() {
    use IntegerSign::{Signed, Unsigned};
    for (source_sign, source_bits, target_sign, target_bits, accepted) in [
        (Signed, 32, Unsigned, 8, true),
        (Signed, 64, Signed, 8, true),
        (Signed, 64, Signed, 32, true),
        (Signed, 8, Signed, 8, true),
        (Signed, 32, Signed, 32, true),
        (Signed, 64, Signed, 64, true),
        (Signed, 16, Signed, 8, true),
        (Unsigned, 64, Unsigned, 32, true),
        (Unsigned, 32, Signed, 16, true),
        (Unsigned, 8, Unsigned, 64, true),
        (Signed, 32, Signed, 64, true),
        (Signed, 64, Signed, 16, true),
        (Unsigned, 16, Unsigned, 64, true),
        (Unsigned, 128, Unsigned, 64, false),
    ] {
        let source = IntegerType::new(source_sign, source_bits).unwrap();
        let target = IntegerType::new(target_sign, target_bits).unwrap();
        assert_eq!(
            exact_cast_has_native_carriers(source, target),
            accepted,
            "{source:?} -> {target:?}"
        );
    }
}
