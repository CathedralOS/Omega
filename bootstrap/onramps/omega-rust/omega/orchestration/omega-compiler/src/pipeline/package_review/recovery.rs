use super::encoding::ROW_MAGIC;
use super::*;
use omega_target::TargetProfile;
use psi_core::PackageKeyIdentity;

const RECOVERY_MAGIC: &[u8] = b"OMEGA-PACKAGE-REVIEW-ROW-RECOVERY\0";

/// Version of the compiler-owned canonical-row recovery envelope.
pub const PACKAGE_REVIEW_CANONICAL_ROW_RECOVERY_VERSION: u16 = 1;

/// Resource ceilings applied while encoding or decoding one canonical-row
/// recovery envelope.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PackageReviewCanonicalRowRecoveryLimits {
    maximum_recovery_bytes: usize,
    maximum_canonical_row_bytes: usize,
    maximum_target_bytes: usize,
    maximum_row_key_bytes: usize,
    maximum_row_value_bytes: usize,
    maximum_source_locations: usize,
    maximum_source_path_bytes: usize,
    maximum_total_source_path_bytes: usize,
    maximum_compiler_derivations: usize,
}

impl PackageReviewCanonicalRowRecoveryLimits {
    #[allow(clippy::too_many_arguments)]
    pub const fn new(
        maximum_recovery_bytes: usize,
        maximum_canonical_row_bytes: usize,
        maximum_target_bytes: usize,
        maximum_row_key_bytes: usize,
        maximum_row_value_bytes: usize,
        maximum_source_locations: usize,
        maximum_source_path_bytes: usize,
        maximum_total_source_path_bytes: usize,
        maximum_compiler_derivations: usize,
    ) -> Self {
        Self {
            maximum_recovery_bytes,
            maximum_canonical_row_bytes,
            maximum_target_bytes,
            maximum_row_key_bytes,
            maximum_row_value_bytes,
            maximum_source_locations,
            maximum_source_path_bytes,
            maximum_total_source_path_bytes,
            maximum_compiler_derivations,
        }
    }
}

impl Default for PackageReviewCanonicalRowRecoveryLimits {
    fn default() -> Self {
        Self::new(
            64 * 1024 * 1024,
            4 * 1024 * 1024,
            4 * 1024,
            1024 * 1024,
            4 * 1024 * 1024,
            262_144,
            1024 * 1024,
            16 * 1024 * 1024,
            4,
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageReviewCanonicalRowRecoveryError {
    message: &'static str,
}

impl PackageReviewCanonicalRowRecoveryError {
    const fn new(message: &'static str) -> Self {
        Self { message }
    }

    pub const fn message(&self) -> &'static str {
        self.message
    }
}

impl std::fmt::Display for PackageReviewCanonicalRowRecoveryError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.message)
    }
}

impl std::error::Error for PackageReviewCanonicalRowRecoveryError {}

/// A review-only row envelope decoded by the compiler together with the
/// package and target parsed from its canonical outer frame.
///
/// The row payload remains opaque: decoding establishes canonical framing and
/// source-sidecar shape, not semantic re-issuance. This is restart metadata for
/// review, not compiler-issued package admission evidence.
#[derive(Debug, Clone)]
pub struct DecodedPackageReviewCanonicalRow {
    package: PackageKeyIdentity,
    target: TargetProfile,
    row: PackageReviewCanonicalRow,
}

impl DecodedPackageReviewCanonicalRow {
    pub const fn package(&self) -> PackageKeyIdentity {
        self.package
    }

    pub const fn target(&self) -> TargetProfile {
        self.target
    }

    pub const fn kind(&self) -> PackageReviewCanonicalRowKind {
        self.row.kind
    }

    pub const fn risk(&self) -> PackageReviewCanonicalRowRisk {
        self.row.risk
    }

    pub fn key_bytes(&self) -> &[u8] {
        &self.row.key_bytes
    }

    pub fn canonical_bytes(&self) -> &[u8] {
        &self.row.canonical_bytes
    }

    pub const fn source(&self) -> &PackageReviewCanonicalRowSource {
        &self.row.source
    }
}

