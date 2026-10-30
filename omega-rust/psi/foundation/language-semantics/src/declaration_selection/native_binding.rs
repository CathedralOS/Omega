//! Atomic native locator values produced by one admitted Build operation.
//!
//! These values retain intent, not provider admission or executable authority.
//! Exact typed vocabulary and original activation custody belong to the source
//! producer; target-specific normalization belongs to Omega provider admission.

/// The value retained by one reached native Build operation. Locators keep
/// their ordinary typed import/syscall sum; table dispatch and stack promises
/// are distinct intents, not ForeignBinding cases.
#[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
pub enum NativeBuildIntent {
    Locator(semantic_vocabulary::NativeBindingLocator),
    Table { drops_receiver: bool },
    SameStackDemand { bytes: u64 },
}

impl NativeBuildIntent {
    pub const fn matches_operation(&self, operation: super::BuildOperation) -> bool {
        use super::BuildOperation;
        matches!(
            (self, operation),
            (Self::Locator(_), BuildOperation::NativeBinding)
                | (
                    Self::Table {
                        drops_receiver: true
                    },
                    BuildOperation::NativeTableFunction
                )
                | (
                    Self::Table {
                        drops_receiver: false
                    },
                    BuildOperation::NativeVtableField
                )
                | (
                    Self::SameStackDemand { .. },
                    BuildOperation::NativeSameStackDemand
                )
        )
    }
}

/// One reached native binding, authenticated by the ordinary Build ticket
/// protocol. The requirement remains an inert lexical request until Product
/// discovery admits the exact declaration and its full contract application.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutedNativeBinding {
    pub request: std::sync::Arc<super::ProductDeclarationRequest>,
    pub selecting_machine: source::SourceSpan,
    pub target: super::ProductTarget,
    pub intent: NativeBuildIntent,
    pub static_environment:
        Option<std::sync::Arc<super::product_operands::ProductStaticEnvironment>>,
}

/// Decode a complete, independently type-authenticated locator value. Field
/// labels are payload descriptors; they confer no nominal or activation authority.
pub fn decode_native_locator(
    value: &build_time_values::BuildTimeValue,
    widths: [u64; 3],
) -> Result<semantic_vocabulary::NativeBindingLocator, String> {
    use build_time_values::BuildTimeValue as Value;
    use semantic_vocabulary::{ForeignLocatorCandidate, NativeBindingLocator};
    fn fields<'a>(value: &'a Value, case: &str, names: &[&str]) -> Result<Vec<&'a Value>, String> {
        let Value::Case { variant, payload } = value else {
            return Err("native binding must retain one exact case".into());
        };
        if variant != case || payload.len() != names.len() {
            return Err("native binding has an unexpected case or payload roster".into());
        }
        names
            .iter()
            .map(|name| {
                let mut values = payload.iter().filter(|(label, _)| label == name);
                let (_, value) = values
                    .next()
                    .ok_or("native binding is missing a payload field")?;
                if values.next().is_some() {
                    return Err("native binding repeats a payload field".into());
                }
                Ok(value)
            })
            .collect()
    }
    fn bytes(value: &Value, width: u64) -> Result<Vec<u8>, String> {
        let Value::Text(bytes) = value else {
            return Err("native locator coordinate must remain fixed bytes".into());
        };
        if u64::try_from(bytes.len()).ok() != Some(width) {
            return Err(
                "native locator byte count disagrees with its exact const application".into(),
            );
        }
        Ok(bytes.clone())
    }
    fn unused(width: u64) -> Result<(), String> {
        if width != 0 {
            return Err("unused native locator const coordinate must be zero".into());
        }
        Ok(())
    }
    let Value::Case { variant, .. } = value else {
        return Err("native binding must be a closed ForeignBinding case".into());
    };
    if variant == "Syscall" {
        for width in widths {
            unused(width)?;
        }
        let values = fields(value, "Syscall", &["number"])?;
        let Value::Int(number) = values[0] else {
            return Err("native syscall number must remain an integer".into());
        };
        return Ok(NativeBindingLocator::Syscall {
            number: u64::try_from(*number).map_err(|_| "native syscall number must fit u64")?,
        });
    }
    let imported = fields(value, "DllImport", &["import"])?;
    let imported = imported[0];
    let Value::Case { variant, .. } = imported else {
        return Err("native DllImport must retain its nominal case".into());
    };
    match variant.as_str() {
        "PeByName" => {
            unused(widths[2])?;
            let v = fields(imported, "PeByName", &["library", "export"])?;
            Ok(NativeBindingLocator::Import(
                ForeignLocatorCandidate::PeByName {
                    library: bytes(v[0], widths[0])?,
                    export: bytes(v[1], widths[1])?,
                },
            ))
        }
        "PeByOrdinal" => {
            unused(widths[1])?;
            unused(widths[2])?;
            let v = fields(imported, "PeByOrdinal", &["library", "ordinal"])?;
            let Value::Int(ordinal) = v[1] else {
                return Err("native ordinal must remain an integer".into());
            };
            let ordinal = u16::try_from(*ordinal).map_err(|_| "native ordinal must fit u16")?;
            if ordinal == 0 {
                return Err("native ordinal must be nonzero".into());
            }
            Ok(NativeBindingLocator::Import(
                ForeignLocatorCandidate::PeByOrdinal {
                    library: bytes(v[0], widths[0])?,
                    ordinal,
                },
            ))
        }
        "ElfVersioned" => {
            let v = fields(imported, "ElfVersioned", &["object", "symbol", "version"])?;
            Ok(NativeBindingLocator::Import(
                ForeignLocatorCandidate::ElfVersioned {
                    object: bytes(v[0], widths[0])?,
                    symbol: bytes(v[1], widths[1])?,
                    version: bytes(v[2], widths[2])?,
                },
            ))
        }
        "MachODylibSymbol" => {
            unused(widths[2])?;
            let v = fields(imported, "MachODylibSymbol", &["install_name", "symbol"])?;
            Ok(NativeBindingLocator::Import(
                ForeignLocatorCandidate::MachODylibSymbol {
                    install_name: bytes(v[0], widths[0])?,
                    symbol: bytes(v[1], widths[1])?,
                },
            ))
        }
        _ => Err("native binding has an unsupported DllImport case".into()),
    }
}
