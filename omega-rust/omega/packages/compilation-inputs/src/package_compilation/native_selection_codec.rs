//! Canonical native-intent section of the existing package Build handoff.
//! Source anchors reuse the provider section's custody encoding. Recovery is
//! inert: original receipts are reconstructed and their complete digest checked.
use super::provider_selection_codec::{
    SelectionSources, decode_anchor, decode_sources, decode_span, encode_sources, integer,
    sequence_length,
};
use super::retained_native_bindings::*;
use effects::provider_plan::{
    EvaluatedBindingEvaluationDigest, EvaluatedBindingMaterializationDigest,
    EvaluatedBindingProducerClosureDigest, EvaluatedBindingReceipt, EvaluatedBindingUsage,
};
use filesystem_authority::observation_codec::{ObservationCursor, push_framed};
use language_semantics::declaration_selection::{
    BuildSelectionOrigin, NativeBindingValue, ProductTarget,
};

const MAGIC: &[u8] = b"OMG-BUILD-NATIVE-CHOICES\x02";

fn pool(rows: &[RetainedNativeBinding]) -> SelectionSources {
    let mut sources = Vec::new();
    for row in rows {
        for anchor in row
            .anchors()
            .chain([row.origin.activation(), row.origin.author()])
            .chain(
                row.same_stack_demand
                    .iter()
                    .flat_map(|demand| [demand.origin.activation(), demand.origin.author()]),
            )
        {
            if !sources.contains(anchor.source()) {
                sources.push(anchor.source().clone());
            }
        }
    }
    SelectionSources { sources }
}

pub fn encode_package_native_bindings(rows: &[RetainedNativeBinding]) -> Vec<u8> {
    let pool = pool(rows);
    let mut output = MAGIC.to_vec();
    encode_sources(&pool.sources, &mut output);
    encode_rows(&pool, rows, &mut output);
    output
}

pub fn package_native_bindings_digest(rows: &[RetainedNativeBinding]) -> [u8; 32] {
    let pool = pool(rows);
    let mut digest =
        crate::digest_encoder::DomainDigestEncoder::new(b"omega-package-native-choices-v2\0");
    digest.u64(pool.sources.len() as u64);
    for source in &pool.sources {
        let relative = source
            .path
            .strip_prefix(&source.package_root)
            .unwrap_or(&source.path);
        digest.bytes(relative.as_os_str().as_encoded_bytes());
        let mut owner = Vec::new();
        super::provider_selection_codec::source_owner(source, &mut owner);
        digest.bytes(&owner);
        digest.raw(&crate::digest_encoder::content_identity(
            source.source.as_bytes(),
        ));
    }
    let mut rows_bytes = Vec::new();
    encode_rows(&pool, rows, &mut rows_bytes);
    digest.bytes(&rows_bytes);
    digest.finish()
}

