use crate::ValueType;
use omega_syntax::NodeKind;

impl omega_target_contract::LayoutSubject for ValueType {
    fn has_data_layout(&self) -> bool {
        match self {
            ValueType::Nominal(id) | ValueType::Applied(id, _) => id.kind == NodeKind::Data,
            ValueType::Callable(contract) => contract.closed_runtime,
            ValueType::Qualified(ty, _) | ValueType::AtLifetime(ty, _) => ty.has_data_layout(),
            ValueType::Bool
            | ValueType::Address
            | ValueType::Integer { width: Some(_), .. }
            | ValueType::Float32
            | ValueType::Float64
            | ValueType::Tuple(_)
            | ValueType::Reference(..)
            | ValueType::Array(..)
            | ValueType::Slice(_) => true,
            _ => false,
        }
    }
}
