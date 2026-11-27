//! A plain, unindexed `data` declaration as a unified native inductive family.
//!
//! Each case contributes one constructor taking its fields in order: fields
//! that hold the data itself, and payload fields of other closed types. The
//! kernel derives the dependent recursor (`I.rec`) and its 1-step
//! constructor computation rules directly from the admitted block, and a
//! branch of an induction takes the case's fields and then one induction
//! hypothesis per field that holds the data.
//!
//! A generic declaration such as `Seq<T>` is one inductive block with one
//! uniform parameter per type parameter ([`GenericPlainData`]). Each of its
//! applications, such as `Seq<u64>`, is a [`PlainData`] whose entries are
//! definitions equal to the block's former, constructors and recursor at the
//! application's type arguments, laid out as a non-generic declaration's
//! entries are. Those definitions unfold to the block, so the kernel checks
//! and reduces only the block's own declarations.

use crate::inductive::{declare_inductive_family, declare_mutual_inductive_family};
use crate::signature::Declaration;
use crate::substitution::{instantiate, shift};
use crate::term::{Level, Sort, Term, TermArena, TermHandle};

/// One field of a case of plain data.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlainField {
    /// The field holds the data itself.
    Recursive,
    /// The field holds another member of the data's mutual block, by its
    /// position in the block.
    Member(u32),
    /// The field holds a value of this closed type.
    Payload(TermHandle),
}

impl PlainField {
    /// Whether the field holds the data or another member of its block: a
    /// field with an induction hypothesis.
    pub fn is_recursive(self) -> bool {
        matches!(self, Self::Recursive | Self::Member(_))
    }
}

/// The fields of each case, in order, of one member of a block of plain
/// data. A declaration on its own is a block of one member.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlainData {
    member: u32,
    /// Every member's cases. A field of another member is
    /// [`PlainField::Member`], and one of the member itself is
    /// [`PlainField::Recursive`].
    members: Vec<Vec<Vec<PlainField>>>,
}

fn apply(arena: &mut TermArena, function: TermHandle, argument: TermHandle) -> TermHandle {
    arena.insert(Term::Apply { function, argument })
}

fn lambda(arena: &mut TermArena, domain: TermHandle, body: TermHandle) -> TermHandle {
    arena.insert(Term::Lambda { domain, body })
}

fn pi(arena: &mut TermArena, domain: TermHandle, codomain: TermHandle) -> TermHandle {
    arena.insert(Term::Pi { domain, codomain })
}

fn variable(arena: &mut TermArena, index: u32) -> TermHandle {
    arena.insert(Term::Variable(index))
}

fn type_zero(arena: &mut TermArena) -> TermHandle {
    arena.insert(Term::Sort(Sort::Type(Level::Constant(0))))
}

impl PlainData {
    /// A declaration needs at least one case.
    pub fn new(cases: Vec<Vec<PlainField>>) -> Option<Self> {
        let mut block = Self::block(vec![cases])?;
        block.pop()
    }

    /// The members of one mutual block, each with at least one case. A field
    /// of member `m` names it as [`PlainField::Member`]`(m)`, or as
    /// [`PlainField::Recursive`] in member `m` itself.
    pub fn block(members: Vec<Vec<Vec<PlainField>>>) -> Option<Vec<Self>> {
        let count = u32::try_from(members.len()).ok()?;
        let total: usize = members.iter().map(Vec::len).sum();
        let valid = count > 0
            && u32::try_from(total).is_ok()
            && members.iter().enumerate().all(|(own, cases)| {
                !cases.is_empty()
                    && cases.iter().all(|fields| {
                        u32::try_from(fields.len()).is_ok()
                            && fields.iter().all(|field| match *field {
                                PlainField::Member(member) => {
                                    member < count && member as usize != own
                                }
                                _ => true,
                            })
                    })
            });
        valid.then(|| {
            (0..count)
                .map(|member| Self {
                    member,
                    members: members.clone(),
                })
                .collect()
        })
    }

    /// A declaration each of whose fields holds the data itself, with this
    /// many fields in each case.
    pub fn recursive(arities: Vec<u32>) -> Option<Self> {
        Self::new(
            arities
                .into_iter()
                .map(|arity| vec![PlainField::Recursive; arity as usize])
                .collect(),
        )
    }

    fn own(&self) -> &[Vec<PlainField>] {
        &self.members[self.member as usize]
    }

    pub fn cases(&self) -> usize {
        self.own().len()
    }

    /// This member's position in its block.
    pub fn member(&self) -> u32 {
        self.member
    }

    /// How many members the block has.
    pub fn members(&self) -> u32 {
        self.members.len() as u32
    }

    /// Whether the data is one member of a block of several.
    pub fn is_mutual(&self) -> bool {
        self.members.len() > 1
    }

    /// Member `member` of the same block.
    pub fn at_member(&self, member: u32) -> Option<Self> {
        ((member as usize) < self.members.len()).then(|| Self {
            member,
            members: self.members.clone(),
        })
    }

    /// How many fields the case has.
    pub fn arity(&self, case: usize) -> u32 {
        self.own()[case].len() as u32
    }

    /// The fields of the case.
    pub fn fields(&self, case: usize) -> &[PlainField] {
        &self.own()[case]
    }

    /// The member of the block a field of the case holds, if it holds one.
    pub fn field_member(&self, case: usize, field: u32) -> Option<u32> {
        match self.own()[case].get(field as usize)? {
            PlainField::Recursive => Some(self.member),
            PlainField::Member(member) => Some(*member),
            PlainField::Payload(_) => None,
        }
    }

    /// How many fields of the case hold the data or another member of its
    /// block: the number of induction hypotheses its branch takes.
    pub fn hypotheses(&self, case: usize) -> u32 {
        self.own()[case]
            .iter()
            .filter(|field| field.is_recursive())
            .count() as u32
    }

    /// Whether any field of any case of any member holds a value of another
    /// type.
    pub fn has_payload(&self) -> bool {
        self.members
            .iter()
            .flatten()
            .flatten()
            .any(|field| matches!(field, PlainField::Payload(_)))
    }

    /// The index of the first member's own entry when the block's
    /// declarations start at `first`.
    pub fn data_entry(first: u32) -> u32 {
        first
    }

    /// The index of the block's first declaration, given this member's entry.
    fn first(&self, data: u32) -> u32 {
        data - self.member
    }

    /// The index of member `member`'s own entry, given this member's entry.
    pub fn member_entry(&self, data: u32, member: u32) -> u32 {
        self.first(data) + member
    }

    /// The position among all of the block's constructors of this member's
    /// case `case`: the block numbers them member by member.
    fn constructor_position(&self, case: usize) -> u32 {
        let before: usize = self.members[..self.member as usize]
            .iter()
            .map(Vec::len)
            .sum();
        (before + case) as u32
    }

    fn constructor_count(&self) -> u32 {
        self.members.iter().map(Vec::len).sum::<usize>() as u32
    }

    /// The index of a case's constructor, given the data's own entry.
    pub fn constructor_entry(&self, data: u32, case: usize) -> u32 {
        self.first(data) + self.members() + self.constructor_position(case)
    }

    /// The index of the induction principle (recursor), given the data's own entry.
    pub fn induction_entry(&self, data: u32) -> u32 {
        self.first(data) + self.members() + self.constructor_count() + self.member
    }

    fn data_type(arena: &mut TermArena, data: u32) -> TermHandle {
        arena.insert(Term::Constant {
            declaration: data,
            levels: Vec::new(),
        })
    }

    /// The type of each field of the case, in order. Each is closed.
    pub fn field_types(&self, arena: &mut TermArena, data: u32, case: usize) -> Vec<TermHandle> {
        let first = self.first(data);
        self.own()[case]
            .iter()
            .map(|field| match *field {
                PlainField::Recursive => Self::data_type(arena, data),
                PlainField::Member(member) => Self::data_type(arena, first + member),
                PlainField::Payload(payload) => payload,
            })
            .collect()
    }

    /// Every declaration of the block, in order, for a signature in which the
    /// first of them has index `first`: each member's inductive type, every
    /// member's constructors, one per case, and each member's
    /// kernel-derived dependent recursor.
    pub fn declarations(&self, arena: &mut TermArena, first: u32) -> Vec<Declaration> {
        let universe = type_zero(arena);
        let mut constructors = Vec::with_capacity(self.constructor_count() as usize);
        for member in 0..self.members() {
            let shape = self.at_member(member).expect("a member of the block");
            let data = first + member;
            for case in 0..shape.cases() {
                let fields = shape.field_types(arena, data, case);
                let result = Self::data_type(arena, data);
                let ty = fields
                    .into_iter()
                    .rev()
                    .fold(result, |codomain, domain| pi(arena, domain, codomain));
                constructors.push((member as usize, ty));
            }
        }
        let formers = vec![universe; self.members.len()];
        declare_mutual_inductive_family(arena, first, 0, 0, &formers, &constructors)
    }

    /// The type of branch `case` of an induction over the data at entry
    /// `data` whose motive is `motive`, a function from the data to a type in
    /// the caller's context. The branch takes the case's fields in order,
    /// then the motive at each field that holds the data, and lands at the
    /// motive of the case. Only for a block of one member: see
    /// [`Self::branch_type_in`].
    pub fn branch_type(
        &self,
        arena: &mut TermArena,
        data: u32,
        motive: TermHandle,
        case: usize,
    ) -> TermHandle {
        debug_assert!(!self.is_mutual());
        self.branch_type_in(arena, data, &[motive], case)
    }

