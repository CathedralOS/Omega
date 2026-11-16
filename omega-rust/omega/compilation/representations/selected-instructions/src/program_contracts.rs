//! Facts object publication needs after instruction selection has finished.
//!
//! Preparation owns the ordered ABI rows. Selection shares that immutable
//! slice and the sealed storage geometry; later native forms retain this
//! contract chunk rather than reopening the prepared program. The metadata
//! commitment is derived here once, never supplied by a caller.

use std::sync::Arc;

use semantic_vocabulary::MachineId;
use target::TargetProfile;
use target_vocabulary::function_abi::LegalizedFunctionAbi;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedProgramContracts {
    pub borrowed_storage_layouts: runtime_abi::NativeBorrowedStorageLayouts,
    profile: Option<TargetProfile>,
    function_abis: Arc<[LegalizedFunctionAbi]>,
    publication: target_vocabulary::NativePublicationFacts,
    metadata_identity: [u8; 32],
}

impl SelectedProgramContracts {
    pub fn new(
        borrowed_storage_layouts: runtime_abi::NativeBorrowedStorageLayouts,
        profile: Option<TargetProfile>,
        function_abis: Arc<[LegalizedFunctionAbi]>,
        publication: target_vocabulary::NativePublicationFacts,
    ) -> Result<Self, &'static str> {
        if function_abis
            .windows(2)
            .any(|rows| rows[0].machine >= rows[1].machine)
        {
            return Err("function ABI rows are unordered or repeated");
        }
        if publication.functions().len() != function_abis.len()
            || publication
                .functions()
                .iter()
                .zip(function_abis.iter())
                .any(|(source, abi)| source.machine != abi.machine)
        {
            return Err("publication functions differ from the ABI roster");
        }
        let mut digest = crate::digest_encoder::DomainDigestEncoder::new(
            b"omega.selected-program-publication-metadata.v1\0",
        );
        match profile {
            None => digest.byte(0),
            Some(profile) => {
                digest.byte(1);
                let identity = profile.identity().as_str().as_bytes();
                digest.raw(&(identity.len() as u64).to_le_bytes());
                digest.raw(identity);
            }
        }
        digest.raw(&(function_abis.len() as u64).to_le_bytes());
        let mut row_bytes = Vec::new();
        for abi in function_abis.iter() {
            if abi.entry_scalar_codecs.len() != abi.call_plan.parameters.len() {
                return Err("function ABI entry conversions have another parameter count");
            }
            for (placement, codec) in abi
                .call_plan
                .parameters
                .iter()
                .zip(&abi.entry_scalar_codecs)
            {
                let valid = if placement.shape.class
                    == calling_conventions::ValueClass::BorrowedScalarReference
                {
                    codec.is_none_or(|word| runtime_abi::ScalarStorageCodec::decode(word).is_some())
                } else {
                    *codec == Some(0)
                };
                if !valid {
                    return Err("function ABI has an incompatible entry conversion");
                }
            }
            if abi.scalar_values().any(|value| {
                semantic_vocabulary::ClosedScalarType::try_from(value.scalar_type).is_err()
            }) {
                return Err("function ABI contains an unclosed scalar type");
            }
            row_bytes.clear();
            target_vocabulary::identity::function_abi::encode_function_abi(&mut row_bytes, abi);
            digest.raw(&row_bytes);
        }
        Ok(Self {
            borrowed_storage_layouts,
            profile,
            function_abis,
            publication,
            metadata_identity: digest.finish(),
        })
    }

    pub const fn profile(&self) -> Option<TargetProfile> {
        self.profile
    }

    pub fn function_abis(&self) -> &Arc<[LegalizedFunctionAbi]> {
        &self.function_abis
    }

    pub fn function_abi(&self, machine: MachineId) -> Option<&LegalizedFunctionAbi> {
        self.function_abis
            .binary_search_by_key(&machine, |row| row.machine)
            .ok()
            .map(|index| &self.function_abis[index])
    }

    pub const fn publication(&self) -> &target_vocabulary::NativePublicationFacts {
        &self.publication
    }

    /// Bind every publication field using only fixed-size commitments. Layout
    /// replacement remains visible without rehashing either roster's body.
    pub fn identity_bytes(&self) -> [u8; 32] {
        let mut digest = crate::digest_encoder::DomainDigestEncoder::new(
            b"omega.selected-program-contracts.v2\0",
        );
        digest.raw(&self.borrowed_storage_layouts.identity_bytes());
        digest.raw(&self.metadata_identity);
        digest.raw(&self.publication.identity_bytes());
        digest.finish()
    }
}

