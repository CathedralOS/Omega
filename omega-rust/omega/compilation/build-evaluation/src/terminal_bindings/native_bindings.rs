//! Commit an owned typed locator through its original prepared Build request.

use super::{TerminalBuildBindingAdapter, activation_ticket, lock_state};
use language_semantics::{
    declaration_selection::{
        BuildOperation, BuildSelectionRequest, ExecutedBuildSelection, ExecutedNativeBinding,
        ProductTarget,
    },
    native_bindings::{NativeLocatorCase, NativeLocatorValue, decode_native_locator},
};
use psi_compiler::{TerminalBuildBindingOperation, TerminalBuildValueShape};
use semantic_vocabulary::{CanonicalStructuralPathSegment as Path, IntegerSign, IntegerValue};
use terminal_interpreter::{
    TerminalBoundaryStorageBuffer, TerminalBoundaryStorageValue, TerminalEffect,
    TerminalEffectRejection, TerminalEffectResult, TerminalScalarValue, TerminalStorageContents,
};

impl TerminalBuildBindingAdapter {
    pub(super) fn commit_native(
        &self,
        effect: &TerminalEffect,
        buffers: &[TerminalBoundaryStorageBuffer],
    ) -> Result<TerminalEffectResult, TerminalEffectRejection> {
        let refuse = TerminalEffectRejection::new;
        let TerminalEffect::BoundaryCall {
            operation,
            boundary,
            arguments,
            structural_arguments,
            ..
        } = effect
        else {
            return Err(refuse("not a native Build effect"));
        };
        let call = self
            .host
            .calls
            .iter()
            .find(|call| {
                call.operation == *operation
                    && call.boundary == *boundary
                    && call.action == TerminalBuildBindingOperation::BindNative
            })
            .ok_or_else(|| refuse("native Build call has no exact source-issued context"))?;
        let BuildSelectionRequest::Declaration(request) = &call.request else {
            return Err(refuse("native Build call has no designated requirement"));
        };
        if request.operation != BuildOperation::NativeBinding {
            return Err(refuse(
                "native Build call disagrees with its designated operation",
            ));
        }
        let schema = call
            .native_locator
            .as_ref()
            .ok_or_else(|| refuse("native Build call lost its checked locator carrier"))?;
        let [presentation] = structural_arguments.as_slice() else {
            return Err(refuse("native Build call requires one owned locator value"));
        };
        let [TerminalBoundaryStorageBuffer::Structural(buffer)] = buffers else {
            return Err(refuse(
                "native Build call needs its complete owned snapshot",
            ));
        };
        if buffer.argument_index() != 0
            || buffer.access() != terminal_psi::StructuralAccess::Owned
            || buffer.structural_type() != schema.structural_type
            || presentation.structural_type != schema.structural_type
        {
            return Err(refuse(
                "native Build snapshot does not match its checked owned carrier",
            ));
        }
        if !buffer
            .values()
            .windows(2)
            .all(|pair| pair[0].path < pair[1].path)
        {
            return Err(refuse(
                "native locator snapshot paths are duplicated or unordered",
            ));
        }
        let mut state = lock_state(&self.host.state).map_err(refuse)?;
        // The structural value has been authenticated independently above;
        // the common ticket owner still compares the exact source request.
        let (ticket, values) = activation_ticket(arguments, &[], &state, &call.request)?;
        let [
            TerminalScalarValue::Integer {
                value: IntegerValue::Unsigned(code),
                ..
            },
        ] = values
        else {
            return Err(refuse("native Build call needs its checked target value"));
        };
        let target = u64::try_from(*code)
            .ok()
            .and_then(ProductTarget::from_code)
            .ok_or_else(|| refuse("native Build call names an unknown catalog target"))?;
        let selecting_machine = call
            .selecting_machine
            .ok_or_else(|| refuse("native Build call lost its selecting machine"))?;
        let bytes = schema
            .widths
            .into_iter()
            .try_fold(0u64, |total, width| total.checked_add(width))
            .ok_or_else(|| refuse("native locator retained-byte count overflowed"))?;
        let storage = self
            .host
            .account
            .as_ref()
            .map(|account| account.reserve_storage(1, bytes))
            .transpose()
            .map_err(|error| {
                TerminalEffectRejection::new(format!("native locator retained storage: {error}"))
            })?;
        let locator = decode_native_locator(
            ValueRef {
                shape: &schema.shape,
                values: buffer.values(),
                path: Vec::new(),
            },
            schema.widths,
        )
        .map_err(TerminalEffectRejection::new)?;
        state
            .executed
            .try_reserve(1)
            .map_err(|_| refuse("cannot retain native Build intent"))?;
        if storage.is_some() {
            state
                .native_storage
                .try_reserve(1)
                .map_err(|_| refuse("cannot retain native Build storage account"))?;
        }
        if let Some(account) = &self.host.account {
            account.charge_result_custody(1, bytes).map_err(|error| {
                TerminalEffectRejection::new(format!("native Build result custody: {error}"))
            })?;
        }
        state
            .executed
            .push(ExecutedBuildSelection::Native(ExecutedNativeBinding {
                request: request.clone(),
                target,
                locator,
                selecting_machine,
            }));
        if let Some(storage) = storage {
            state.native_storage.push(storage);
        }
        state.pending[ticket] = None;
        Ok(TerminalEffectResult::Unit)
    }
}

