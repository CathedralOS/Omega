use super::framing::binary;
use crate::encoding::encode::encoder::text::{HEADER, MAXIMUM_MARKUP_DEPTH, Writer};
use crate::encoding::{PackagePolicyRecoveryLimits, encode::encoder::Encoder};

fn fields(encoder: &mut Encoder) -> Result<(), crate::encoding::PackageReviewEncodingError> {
    encoder.field("unsigned_byte", |encoder| {
        encoder.byte(255);
        Ok(())
    })?;
    encoder.field("unsigned_short", |encoder| {
        encoder.u16(u16::MAX);
        Ok(())
    })?;
    encoder.field("unsigned_word", |encoder| {
        encoder.u32(u32::MAX);
        Ok(())
    })?;
    encoder.field("unsigned_long", |encoder| {
        encoder.u64(u64::MAX);
        Ok(())
    })?;
    encoder.field("signed_long", |encoder| {
        encoder.i64(i64::MIN);
        Ok(())
    })?;
    encoder.field("signed_wide", |encoder| {
        encoder.i128(i128::MIN);
        Ok(())
    })?;
    encoder.field("unsigned_wide", |encoder| {
        encoder.u128(u128::MAX);
        Ok(())
    })?;
    encoder.field("truth", |encoder| {
        encoder.boolean(true);
        Ok(())
    })?;
    encoder.field("variant", |encoder| {
        encoder.tag("last", 255);
        Ok(())
    })?;
    encoder.field("fixed_header", |encoder| {
        encoder.fixed_bytes(b"hello\0");
        Ok(())
    })?;
    encoder.field("digest", |encoder| {
        encoder.fixed_bytes(&[171; 32]);
        Ok(())
    })?;
    encoder.field("text", |encoder| {
        encoder.string("quotation\"slash\\unicode雪\n")
    })?;
    encoder.field("bytes", |encoder| encoder.bytes(&[0, 255, b'"', b'\\']))?;
    encoder.field("absent", |encoder| {
        encoder.option(None::<&u64>, |encoder, value| {
            encoder.u64(*value);
            Ok(())
        })
    })?;
    encoder.field("present", |encoder| {
        encoder.option(Some(&7u64), |encoder, value| {
            encoder.u64(*value);
            Ok(())
        })
    })?;
    encoder.field("sequence", |encoder| {
        encoder.sequence(&[1u32, 2], |encoder, value| {
            encoder.u32(*value);
            Ok(())
        })
    })
}

#[test]
fn every_scalar_and_container_preserves_the_original_binary_stream() {
    let mut bytes = Encoder::policy_bounded(4 * 1024 * 1024);
    fields(&mut bytes).unwrap();
    let bytes = bytes.finish().unwrap();
    let mut text = Encoder::policy_text(Writer::new(32 * 1024 * 1024, None));
    fields(&mut text).unwrap();
    let text = text.finish_text().unwrap();
    assert_eq!(
        binary(&text, PackagePolicyRecoveryLimits::default())
            .unwrap()
            .0,
        bytes
    );
    let mut compare = Encoder::policy_text(Writer::new(text.len(), Some(&text)));
    fields(&mut compare).unwrap();
    assert!(
        compare.finish_text().unwrap().is_empty(),
        "verification never collects another text buffer"
    );
    assert!(text.contains("\\xff"));
    assert!(text.contains("option none"));
    assert!(text.contains("option some {"));
    assert!(text.contains("tag last 255"));
}

#[test]
fn missing_field_labels_and_ignored_text_errors_cannot_publish() {
    let mut writer = Encoder::policy_text(Writer::new(1024, None));
    writer.byte(0);
    assert!(writer.finish_text().is_err());
    let mut writer = Encoder::policy_text(Writer::new(1024, None));
    let _ = writer.field("compound", |encoder| {
        encoder.byte(0);
        encoder.byte(1);
        Ok(())
    });
    assert!(writer.check().is_err());
    assert!(writer.finish_text().is_err());
    let mut writer = Encoder::policy_text(Writer::new(HEADER.len(), None));
    let _ = writer.field("too_large", |encoder| {
        encoder.byte(0);
        Ok(())
    });
    assert!(writer.finish_text().is_err());
}

#[test]
fn malformed_framing_never_allocates_from_untrusted_counts() {
    use crate::encoding::PackagePolicyRecoveryError as Error;
    for body in [
        "sequence 18446744073709551615 {\n",
        "u128 340282366920938463463374607431768211456\n",
        "string \"\\xgg\"\n",
        "string \"\\q\"\n",
        "string \"unclosed",
        "option maybe\n",
        "digest 00\n",
        "tag unknown 256\n",
        "}\n",
        "field x {\n",
    ] {
        assert!(
            binary(
                &format!("{HEADER}{body}"),
                PackagePolicyRecoveryLimits::default()
            )
            .is_err(),
            "{body}"
        );
    }
    let deep = format!(
        "{HEADER}{}{}",
        "field nested {\n".repeat(MAXIMUM_MARKUP_DEPTH + 1),
        "}\n".repeat(MAXIMUM_MARKUP_DEPTH + 1)
    );
    assert_eq!(
        binary(&deep, PackagePolicyRecoveryLimits::default()),
        Err(Error::NestingLimitExceeded)
    );
    let limits = PackagePolicyRecoveryLimits::new(1024, 2, 64, 1024, 8);
    assert_eq!(
        binary(&format!("{HEADER}string \"abc\"\n"), limits),
        Err(Error::FieldTooLarge)
    );
}
