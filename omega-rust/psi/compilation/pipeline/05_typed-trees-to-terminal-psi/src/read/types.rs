/*
The type family of the reader: which typed-tree type is a scalar, by the
`builtin_type_atom` route, which is a record, by its data declaration
(`places`), and the type and value an integer literal lands on.
*/

use crate::body::Qualification;
use numerics::integer_carrier::FixedInteger;
use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue, MachineId, ScalarType};
use symbols::BuiltinTypeAtom;
use terminal_check_tables::{Checked, Construct};
use typed_trees::closed_numeric;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

use super::Program;
use crate::body::{Policy, ValueType};
use crate::{Unserved, refuse};

/// The checker's integer for a source carrier. The checker states integer
/// values as host 128-bit integers, so a wider carrier is refused by name.
fn integer(carrier: FixedInteger, machine: MachineId) -> Checked<ScalarType> {
    IntegerType::from_fixed(carrier)
        .map(ScalarType::Integer)
        .ok_or_else(|| refuse(machine, Construct::Machine, Unserved::Type("integer width")))
}

/// The scalar type of a builtin atom, by its compiler-installed slot.
pub(super) fn atom_type(atom: BuiltinTypeAtom, machine: MachineId) -> Checked<ScalarType> {
    match atom {
        BuiltinTypeAtom::Bool => Ok(ScalarType::Boolean),
        BuiltinTypeAtom::Integer(carrier) => integer(carrier, machine),
        BuiltinTypeAtom::Address => {
            Err(refuse(machine, Construct::Machine, Unserved::Type("addr")))
        }
        BuiltinTypeAtom::F32 | BuiltinTypeAtom::F64 => {
            Err(refuse(machine, Construct::Machine, Unserved::Type("float")))
        }
        BuiltinTypeAtom::Slice => Err(refuse(machine, Construct::Machine, Unserved::Type("slice"))),
        BuiltinTypeAtom::SyscallResult => Err(refuse(
            machine,
            Construct::Machine,
            Unserved::Type("syscall result"),
        )),
        BuiltinTypeAtom::Terminal | BuiltinTypeAtom::Never => Err(refuse(
            machine,
            Construct::Machine,
            Unserved::Type("terminal or never"),
        )),
        BuiltinTypeAtom::UInt | BuiltinTypeAtom::Int => Err(refuse(
            machine,
            Construct::Machine,
            Unserved::Type("anonymous integer"),
        )),
        BuiltinTypeAtom::AtomicBool | BuiltinTypeAtom::AtomicU32 | BuiltinTypeAtom::AtomicU64 => {
            Err(refuse(
                machine,
                Construct::Machine,
                Unserved::Type("atomic"),
            ))
        }
    }
}

/// The integer type a literal landed on, through the same width table.
fn landed_type(
    landed: numerics::literals::LandedIntegerType,
    machine: MachineId,
) -> Checked<ScalarType> {
    use numerics::literals::LandedIntegerType as Landed;
    match landed {
        Landed::Fixed(carrier) => integer(carrier, machine),
        Landed::Addr => Err(refuse(
            machine,
            Construct::Machine,
            Unserved::Type("addr literal"),
        )),
    }
}