fn origin(pool: &SelectionSources, origin: &BuildSelectionOrigin, out: &mut Vec<u8>) {
    pool.anchor(origin.activation(), out);
    pool.anchor(origin.author(), out);
    integer(out, origin.occurrence().start as u64);
    integer(out, origin.occurrence().end as u64);
    optional(origin.event_ordinal().as_ref(), out, |value, out| {
        integer(out, *value)
    });
}
fn optional<T>(value: Option<&T>, out: &mut Vec<u8>, encode: impl FnOnce(&T, &mut Vec<u8>)) {
    match value {
        None => out.push(0),
        Some(value) => {
            out.push(1);
            encode(value, out);
        }
    }
}
fn text(value: &str, out: &mut Vec<u8>) {
    push_framed(out, value.as_bytes());
}
fn texts(values: &[String], out: &mut Vec<u8>) {
    integer(out, values.len() as u64);
    for value in values {
        text(value, out);
    }
}
fn encode_rows(pool: &SelectionSources, rows: &[RetainedNativeBinding], out: &mut Vec<u8>) {
    integer(out, rows.len() as u64);
    for row in rows {
        pool.anchor(&row.requirement, out);
        text(&row.requirement_identity, out);
        match &row.application {
            RetainedNativeApplication::Universal => out.push(0),
            RetainedNativeApplication::Applied { outer, local } => {
                out.push(1);
                optional(outer.as_ref(), out, |s, o| text(s, o));
                optional(local.as_ref(), out, |local, out| {
                    texts(&local.arguments, out);
                    texts(&local.lifetimes, out);
                });
            }
        }
        origin(pool, &row.origin, out);
        text(&row.author_path, out);
        integer(out, row.target.code());
        locator(&row.raw_locator, out);
        for width in row.widths {
            integer(out, width);
        }
        optional(row.table.as_ref(), out, |table, out| {
            pool.anchor(&table.table, out);
            text(&table.table_identity, out);
            pool.anchor(&table.field, out);
            text(&table.field_identity, out);
        });
        optional(row.same_stack_demand.as_ref(), out, |demand, out| {
            integer(out, demand.bytes);
            origin(pool, &demand.origin, out);
        });
        optional(row.unavailable.as_ref(), out, |s, o| text(s, o));
        crate::build_execution::build_record_codec::encode_build_evaluation_usage(&row.usage, out);
        out.extend_from_slice(&row.observation);
        text(&row.producer_callable, out);
        receipt(&row.receipt, out);
    }
}
fn locator(value: &NativeBindingValue, out: &mut Vec<u8>) {
    match value {
        NativeBindingValue::PeByName { library, export } => {
            out.push(0);
            push_framed(out, library);
            push_framed(out, export);
        }
        NativeBindingValue::PeByOrdinal { library, ordinal } => {
            out.push(1);
            push_framed(out, library);
            integer(out, u64::from(*ordinal));
        }
        NativeBindingValue::ElfVersioned {
            object,
            symbol,
            version,
        } => {
            out.push(2);
            push_framed(out, object);
            push_framed(out, symbol);
            push_framed(out, version);
        }
        NativeBindingValue::MachODylibSymbol {
            install_name,
            symbol,
        } => {
            out.push(3);
            push_framed(out, install_name);
            push_framed(out, symbol);
        }
        NativeBindingValue::Syscall { number } => {
            out.push(4);
            integer(out, *number);
        }
        NativeBindingValue::TableFunction => out.push(5),
        NativeBindingValue::VtableField => out.push(6),
        NativeBindingValue::SameStackDemand { bytes } => {
            out.push(7);
            integer(out, *bytes);
        }
    }
}
fn receipt(value: &EvaluatedBindingReceipt, out: &mut Vec<u8>) {
    optional(value.producer_package().as_ref(), out, |package, out| {
        out.extend_from_slice(&package.digest())
    });
    out.extend_from_slice(&value.producer_closure_digest().as_bytes());
    integer(out, u64::from(value.evaluator_semantics_marker()));
    let usage = value.evaluation_usage();
    for value in [
        u64::from(usage.usage_schema_version()),
        u64::from(usage.step_schedule_marker()),
        usage.fuel_units(),
        usage.fuel_ceiling(),
        usage.build_log_bytes(),
        usage.filesystem_operation_attempts(),
        usage.peak_live_cells(),
        usage.peak_live_text_bytes(),
        usage.result_cells(),
        usage.result_text_bytes(),
    ] {
        integer(out, value);
    }
    out.extend_from_slice(&value.evaluation_digest().as_bytes());
    integer(out, u64::from(value.materializer_schema_version()));
    out.extend_from_slice(&value.materialization_digest().as_bytes());
    out.extend_from_slice(&value.locator_identity_digest().as_bytes());
    out.extend_from_slice(&value.identity_digest());
}

