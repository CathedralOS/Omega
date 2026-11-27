use crate::read::refuse_at;
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
- `wiki/spec/language/generics.md`, "Static type equality": equal layout
  or bytes do not identify different nominal types. A value of one declared
  data type cannot supply a destination of another declared data type.
  Under "Closed applications and canonical identity", the value of a closed
  const argument also identifies the application. Two different values for
  the same binder distinguish the types.
- `wiki/spec/language/lifetimes.md`, "Returned views": "An explicit result
  lifetime relates its reference leaves to the corresponding input lifetime
  ... A signature does not authorize returning unrelated storage", and the
  language has no "general authored outlives bounds". So a machine that
  returns a view of one of its own lifetimes where its result names another
  of its own lifetimes is always an error. Any other difference of regions
  (an elided lifetime, a lifetime a data declaration binds, a region not
  yet decided) stays a refusal. A loan of the machine's own storage is also
  known: "A view of local storage does not [qualify], even when empty" under
  caller-chosen result lifetimes. It cannot establish an independently declared
  machine lifetime at any receiving position.

Every other rejection is made where its rule is checked (`casts`,
`contracts`, a domain name nothing declares in `types`, an `invokes` row
naming nothing in `services`), through the same `crate::reject`.
*/

use semantic_vocabulary::{IeeeFloatFormat, IntegerSign, IntegerWidth, MachineId, ScalarType};
use terminal_check_tables::Refusal;