impl Program<'_> {
    pub(super) fn scalar_type(
        &self,
        id: MachineId,
        handle: TypeReferenceHandle,
    ) -> Checked<ScalarType> {
        let typed = self.typed;
        if !handle.is_valid() {
            return Err(refuse(
                id,
                Construct::Machine,
                Unserved::Type("missing type"),
            ));
        }
        let unserved = |what| Err(refuse(id, Construct::Machine, Unserved::Type(what)));
        match typed.type_reference_table.type_reference(handle) {
            TypeReferenceNode::Named { symbol, .. } => {
                if !symbol.is_valid() {
                    return Err(refuse(
                        id,
                        Construct::Machine,
                        Unserved::Spelling("type without a symbol"),
                    ));
                }
                match typed.symbols.builtin_type_atom(*symbol) {
                    Some(BuiltinTypeAtom::Address) if self.descriptor_storage => Ok(
                        ScalarType::Integer(IntegerType::supported_address_representation()),
                    ),
                    Some(atom) => atom_type(atom, id),
                    None => unserved("nominal type"),
                }
            }
            TypeReferenceNode::Reference { .. } => unserved("reference"),
            TypeReferenceNode::Constrained { .. } => unserved("constrained"),
            TypeReferenceNode::FixedArray { .. } => unserved("fixed array"),
            TypeReferenceNode::Slice { .. } => unserved("slice"),
            TypeReferenceNode::Generic { .. } => unserved("generic application"),
            TypeReferenceNode::ConstExpression(_) => unserved("const expression"),
            TypeReferenceNode::DynamicTrait { .. } => unserved("dynamic trait"),
            TypeReferenceNode::Unit => unserved("unit as a value"),
        }
    }

    /// Whether two closed types are the same type, when both are builtin
    /// scalar types: decided by the compiler-installed atom each symbol
    /// names, never by a spelling. A type parameter the machine being read
    /// is checked at (`Reading::Alternative`, or a generic machine applied
    /// at a call's type arguments) is the scalar type the scope fixes it
    /// to. `None` for any other operand (a rigid type parameter, a data
    /// type, an application), which is not decided here. generics.md,
    /// "Static type equality": "`==` between type expressions states exact
    /// normalized type equality".
    fn closed_type_equation(
        &self,
        left: TypeReferenceHandle,
        right: TypeReferenceHandle,
    ) -> Option<bool> {
        enum Closed {
            Atom(BuiltinTypeAtom),
            Scalar(ScalarType),
        }
        let closed = |handle: TypeReferenceHandle| {
            if !handle.is_valid() {
                return None;
            }
            match self.typed.type_reference_table.type_reference(handle) {
                TypeReferenceNode::Named { symbol, .. } if symbol.is_valid() => {
                    if let Some((_, bound)) = self.scope.iter().find(|(binder, _)| binder == symbol)
                    {
                        return match bound {
                            ValueType::Scalar(scalar_type, Qualification::NONE) => {
                                Some(Closed::Scalar(*scalar_type))
                            }
                            ValueType::Scalar(..)
                            | ValueType::Rigid(_)
                            | ValueType::Record(_, _, _) => None,
                        };
                    }
                    self.typed
                        .symbols
                        .builtin_type_atom(*symbol)
                        .map(Closed::Atom)
                }
                _ => None,
            }
        };
        // The identity only names a refusal, which is discarded here.
        let scalar = |atom: BuiltinTypeAtom| {
            MachineId::new(1).and_then(|machine| atom_type(atom, machine).ok())
        };
        Some(match (closed(left)?, closed(right)?) {
            (Closed::Atom(left), Closed::Atom(right)) => left == right,
            (Closed::Scalar(left), Closed::Scalar(right)) => left == right,
            (Closed::Scalar(fixed), Closed::Atom(atom))
            | (Closed::Atom(atom), Closed::Scalar(fixed)) => scalar(atom)? == fixed,
        })
    }

    /// Whether closed type equations make `expression` false read left to
    /// right: it is decided false, or it is a conjunction one of whose
    /// conjuncts is. generics.md, "Static type equality": "A type equation
    /// in a contract guards what follows it, as a case test does ...
    /// Conjuncts are read left to right; a disjunct whose type equation is
    /// false for the instantiation holds vacuously." So in the machine for
    /// `u16`, the disjunct `T == u8 && result == 7u8` contributes nothing,
    /// and `result == 7u8`, which is not well-typed there, is never read.
    pub(super) fn guarded_false(&self, expression: ExpressionHandle) -> bool {
        use typed_trees::expression::BinaryOperator;
        if self.static_truth(expression) == Some(false) {
            return true;
        }
        if !expression.is_valid() {
            return false;
        }
        match self.typed.expression_table.expression(expression) {
            ExpressionNode::Binary(binary) if binary.operator == BinaryOperator::And => {
                self.guarded_false(binary.left) || self.guarded_false(binary.right)
            }
            _ => false,
        }
    }

    /// The truth of an expression that closed type equations decide: an
    /// equation between two closed builtin types, a local bound to such a
    /// truth, and `==`, `&&`, `||` and `!` over those, where `==` may also
    /// compare one with a Boolean literal. `None` for every other
    /// expression, a lone Boolean literal included: only a type equation
    /// makes a condition static. generics.md, "Static type equality": "a
    /// branch with established `T == u32` may use that equality to check its
    /// operations and values as u32", so in a machine generated for `u32`
    /// the branch `u32 == u64` is one no value reaches and no rule types.
    pub(super) fn static_truth(&self, expression: ExpressionHandle) -> Option<bool> {
        use typed_trees::expression::{BinaryOperator, UnaryOperator};
        if !expression.is_valid() {
            return None;
        }
        let table = &self.typed.expression_table;
        match table.expression(expression) {
            ExpressionNode::Name(path) => {
                let resolved = path.resolved_symbol()?;
                if table.name_path_members(path.members).len() != 1 {
                    return None;
                }
                self.static_truths
                    .iter()
                    .rev()
                    .find(|(symbol, _)| *symbol == resolved)
                    .map(|(_, truth)| *truth)
            }
            ExpressionNode::Unary(unary) if unary.operator == UnaryOperator::LogicalNot => {
                self.static_truth(unary.operand).map(|truth| !truth)
            }
            ExpressionNode::Binary(binary) => {
                let literal = |handle: ExpressionHandle| match handle
                    .is_valid()
                    .then(|| table.expression(handle))
                {
                    Some(ExpressionNode::Boolean(value)) => Some(*value),
                    _ => None,
                };
                match binary.operator {
                    BinaryOperator::Equal => {
                        if let (
                            Some(ExpressionNode::TypeExpression(left)),
                            Some(ExpressionNode::TypeExpression(right)),
                        ) = (
                            binary
                                .left
                                .is_valid()
                                .then(|| table.expression(binary.left)),
                            binary
                                .right
                                .is_valid()
                                .then(|| table.expression(binary.right)),
                        ) {
                            return self.closed_type_equation(*left, *right);
                        }
                        match (
                            self.static_truth(binary.left),
                            self.static_truth(binary.right),
                        ) {
                            (Some(left), Some(right)) => Some(left == right),
                            (Some(truth), None) => {
                                literal(binary.right).map(|value| truth == value)
                            }
                            (None, Some(truth)) => literal(binary.left).map(|value| truth == value),
                            (None, None) => None,
                        }
                    }
                    BinaryOperator::And => {
                        Some(self.static_truth(binary.left)? && self.static_truth(binary.right)?)
                    }
                    BinaryOperator::Or => {
                        Some(self.static_truth(binary.left)? || self.static_truth(binary.right)?)
                    }
                    _ => None,
                }
            }
            _ => None,
        }
    }

    /// A formal's, local's, result's or field's type: a record when the
    /// type is a `Named` data declaration, else whatever the scalar route
    /// says, including its refusals.
    /// Describe the actual carrier without interpreting or establishing its
    /// declared domains. Full qualification remains in the ABI/source contract.
    pub(super) fn storage_carrier(
        &self,
        id: MachineId,
        mut handle: TypeReferenceHandle,
    ) -> Checked<TypeReferenceHandle> {
        if !self.descriptor_storage {
            return Ok(handle);
        }
        let mut visiting = Vec::new();
        loop {
            if !self
                .typed
                .type_reference_table
                .contains_type_reference(handle)
                || visiting.contains(&handle)
            {
                return Err(refuse(
                    id,
                    Construct::Machine,
                    Unserved::Type("invalid constrained storage carrier"),
                ));
            }
            visiting.push(handle);
            match self.typed.type_reference_table.type_reference(handle) {
                TypeReferenceNode::Constrained { base_type, .. } => handle = *base_type,
                _ => return Ok(handle),
            }
        }
    }

    /// `value_type` where a byte view in a domain is served: a data field's
    /// type and a cast's result type (`view_type`).
    pub(super) fn qualified_view_type(
        &mut self,
        id: MachineId,
        handle: TypeReferenceHandle,
    ) -> Checked<ValueType> {
        let outer = std::mem::replace(&mut self.qualified_views, true);
        let read = self.value_type(id, handle);
        self.qualified_views = outer;
        read
    }

    pub(super) fn value_type(
        &mut self,
        id: MachineId,
        handle: TypeReferenceHandle,
    ) -> Checked<ValueType> {
        if handle.is_valid()
            && let Some(view) = self.view_type(id, handle)?
        {
            return Ok(view);
        }
        if handle.is_valid()
            && let TypeReferenceNode::FixedArray {
                element_type,
                length,
            } = self.typed.type_reference_table.type_reference(handle)
        {
            return self.array_type(id, handle, *element_type, length);
        }
        // A type parameter in scope is what the scope says it stands for,
        // found by the binder's symbol.
        if handle.is_valid()
            && let TypeReferenceNode::Named { symbol, .. } =
                self.typed.type_reference_table.type_reference(handle)
            && symbol.is_valid()
            && let Some((_, bound)) = self.scope.iter().find(|(binder, _)| binder == symbol)
        {
            return Ok(*bound);
        }
        if handle.is_valid()
            && let TypeReferenceNode::Named { symbol, .. } =
                self.typed.type_reference_table.type_reference(handle)
            && symbol.is_valid()
            && self.typed.symbols.builtin_type_atom(*symbol).is_none()
            && self.data_declaration(*symbol).is_some()
        {
            return self.record_at(id, handle);
        }
        if handle.is_valid()
            && let TypeReferenceNode::Generic {
                arguments,
                lifetime_arguments,
                ..
            } = self.typed.type_reference_table.type_reference(handle)
            && !arguments.is_empty()
            && lifetime_arguments.is_empty()
            && self.typed.closed_data_definition(handle).is_some()
        {
            return self.record_at(id, handle);
        }
        if handle.is_valid()
            && let TypeReferenceNode::Constrained {
                base_type,
                constraints,
            } = self.typed.type_reference_table.type_reference(handle)
        {
            return self.qualified_type(id, *base_type, *constraints);
        }
        // `Inner<'a>`: data applied at lifetime arguments and nothing else
        // is the declaration's one record row, because a lifetime argument
        // is an erased region; the regions it names are part of the value
        // type (`places::occurrence_regions`, `regions`). The
        // application itself is checked against the declaration and the
        // owner of the lifetimes it names (`lifetime_application`). An
        // application with a type, const or machine argument, or of data
        // that declares one, stays with the scalar route's refusal (`type
        // generic application`).
        if handle.is_valid()
            && let TypeReferenceNode::Generic {
                base_symbol,
                lifetime_arguments,
                arguments,
                ..
            } = self.typed.type_reference_table.type_reference(handle)
            && arguments.is_empty()
            && base_symbol.is_valid()
            && self.typed.symbols.builtin_type_atom(*base_symbol).is_none()
            && let Some(data) = self.data_declaration(*base_symbol)
            && data.type_parameters.is_empty()
        {
            use super::places::RetainedLifetime;
            use typed_trees::type_identity::{
                lifetime_parameter_identity, lifetime_parameter_name,
            };
            let typed = self.typed;
            let table = &typed.type_reference_table;
            let retained: Vec<RetainedLifetime<'_>> = table
                .operand_lifetimes(handle)
                .iter()
                .map(|region| {
                    if region.declaration.is_valid() {
                        RetainedLifetime::Resolved(
                            lifetime_parameter_name(typed, *region).map(|name| name.as_str()),
                        )
                    } else {
                        RetainedLifetime::Open
                    }
                })
                .collect();
            let owner = match table.operand_lifetime_owner(handle) {
                retained if retained.is_valid() => retained,
                _ => self.lifetime_owner,
            };
            super::places::lifetime_application(
                id,
                data.lifetime_parameters.len(),
                lifetime_arguments,
                &retained,
                |tag| owner.is_valid() && lifetime_parameter_identity(typed, owner, tag).is_some(),
            )?;
            // The occurrence is kept as the record's source application,
            // as a bare `Inner` is: the product's correspondence needs one
            // for every nominal record, however its type was written.
            return self.record_at(id, handle);
        }
        self.scalar_type(id, handle).map(ValueType::plain)
    }

    /// `T in D & E`: the carrier `T` and the domains its type carries, read
    /// from each constraint node's kind and symbol and never from a spelling.
    /// An arithmetic policy and a declared domain are members of the one
    /// qualification (`body::Qualification`), on a scalar or a data carrier.
    ///
    /// numeric_values.md, "Overflow policies": "A type may carry any number
    /// of value qualifications but at most one arithmetic policy", and a
    /// policy qualifies an integer.
    ///
    /// Refused by name: `Trapping`, whose every operation is its own `Trap`
    /// site; a compiler-owned domain; and the declared domains whose meaning
    /// needs more than their own declaration (`declared_domain`).
    fn qualified_type(
        &mut self,
        id: MachineId,
        base_type: TypeReferenceHandle,
        constraints: arena::HandleSpan<typed_trees::types::TypeConstraintNode>,
    ) -> Checked<ValueType> {
        let carrier = self.value_type(id, base_type)?;
        let qualification = self.type_qualification(id, carrier, constraints)?;
        for domain in qualification.domains.members() {
            if !carrier.qualification().domains.contains(domain) {
                self.declared_domain(id, domain, carrier)?;
            }
        }
        carrier.qualified(qualification).ok_or_else(|| {
            refuse(
                id,
                Construct::Machine,
                Unserved::Type("domain on a type parameter"),
            )
        })
    }

    /// Read the qualification once for both type formation and casts. Type
    /// formation admits new domains; a clause cast can only retain domains
    /// already checked on its operand.
    fn type_qualification(
        &self,
        id: MachineId,
        carrier: ValueType,
        constraints: arena::HandleSpan<typed_trees::types::TypeConstraintNode>,
    ) -> Checked<Qualification> {
        use numerics::arithmetic::ArithmeticDomain;
        use typed_trees::types::{DomainConstraintSubject, TypeConstraintNode};
        let unserved = |what| Err(refuse(id, Construct::Machine, Unserved::Type(what)));
        let mut qualification = carrier.qualification();
        let typed = self.typed;
        for constraint in typed.type_reference_table.constraints(constraints) {
            match constraint {
                TypeConstraintNode::ArithmeticDomain(ArithmeticDomain::Exact) => {}
                TypeConstraintNode::ArithmeticDomain(ArithmeticDomain::Trapping) => {
                    return unserved("constrained by Trapping");
                }
                TypeConstraintNode::ArithmeticDomain(
                    policy @ (ArithmeticDomain::Wrapping | ArithmeticDomain::Saturating),
                ) => {
                    if qualification.policy.is_some() {
                        return unserved("constrained by more than one arithmetic policy");
                    }
                    if !matches!(carrier, ValueType::Scalar(ScalarType::Integer(_), _)) {
                        return unserved("arithmetic policy on a type that is not an integer");
                    }
                    qualification.policy = Some(match policy {
                        ArithmeticDomain::Wrapping => Policy::Wrapping,
                        _ => Policy::Saturating,
                    });
                }
                TypeConstraintNode::Domain(domain)
                    if domain.subject == DomainConstraintSubject::Declared =>
                {
                    if !domain.arguments.is_empty() {
                        return unserved("constrained by a domain applied at arguments");
                    }
                    let Some(domains) = qualification.domains.with(domain.symbol) else {
                        return unserved(
                            "constrained by one domain twice or by more than four declared domains",
                        );
                    };
                    qualification.domains = domains;
                }
                TypeConstraintNode::Domain(_) => {
                    return unserved("constrained by a compiler-owned domain");
                }
                TypeConstraintNode::Named(_) => {
                    return unserved("constrained by a name no declaration answers");
                }
            }
        }
        Ok(qualification)
    }

    /// Read a scalar domain cast's result without forming a new domain or
    /// storage representation. Its carrier and retained domains must come
    /// from the already checked operand; additions need establishment proof.
    pub(super) fn scalar_domain_cast_type(
        &self,
        id: MachineId,
        handle: TypeReferenceHandle,
        before: ValueType,
    ) -> Checked<ValueType> {
        let after = self.qualified_scalar_type(id, handle)?;
        if before.qualified(Qualification::NONE) != after.qualified(Qualification::NONE) {
            return Err(refuse(
                id,
                Construct::Machine,
                Unserved::Typing("domain cast entry that changes the carrier"),
            ));
        }
        if !after
            .qualification()
            .domains
            .is_subset_of(&before.qualification().domains)
        {
            return Err(refuse(
                id,
                Construct::Machine,
                Unserved::Contract("domain establishment in a clause term"),
            ));
        }
        Ok(after)
    }

    fn qualified_scalar_type(
        &self,
        id: MachineId,
        handle: TypeReferenceHandle,
    ) -> Checked<ValueType> {
        if handle.is_valid() {
            match self.typed.type_reference_table.type_reference(handle) {
                TypeReferenceNode::Constrained {
                    base_type,
                    constraints,
                } => {
                    let carrier = self.qualified_scalar_type(id, *base_type)?;
                    let qualification = self.type_qualification(id, carrier, *constraints)?;
                    return carrier.qualified(qualification).ok_or_else(|| {
                        refuse(
                            id,
                            Construct::Machine,
                            Unserved::Type("domain on a type parameter"),
                        )
                    });
                }
                TypeReferenceNode::Named { symbol, .. } => {
                    if let Some((_, bound @ ValueType::Scalar(..))) =
                        self.scope.iter().find(|(binder, _)| binder == symbol)
                    {
                        return Ok(*bound);
                    }
                }
                _ => {}
            }
        }
        self.scalar_type(id, handle).map(ValueType::plain)
    }

    /// Whether the checker reads the declared domain `domain` on `carrier`:
    /// it must be a domain of exactly that carrier (domains.md,
    /// "Carrier-exact membership": "form membership against the domain's
    /// exact resolved applied carrier"), with nothing typing left unserved
    /// on it.
    ///
    /// A domain whose meaning is not on its own declaration is refused by
    /// name until the checker reads that meaning: an alias (its expansion),
    /// a child (a type carrying it carries every ancestor), an indexed or
    /// carrier-generic family (its application), and a domain that selects
    /// a cleanup machine.
    pub(super) fn declared_domain(
        &mut self,
        id: MachineId,
        domain: symbols::SymbolHandle,
        carrier: ValueType,
    ) -> Checked<()> {
        let unserved = |what| Err(refuse(id, Construct::Machine, Unserved::Type(what)));
        let typed = self.typed;
        let Some(definition) = typed.domain_definition(domain) else {
            return unserved("constrained by a domain with no declaration");
        };
        self.served(id, domain)?;
        if definition.alias.is_some() {
            return unserved("constrained by a domain alias");
        }
        if !typed.domain_type_parameters(definition).is_empty()
            || !definition.index_arguments.is_empty()
        {
            return unserved("constrained by a generic or indexed domain family");
        }
        if definition.cleanup_selection.is_some() {
            return unserved("constrained by a domain that selects a cleanup machine");
        }
        if typed_trees::domain::subject_membership_facts(typed, definition)
            .any(|membership| membership.domain_symbol != domain)
        {
            return unserved("constrained by a domain that refines another domain");
        }
        let Some(plain) = carrier.qualified(crate::body::Qualification::NONE) else {
            return unserved("domain on a type parameter");
        };
        if self.value_type(id, definition.target_type)? != plain {
            return unserved("constrained by a domain declared on another carrier");
        }
        // A predicate over the fields of a data value has no owning
        // contract to join when that value is a local or a formal
        // (`predicates`); only a scalar's is stated yet.
        if super::authority::has_predicates(definition) && !matches!(plain, ValueType::Scalar(..)) {
            return unserved("constrained by a domain with predicates on a data carrier");
        }
        Ok(())
    }

    /// A state's result: the invalid handle and `Unit` are both a Unit
    /// machine, because that is how the typed trees record "no result".
    pub(super) fn result_type(
        &mut self,
        id: MachineId,
        handle: TypeReferenceHandle,
    ) -> Checked<Option<ValueType>> {
        if !handle.is_valid() {
            return Ok(None);
        }
        // Every other node kind goes through `scalar_type`'s exhaustive match.
        if matches!(
            self.typed.type_reference_table.type_reference(handle),
            TypeReferenceNode::Unit
        ) {
            return Ok(None);
        }
        self.value_type(id, handle).map(Some)
    }

    /// An integer literal that stage 04 left without a landed type: it lands
    /// in the type its context expects (`wiki/spec/language/domains.md`,
    /// "Literals and expected types").
    /// Whether `expression` is wholly anonymous: an integer or decimal
    /// literal no suffix landed, or a binary operation over two such
    /// operands. numeric_values.md: "Decimal literals and wholly anonymous
    /// arithmetic denote exact rationals", so such a tree has no carrier of
    /// its own and lands, complete, in its context's type
    /// (`anonymous_value`). Its partner in a binary operation is therefore
    /// read first.
    pub(super) fn wholly_anonymous(&self, expression: ExpressionHandle) -> bool {
        let table = &self.typed.expression_table;
        let mut pending = vec![expression];
        while let Some(expression) = pending.pop() {
            if !expression.is_valid() {
                return false;
            }
            match table.expression(expression) {
                ExpressionNode::Integer(literal) if literal.landing().is_none() => {}
                ExpressionNode::Float(literal) if literal.landing().is_none() => {}
                // A suffixed operator has no anonymous meaning
                // (numeric_values.md, "Policy operators": "A wholly
                // anonymous tree rejects"), so a tree holding one is read
                // operator by operator and refused there.
                ExpressionNode::Binary(binary)
                    if binary.policy == numerics::arithmetic::ArithmeticDomain::Exact =>
                {
                    pending.extend([binary.left, binary.right]);
                }
                _ => return false,
            }
        }
        true
    }

    /// The value a wholly anonymous binary tree lands as in `expected`.
    /// numeric_values.md: anonymous integer-literal division "is rational
    /// division, not truncation", and the tree keeps "exact rational
    /// intermediates until landing requires an integral, in-range value".
    /// So `7 / 2` is `7/2`, never an `i32` division by two that yields 3,
    /// and `(2147483647 + 1) - 1` is an `i32` value although its
    /// intermediate is not. The tree is evaluated exactly, by the same
    /// typed-tree evaluator stage 05 lands it with, and only a value that is
    /// an integer of `expected`, with no fractional intermediate, becomes a
    /// literal. Every other tree is refused: one whose operator has no anonymous meaning
    /// (remainder, bitwise, shift, comparison: "Builtin remainder requires
    /// integer operand meaning before folding"), one whose value or any
    /// intermediate is fractional, and one outside `expected`.
    pub(super) fn anonymous_value(
        &self,
        id: MachineId,
        expression: ExpressionHandle,
        expected: Option<ScalarType>,
    ) -> Checked<(IntegerValue, IntegerType)> {
        let unserved = |what| Err(refuse(id, Construct::Machine, Unserved::Expression(what)));
        let typed = self.typed;
        let Some(evaluated) = closed_numeric::evaluate_anonymous_value::<true>(
            typed,
            expression,
            &[],
            &mut |expression| closed_numeric::has_anonymous_operator_meaning(typed, expression),
        ) else {
            return unserved("anonymous operation without an exact meaning");
        };
        let Some(value) = evaluated.value.to_integer_exact() else {
            return unserved("fractional anonymous value");
        };
        // "integer landing requires an integral in-range result and
        // explicit `as` when acknowledging fractional intermediates": `(7 /
        // 2) * 2` is 7, but lands only through an `as`, which is outside the
        // subset.
        if evaluated.fractional_origin.is_valid() {
            return unserved("anonymous value with a fractional intermediate");
        }
        let Some(ScalarType::Integer(integer)) = expected else {
            return unserved("anonymous value without an expected integer type");
        };
        let value = match integer.sign() {
            IntegerSign::Signed => value
                .to_i64()
                .map(|value| IntegerValue::Signed(i128::from(value))),
            IntegerSign::Unsigned => value
                .to_u64()
                .map(|value| IntegerValue::Unsigned(u128::from(value))),
        };
        match value {
            Some(value) if integer.admits(value) => Ok((value, integer)),
            Some(_) | None => unserved("anonymous value outside its type"),
        }
    }

    /// An integer literal's value and type: the type stage 04 landed it in,
    /// or else the type its context expects. Both, when present, must agree.
    pub(super) fn literal(
        &self,
        id: MachineId,
        literal: &numerics::literals::IntegerLiteral,
        expected: Option<ScalarType>,
    ) -> Checked<(IntegerValue, IntegerType)> {
        let unserved = |what| Err(refuse(id, Construct::Machine, Unserved::Expression(what)));
        let landed = match literal.landing() {
            Some(landing) => {
                if landing.domain != numerics::arithmetic::ArithmeticDomain::Exact {
                    return unserved("integer literal in a non-exact domain");
                }
                Some(landed_type(landing.landed_type, id)?)
            }
            None => None,
        };
        let scalar_type = match (landed, expected) {
            (Some(landed), Some(expected)) if landed != expected => {
                return Err(refuse(
                    id,
                    Construct::Machine,
                    Unserved::Typing("literal landed type"),
                ));
            }
            (Some(scalar_type), _) | (None, Some(scalar_type)) => scalar_type,
            (None, None) => return unserved("anonymous integer literal without an expected type"),
        };
        let ScalarType::Integer(integer) = scalar_type else {
            return Err(refuse(
                id,
                Construct::Machine,
                Unserved::Typing("integer literal in a non-integer place"),
            ));
        };
        let value = match (integer.sign(), literal.is_negative()) {
            (IntegerSign::Signed, _) => literal
                .value_i64()
                .map(|value| IntegerValue::Signed(i128::from(value))),
            (IntegerSign::Unsigned, false) => literal
                .value_u64()
                .map(|value| IntegerValue::Unsigned(u128::from(value))),
            (IntegerSign::Unsigned, true) => None,
        };
        match value {
            Some(value) if integer.admits(value) => Ok((value, integer)),
            Some(_) | None => unserved("integer literal outside its type"),
        }
    }
}
