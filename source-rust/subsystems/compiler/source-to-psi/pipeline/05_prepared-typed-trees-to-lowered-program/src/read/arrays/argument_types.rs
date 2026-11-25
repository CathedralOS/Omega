//! Compare array types at a call using the scalar arguments that supply their lengths.

use crate::body::{Signature, StorageShape, ValueType};
use super::Program;
use semantic_vocabulary::{MachineId, StructuralTypeId};
use terminal_psi::ArrayExtent;

impl Program<'_> {
    pub(in crate::read) fn array_argument_storage_matches(
        &self,
        mut actual: StructuralTypeId,
        mut expected: StructuralTypeId,
        length_matches: impl Fn(&ArrayExtent, &ArrayExtent) -> bool,
    ) -> bool {
        for _ in 0..=self.records.len() {
            if actual == expected {
                return true;
            }
            let (Some(left), Some(right)) = (self.records.get(actual), self.records.get(expected))
            else {
                return false;
            };
            let (
                StorageShape::FixedArray {
                    element: actual_element,
                    qualification: actual_qualification,
                    length: actual_length,
                },
                StorageShape::FixedArray {
                    element: expected_element,
                    qualification: expected_qualification,
                    length: expected_length,
                },
            ) = (&left.shape, &right.shape)
            else {
                return false;
            };
            if actual_qualification != expected_qualification
                || left.multiplicity != right.multiplicity
                || !left.requires.is_empty()
                || !right.requires.is_empty()
                || left.cleanup.is_some()
                || right.cleanup.is_some()
                || (actual_length != expected_length && !length_matches(actual_length, expected_length))
            {
                return false;
            }
            actual = *actual_element;
            expected = *expected_element;
        }
        false
    }

    /// The refinement call forwards every public input to the same position
    /// in the implementation. An array sized by one of those inputs therefore
    /// has the caller's length at this call. Each declaration still owns its
    /// original storage types and checks its body with its own const inputs.
    pub(crate) fn apply_forwarded_array_lengths(
        &self,
        caller: MachineId,
        callee: MachineId,
        required: &Signature,
        supplied: &mut Signature,
    ) {
        if !self.rigid {
            return;
        }
        let inputs = required.formal_values();
        let formals = supplied.formal_values();
        let forwarded = |actual_length: &ArrayExtent, expected_length: &ArrayExtent| {
            let (
                ArrayExtent::Parameter {
                    owner: from,
                    value: input,
                },
                ArrayExtent::Parameter {
                    owner: to,
                    value: formal,
                },
            ) = (actual_length, expected_length)
            else {
                return false;
            };
            *from == caller
                && *to == callee
                && inputs.iter().zip(&formals).any(|(actual, expected)| {
                    actual.as_ref() == Some(input) && expected.as_ref() == Some(formal)
                })
        };
        for (actual, expected) in required
            .parameters
            .iter()
            .zip(&mut supplied.parameters)
            .chain(required.result.iter().zip(supplied.result.iter_mut()))
        {
            let (ValueType::Record(actual, ..), ValueType::Record(expected, ..)) =
                (actual, expected)
            else {
                continue;
            };
            if self.array_argument_storage_matches(*actual, *expected, forwarded) {
                *expected = *actual;
            }
        }
    }
}