pub fn encode_package_review_canonical_row(
    row: &PackageReviewCanonicalRow,
) -> Result<Vec<u8>, PackageReviewCanonicalRowRecoveryError> {
    encode_package_review_canonical_row_with_limits(
        row,
        PackageReviewCanonicalRowRecoveryLimits::default(),
    )
}

pub fn encode_package_review_canonical_row_with_limits(
    row: &PackageReviewCanonicalRow,
    limits: PackageReviewCanonicalRowRecoveryLimits,
) -> Result<Vec<u8>, PackageReviewCanonicalRowRecoveryError> {
    let framing = parse_canonical_row(&row.canonical_bytes, limits)?;
    if row.kind != framing.kind || row.risk != framing.risk || row.key_bytes != framing.key_bytes {
        return Err(PackageReviewCanonicalRowRecoveryError::new(
            "canonical package-review row fields disagree with its canonical outer frame",
        ));
    }
    validate_source(&row.source, limits)?;

    let mut encoder = RecoveryEncoder::bounded(limits.maximum_recovery_bytes);
    encoder.fixed_bytes(RECOVERY_MAGIC);
    encoder.u16(PACKAGE_REVIEW_CANONICAL_ROW_RECOVERY_VERSION);
    encoder.bytes(&row.canonical_bytes)?;
    encoder.usize(row.source.authored_locations.len())?;
    for location in &row.source.authored_locations {
        encode_location(&mut encoder, location)?;
    }
    encoder.usize(row.source.compiler_derivations.len())?;
    for derivation in &row.source.compiler_derivations {
        encoder.byte(synthetic_source_tag(*derivation));
    }
    encoder.finish()
}

pub fn decode_package_review_canonical_row(
    bytes: &[u8],
) -> Result<DecodedPackageReviewCanonicalRow, PackageReviewCanonicalRowRecoveryError> {
    decode_package_review_canonical_row_with_limits(
        bytes,
        PackageReviewCanonicalRowRecoveryLimits::default(),
    )
}

pub fn decode_package_review_canonical_row_with_limits(
    bytes: &[u8],
    limits: PackageReviewCanonicalRowRecoveryLimits,
) -> Result<DecodedPackageReviewCanonicalRow, PackageReviewCanonicalRowRecoveryError> {
    if bytes.len() > limits.maximum_recovery_bytes {
        return Err(PackageReviewCanonicalRowRecoveryError::new(
            "canonical-row recovery envelope exceeds its byte ceiling",
        ));
    }
    let mut decoder = RecoveryDecoder::new(bytes);
    decoder.fixed_bytes(RECOVERY_MAGIC)?;
    if decoder.u16()? != PACKAGE_REVIEW_CANONICAL_ROW_RECOVERY_VERSION {
        return Err(PackageReviewCanonicalRowRecoveryError::new(
            "unsupported canonical-row recovery envelope version",
        ));
    }
    let canonical_bytes = clone_bytes(
        decoder.bytes(limits.maximum_canonical_row_bytes)?,
        "canonical package-review row allocation failed",
    )?;
    let framing = parse_canonical_row(&canonical_bytes, limits)?;

    let location_count = decoder.usize()?;
    if location_count > limits.maximum_source_locations {
        return Err(PackageReviewCanonicalRowRecoveryError::new(
            "canonical-row recovery source-location count exceeds its ceiling",
        ));
    }
    let mut authored_locations = Vec::new();
    authored_locations
        .try_reserve(location_count)
        .map_err(|_| {
            PackageReviewCanonicalRowRecoveryError::new(
                "canonical-row recovery source-location allocation failed",
            )
        })?;
    let mut total_path_bytes = 0usize;
    for _ in 0..location_count {
        authored_locations.push(decode_location(
            &mut decoder,
            limits,
            &mut total_path_bytes,
        )?);
    }

    let derivation_count = decoder.usize()?;
    if derivation_count > limits.maximum_compiler_derivations {
        return Err(PackageReviewCanonicalRowRecoveryError::new(
            "canonical-row recovery compiler-derivation count exceeds its ceiling",
        ));
    }
    let mut compiler_derivations = Vec::new();
    compiler_derivations
        .try_reserve(derivation_count)
        .map_err(|_| {
            PackageReviewCanonicalRowRecoveryError::new(
                "canonical-row recovery compiler-derivation allocation failed",
            )
        })?;
    for _ in 0..derivation_count {
        compiler_derivations.push(decode_synthetic_source(decoder.byte()?)?);
    }
    decoder.finish()?;

    let source = PackageReviewCanonicalRowSource::mixed(authored_locations, compiler_derivations);
    validate_source(&source, limits)?;
    let decoded = DecodedPackageReviewCanonicalRow {
        package: framing.package,
        target: framing.target,
        row: PackageReviewCanonicalRow {
            kind: framing.kind,
            risk: framing.risk,
            key_bytes: framing.key_bytes,
            canonical_bytes,
            source,
        },
    };
    if encode_package_review_canonical_row_with_limits(&decoded.row, limits)? != bytes {
        return Err(PackageReviewCanonicalRowRecoveryError::new(
            "canonical-row recovery input is not in canonical form",
        ));
    }
    Ok(decoded)
}

