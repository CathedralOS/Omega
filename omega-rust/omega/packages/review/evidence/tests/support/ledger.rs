use semantic_vocabulary::PackageKeyIdentity;

pub(crate) const LEDGER_MAGIC: &[u8] = b"OMEGA-ORDINARY-PACKAGE-OBLIGATION-LEDGER\0";

pub(crate) const ROW_CANONICAL_MAGIC: &[u8] = b"OMEGA-PACKAGE-REVIEW-ROW\0";
pub(crate) const ROW_RECOVERY_MAGIC: &[u8] = b"OMEGA-PACKAGE-REVIEW-ROW-RECOVERY\0";

pub(crate) fn read_ledger_u64(bytes: &[u8], position: &mut usize) -> usize {
    let end = *position + 8;
    let value = u64::from_le_bytes(bytes[*position..end].try_into().unwrap());
    *position = end;
    usize::try_from(value).unwrap()
}

/// Append one u64-length-prefixed byte field exactly as the ledger and
/// canonical-row encoders do.
pub(crate) fn push_length_prefixed(output: &mut Vec<u8>, bytes: &[u8]) {
    output.extend_from_slice(&u64::try_from(bytes.len()).unwrap().to_le_bytes());
    output.extend_from_slice(bytes);
}

pub(crate) fn ledger_target_range(bytes: &[u8]) -> std::ops::Range<usize> {
    let mut position = LEDGER_MAGIC.len() + 4 * std::mem::size_of::<u16>() + 32;
    let length = read_ledger_u64(bytes, &mut position);
    position..position + length
}

pub(crate) fn ledger_closure_package_range(bytes: &[u8]) -> std::ops::Range<usize> {
    let target = ledger_target_range(bytes);
    let mut position = target.end + 32 + 1;
    let count = read_ledger_u64(bytes, &mut position);
    position..position + count * 32
}

pub(crate) fn ledger_row_frames(bytes: &[u8]) -> Vec<std::ops::Range<usize>> {
    let packages = ledger_closure_package_range(bytes);
    let mut position = packages.end;
    let dependencies = read_ledger_u64(bytes, &mut position);
    for _ in 0..dependencies {
        position += 32;
        let alias_length = read_ledger_u64(bytes, &mut position);
        position += alias_length + 32;
    }
    let rows = read_ledger_u64(bytes, &mut position);
    (0..rows)
        .map(|_| {
            let start = position;
            let length = read_ledger_u64(bytes, &mut position);
            position += length;
            start..position
        })
        .collect()
}

/// Exact wire coordinates of every representable ledger field, so a test can
/// substitute one field at a time without hard-coding offsets.
pub(crate) struct LedgerFieldSpans {
    pub(crate) magic: std::ops::Range<usize>,
    pub(crate) encoding_version: std::ops::Range<usize>,
    pub(crate) schema_version: std::ops::Range<usize>,
    pub(crate) review_version: std::ops::Range<usize>,
    pub(crate) row_version: std::ops::Range<usize>,
    pub(crate) package: std::ops::Range<usize>,
    pub(crate) target_length: std::ops::Range<usize>,
    pub(crate) target: std::ops::Range<usize>,
    pub(crate) closure_root: std::ops::Range<usize>,
    pub(crate) root_role: usize,
    pub(crate) package_count: std::ops::Range<usize>,
    pub(crate) packages: Vec<std::ops::Range<usize>>,
    pub(crate) dependency_count: std::ops::Range<usize>,
    pub(crate) dependencies: Vec<LedgerDependencySpans>,
    pub(crate) row_count: std::ops::Range<usize>,
    /// One range per row frame including its u64 length prefix.
    pub(crate) rows: Vec<std::ops::Range<usize>>,
}

pub(crate) struct LedgerDependencySpans {
    pub(crate) requester: std::ops::Range<usize>,
    pub(crate) alias_length: std::ops::Range<usize>,
    pub(crate) alias: std::ops::Range<usize>,
    pub(crate) target: std::ops::Range<usize>,
}

pub(crate) fn ledger_field_spans(bytes: &[u8]) -> LedgerFieldSpans {
    let mut position = 0usize;
    let magic = position..position + LEDGER_MAGIC.len();
    position = magic.end;
    let encoding_version = position..position + 2;
    position += 2;
    let schema_version = position..position + 2;
    position += 2;
    let review_version = position..position + 2;
    position += 2;
    let row_version = position..position + 2;
    position += 2;
    let package = position..position + 32;
    position += 32;
    let target_length = position..position + 8;
    let length = read_ledger_u64(bytes, &mut position);
    let target = position..position + length;
    position = target.end;
    let closure_root = position..position + 32;
    position += 32;
    let root_role = position;
    position += 1;
    let package_count = position..position + 8;
    let count = read_ledger_u64(bytes, &mut position);
    let packages = (0..count)
        .map(|_| {
            let package = position..position + 32;
            position += 32;
            package
        })
        .collect();
    let dependency_count = position..position + 8;
    let count = read_ledger_u64(bytes, &mut position);
    let dependencies = (0..count)
        .map(|_| {
            let requester = position..position + 32;
            position += 32;
            let alias_length = position..position + 8;
            let length = read_ledger_u64(bytes, &mut position);
            let alias = position..position + length;
            position += length;
            let target = position..position + 32;
            position += 32;
            LedgerDependencySpans {
                requester,
                alias_length,
                alias,
                target,
            }
        })
        .collect();
    let row_count = position..position + 8;
    let count = read_ledger_u64(bytes, &mut position);
    let rows = (0..count)
        .map(|_| {
            let start = position;
            let length = read_ledger_u64(bytes, &mut position);
            position += length;
            start..position
        })
        .collect();
    assert_eq!(
        position,
        bytes.len(),
        "ledger field spans must consume the whole encoding"
    );
    LedgerFieldSpans {
        magic,
        encoding_version,
        schema_version,
        review_version,
        row_version,
        package,
        target_length,
        target,
        closure_root,
        root_role,
        package_count,
        packages,
        dependency_count,
        dependencies,
        row_count,
        rows,
    }
}

