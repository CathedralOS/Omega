/*
The reader's rejections: where it meets a program the specification says is
wrong, as opposed to one it does not read yet.

The reader types every expression itself, so it is the first place a
compilation learns that a value's type is not its destination's. Most such
differences are forms it cannot judge (two carriers it does not relate, a
view whose region it does not follow), and those stay refusals
(`crate::refuse`, "not supported yet"). A difference is a rejection
(`crate::reject`) only when the specification defines the error for every
program of that form, and each rule here names its sentence:

- `wiki/spec/language/domains.md`, "Literals and expected types": "the same
  rule governs arguments, field initializers, returns and arm results: the
  supplied expression's static type must carry exactly the domains the
  destination declares", and "Domain casts": "A declared type gains or loses
  a domain only through a cast that names that domain. No proof, expected
  type, literal, call boundary or join attaches or drops one." So one
  carrier with two domain sets is always an error, whatever the checker can
  prove about the value.
- `wiki/spec/language/lifetimes.md`, "Returned views": "An explicit result
  lifetime relates its reference leaves to the corresponding input lifetime
  ... A signature does not authorize returning unrelated storage", and the
  language has no "general authored outlives bounds". So a machine that
  returns a view of one of its own lifetimes where its result names another
  of its own lifetimes is always an error. Any other difference of regions
  (an elided lifetime, a lifetime a data declaration binds, a region not
  yet decided) stays a refusal.

Every other rejection is made where its rule is checked (`casts`,
`contracts`, a domain name nothing declares in `types`, an `invokes` row
naming nothing in `services`), through the same `crate::reject`.
*/

use semantic_vocabulary::MachineId;
use terminal_check_tables::{Construct, Refusal};

use super::Program;
use super::regions::Region;
use crate::body::{Policy, Qualification, ValueType};
use crate::{SourceOrigin, Unserved, refuse, reject};

/// What a value is supplied to, for the two rules above.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Destination {
    Parameter,
    /// The machine's result, from a returned value or an arm's value.
    Result,
    Field,
    Local,
    /// The place an assignment stores into.
    Assigned,
}

impl Destination {
    const fn named(self) -> &'static str {
        match self {
            Self::Parameter => "the parameter it is passed for",
            Self::Result => "the machine's result",
            Self::Field => "the field it initializes",
            Self::Local => "the local it initializes",
            Self::Assigned => "the place it is assigned to",
        }
    }
}

impl Program<'_> {
    /// The refusal for a value of type `found` supplied at `at` to a
    /// destination of type `expected`, which differ: a rejection when one of
    /// this file's rules makes the difference an error, else the refusal
    /// `unserved` names.
    pub(super) fn mistyped(
        &self,
        id: MachineId,
        at: SourceOrigin,
        destination: Destination,
        expected: Option<ValueType>,
        found: Option<ValueType>,
        unserved: &'static str,
    ) -> Refusal {
        let refused = refuse(id, Construct::Machine, Unserved::Typing(unserved));
        let (Some(expected), Some(found)) = (expected, found) else {
            return refused;
        };
        let (wanted, carried) = (expected.qualification(), found.qualification());
        let carrier = |value_type: ValueType| value_type.erased().qualified(Qualification::NONE);
        if wanted != carried && carrier(expected).is_some() && carrier(expected) == carrier(found) {
            return reject(
                id,
                at,
                format!(
                    "this value's type carries {} and {} declares {}: a type gains or loses a \
                     domain only through a cast that names the domain (`as +D`, `as -D`), never \
                     by where the value is supplied",
                    self.domains_named(carried),
                    destination.named(),
                    self.domains_named(wanted),
                ),
            );
        }
        if destination == Destination::Result
            && expected.erased() == found.erased()
            && self.names_two_own_lifetimes(expected, found)
        {
            return reject(
                id,
                at,
                "this view is of the region of another lifetime of the machine than its result \
                 names: a result lifetime relates the returned view to the input of that \
                 lifetime, and nothing relates two lifetimes of one machine",
            );
        }
        refused
    }

    /// Whether the two types, equal but for their regions, have at some
    /// position two different lifetime parameters of one machine. A
    /// parameter a data declaration binds is not one: it is replaced by the
    /// regions of the value it is read from (`regions`).
    fn names_two_own_lifetimes(&self, expected: ValueType, found: ValueType) -> bool {
        let declared = self.region_tuple(expected.regions());
        let returned = self.region_tuple(found.regions());
        declared.len() == returned.len()
            && declared.iter().zip(returned).any(|(declared, returned)| {
                match (self.resolved(*declared), self.resolved(*returned)) {
                    (Region::Parameter(declared), Region::Parameter(returned)) => {
                        declared.declaration == returned.declaration
                            && declared.ordinal != returned.ordinal
                            && self.data_declaration(declared.declaration).is_none()
                    }
                    _ => false,
                }
            })
    }

    /// The domains of `qualification` as a diagnostic names them.
    pub(super) fn domains_named(&self, qualification: Qualification) -> String {
        let mut names = Vec::new();
        match qualification.policy {
            Some(Policy::Wrapping) => names.push("`Wrapping`".to_owned()),
            Some(Policy::Saturating) => names.push("`Saturating`".to_owned()),
            None => {}
        }
        names.extend(
            qualification
                .domains
                .members()
                .map(|domain| self.domain_named(domain)),
        );
        if names.is_empty() {
            "no domain".to_owned()
        } else {
            names.join(" and ")
        }
    }

    /// One declared domain as a diagnostic names it.
    pub(super) fn domain_named(&self, domain: symbols::SymbolHandle) -> String {
        match self.typed.domain_definition(domain) {
            Some(definition) => format!("`{}`", definition.name.as_str()),
            None => "a domain with no declaration".to_owned(),
        }
    }
}