    /// The type of branch `case` under one motive per member of the block.
    pub fn branch_type_in(
        &self,
        arena: &mut TermArena,
        data: u32,
        motives: &[TermHandle],
        case: usize,
    ) -> TermHandle {
        let binders = self.case_binders_in(arena, data, motives, case);
        let depth = self.arity(case) + self.hypotheses(case);
        let motive_under = shift(arena, motives[self.member as usize], 0, depth);
        let value = self.case_value(arena, data, case, 0);
        let conclusion = apply(arena, motive_under, value);
        binders
            .into_iter()
            .rev()
            .fold(conclusion, |codomain, domain| pi(arena, domain, codomain))
    }

    /// The types of the binders a branch body lives under, each under the
    /// ones before it: the case's fields, then the motive at each field that
    /// holds the data. Only for a block of one member.
    pub fn case_binders(
        &self,
        arena: &mut TermArena,
        data: u32,
        motive: TermHandle,
        case: usize,
    ) -> Vec<TermHandle> {
        debug_assert!(!self.is_mutual());
        self.case_binders_in(arena, data, &[motive], case)
    }

    /// The binders of a branch under one motive per member of the block:
    /// the hypothesis at a field of member `m` is motive `m` at the field.
    pub fn case_binders_in(
        &self,
        arena: &mut TermArena,
        data: u32,
        motives: &[TermHandle],
        case: usize,
    ) -> Vec<TermHandle> {
        let arity = self.arity(case);
        let mut binders = self.field_types(arena, data, case);
        let mut hypothesis = 0;
        for field in 0..arity {
            let Some(member) = self.field_member(case, field) else {
                continue;
            };
            let motive = shift(arena, motives[member as usize], 0, arity + hypothesis);
            let value = variable(arena, arity + hypothesis - 1 - field);
            binders.push(apply(arena, motive, value));
            hypothesis += 1;
        }
        binders
    }

    /// A field of the case, seen from a branch body past `inner` further
    /// binders.
    pub fn field(&self, arena: &mut TermArena, case: usize, field: u32, inner: u32) -> TermHandle {
        variable(
            arena,
            inner + self.hypotheses(case) + self.arity(case) - 1 - field,
        )
    }

    /// The induction hypothesis for that field, seen from the same place,
    /// when the field holds the data or another member of its block.
    pub fn hypothesis(
        &self,
        arena: &mut TermArena,
        case: usize,
        field: u32,
        inner: u32,
    ) -> Option<TermHandle> {
        let fields = self.own()[case].as_slice();
        if !fields.get(field as usize)?.is_recursive() {
            return None;
        }
        let position = fields[..field as usize]
            .iter()
            .filter(|kind| kind.is_recursive())
            .count() as u32;
        Some(variable(
            arena,
            inner + self.hypotheses(case) - 1 - position,
        ))
    }

    /// The value of the case over its fields, seen from the same place.
    pub fn case_value(
        &self,
        arena: &mut TermArena,
        data: u32,
        case: usize,
        inner: u32,
    ) -> TermHandle {
        let constructor = arena.insert(Term::Constant {
            declaration: self.constructor_entry(data, case),
            levels: Vec::new(),
        });
        let fields: Vec<_> = (0..self.arity(case))
            .map(|field| self.field(arena, case, field, inner))
            .collect();
        arena.apply_arguments(constructor, &fields)
    }

    /// Wraps a branch body in its binders. `motive` is in the caller's
    /// context; `body` lives under the binders of `case_binders`. Only for a
    /// block of one member.
    pub fn branch(
        &self,
        arena: &mut TermArena,
        data: u32,
        motive: TermHandle,
        case: usize,
        body: TermHandle,
    ) -> TermHandle {
        debug_assert!(!self.is_mutual());
        self.branch_in(arena, data, &[motive], case, body)
    }

    /// Wraps a branch body in its binders under one motive per member.
    pub fn branch_in(
        &self,
        arena: &mut TermArena,
        data: u32,
        motives: &[TermHandle],
        case: usize,
        body: TermHandle,
    ) -> TermHandle {
        let binders = self.case_binders_in(arena, data, motives, case);
        binders
            .into_iter()
            .rev()
            .fold(body, |body, domain| lambda(arena, domain, body))
    }

    /// The induction over `subject` with one closed-over branch per case,
    /// each built by `branch`. Its type is `motive(subject)`. Only for a
    /// block of one member.
    pub fn induct(
        &self,
        arena: &mut TermArena,
        data: u32,
        motive: TermHandle,
        branches: &[TermHandle],
        subject: TermHandle,
    ) -> TermHandle {
        self.induct_at_level(arena, data, Level::Constant(0), motive, branches, subject)
    }

    /// The induction over `subject` into a motive at universe level `level`.
    /// Only for a block of one member.
    pub fn induct_at_level(
        &self,
        arena: &mut TermArena,
        data: u32,
        level: Level,
        motive: TermHandle,
        branches: &[TermHandle],
        subject: TermHandle,
    ) -> TermHandle {
        debug_assert!(!self.is_mutual());
        self.induct_in(arena, data, level, &[motive], branches, subject)
    }

    /// The induction over `subject`, a value of this member, under one
    /// motive per member and one branch per constructor of the block, member
    /// by member. Its type is this member's motive at `subject`.
    pub fn induct_in(
        &self,
        arena: &mut TermArena,
        data: u32,
        level: Level,
        motives: &[TermHandle],
        branches: &[TermHandle],
        subject: TermHandle,
    ) -> TermHandle {
        debug_assert_eq!(motives.len(), self.members.len());
        debug_assert_eq!(branches.len(), self.constructor_count() as usize);
        let induction = arena.insert(Term::Constant {
            declaration: self.induction_entry(data),
            levels: vec![level],
        });
        let mut term = induction;
        for &motive in motives {
            term = apply(arena, term, motive);
        }
        for &branch in branches {
            term = apply(arena, term, branch);
        }
        apply(arena, term, subject)
    }

    /// Derived kernel proof that this plain inductive data is a set
    /// (`Π (x y: D) (p q: Id D x y). Id (Id D x y) p q`) via Hedberg's
    /// theorem, using only `D.rec` and `J` with zero assumptions (no UIP).
    /// Only data whose fields all hold the data itself has it: a payload's
    /// equality would have to be decided too, and nothing here decides it.
    pub fn is_set(&self, arena: &mut TermArena, data: u32) -> Option<TermHandle> {
        if self.has_payload() || self.is_mutual() {
            return None;
        }
        let ty = arena.insert(Term::Constant {
            declaration: data,
            levels: Vec::new(),
        });
        let f = self.build_all_f(arena, data, ty);
        let constant = self.build_all_const(arena, data, ty, f);
        Some(hedberg(arena, ty, f, constant))
    }

    /// Derives `f : Π (x y: D). Id D x y -> Id D x y` by structural induction on `x`.
    fn build_all_f(&self, arena: &mut TermArena, data: u32, ty: TermHandle) -> TermHandle {
        // Motive `λx: D. (y: D) -> Id D x y -> Id D x y`
        let motive = {
            let x = variable(arena, 1);
            let y = variable(arena, 0);
            let eq = id_term(arena, ty, x, y);
            let eq_1 = shift(arena, eq, 0, 1);
            let arrow = pi(arena, eq, eq_1);
            let pi_y = pi(arena, ty, arrow);
            lambda(arena, ty, pi_y)
        };
        let mut branches = Vec::with_capacity(self.cases());
        for case in 0..self.cases() {
            let arity = self.arity(case);
            let x_val = self.case_value(arena, data, case, 0);
            let fx = self.build_fx(arena, data, ty, case, arity, x_val);
            branches.push(self.branch(arena, data, motive, case, fx));
        }
        let x = variable(arena, 0);
        let motive_1 = shift(arena, motive, 0, 1);
        let branches_1: Vec<_> = branches.iter().map(|&b| shift(arena, b, 0, 1)).collect();
        let body = self.induct(arena, data, motive_1, &branches_1, x);
        lambda(arena, ty, body)
    }

