//! A closed module keeps its scalar descriptors, canonical execution bytes
//! and their semantic identity together. Closure does not show that a
//! checker accepted the program or that its producer can be trusted. The
//! encoder walks the whole module: it refuses a symbolic width wherever one
//! occurs, without substituting a value or keeping a second list of fields.

use std::collections::BTreeMap;
use std::sync::Arc;

use semantic_vocabulary::{ClosedIntegerType, ClosedScalarType, IntegerType, ScalarType};
use terminal_psi::{TerminalModule, TerminalPsiIdentity};

use crate::CodecError;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ClosedTypeCatalog {
    integers: BTreeMap<IntegerType, ClosedIntegerType>,
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

    /// Every closed integer type this module retains, for a consumer that
    /// must carry the membership forward without the module itself.
    pub fn retained_integers(&self) -> impl Iterator<Item = ClosedIntegerType> + '_ {
        self.integers.values().copied()
    }

    pub fn scalar(&self, scalar: ScalarType) -> Result<ClosedScalarType, CodecError> {
        Ok(match scalar {
            ScalarType::Boolean => ClosedScalarType::Boolean,
            ScalarType::Integer(integer) => ClosedScalarType::Integer(self.integer(integer)?),
            ScalarType::IeeeFloat(format) => ClosedScalarType::IeeeFloat(format),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClosedTerminalModule {
    module: Arc<TerminalModule>,
    types: ClosedTypeCatalog,
    bytes: Arc<[u8]>,
    identity: TerminalPsiIdentity,
}

impl ClosedTerminalModule {
    pub fn new(module: TerminalModule) -> Result<Self, CodecError> {
        let (bytes, types) = crate::semantic_module::encode_closed_execution_module(&module)?;
        Ok(Self::from_encoding(module, bytes, types))
    }

    pub fn from_semantic_bytes(bytes: &[u8]) -> Result<Self, CodecError> {
        let (module, bytes, types) = crate::semantic_module::decode_closed_parts(bytes)?;
        Ok(Self::from_encoding(module, bytes, types))
    }

    fn from_encoding(module: TerminalModule, bytes: Vec<u8>, types: ClosedTypeCatalog) -> Self {
        let identity = TerminalPsiIdentity {
            vocabulary_marker: module.vocabulary_marker,
            program_fingerprint: crate::semantic_module::encoded_fingerprint(&module, &bytes),
        };
        Self {
            module: Arc::new(module),
            types,
            bytes: bytes.into(),
            identity,
        }
    }

    pub fn module(&self) -> &TerminalModule {
        &self.module
    }

    pub const fn closed_types(&self) -> &ClosedTypeCatalog {
        &self.types
    }

    pub fn semantic_bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub const fn identity(&self) -> TerminalPsiIdentity {
        self.identity
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::semantic_module::scalar_wire::encode_integer_type;
    use crate::wire_fields::wire::Writer;
    use semantic_vocabulary::{
        IntegerMathTerm, IntegerSign, IntegerValue, MachineId, Proposition, StructuralTypeId,
        ValueId, WidthParameter,
    };
    use terminal_psi::{
        StructuralTypeDeclaration, StructuralTypeShape, TerminalIdentityTableBuilder,
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

    fn module(integer: IntegerType) -> TerminalModule {
        let mut identities = TerminalIdentityTableBuilder::default();
        let identity = identities.intern("closed_width_test");
        let mut module = TerminalModule::declarations();
        module.identities = identities.build();
        module.structural_types.push(StructuralTypeDeclaration {
            declaration_identity: Default::default(),
            quotient_carrier: None,
            id: StructuralTypeId::new(1).unwrap(),
            identity,
            owns_linear_claim: false,
            copyable: true,
            no_code_affine_cleanup: false,
            descriptor_cleanup: false,
            shape: StructuralTypeShape::PrimitiveScalar(ScalarType::Integer(integer)),
            scalar_restriction: None,
            requires: Vec::new(),
        });
        module
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
    fn a_dormant_symbolic_type_is_refused_by_the_complete_encoder() {
        assert!(matches!(
            ClosedTerminalModule::new(module(parameterized())),
            Err(CodecError::IntegerWidthClosure(_))
        ));
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

    #[test]
    fn closing_and_retaining_keep_the_original_module_storage_and_bytes() {
        let integer = IntegerType::new(IntegerSign::Unsigned, 32).unwrap();
        let module = module(integer);
        let storage = module.structural_types.as_ptr();
        let closed = ClosedTerminalModule::new(module).unwrap();
        let retained = closed.clone();
        assert_eq!(closed.module().structural_types.as_ptr(), storage);
        assert_eq!(retained.module().structural_types.as_ptr(), storage);
        assert!(std::ptr::eq(
            closed.semantic_bytes(),
            retained.semantic_bytes()
        ));
        assert_eq!(closed.identity(), retained.identity());
        assert_eq!(
            closed
                .closed_types()
                .scalar(ScalarType::Integer(integer))
                .unwrap()
                .as_checking(),
            ScalarType::Integer(integer)
        );
        assert!(
            closed
                .closed_types()
                .integer(IntegerType::new(IntegerSign::Unsigned, 13).unwrap())
                .is_err(),
            "an independently closed foreign input is not a retained module leaf"
        );
    }
}