use super::Program;
use super::regions::Region;
use crate::body::{Policy, Qualification, ValueType};
use crate::{SourceOrigin, Unserved, reject};

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
    /// The element type of the array an array literal constructs.
    ArrayElement,
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
            Self::ArrayElement => "the array element it initializes",
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
        let refused = refuse_at(id, Unserved::Typing(unserved));
        let (Some(expected), Some(found)) = (expected, found) else {
            return refused;
        };
        let array_length = |ty| {
            let ValueType::Record(row, _, _) = ty else {
                return None;
            };
            self.records.get(row)?.shape.fixed_array_extent()?.literal()
        };
        if let (Some(wanted), Some(actual)) = (array_length(expected), array_length(found))
            && wanted != actual
        {
            return reject(
                id,
                at,
                format!(
                    "{} expects an array with {wanted} elements but the supplied array has {actual}",
                    destination.named()
                ),
            );
        }
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
        if let Some(reason) = self.different_static_arguments(destination, expected, found) {
            return reject(id, at, reason);
        }
        let nominal = |value| {
            let ValueType::Record(record, _, _) = value else {
                return None;
            };
            if !matches!(
                self.records.get(record)?.shape,
                crate::StorageShape::Record(_)
                    | crate::StorageShape::Sum(_)
                    | crate::StorageShape::Opaque
            ) {
                return None;
            }
            let symbol = self.record_sources.get(record)?.declaration;
            let declaration = self.typed.unique_data_definition_by_symbol(symbol)?;
            if declaration.generic_instance.is_some() {
                return None;
            }
            Some((symbol, self.typed.normalized_nominal_type_identity(symbol)?))
        };
        if self.is_borrowed_view(expected)
            && let Some((carried, _)) = nominal(found)
        {
            return reject(
                id,
                at,
                format!(
                    "{} expects a slice, but the supplied data type is `{}`",
                    destination.named(),
                    self.typed.symbols.display_path(carried, "::"),
                ),
            );
        }
        if let (Some((wanted, wanted_identity)), Some((carried, carried_identity))) =
            (nominal(expected), nominal(found))
            && wanted_identity != carried_identity
        {
            return reject(
                id,
                at,
                format!(
                    "{} expects the `{}` data type but got `{}`, and these are incompatible data types because their declarations differ",
                    destination.named(),
                    self.typed.symbols.display_path(wanted, "::"),
                    self.typed.symbols.display_path(carried, "::"),
                ),
            );
        }
        if expected.erased() == found.erased()
            && self.names_own_storage_as_declared_lifetime(id, expected, found)
        {
            return reject(
                id,
                at,
                format!(
                    "this view borrows storage owned by the current invocation, but {} requires \
                     one of the machine's declared lifetimes: local storage cannot establish \
                     that promise, even for an empty view",
                    destination.named(),
                ),
            );
        }
        // Two lifetime parameters of one machine are unrelated wherever the
        // value goes: a result, a field of a record literal, a local or an
        // argument that names the other lifetime.
        if expected.erased() == found.erased() && self.names_two_own_lifetimes(expected, found) {
            return reject(
                id,
                at,
                match destination {
                    Destination::Result => String::from(
                        "this view is of the region of another lifetime of the machine than its \
                         result names: a result lifetime relates the returned view to the input \
                         of that lifetime, and nothing relates two lifetimes of one machine",
                    ),
                    other => format!(
                        "this view is of the region of another lifetime of the machine than {} \
                         names: nothing relates two lifetimes of one machine",
                        other.named()
                    ),
                },
            );
        }
        refused
    }

    /// Compare closed arguments of the same declaration in their supplying scopes.
    /// An unresolved argument or reference region can still require proof.
    fn different_static_arguments(
        &self,
        destination: Destination,
        expected: ValueType,
        found: ValueType,
    ) -> Option<String> {
        use crate::source_state::{data::TypeParameterKind, types::TypeReferenceNode};

        let applied = |value| {
            let ValueType::Record(row, _, _) = value else {
                return None;
            };
            let source = self.record_sources.get(row)?.application.as_ref()?;
            let table = &self.typed.type_reference_table;
            let mut reference = table.unconstrained_type_reference(source.reference);
            if let TypeReferenceNode::Named { symbol, .. } = table.type_reference(reference) {
                reference = self
                    .typed
                    .unique_data_definition_by_symbol(*symbol)?
                    .generic_instance?;
            }
            let TypeReferenceNode::Generic {
                base_symbol,
                arguments,
                ..
            } = table.type_reference(reference)
            else {
                return None;
            };
            Some((
                reference,
                *base_symbol,
                table.type_reference_handles(*arguments),
                &source.scope,
            ))
        };
        let (wanted, family, wanted_args, wanted_scope) = applied(expected)?;
        let (carried, other_family, carried_args, carried_scope) = applied(found)?;
        let definition = self.typed.unique_data_definition_by_symbol(family)?;
        let parameters = self.typed.data_type_parameters(definition);
        if family != other_family
            || wanted_args.len() != parameters.len()
            || carried_args.len() != parameters.len()
        {
            return None;
        }
        let implementation = |reference| {
            let selected = self
                .typed
                .static_operand_from_type_reference(reference)
                .ok()?
                .symbol;
            (selected.is_valid()
                && self.typed.symbols.get(selected).kind == symbols::SymbolKind::Conformance)
                .then(|| self.typed.normalized_nominal_type_identity(selected))
                .flatten()
        };
        for ((parameter, wanted_arg), carried_arg) in
            parameters.iter().zip(wanted_args).zip(carried_args)
        {
            let (Ok(left), Ok(right)) = (
                wanted_scope.supplied_type_reference(self.typed, *wanted_arg),
                carried_scope.supplied_type_reference(self.typed, *carried_arg),
            ) else {
                continue;
            };
            match parameter.kind {
                TypeParameterKind::Type => {
                    let closed_type = |source: &crate::source_state::SourceTypeApplication| {
                        crate::source_state::domain::index_argument_is_closed(
                            self.typed,
                            source.reference,
                        )
                        .then(|| {
                            source
                                .scope
                                .region_free_type_identity(self.typed, source.reference)
                        })
                        .flatten()
                    };
                    if let (Some(wanted), Some(carried)) = (closed_type(&left), closed_type(&right))
                        && wanted != carried
                    {
                        return Some(format!(
                            "{} requires `{}` with `{} = {}`, but this value carries `{}`",
                            destination.named(),
                            definition.name.as_str(),
                            parameter.name.as_str(),
                            self.typed
                                .display_type_reference_with_constraints(left.reference),
                            self.typed
                                .display_type_reference_with_constraints(right.reference),
                        ));
                    }
                }
                TypeParameterKind::Implementation => {
                    if let (Some(left), Some(right)) = (
                        implementation(left.reference),
                        implementation(right.reference),
                    ) && left != right
                    {
                        return Some(format!(
                            "{} expects `{}` but got `{}`; these types select different implementations",
                            destination.named(),
                            self.typed.display_type_reference(wanted),
                            self.typed.display_type_reference(carried),
                        ));
                    }
                }
                TypeParameterKind::Const { type_reference }
                | TypeParameterKind::Value { type_reference } => {
                    if let Some((expected, actual)) =
                        self.different_const_values(type_reference, left.reference, right.reference)
                    {
                        return Some(format!(
                            "type mismatch: {} requires `{}` with `{} = {expected}` but this value supplies {actual}",
                            destination.named(),
                            definition.name.as_str(),
                            parameter.name.as_str(),
                        ));
                    }
                }
                _ => {}
            }
        }
        None
    }

    fn different_const_values(
        &self,
        carrier: crate::source_state::types::TypeReferenceHandle,
        expected: crate::source_state::types::TypeReferenceHandle,
        found: crate::source_state::types::TypeReferenceHandle,
    ) -> Option<(String, String)> {
        use crate::source_state::expression::ExpressionNode;

        let carrier = self.typed.primitive_type_reference(carrier)?;
        let value = |reference| {
            let operand = self
                .typed
                .static_operand_from_type_reference(reference)
                .ok()?;
            self.typed
                .prepare_callable_scalar_argument(carrier, &operand)
                .ok()
        };
        match (value(expected)?, value(found)?) {
            (ExpressionNode::Integer(left), ExpressionNode::Integer(right)) => {
                let (left, right) = (left.value_bignum()?, right.value_bignum()?);
                (left != right).then(|| (left.to_string(), right.to_string()))
            }
            (ExpressionNode::Boolean(left), ExpressionNode::Boolean(right)) => {
                (left != right).then(|| (left.to_string(), right.to_string()))
            }
            _ => None,
        }
    }

    /// Only an exact local-storage ceiling supplies this error. Unknown fixed
    /// regions and unresolved loan classes retain their ordinary refusal.
    fn names_own_storage_as_declared_lifetime(
        &self,
        id: MachineId,
        expected: ValueType,
        found: ValueType,
    ) -> bool {
        let Some(symbol) = self.symbols.get(id) else {
            return false;
        };
        let declared = self.region_tuple(expected.regions());
        let supplied = self.region_tuple(found.regions());
        declared.len() == supplied.len()
            && declared.iter().zip(supplied).any(|(declared, supplied)| {
                matches!(
                    (self.resolved(*declared), self.named_ceiling(*supplied)),
                    (Region::Parameter(parameter), Region::Storage { machine, .. })
                        if parameter.declaration == *symbol && machine == id
                )
            })
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
                match (self.resolved(*declared), self.named_ceiling(*returned)) {
                    (Region::Parameter(declared), Region::Parameter(returned)) => {
                        declared.declaration == returned.declaration
                            && declared.ordinal != returned.ordinal
                            && self.data_declaration(declared.declaration).is_none()
                    }
                    _ => false,
                }
            })
    }

    /// The rejection when exactly one side of a machine's result is Unit.
    /// A machine that declares no result type completes with Unit, so a
    /// value it supplies as an arm or final value has nowhere to go, and a
    /// call of such a machine supplies no value to a machine that declares
    /// one. Result compatibility is the ordinary call rule
    /// (`wiki/spec/language/state_contracts.md`, "Terminal calls in
    /// transition arms"). The diagnostic names the machine whose header
    /// lacks the result type, because that header is what the author must
    /// change, even when the mismatch is found in its caller.
    pub(super) fn unit_result_mismatch(
        &self,
        id: MachineId,
        expected: Option<ValueType>,
        supplied: &crate::body::Expression,
    ) -> Option<Refusal> {
        let machine = self.machine_named(id);
        match (expected, supplied.result) {
            (None, Some(found)) => {
                let (found, hint) = match self.value_type_named(found) {
                    Some(name) => (format!("`{name}`"), format!(" (`-> {name}`)")),
                    None => (String::from("other than Unit"), String::new()),
                };
                Some(reject(
                    id,
                    supplied.source,
                    format!(
                        "machine `{machine}` declares no result type, so it completes with \
                         Unit, but this value has type {found}: declare the result in its \
                         header{hint}"
                    ),
                ))
            }
            (Some(expected), None) => {
                let declared = self
                    .value_type_named(expected)
                    .map_or_else(String::new, |name| format!(" `{name}`"));
                let callee = match &supplied.kind {
                    crate::body::ExpressionKind::Call {
                        callee: crate::body::CallTarget::Machine(callee),
                        ..
                    } => Some(self.machine_named(*callee)),
                    _ => None,
                };
                Some(reject(
                    id,
                    supplied.source,
                    match callee {
                        Some(callee) => format!(
                            "machine `{machine}` declares the result type{declared}, but this \
                             value is the result of `{callee}`, which declares no result type \
                             and so completes with Unit: declare the result in `{callee}`'s \
                             header"
                        ),
                        None => format!(
                            "machine `{machine}` declares the result type{declared}, but this \
                             value is Unit"
                        ),
                    },
                ))
            }
            _ => None,
        }
    }

    /// The machine `id` as a diagnostic names it.
    fn machine_named(&self, id: MachineId) -> String {
        self.symbols
            .get(id)
            .copied()
            .filter(|symbol| symbol.is_valid())
            .map_or_else(
                || String::from("<unnamed>"),
                |symbol| self.typed.symbols.display_path(symbol, "::"),
            )
    }

    /// A value type as a diagnostic names it, when its name is known.
    fn value_type_named(&self, value_type: ValueType) -> Option<String> {
        match value_type {
            ValueType::Scalar(scalar, _) => Some(scalar_named(scalar)),
            ValueType::Record(row, _, _) => match self.record_sources.get(row) {
                Some(source) if source.declaration.is_valid() => {
                    Some(self.typed.symbols.display_path(source.declaration, "::"))
                }
                _ => self.records.get(row).map(|record| record.identity.clone()),
            },
            ValueType::Rigid(_) => None,
        }
    }

    /// The domains of `qualification` as a diagnostic names them.
    pub(super) fn domains_named(&self, qualification: Qualification) -> String {
        let mut names = Vec::new();
        match qualification.policy {
            Some(Policy::Wrapping) => names.push("`Wrapping`".to_owned()),
            Some(Policy::Saturating) => names.push("`Saturating`".to_owned()),
            Some(Policy::Trapping) => names.push("`Trapping`".to_owned()),
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
    pub(crate) fn domain_named(&self, domain: crate::body::DomainInstance) -> String {
        match self.typed.domain_definition(domain.declaration) {
            Some(definition) => {
                match self
                    .typed
                    .domain_arguments_for_instance(domain.declaration, domain.identity)
                {
                    Some([]) => format!("`{}`", definition.name.as_str()),
                    Some(arguments) => format!(
                        "`{}` at ({})",
                        definition.name.as_str(),
                        arguments
                            .iter()
                            .map(|argument| self
                                .typed
                                .display_type_reference_with_constraints(*argument))
                            .collect::<Vec<_>>()
                            .join(", ")
                    ),
                    None => format!(
                        "`{}` with an unavailable application",
                        definition.name.as_str()
                    ),
                }
            }
            None => "a domain with no declaration".to_owned(),
        }
    }
}

/// The rejection for a call that names a point inside a machine's body other
/// than its entry. `wiki/spec/language/machines.md`, "Nested machines and
/// identity": a call "invokes its entry, not a saved activation or a private
/// point inside its body", and the source `state` declaration is retired, so
/// no source name denotes such a point. The rule holds for a runtime call and
/// for a call a clause unfolds alike. Resolution gives a call only a machine
/// or its entry state, so reaching this means the typed trees named another
/// state: it is refused as the rule, never read as the machine's entry.
pub(super) fn call_inside_a_machine_body(id: MachineId) -> Refusal {
    Refusal {
        reason: terminal_check_tables::RefusalReason::Violation,
        site: terminal_check_tables::RefusalSite::Machine(id, crate::Construct::Machine),
        source_span: None,
        detail: String::from(
            "a call runs a machine from its entry; this call names a point inside a \
             machine's body other than its entry",
        ),
        obligation: None,
    }
}

/// The rejection for a reference to a scalar supplied where a value is
/// read. `wiki/spec/language/expressions.md`, "Explicit dereference": "Omega
/// never inserts a dereference to satisfy an expected value type, including
/// for `[copy]` types", at bindings, assignments, field initializers,
/// returns, call arguments, operator operands and value patterns; and
/// `domains.md`: "Reading a copyable primitive through a reference requires
/// explicit dereference". A projection through a reference (`entry.owner`)
/// is a place and is not this case: only the whole reference read as its
/// scalar is.
pub(super) fn implicit_dereference(
    id: MachineId,
    at: SourceOrigin,
    referent: ScalarType,
) -> Refusal {
    reject(
        id,
        at,
        format!(
            "this is a reference to `{}`, not a `{}`: Omega never inserts a dereference, so \
             write `$` before it to read the referenced value",
            scalar_named(referent),
            scalar_named(referent),
        ),
    )
}

/// The rejection for `x in D` where `x` has another carrier than `D`'s.
/// `wiki/spec/language/domains.md`, "Carrier-exact membership".
pub(super) fn membership_carrier(
    id: MachineId,
    at: SourceOrigin,
    found: ScalarType,
    wanted: ScalarType,
) -> Refusal {
    reject(
        id,
        at,
        format!(
            "this value has carrier `{}` but the domain classifies `{}` values: membership is \
             formed against the domain's exact carrier, so convert first, as in `(x as {}) in D`",
            scalar_named(found),
            scalar_named(wanted),
            scalar_named(wanted),
        ),
    )
}

/// The resolved scalar carrier, without domains or physical storage choices.
pub(super) fn scalar_named(scalar: ScalarType) -> String {
    match scalar {
        ScalarType::Boolean => "bool".to_owned(),
        ScalarType::Integer(integer) if integer.is_address() => "address".to_owned(),
        ScalarType::Integer(integer) => {
            let prefix = match integer.sign() {
                IntegerSign::Signed => 'i',
                IntegerSign::Unsigned => 'u',
            };
            match integer.width() {
                IntegerWidth::Bits(bits) => format!("{prefix}{bits}"),
                // Proof input IDs depend on allocation order and are not
                // the source name of this width parameter.
                IntegerWidth::Parameter(_) => format!("{prefix}<width parameter>"),
            }
        }
        ScalarType::IeeeFloat(IeeeFloatFormat::Binary32) => "f32".to_owned(),
        ScalarType::IeeeFloat(IeeeFloatFormat::Binary64) => "f64".to_owned(),
        ScalarType::Callable(contract) => format!("machine contract {}", contract.get()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source_state::TypedTrees;
    use semantic_vocabulary::{IntegerType, StructuralTypeId, ValueId, WidthParameter};
    use terminal_check_tables::{RefusalReason, values::RigidType};

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
                    assert_eq!(refusal.reason, RefusalReason::Violation);
                    assert_eq!(refusal.source_span, None);
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
        assert_eq!(refusal.reason, RefusalReason::Violation);
        assert_eq!(refusal.source_span, None);
        assert!(refusal.detail.contains("`u<width parameter>`"));
        assert!(refusal.detail.contains("`bool`"));
    }

    #[test]
    fn a_fixed_array_length_mismatch_names_both_lengths() {
        use crate::body::{RecordType, StorageShape};
        use terminal_psi::ArrayExtent;
        let typed = TypedTrees::default();
        let mut program = Program::new(&typed, false);
        let id = MachineId::new(1).unwrap();
        let element = program.records.push(RecordType {
            identity: "u8".into(),
            shape: StorageShape::PrimitiveScalar {
                scalar_type: ScalarType::Integer(
                    IntegerType::new(IntegerSign::Unsigned, 8).unwrap(),
                ),
                qualification: Qualification::NONE,
            },
            multiplicity: terminal_psi::StructuralMultiplicity::Unrestricted,
            cleanup: None,
            requires: Vec::new(),
            authored_layout: false,
            erased: Vec::new(),
        });
        let mut array = |length| {
            ValueType::record(program.records.push(RecordType {
                shape: StorageShape::FixedArray {
                    element,
                    qualification: Qualification::NONE,
                    length,
                },
                identity: format!("[u8; {length}]"),
                multiplicity: terminal_psi::StructuralMultiplicity::Unrestricted,
                cleanup: None,
                requires: Vec::new(),
                authored_layout: false,
                erased: Vec::new(),
            }))
        };
        let three = array(ArrayExtent::Literal(3));
        let four = array(ArrayExtent::Literal(4));
        let unknown = array(ArrayExtent::Parameter {
            owner: id,
            value: ValueId::new(1).unwrap(),
        });
        let mismatch = |actual| {
            program.mistyped(
                id,
                SourceOrigin::Unpositioned(crate::Unpositioned::SpanAbsent),
                Destination::Parameter,
                Some(four),
                Some(actual),
                "argument types",
            )
        };
        let refused = mismatch(three);
        assert_eq!(refused.reason, RefusalReason::Violation);
        assert!(
            refused
                .detail
                .contains("expects an array with 4 elements but the supplied array has 3")
        );
        assert_eq!(mismatch(unknown).reason, RefusalReason::Unsupported);
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
