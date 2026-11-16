//! The closed integer types one complete encoding retains. The encoder walks
//! the whole module: it refuses a symbolic width wherever one occurs, without
//! substituting a value or keeping a second list of fields. The sealed module
//! that keeps this catalog beside its bytes and identity is
//! `terminal_codec::ClosedTerminalModule`.

use std::collections::BTreeMap;

use semantic_vocabulary::{
    CallableContractId, ClosedIntegerType, ClosedScalarType, IntegerType, ScalarType,
};

use crate::CodecError;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ClosedTypeCatalog {
    integers: BTreeMap<IntegerType, ClosedIntegerType>,
    callables: Vec<CallableContractId>,
}

impl ClosedTypeCatalog {
    pub(crate) fn record(&mut self, integer: IntegerType) -> Result<ClosedIntegerType, CodecError> {
        if let Some(closed) = self.integers.get(&integer) {
            return Ok(*closed);
        }
        let closed =
            ClosedIntegerType::try_from(integer).map_err(CodecError::IntegerWidthClosure)?;
        self.integers.insert(integer, closed);
        Ok(closed)
    }

    /// A raw input must name a descriptor this module actually retains.
    /// Independently constructed native constants use the core closed types.
    pub fn integer(&self, integer: IntegerType) -> Result<ClosedIntegerType, CodecError> {
        self.integers
            .get(&integer)
            .copied()
            .ok_or(CodecError::MalformedStructuralFoundation(
                "integer type is not retained by this closed module",
            ))
    }

    /// Membership comes from the exact module catalog at complete encoding.
    pub(crate) fn record_callables(&mut self, module: &terminal_psi::TerminalModule) {
        self.callables = module
            .callables
            .contracts
            .iter()
            .filter(|contract| module.callables.contract(contract.id).is_some())
            .map(|contract| contract.id)
            .collect();
        self.callables.sort_unstable();
        self.callables.dedup();
    }

    pub fn scalar(&self, scalar: ScalarType) -> Result<ClosedScalarType, CodecError> {
        Ok(match scalar {
            ScalarType::Boolean => ClosedScalarType::Boolean,
            ScalarType::Integer(integer) => ClosedScalarType::Integer(self.integer(integer)?),
            ScalarType::IeeeFloat(format) => ClosedScalarType::IeeeFloat(format),
            ScalarType::Callable(contract) => {
                self.callables.binary_search(&contract).map_err(|_| {
                    CodecError::MalformedStructuralFoundation(
                        "callable contract is not retained by this closed module",
                    )
                })?;
                ClosedScalarType::Callable(contract)
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::semantic_module::scalar_wire::encode_integer_type;
    use crate::wire_fields::wire::Writer;
    use semantic_vocabulary::{
        IntegerMathTerm, IntegerSign, IntegerValue, MachineId, Proposition, ValueId, WidthParameter,
    };

    fn parameterized() -> IntegerType {
        IntegerType::parameterized(
            IntegerSign::Unsigned,
            WidthParameter {
                owner: MachineId::new(1).unwrap(),
                value: ValueId::new(3).unwrap(),
            },
        )
    }

    #[test]
    fn closed_integer_encoding_keeps_the_existing_fixed_and_address_bytes() {
        for (integer, expected) in [
            (
                IntegerType::new(IntegerSign::Signed, 32).unwrap(),
                [1, 32, 0],
            ),
            (
                IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
                [2, 64, 0],
            ),
            (IntegerType::address(64).unwrap(), [3, 64, 0]),
        ] {
            let mut writer = Writer::default();
            encode_integer_type(&mut writer, integer).unwrap();
            let (bytes, catalog) = writer.finish_closed();
            assert_eq!(bytes, expected);
            assert_eq!(catalog.integer(integer).unwrap().as_checking(), integer);
        }
    }

    #[test]
    fn a_symbolic_integer_inside_a_mathematical_proposition_is_refused_by_runtime_encoding() {
        let proposition = Proposition::IntegerMathLessOrEqual(
            IntegerMathTerm::MathValue {
                source_type: parameterized(),
                value: ValueId::new(2).unwrap(),
            },
            IntegerMathTerm::literal(IntegerValue::Unsigned(10)),
        );
        let mut runtime = Writer::default();
        assert!(matches!(
            crate::semantic_module::encode_proposition(&mut runtime, &proposition),
            Err(CodecError::IntegerWidthClosure(_))
        ));
        assert!(crate::canonical_proposition_order_key(&proposition).is_ok());
    }

    #[test]
    fn checking_keys_retain_width_owner_value_and_sign_without_changing_closed_bytes() {
        use semantic_vocabulary::{ScalarTerm, WidthParameter};
        let key = |sign, owner, value| {
            crate::canonical_scalar_term_order_key(&ScalarTerm::Value {
                id: ValueId::new(9).unwrap(),
                scalar_type: ScalarType::Integer(IntegerType::parameterized(
                    sign,
                    WidthParameter {
                        owner: MachineId::new(owner).unwrap(),
                        value: ValueId::new(value).unwrap(),
                    },
                )),
            })
            .unwrap()
        };
        let base = key(IntegerSign::Unsigned, 1, 3);
        assert!(!base.is_empty());
        assert_eq!(base, key(IntegerSign::Unsigned, 1, 3));
        assert_ne!(base, key(IntegerSign::Unsigned, 2, 3));
        assert_ne!(base, key(IntegerSign::Unsigned, 1, 4));
        assert_ne!(base, key(IntegerSign::Signed, 1, 3));
        let integer = IntegerType::new(IntegerSign::Unsigned, 32).unwrap();
        let term = ScalarTerm::Value {
            id: ValueId::new(9).unwrap(),
            scalar_type: ScalarType::Integer(integer),
        };
        let mut runtime = Writer::default();
        crate::semantic_module::scalar_term_wire::encode_scalar_term(&mut runtime, &term).unwrap();
        assert_eq!(
            crate::canonical_scalar_term_order_key(&term).unwrap(),
            runtime.finish()
        );
    }

    #[test]
    fn nested_checking_keys_preserve_the_role_without_opening_publication() {
        let mut checking = Writer::checking_identity();
        let mut nested = checking.nested();
        encode_integer_type(&mut nested, parameterized()).unwrap();
        let bytes = nested.finish();
        let mut expected = vec![2];
        parameterized()
            .width()
            .write_identity(|part| expected.extend_from_slice(part));
        assert_eq!(bytes, expected);
        checking.bytes(&bytes);
        assert_eq!(checking.finish(), expected);
        let mut runtime = Writer::default().nested();
        assert!(matches!(
            encode_integer_type(&mut runtime, parameterized()),
            Err(CodecError::IntegerWidthClosure(_))
        ));
    }

    #[test]
    fn nested_key_writers_share_the_same_closed_descriptors() {
        let integer = IntegerType::new(IntegerSign::Unsigned, 13).unwrap();
        let mut writer = Writer::default();
        let mut key = writer.nested();
        encode_integer_type(&mut key, integer).unwrap();
        writer.bytes(&key.finish());
        let (bytes, catalog) = writer.finish_closed();
        assert_eq!(bytes, [2, 13, 0]);
        assert_eq!(catalog.integer(integer).unwrap().bits(), 13);
    }
}
