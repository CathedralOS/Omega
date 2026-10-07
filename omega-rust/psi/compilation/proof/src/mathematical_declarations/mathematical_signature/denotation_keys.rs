//! Denotation key writing: the canonical byte key every denoted term
//! takes, so structurally identical open terms intern to one constant.

use numerics::literals::FloatFormat;

use proof_admission::{Level, Term, TermHandle};
use typed_trees::expression::{ExpressionHandle, ExpressionNode, StaticMachineArgument};
use typed_trees::name::Identifier;

use super::{Elaborator, ScalarCarrier};

impl<'a> Elaborator<'a> {
    /// The carrier prefix of an open-term key — structure alone cannot
    /// collide across carriers, but the prefix makes the invariant
    /// explicit rather than relying on carrier determinism.
    pub(super) fn write_carrier_key(&self, carrier: ScalarCarrier, out: &mut Vec<u8>) {
        match carrier {
            ScalarCarrier::Integer => out.push(0),
            ScalarCarrier::Boolean => out.push(1),
            ScalarCarrier::Address => out.push(2),
            ScalarCarrier::Float32 => out.push(3),
            ScalarCarrier::Float64 => out.push(4),
            ScalarCarrier::Unit => out.push(5),
            ScalarCarrier::Carrier(position) => {
                out.push(6);
                out.extend_from_slice(&position.to_le_bytes());
            }
        }
    }

    /// A structural key identifying the open term's opaque constant: two
    /// expressions intern together exactly when their shapes agree.
    /// Names key by resolved symbol (one declaration's `x` never merges
    /// with another's), literals key by exact value (`0x2` and `2`
    /// share), and composite forms write head, operator and children.
    /// An absent child — an open range's missing endpoint — writes a
    /// distinct tag rather than dereferencing an invalid handle.
    pub(super) fn write_expression_key(&self, handle: ExpressionHandle, out: &mut Vec<u8>) {
        if !handle.is_valid() {
            out.push(0xff);
            return;
        }
        match self.program.expression_table.expression(handle) {
            ExpressionNode::Match(expression) => {
                out.push(0);
                self.write_expression_key(expression.subject, out);
                let arms = self.program.expression_table.match_arms(expression.arms);
                out.extend_from_slice(&(arms.len() as u32).to_le_bytes());
                for arm in arms {
                    match arm.pattern {
                        typed_trees::expression::MatchPattern::Value(pattern) => {
                            out.push(0);
                            self.write_expression_key(pattern, out);
                        }
                        typed_trees::expression::MatchPattern::Wildcard => out.push(1),
                    }
                    self.write_expression_key(arm.value, out);
                }
            }
            ExpressionNode::ArrayLiteral(elements) => {
                out.push(1);
                let elements = self.program.expression_table.expression_handles(*elements);
                out.extend_from_slice(&(elements.len() as u32).to_le_bytes());
                for element in elements {
                    self.write_expression_key(*element, out);
                }
            }
            ExpressionNode::Atomic(atomic) => {
                out.push(2);
                self.write_expression_key(atomic.value, out);
                self.write_expression_key(atomic.result, out);
            }
            ExpressionNode::Binary(binary) => {
                out.push(3);
                out.push(binary.operator as u8);
                self.write_expression_key(binary.left, out);
                self.write_expression_key(binary.right, out);
            }
            ExpressionNode::Boolean(value) => {
                out.push(4);
                out.push(*value as u8);
            }
            ExpressionNode::Cast(cast) => {
                out.push(5);
                self.write_expression_key(cast.value, out);
                out.extend_from_slice(&cast.target_type.arena_index().to_le_bytes());
                out.extend_from_slice(&cast.result_type.arena_index().to_le_bytes());
                out.push(cast.domain as u8);
            }
            ExpressionNode::Call(call) => {
                out.push(6);
                out.extend_from_slice(&call.target_symbol.arena_index().to_le_bytes());
                out.push(call.receiver.is_valid() as u8);
                if call.receiver.is_valid() {
                    self.write_expression_key(call.receiver, out);
                }
                out.push(call.quotient_operation.is_some() as u8);
                out.push(call.private_layout_operation.is_some() as u8);
                out.push(call.static_requirement_dispatch.is_some() as u8);
                out.extend_from_slice(&(call.evidence_arguments.len() as u32).to_le_bytes());
                for name in call.evidence_arguments.iter() {
                    self.write_identifier_key(name, out);
                }
                out.extend_from_slice(&(call.machine_arguments.len() as u32).to_le_bytes());
                for argument in call.machine_arguments.iter() {
                    self.write_machine_argument_key(argument, out);
                }
                let arguments = self
                    .program
                    .expression_table
                    .expression_handles(call.arguments);
                out.extend_from_slice(&(arguments.len() as u32).to_le_bytes());
                for argument in arguments {
                    self.write_expression_key(*argument, out);
                }
            }
            ExpressionNode::UndeterminedNaN(format) => {
                // Semantic identity has no raw-bit payload and is not an index atom.
                out.push(18);
                out.push(match format {
                    FloatFormat::F32 => 1,
                    FloatFormat::F64 => 2,
                });
            }
            ExpressionNode::Float(literal) => {
                out.push(7);
                // Literals denote by exact value — `1.5` and `1.50`
                // share — so the key is the spelling's correct rounding
                // at each format plus the landing, not the authored
                // text. Writing both reads keeps the key independent of
                // which float carrier encloses the open term.
                out.push(match literal.landing() {
                    None => 0,
                    Some(FloatFormat::F32) => 1,
                    Some(FloatFormat::F64) => 2,
                });
                out.extend_from_slice(&literal.f32_bits().to_le_bytes());
                out.extend_from_slice(&literal.value_f64().to_bits().to_le_bytes());
            }
            ExpressionNode::Indexed(indexed) => {
                out.push(8);
                self.write_expression_key(indexed.collection, out);
                self.write_expression_key(indexed.index, out);
            }
            ExpressionNode::Integer(literal) => {
                out.push(9);
                // Literals denote by exact value — `0x2` and `2` share —
                // so the key is the value, not the authored text.
                match literal.value_bignum() {
                    Some(value) => out.extend_from_slice(value.to_string().as_bytes()),
                    None => out.extend_from_slice(literal.text().as_bytes()),
                }
            }
            ExpressionNode::Member(member) => {
                out.push(10);
                self.write_expression_key(member.receiver, out);
                out.extend_from_slice(&member.member_symbol.arena_index().to_le_bytes());
                self.write_identifier_key(&member.member, out);
            }
            ExpressionNode::Borrow(borrow) => {
                out.push(11);
                self.write_expression_key(borrow.target, out);
            }
            ExpressionNode::Name(path) => {
                out.push(12);
                out.extend_from_slice(&path.symbol.arena_index().to_le_bytes());
                out.extend_from_slice(&path.head_symbol.arena_index().to_le_bytes());
                for symbol in self
                    .program
                    .expression_table
                    .name_path_member_symbols(path.member_symbols)
                {
                    out.extend_from_slice(&symbol.arena_index().to_le_bytes());
                }
                for member in self
                    .program
                    .expression_table
                    .name_path_members(path.members)
                {
                    self.write_identifier_key(member, out);
                }
            }
            ExpressionNode::Range(range) => {
                out.push(13);
                self.write_expression_key(range.start, out);
                self.write_expression_key(range.end, out);
                out.push(range.end_inclusive as u8);
            }
            ExpressionNode::StructLiteral(literal) => {
                out.push(14);
                out.extend_from_slice(&literal.type_symbol.arena_index().to_le_bytes());
                out.extend_from_slice(
                    &literal
                        .case_symbol
                        .map_or(u32::MAX, |symbol| symbol.arena_index())
                        .to_le_bytes(),
                );
                for field in self.program.expression_table.struct_fields(literal.fields) {
                    self.write_identifier_key(&field.name, out);
                    out.extend_from_slice(&field.field_symbol.arena_index().to_le_bytes());
                    self.write_expression_key(field.value, out);
                }
            }
            ExpressionNode::String(bytes) => {
                out.push(15);
                out.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
                out.extend_from_slice(bytes);
            }
            ExpressionNode::Unary(unary) => {
                out.push(16);
                out.push(unary.operator as u8);
                self.write_expression_key(unary.operand, out);
            }
            ExpressionNode::ZeroValue(reference) => {
                out.push(17);
                out.extend_from_slice(&reference.arena_index().to_le_bytes());
            }
        }
    }