/// Exact wire coordinates inside one row's canonical frame, so a test can
/// substitute one row field at a time without hard-coding offsets.
pub(crate) struct CanonicalRowSpans {
    pub(crate) magic: std::ops::Range<usize>,
    pub(crate) row_version: std::ops::Range<usize>,
    pub(crate) review_version: std::ops::Range<usize>,
    pub(crate) package: std::ops::Range<usize>,
    pub(crate) target: std::ops::Range<usize>,
    pub(crate) kind: usize,
    pub(crate) risk: usize,
    pub(crate) key: std::ops::Range<usize>,
    pub(crate) value: std::ops::Range<usize>,
}

pub(crate) fn canonical_row_spans(row: &[u8]) -> CanonicalRowSpans {
    let mut position = 0usize;
    let magic = position..position + ROW_CANONICAL_MAGIC.len();
    position = magic.end;
    let row_version = position..position + 2;
    position += 2;
    let review_version = position..position + 2;
    position += 2;
    let package = position..position + 32;
    position += 32;
    let length = read_ledger_u64(row, &mut position);
    let target = position..position + length;
    position = target.end;
    let kind = position;
    position += 1;
    let risk = position;
    position += 1;
    let length = read_ledger_u64(row, &mut position);
    let key = position..position + length;
    position = key.end;
    let length = read_ledger_u64(row, &mut position);
    let value = position..position + length;
    position = value.end;
    assert_eq!(
        position,
        row.len(),
        "canonical row spans must consume the whole frame"
    );
    CanonicalRowSpans {
        magic,
        row_version,
        review_version,
        package,
        target,
        kind,
        risk,
        key,
        value,
    }
}

/// Hand-encode one canonical row frame for the compile-free ledger fixture.
/// `kind`/`risk` are the raw wire tags decoded by the row framing parser; the
/// caller chooses pairs the canonical-risk join accepts.
pub(crate) fn crafted_canonical_row(
    package: PackageKeyIdentity,
    target: &str,
    kind: u8,
    risk: u8,
    key: &[u8],
    value: &[u8],
) -> Vec<u8> {
    let mut row = Vec::new();
    row.extend_from_slice(ROW_CANONICAL_MAGIC);
    row.extend_from_slice(
        &package_evidence::encoding::PACKAGE_REVIEW_ROW_ENCODING_VERSION.to_le_bytes(),
    );
    row.extend_from_slice(
        &package_evidence::encoding::PACKAGE_REVIEW_ENCODING_VERSION.to_le_bytes(),
    );
    row.extend_from_slice(&package.digest());
    push_length_prefixed(&mut row, target.as_bytes());
    row.push(kind);
    row.push(risk);
    push_length_prefixed(&mut row, key);
    push_length_prefixed(&mut row, value);
    row
}

/// Wrap one canonical row frame in its recovery envelope so the public row
/// decoder can issue a `DecodedPackageReviewCanonicalRow` for ledger recovery.
/// Every row needs at least one source element, so the caller supplies one or
/// more valid compiler-derivation tags.
pub(crate) fn crafted_row_envelope(canonical: &[u8], derivations: &[u8]) -> Vec<u8> {
    crafted_row_envelope_with_locations(canonical, &[], derivations)
}

/// Wrap one canonical row frame carrying exactly one authored source location
/// owned by `owner`, so rows that describe source-backed facts recover through
/// the same provenance shape compiler issuance produces.
pub(crate) fn crafted_authored_row_envelope(
    canonical: &[u8],
    owner: PackageKeyIdentity,
    relative_path: &str,
    start_byte: u64,
    end_byte: u64,
    role_tag: u8,
) -> Vec<u8> {
    let mut location = Vec::new();
    // Package-owned location tag.
    location.push(0);
    location.extend_from_slice(&owner.digest());
    push_length_prefixed(&mut location, relative_path.as_bytes());
    location.extend_from_slice(&start_byte.to_le_bytes());
    location.extend_from_slice(&end_byte.to_le_bytes());
    location.push(role_tag);
    crafted_row_envelope_with_locations(canonical, &[location], &[])
}

fn crafted_row_envelope_with_locations(
    canonical: &[u8],
    locations: &[Vec<u8>],
    derivations: &[u8],
) -> Vec<u8> {
    let mut envelope = Vec::new();
    envelope.extend_from_slice(ROW_RECOVERY_MAGIC);
    envelope.extend_from_slice(
        &package_evidence::encoding::PACKAGE_REVIEW_CANONICAL_ROW_RECOVERY_VERSION.to_le_bytes(),
    );
    push_length_prefixed(&mut envelope, canonical);
    envelope.extend_from_slice(&u64::try_from(locations.len()).unwrap().to_le_bytes());
    for location in locations {
        envelope.extend_from_slice(location);
    }
    envelope.extend_from_slice(&u64::try_from(derivations.len()).unwrap().to_le_bytes());
    envelope.extend_from_slice(derivations);
    envelope
}