    /// Derives `c : Π (x y: D) (p q: Id D x y). Id (Id D x y) (f x y p) (f x y q)`
    /// by structural induction on `x`.
    fn build_all_const(
        &self,
        arena: &mut TermArena,
        data: u32,
        ty: TermHandle,
        f: TermHandle,
    ) -> TermHandle {
        // Motive `λx: D. (y: D) -> (p q: Id D x y) -> Id (Id D x y) (f x y p) (f x y q)`
        let motive = {
            let x1 = variable(arena, 1);
            let y0 = variable(arena, 0);
            let eq_xy = id_term(arena, ty, x1, y0);
            let eq_xy_1 = shift(arena, eq_xy, 0, 1);
            let x3 = variable(arena, 3);
            let y2 = variable(arena, 2);
            let p1 = variable(arena, 1);
            let q0 = variable(arena, 0);
            let eq_xy_2 = id_term(arena, ty, x3, y2);
            let f_4 = shift(arena, f, 0, 4);
            let f_x_y_p = arena.apply_arguments(f_4, &[x3, y2, p1]);
            let f_x_y_q = arena.apply_arguments(f_4, &[x3, y2, q0]);
            let goal = id_term(arena, eq_xy_2, f_x_y_p, f_x_y_q);
            let pi_q = pi(arena, eq_xy_1, goal);
            let pi_p = pi(arena, eq_xy, pi_q);
            let pi_y = pi(arena, ty, pi_p);
            lambda(arena, ty, pi_y)
        };
        let mut branches = Vec::with_capacity(self.cases());
        for case in 0..self.cases() {
            let arity = self.arity(case);
            let x_val = self.case_value(arena, data, case, 0);
            let cx = self.build_cx(arena, data, ty, f, case, arity, x_val);
            branches.push(self.branch(arena, data, motive, case, cx));
        }
        let x = variable(arena, 0);
        let motive_1 = shift(arena, motive, 0, 1);
        let branches_1: Vec<_> = branches.iter().map(|&b| shift(arena, b, 0, 1)).collect();
        let body = self.induct(arena, data, motive_1, &branches_1, x);
        lambda(arena, ty, body)
    }

    fn build_fx(
        &self,
        arena: &mut TermArena,
        data: u32,
        ty: TermHandle,
        case: usize,
        arity: u32,
        x_val: TermHandle,
    ) -> TermHandle {
        let inner_motive = {
            let x_1 = shift(arena, x_val, 0, 1);
            let y0 = variable(arena, 0);
            let eq = id_term(arena, ty, x_1, y0);
            let eq_1 = shift(arena, eq, 0, 1);
            let arrow = pi(arena, eq, eq_1);
            lambda(arena, ty, arrow)
        };
        let mut inner_branches = Vec::with_capacity(self.cases());
        for other in 0..self.cases() {
            let other_arity = self.arity(other);
            let inner_depth = 2 * other_arity;
            let x_here = shift(arena, x_val, 0, inner_depth);
            let y_here = self.case_value(arena, data, other, 0);
            let eq_xy = id_term(arena, ty, x_here, y_here);
            let body = if other != case {
                let p = variable(arena, 0);
                lambda(arena, eq_xy, p)
            } else if arity == 0 {
                let r = arena.insert(Term::Refl { ty, value: x_here });
                lambda(arena, eq_xy, r)
            } else {
                let d = inner_depth + 1;
                let x_p = shift(arena, x_val, 0, d);
                let y_p = self.case_value(arena, data, other, 1);
                let p = variable(arena, 0);
                let mut xs = Vec::with_capacity(arity as usize);
                let mut ys = Vec::with_capacity(arity as usize);
                let mut rs = Vec::with_capacity(arity as usize);
                for field in 0..arity {
                    let x_j = self.field(arena, case, field, d);
                    let y_j = self.field(arena, other, field, 1);
                    let proj_j = self.projection(arena, data, ty, case, field, x_j);
                    let p_j = ap(arena, ty, ty, proj_j, x_p, y_p, p);
                    // `ih_j` directly has type `(y: D) -> Id D x_j y -> Id D x_j y`.
                    let ih_j = self
                        .hypothesis(arena, case, field, d)
                        .expect("every field holds the data");
                    let r_j = arena.apply_arguments(ih_j, &[y_j, p_j]);
                    xs.push(x_j);
                    ys.push(y_j);
                    rs.push(r_j);
                }
                let constructor = arena.insert(Term::Constant {
                    declaration: self.constructor_entry(data, case),
                    levels: Vec::new(),
                });
                let combined = congr_many(arena, ty, ty, constructor, &xs, &ys, &rs);
                lambda(arena, eq_xy, combined)
            };
            inner_branches.push(self.branch(arena, data, inner_motive, other, body));
        }
        let y = variable(arena, 0);
        let m1 = shift(arena, inner_motive, 0, 1);
        let bs1: Vec<_> = inner_branches
            .iter()
            .map(|&b| shift(arena, b, 0, 1))
            .collect();
        let body = self.induct(arena, data, m1, &bs1, y);
        lambda(arena, ty, body)
    }

    fn build_cx(
        &self,
        arena: &mut TermArena,
        data: u32,
        ty: TermHandle,
        f: TermHandle,
        case: usize,
        arity: u32,
        x_val: TermHandle,
    ) -> TermHandle {
        let inner_motive = {
            let x_1 = shift(arena, x_val, 0, 1);
            let y0 = variable(arena, 0);
            let eq_1 = id_term(arena, ty, x_1, y0);
            let x_3 = shift(arena, x_val, 0, 3);
            let y2 = variable(arena, 2);
            let p1 = variable(arena, 1);
            let q0 = variable(arena, 0);
            let eq_3 = id_term(arena, ty, x_3, y2);
            let f_here = shift(arena, f, 0, 2 * arity + 3);
            let f_x_y_p = arena.apply_arguments(f_here, &[x_3, y2, p1]);
            let f_x_y_q = arena.apply_arguments(f_here, &[x_3, y2, q0]);
            let goal = id_term(arena, eq_3, f_x_y_p, f_x_y_q);
            let eq_1_shifted = shift(arena, eq_1, 0, 1);
            let pi_q = pi(arena, eq_1_shifted, goal);
            let pi_p = pi(arena, eq_1, pi_q);
            lambda(arena, ty, pi_p)
        };
        let mut inner_branches = Vec::with_capacity(self.cases());
        for other in 0..self.cases() {
            let other_arity = self.arity(other);
            let inner_depth = 2 * other_arity;
            let x_here = shift(arena, x_val, 0, inner_depth);
            let y_here = self.case_value(arena, data, other, 0);
            let eq_xy = id_term(arena, ty, x_here, y_here);
            let eq_xy_1 = shift(arena, eq_xy, 0, 1);
            let body = if other != case {
                let x_2 = shift(arena, x_here, 0, 2);
                let y_2 = shift(arena, y_here, 0, 2);
                let eq_2 = id_term(arena, ty, x_2, y_2);
                let p = variable(arena, 1);
                let q = variable(arena, 0);
                let goal = id_term(arena, eq_2, p, q);
                let bot = self.disjoint_constructors(arena, data, ty, case, x_2, y_2, p);
                let proof = apply(arena, bot, goal);
                let inner = lambda(arena, eq_xy_1, proof);
                lambda(arena, eq_xy, inner)
            } else if arity == 0 {
                let r = arena.insert(Term::Refl { ty, value: x_here });
                let proof = arena.insert(Term::Refl {
                    ty: eq_xy,
                    value: r,
                });
                let proof = shift(arena, proof, 0, 2);
                let inner = lambda(arena, eq_xy_1, proof);
                lambda(arena, eq_xy, inner)
            } else {
                let d = inner_depth + 2;
                let x_pq = shift(arena, x_val, 0, d);
                let y_pq = self.case_value(arena, data, other, 2);
                let p = variable(arena, 1);
                let q = variable(arena, 0);
                let f_here = shift(arena, f, 0, 2 * arity + d);
                let mut xs = Vec::with_capacity(arity as usize);
                let mut ys = Vec::with_capacity(arity as usize);
                let mut eq_tys = Vec::with_capacity(arity as usize);
                let mut r_ps = Vec::with_capacity(arity as usize);
                let mut r_qs = Vec::with_capacity(arity as usize);
                let mut cs = Vec::with_capacity(arity as usize);
                for field in 0..arity {
                    let x_j = self.field(arena, case, field, d);
                    let y_j = self.field(arena, other, field, 2);
                    let proj_j = self.projection(arena, data, ty, case, field, x_j);
                    let p_j = ap(arena, ty, ty, proj_j, x_pq, y_pq, p);
                    let q_j = ap(arena, ty, ty, proj_j, x_pq, y_pq, q);
                    // `ih_j` directly has type `(y: D) -> (p q: Id D x_j y) -> Id (Id D x_j y) (f x_j y p) (f x_j y q)`.
                    let ih_j = self
                        .hypothesis(arena, case, field, d)
                        .expect("every field holds the data");
                    let r_pj = arena.apply_arguments(f_here, &[x_j, y_j, p_j]);
                    let r_qj = arena.apply_arguments(f_here, &[x_j, y_j, q_j]);
                    let c_pqj = arena.apply_arguments(ih_j, &[y_j, p_j, q_j]);
                    xs.push(x_j);
                    ys.push(y_j);
                    eq_tys.push(id_term(arena, ty, x_j, y_j));
                    r_ps.push(r_pj);
                    r_qs.push(r_qj);
                    cs.push(c_pqj);
                }
                let constructor = arena.insert(Term::Constant {
                    declaration: self.constructor_entry(data, case),
                    levels: Vec::new(),
                });
                let result_eq_ty = id_term(arena, ty, x_pq, y_pq);
                let g = {
                    let k = arity;
                    let xs_k: Vec<_> = xs.iter().map(|&t| shift(arena, t, 0, k)).collect();
                    let ys_k: Vec<_> = ys.iter().map(|&t| shift(arena, t, 0, k)).collect();
                    let rs_k: Vec<_> = (0..k).map(|j| variable(arena, k - 1 - j)).collect();
                    let mut body = congr_many(arena, ty, ty, constructor, &xs_k, &ys_k, &rs_k);
                    for j in (0..k as usize).rev() {
                        let dom = shift(arena, eq_tys[j], 0, j as u32);
                        body = lambda(arena, dom, body);
                    }
                    body
                };
                let proof = congr_many_typed(arena, &eq_tys, result_eq_ty, g, &r_ps, &r_qs, &cs);
                let inner = lambda(arena, eq_xy_1, proof);
                lambda(arena, eq_xy, inner)
            };
            inner_branches.push(self.branch(arena, data, inner_motive, other, body));
        }
        let y = variable(arena, 0);
        let m1 = shift(arena, inner_motive, 0, 1);
        let bs1: Vec<_> = inner_branches
            .iter()
            .map(|&b| shift(arena, b, 0, 1))
            .collect();
        let body = self.induct(arena, data, m1, &bs1, y);
        lambda(arena, ty, body)
    }