    pub(super) fn write_identifier_key(&self, identifier: &Identifier, out: &mut Vec<u8>) {
        let text = identifier.as_str();
        out.extend_from_slice(&(text.len() as u32).to_le_bytes());
        out.extend_from_slice(text.as_bytes());
    }

    pub(super) fn write_machine_argument_key(
        &self,
        argument: &StaticMachineArgument,
        out: &mut Vec<u8>,
    ) {
        out.extend_from_slice(&argument.type_reference.arena_index().to_le_bytes());
        for name in argument.path.iter() {
            self.write_identifier_key(name, out);
        }
        out.extend_from_slice(&argument.symbol.arena_index().to_le_bytes());
        if let Some(literal) = &argument.const_literal
            && let Some(value) = literal.value_bignum()
        {
            out.extend_from_slice(value.to_string().as_bytes());
        }
        if let Some(application) = &argument.application {
            for name in application.lifetime_arguments.iter() {
                self.write_identifier_key(name, out);
            }
            for argument in application.arguments.iter() {
                self.write_machine_argument_key(argument, out);
            }
        }
        if let Some(projection) = &argument.evidence_projection {
            self.write_identifier_key(&projection.term, out);
            self.write_identifier_key(&projection.member, out);
        }
    }

    /// A structural key ordering denoted terms canonically — `x == y` and
    /// `y == x` denote one `Id`, and connective spines sort by it. The
    /// bounded denotation canonicalizes at the proposition level; the
    /// signature carries no proposition stage, so kernel terms order by
    /// structure here.
    pub(super) fn denotation_key(&self, term: TermHandle) -> Vec<u8> {
        let mut key = Vec::new();
        self.write_denotation_key(term, &mut key);
        key
    }

