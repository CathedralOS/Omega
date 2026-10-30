use super::{Encoder, PackageReviewEncodingError};
use semantic_vocabulary::PackageKeyIdentity;

/// The scalar field forms: one named field whose payload is a single scalar
/// write, delegating to `Encoder::field` for the field's own framing. A policy
/// row states the scalar value and the field's name; the wrapper closure that
/// restated the framing at every site is not part of the encoder's surface.
/// A writer either reports a failure or cannot: the field's own check reports
/// either, so both kinds are the same form here.
macro_rules! scalar_field_forms {
    (@total $($method:ident($value:ty) => $writer:ident),+ $(,)?) => {
        $(
            pub(crate) fn $method(
                &mut self,
                name: &'static str,
                value: $value,
            ) -> Result<(), PackageReviewEncodingError> {
                self.field(name, |encoder| {
                    encoder.$writer(value);
                    Ok(())
                })
            }
        )+
    };
    (@fallible $($method:ident($value:ty) => $writer:ident),+ $(,)?) => {
        $(
            pub(crate) fn $method(
                &mut self,
                name: &'static str,
                value: $value,
            ) -> Result<(), PackageReviewEncodingError> {
                self.field(name, |encoder| {
                    encoder.$writer(value)?;
                    Ok(())
                })
            }
        )+
    };
}

/// The little-endian scalar payloads: each writes its own width and the field
/// text that names it, and carries no framing of its own.
macro_rules! scalar_numbers {
    ($($method:ident($value:ty)),+ $(,)?) => {
        $(
            pub(crate) fn $method(&mut self, value: $value) {
                if let Some(text) = &mut self.text {
                    text.number(stringify!($method), value);
                }
                self.append(&value.to_le_bytes());
            }
        )+
    };
}

impl Encoder<'_> {
    scalar_field_forms!(@total
        byte_field(u8) => byte,
        u16_field(u16) => u16,
        u32_field(u32) => u32,
        u64_field(u64) => u64,
        i64_field(i64) => i64,
        boolean_field(bool) => boolean,
        fixed_bytes_field(&[u8]) => fixed_bytes,
        package_identity_field(PackageKeyIdentity) => package_identity,
        optional_package_identity_field(Option<PackageKeyIdentity>) => optional_package_identity,
    );
    scalar_field_forms!(@fallible
        string_field(&str) => string,
        usize_field(usize) => usize,
    );

    /// One tag field: the tag's own name and value under this field's name,
    /// for the tag-typed rows that carried the wrapper as well.
    pub(crate) fn tag_field(
        &mut self,
        name: &'static str,
        tag: &'static str,
        value: u8,
    ) -> Result<(), PackageReviewEncodingError> {
        self.field(name, |encoder| {
            encoder.tag(tag, value);
            Ok(())
        })
    }
    pub(crate) fn fixed_bytes(&mut self, value: &[u8]) {
        if let Some(text) = &mut self.text {
            text.bytes("fixed", value);
        }
        self.append(value);
    }

    pub(crate) fn byte(&mut self, value: u8) {
        if let Some(text) = &mut self.text {
            text.number("u8", value);
        }
        self.append(&[value]);
    }

    pub(crate) fn tag(&mut self, name: &'static str, value: u8) {
        if let Some(text) = &mut self.text {
            text.tag(name, value);
        }
        self.append(&[value]);
    }

    pub(crate) fn boolean(&mut self, value: bool) {
        if let Some(text) = &mut self.text {
            text.boolean(value);
        }
        self.append(&[u8::from(value)]);
    }

    scalar_numbers!(u16(u16), u32(u32), u64(u64), i64(i64));

    pub(crate) fn usize(&mut self, value: usize) -> Result<(), PackageReviewEncodingError> {
        self.u64(u64::try_from(value).map_err(|_| {
            PackageReviewEncodingError::new(
                "package review value exceeds the portable encoding range",
            )
        })?);
        self.check()
    }

    pub(crate) fn bytes(&mut self, value: &[u8]) -> Result<(), PackageReviewEncodingError> {
        self.byte_string("bytes", value)
    }

    pub(crate) fn string(&mut self, value: &str) -> Result<(), PackageReviewEncodingError> {
        self.byte_string("string", value.as_bytes())
    }

    fn byte_string(
        &mut self,
        kind: &'static str,
        value: &[u8],
    ) -> Result<(), PackageReviewEncodingError> {
        let count = u64::try_from(value.len()).map_err(|_| {
            PackageReviewEncodingError::new(
                "package review value exceeds the portable encoding range",
            )
        })?;
        if let Some(text) = &mut self.text {
            text.bytes(kind, value);
        }
        self.append(&count.to_le_bytes());
        self.append(value);
        self.check()
    }
}
