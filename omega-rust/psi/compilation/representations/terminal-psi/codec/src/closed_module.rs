//! Publication retains the exact judged module together with its closed scalar
//! descriptors and the bytes that established their identity. The encoder is
//! the complete traversal: it refuses a symbolic width where that width occurs,
//! without substituting a value or maintaining another list of module fields.

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
    fn a_symbolic_integer_inside_a_mathematical_proposition_is_refused() {
        let proposition = Proposition::IntegerMathLessOrEqual(
            IntegerMathTerm::MathValue {
                source_type: parameterized(),
                value: ValueId::new(2).unwrap(),
            },
            IntegerMathTerm::literal(IntegerValue::Unsigned(10)),
        );
        assert!(matches!(
            crate::canonical_proposition_order_key(&proposition),
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