struct ParsedCanonicalRow {
    package: PackageKeyIdentity,
    target: TargetProfile,
    kind: PackageReviewCanonicalRowKind,
    risk: PackageReviewCanonicalRowRisk,
    key_bytes: Vec<u8>,
}

fn parse_canonical_row(
    bytes: &[u8],
    limits: PackageReviewCanonicalRowRecoveryLimits,
) -> Result<ParsedCanonicalRow, PackageReviewCanonicalRowRecoveryError> {
    if bytes.len() > limits.maximum_canonical_row_bytes {
        return Err(PackageReviewCanonicalRowRecoveryError::new(
            "canonical package-review row exceeds its byte ceiling",
        ));
    }
    let mut decoder = RecoveryDecoder::new(bytes);
    decoder.fixed_bytes(ROW_MAGIC)?;
    if decoder.u16()? != PACKAGE_REVIEW_ROW_ENCODING_VERSION {
        return Err(PackageReviewCanonicalRowRecoveryError::new(
            "unsupported canonical package-review row version",
        ));
    }
    if decoder.u16()? != PACKAGE_REVIEW_ENCODING_VERSION {
        return Err(PackageReviewCanonicalRowRecoveryError::new(
            "canonical package-review row names an unsupported review schema",
        ));
    }
    let package_digest = decoder.array_32()?;
    let package = PackageKeyIdentity::from_digest(package_digest).ok_or_else(|| {
        PackageReviewCanonicalRowRecoveryError::new(
            "canonical package-review row contains an invalid package identity",
        )
    })?;
    let target_name = decoder.string(limits.maximum_target_bytes)?;
    let target = TargetProfile::from_omega_target_name(Some(target_name)).map_err(|_| {
        PackageReviewCanonicalRowRecoveryError::new(
            "canonical package-review row contains a noncanonical target",
        )
    })?;
    if target.target_name() != target_name {
        return Err(PackageReviewCanonicalRowRecoveryError::new(
            "canonical package-review row contains a noncanonical target",
        ));
    }
    let kind = decode_kind(decoder.byte()?)?;
    let risk = decode_risk(decoder.byte()?)?;
    if risk != canonical_risk(kind) {
        return Err(PackageReviewCanonicalRowRecoveryError::new(
            "canonical package-review row contains a noncanonical risk for its kind",
        ));
    }
    let key_bytes = clone_bytes(
        decoder.bytes(limits.maximum_row_key_bytes)?,
        "canonical package-review row key allocation failed",
    )?;
    let _value_bytes = decoder.bytes(limits.maximum_row_value_bytes)?;
    decoder.finish()?;
    Ok(ParsedCanonicalRow {
        package,
        target,
        kind,
        risk,
        key_bytes,
    })
}

fn encode_location(
    encoder: &mut RecoveryEncoder,
    location: &PackageReviewSourceLocation,
) -> Result<(), PackageReviewCanonicalRowRecoveryError> {
    match location.owner {
        PackageReviewSourceLocationOwner::Package(package) => {
            encoder.byte(0);
            encoder.fixed_bytes(&package.digest());
        }
        PackageReviewSourceLocationOwner::Toolchain(source) => {
            encoder.byte(1);
            encoder.fixed_bytes(&source.digest());
        }
    }
    encoder.string(&location.relative_path)?;
    encoder.u64(location.start_byte);
    encoder.u64(location.end_byte);
    encoder.byte(source_location_role_tag(location.role));
    Ok(())
}