impl Default for SelectedProgramContracts {
    fn default() -> Self {
        Self::new(Default::default(), None, Arc::from([]), Default::default())
            .expect("the empty ABI and publication rosters are ordered")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use calling_conventions::{CallSignature, CallingPolicy, ValueShape, evaluate_call_plan};
    use semantic_vocabulary::{IntegerSign, IntegerType, ScalarType, ValueId, WidthParameter};
    use target_vocabulary::{ScalarAbiValue, ScalarFunctionAbi};

    fn abi() -> LegalizedFunctionAbi {
        let call_plan = evaluate_call_plan(
            CallingPolicy::MicrosoftX64,
            &CallSignature {
                parameters: vec![ValueShape::integer(8, 8)],
                result: None,
            },
        )
        .unwrap();
        let scalar_parameters = vec![ScalarAbiValue {
            value: ValueId::new(1).unwrap(),
            scalar_type: ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap()),
            placement: call_plan.parameters[0].clone(),
        }];
        LegalizedFunctionAbi {
            machine: MachineId::new(1).unwrap(),
            call_plan: call_plan.clone(),
            entry_scalar_codecs: vec![Some(0)],
            scalar_parameters: scalar_parameters.clone(),
            structural_parameters: Vec::new(),
            scalar: Some(ScalarFunctionAbi {
                call_plan,
                parameters: scalar_parameters,
                result: None,
            }),
            mixed_structural_scalar: None,
        }
    }

    fn publication(machine: MachineId) -> target_vocabulary::NativePublicationFacts {
        target_vocabulary::NativePublicationFacts::seal(vec![
            target_vocabulary::NativeFunctionPublication {
                machine,
                operation_count: 0,
                sites: Vec::new(),
                byte_literals: Vec::new(),
            },
        ])
        .unwrap()
    }

    #[test]
    fn publication_contracts_share_the_roster_and_bind_every_field() {
        let rows: Arc<[LegalizedFunctionAbi]> = Arc::from([abi()]);
        let original = SelectedProgramContracts::new(
            Default::default(),
            Some(TargetProfile::WindowsX64),
            Arc::clone(&rows),
            publication(rows[0].machine),
        )
        .unwrap();
        assert!(Arc::ptr_eq(original.function_abis(), &rows));
        assert_eq!(original.function_abi(rows[0].machine), Some(&rows[0]));
        let identity = original.identity_bytes();
        let profile = SelectedProgramContracts::new(
            Default::default(),
            Some(TargetProfile::UefiX64),
            Arc::clone(&rows),
            publication(rows[0].machine),
        )
        .unwrap();
        assert_ne!(identity, profile.identity_bytes());
        let mut changed = abi();
        changed.machine = MachineId::new(2).unwrap();
        let abi = SelectedProgramContracts::new(
            Default::default(),
            Some(TargetProfile::WindowsX64),
            Arc::from([changed]),
            publication(MachineId::new(2).unwrap()),
        )
        .unwrap();
        assert_ne!(identity, abi.identity_bytes());
        let mut geometry = original;
        geometry.borrowed_storage_layouts = runtime_abi::NativeBorrowedStorageLayouts::seal(vec![
            runtime_abi::NativeBorrowedStorageLayout::new(
                semantic_vocabulary::StructuralTypeId::new(1).unwrap(),
                8,
                8,
                [],
                [],
                [],
            ),
        ])
        .unwrap();
        assert_ne!(identity, geometry.identity_bytes());
    }

    #[test]
    fn publication_contracts_refuse_duplicate_and_unclosed_abi_rows() {
        assert_eq!(
            SelectedProgramContracts::new(
                Default::default(),
                None,
                Arc::from([abi(), abi()]),
                Default::default()
            )
            .unwrap_err(),
            "function ABI rows are unordered or repeated"
        );
        let mut unclosed = abi();
        unclosed.scalar_parameters[0].scalar_type =
            ScalarType::Integer(IntegerType::parameterized(
                IntegerSign::Unsigned,
                WidthParameter {
                    owner: unclosed.machine,
                    value: ValueId::new(2).unwrap(),
                },
            ));
        assert_eq!(
            SelectedProgramContracts::new(
                Default::default(),
                None,
                Arc::from([unclosed]),
                publication(MachineId::new(1).unwrap())
            )
            .unwrap_err(),
            "function ABI contains an unclosed scalar type"
        );
    }
}
