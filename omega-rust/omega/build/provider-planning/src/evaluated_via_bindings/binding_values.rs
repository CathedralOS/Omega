//! Decoded binding values, exact fields and retained usage.

use build_time_evaluation::{BuildTimeValue, EvaluationUsage};
use effects::provider_plan::EvaluatedBindingUsage;
use target::ForeignLocatorCandidate;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum DecodedBindingValue {
    Import(ForeignLocatorCandidate),
    Syscall { number: u64 },
}

pub(crate) fn decode_binding_value(
    value: &BuildTimeValue,
    widths: [u64; 3],
) -> Result<DecodedBindingValue, String> {
    let BuildTimeValue::Case { variant, payload } = value else {
        return Err("ordinary external `via` must evaluate to one exact Binding case".to_owned());
    };
    if variant == "Syscall" {
        require_unused_width(widths[0], "Syscall ObjectLength")?;
        require_unused_width(widths[1], "Syscall SymbolLength")?;
        require_unused_width(widths[2], "Syscall VersionLength")?;
        let [number] = exact_fields(payload, ["number"])?;
        let BuildTimeValue::Int(number) = number else {
            return Err(
                "ForeignBinding::Syscall number must evaluate as u64-compatible Int".to_owned(),
            );
        };
        let number = u64::try_from(*number)
            .map_err(|_| "ForeignBinding::Syscall number must fit u64".to_owned())?;
        return Ok(DecodedBindingValue::Syscall { number });
    }
    if variant != "DllImport" || payload.len() != 1 || payload[0].0 != "import" {
        return Err(
            "ordinary external `via` must evaluate to the exact ForeignBinding::DllImport or ForeignBinding::Syscall payload"
                .to_owned(),
        );
    }
    let BuildTimeValue::Case { variant, payload } = &payload[0].1 else {
        return Err("ForeignBinding::DllImport must contain one DllImport case".to_owned());
    };
    match variant.as_str() {
        "PeByName" => {
            require_unused_width(widths[2], "PeByName VersionLength")?;
            let [library, export] = exact_fields(payload, ["library", "export"])?;
            Ok(DecodedBindingValue::Import(
                ForeignLocatorCandidate::PeByName {
                    library: exact_bytes(library, widths[0], "PeByName library")?,
                    export: exact_bytes(export, widths[1], "PeByName export")?,
                },
            ))
        }
        "PeByOrdinal" => {
            require_unused_width(widths[1], "PeByOrdinal SymbolLength")?;
            require_unused_width(widths[2], "PeByOrdinal VersionLength")?;
            let [library, ordinal] = exact_fields(payload, ["library", "ordinal"])?;
            let BuildTimeValue::Int(ordinal) = ordinal else {
                return Err("PeByOrdinal ordinal must evaluate as u16-compatible Int".to_owned());
            };
            let ordinal = u16::try_from(*ordinal)
                .map_err(|_| "PeByOrdinal ordinal must fit nonzero u16".to_owned())?;
            if ordinal == 0 {
                return Err("PeByOrdinal ordinal must be nonzero".to_owned());
            }
            Ok(DecodedBindingValue::Import(
                ForeignLocatorCandidate::PeByOrdinal {
                    library: exact_bytes(library, widths[0], "PeByOrdinal library")?,
                    ordinal,
                },
            ))
        }
        "ElfVersioned" => {
            let [object, symbol, version] = exact_fields(payload, ["object", "symbol", "version"])?;
            Ok(DecodedBindingValue::Import(
                ForeignLocatorCandidate::ElfVersioned {
                    object: exact_bytes(object, widths[0], "ElfVersioned object")?,
                    symbol: exact_bytes(symbol, widths[1], "ElfVersioned symbol")?,
                    version: exact_bytes(version, widths[2], "ElfVersioned version")?,
                },
            ))
        }
        "MachODylibSymbol" => {
            require_unused_width(widths[2], "MachODylibSymbol VersionLength")?;
            let [install_name, symbol] = exact_fields(payload, ["install_name", "symbol"])?;
            Ok(DecodedBindingValue::Import(
                ForeignLocatorCandidate::MachODylibSymbol {
                    install_name: exact_bytes(
                        install_name,
                        widths[0],
                        "MachODylibSymbol install_name",
                    )?,
                    symbol: exact_bytes(symbol, widths[1], "MachODylibSymbol symbol")?,
                },
            ))
        }
        _ => Err(format!("unknown compiler-owned DllImport case `{variant}`")),
    }
}

fn exact_fields<'a, const N: usize>(
    payload: &'a [(String, BuildTimeValue)],
    names: [&str; N],
) -> Result<[&'a BuildTimeValue; N], String> {
    if payload.len() != N
        || payload
            .iter()
            .zip(names)
            .any(|((actual, _), expected)| actual != expected)
    {
        return Err(
            "evaluated foreign locator payload fields drifted from the compiler-owned declaration"
                .to_owned(),
        );
    }
    Ok(std::array::from_fn(|index| &payload[index].1))
}

fn exact_bytes(value: &BuildTimeValue, width: u64, label: &str) -> Result<Vec<u8>, String> {
    let BuildTimeValue::Array(elements) = value else {
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

fn require_unused_width(width: u64, label: &str) -> Result<(), String> {
    (width == 0)
        .then_some(())
        .ok_or_else(|| format!("{label} must be zero for this locator case"))
}

pub(crate) fn retained_usage(usage: EvaluationUsage) -> Result<EvaluatedBindingUsage, String> {
    EvaluatedBindingUsage::from_evaluator(
        usage.schema().schema_version(),
        usage.schedule().marker(),
        usage.fuel_units(),
        usage.fuel_ceiling(),
        usage.build_log_bytes(),
        usage.filesystem_operation_attempts(),
        usage.peak_live_cells(),
        usage.peak_live_text_bytes(),
        usage.result_cells(),
        usage.result_text_bytes(),
    )
}