    /// `proj : D -> D` returning field `field` on `case` and `default` on all other cases.
    fn projection(
        &self,
        arena: &mut TermArena,
        data: u32,
        ty: TermHandle,
        case: usize,
        field: u32,
        default: TermHandle,
    ) -> TermHandle {
        let motive = lambda(arena, ty, ty);
        let mut branches = Vec::with_capacity(self.cases());
        for c in 0..self.cases() {
            let body = if c == case {
                self.field(arena, c, field, 0)
            } else {
                shift(arena, default, 0, 2 * self.arity(c))
            };
            branches.push(self.branch(arena, data, motive, c, body));
        }
        let subject = variable(arena, 0);
        let bs1: Vec<_> = branches.iter().map(|&b| shift(arena, b, 0, 1)).collect();
        let body = self.induct(arena, data, motive, &bs1, subject);
        lambda(arena, ty, body)
    }

    /// Given `p : Id D x y` where `x` has head constructor `case` and `y` has
    /// a different head constructor, derives `Π (X: Type 0). X`.
    fn disjoint_constructors(
        &self,
        arena: &mut TermArena,
        data: u32,
        ty: TermHandle,
        case: usize,
        x: TermHandle,
        y: TermHandle,
        p: TermHandle,
    ) -> TermHandle {
        let type0 = type_zero(arena);
        let type1 = arena.insert(Term::Sort(Sort::Type(Level::Constant(1))));
        let unit1 = {
            let x_0 = variable(arena, 0);
            let x_1 = variable(arena, 1);
            let arrow = pi(arena, x_0, x_1);
            pi(arena, type0, arrow)
        };
        let empty1 = {
            let x_var = variable(arena, 0);
            pi(arena, type0, x_var)
        };
        let unit1_val = {
            let x_var = variable(arena, 0);
            let v_var = variable(arena, 0);
            let inner = lambda(arena, x_var, v_var);
            lambda(arena, type0, inner)
        };
        let disc_motive = lambda(arena, ty, type1);
        let mut disc_branches = Vec::with_capacity(self.cases());
        for c in 0..self.cases() {
            let body = if c == case { unit1 } else { empty1 };
            disc_branches.push(self.branch(arena, data, disc_motive, c, body));
        }
        let disc = {
            let subject = variable(arena, 0);
            let body = self.induct_at_level(
                arena,
                data,
                Level::Constant(2),
                disc_motive,
                &disc_branches,
                subject,
            );
            lambda(arena, ty, body)
        };
        let j_motive = {
            let x_1 = shift(arena, x, 0, 1);
            let y0 = variable(arena, 0);
            let eq = id_term(arena, ty, x_1, y0);
            let y1 = variable(arena, 1);
            let res = apply(arena, disc, y1);
            let inner = lambda(arena, eq, res);
            lambda(arena, ty, inner)
        };
        arena.insert(Term::IdElim {
            motive: j_motive,
            base: unit1_val,
            endpoint: y,
            proof: p,
        })
    }
}

/// One field of a case of a generic plain data declaration.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GenericPlainField {
    /// The field holds the data itself at the declaration's own parameters.
    Recursive,
    /// The field holds a value of the type parameter at this position.
    Parameter(u32),
    /// The field holds a value of this closed type, whatever the arguments.
    Closed(TermHandle),
}

/// A generic plain data declaration: `parameters` uniform type parameters,
/// each ranging over `Type 0`, and the fields of each case.
///
/// The kernel declares it as one inductive block whose former is
/// `Π(T_0 .. T_{n-1} : Type 0). Type 0`, with each constructor and the
/// recursor taking the parameters first. The kernel's own block check
/// decides that each constructor returns the family at exactly these
/// parameters and that every recursive occurrence is strictly positive.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GenericPlainData {
    parameters: u32,
    cases: Vec<Vec<GenericPlainField>>,
}

impl GenericPlainData {
    /// A declaration needs at least one parameter and one case, and each
    /// parameter a field names must be one of its own.
    pub fn new(parameters: u32, cases: Vec<Vec<GenericPlainField>>) -> Option<Self> {
        let named_parameters = cases.iter().flatten().all(|field| match field {
            GenericPlainField::Parameter(position) => *position < parameters,
            _ => true,
        });
        (parameters > 0
            && !cases.is_empty()
            && named_parameters
            && u32::try_from(cases.len()).is_ok()
            && cases
                .iter()
                .all(|fields| u32::try_from(fields.len()).is_ok()))
        .then_some(Self { parameters, cases })
    }

    pub fn parameters(&self) -> u32 {
        self.parameters
    }

    pub fn cases(&self) -> usize {
        self.cases.len()
    }

    /// How many declarations the block takes: the former, one constructor
    /// per case and the recursor. An application takes as many.
    pub fn width(&self) -> u32 {
        2 + self.cases.len() as u32
    }

    /// The block's declarations when its former has index `first`.
    pub fn declarations(&self, arena: &mut TermArena, first: u32) -> Vec<Declaration> {
        let universe = type_zero(arena);
        let mut former = universe;
        for _ in 0..self.parameters {
            former = pi(arena, universe, former);
        }
        let n = self.parameters;
        // The family at its own parameters, seen past `inner` binders after
        // them.
        let at_parameters = |arena: &mut TermArena, inner: u32| {
            let data = PlainData::data_type(arena, first);
            let parameters: Vec<_> = (0..n)
                .map(|position| variable(arena, inner + n - 1 - position))
                .collect();
            arena.apply_arguments(data, &parameters)
        };
        let constructor_types: Vec<TermHandle> = self
            .cases
            .iter()
            .map(|fields| {
                let domains: Vec<_> = fields
                    .iter()
                    .enumerate()
                    .map(|(before, field)| {
                        let before = before as u32;
                        match *field {
                            GenericPlainField::Recursive => at_parameters(arena, before),
                            GenericPlainField::Parameter(position) => {
                                variable(arena, before + n - 1 - position)
                            }
                            GenericPlainField::Closed(ty) => ty,
                        }
                    })
                    .collect();
                let mut ty = at_parameters(arena, fields.len() as u32);
                for &domain in domains.iter().rev() {
                    ty = pi(arena, domain, ty);
                }
                for _ in 0..n {
                    ty = pi(arena, universe, ty);
                }
                ty
            })
            .collect();
        declare_inductive_family(arena, first, 0, n, former, &constructor_types)
    }

    /// The application of the block whose former has index `generic` to the
    /// closed types `arguments`, as definitions starting at index `first`:
    /// the family at the arguments, each constructor at them, and the
    /// recursor at them. `recursor_type` is the type of the block's
    /// recursor. The returned shape names the payload of each field at the
    /// arguments, so the application reads as plain data whose entry is
    /// `first`.
    pub fn instance(
        &self,
        arena: &mut TermArena,
        generic: u32,
        recursor_type: TermHandle,
        first: u32,
        arguments: &[TermHandle],
    ) -> Option<(PlainData, Vec<Declaration>)> {
        if arguments.len() != self.parameters as usize {
            return None;
        }
        let shape = PlainData::new(
            self.cases
                .iter()
                .map(|fields| {
                    fields
                        .iter()
                        .map(|field| match *field {
                            GenericPlainField::Recursive => PlainField::Recursive,
                            GenericPlainField::Parameter(position) => {
                                PlainField::Payload(arguments[position as usize])
                            }
                            GenericPlainField::Closed(ty) => PlainField::Payload(ty),
                        })
                        .collect()
                })
                .collect(),
        )?;
        let universe = type_zero(arena);
        let at = |arena: &mut TermArena, declaration: u32, levels: Vec<Level>| {
            let head = arena.insert(Term::Constant {
                declaration,
                levels,
            });
            arena.apply_arguments(head, arguments)
        };
        let mut declarations = Vec::with_capacity(self.width() as usize);
        let family = at(arena, generic, Vec::new());
        declarations.push(Declaration::definition(0, universe, family));
        for case in 0..self.cases() {
            let fields = shape.field_types(arena, first, case);
            let data = PlainData::data_type(arena, first);
            let ty = fields
                .into_iter()
                .rev()
                .fold(data, |codomain, domain| pi(arena, domain, codomain));
            let value = at(arena, generic + 1 + case as u32, Vec::new());
            declarations.push(Declaration::definition(0, ty, value));
        }
        let mut body = recursor_type;
        for _ in 0..self.parameters {
            let Term::Pi { codomain, .. } = arena.get(body) else {
                return None;
            };
            body = codomain;
        }
        let ty = instantiate(arena, body, arguments);
        let value = at(
            arena,
            generic + 1 + self.cases.len() as u32,
            vec![Level::Parameter(0)],
        );
        declarations.push(Declaration::definition(1, ty, value));
        Some((shape, declarations))
    }
}

