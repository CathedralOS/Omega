//! The exact per-operation law a binding's selected codec enforces.
//!
//! `frame` owns the bounded envelope every codec shares — the 4 KiB cap,
//! the operation/length header, the no-trailing-bytes rule. It does not
//! know which operations a contract offers or what payload each carries;
//! that is the per-contract operation schema this module holds. The
//! mediator checks a delivered frame against the *destination* endpoint's
//! schema: a request must name an operation the export's contract offers
//! with the payload law it assigns, and a response must satisfy the
//! import's demanded contract on the way back.
//!
//! Schemas are registered by contract identity at preparation, before any
//! endpoint exists. A bound endpoint whose contract has no registered
//! schema rejects the installation outright — a channel the codec cannot
//! check is never carried live. The schema values themselves are what the
//! selected codec would generate from the contract; this crate enforces
//! them, it does not derive them.

use super::frame::Frame;
use crate::deployment_plan::Identity;
use std::collections::BTreeMap;
use std::fmt;

/// The payload law one operation obeys on its contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PayloadSchema {
    /// The frame carries no payload.
    Empty,
    /// The payload is exactly `bytes` long.
    Exact { bytes: usize },
    /// The payload is at most `bytes` long.
    Bounded { bytes: usize },
}

impl PayloadSchema {
    fn accepts(&self, length: usize) -> bool {
        match self {
            Self::Empty => length == 0,
            Self::Exact { bytes } => length == *bytes,
            Self::Bounded { bytes } => length <= *bytes,
        }
    }
}

impl fmt::Display for PayloadSchema {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => formatter.write_str("no payload"),
            Self::Exact { bytes } => write!(formatter, "exactly {bytes} payload bytes"),
            Self::Bounded { bytes } => write!(formatter, "at most {bytes} payload bytes"),
        }
    }
}

/// Why a decoded frame violates a contract's operation schema.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SchemaViolation {
    /// The contract does not offer this operation.
    UnlistedOperation { operation: u32 },
    /// The payload length breaks the operation's law.
    Payload {
        operation: u32,
        expected: PayloadSchema,
        found: usize,
    },
}

impl fmt::Display for SchemaViolation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnlistedOperation { operation } => {
                write!(
                    formatter,
                    "operation {operation} is not offered by this contract"
                )
            }
            Self::Payload {
                operation,
                expected,
                found,
            } => write!(
                formatter,
                "operation {operation} expects {expected}, found {found}"
            ),
        }
    }
}

impl std::error::Error for SchemaViolation {}

/// One contract's complete operation table. An operation absent from the
/// table is invalid on the wire — the schema is closed, not a prefix.
#[derive(Debug, Clone)]
pub struct OperationSchema {
    operations: BTreeMap<u32, PayloadSchema>,
}

impl OperationSchema {
    /// The contract's exact `(operation, payload law)` table.
    pub fn new(operations: impl IntoIterator<Item = (u32, PayloadSchema)>) -> Self {
        Self {
            operations: operations.into_iter().collect(),
        }
    }

    fn check(&self, frame: &Frame) -> Result<(), SchemaViolation> {
        let Some(law) = self.operations.get(&frame.operation) else {
            return Err(SchemaViolation::UnlistedOperation {
                operation: frame.operation,
            });
        };
        if !law.accepts(frame.payload.len()) {
            return Err(SchemaViolation::Payload {
                operation: frame.operation,
                expected: *law,
                found: frame.payload.len(),
            });
        }
        Ok(())
    }
}

/// Why registration refused a schema.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SchemaRegistrationError {
    /// A different schema already governs this contract — registering a
    /// second table for one contract identity is a substitution attempt.
    ContractAlreadyRegistered { contract: Identity },
}

impl fmt::Display for SchemaRegistrationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ContractAlreadyRegistered { .. } => {
                formatter.write_str("an operation schema already governs this contract")
            }
        }
    }
}

impl std::error::Error for SchemaRegistrationError {}

/// The operation schemas admitted for one installation, keyed by the exact
/// contract identity each endpoint declares. Requests are checked against
/// the export end's contract; responses against the import end's.
#[derive(Debug, Default)]
pub struct OperationSchemas {
    by_contract: BTreeMap<Identity, OperationSchema>,
}

impl OperationSchemas {
    pub fn new() -> Self {
        Self::default()
    }

    /// Bind `schema` to `contract`. Re-registering a contract refuses: two
    /// operation tables for one identity is substitution, not versioning.
    pub fn register(
        &mut self,
        contract: Identity,
        schema: OperationSchema,
    ) -> Result<(), SchemaRegistrationError> {
        if self.by_contract.contains_key(&contract) {
            return Err(SchemaRegistrationError::ContractAlreadyRegistered { contract });
        }
        self.by_contract.insert(contract, schema);
        Ok(())
    }

    pub(crate) fn for_contract(&self, contract: &Identity) -> Option<&OperationSchema> {
        self.by_contract.get(contract)
    }

    /// Check `frame` against `contract`'s table. A contract with no
    /// registered schema rejects — preparation refuses to bind such
    /// endpoints, so reaching here without one is already a substitution.
    pub(crate) fn check(&self, contract: &Identity, frame: &Frame) -> Result<(), SchemaViolation> {
        match self.for_contract(contract) {
            Some(schema) => schema.check(frame),
            None => Err(SchemaViolation::UnlistedOperation {
                operation: frame.operation,
            }),
        }
    }
}