fn read_optional<T>(
    cursor: &mut ObservationCursor<'_>,
    read: impl FnOnce(&mut ObservationCursor<'_>) -> Option<T>,
) -> Option<Option<T>> {
    match cursor.u8()? {
        0 => Some(None),
        1 => Some(Some(read(cursor)?)),
        _ => None,
    }
}
fn read_text(cursor: &mut ObservationCursor<'_>) -> Option<String> {
    Some(std::str::from_utf8(cursor.framed()?).ok()?.to_owned())
}
fn read_texts(cursor: &mut ObservationCursor<'_>) -> Option<Vec<String>> {
    let count = sequence_length(cursor)?;
    (0..count).map(|_| read_text(cursor)).collect()
}
fn read_origin(
    cursor: &mut ObservationCursor<'_>,
    sources: &[source::SourceFile],
) -> Option<BuildSelectionOrigin> {
    let activation = decode_anchor(cursor, sources)?;
    let author = decode_anchor(cursor, sources)?;
    let occurrence = decode_span(cursor)?;
    let ordinal = read_optional(cursor, |c| c.u64())?;
    BuildSelectionOrigin::from_retained_parts(activation, author, occurrence, ordinal).ok()
}
fn read_locator(cursor: &mut ObservationCursor<'_>) -> Option<NativeBindingValue> {
    Some(match cursor.u8()? {
        0 => NativeBindingValue::PeByName {
            library: cursor.framed()?.to_vec(),
            export: cursor.framed()?.to_vec(),
        },
        1 => NativeBindingValue::PeByOrdinal {
            library: cursor.framed()?.to_vec(),
            ordinal: u16::try_from(cursor.u64()?).ok()?,
        },
        2 => NativeBindingValue::ElfVersioned {
            object: cursor.framed()?.to_vec(),
            symbol: cursor.framed()?.to_vec(),
            version: cursor.framed()?.to_vec(),
        },
        3 => NativeBindingValue::MachODylibSymbol {
            install_name: cursor.framed()?.to_vec(),
            symbol: cursor.framed()?.to_vec(),
        },
        4 => NativeBindingValue::Syscall {
            number: cursor.u64()?,
        },
        5 => NativeBindingValue::TableFunction,
        6 => NativeBindingValue::VtableField,
        7 => NativeBindingValue::SameStackDemand {
            bytes: cursor.u64()?,
        },
        _ => return None,
    })
}
fn digest(cursor: &mut ObservationCursor<'_>) -> Option<[u8; 32]> {
    cursor.bytes(32)?.try_into().ok()
}
fn read_receipt(
    cursor: &mut ObservationCursor<'_>,
    producer: &str,
) -> Option<EvaluatedBindingReceipt> {
    let package = read_optional(cursor, |cursor| {
        semantic_vocabulary::PackageKeyIdentity::from_digest(digest(cursor)?)
    })?;
    let closure = EvaluatedBindingProducerClosureDigest::from_bytes(digest(cursor)?).ok()?;
    let evaluator = u32::try_from(cursor.u64()?).ok()?;
    let usage = EvaluatedBindingUsage::from_evaluator(
        u32::try_from(cursor.u64()?).ok()?,
        u32::try_from(cursor.u64()?).ok()?,
        cursor.u64()?,
        cursor.u64()?,
        cursor.u64()?,
        cursor.u64()?,
        cursor.u64()?,
        cursor.u64()?,
        cursor.u64()?,
        cursor.u64()?,
    )
    .ok()?;
    let evaluation = EvaluatedBindingEvaluationDigest::from_bytes(digest(cursor)?).ok()?;
    let schema = u32::try_from(cursor.u64()?).ok()?;
    let materialization =
        EvaluatedBindingMaterializationDigest::from_bytes(digest(cursor)?).ok()?;
    let locator = target::ForeignLocatorIdentityDigest::from_digest(digest(cursor)?);
    let expected = digest(cursor)?;
    let value = EvaluatedBindingReceipt::from_evaluation(
        package,
        producer,
        &mut terminal_psi::TerminalIdentityTableBuilder::default(),
        closure,
        evaluator,
        usage,
        evaluation,
        schema,
        materialization,
        locator,
    )
    .ok()?;
    (value.identity_digest() == expected).then_some(value)
}

pub fn decode_package_native_bindings(bytes: &[u8]) -> Option<Vec<RetainedNativeBinding>> {
    let mut cursor = ObservationCursor::new(bytes);
    if cursor.bytes(MAGIC.len())? != MAGIC {
        return None;
    }
    let sources = decode_sources(&mut cursor)?;
    let count = sequence_length(&mut cursor)?;
    let mut rows = Vec::with_capacity(count);
    for _ in 0..count {
        let requirement = decode_anchor(&mut cursor, &sources)?;
        let requirement_identity = read_text(&mut cursor)?;
        let application = match cursor.u8()? {
            0 => RetainedNativeApplication::Universal,
            1 => RetainedNativeApplication::Applied {
                outer: read_optional(&mut cursor, read_text)?,
                local: read_optional(&mut cursor, |cursor| {
                    Some(RetainedNativeLocalApplication {
                        arguments: read_texts(cursor)?,
                        lifetimes: read_texts(cursor)?,
                    })
                })?,
            },
            _ => return None,
        };
        let origin = read_origin(&mut cursor, &sources)?;
        let author_path = read_text(&mut cursor)?;
        let target = ProductTarget::from_code(cursor.u64()?)?;
        let raw_locator = read_locator(&mut cursor)?;
        let widths = [cursor.u64()?, cursor.u64()?, cursor.u64()?];
        let table = read_optional(&mut cursor, |cursor| {
            Some(RetainedNativeTable {
                table: decode_anchor(cursor, &sources)?,
                table_identity: read_text(cursor)?,
                field: decode_anchor(cursor, &sources)?,
                field_identity: read_text(cursor)?,
            })
        })?;
        let same_stack_demand = read_optional(&mut cursor, |cursor| {
            Some(RetainedNativeSameStackDemand {
                bytes: cursor.u64()?,
                origin: read_origin(cursor, &sources)?,
            })
        })?;
        let unavailable = read_optional(&mut cursor, read_text)?;
        let usage = crate::build_execution::build_record_codec::recover_build_evaluation_usage(
            &mut cursor,
        )?;
        let observation = digest(&mut cursor)?;
        let producer_callable = read_text(&mut cursor)?;
        let receipt = read_receipt(&mut cursor, &producer_callable)?;
        let row = RetainedNativeBinding {
            requirement,
            requirement_identity,
            application,
            origin,
            author_path,
            target,
            raw_locator,
            widths,
            table,
            same_stack_demand,
            unavailable,
            usage,
            observation,
            producer_callable,
            receipt,
        };
        row.validate().ok()?;
        rows.push(row);
    }
    cursor.exhausted().then_some(rows)
}