fn decode_location(
    decoder: &mut RecoveryDecoder<'_>,
    limits: PackageReviewCanonicalRowRecoveryLimits,
    total_path_bytes: &mut usize,
) -> Result<PackageReviewSourceLocation, PackageReviewCanonicalRowRecoveryError> {
    let owner_tag = decoder.byte()?;
    let digest = decoder.array_32()?;
    let owner = match owner_tag {
        0 => PackageReviewSourceLocationOwner::Package(
            PackageKeyIdentity::from_digest(digest).ok_or_else(|| {
                PackageReviewCanonicalRowRecoveryError::new(
                    "canonical-row recovery source contains an invalid package owner",
                )
            })?,
        ),
        1 => PackageReviewSourceLocationOwner::Toolchain(PackageReviewToolchainSourceIdentity {
            digest,
        }),
        _ => {
            return Err(PackageReviewCanonicalRowRecoveryError::new(
                "canonical-row recovery source contains an unknown owner tag",
            ));
        }
    };
    let relative_path = clone_string(
        decoder.string(limits.maximum_source_path_bytes)?,
        "canonical-row recovery source-path allocation failed",
    )?;
    *total_path_bytes = total_path_bytes
        .checked_add(relative_path.len())
        .ok_or_else(|| {
            PackageReviewCanonicalRowRecoveryError::new(
                "canonical-row recovery source-path byte count overflow",
            )
        })?;
    if *total_path_bytes > limits.maximum_total_source_path_bytes {
        return Err(PackageReviewCanonicalRowRecoveryError::new(
            "canonical-row recovery source paths exceed their total byte ceiling",
        ));
    }
    let start_byte = decoder.u64()?;
    let end_byte = decoder.u64()?;
    let role = decode_source_location_role(decoder.byte()?)?;
    Ok(PackageReviewSourceLocation {
        owner,
        relative_path,
        start_byte,
        end_byte,
        role,
    })
}

fn validate_source(
    source: &PackageReviewCanonicalRowSource,
    limits: PackageReviewCanonicalRowRecoveryLimits,
) -> Result<(), PackageReviewCanonicalRowRecoveryError> {
    if source.authored_locations.is_empty() && source.compiler_derivations.is_empty() {
        return Err(PackageReviewCanonicalRowRecoveryError::new(
            "canonical-row recovery source has no authored location or compiler derivation",
        ));
    }
    if source.authored_locations.len() > limits.maximum_source_locations {
        return Err(PackageReviewCanonicalRowRecoveryError::new(
            "canonical-row recovery source-location count exceeds its ceiling",
        ));
    }
    if source.compiler_derivations.len() > limits.maximum_compiler_derivations {
        return Err(PackageReviewCanonicalRowRecoveryError::new(
            "canonical-row recovery compiler-derivation count exceeds its ceiling",
        ));
    }
    if source
        .authored_locations
        .windows(2)
        .any(|pair| pair[0] >= pair[1])
    {
        return Err(PackageReviewCanonicalRowRecoveryError::new(
            "canonical-row recovery source locations are not strictly ordered",
        ));
    }
    if source
        .compiler_derivations
        .windows(2)
        .any(|pair| pair[0] >= pair[1])
    {
        return Err(PackageReviewCanonicalRowRecoveryError::new(
            "canonical-row recovery compiler derivations are not strictly ordered",
        ));
    }
    let mut total_path_bytes = 0usize;
    for location in &source.authored_locations {
        validate_location(location, limits)?;
        total_path_bytes = total_path_bytes
            .checked_add(location.relative_path.len())
            .ok_or_else(|| {
                PackageReviewCanonicalRowRecoveryError::new(
                    "canonical-row recovery source-path byte count overflow",
                )
            })?;
    }
    if total_path_bytes > limits.maximum_total_source_path_bytes {
        return Err(PackageReviewCanonicalRowRecoveryError::new(
            "canonical-row recovery source paths exceed their total byte ceiling",
        ));
    }
    Ok(())
}

