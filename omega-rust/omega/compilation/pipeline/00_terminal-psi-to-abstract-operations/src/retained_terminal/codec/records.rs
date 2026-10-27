//! Exhaustive records: a new semantic field must acquire a wire representation.
macro_rules! record_fields {
    ($value:path { $($field:ident),* $(,)? }) => {
        impl $crate::retained_terminal::codec::value::RecordValue for $value {
            fn write(&self, output: &mut $crate::retained_terminal::codec::value::RecordWriter)
                -> Result<(), $crate::retained_terminal::codec::value::RecordError>
            {
                let Self { $($field),* } = self;
                $($crate::retained_terminal::codec::value::RecordValue::write($field, output)?;)*
                Ok(())
            }
            fn read(input: &mut $crate::retained_terminal::codec::value::RecordReader<'_>)
                -> Result<Self, $crate::retained_terminal::codec::value::RecordError>
            {
                input.record(|input| Ok(Self {
                    $($field: $crate::retained_terminal::codec::value::RecordValue::read(input)?),*
                }))
            }
        }
    };
}
pub(super) use record_fields;