    pub(super) fn write_denotation_key(&self, term: TermHandle, out: &mut Vec<u8>) {
        match self.arena.get(term) {
            Term::Dummy => out.push(0),
            Term::Variable(index) => {
                out.push(1);
                out.extend_from_slice(&index.to_le_bytes());
            }
            Term::Sort(sort) => {
                out.push(2);
                out.push(sort.is_strict() as u8);
                self.write_level_key(sort.level(), out);
            }
            Term::Constant {
                declaration,
                levels,
            } => {
                out.push(3);
                out.extend_from_slice(&declaration.to_le_bytes());
                for level in levels {
                    self.write_level_key(level, out);
                }
            }
            Term::Pi { domain, codomain } => self.write_pair_key(4, domain, codomain, out),
            Term::Lambda { domain, body } => self.write_pair_key(5, domain, body, out),
            Term::Apply { function, argument } => self.write_pair_key(6, function, argument, out),
            Term::Sigma { domain, codomain } => self.write_pair_key(7, domain, codomain, out),
            Term::Pair { first, second } => self.write_pair_key(8, first, second, out),
            Term::Fst { pair } => {
                out.push(9);
                self.write_denotation_key(pair, out);
            }
            Term::Snd { pair } => {
                out.push(10);
                self.write_denotation_key(pair, out);
            }
            Term::Two => out.push(11),
            Term::TwoZero => out.push(12),
            Term::TwoOne => out.push(13),
            Term::CaseTwo {
                motive,
                zero_branch,
                one_branch,
                scrutinee,
            } => {
                out.push(14);
                self.write_denotation_key(motive, out);
                self.write_denotation_key(zero_branch, out);
                self.write_denotation_key(one_branch, out);
                self.write_denotation_key(scrutinee, out);
            }
            Term::Id { ty, left, right } => {
                out.push(15);
                self.write_denotation_key(ty, out);
                self.write_denotation_key(left, out);
                self.write_denotation_key(right, out);
            }
            Term::Refl { ty, value } => self.write_pair_key(16, ty, value, out),
            Term::IdElim {
                motive,
                base,
                endpoint,
                proof,
            } => {
                out.push(17);
                self.write_denotation_key(motive, out);
                self.write_denotation_key(base, out);
                self.write_denotation_key(endpoint, out);
                self.write_denotation_key(proof, out);
            }
            Term::W { carrier, children } => self.write_pair_key(18, carrier, children, out),
            Term::Sup {
                carrier,
                children,
                label,
                function,
            } => {
                out.push(19);
                self.write_denotation_key(carrier, out);
                self.write_denotation_key(children, out);
                self.write_denotation_key(label, out);
                self.write_denotation_key(function, out);
            }
            Term::IndW { motive, step, tree } => {
                out.push(20);
                self.write_denotation_key(motive, out);
                self.write_denotation_key(step, out);
                self.write_denotation_key(tree, out);
            }
            Term::Empty => out.push(21),
            Term::EmptyElim { ty, scrutinee } => self.write_pair_key(22, ty, scrutinee, out),
            Term::Squash { ty } => {
                out.push(23);
                self.write_denotation_key(ty, out);
            }
            Term::SquashIntro { ty, value } => self.write_pair_key(24, ty, value, out),
            Term::SquashElim {
                proposition,
                function,
                scrutinee,
            } => {
                out.push(25);
                self.write_denotation_key(proposition, out);
                self.write_denotation_key(function, out);
                self.write_denotation_key(scrutinee, out);
            }
            Term::Box { ty } => {
                out.push(26);
                self.write_denotation_key(ty, out);
            }
            Term::BoxIntro { ty, value } => self.write_pair_key(27, ty, value, out),
            Term::BoxElim {
                motive,
                body,
                scrutinee,
            } => {
                out.push(28);
                self.write_denotation_key(motive, out);
                self.write_denotation_key(body, out);
                self.write_denotation_key(scrutinee, out);
            }
        }
    }

    pub(super) fn write_pair_key(
        &self,
        tag: u8,
        first: TermHandle,
        second: TermHandle,
        out: &mut Vec<u8>,
    ) {
        out.push(tag);
        self.write_denotation_key(first, out);
        self.write_denotation_key(second, out);
    }

    pub(super) fn write_level_key(&self, level: Level, out: &mut Vec<u8>) {
        match level {
            Level::Constant(value) => {
                out.push(0);
                out.extend_from_slice(&value.to_le_bytes());
            }
            Level::Parameter(index) => {
                out.push(1);
                out.extend_from_slice(&index.to_le_bytes());
            }
            Level::Successor(inner) => {
                out.push(2);
                self.write_level_key(*inner, out);
            }
            Level::Maximum(left, right) => {
                out.push(3);
                self.write_level_key(*left, out);
                self.write_level_key(*right, out);
            }
        }
    }
}