#[derive(Clone)]
struct ValueRef<'value> {
    shape: &'value TerminalBuildValueShape,
    values: &'value [TerminalBoundaryStorageValue],
    path: Vec<Path>,
}
impl ValueRef<'_> {
    fn observation(&self) -> Result<TerminalStorageContents, String> {
        self.values
            .binary_search_by(|value| value.path.as_slice().cmp(self.path.as_slice()))
            .map(|index| self.values[index].contents)
            .map_err(|_| "native locator snapshot omitted an actual semantic cell".to_owned())
    }
}
impl NativeLocatorValue for ValueRef<'_> {
    fn case(&self) -> Result<NativeLocatorCase<'_, Self>, String> {
        let shape = self.shape;
        let TerminalBuildValueShape::Sum { cases } = shape else {
            return Err("native locator expected its checked sum shape".into());
        };
        let TerminalStorageContents::Case(selected) = self.observation()? else {
            return Err("native locator has no observed selected case".into());
        };
        let case = cases
            .iter()
            .find(|case| case.id == selected)
            .ok_or("native locator selected a case outside its checked carrier")?;
        Ok(NativeLocatorCase {
            name: &case.name,
            fields: case
                .fields
                .iter()
                .map(|field| {
                    let mut path = self.path.clone();
                    path.push(Path::Case(selected));
                    path.push(Path::Field(field.id));
                    (
                        field.name.as_str(),
                        Self {
                            shape: &field.shape,
                            values: self.values,
                            path,
                        },
                    )
                })
                .collect(),
        })
    }
    fn integer(&self) -> Result<i128, String> {
        let TerminalBuildValueShape::Scalar(expected) = self.shape else {
            return Err("native locator expected its checked integer carrier".into());
        };
        let TerminalStorageContents::Scalar {
            value: TerminalScalarValue::Integer { scalar_type, value },
            stored_scalar,
        } = self.observation()?
        else {
            return Err("native locator field did not observe an integer".into());
        };
        if *expected != semantic_vocabulary::ScalarType::Integer(scalar_type)
            || stored_scalar != *expected
        {
            return Err("native locator integer carrier drifted from its checked type".into());
        }
        match value {
            IntegerValue::Signed(value) => Ok(value),
            IntegerValue::Unsigned(value) => i128::try_from(value)
                .map_err(|_| "native locator integer exceeds its semantic carrier".into()),
        }
    }
    fn bytes(&self, width: u64, label: &str) -> Result<Vec<u8>, String> {
        let TerminalBuildValueShape::Array { length, element } = self.shape else {
            return Err(format!("{label} has no owned fixed-byte array carrier"));
        };
        if *length != width
            || !matches!(element.as_ref(), TerminalBuildValueShape::Scalar(semantic_vocabulary::ScalarType::Integer(carrier)) if carrier.bits() == 8 && carrier.sign() == IntegerSign::Unsigned && !carrier.is_address())
        {
            return Err(format!(
                "{label} width or element carrier differs from its checked native coordinate"
            ));
        }
        let count =
            usize::try_from(width).map_err(|_| format!("{label} does not fit host storage"))?;
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(count)
            .map_err(|_| format!("{label} allocation was refused"))?;
        let mut child = Self {
            shape: element,
            values: self.values,
            path: self.path.clone(),
        };
        for index in 0..width {
            child.path.push(Path::FixedIndex(index));
            let value = child.integer()?;
            child.path.pop();
            bytes.push(
                u8::try_from(value).map_err(|_| format!("{label} contains a value outside u8"))?,
            );
        }
        Ok(bytes)
    }
}