fn validate_location(
    location: &PackageReviewSourceLocation,
    limits: PackageReviewCanonicalRowRecoveryLimits,
) -> Result<(), PackageReviewCanonicalRowRecoveryError> {
    if location.relative_path.is_empty()
        || location.relative_path.len() > limits.maximum_source_path_bytes
        || location.relative_path.starts_with('/')
        || location
            .relative_path
            .split('/')
            .any(|component| component.is_empty() || component == "." || component == "..")
    {
        return Err(PackageReviewCanonicalRowRecoveryError::new(
            "canonical-row recovery source contains a noncanonical relative path",
        ));
    }
    if location.start_byte >= location.end_byte {
        return Err(PackageReviewCanonicalRowRecoveryError::new(
            "canonical-row recovery source contains an invalid byte span",
        ));
    }
    Ok(())
}

const fn canonical_risk(kind: PackageReviewCanonicalRowKind) -> PackageReviewCanonicalRowRisk {
    match kind {
        PackageReviewCanonicalRowKind::RepresentationTcb
        | PackageReviewCanonicalRowKind::DangerousAuthoritySlack => {
            PackageReviewCanonicalRowRisk::AuditRecommended
        }
        PackageReviewCanonicalRowKind::SelectedProviderSet => {
            PackageReviewCanonicalRowRisk::OpaqueBlocking
        }
        PackageReviewCanonicalRowKind::ProjectionHeader
        | PackageReviewCanonicalRowKind::PublicTrait
        | PackageReviewCanonicalRowKind::PublicDomain
        | PackageReviewCanonicalRowKind::PublicData
        | PackageReviewCanonicalRowKind::Callable
        | PackageReviewCanonicalRowKind::DangerousAuthority
        | PackageReviewCanonicalRowKind::AcceptedClaim => PackageReviewCanonicalRowRisk::Blocking,
    }
}

fn decode_kind(
    tag: u8,
) -> Result<PackageReviewCanonicalRowKind, PackageReviewCanonicalRowRecoveryError> {
    match tag {
        0 => Ok(PackageReviewCanonicalRowKind::ProjectionHeader),
        1 => Ok(PackageReviewCanonicalRowKind::PublicTrait),
        2 => Ok(PackageReviewCanonicalRowKind::PublicDomain),
        3 => Ok(PackageReviewCanonicalRowKind::PublicData),
        4 => Ok(PackageReviewCanonicalRowKind::RepresentationTcb),
        5 => Ok(PackageReviewCanonicalRowKind::Callable),
        6 => Ok(PackageReviewCanonicalRowKind::DangerousAuthority),
        7 => Ok(PackageReviewCanonicalRowKind::SelectedProviderSet),
        8 => Ok(PackageReviewCanonicalRowKind::AcceptedClaim),
        9 => Ok(PackageReviewCanonicalRowKind::DangerousAuthoritySlack),
        _ => Err(PackageReviewCanonicalRowRecoveryError::new(
            "canonical package-review row contains an unknown kind tag",
        )),
    }
}

fn decode_risk(
    tag: u8,
) -> Result<PackageReviewCanonicalRowRisk, PackageReviewCanonicalRowRecoveryError> {
    match tag {
        0 => Ok(PackageReviewCanonicalRowRisk::Blocking),
        1 => Ok(PackageReviewCanonicalRowRisk::AuditRecommended),
        2 => Ok(PackageReviewCanonicalRowRisk::OpaqueBlocking),
        _ => Err(PackageReviewCanonicalRowRecoveryError::new(
            "canonical package-review row contains an unknown risk tag",
        )),
    }
}

const fn source_location_role_tag(role: PackageReviewSourceLocationRole) -> u8 {
    match role {
        PackageReviewSourceLocationRole::Declaration => 0,
        PackageReviewSourceLocationRole::DerivationOrigin => 1,
        PackageReviewSourceLocationRole::AuthorityDeclaration => 2,
        PackageReviewSourceLocationRole::AuthorityExposure => 3,
        PackageReviewSourceLocationRole::ProviderSelection => 4,
        PackageReviewSourceLocationRole::ProviderSchemaDeclaration => 5,
        PackageReviewSourceLocationRole::ProviderTypeDeclaration => 6,
        PackageReviewSourceLocationRole::ProviderRealization => 7,
    }
}

