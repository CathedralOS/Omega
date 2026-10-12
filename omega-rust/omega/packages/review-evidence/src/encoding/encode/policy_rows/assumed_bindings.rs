//! One bounded risk row for each exact assumed relationship. Reuse the lock's
//! existing row frame and budget rather than inventing an admission database.

use crate::encoding::PackageReviewEncodingError;
use crate::record::{
    PackagePolicyAssumedBindingContracts, PackagePolicyRow, PackagePolicyRowKind,
    PackagePolicyRowLimits, PackagePolicyRowUsage, PackageReviewAssumedBindingContract,
};

use super::{Builder, Encoder, rejected};

impl PackagePolicyAssumedBindingContracts {
    pub fn acceptance_rows_with_limits(
        &self,
        limits: PackagePolicyRowLimits,
    ) -> Result<(Vec<PackagePolicyRow>, PackagePolicyRowUsage), PackageReviewEncodingError> {
        let mut builder = Builder::new(
            self.package(),
            self.target(),
            self.relationships().len(),
            limits.bounded(),
            true,
        )?;
        project(&mut builder, self)?;
        builder.finish()
    }
}

pub(super) fn project(
    builder: &mut Builder,
    contracts: &PackagePolicyAssumedBindingContracts,
) -> Result<(), PackageReviewEncodingError> {
    for relationship in contracts.relationships() {
        builder.push(
            PackagePolicyRowKind::AssumedBindingContract,
            true,
            true,
            |encoder| {
                encoder.fixed_bytes(&relationship.relationship.digest().as_bytes());
                Ok(())
            },
            |encoder| encode_relationship(encoder, relationship),
        )?;
    }
    Ok(())
}

pub(in crate::encoding) fn encode_relationship(
    encoder: &mut Encoder,
    value: &PackageReviewAssumedBindingContract,
) -> Result<(), PackageReviewEncodingError> {
    encoder.field("risk", |encoder| {
        encoder.string("Extremely high risk - assumed binding contract")
    })?;
    encoder.field("runtime_only", |encoder| {
        encoder.boolean(true);
        Ok(())
    })?;
    encoder.field("relationship", |encoder| {
        encoder.fixed_bytes(&value.relationship.digest().as_bytes());
        Ok(())
    })?;
    // Exact component commitments remain inspectable. A signature-shaped
    // description cannot hide a changed application, body or dependency.
    let content = value.relationship.commitments();
    for (name, digest) in [
        (
            "required_declaration_revision",
            content.required.declaration_revision,
        ),
        ("required_application", content.required.application),
        ("required_public_contract", content.required.public_contract),
        (
            "supplied_declaration_revision",
            content.supplied.declaration_revision,
        ),
        ("supplied_application", content.supplied.application),
        ("supplied_public_contract", content.supplied.public_contract),
        ("root_scope_commitment", content.root_scope),
        ("subject_mapping", content.subject_mapping),
        ("selected_provider", *content.selected_provider.as_bytes()),
        (
            "selected_implementation_commitment",
            content.selected_implementation,
        ),
        ("semantic_dependencies", content.semantic_dependencies),
        ("calling_plan", content.calling_plan.as_bytes()),
        ("structural_compatibility", content.structural_compatibility),
        ("domain_authority", content.domain_authority),
        ("semantic_obligations", content.semantic_obligations),
    ] {
        encoder.field(name, |encoder| {
            encoder.fixed_bytes(&digest);
            Ok(())
        })?;
    }
    for (name, text) in [
        ("required_contract", value.required_contract.as_str()),
        ("supplied_contract", value.supplied_contract.as_str()),
        (
            "selected_implementation",
            value.selected_implementation.as_str(),
        ),
        (
            "assumption_difference",
            value.assumption_difference.as_str(),
        ),
        ("authority_introduced", value.authority_introduced.as_str()),
        ("root_scope", value.root_scope.as_str()),
        ("dependency_path", value.dependency_path.as_str()),
    ] {
        if text.is_empty() || text.contains('\0') {
            return Err(rejected(
                "assumed binding review requires complete readable risk meaning",
            ));
        }
        encoder.field(name, |encoder| encoder.string(text))?;
    }
    encoder.field("demonstrated_contradictions", |encoder| {
        encoder.sequence(
            &value.demonstrated_contradictions,
            |encoder, contradiction| {
                if contradiction.is_empty() || contradiction.contains('\0') {
                    return Err(rejected(
                        "assumed binding contradiction requires exact readable facts",
                    ));
                }
                encoder.string(contradiction)
            },
        )
    })
}
