/*
The reader's rejections: where it meets a program the specification says is
wrong, as opposed to one it does not read yet.

The reader computes each supported expression's type while resolving its
body, so receiving-type comparisons share this diagnostic owner. Differences
it cannot judge (a view whose region it does not follow, for example) stay
refusals
(`crate::refuse`, "not supported yet"). A difference is a rejection
(`crate::reject`) only when the specification defines the error for every
program of that form, and each rule here names its sentence:

- `wiki/spec/language/numeric_values.md`, "Integer equality and order":
  comparison "adds neither implicit arithmetic widening nor conversion at
  assignments, arguments, or returns", and "Literal landing and
  destinations": a suffixed literal retains its carrier through storage,
  arguments and returns. Once source and destination have scalar types,
  different carriers cannot agree by value or by sharing a storage width. Anonymous
  literal landing has already happened in the expression reader.
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

use semantic_vocabulary::{IeeeFloatFormat, IntegerSign, MachineId, ScalarType};
use terminal_check_tables::{Construct, Refusal};

use super::Program;
use super::regions::Region;
use crate::body::{Policy, Qualification, ValueType};
use crate::{SourceOrigin, Unserved, refuse, reject};

/// What a value is supplied to, for the rules above.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Destination {
    Parameter,
    /// The machine's result, from a returned value or an arm's value.
    Result,
    /// The common scalar type chosen by a match's destination or earlier arm.
    MatchArm,
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
            Self::MatchArm => "the match's common result type",
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
        if let (ValueType::Scalar(expected, _), ValueType::Scalar(found, _)) = (expected, found)
            && expected != found
        {
            return reject(
                id,
                at,
                format!(
                    "this value has scalar carrier `{}` but {} requires `{}`: there is no \
                     implicit scalar conversion at a receiving type",
                    scalar_named(found),
                    destination.named(),
                    scalar_named(expected),
                ),
            );
        }
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

/// The resolved scalar carrier, without domains or physical storage choices.
fn scalar_named(scalar: ScalarType) -> String {
    match scalar {
        ScalarType::Boolean => "bool".to_owned(),
        ScalarType::Integer(integer) if integer.is_address() => "addr".to_owned(),
        ScalarType::Integer(integer) => {
            let sign = match integer.sign() {
                IntegerSign::Signed => 'i',
                IntegerSign::Unsigned => 'u',
            };
            match integer.width() {
                semantic_vocabulary::IntegerWidth::Bits(bits) => format!("{sign}{bits}"),
                semantic_vocabulary::IntegerWidth::Parameter(_) => {
                    format!("{sign}<width parameter>")
                }
            }
        }
        ScalarType::IeeeFloat(IeeeFloatFormat::Binary32) => "f32".to_owned(),
        ScalarType::IeeeFloat(IeeeFloatFormat::Binary64) => "f64".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use semantic_vocabulary::{IntegerType, StructuralTypeId, ValueId, WidthParameter};
    use terminal_check_tables::{RefusalReason, values::RigidType};
    use typed_trees::TypedTrees;

    #[test]
    fn scalar_names_distinguish_closed_and_parameter_widths() {
        assert_eq!(
            scalar_named(ScalarType::Integer(
                IntegerType::new(IntegerSign::Unsigned, 1).unwrap()
            )),
            "u1",
        );
        assert_eq!(
            scalar_named(ScalarType::Integer(IntegerType::parameterized(
                IntegerSign::Signed,
                WidthParameter {
                    owner: MachineId::new(1).unwrap(),
                    value: ValueId::new(1).unwrap(),
                },
            ))),
            "i<width parameter>",
        );
    }

    #[test]
    fn distinct_scalar_carriers_reject_for_every_receiving_destination() {
        let typed = TypedTrees::default();
        let program = Program::new(&typed, false);
        let scalars = [
            ScalarType::Boolean,
            ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 1).unwrap()),
            ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 32).unwrap()),
            ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 32).unwrap()),
            ScalarType::Integer(IntegerType::supported_address_representation()),
            ScalarType::IeeeFloat(IeeeFloatFormat::Binary32),
            ScalarType::IeeeFloat(IeeeFloatFormat::Binary64),
        ];
        for destination in [
            Destination::Parameter,
            Destination::Result,
            Destination::MatchArm,
            Destination::Field,
            Destination::Local,
            Destination::Assigned,
        ] {
            for expected in scalars {
                for found in scalars {
                    if expected == found {
                        continue;
                    }
                    let refusal = program.mistyped(
                        MachineId::new(1).unwrap(),
                        SourceOrigin::Unpositioned(crate::Unpositioned::SpanAbsent),
                        destination,
                        Some(ValueType::plain(expected)),
                        Some(ValueType::plain(found)),
                        "test mismatch",
                    );
                    assert_eq!(refusal.reason, RefusalReason::Violation(None));
                    assert!(
                        refusal
                            .detail
                            .contains(&format!("`{}`", scalar_named(expected)))
                    );
                    assert!(
                        refusal
                            .detail
                            .contains(&format!("`{}`", scalar_named(found)))
                    );
                    assert!(refusal.detail.contains(destination.named()));
                }
            }
        }
        // A definite family mismatch still names an open integer carrier
        // without inventing its width or changing its parameter identity.
        let open_integer = ScalarType::Integer(IntegerType::parameterized(
            IntegerSign::Unsigned,
            semantic_vocabulary::WidthParameter {
                owner: MachineId::new(1).unwrap(),
                value: semantic_vocabulary::ValueId::new(1).unwrap(),
            },
        ));
        let refusal = program.mistyped(
            MachineId::new(1).unwrap(),
            SourceOrigin::Unpositioned(crate::Unpositioned::SpanAbsent),
            Destination::Result,
            Some(ValueType::plain(ScalarType::Boolean)),
            Some(ValueType::plain(open_integer)),
            "test mismatch",
        );
        assert_eq!(refusal.reason, RefusalReason::Violation(None));
        assert!(refusal.detail.contains("`u<width parameter>`"));
        assert!(refusal.detail.contains("`bool`"));
    }

    #[test]
    fn an_undecided_type_relationship_stays_unsupported() {
        let typed = TypedTrees::default();
        let program = Program::new(&typed, false);
        let boolean = Some(ValueType::plain(ScalarType::Boolean));
        let record = Some(ValueType::record(StructuralTypeId::new(1).unwrap()));
        for (expected, found) in [
            (None, boolean),
            (boolean, None),
            (boolean, Some(ValueType::Rigid(RigidType(1)))),
            (record, boolean),
            (
                record,
                Some(ValueType::record(StructuralTypeId::new(2).unwrap())),
            ),
        ] {
            let refusal = program.mistyped(
                MachineId::new(1).unwrap(),
                SourceOrigin::Unpositioned(crate::Unpositioned::SpanAbsent),
                Destination::Result,
                expected,
                found,
                "undecided relationship",
            );
            assert_eq!(refusal.reason, RefusalReason::Unsupported, "{refusal:?}");
            assert_eq!(refusal.detail, "typing undecided relationship");
        }
    }
}
