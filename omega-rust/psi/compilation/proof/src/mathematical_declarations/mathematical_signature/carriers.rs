//! Carrier classification and interning: which scalar carrier an operand
//! inhabits, and the lazy `Type 0` assumptions (`Int`, `Boolean`, `addr`,
//! the float formats, `IntLt`/`IntLe`/`Not`/`IntAdd`/`IntSub`, literals)
//! the bounded vocabulary interns on first use.

use numerics::bignum::BigInt;
use numerics::literals::{FloatFormat, LandedIntegerType};

use proof_admission::{Declaration, Level, Sort, Term, TermHandle};
use symbols::{BuiltinTypeAtom, SymbolHandle};
use typed_trees::data::DataMember;
use typed_trees::expression::{
    ExpressionHandle, ExpressionNode, TableMemberExpression, TableNamePath, UnaryOperator,
};
use typed_trees::types::{PrimitiveType, TypeReferenceHandle, TypeReferenceNode};

use super::{Elaborator, ScalarCarrier};

impl<'a> Elaborator<'a> {
    /// The scalar carrier one operand expression inhabits, when it is
    /// determinable from literal landings, scope domains, declaration
    /// codomains, operation heads, `data` field declarations and cast
    /// targets. `None` means the carrier is not a denoted scalar
    /// carrier — anonymous literals defer so the other operand's
    /// carrier supplies theirs, mirroring `infer_scalar_type`.
    pub(super) fn operand_carrier(&self, handle: ExpressionHandle) -> Option<ScalarCarrier> {
        match self.program.expression_table.expression(handle) {
            ExpressionNode::Integer(literal) => {
                literal.landing().map(|landing| match landing.landed_type {
                    LandedIntegerType::Addr => ScalarCarrier::Address,
                    _ => ScalarCarrier::Integer,
                })
            }
            ExpressionNode::Boolean(_) => Some(ScalarCarrier::Boolean),
            // Anonymous float literals defer like integer ones; only a
            // landed literal determines its carrier.
            ExpressionNode::Float(literal) => literal.landing().map(|format| match format {
                FloatFormat::F32 => ScalarCarrier::Float32,
                FloatFormat::F64 => ScalarCarrier::Float64,
            }),
            ExpressionNode::Name(path) => self.name_carrier(path),
            ExpressionNode::Call(call) => {
                self.declaration_result_carrier(validation::call_target_symbol(self.program, call))
            }
            // An arithmetic expression's carrier is the operands' shared
            // carrier: `Int` for integers, the format carrier for floats,
            // and one operand's carrier when an anonymous literal defers.
            // Operands that disagree determine no carrier.
            ExpressionNode::Binary(binary) => match binary.operator {
                op if op.is_arithmetic() => {
                    match (
                        self.operand_carrier(binary.left),
                        self.operand_carrier(binary.right),
                    ) {
                        (Some(left), Some(right)) if left == right => Some(left),
                        (Some(carrier), None) | (None, Some(carrier)) => Some(carrier),
                        // All-anonymous operands (`2 + 3`) are integer
                        // arithmetic when the expression evaluates
                        // closed — but only when no float leaf sneaks
                        // into that evaluation (`4.0 / 2.0` evaluates
                        // to `2` too; it is not `Int` arithmetic).
                        (None, None)
                            if !self.expression_has_float_leaf(handle)
                                && self
                                    .program
                                    .closed_integer_expression_value(handle)
                                    .is_some() =>
                        {
                            Some(ScalarCarrier::Integer)
                        }
                        _ => None,
                    }
                }
                _ => Some(ScalarCarrier::Boolean),
            },
            ExpressionNode::Unary(unary) => Some(match unary.operator {
                UnaryOperator::LogicalNot => ScalarCarrier::Boolean,
                UnaryOperator::BitwiseNot => ScalarCarrier::Integer,
            }),
            ExpressionNode::Member(member) => self.member_carrier(member),
            ExpressionNode::Cast(cast) => self.scalar_carrier_of_type_reference(cast.target_type),
            ExpressionNode::ZeroValue(reference) => {
                self.scalar_carrier_of_type_reference(*reference)
            }
            // A `data` literal inhabits its type's interned carrier; the
            // type must already appear in the signature (an annotation or
            // parameter interned it) or the carrier genuinely is not in
            // scope and the expression keeps refusing.
            ExpressionNode::StructLiteral(literal) => self
                .carriers
                .get(&literal.type_symbol)
                .map(|&position| self.carrier_class_at(position)),
            // A match denotes at the one carrier every arm inhabits;
            // disagreeing or indeterminate arms return no carrier.
            ExpressionNode::Match(matched) => {
                let mut carrier = None;
                for arm in self
                    .program
                    .expression_table
                    .match_arms(matched.arms)
                    .iter()
                {
                    let arm_carrier = self.operand_carrier(arm.value)?;
                    match carrier {
                        None => carrier = Some(arm_carrier),
                        Some(agreed) if agreed == arm_carrier => {}
                        Some(_) => return None,
                    }
                }
                carrier
            }
            _ => None,
        }
    }

