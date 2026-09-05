//! Exact service-schema permission meaning under one aggregate writer budget.

use super::{
    declarations::encode_type_parameter, encoder::Encoder,
    selected_providers::encode_service_method, values::identity::encode_nominal,
};
use crate::encoding::{
    PACKAGE_TERMINAL_PERMISSION_POLICY_VERSION, PackageReviewEncodingError,
    TERMINAL_PERMISSION_POLICY_MAGIC,
};
use crate::record::PackagePolicyTerminalPermissions;

impl PackagePolicyTerminalPermissions {
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, PackageReviewEncodingError> {
        self.validate_canonical_structure()
            .map_err(PackageReviewEncodingError::new)?;
        let mut encoder = Encoder::policy_bounded(4 * 1024 * 1024);
        encoder.fixed_bytes(TERMINAL_PERMISSION_POLICY_MAGIC);
        encoder.u16(PACKAGE_TERMINAL_PERMISSION_POLICY_VERSION);
        encoder.package_identity(self.package);
        encoder.string(self.target.identity().as_str())?;
        encoder.sequence(&self.services, |encoder, service| {
            encode_nominal(encoder, &service.service)?;
            encoder.sequence(&service.static_parameters, encode_type_parameter)?;
            encoder.u32(service.lifetime_parameter_count);
            encoder.sequence(&service.methods, encode_service_method)?;
            encoder.sequence(&service.permissions, |encoder, permission| {
                encode_nominal(encoder, &permission.requirement)?;
                encoder.sequence(permission.permitted.classes(), |encoder, class| {
                    encoder.byte(class.canonical_tag());
                    Ok(())
                })
            })
        })?;
        encoder.finish()
    }
}
