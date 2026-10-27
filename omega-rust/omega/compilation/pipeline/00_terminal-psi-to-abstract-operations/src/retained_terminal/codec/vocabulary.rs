//! Compiler-private, complete ordinary vocabulary for native derivation inputs.
//!
//! Decoding produces inert candidates. Calling-plan, entry, provider and artifact
//! admission still belongs to the ordinary receiving owners.

use super::records::record_fields;
use super::value;

// The exhaustive write match and field patterns make new variants and payload
// fields compile errors until their retained representation is specified.
macro_rules! record_variants {
    ($value:path { $( $tag:literal => $variant:ident
        $( ( $($element:ident),* $(,)? ) )?
        $( { $($field:ident),* $(,)? } )?
    ),* $(,)? }) => {
        impl super::value::RecordValue for $value {
            fn write(&self, output: &mut super::value::RecordWriter)
                -> Result<(), super::value::RecordError>
            {
                match self {
                    $(Self::$variant $( ( $($element),* ) )? $( { $($field),* } )? => {
                        super::value::RecordValue::write(&($tag as u16), output)?;
                        $( $(super::value::RecordValue::write($element, output)?;)* )?
                        $( $(super::value::RecordValue::write($field, output)?;)* )?
                        Ok(())
                    }),*
                }
            }
            fn read(input: &mut super::value::RecordReader<'_>)
                -> Result<Self, super::value::RecordError>
            {
                input.record(|input| match <u16 as super::value::RecordValue>::read(input)? {
                    $($tag => Ok(Self::$variant
                        $( ( $({ let $element = super::value::RecordValue::read(input)?; $element }),* ) )?
                        $( { $($field: super::value::RecordValue::read(input)?),* } )?
                    ),)*
                    _ => Err(input.error(concat!("invalid ", stringify!($value), " variant"))),
                })
            }
        }
    };
}

mod calling;
mod providers;
mod receipts;
mod recovered;
mod semantic;
mod targets;
