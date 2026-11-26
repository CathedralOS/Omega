#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, record_storage::Record)]
pub enum ObservationModel {
    Stable,
    External,
    Atomic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, record_storage::Record)]
pub enum AccessExposure {
    Exported,
    BindingPrivate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, record_storage::Record)]
pub enum ExternalRead {
    None,
    Read,
    Take,
}

#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, record_storage::Record,
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
    /// Every permission in field order; access-plan fingerprints hash this order.
    pub const fn bits(self) -> [bool; 12] {
        [
            self.load,
            self.store,
            self.fetch_add,
            self.fetch_sub,
            self.fetch_xor,
            self.fetch_or,
            self.fetch_and,
            self.swap,
            self.compare_exchange,
            self.compare_exchange_once,
            self.try_exchange,
            self.try_exchange_once,
        ]
    }

    pub const fn any(self) -> bool {
        let bits = self.bits();
        let mut index = 0;
        while index < bits.len() {
            if bits[index] {
                return true;
            }
            index += 1;
        }
        false
    }

    pub const fn contains(self, required: Self) -> bool {
        let (granted, required) = (self.bits(), required.bits());
        let mut index = 0;
        while index < granted.len() {
            if required[index] && !granted[index] {
                return false;
            }
            index += 1;
        }
        true
    }
}

#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, record_storage::Record,
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

#[derive(Debug, Clone, PartialEq, Eq, Hash, record_storage::Record)]
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