fn decode_source_location_role(
    tag: u8,
) -> Result<PackageReviewSourceLocationRole, PackageReviewCanonicalRowRecoveryError> {
    match tag {
        0 => Ok(PackageReviewSourceLocationRole::Declaration),
        1 => Ok(PackageReviewSourceLocationRole::DerivationOrigin),
        2 => Ok(PackageReviewSourceLocationRole::AuthorityDeclaration),
        3 => Ok(PackageReviewSourceLocationRole::AuthorityExposure),
        4 => Ok(PackageReviewSourceLocationRole::ProviderSelection),
        5 => Ok(PackageReviewSourceLocationRole::ProviderSchemaDeclaration),
        6 => Ok(PackageReviewSourceLocationRole::ProviderTypeDeclaration),
        7 => Ok(PackageReviewSourceLocationRole::ProviderRealization),
        _ => Err(PackageReviewCanonicalRowRecoveryError::new(
            "canonical-row recovery source contains an unknown role tag",
        )),
    }
}

const fn synthetic_source_tag(kind: PackageReviewSyntheticSourceKind) -> u8 {
    match kind {
        PackageReviewSyntheticSourceKind::ProjectionHeader => 0,
        PackageReviewSyntheticSourceKind::EmptySelectedProviderSet => 1,
        PackageReviewSyntheticSourceKind::UniqueCoveringProviderSelection => 2,
        PackageReviewSyntheticSourceKind::FreeExternalProviderType => 3,
    }
}

fn decode_synthetic_source(
    tag: u8,
) -> Result<PackageReviewSyntheticSourceKind, PackageReviewCanonicalRowRecoveryError> {
    match tag {
        0 => Ok(PackageReviewSyntheticSourceKind::ProjectionHeader),
        1 => Ok(PackageReviewSyntheticSourceKind::EmptySelectedProviderSet),
        2 => Ok(PackageReviewSyntheticSourceKind::UniqueCoveringProviderSelection),
        3 => Ok(PackageReviewSyntheticSourceKind::FreeExternalProviderType),
        _ => Err(PackageReviewCanonicalRowRecoveryError::new(
            "canonical-row recovery source contains an unknown compiler-derivation tag",
        )),
    }
}

fn clone_bytes(
    bytes: &[u8],
    allocation_error: &'static str,
) -> Result<Vec<u8>, PackageReviewCanonicalRowRecoveryError> {
    let mut output = Vec::new();
    output
        .try_reserve(bytes.len())
        .map_err(|_| PackageReviewCanonicalRowRecoveryError::new(allocation_error))?;
    output.extend_from_slice(bytes);
    Ok(output)
}

fn clone_string(
    value: &str,
    allocation_error: &'static str,
) -> Result<String, PackageReviewCanonicalRowRecoveryError> {
    let mut output = String::new();
    output
        .try_reserve(value.len())
        .map_err(|_| PackageReviewCanonicalRowRecoveryError::new(allocation_error))?;
    output.push_str(value);
    Ok(output)
}

struct RecoveryEncoder {
    output: Vec<u8>,
    maximum_bytes: usize,
    exceeded: bool,
}

impl RecoveryEncoder {
    fn bounded(maximum_bytes: usize) -> Self {
        Self {
            output: Vec::new(),
            maximum_bytes,
            exceeded: false,
        }
    }

    fn append(&mut self, bytes: &[u8]) {
        if self.exceeded {
            return;
        }
        let Some(required) = self.output.len().checked_add(bytes.len()) else {
            self.exceeded = true;
            return;
        };
        if required > self.maximum_bytes || self.output.try_reserve(bytes.len()).is_err() {
            self.exceeded = true;
            return;
        }
        self.output.extend_from_slice(bytes);
    }

    fn finish(self) -> Result<Vec<u8>, PackageReviewCanonicalRowRecoveryError> {
        if self.exceeded {
            Err(PackageReviewCanonicalRowRecoveryError::new(
                "canonical-row recovery envelope exceeds its byte ceiling",
            ))
        } else {
            Ok(self.output)
        }
    }

    fn fixed_bytes(&mut self, bytes: &[u8]) {
        self.append(bytes);
    }

    fn byte(&mut self, value: u8) {
        self.append(&[value]);
    }

    fn u16(&mut self, value: u16) {
        self.append(&value.to_le_bytes());
    }

