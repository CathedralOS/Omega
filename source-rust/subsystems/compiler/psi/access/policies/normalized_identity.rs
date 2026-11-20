macro_rules! normalized_identity {
    ($name:ident, $label:literal $(, $field_vis:vis)?) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name($($field_vis)? u64);
        impl record_storage::Record for $name {
            fn write(&self, output: &mut record_storage::RecordWriter) -> Result<(), record_storage::RecordError> { record_storage::Record::write(&self.0, output) }
            fn read<Context>(input: &mut record_storage::RecordReader<'_, Context>) -> Result<Self, record_storage::RecordError> {
                Self::from_normalized_identity(record_storage::Record::read(input)?)
                    .map_err(|_| input.error(concat!($label, " identity cannot be zero")))
            }
        }

        impl $name {
            pub fn from_normalized_identity(identity: u64) -> Result<Self, AccessPlanDiagnostic> {
                if identity == 0 {
                    return Err(AccessPlanDiagnostic(
                        concat!($label, " cannot be zero").into(),
                    ));
                }
                Ok(Self(identity))
            }

            pub const fn normalized_identity(self) -> u64 {
                self.0
            }
        }
    };
}

pub(crate) use normalized_identity;