    /// Whether an expression's syntax carries a float literal leaf.
    /// `closed_integer_expression_value` evaluates rationals, so a
    /// closed `4.0 / 2.0` yields `2` — the gate that keeps all-integer
    /// anonymous arithmetic on `Int` must not admit it.
    pub(super) fn expression_has_float_leaf(&self, handle: ExpressionHandle) -> bool {
        if !handle.is_valid() {
            return false;
        }
        match self.program.expression_table.expression(handle) {
            ExpressionNode::Float(_) | ExpressionNode::UndeterminedNaN(_) => true,
            ExpressionNode::Binary(binary) => {
                self.expression_has_float_leaf(binary.left)
                    || self.expression_has_float_leaf(binary.right)
            }
            ExpressionNode::Unary(unary) => self.expression_has_float_leaf(unary.operand),
            _ => false,
        }
    }

    /// The carrier a `data` field projection inhabits. A bound
    /// `member_symbol` names the field directly; when resolution left
    /// the member unbound — a mathematical parameter's `p.x`, whose
    /// declared type the resolver does not model — the receiver's
    /// carrier already names its data declaration, and the authored
    /// field name selects within it.
    pub(super) fn member_carrier(&self, member: &TableMemberExpression) -> Option<ScalarCarrier> {
        if member.member_symbol.is_valid() {
            let field_type = self
                .program
                .any_field_owner(member.member_symbol)
                .map(|(_, field)| field.type_reference)?;
            return self.scalar_carrier_of_type_reference(field_type);
        }
        let ScalarCarrier::Carrier(position) = self.operand_carrier(member.receiver)? else {
            return None;
        };
        let symbol = self
            .carriers
            .iter()
            .find(|(_, carrier_position)| **carrier_position == position)
            .map(|(symbol, _)| *symbol)?;
        let field_type = self
            .program
            .data_definition_by_symbol(symbol)
            .and_then(|data| {
                self.program
                    .data_members(data)
                    .iter()
                    .find_map(|member_decl| match member_decl {
                        DataMember::Field(field) if field.name == member.member => {
                            Some(field.type_reference)
                        }
                        DataMember::Variant(variant) => self
                            .program
                            .data_payload_fields(variant)
                            .iter()
                            .find(|field| field.name == member.member)
                            .map(|field| field.type_reference),
                        _ => None,
                    })
            })?;
        self.scalar_carrier_of_type_reference(field_type)
    }

    /// The carrier a `Name` inhabits: a scope entry's domain when the
    /// entry names a `Type 0` carrier assumption, or an earlier
    /// declaration's result carrier. A scope entry bound at a `Sort`,
    /// `Pi` or `Variable` domain — type binders, arrow parameters —
    /// carries no scalar carrier.
    pub(super) fn name_carrier(&self, path: &TableNamePath) -> Option<ScalarCarrier> {
        if path.symbol.is_valid() {
            if let Some(position) = self
                .scope
                .iter()
                .rposition(|entry| entry.symbol == Some(path.symbol))
            {
                return self.domain_carrier(position);
            }
            return self.declaration_result_carrier(path.symbol);
        }
        let members = self
            .program
            .expression_table
            .name_path_members(path.members);
        let [name] = members else { return None };
        let position = self.scope.iter().rposition(|entry| entry.name == *name)?;
        self.domain_carrier(position)
    }

