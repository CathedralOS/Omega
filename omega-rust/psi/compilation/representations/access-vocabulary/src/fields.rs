#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record)]
pub enum ObservationModel {
    Stable,
    External,
    Atomic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record)]
pub enum AccessExposure {
    Exported,
    BindingPrivate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record)]
pub enum ExternalRead {
    None,
    Read,
    Take,
}

#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record,
)]
pub struct AtomicPermissions {
    pub load: bool,
    pub store: bool,
    pub fetch_add: bool,
    pub fetch_sub: bool,
    pub fetch_xor: bool,
    pub fetch_or: bool,
    pub fetch_and: bool,
    pub swap: bool,
    pub compare_exchange: bool,
    pub compare_exchange_once: bool,
    pub try_exchange: bool,
    pub try_exchange_once: bool,
}

impl AtomicPermissions {
    pub const fn any(self) -> bool {
        self.load
            || self.store
            || self.fetch_add
            || self.fetch_sub
            || self.fetch_xor
            || self.fetch_or
            || self.fetch_and
            || self.swap
            || self.compare_exchange
            || self.compare_exchange_once
            || self.try_exchange
            || self.try_exchange_once
    }

    pub const fn contains(self, required: Self) -> bool {
        (!required.load || self.load)
            && (!required.store || self.store)
            && (!required.fetch_add || self.fetch_add)
            && (!required.fetch_sub || self.fetch_sub)
            && (!required.fetch_xor || self.fetch_xor)
            && (!required.fetch_or || self.fetch_or)
            && (!required.fetch_and || self.fetch_and)
            && (!required.swap || self.swap)
            && (!required.compare_exchange || self.compare_exchange)
            && (!required.compare_exchange_once || self.compare_exchange_once)
            && (!required.try_exchange || self.try_exchange)
            && (!required.try_exchange_once || self.try_exchange_once)
    }
}

#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record,
)]
pub struct AccessPermissions {
    pub read: bool,
    pub take: bool,
    pub write: bool,
    pub atomic: AtomicPermissions,
}

impl AccessPermissions {
    pub const fn any(self) -> bool {
        self.read || self.take || self.write || self.atomic.any()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, compiler_storage::Record)]
pub enum FieldAccess {
    Inaccessible,
    Region {
        byte_length: u64,
        byte_alignment: u64,
        transfer_width_bits: u16,
        observation: ObservationModel,
        permissions: AccessPermissions,
        exposure: AccessExposure,
    },
    Stable {
        transfer_width_bits: u16,
        read: bool,
        write: bool,
        exposure: AccessExposure,
    },
    External {
        transfer_width_bits: u16,
        read: ExternalRead,
        write: bool,
        exposure: AccessExposure,
    },
    Atomic {
        transfer_width_bits: u16,
        operations: AtomicPermissions,
        exposure: AccessExposure,
    },
}

impl FieldAccess {
    pub fn operation_names(&self) -> Vec<&'static str> {
        match self {
            FieldAccess::Inaccessible | FieldAccess::Atomic { .. } => Vec::new(),
            FieldAccess::Region { permissions, .. } => {
                ordinary_operation_names(permissions.read, permissions.take, permissions.write)
            }
            FieldAccess::Stable { read, write, .. } => {
                ordinary_operation_names(*read, false, *write)
            }
            FieldAccess::External { read, write, .. } => ordinary_operation_names(
                *read == ExternalRead::Read,
                *read == ExternalRead::Take,
                *write,
            ),
        }
    }
}

fn ordinary_operation_names(read: bool, take: bool, write: bool) -> Vec<&'static str> {
    [("read", read), ("take", take), ("write", write)]
        .into_iter()
        .filter_map(|(name, admitted)| admitted.then_some(name))
        .collect()
}