fn id_term(
    arena: &mut TermArena,
    ty: TermHandle,
    left: TermHandle,
    right: TermHandle,
) -> TermHandle {
    arena.insert(Term::Id { ty, left, right })
}

fn id_sym(
    arena: &mut TermArena,
    ty: TermHandle,
    x: TermHandle,
    y: TermHandle,
    p: TermHandle,
) -> TermHandle {
    let ty_1 = shift(arena, ty, 0, 1);
    let ty_2 = shift(arena, ty, 0, 2);
    let x_1 = shift(arena, x, 0, 1);
    let x_2 = shift(arena, x, 0, 2);
    let v0 = variable(arena, 0);
    let v1 = variable(arena, 1);
    let eq = id_term(arena, ty_1, x_1, v0);
    let goal = id_term(arena, ty_2, v1, x_2);
    let inner = lambda(arena, eq, goal);
    let motive = lambda(arena, ty, inner);
    let base = arena.insert(Term::Refl { ty, value: x });
    arena.insert(Term::IdElim {
        motive,
        base,
        endpoint: y,
        proof: p,
    })
}

fn id_trans(
    arena: &mut TermArena,
    ty: TermHandle,
    x: TermHandle,
    y: TermHandle,
    z: TermHandle,
    p: TermHandle,
    q: TermHandle,
) -> TermHandle {
    let ty_1 = shift(arena, ty, 0, 1);
    let ty_2 = shift(arena, ty, 0, 2);
    let y_1 = shift(arena, y, 0, 1);
    let x_2 = shift(arena, x, 0, 2);
    let v0 = variable(arena, 0);
    let v1 = variable(arena, 1);
    let eq = id_term(arena, ty_1, y_1, v0);
    let goal = id_term(arena, ty_2, x_2, v1);
    let inner = lambda(arena, eq, goal);
    let motive = lambda(arena, ty, inner);
    arena.insert(Term::IdElim {
        motive,
        base: p,
        endpoint: z,
        proof: q,
    })
}

/// `Id (Id ty y y) (id_trans (id_sym p) p) (refl y)`
fn id_cancel_left(
    arena: &mut TermArena,
    ty: TermHandle,
    x: TermHandle,
    y: TermHandle,
    p: TermHandle,
) -> TermHandle {
    let ty_1 = shift(arena, ty, 0, 1);
    let ty_2 = shift(arena, ty, 0, 2);
    let x_1 = shift(arena, x, 0, 1);
    let x_2 = shift(arena, x, 0, 2);
    let v0 = variable(arena, 0);
    let eq = id_term(arena, ty_1, x_1, v0);
    let y_var = variable(arena, 1);
    let p_var = variable(arena, 0);
    let sym = id_sym(arena, ty_2, x_2, y_var, p_var);
    let lhs = id_trans(arena, ty_2, y_var, x_2, y_var, sym, p_var);
    let rhs = arena.insert(Term::Refl {
        ty: ty_2,
        value: y_var,
    });
    let eq_yy = id_term(arena, ty_2, y_var, y_var);
    let goal = id_term(arena, eq_yy, lhs, rhs);
    let inner = lambda(arena, eq, goal);
    let motive = lambda(arena, ty, inner);
    let refl_x = arena.insert(Term::Refl { ty, value: x });
    let eq_xx = id_term(arena, ty, x, x);
    let base = arena.insert(Term::Refl {
        ty: eq_xx,
        value: refl_x,
    });
    arena.insert(Term::IdElim {
        motive,
        base,
        endpoint: y,
        proof: p,
    })
}

fn ap(
    arena: &mut TermArena,
    dom: TermHandle,
    cod: TermHandle,
    func: TermHandle,
    x: TermHandle,
    y: TermHandle,
    p: TermHandle,
) -> TermHandle {
    let dom_1 = shift(arena, dom, 0, 1);
    let cod_2 = shift(arena, cod, 0, 2);
    let func_2 = shift(arena, func, 0, 2);
    let x_1 = shift(arena, x, 0, 1);
    let x_2 = shift(arena, x, 0, 2);
    let v0 = variable(arena, 0);
    let v1 = variable(arena, 1);
    let eq = id_term(arena, dom_1, x_1, v0);
    let fx = apply(arena, func_2, x_2);
    let fy = apply(arena, func_2, v1);
    let goal = id_term(arena, cod_2, fx, fy);
    let inner = lambda(arena, eq, goal);
    let motive = lambda(arena, dom, inner);
    let fx0 = apply(arena, func, x);
    let base = arena.insert(Term::Refl {
        ty: cod,
        value: fx0,
    });
    arena.insert(Term::IdElim {
        motive,
        base,
        endpoint: y,
        proof: p,
    })
}

fn congr_many(
    arena: &mut TermArena,
    arg_ty: TermHandle,
    ret_ty: TermHandle,
    func: TermHandle,
    lefts: &[TermHandle],
    rights: &[TermHandle],
    proofs: &[TermHandle],
) -> TermHandle {
    let arg_tys = vec![arg_ty; lefts.len()];
    congr_many_typed(arena, &arg_tys, ret_ty, func, lefts, rights, proofs)
}

fn congr_many_typed(
    arena: &mut TermArena,
    arg_tys: &[TermHandle],
    ret_ty: TermHandle,
    func: TermHandle,
    lefts: &[TermHandle],
    rights: &[TermHandle],
    proofs: &[TermHandle],
) -> TermHandle {
    debug_assert_eq!(arg_tys.len(), lefts.len());
    debug_assert_eq!(lefts.len(), rights.len());
    debug_assert_eq!(lefts.len(), proofs.len());
    let mut acc: Option<TermHandle> = None;
    let start = arena.apply_arguments(func, lefts);
    let mut current_args = lefts.to_vec();
    for i in 0..lefts.len() {
        let mut step_args: Vec<TermHandle> = current_args
            .iter()
            .map(|&a| shift(arena, a, 0, 1))
            .collect();
        step_args[i] = variable(arena, 0);
        let func_1 = shift(arena, func, 0, 1);
        let step_body = arena.apply_arguments(func_1, &step_args);
        let step_fn = lambda(arena, arg_tys[i], step_body);
        let step = ap(
            arena, arg_tys[i], ret_ty, step_fn, lefts[i], rights[i], proofs[i],
        );
        let mid = arena.apply_arguments(func, &current_args);
        current_args[i] = rights[i];
        let next = arena.apply_arguments(func, &current_args);
        acc = Some(match acc {
            None => step,
            Some(prev) => id_trans(arena, ret_ty, start, mid, next, prev, step),
        });
    }
    acc.unwrap_or_else(|| {
        arena.insert(Term::Refl {
            ty: ret_ty,
            value: start,
        })
    })
}