    /// The carrier class one scope entry's domain names, when it is a
    /// `Type 0` carrier assumption at all.
    pub(super) fn domain_carrier(&self, scope_position: usize) -> Option<ScalarCarrier> {
        match self.arena.get(self.scope[scope_position].domain) {
            Term::Constant { declaration, .. } => Some(self.carrier_class_at(declaration)),
            _ => None,
        }
    }

    /// The carrier an earlier authored declaration's result inhabits —
    /// the innermost codomain of its `Pi` spine — or the authored
    /// return type of a `machine` declaration, which carries no kernel
    /// declaration and lets its calls intern as opaque constants.
    pub(super) fn declaration_result_carrier(&self, symbol: SymbolHandle) -> Option<ScalarCarrier> {
        if self.program.symbols.builtin_type_member(symbol)
            == Some(symbols::BuiltinTypeMember::AddressBound)
            || self.program.symbols.builtin_function_for_symbol(symbol)
                == Some(symbols::BuiltinFunction::IntegerEmbed)
        {
            return Some(ScalarCarrier::Integer);
        }
        if let Some(&position) = self.authored_positions.get(&symbol) {
            let mut ty = self.declarations[position as usize].ty;
            while let Term::Pi { codomain, .. } = self.arena.get(ty) {
                ty = codomain;
            }
            return match self.arena.get(ty) {
                Term::Constant { declaration, .. } => Some(self.carrier_class_at(declaration)),
                _ => None,
            };
        }
        // Ordinary calls retain their selected declaration: a state, operator
        // requirement or other callable signature. Borrow the shared result
        // query rather than excluding declarations without a machine body.
        let result = validation::call_target_return_type(self.program, symbol)?;
        self.scalar_carrier_of_type_reference(result)
    }

    /// The scalar carrier a type reference names, when it is one —
    /// primitives and the `Int`/`UInt` atoms map onto the shared
    /// carriers without interning; other symbols only name a carrier
    /// already interned.
    pub(super) fn scalar_carrier_of_type_reference(
        &self,
        reference: TypeReferenceHandle,
    ) -> Option<ScalarCarrier> {
        if let Some(primitive) = self.program.primitive_type_reference(reference) {
            return match primitive {
                PrimitiveType::Bool => Some(ScalarCarrier::Boolean),
                PrimitiveType::Addr => Some(ScalarCarrier::Address),
                PrimitiveType::F32 => Some(ScalarCarrier::Float32),
                PrimitiveType::F64 => Some(ScalarCarrier::Float64),
                _ => Some(ScalarCarrier::Integer),
            };
        }
        if matches!(
            self.program.type_reference_table.type_reference(reference),
            TypeReferenceNode::Unit
        ) {
            return Some(ScalarCarrier::Unit);
        }
        if let TypeReferenceNode::Named { symbol, .. } =
            self.program.type_reference_table.type_reference(reference)
        {
            return match self.program.symbols.builtin_type_atom(*symbol) {
                Some(BuiltinTypeAtom::Bool) => Some(ScalarCarrier::Boolean),
                Some(BuiltinTypeAtom::Address) => Some(ScalarCarrier::Address),
                Some(BuiltinTypeAtom::F32) => Some(ScalarCarrier::Float32),
                Some(BuiltinTypeAtom::F64) => Some(ScalarCarrier::Float64),
                Some(
                    BuiltinTypeAtom::I8
                    | BuiltinTypeAtom::I16
                    | BuiltinTypeAtom::I32
                    | BuiltinTypeAtom::I64
                    | BuiltinTypeAtom::U8
                    | BuiltinTypeAtom::U16
                    | BuiltinTypeAtom::U32
                    | BuiltinTypeAtom::U64
                    | BuiltinTypeAtom::Int
                    | BuiltinTypeAtom::UInt,
                ) => Some(ScalarCarrier::Integer),
                _ => self
                    .carriers
                    .get(symbol)
                    .map(|&position| self.carrier_class_at(position)),
            };
        }
        None
    }

    /// Which scalar carrier a signature position is: the shared `Int`,
    /// `bool` and `addr` carriers match their classes; every other
    /// interned carrier names its own position.
    pub(super) fn carrier_class_at(&self, position: u32) -> ScalarCarrier {
        if let Some(alias) = self
            .source_aliases
            .iter()
            .find(|alias| alias.declaration == position)
        {
            return alias.carrier;
        }
        if self.integer == Some(position) {
            ScalarCarrier::Integer
        } else if self.boolean == Some(position) {
            ScalarCarrier::Boolean
        } else if self.address == Some(position) {
            ScalarCarrier::Address
        } else if self.float32 == Some(position) {
            ScalarCarrier::Float32
        } else if self.float64 == Some(position) {
            ScalarCarrier::Float64
        } else if self.unit == Some(position) {
            ScalarCarrier::Unit
        } else {
            ScalarCarrier::Carrier(position)
        }
    }