    fn u64(&mut self, value: u64) {
        self.append(&value.to_le_bytes());
    }

    fn usize(&mut self, value: usize) -> Result<(), PackageReviewCanonicalRowRecoveryError> {
        self.u64(u64::try_from(value).map_err(|_| {
            PackageReviewCanonicalRowRecoveryError::new(
                "canonical-row recovery value exceeds the portable encoding range",
            )
        })?);
        Ok(())
    }

    fn bytes(&mut self, bytes: &[u8]) -> Result<(), PackageReviewCanonicalRowRecoveryError> {
        self.usize(bytes.len())?;
        self.append(bytes);
        Ok(())
    }

    fn string(&mut self, value: &str) -> Result<(), PackageReviewCanonicalRowRecoveryError> {
        self.bytes(value.as_bytes())
    }
}

struct RecoveryDecoder<'bytes> {
    bytes: &'bytes [u8],
    position: usize,
}

impl<'bytes> RecoveryDecoder<'bytes> {
    const fn new(bytes: &'bytes [u8]) -> Self {
        Self { bytes, position: 0 }
    }

    fn take(
        &mut self,
        count: usize,
    ) -> Result<&'bytes [u8], PackageReviewCanonicalRowRecoveryError> {
        let end = self.position.checked_add(count).ok_or_else(|| {
            PackageReviewCanonicalRowRecoveryError::new(
                "canonical-row recovery length frame overflow",
            )
        })?;
        let value = self.bytes.get(self.position..end).ok_or_else(|| {
            PackageReviewCanonicalRowRecoveryError::new("canonical-row recovery input is truncated")
        })?;
        self.position = end;
        Ok(value)
    }

    fn fixed_bytes(
        &mut self,
        expected: &[u8],
    ) -> Result<(), PackageReviewCanonicalRowRecoveryError> {
        if self.take(expected.len())? != expected {
            return Err(PackageReviewCanonicalRowRecoveryError::new(
                "canonical-row recovery input has invalid framing magic",
            ));
        }
        Ok(())
    }

    fn byte(&mut self) -> Result<u8, PackageReviewCanonicalRowRecoveryError> {
        Ok(self.take(1)?[0])
    }

    fn u16(&mut self) -> Result<u16, PackageReviewCanonicalRowRecoveryError> {
        Ok(u16::from_le_bytes(
            self.take(2)?.try_into().expect("two-byte decoder slice"),
        ))
    }

    fn u64(&mut self) -> Result<u64, PackageReviewCanonicalRowRecoveryError> {
        Ok(u64::from_le_bytes(
            self.take(8)?.try_into().expect("eight-byte decoder slice"),
        ))
    }

    fn usize(&mut self) -> Result<usize, PackageReviewCanonicalRowRecoveryError> {
        usize::try_from(self.u64()?).map_err(|_| {
            PackageReviewCanonicalRowRecoveryError::new(
                "canonical-row recovery length exceeds the host range",
            )
        })
    }

    fn bytes(
        &mut self,
        maximum_bytes: usize,
    ) -> Result<&'bytes [u8], PackageReviewCanonicalRowRecoveryError> {
        let count = self.usize()?;
        if count > maximum_bytes {
            return Err(PackageReviewCanonicalRowRecoveryError::new(
                "canonical-row recovery length frame exceeds its field ceiling",
            ));
        }
        self.take(count)
    }

    fn string(
        &mut self,
        maximum_bytes: usize,
    ) -> Result<&'bytes str, PackageReviewCanonicalRowRecoveryError> {
        std::str::from_utf8(self.bytes(maximum_bytes)?).map_err(|_| {
            PackageReviewCanonicalRowRecoveryError::new(
                "canonical-row recovery string is not valid UTF-8",
            )
        })
    }

    fn array_32(&mut self) -> Result<[u8; 32], PackageReviewCanonicalRowRecoveryError> {
        Ok(self
            .take(32)?
            .try_into()
            .expect("thirty-two-byte decoder slice"))
    }

    fn finish(self) -> Result<(), PackageReviewCanonicalRowRecoveryError> {
        if self.position != self.bytes.len() {
            return Err(PackageReviewCanonicalRowRecoveryError::new(
                "canonical-row recovery input contains trailing bytes",
            ));
        }
        Ok(())
    }
}
