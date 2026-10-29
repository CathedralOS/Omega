//! Decode an owned native-locator observation against its checked widths.
//! Labels describe the core schema; callers establish exact nominal authority.
//! Target validation, foreign contracts and execution permission are separate.

use build_time_values::BuildTimeValue;
use semantic_vocabulary::{ForeignLocatorCandidate, NativeBindingLocator};

/// Read-only observations from one actual engine value. Child views retain
/// their own storage; the decoder neither allocates an alternate value tree
/// nor converts native layout bytes into semantic authority.
pub trait NativeLocatorValue: Clone {
    fn case(&self) -> Result<NativeLocatorCase<'_, Self>, String>;
    fn integer(&self) -> Result<i128, String>;
    fn bytes(&self, width: u64, label: &str) -> Result<Vec<u8>, String>;
}

pub struct NativeLocatorCase<'value, Value> {
    pub name: &'value str,
    pub fields: Vec<(&'value str, Value)>,
}

pub fn decode_binding_value(
    value: &BuildTimeValue,
    widths: [u64; 3],
) -> Result<NativeBindingLocator, String> {
    decode_native_locator(value, widths)
}

pub fn decode_native_locator<Value: NativeLocatorValue>(
    value: Value,
    widths: [u64; 3],
) -> Result<NativeBindingLocator, String> {
    let binding = value.case()?;
    if binding.name == "Syscall" {
        require_unused_width(widths[0], "Syscall ObjectLength")?;
        require_unused_width(widths[1], "Syscall SymbolLength")?;
        require_unused_width(widths[2], "Syscall VersionLength")?;
        let [number] = exact_fields(binding, ["number"])?;
        let number = u64::try_from(number.integer()?)
            .map_err(|_| "ForeignBinding::Syscall number must fit u64".to_owned())?;
        return Ok(NativeBindingLocator::Syscall { number });
    }
    if binding.name != "DllImport" {
        return Err("native binding must evaluate to its exact ForeignBinding::DllImport or ForeignBinding::Syscall payload".to_owned());
    }
    let [import] = exact_fields(binding, ["import"])?;
    let case = import.case()?;
    let locator = match case.name {
        "PeByName" => {
            require_unused_width(widths[2], "PeByName VersionLength")?;
            let [library, export] = exact_fields(case, ["library", "export"])?;
            ForeignLocatorCandidate::PeByName {
                library: library.bytes(widths[0], "PeByName library")?,
                export: export.bytes(widths[1], "PeByName export")?,
            }
        }
        "PeByOrdinal" => {
            require_unused_width(widths[1], "PeByOrdinal SymbolLength")?;
            require_unused_width(widths[2], "PeByOrdinal VersionLength")?;
            let [library, ordinal] = exact_fields(case, ["library", "ordinal"])?;
            let ordinal = u16::try_from(ordinal.integer()?)
                .map_err(|_| "PeByOrdinal ordinal must fit nonzero u16".to_owned())?;
            if ordinal == 0 {
                return Err("PeByOrdinal ordinal must be nonzero".to_owned());
            }
            ForeignLocatorCandidate::PeByOrdinal {
                library: library.bytes(widths[0], "PeByOrdinal library")?,
                ordinal,
            }
        }
        "ElfVersioned" => {
            let [object, symbol, version] = exact_fields(case, ["object", "symbol", "version"])?;
            ForeignLocatorCandidate::ElfVersioned {
                object: object.bytes(widths[0], "ElfVersioned object")?,
                symbol: symbol.bytes(widths[1], "ElfVersioned symbol")?,
                version: version.bytes(widths[2], "ElfVersioned version")?,
            }
        }
        "MachODylibSymbol" => {
            require_unused_width(widths[2], "MachODylibSymbol VersionLength")?;
            let [install_name, symbol] = exact_fields(case, ["install_name", "symbol"])?;
            ForeignLocatorCandidate::MachODylibSymbol {
                install_name: install_name.bytes(widths[0], "MachODylibSymbol install_name")?,
                symbol: symbol.bytes(widths[1], "MachODylibSymbol symbol")?,
            }
        }
        name => return Err(format!("unknown compiler-owned DllImport case `{name}`")),
    };
    Ok(NativeBindingLocator::Import(locator))
}

fn exact_fields<Value: NativeLocatorValue, const N: usize>(
    case: NativeLocatorCase<'_, Value>,
    names: [&str; N],
) -> Result<[Value; N], String> {
    if case.fields.len() != N
        || case
            .fields
            .iter()
            .zip(names)
            .any(|((actual, _), expected)| *actual != expected)
    {
        return Err(
            "evaluated foreign locator payload fields drifted from the compiler-owned declaration"
                .to_owned(),
        );
    }
    Ok(std::array::from_fn(|index| case.fields[index].1.clone()))
}

impl NativeLocatorValue for &BuildTimeValue {
    fn case(&self) -> Result<NativeLocatorCase<'_, Self>, String> {
        let BuildTimeValue::Case { variant, payload } = *self else {
            return Err("native binding value has no exact selected core case".to_owned());
        };
        Ok(NativeLocatorCase {
            name: variant,
            fields: payload
                .iter()
                .map(|(name, value)| (name.as_str(), value))
                .collect(),
        })
    }
    fn integer(&self) -> Result<i128, String> {
        match self {
            BuildTimeValue::Int(value) => Ok(i128::from(*value)),
            _ => Err("native locator field must evaluate as an integer".to_owned()),
        }
    }
    fn bytes(&self, width: u64, label: &str) -> Result<Vec<u8>, String> {
        let BuildTimeValue::Array(elements) = self else {
            return Err(format!("{label} must evaluate as a fixed byte array"));
        };
        let expected = usize::try_from(width)
            .map_err(|_| format!("{label} width does not fit the compiler host"))?;
        if elements.len() != expected {
            return Err(format!(
                "{label} evaluated length does not match its const width"
            ));
        }
        elements
            .iter()
            .map(|element| match element {
                BuildTimeValue::Int(byte) => {
                    u8::try_from(*byte).map_err(|_| format!("{label} contains a value outside u8"))
                }
                _ => Err(format!("{label} contains a non-integer byte")),
            })
            .collect()
    }
}

fn require_unused_width(width: u64, label: &str) -> Result<(), String> {
    (width == 0)
        .then_some(())
        .ok_or_else(|| format!("{label} must be zero for this locator case"))
}
