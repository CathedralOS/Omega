//! Follow a parked writable home's descriptors through the stores that
//! replaced them, reading published Psi alone.
//!
//! `let mut h = Holder{r: first}; h.r = second;` constructs the home and then
//! replaces one shared descriptor field
//! (`wiki/spec/language/lifetimes.md#carried-loan-transport`). When a later
//! call parks with `h` live, the owner row must say that `h.r` now holds the
//! loan of `second`. This reader replays the home's construction and stores in
//! body order and compares the result with each published embedded loan.
use omega_psi::{
    CallSite, EmbeddedLoanSource, LoweredPackage, OperationId, OperationKind, Parameter, PlaceId, PlaceProjection, StructuralStoreDisposition, ValueOperand,
};

use super::machine;

/// One descriptor in the parked home, with the store that last supplied it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StoredDescriptor {
    pub path: Vec<PlaceProjection>,
    /// The incoming formal whose loan the field holds.
    pub formal: usize,
    /// The field store that installed it, or `None` for the construction.
    pub store: Option<OperationId>,
}

/// For the one parked call of `caller`, each embedded loan of its one owner row.
pub fn parked_field_stores(
    package: &LoweredPackage,
    caller: &str,
) -> Result<Vec<StoredDescriptor>, &'static str> {
    let body = &machine(package, caller).ok_or("missing caller")?.body;
    let [site] = body.suspension_sites.as_slice() else {
        return Err("expected one parked call");
    };
    let CallSite::Operation(call) = site.call else {
        return Err("expected an ordinary parked call");
    };
    let [owner] = site.retained_owners.as_slice() else {
        return Err("expected one parked owner");
    };
    if !body.mutable_homes.contains(&owner.place) {
        return Err("expected a writable home");
    }
    // Replay the home: (path, source place, store) rows, newest last.
    let mut contents: Vec<(Vec<PlaceProjection>, PlaceId, Option<OperationId>)> = Vec::new();
    for op in &body.operations[..call.index()] {
        match &op.kind {
            OperationKind::EstablishRecord { fields } if op.result == omega_psi::OperationResult::Structural(owner.place) => {
                for field in fields {
                    let ValueOperand::CopyWhole(source) = field.operand else {
                        return Err("this reader follows copied descriptor initializers only");
                    };
                    contents.push((vec![PlaceProjection::Field(field.field.clone())], source, None));
                }
            }
            OperationKind::StructuralStore {
                destination,
                value,
                disposition,
            } if destination.root == owner.place => {
                if destination.path.is_empty() || destination.path.contains(&PlaceProjection::Referent) {
                    return Err("this reader follows field stores into the home only");
                }
                if *disposition != StructuralStoreDisposition::ReplaceUnrestricted {
                    return Err("a shared descriptor store displaces freely discardable content");
                }
                contents.retain(|(path, _, _)| !path.starts_with(&destination.path));
                contents.push((destination.path.clone(), *value, Some(op.id)));
            }
            OperationKind::BorrowPlace { source, access } if source.root == owner.place && *access != omega_psi::ReferenceAccess::Shared => {
                return Err("a writable alias of the home hides later stores");
            }
            _ => {}
        }
    }
    let mut rows = Vec::new();
    for loan in &owner.embedded_loans {
        let (_, source, store) = contents
            .iter()
            .rev()
            .find(|(path, _, _)| *path == loan.path)
            .ok_or("an embedded loan has no reaching store or initializer")?;
        let formal = body
            .parameters
            .iter()
            .position(|p| *p == Parameter::Structural(*source))
            .ok_or("this reader expects a descriptor copied from an incoming formal")?;
        let [origin] = loan.origins.as_slice() else {
            return Err("the published embedded loan names a different source than the reaching store");
        };
        let incoming = EmbeddedLoanSource::Formal {
            formal: *source,
            path: Vec::new(),
        };
        if origin.source != incoming || !origin.arrival.is_empty() || !origin.reborrows.is_empty() {
            return Err("the published embedded loan names a different source than the reaching store");
        }
        rows.push(StoredDescriptor {
            path: loan.path.clone(),
            formal,
            store: *store,
        });
    }
    Ok(rows)
}