/// Hedberg's theorem: from a constant endomap `(f, constant)` on `Id ty x y`,
/// derives `Π (x y: ty) (p q: Id ty x y). Id (Id ty x y) p q`.
fn hedberg(
    arena: &mut TermArena,
    ty: TermHandle,
    f: TermHandle,
    constant: TermHandle,
) -> TermHandle {
    let x = variable(arena, 3);
    let y = variable(arena, 2);
    let p = variable(arena, 1);
    let q = variable(arena, 0);
    let f_4 = shift(arena, f, 0, 4);
    let c_4 = shift(arena, constant, 0, 4);
    let eq_xy = id_term(arena, ty, x, y);

    let left_inv = |arena: &mut TermArena, path: TermHandle| {
        let x_1 = variable(arena, 4);
        let x_2 = variable(arena, 5);
        let f_6 = shift(arena, f, 0, 6);
        let y_0 = variable(arena, 0);
        let y_prime = variable(arena, 1);
        let p_prime = variable(arena, 0);
        let eq_xp = id_term(arena, ty, x_1, y_0);
        let eq_xp_2 = id_term(arena, ty, x_2, y_prime);
        let refl_x2 = arena.insert(Term::Refl { ty, value: x_2 });
        let fxx = arena.apply_arguments(f_6, &[x_2, x_2, refl_x2]);
        let sym_fxx = id_sym(arena, ty, x_2, x_2, fxx);
        let fxyp = arena.apply_arguments(f_6, &[x_2, y_prime, p_prime]);
        let lhs = id_trans(arena, ty, x_2, x_2, y_prime, sym_fxx, fxyp);
        let goal = id_term(arena, eq_xp_2, lhs, p_prime);
        let inner = lambda(arena, eq_xp, goal);
        let motive = lambda(arena, ty, inner);
        let refl_x = arena.insert(Term::Refl { ty, value: x });
        let fxx0 = arena.apply_arguments(f_4, &[x, x, refl_x]);
        let base = id_cancel_left(arena, ty, x, x, fxx0);
        arena.insert(Term::IdElim {
            motive,
            base,
            endpoint: y,
            proof: path,
        })
    };

    let refl_x = arena.insert(Term::Refl { ty, value: x });
    let fxx = arena.apply_arguments(f_4, &[x, x, refl_x]);
    let sym_fxx = id_sym(arena, ty, x, x, fxx);
    let fxyp = arena.apply_arguments(f_4, &[x, y, p]);
    let fxyq = arena.apply_arguments(f_4, &[x, y, q]);
    let lhs_p = id_trans(arena, ty, x, x, y, sym_fxx, fxyp);
    let lhs_q = id_trans(arena, ty, x, x, y, sym_fxx, fxyq);

    let inv_p = left_inv(arena, p);
    let step1 = id_sym(arena, eq_xy, lhs_p, p, inv_p);
    let const_pq = arena.apply_arguments(c_4, &[x, y, p, q]);
    let prepend = {
        let x_1 = variable(arena, 4);
        let y_1 = variable(arena, 3);
        let r_0 = variable(arena, 0);
        let sym_1 = shift(arena, sym_fxx, 0, 1);
        let body = id_trans(arena, ty, x_1, x_1, y_1, sym_1, r_0);
        lambda(arena, eq_xy, body)
    };
    let step2 = ap(arena, eq_xy, eq_xy, prepend, fxyp, fxyq, const_pq);
    let step3 = left_inv(arena, q);
    let step23 = id_trans(arena, eq_xy, lhs_p, lhs_q, q, step2, step3);
    let body = id_trans(arena, eq_xy, p, lhs_p, q, step1, step23);

    let v1 = variable(arena, 1);
    let v0 = variable(arena, 0);
    let eq_outer = id_term(arena, ty, v1, v0);
    let eq_outer_1 = shift(arena, eq_outer, 0, 1);
    let lam_q = lambda(arena, eq_outer_1, body);
    let lam_p = lambda(arena, eq_outer, lam_q);
    let lam_y = lambda(arena, ty, lam_p);
    lambda(arena, ty, lam_y)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::conversion::{Budget, convertible};
    use crate::signature::{check_signature, judgment_assumption_closure};
    use crate::typing::{Context, CoreError, check_type};

    fn constant(arena: &mut TermArena, declaration: u32) -> TermHandle {
        arena.insert(Term::Constant {
            declaration,
            levels: vec![],
        })
    }

    fn call(arena: &mut TermArena, function: u32, arguments: &[TermHandle]) -> TermHandle {
        let function = constant(arena, function);
        arena.apply_arguments(function, arguments)
    }

    const PEANO: u32 = 0;
    const ZERO: u32 = 1;
    const SUCC: u32 = 2;
    const PLUS: u32 = 4;

    /// `Peano`'s declarations and `plus`, defined by cases on its first
    /// parameter with the other parameter quantified in the motive.
    fn peano(arena: &mut TermArena) -> (PlainData, Vec<Declaration>) {
        let data = PlainData::recursive(vec![0, 1]).unwrap();
        let mut declarations = data.declarations(arena, 0);
        assert_eq!(PlainData::data_entry(0), PEANO);
        assert_eq!(data.constructor_entry(PEANO, 1), SUCC);
        assert_eq!(declarations.len() as u32, PLUS);
        let ty = constant(arena, PEANO);
        let plus_type = {
            let inner = pi(arena, ty, ty);
            pi(arena, ty, inner)
        };
        let plus = {
            let rest = pi(arena, ty, ty);
            let motive = lambda(arena, ty, rest);
            let other = variable(arena, 0);
            let zero = lambda(arena, ty, other);
            let hypothesis = data.hypothesis(arena, 1, 0, 1).unwrap();
            let other = variable(arena, 0);
            let smaller = apply(arena, hypothesis, other);
            let next = call(arena, SUCC, &[smaller]);
            let body = lambda(arena, ty, next);
            let successor = data.branch(arena, PEANO, motive, 1, body);
            let subject = variable(arena, 1);
            let recursion = data.induct(arena, PEANO, motive, &[zero, successor], subject);
            let other = variable(arena, 0);
            let body = apply(arena, recursion, other);
            let inner = lambda(arena, ty, body);
            lambda(arena, ty, inner)
        };
        declarations.push(Declaration::definition(0, plus_type, plus));
        (data, declarations)
    }

    fn checked(
        arena: &mut TermArena,
        declarations: &[Declaration],
        budget: &mut Budget,
    ) -> Context {
        let signature = check_signature(arena, declarations, budget)
            .unwrap_or_else(|error| panic!("{error:?}"));
        Context::empty().with_signature(signature)
    }

    #[test]
    fn a_function_by_cases_computes_on_a_constructor_over_any_fields() {
        let mut arena = TermArena::new();
        let mut budget = Budget::default();
        let (_, declarations) = peano(&mut arena);
        let ty = constant(&mut arena, PEANO);
        let context = checked(&mut arena, &declarations, &mut budget)
            .extend(ty)
            .extend(ty);
        let (m, b) = (variable(&mut arena, 1), variable(&mut arena, 0));
        let successor = call(&mut arena, SUCC, &[m]);
        let unfolded = call(&mut arena, PLUS, &[successor, b]);
        let smaller = call(&mut arena, PLUS, &[m, b]);
        let expected = call(&mut arena, SUCC, &[smaller]);
        assert!(convertible(&mut arena, &context, unfolded, expected, ty, &mut budget).unwrap());
        let zero = constant(&mut arena, ZERO);
        let from_zero = call(&mut arena, PLUS, &[zero, b]);
        assert!(convertible(&mut arena, &context, from_zero, b, ty, &mut budget).unwrap());
        assert!(!convertible(&mut arena, &context, from_zero, m, ty, &mut budget).unwrap());
        let stuck = call(&mut arena, PLUS, &[m, zero]);
        assert!(!convertible(&mut arena, &context, stuck, m, ty, &mut budget).unwrap());
    }

    /// `Seq<T> { Empty; Cons(head: T, tail: Seq<T>) }` at 0..4 and its
    /// application to `Two` at 4..8, then `append` over the application,
    /// defined by cases on its first formal.
    fn sequences(arena: &mut TermArena) -> (PlainData, Vec<Declaration>) {
        let generic = GenericPlainData::new(
            1,
            vec![
                vec![],
                vec![
                    GenericPlainField::Parameter(0),
                    GenericPlainField::Recursive,
                ],
            ],
        )
        .unwrap();
        let mut declarations = generic.declarations(arena, 0);
        assert_eq!(declarations.len() as u32, generic.width());
        let recursor_type = declarations[3].ty;
        let two = arena.insert(Term::Two);
        let (data, instance) = generic
            .instance(arena, 0, recursor_type, 4, &[two])
            .unwrap();
        declarations.extend(instance);
        assert_eq!(
            data.fields(1),
            [PlainField::Payload(two), PlainField::Recursive]
        );
        assert_eq!(data.hypotheses(1), 1);
        // append(s, t): Empty -> t, Cons(h, rest) -> Cons(h, append(rest, t)).
        let ty = constant(arena, 4);
        let append_type = {
            let inner = pi(arena, ty, ty);
            pi(arena, ty, inner)
        };
        let append = {
            let rest = pi(arena, ty, ty);
            let motive = lambda(arena, ty, rest);
            let other = variable(arena, 0);
            let empty = lambda(arena, ty, other);
            let head = data.field(arena, 1, 0, 1);
            let hypothesis = data.hypothesis(arena, 1, 1, 1).unwrap();
            assert!(data.hypothesis(arena, 1, 0, 1).is_none());
            let other = variable(arena, 0);
            let appended = apply(arena, hypothesis, other);
            let next = call(arena, 6, &[head, appended]);
            let body = lambda(arena, ty, next);
            let cons = data.branch(arena, 4, motive, 1, body);
            let subject = variable(arena, 1);
            let recursion = data.induct(arena, 4, motive, &[empty, cons], subject);
            let other = variable(arena, 0);
            let body = apply(arena, recursion, other);
            let inner = lambda(arena, ty, body);
            lambda(arena, ty, inner)
        };
        declarations.push(Declaration::definition(0, append_type, append));
        (data, declarations)
    }

    #[test]
    fn an_application_of_generic_data_computes_through_the_generic_block() {
        let mut arena = TermArena::new();
        let mut budget = Budget::default();
        let (_, declarations) = sequences(&mut arena);
        let ty = constant(&mut arena, 4);
        let context = checked(&mut arena, &declarations, &mut budget)
            .extend(ty)
            .extend(ty);
        let (s, t) = (variable(&mut arena, 1), variable(&mut arena, 0));
        let zero = arena.insert(Term::TwoZero);
        let one = arena.insert(Term::TwoOne);
        // append(Cons(0, s), t) = Cons(0, append(s, t)): one step of the
        // recursor at the generic block's constructor.
        let consed = call(&mut arena, 6, &[zero, s]);
        let unfolded = call(&mut arena, 8, &[consed, t]);
        let smaller = call(&mut arena, 8, &[s, t]);
        let expected = call(&mut arena, 6, &[zero, smaller]);
        assert!(convertible(&mut arena, &context, unfolded, expected, ty, &mut budget).unwrap());
        // The payload is kept: the head is not `1`.
        let wrong = call(&mut arena, 6, &[one, smaller]);
        assert!(!convertible(&mut arena, &context, unfolded, wrong, ty, &mut budget).unwrap());
        // The application's constructor is the generic one at `Two`.
        let generic_cons = {
            let head = constant(&mut arena, 2);
            let two = arena.insert(Term::Two);
            let applied = apply(&mut arena, head, two);
            arena.apply_arguments(applied, &[zero, s])
        };
        assert!(convertible(&mut arena, &context, consed, generic_cons, ty, &mut budget).unwrap());
        let empty = constant(&mut arena, 5);
        let from_empty = call(&mut arena, 8, &[empty, t]);
        assert!(convertible(&mut arena, &context, from_empty, t, ty, &mut budget).unwrap());
    }

    /// `Peano` at 0..4, then the block `Even { Zero; Next(odd: Odd) }`,
    /// `Odd { Next(even: Even) }` at 4..11 (`Even`, `Odd`, `Even::Zero`,
    /// `Even::Next`, `Odd::Next`, `Even.rec`, `Odd.rec`), then
    /// `half: Even -> Peano` at 11 by mutual recursion: `Zero` to zero,
    /// `Even::Next(o)` to the successor of `o`'s half, `Odd::Next(e)` to
    /// `e`'s half.
    fn parity(arena: &mut TermArena) -> (Vec<PlainData>, Vec<Declaration>) {
        let peano = PlainData::recursive(vec![0, 1]).unwrap();
        let mut declarations = peano.declarations(arena, 0);
        let block = PlainData::block(vec![
            vec![vec![], vec![PlainField::Member(1)]],
            vec![vec![PlainField::Member(0)]],
        ])
        .unwrap();
        let (even, odd) = (&block[0], &block[1]);
        assert!(even.is_mutual());
        assert_eq!((even.member_entry(4, 1), odd.member_entry(5, 0)), (5, 4));
        assert_eq!(
            (
                even.constructor_entry(4, 0),
                even.constructor_entry(4, 1),
                odd.constructor_entry(5, 0)
            ),
            (6, 7, 8)
        );
        assert_eq!((even.induction_entry(4), odd.induction_entry(5)), (9, 10));
        assert_eq!(even.field_member(1, 0), Some(1));
        declarations.extend(even.declarations(arena, 4));
        assert_eq!(declarations.len(), 11);
        let nat = constant(arena, 0);
        let (even_ty, odd_ty) = (constant(arena, 4), constant(arena, 5));
        let motives = [lambda(arena, even_ty, nat), lambda(arena, odd_ty, nat)];
        let zero = constant(arena, 1);
        let at_zero = even.branch_in(arena, 4, &motives, 0, zero);
        let hypothesis = even.hypothesis(arena, 1, 0, 0).unwrap();
        let successor = call(arena, 2, &[hypothesis]);
        let at_even_next = even.branch_in(arena, 4, &motives, 1, successor);
        let hypothesis = odd.hypothesis(arena, 0, 0, 0).unwrap();
        let at_odd_next = odd.branch_in(arena, 5, &motives, 0, hypothesis);
        let subject = variable(arena, 0);
        let half = even.induct_in(
            arena,
            4,
            Level::Constant(0),
            &motives,
            &[at_zero, at_even_next, at_odd_next],
            subject,
        );
        let half = lambda(arena, even_ty, half);
        let half_type = pi(arena, even_ty, nat);
        declarations.push(Declaration::definition(0, half_type, half));
        (block, declarations)
    }

    #[test]
    fn a_mutual_block_computes_through_each_members_recursor() {
        let mut arena = TermArena::new();
        let mut budget = Budget::default();
        let (_, declarations) = parity(&mut arena);
        let context = checked(&mut arena, &declarations, &mut budget);
        let nat = constant(&mut arena, 0);
        // half(Even::Next(Odd::Next(Even::Zero))) = Succ(Zero).
        let even_zero = constant(&mut arena, 6);
        let odd_next = call(&mut arena, 8, &[even_zero]);
        let two = call(&mut arena, 7, &[odd_next]);
        let halved = call(&mut arena, 11, &[two]);
        let zero = constant(&mut arena, 1);
        let one = call(&mut arena, 2, &[zero]);
        assert!(convertible(&mut arena, &context, halved, one, nat, &mut budget).unwrap());
        assert!(!convertible(&mut arena, &context, halved, zero, nat, &mut budget).unwrap());
        let at_zero = call(&mut arena, 11, &[even_zero]);
        assert!(convertible(&mut arena, &context, at_zero, zero, nat, &mut budget).unwrap());
    }

    #[test]
    fn a_mutual_block_with_a_member_field_at_its_own_position_or_out_of_range_is_refused() {
        assert!(PlainData::block(vec![vec![vec![PlainField::Member(0)]]]).is_none());
        assert!(PlainData::block(vec![vec![vec![]], vec![vec![PlainField::Member(2)]]]).is_none());
        assert!(PlainData::block(vec![vec![vec![]], vec![]]).is_none());
    }

    #[test]
    fn a_mutual_block_whose_odd_recursor_is_forged_is_refused() {
        let mut arena = TermArena::new();
        let (_, mut declarations) = parity(&mut arena);
        // `Odd.rec` stated at `Even.rec`'s type.
        declarations[10] = Declaration::recursor(1, 5, declarations[9].ty);
        let refused = check_signature(&mut arena, &declarations, &mut Budget::default());
        assert_eq!(refused.unwrap_err(), CoreError::InvalidRecursorType);
    }

    #[test]
    fn an_application_with_its_constructors_exchanged_is_refused() {
        let mut arena = TermArena::new();
        let (_, mut declarations) = sequences(&mut arena);
        // A forged application: `Empty` is said to be the generic `Cons`.
        let cons_value = declarations[6].body.unwrap();
        declarations[5] = Declaration::definition(0, declarations[5].ty, cons_value);
        assert!(check_signature(&mut arena, &declarations, &mut Budget::default()).is_err());
    }

    #[test]
    fn generic_data_with_an_unknown_parameter_or_arguments_is_refused() {
        assert!(GenericPlainData::new(1, vec![vec![GenericPlainField::Parameter(1)]]).is_none());
        assert!(GenericPlainData::new(0, vec![vec![]]).is_none());
        let mut arena = TermArena::new();
        let generic = GenericPlainData::new(1, vec![vec![]]).unwrap();
        let declarations = generic.declarations(&mut arena, 0);
        let two = arena.insert(Term::Two);
        assert!(
            generic
                .instance(&mut arena, 0, declarations[2].ty, 3, &[two, two])
                .is_none()
        );
    }

    /// `Succ(from) = Succ(to)` from a proof of `from = to`, in the proof's
    /// own context.
    fn under_successor(
        arena: &mut TermArena,
        from: TermHandle,
        to: TermHandle,
        equality: TermHandle,
    ) -> TermHandle {
        let ty = constant(arena, PEANO);
        let from_one = shift(arena, from, 0, 1);
        let from_two = shift(arena, from, 0, 2);
        let bound = variable(arena, 0);
        let domain = arena.insert(Term::Id {
            ty,
            left: from_one,
            right: bound,
        });
        let left = call(arena, SUCC, &[from_two]);
        let bound = variable(arena, 1);
        let right = call(arena, SUCC, &[bound]);
        let conclusion = arena.insert(Term::Id { ty, left, right });
        let inner = lambda(arena, domain, conclusion);
        let motive = lambda(arena, ty, inner);
        let value = call(arena, SUCC, &[from]);
        let base = arena.insert(Term::Refl { ty, value });
        arena.insert(Term::IdElim {
            motive,
            base,
            endpoint: to,
            proof: equality,
        })
    }

    #[test]
    fn a_law_that_returns_its_own_argument_is_proved_by_induction() {
        let mut arena = TermArena::new();
        let mut budget = Budget::default();
        let (data, mut declarations) = peano(&mut arena);
        let ty = constant(&mut arena, PEANO);
        let law = |arena: &mut TermArena, subject: u32| {
            let subject = variable(arena, subject);
            let zero = constant(arena, ZERO);
            let left = call(arena, PLUS, &[subject, zero]);
            arena.insert(Term::Id {
                ty,
                left,
                right: subject,
            })
        };
        let statement = {
            let body = law(&mut arena, 0);
            pi(&mut arena, ty, body)
        };
        let proof = {
            let body = law(&mut arena, 0);
            let motive = lambda(&mut arena, ty, body);
            let zero = constant(&mut arena, ZERO);
            let base = arena.insert(Term::Refl { ty, value: zero });
            let successor = {
                let smaller = data.field(&mut arena, 1, 0, 0);
                let zero = constant(&mut arena, ZERO);
                let from = call(&mut arena, PLUS, &[smaller, zero]);
                let hypothesis = data.hypothesis(&mut arena, 1, 0, 0).unwrap();
                let body = under_successor(&mut arena, from, smaller, hypothesis);
                data.branch(&mut arena, PEANO, motive, 1, body)
            };
            let subject = variable(&mut arena, 0);
            let body = data.induct(&mut arena, PEANO, motive, &[base, successor], subject);
            lambda(&mut arena, ty, body)
        };
        declarations.push(Declaration::definition(0, statement, proof));
        let before = budget.remaining();
        let signature = check_signature(&mut arena, &declarations, &mut budget)
            .unwrap_or_else(|error| panic!("{error:?}"));
        assert!(judgment_assumption_closure(&arena, &signature, &[statement, proof]).is_empty());
        eprintln!(
            "peano declarations and right identity: {} conversion steps, {} terms",
            before - budget.remaining(),
            arena.len()
        );
    }

    fn associativity(arena: &mut TermArena, tree: u32, left: u32, right: u32) -> TermHandle {
        let ty = constant(arena, PEANO);
        let (tree, left, right) = (
            variable(arena, tree),
            variable(arena, left),
            variable(arena, right),
        );
        let inner = call(arena, PLUS, &[tree, left]);
        let lhs = call(arena, PLUS, &[inner, right]);
        let inner = call(arena, PLUS, &[left, right]);
        let rhs = call(arena, PLUS, &[tree, inner]);
        arena.insert(Term::Id {
            ty,
            left: lhs,
            right: rhs,
        })
    }

    #[test]
    fn peano_addition_is_associative_by_induction_without_assumptions() {
        let mut arena = TermArena::new();
        let mut budget = Budget::default();
        let (data, mut declarations) = peano(&mut arena);
        let ty = constant(&mut arena, PEANO);
        let statement = {
            let body = associativity(&mut arena, 2, 1, 0);
            let c = pi(&mut arena, ty, body);
            let b = pi(&mut arena, ty, c);
            pi(&mut arena, ty, b)
        };
        let proof = {
            let body = associativity(&mut arena, 2, 1, 0);
            let c = pi(&mut arena, ty, body);
            let b = pi(&mut arena, ty, c);
            let motive = lambda(&mut arena, ty, b);
            let base = {
                let (b, c) = (variable(&mut arena, 1), variable(&mut arena, 0));
                let value = call(&mut arena, PLUS, &[b, c]);
                let refl = arena.insert(Term::Refl { ty, value });
                let c = lambda(&mut arena, ty, refl);
                lambda(&mut arena, ty, c)
            };
            let successor = {
                let smaller = data.field(&mut arena, 1, 0, 2);
                let (b, c) = (variable(&mut arena, 1), variable(&mut arena, 0));
                let inner = call(&mut arena, PLUS, &[smaller, b]);
                let from = call(&mut arena, PLUS, &[inner, c]);
                let inner = call(&mut arena, PLUS, &[b, c]);
                let to = call(&mut arena, PLUS, &[smaller, inner]);
                let hypothesis = data.hypothesis(&mut arena, 1, 0, 2).unwrap();
                let equality = arena.apply_arguments(hypothesis, &[b, c]);
                let step = under_successor(&mut arena, from, to, equality);
                let c = lambda(&mut arena, ty, step);
                let body = lambda(&mut arena, ty, c);
                data.branch(&mut arena, PEANO, motive, 1, body)
            };
            let tree = variable(&mut arena, 2);
            let induction = data.induct(&mut arena, PEANO, motive, &[base, successor], tree);
            let (b, c) = (variable(&mut arena, 1), variable(&mut arena, 0));
            let body = arena.apply_arguments(induction, &[b, c]);
            let c = lambda(&mut arena, ty, body);
            let b = lambda(&mut arena, ty, c);
            lambda(&mut arena, ty, b)
        };
        declarations.push(Declaration::definition(0, statement, proof));
        let before = budget.remaining();
        let signature = check_signature(&mut arena, &declarations, &mut budget)
            .unwrap_or_else(|error| panic!("{error:?}"));
        assert!(judgment_assumption_closure(&arena, &signature, &[statement, proof]).is_empty());
        eprintln!(
            "peano associativity: {} conversion steps, {} terms",
            before - budget.remaining(),
            arena.len()
        );

        let context = Context::empty().with_signature(signature);
        let zero = constant(&mut arena, ZERO);
        let one = call(&mut arena, SUCC, &[zero]);
        let two = call(&mut arena, SUCC, &[one]);
        let three = call(&mut arena, SUCC, &[two]);
        let sum = call(&mut arena, PLUS, &[two, one]);
        assert!(convertible(&mut arena, &context, sum, three, ty, &mut budget).unwrap());
        assert!(!convertible(&mut arena, &context, sum, two, ty, &mut budget).unwrap());
        let reflexivity = arena.insert(Term::Refl { ty, value: three });
        let equation = arena.insert(Term::Id {
            ty,
            left: sum,
            right: three,
        });
        check_type(&mut arena, &context, reflexivity, equation, &mut budget).unwrap();
    }

    #[test]
    fn wider_declarations_form_and_select_each_field() {
        let mut arena = TermArena::new();
        let mut budget = Budget::default();
        let data = PlainData::recursive(vec![0, 2, 3]).unwrap();
        let mut declarations = data.declarations(&mut arena, 0);
        let tree = PlainData::data_entry(0);
        let ty = constant(&mut arena, tree);
        let (leaf, two, three) = (tree + 1, tree + 2, tree + 3);
        let pick = declarations.len() as u32;
        let pick_type = pi(&mut arena, ty, ty);
        let body = {
            let motive = lambda(&mut arena, ty, ty);
            let leaf_branch = constant(&mut arena, leaf);
            let first = data.field(&mut arena, 1, 0, 0);
            let first = data.branch(&mut arena, tree, motive, 1, first);
            let last = data.hypothesis(&mut arena, 2, 2, 0).unwrap();
            let last = data.branch(&mut arena, tree, motive, 2, last);
            let subject = variable(&mut arena, 0);
            let recursion = data.induct(
                &mut arena,
                tree,
                motive,
                &[leaf_branch, first, last],
                subject,
            );
            lambda(&mut arena, ty, recursion)
        };
        declarations.push(Declaration::definition(0, pick_type, body));
        let context = checked(&mut arena, &declarations, &mut budget)
            .extend(ty)
            .extend(ty);
        let (x, y) = (variable(&mut arena, 1), variable(&mut arena, 0));
        let pair = call(&mut arena, two, &[x, y]);
        let picked = call(&mut arena, pick, &[pair]);
        assert!(convertible(&mut arena, &context, picked, x, ty, &mut budget).unwrap());
        assert!(!convertible(&mut arena, &context, picked, y, ty, &mut budget).unwrap());
        let triple = call(&mut arena, three, &[x, y, pair]);
        let picked = call(&mut arena, pick, &[triple]);
        assert!(convertible(&mut arena, &context, picked, x, ty, &mut budget).unwrap());
        let leaf_value = constant(&mut arena, leaf);
        let picked = call(&mut arena, pick, &[leaf_value]);
        assert!(convertible(&mut arena, &context, picked, leaf_value, ty, &mut budget).unwrap());
    }

    #[test]
    fn a_declaration_checks_within_one_default_budget() {
        for arities in [
            vec![0, 1],
            vec![0, 2],
            vec![0, 1, 1],
            vec![0, 2, 3],
            vec![1, 1, 1, 1],
            vec![0, 0, 0, 1, 1, 1, 2, 2, 2, 2, 3, 3],
        ] {
            let mut arena = TermArena::new();
            let data = PlainData::recursive(arities.clone()).unwrap();
            let declarations = data.declarations(&mut arena, 0);
            let mut budget = Budget::default();
            let before = budget.remaining();
            check_signature(&mut arena, &declarations, &mut budget)
                .unwrap_or_else(|error| panic!("{arities:?}: {error:?}"));
            let spent = before - budget.remaining();
            assert_eq!(
                spent, 0,
                "{arities:?}: native inductive decl should take 0 steps"
            );
        }
    }

    #[test]
    fn a_declaration_with_one_case_forms() {
        for arities in [vec![0], vec![1], vec![2]] {
            let mut arena = TermArena::new();
            let data = PlainData::recursive(arities.clone()).unwrap();
            let declarations = data.declarations(&mut arena, 0);
            check_signature(&mut arena, &declarations, &mut Budget::default())
                .unwrap_or_else(|error| panic!("{arities:?}: {error:?}"));
        }
    }

    #[test]
    fn plain_data_is_set_without_uip() {
        for arities in [vec![0], vec![0, 0], vec![0, 1], vec![0, 2]] {
            let mut arena = TermArena::new();
            let data = PlainData::recursive(arities.clone()).unwrap();
            let declarations = data.declarations(&mut arena, 0);
            let ty = constant(&mut arena, 0);
            let v1 = variable(&mut arena, 1);
            let v0 = variable(&mut arena, 0);
            let eq_xy = id_term(&mut arena, ty, v1, v0);
            let v3 = variable(&mut arena, 3);
            let v2 = variable(&mut arena, 2);
            let eq_xy_2 = id_term(&mut arena, ty, v3, v2);
            let goal = id_term(&mut arena, eq_xy_2, v1, v0);
            let eq_xy_1 = shift(&mut arena, eq_xy, 0, 1);
            let pi_q = pi(&mut arena, eq_xy_1, goal);
            let pi_p = pi(&mut arena, eq_xy, pi_q);
            let pi_y = pi(&mut arena, ty, pi_p);
            let statement = pi(&mut arena, ty, pi_y);

            let proof = data.is_set(&mut arena, 0).unwrap();
            let mut budget = Budget::new(100_000);
            let signature = check_signature(&mut arena, &declarations, &mut budget).unwrap();
            let context = Context::empty().with_signature(signature.clone());
            check_type(&mut arena, &context, proof, statement, &mut budget)
                .unwrap_or_else(|error| panic!("{arities:?}: {error:?}"));
            let closure = judgment_assumption_closure(&arena, &signature, &[proof, statement]);
            assert!(
                closure.is_empty(),
                "{arities:?}: Hedberg set proof must use zero assumptions, got {closure:?}"
            );
        }
    }
}
