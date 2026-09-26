use crate::package_evidence::encoding::PackageReviewEncodingError;
use crate::package_evidence::encoding::encode::values::expressions::Encoder;
use crate::package_evidence::encoding::encode::values::identity::encode_nominal;
use crate::package_evidence::record::PackageReviewContractEvidenceArgument;
use crate::package_evidence::record::PackageReviewContractKind;

pub(super) fn encode_contract_evidence_argument(
    encoder: &mut Encoder,
    argument: &PackageReviewContractEvidenceArgument,
) -> Result<(), PackageReviewEncodingError> {
    encoder.field("lane_position", |encoder| {
        encoder.u32(argument.lane_position());
        Ok(())
    })?;
    for (field, term) in [
        ("source", argument.source()),
        ("parameter", argument.parameter()),
    ] {
        encoder.field(field, |encoder| {
            encoder.field("owner", |encoder| encode_nominal(encoder, term.owner()))?;
            encoder.field("kind", |encoder| {
                match term.kind() {
                    PackageReviewContractKind::Requires => encoder.tag("requires", 0),
                    PackageReviewContractKind::Ensures => encoder.tag("ensures", 1),
                };
                Ok(())
            })?;
            encoder.field("lane_position", |encoder| {
                encoder.u32(term.lane_position());
                Ok(())
            })?;
            Ok(())
        })?;
    }
    Ok(())
}
