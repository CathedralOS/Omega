use crate::{
    CompilerFunctionValidationEvidence, CompilerTextValidationEvidence,
    PlacedExecutableRegionInventory,
};
use psi_diagnostics::Diagnostic;

pub const FINAL_FOOTPRINT_CERTIFICATE_MARKER: &str = "omega.final-footprint-certificate.current";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum FinalFootprintClass {
    CompilerFunctions,
    CompilerFunctionRelocationEnvelope,
    CompilerFunctionCallReturnMechanics,
    CompilerFunctionInstructionEnumeration,
    CompilerFunctionBodySpecificationSubset,
    CatalogCheckedAssembly,
    ImportThunks,
    RelaxationProducts,
    Veneers,
    GeneratedStubs,
    CompilerFunctionBodyFootprintDecoding,
    AdmittedLeaves,
}

impl FinalFootprintClass {
    pub const fn name(self) -> &'static str {
        match self {
            Self::CompilerFunctions => "compiler_functions",
            Self::ImportThunks => "import_thunks",
            Self::RelaxationProducts => "relaxation_products",
            Self::Veneers => "veneers",
            Self::GeneratedStubs => "generated_stubs",
            Self::CompilerFunctionRelocationEnvelope => "compiler_function_relocation_envelope",
            Self::CompilerFunctionCallReturnMechanics => "compiler_function_call_return_mechanics",
            Self::CompilerFunctionInstructionEnumeration => {
                "compiler_function_instruction_enumeration"
            }
            Self::CompilerFunctionBodySpecificationSubset => {
                "compiler_function_body_specification_subset"
            }
            Self::CatalogCheckedAssembly => "catalog_checked_assembly",
            Self::CompilerFunctionBodyFootprintDecoding => {
                "compiler_function_body_footprint_decoding"
            }
            Self::AdmittedLeaves => "admitted_leaves",
        }
    }

    const fn tag(self) -> u8 {
        match self {
            Self::CompilerFunctions => 1,
            Self::CompilerFunctionRelocationEnvelope => 2,
            Self::CompilerFunctionCallReturnMechanics => 3,
            Self::CompilerFunctionInstructionEnumeration => 11,
            Self::CompilerFunctionBodySpecificationSubset => 12,
            Self::CatalogCheckedAssembly => 4,
            Self::ImportThunks => 5,
            Self::RelaxationProducts => 6,
            Self::Veneers => 7,
            Self::GeneratedStubs => 8,
            Self::CompilerFunctionBodyFootprintDecoding => 9,
            Self::AdmittedLeaves => 10,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FinalFootprintCoverage {
    pub enumeration_complete: bool,
    pub region_enumeration_complete: bool,
    pub footprint_enumeration_complete: bool,
    pub covered_classes: Vec<FinalFootprintClass>,
    pub absent_by_construction_classes: Vec<FinalFootprintClass>,
    pub final_byte_validated_classes: Vec<FinalFootprintClass>,
    pub missing_classes: Vec<FinalFootprintClass>,
}

impl FinalFootprintCoverage {
    pub fn current_partial() -> Self {
        Self {
            enumeration_complete: false,
            region_enumeration_complete: true,
            footprint_enumeration_complete: false,
            covered_classes: vec![
                FinalFootprintClass::CompilerFunctions,
                FinalFootprintClass::ImportThunks,
            ],
            absent_by_construction_classes: vec![
                FinalFootprintClass::RelaxationProducts,
                FinalFootprintClass::Veneers,
                FinalFootprintClass::GeneratedStubs,
                FinalFootprintClass::AdmittedLeaves,
            ],
            final_byte_validated_classes: vec![
                FinalFootprintClass::CompilerFunctionRelocationEnvelope,
                FinalFootprintClass::CompilerFunctionCallReturnMechanics,
                FinalFootprintClass::CompilerFunctionInstructionEnumeration,
                FinalFootprintClass::CompilerFunctionBodySpecificationSubset,
                FinalFootprintClass::CatalogCheckedAssembly,
                FinalFootprintClass::ImportThunks,
            ],
            missing_classes: vec![FinalFootprintClass::CompilerFunctionBodyFootprintDecoding],
        }
    }

    pub fn validate_normalized(&self) -> Result<(), Diagnostic> {
        for (name, classes) in [
            ("covered", &self.covered_classes),
            (
                "absent-by-construction",
                &self.absent_by_construction_classes,
            ),
            ("final-byte-validated", &self.final_byte_validated_classes),
            ("missing", &self.missing_classes),
        ] {
            if classes.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(Diagnostic::error(format!(
                    "final footprint certificate {name} classes are not strictly normalized"
                )));
            }
        }
        for (left_name, left, right_name, right) in [
            (
                "covered",
                &self.covered_classes,
                "absent-by-construction",
                &self.absent_by_construction_classes,
            ),
            (
                "covered",
                &self.covered_classes,
                "missing",
                &self.missing_classes,
            ),
            (
                "absent-by-construction",
                &self.absent_by_construction_classes,
                "missing",
                &self.missing_classes,
            ),
        ] {
            if let Some(class) = left.iter().find(|class| right.contains(class)) {
                return Err(Diagnostic::error(format!(
                    "final footprint class `{}` is both {left_name} and {right_name}",
                    class.name()
                )));
            }
        }
        if self.enumeration_complete
            != (self.region_enumeration_complete && self.footprint_enumeration_complete)
        {
            return Err(Diagnostic::error(
                "final footprint certificate completeness flags disagree",
            ));
        }
        if self.footprint_enumeration_complete && !self.missing_classes.is_empty() {
            return Err(Diagnostic::error(
                "complete final footprint enumeration cannot retain missing classes",
            ));
        }
        Ok(())
    }

    pub fn fingerprint(&self) -> u64 {
        let mut hash = FNV_OFFSET;
        fingerprint_bytes(
            &mut hash,
            &[
                u8::from(self.enumeration_complete),
                u8::from(self.region_enumeration_complete),
                u8::from(self.footprint_enumeration_complete),
            ],
        );
        for classes in [
            &self.covered_classes,
            &self.absent_by_construction_classes,
            &self.final_byte_validated_classes,
            &self.missing_classes,
        ] {
            fingerprint_bytes(&mut hash, &(classes.len() as u64).to_le_bytes());
            for class in classes {
                fingerprint_bytes(&mut hash, &[class.tag()]);
            }
        }
        hash
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FinalFootprintCertificate {
    pub marker: &'static str,
    pub certificate_fingerprint: u64,
    pub coverage_fingerprint: u64,
    pub coverage: FinalFootprintCoverage,
    pub boundary_contract_fingerprint: Option<u64>,
    pub implementation_evidence_fingerprint: u64,
    pub implementation_fragment_count: usize,
    pub compiler_text_validation: CompilerTextValidationEvidence,
    pub compiler_function_validation: CompilerFunctionValidationEvidence,
    pub inventory: PlacedExecutableRegionInventory,
    pub boundary_placement_binding_fingerprint: u64,
}

impl FinalFootprintCertificate {
    pub fn current_partial(
        boundary_contract_fingerprint: Option<u64>,
        implementation_evidence_fingerprint: u64,
        implementation_fragment_count: usize,
        compiler_text_validation: CompilerTextValidationEvidence,
        compiler_function_validation: CompilerFunctionValidationEvidence,
        inventory: PlacedExecutableRegionInventory,
    ) -> Result<Self, Diagnostic> {
        if !inventory.unclassified_gaps.is_empty() {
            return Err(Diagnostic::error(
                "region-complete final footprint certificate cannot retain executable gaps",
            ));
        }
        if compiler_function_validation.body_specification_instruction_count > 0
            && boundary_contract_fingerprint
                != Some(
                    compiler_function_validation.body_specification_boundary_contract_fingerprint,
                )
        {
            return Err(Diagnostic::error(
                "final body-specification footprint evidence names a different boundary contract",
            ));
        }
        if compiler_function_validation.fixed_mechanics_instruction_count > 0
            && boundary_contract_fingerprint
                != Some(compiler_function_validation.fixed_mechanics_boundary_contract_fingerprint)
        {
            return Err(Diagnostic::error(
                "final call-return footprint evidence names a different boundary contract",
            ));
        }
        let coverage = FinalFootprintCoverage::current_partial();
        coverage.validate_normalized()?;
        let coverage_fingerprint = coverage.fingerprint();
        let boundary_placement_binding_fingerprint = placement_binding_fingerprint(
            boundary_contract_fingerprint,
            implementation_evidence_fingerprint,
            compiler_text_validation.derivation_fingerprint,
            compiler_function_validation.evidence_fingerprint(),
            inventory.inventory_fingerprint,
        );
        let certificate_fingerprint = certificate_fingerprint(
            coverage_fingerprint,
            boundary_placement_binding_fingerprint,
            compiler_text_validation.derivation_fingerprint,
            compiler_function_validation.evidence_fingerprint(),
            inventory.inventory_fingerprint,
        );
        Ok(Self {
            marker: FINAL_FOOTPRINT_CERTIFICATE_MARKER,
            certificate_fingerprint,
            coverage_fingerprint,
            coverage,
            boundary_contract_fingerprint,
            implementation_evidence_fingerprint,
            implementation_fragment_count,
            compiler_text_validation,
            compiler_function_validation,
            inventory,
            boundary_placement_binding_fingerprint,
        })
    }

    pub fn validate_identity(&self) -> Result<(), Diagnostic> {
        if self.marker != FINAL_FOOTPRINT_CERTIFICATE_MARKER {
            return Err(Diagnostic::error(
                "unsupported final footprint certificate marker",
            ));
        }
        self.coverage.validate_normalized()?;
        if self
            .compiler_function_validation
            .body_specification_instruction_count
            > 0
            && self.boundary_contract_fingerprint
                != Some(
                    self.compiler_function_validation
                        .body_specification_boundary_contract_fingerprint,
                )
        {
            return Err(Diagnostic::error(
                "final body-specification footprint evidence names a different boundary contract",
            ));
        }
        if self
            .compiler_function_validation
            .fixed_mechanics_instruction_count
            > 0
            && self.boundary_contract_fingerprint
                != Some(
                    self.compiler_function_validation
                        .fixed_mechanics_boundary_contract_fingerprint,
                )
        {
            return Err(Diagnostic::error(
                "final call-return footprint evidence names a different boundary contract",
            ));
        }
        if self.coverage.region_enumeration_complete && !self.inventory.unclassified_gaps.is_empty()
        {
            return Err(Diagnostic::error(
                "region-complete final footprint certificate retains executable gaps",
            ));
        }
        let expected_coverage = self.coverage.fingerprint();
        if self.coverage_fingerprint != expected_coverage {
            return Err(Diagnostic::error(
                "final footprint certificate coverage fingerprint mismatch",
            ));
        }
        let expected_binding = placement_binding_fingerprint(
            self.boundary_contract_fingerprint,
            self.implementation_evidence_fingerprint,
            self.compiler_text_validation.derivation_fingerprint,
            self.compiler_function_validation.evidence_fingerprint(),
            self.inventory.inventory_fingerprint,
        );
        if self.boundary_placement_binding_fingerprint != expected_binding {
            return Err(Diagnostic::error(
                "final footprint certificate placement binding mismatch",
            ));
        }
        let expected_certificate = certificate_fingerprint(
            expected_coverage,
            expected_binding,
            self.compiler_text_validation.derivation_fingerprint,
            self.compiler_function_validation.evidence_fingerprint(),
            self.inventory.inventory_fingerprint,
        );
        if self.certificate_fingerprint != expected_certificate {
            return Err(Diagnostic::error(
                "final footprint certificate identity mismatch",
            ));
        }
        Ok(())
    }
}

fn placement_binding_fingerprint(
    boundary_contract_fingerprint: Option<u64>,
    implementation_evidence_fingerprint: u64,
    compiler_text_derivation_fingerprint: u64,
    compiler_function_validation_fingerprint: u64,
    inventory_fingerprint: u64,
) -> u64 {
    let mut hash = FNV_OFFSET;
    fingerprint_bytes(
        &mut hash,
        &[
            u8::from(boundary_contract_fingerprint.is_some()),
            0x42,
            0x50,
            0x42,
        ],
    );
    fingerprint_bytes(
        &mut hash,
        &boundary_contract_fingerprint
            .unwrap_or_default()
            .to_le_bytes(),
    );
    fingerprint_bytes(
        &mut hash,
        &implementation_evidence_fingerprint.to_le_bytes(),
    );
    fingerprint_bytes(
        &mut hash,
        &compiler_text_derivation_fingerprint.to_le_bytes(),
    );
    fingerprint_bytes(
        &mut hash,
        &compiler_function_validation_fingerprint.to_le_bytes(),
    );
    fingerprint_bytes(&mut hash, &inventory_fingerprint.to_le_bytes());
    hash
}

fn certificate_fingerprint(
    coverage_fingerprint: u64,
    boundary_placement_binding_fingerprint: u64,
    compiler_text_derivation_fingerprint: u64,
    compiler_function_validation_fingerprint: u64,
    inventory_fingerprint: u64,
) -> u64 {
    let mut hash = FNV_OFFSET;
    fingerprint_bytes(&mut hash, FINAL_FOOTPRINT_CERTIFICATE_MARKER.as_bytes());
    fingerprint_bytes(&mut hash, &coverage_fingerprint.to_le_bytes());
    fingerprint_bytes(
        &mut hash,
        &boundary_placement_binding_fingerprint.to_le_bytes(),
    );
    fingerprint_bytes(
        &mut hash,
        &compiler_text_derivation_fingerprint.to_le_bytes(),
    );
    fingerprint_bytes(
        &mut hash,
        &compiler_function_validation_fingerprint.to_le_bytes(),
    );
    fingerprint_bytes(&mut hash, &inventory_fingerprint.to_le_bytes());
    hash
}

const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;

fn fingerprint_bytes(hash: &mut u64, bytes: &[u8]) {
    for byte in bytes {
        *hash ^= u64::from(*byte);
        *hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn certificate() -> FinalFootprintCertificate {
        FinalFootprintCertificate::current_partial(
            Some(1),
            2,
            3,
            CompilerTextValidationEvidence {
                encoded_text_fingerprint: 4,
                final_compiler_text_fingerprint: 5,
                relocation_envelope_fingerprint: 6,
                checked_instruction_validation_fingerprint: 7,
                derivation_fingerprint: 8,
                text_relocation_count: 9,
                checked_instruction_validation_count: 10,
            },
            CompilerFunctionValidationEvidence {
                function_count: 1,
                instruction_count: 2,
                zero_width_instruction_count: 0,
                checked_assembly_instruction_count: 0,
                fixed_mechanics_instruction_count: 2,
                fixed_mechanics_validation_fingerprint: 14,
                fixed_mechanics_boundary_contract_fingerprint: 1,
                fixed_mechanics_footprint_fingerprint: 17,
                body_specification_instruction_count: 3,
                body_specification_validation_fingerprint: 15,
                body_specification_boundary_contract_fingerprint: 1,
                body_specification_footprint_fingerprint: 16,
                composed_footprint_fingerprint: 18,
                validation_fingerprint: 11,
            },
            PlacedExecutableRegionInventory {
                text_address: 0x1000,
                text_byte_count: 4,
                text_fingerprint: 12,
                inventory_fingerprint: 13,
                regions: Vec::new(),
                unclassified_gaps: Vec::new(),
            },
        )
        .expect("partial certificate")
    }

    #[test]
    fn partial_certificate_binds_coverage_placement_text_and_inventory() {
        let certificate = certificate();
        certificate.validate_identity().expect("valid identity");

        for drifted in [
            {
                let mut value = certificate.clone();
                value.marker = "omega.final-footprint-certificate.stale";
                value
            },
            {
                let mut value = certificate.clone();
                value.coverage.missing_classes.pop();
                value
            },
            {
                let mut value = certificate.clone();
                value.boundary_contract_fingerprint = Some(99);
                value
            },
            {
                let mut value = certificate.clone();
                value.compiler_text_validation.derivation_fingerprint = 99;
                value
            },
            {
                let mut value = certificate.clone();
                value.compiler_function_validation.validation_fingerprint = 99;
                value
            },
            {
                let mut value = certificate.clone();
                value.compiler_function_validation.instruction_count = 99;
                value
            },
            {
                let mut value = certificate.clone();
                value.inventory.inventory_fingerprint = 99;
                value
            },
        ] {
            assert!(drifted.validate_identity().is_err());
        }
    }

    #[test]
    fn partial_coverage_classifies_admitted_leaves_as_absent() {
        let coverage = FinalFootprintCoverage::current_partial();
        assert!(
            coverage
                .absent_by_construction_classes
                .contains(&FinalFootprintClass::AdmittedLeaves)
        );
        assert!(
            !coverage
                .missing_classes
                .contains(&FinalFootprintClass::AdmittedLeaves)
        );
        coverage.validate_normalized().expect("normalized coverage");
    }

    #[test]
    fn coverage_rejects_conflicting_class_statuses() {
        let mut coverage = FinalFootprintCoverage::current_partial();
        coverage
            .missing_classes
            .push(FinalFootprintClass::AdmittedLeaves);
        assert!(
            coverage
                .validate_normalized()
                .expect_err("one class cannot be both absent and missing")
                .message
                .contains("both absent-by-construction and missing")
        );
    }

    #[test]
    fn region_complete_certificate_rejects_gaps() {
        let mut inventory = certificate().inventory;
        inventory
            .unclassified_gaps
            .push(crate::PlacedExecutableGap {
                section_offset: 0,
                address: 0x1000,
                byte_count: 1,
                byte_fingerprint: 1,
            });
        assert!(
            FinalFootprintCertificate::current_partial(
                None,
                0,
                0,
                CompilerTextValidationEvidence {
                    encoded_text_fingerprint: 0,
                    final_compiler_text_fingerprint: 0,
                    relocation_envelope_fingerprint: 0,
                    checked_instruction_validation_fingerprint: 0,
                    derivation_fingerprint: 0,
                    text_relocation_count: 0,
                    checked_instruction_validation_count: 0,
                },
                CompilerFunctionValidationEvidence {
                    function_count: 0,
                    instruction_count: 0,
                    zero_width_instruction_count: 0,
                    checked_assembly_instruction_count: 0,
                    fixed_mechanics_instruction_count: 0,
                    fixed_mechanics_validation_fingerprint: 0,
                    fixed_mechanics_boundary_contract_fingerprint: 0,
                    fixed_mechanics_footprint_fingerprint: 0,
                    body_specification_instruction_count: 0,
                    body_specification_validation_fingerprint: 0,
                    body_specification_boundary_contract_fingerprint: 0,
                    body_specification_footprint_fingerprint: 0,
                    composed_footprint_fingerprint: 0,
                    validation_fingerprint: 0,
                },
                inventory,
            )
            .is_err()
        );
    }
}