    /// A diagnostic-friendly name for the carrier family.
    pub(super) fn carrier_name(&self, carrier: ScalarCarrier) -> &'static str {
        match carrier {
            ScalarCarrier::Integer => "integer",
            ScalarCarrier::Boolean => "boolean",
            ScalarCarrier::Address => "address",
            ScalarCarrier::Float32 => "f32",
            ScalarCarrier::Float64 => "f64",
            ScalarCarrier::Unit => "unit",
            ScalarCarrier::Carrier(..) => "scalar",
        }
    }

    /// The carrier's signature position, interning it on first use.
    pub(super) fn carrier_position(&mut self, carrier: ScalarCarrier) -> u32 {
        match carrier {
            ScalarCarrier::Integer => self.integer_carrier(),
            ScalarCarrier::Boolean => self.boolean_carrier(),
            ScalarCarrier::Address => self.address_carrier(),
            ScalarCarrier::Float32 => self.float32_carrier(),
            ScalarCarrier::Float64 => self.float64_carrier(),
            ScalarCarrier::Unit => self.unit_carrier(),
            ScalarCarrier::Carrier(position) => position,
        }
    }

    /// The carrier as a `Constant` term.
    pub(super) fn carrier_term(&mut self, carrier: ScalarCarrier) -> TermHandle {
        let position = self.carrier_position(carrier);
        self.constant(position)
    }

    /// `Int : Type 0` — the shared mathematical-integer carrier
    /// assumption every fixed integer element and relation names.
    pub(super) fn integer_carrier(&mut self) -> u32 {
        if self.integer.is_none() {
            self.integer = Some(self.push_type_carrier());
        }
        self.integer.unwrap()
    }

    /// `bool : Type 0`.
    pub(super) fn boolean_carrier(&mut self) -> u32 {
        if self.boolean.is_none() {
            self.boolean = Some(self.push_type_carrier());
        }
        self.boolean.unwrap()
    }

    /// `addr : Type 0` — distinct from `Int`, so address values never
    /// meet the fixed-integer order relations.
    pub(super) fn address_carrier(&mut self) -> u32 {
        if self.address.is_none() {
            self.address = Some(self.push_type_carrier());
        }
        self.address.unwrap()
    }

    /// `f32 : Type 0` — the float formats each intern one carrier, kept
    /// distinct so an `f32` element never denotes into `f64`.
    pub(super) fn float32_carrier(&mut self) -> u32 {
        if self.float32.is_none() {
            self.float32 = Some(self.push_type_carrier());
        }
        self.float32.unwrap()
    }

    /// `f64 : Type 0`.
    pub(super) fn float64_carrier(&mut self) -> u32 {
        if self.float64.is_none() {
            self.float64 = Some(self.push_type_carrier());
        }
        self.float64.unwrap()
    }

    /// `Unit : Type 0` — the unit type interns a dedicated carrier, so
    /// `()` binder domains and call-carried `()` results denote without
    /// claiming a fixed-integer or `Two` vocabulary position.
    pub(super) fn unit_carrier(&mut self) -> u32 {
        if self.unit.is_none() {
            self.unit = Some(self.push_type_carrier());
        }
        self.unit.unwrap()
    }

    /// `Π(_ : Int). Π(_ : Int). Type 0` — the shape `IntLt`/`IntLe`
    /// share.
    pub(super) fn integer_relation(&mut self) -> u32 {
        let int = self.integer_constant();
        let codomain = self.type_zero();
        let inner = self.arena.insert(Term::Pi {
            domain: int,
            codomain,
        });
        let ty = self.arena.insert(Term::Pi {
            domain: int,
            codomain: inner,
        });
        self.push_assumption(ty)
    }

    pub(super) fn integer_less_than(&mut self) -> u32 {
        if self.integer_less_than.is_none() {
            self.integer_less_than = Some(self.integer_relation());
        }
        self.integer_less_than.unwrap()
    }

    pub(super) fn integer_less_or_equal(&mut self) -> u32 {
        if self.integer_less_or_equal.is_none() {
            self.integer_less_or_equal = Some(self.integer_relation());
        }
        self.integer_less_or_equal.unwrap()
    }

    /// `Not : Π(_ : Type 0). Type 0` — the negation assumption `!=`
    /// denotes through, interned on demand like `IntLt`/`IntLe`.
    pub(super) fn proposition_negation(&mut self) -> u32 {
        if self.proposition_negation.is_none() {
            let domain = self.type_zero();
            let codomain = self.type_zero();
            let ty = self.arena.insert(Term::Pi { domain, codomain });
            self.proposition_negation = Some(self.push_assumption(ty));
        }
        self.proposition_negation.unwrap()
    }

    /// `Π(_ : Int). Π(_ : Int). Int` — the shape `IntAdd`/`IntSub`
    /// share.
    pub(super) fn integer_operation(&mut self) -> u32 {
        let int = self.integer_constant();
        let inner = self.arena.insert(Term::Pi {
            domain: int,
            codomain: int,
        });
        let ty = self.arena.insert(Term::Pi {
            domain: int,
            codomain: inner,
        });
        self.push_assumption(ty)
    }

    pub(super) fn integer_add(&mut self) -> u32 {
        if self.integer_add.is_none() {
            self.integer_add = Some(self.integer_operation());
        }
        self.integer_add.unwrap()
    }

    pub(super) fn integer_subtract(&mut self) -> u32 {
        if self.integer_subtract.is_none() {
            self.integer_subtract = Some(self.integer_operation());
        }
        self.integer_subtract.unwrap()
    }

    /// `Constant` term for a signature position.
    pub(super) fn constant(&mut self, position: u32) -> TermHandle {
        self.arena.insert(Term::Constant {
            declaration: position,
            levels: Vec::new(),
        })
    }

    /// `Type 0` as a term.
    pub(super) fn type_zero(&mut self) -> TermHandle {
        self.arena
            .insert(Term::Sort(Sort::Type(Level::Constant(0))))
    }

    /// Push an assumption declaration and return its position.
    pub(super) fn push_assumption(&mut self, ty: TermHandle) -> u32 {
        let position = self.declarations.len() as u32;
        self.declarations.push(Declaration::assumption(0, ty));
        position
    }

    /// A fresh `Type 0` carrier assumption's position.
    pub(super) fn push_type_carrier(&mut self) -> u32 {
        let ty = self.type_zero();
        self.push_assumption(ty)
    }

    /// `Int`-constant the `Int` carrier names — interned once.
    pub(super) fn integer_constant(&mut self) -> TermHandle {
        let position = self.integer_carrier();
        self.constant(position)
    }

    /// One closed scalar literal's constant, interned by exact value at
    /// its carrier — `2 + 0` and `2` name one constant, so evaluated
    /// equalities stay `refl`-provable without a decision assumption.
    pub(super) fn numeric_literal(
        &mut self,
        carrier: u32,
        value: BigInt,
    ) -> Result<TermHandle, Vec<diagnostics::Diagnostic>> {
        if self.integer == Some(carrier)
            && let Some(theory) = &mut self.integer_theory
        {
            let term = theory
                .literal(&mut self.arena, &mut self.declarations, value.clone())
                .map_err(|error| {
                    self.refuse(format!(
                        "source integer literal could not be denoted: {error}"
                    ))
                })?;
            let Term::Constant { declaration, .. } = self.arena.get(term) else {
                return Err(
                    self.refuse("shared integer literal lost its canonical declaration".into())
                );
            };
            self.numeric_literals.insert((carrier, value), declaration);
            return Ok(term);
        }
        Ok(interned_literal!(
            self.numeric_literals,
            (carrier, value),
            carrier
        ))
    }

    /// `true`/`false` interned at the `bool` carrier.
    pub(super) fn boolean_literal(&mut self, value: bool) -> TermHandle {
        if let Some(position) = self.boolean_literals[value as usize] {
            return self.constant(position);
        }
        let carrier = self.boolean_carrier();
        let ty = self.constant(carrier);
        let position = self.push_assumption(ty);
        self.boolean_literals[value as usize] = Some(position);
        self.constant(position)
    }
}
