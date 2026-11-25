use std::collections::HashSet;

use arena::Arena;

#[derive(record_storage::Record)]
pub enum Level {
    Constant(u32),
    Parameter(u32),
    Successor(Box<Level>),
    Maximum(Box<Level>, Box<Level>),
}

mod levels {
    use super::Level;
    use std::collections::BTreeMap;
    use std::fmt;
    use std::hash::{Hash, Hasher};

    impl Level {
        /// Required parameter prefix; this does not validate a universe scope.
        pub fn parameter_arity(&self) -> u32 {
            let mut pending = vec![self];
            let mut arity = 0;
            while let Some(level) = pending.pop() {
                match level {
                    Level::Parameter(index) => arity = arity.max(index.saturating_add(1)),
                    Level::Successor(inner) => pending.push(inner),
                    Level::Maximum(left, right) => {
                        pending.push(left);
                        pending.push(right);
                    }
                    Level::Constant(_) => {}
                }
            }
            arity
        }

        /// Substitute exact universe actuals without resizing or cumulativity.
        pub fn instantiate(&self, arguments: &[Level]) -> Result<Level, u32> {
            instantiate_level(self, arguments)
        }

        /// Closed constants fold, with the original checked overflow boundary.
        pub fn successor(self) -> Option<Level> {
            match self {
                Level::Constant(value) => value.checked_add(1).map(Level::Constant),
                _ => Some(Level::Successor(Box::new(self))),
            }
        }

        pub fn maximum(self, other: Level) -> Level {
            match (self, other) {
                (Level::Constant(left), Level::Constant(right)) => Level::Constant(left.max(right)),
                (left, right) => Level::Maximum(Box::new(left), Box::new(right)),
            }
        }
    }

    fn map_parameters<E>(
        root: &Level,
        parameter: &mut impl FnMut(u32) -> Result<Level, E>,
    ) -> Result<Level, E> {
        match root {
            Level::Constant(value) => return Ok(Level::Constant(*value)),
            Level::Parameter(index) => return parameter(*index),
            Level::Successor(_) | Level::Maximum(_, _) => {}
        }
        enum Work<'a> {
            Visit(&'a Level),
            Successor,
            Maximum,
        }
        let mut pending = vec![Work::Visit(root)];
        let mut values = Vec::new();
        while let Some(work) = pending.pop() {
            match work {
                Work::Visit(level) => match level {
                    Level::Constant(value) => values.push(Level::Constant(*value)),
                    Level::Parameter(index) => values.push(parameter(*index)?),
                    Level::Successor(inner) => {
                        pending.push(Work::Successor);
                        pending.push(Work::Visit(inner));
                    }
                    Level::Maximum(left, right) => {
                        pending.push(Work::Maximum);
                        pending.push(Work::Visit(right));
                        pending.push(Work::Visit(left));
                    }
                },
                Work::Successor => {
                    let inner = values.pop().expect("successor has one rebuilt child");
                    values.push(Level::Successor(Box::new(inner)));
                }
                Work::Maximum => {
                    let right = values.pop().expect("maximum has a right child");
                    let left = values.pop().expect("maximum has a left child");
                    values.push(Level::Maximum(Box::new(left), Box::new(right)));
                }
            }
        }
        debug_assert_eq!(values.len(), 1);
        Ok(values.pop().expect("a level has one rebuilt root"))
    }

    impl Clone for Level {
        fn clone(&self) -> Self {
            map_parameters(self, &mut |index| {
                Ok::<_, std::convert::Infallible>(Level::Parameter(index))
            })
            .expect("copying level syntax cannot refuse")
        }
    }

    impl PartialEq for Level {
        fn eq(&self, other: &Self) -> bool {
            let mut current = (self, other);
            let mut pending: Vec<(&Level, &Level)> = Vec::new();
            loop {
                match current {
                    (Level::Constant(left), Level::Constant(right))
                    | (Level::Parameter(left), Level::Parameter(right)) => {
                        if left != right {
                            return false;
                        }
                    }
                    (Level::Successor(left), Level::Successor(right)) => {
                        current = (left, right);
                        continue;
                    }
                    (Level::Maximum(ll, lr), Level::Maximum(rl, rr)) => {
                        pending.push((lr, rr));
                        current = (ll, rl);
                        continue;
                    }
                    _ => return false,
                }
                match pending.pop() {
                    Some(next) => current = next,
                    None => return true,
                }
            }
        }
    }
    impl Eq for Level {}

    impl Hash for Level {
        fn hash<H: Hasher>(&self, state: &mut H) {
            let mut current = self;
            let mut pending: Vec<&Level> = Vec::new();
            loop {
                std::mem::discriminant(current).hash(state);
                match current {
                    Level::Constant(value) | Level::Parameter(value) => value.hash(state),
                    Level::Successor(inner) => {
                        current = inner;
                        continue;
                    }
                    Level::Maximum(left, right) => {
                        pending.push(right);
                        current = left;
                        continue;
                    }
                }
                match pending.pop() {
                    Some(next) => current = next,
                    None => return,
                }
            }
        }
    }

    impl fmt::Debug for Level {
        fn fmt(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
            match self {
                Level::Constant(value) => {
                    return output.debug_tuple("Constant").field(value).finish();
                }
                Level::Parameter(index) => {
                    return output.debug_tuple("Parameter").field(index).finish();
                }
                Level::Successor(_) | Level::Maximum(_, _) => {}
            }
            enum Work<'a> {
                Visit(&'a Level, usize),
                Text(&'static str),
                Indent(usize),
            }
            let pretty = output.alternate();
            let mut pending = vec![Work::Visit(self, 0)];
            while let Some(work) = pending.pop() {
                match work {
                    Work::Text(text) => output.write_str(text)?,
                    Work::Indent(count) => {
                        for _ in 0..count {
                            output.write_str(" ")?;
                        }
                    }
                    Work::Visit(level, indent) => {
                        let name = match level {
                            Level::Constant(_) => "Constant",
                            Level::Parameter(_) => "Parameter",
                            Level::Successor(_) => "Successor",
                            Level::Maximum(_, _) => "Maximum",
                        };
                        write!(output, "{name}(")?;
                        if pretty {
                            output.write_str("\n")?;
                        }
                        pending.push(Work::Text(")"));
                        if pretty {
                            pending.push(Work::Indent(indent));
                        }
                        match level {
                            Level::Constant(value) | Level::Parameter(value) => {
                                if pretty {
                                    for _ in 0..indent + 4 {
                                        output.write_str(" ")?;
                                    }
                                }
                                write!(output, "{value:?}")?;
                                if pretty {
                                    output.write_str(",\n")?;
                                }
                            }
                            Level::Successor(inner) => {
                                if pretty {
                                    pending.push(Work::Text(",\n"));
                                }
                                pending.push(Work::Visit(inner, indent + 4));
                                if pretty {
                                    pending.push(Work::Indent(indent + 4));
                                }
                            }
                            Level::Maximum(left, right) => {
                                if pretty {
                                    pending.push(Work::Text(",\n"));
                                }
                                pending.push(Work::Visit(right, indent + 4));
                                if pretty {
                                    pending.push(Work::Indent(indent + 4));
                                }
                                pending.push(Work::Text(if pretty { ",\n" } else { ", " }));
                                pending.push(Work::Visit(left, indent + 4));
                                if pretty {
                                    pending.push(Work::Indent(indent + 4));
                                }
                            }
                        }
                    }
                }
            }
            Ok(())
        }
    }

    impl Drop for Level {
        fn drop(&mut self) {
            fn detach(level: &mut Level, pending: &mut Vec<Level>) {
                match level {
                    Level::Successor(inner) => {
                        pending.push(*std::mem::replace(inner, Box::new(Level::Constant(0))))
                    }
                    Level::Maximum(left, right) => {
                        pending.push(*std::mem::replace(right, Box::new(Level::Constant(0))));
                        pending.push(*std::mem::replace(left, Box::new(Level::Constant(0))));
                    }
                    Level::Constant(_) | Level::Parameter(_) => {}
                }
            }
            let mut pending = Vec::new();
            detach(self, &mut pending);
            while let Some(mut level) = pending.pop() {
                detach(&mut level, &mut pending);
            }
        }
    }

    #[derive(PartialEq, Eq)]
    struct NormalLevel {
        constant: u64,
        offsets: BTreeMap<u32, u64>,
    }
    fn normal_form(root: &Level) -> NormalLevel {
        let mut form = NormalLevel {
            constant: 0,
            offsets: BTreeMap::new(),
        };
        let mut pending: Vec<(&Level, u64)> = Vec::new();
        let mut current = (root, 0u64);
        loop {
            let (level, offset) = current;
            match level {
                Level::Constant(value) => {
                    form.constant = form.constant.max(u64::from(*value) + offset)
                }
                Level::Parameter(index) => {
                    form.constant = form.constant.max(offset);
                    form.offsets
                        .entry(*index)
                        .and_modify(|old| *old = (*old).max(offset))
                        .or_insert(offset);
                }
                Level::Successor(inner) => {
                    current = (inner, offset + 1);
                    continue;
                }
                Level::Maximum(left, right) => {
                    pending.push((right, offset));
                    current = (left, offset);
                    continue;
                }
            }
            match pending.pop() {
                Some(next) => current = next,
                None => break,
            }
        }
        if form
            .offsets
            .values()
            .max()
            .is_some_and(|offset| *offset >= form.constant)
        {
            form.constant = 0;
        }
        form
    }

    pub(crate) fn levels_equal(left: &Level, right: &Level) -> bool {
        normal_form(left) == normal_form(right)
    }

    pub(crate) fn level_leq(left: &Level, right: &Level) -> bool {
        let left = normal_form(left);
        let right = normal_form(right);
        let floor = |form: &NormalLevel| {
            form.constant
                .max(form.offsets.values().copied().max().unwrap_or(0))
        };
        floor(&left) <= floor(&right)
            && left.offsets.iter().all(|(param, &left_offset)| {
                right
                    .offsets
                    .get(param)
                    .is_some_and(|&right_offset| left_offset <= right_offset)
            })
    }

    pub(crate) fn instantiate_level(level: &Level, arguments: &[Level]) -> Result<Level, u32> {
        map_parameters(level, &mut |index| {
            arguments
                .get(usize::try_from(index).unwrap_or(usize::MAX))
                .cloned()
                .ok_or(index)
        })
    }
}
pub(crate) use levels::{instantiate_level, level_leq, levels_equal};

pub fn sorts_equal(left: &Sort, right: &Sort) -> bool {
    match (left, right) {
        (Sort::Type(left), Sort::Type(right)) | (Sort::Strict(left), Sort::Strict(right)) => {
            levels_equal(left, right)
        }
        _ => false,
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, record_storage::Record)]
pub enum Sort {
    Type(Level),
    Strict(Level),
}

impl Sort {
    pub fn level(&self) -> Level {
        match self {
            Sort::Type(level) | Sort::Strict(level) => level.clone(),
        }
    }

    pub fn is_strict(&self) -> bool {
        matches!(self, Sort::Strict(_))
    }
}

pub type TermHandle = arena::Handle<Term>;

#[derive(Clone, Debug, PartialEq, Eq, Hash, Default, record_storage::Record)]
pub enum Term {
    #[default]
    Dummy,
    Variable(u32),
    Sort(Sort),
    Pi {
        domain: TermHandle,
        codomain: TermHandle,
    },
    Lambda {
        domain: TermHandle,
        body: TermHandle,
    },
    Apply {
        function: TermHandle,
        argument: TermHandle,
    },
    Sigma {
        domain: TermHandle,
        codomain: TermHandle,
    },
    Pair {
        first: TermHandle,
        second: TermHandle,
    },
    Fst {
        pair: TermHandle,
    },
    Snd {
        pair: TermHandle,
    },
    Two,
    TwoZero,
    TwoOne,
    CaseTwo {
        motive: TermHandle,
        zero_branch: TermHandle,
        one_branch: TermHandle,
        scrutinee: TermHandle,
    },
    Id {
        ty: TermHandle,
        left: TermHandle,
        right: TermHandle,
    },
    Refl {
        ty: TermHandle,
        value: TermHandle,
    },
    IdElim {
        motive: TermHandle,
        base: TermHandle,
        endpoint: TermHandle,
        proof: TermHandle,
    },
    Empty,
    EmptyElim {
        ty: TermHandle,
        scrutinee: TermHandle,
    },
    Squash {
        ty: TermHandle,
    },
    SquashIntro {
        ty: TermHandle,
        value: TermHandle,
    },
    SquashElim {
        proposition: TermHandle,
        function: TermHandle,
        scrutinee: TermHandle,
    },
    Box {
        ty: TermHandle,
    },
    BoxIntro {
        ty: TermHandle,
        value: TermHandle,
    },
    BoxElim {
        motive: TermHandle,
        body: TermHandle,
        scrutinee: TermHandle,
    },
    Constant {
        declaration: u32,
        levels: Vec<Level>,
    },
}

pub(crate) const MAX_SUBTERMS: usize = 4;

pub(crate) struct Subterms {
    slots: [Option<(TermHandle, u32)>; MAX_SUBTERMS],
    count: usize,
}

impl Subterms {
    fn new() -> Self {
        Self {
            slots: [None; MAX_SUBTERMS],
            count: 0,
        }
    }

    fn extend(&mut self, children: impl IntoIterator<Item = (TermHandle, u32)>) {
        for (handle, binders) in children {
            self.slots[self.count] = Some((handle, binders));
            self.count += 1;
        }
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.count == 0
    }

    pub(crate) fn len(&self) -> usize {
        self.count
    }

    pub(crate) fn iter(
        &self,
    ) -> impl DoubleEndedIterator<Item = (TermHandle, u32)> + ExactSizeIterator + '_ {
        self.slots[..self.count]
            .iter()
            .map(|slot| slot.expect("one initialized term slot"))
    }
}

impl Term {
    /// Visit the immediate subterms in their constructor's order.
    pub fn children(&self) -> impl Iterator<Item = TermHandle> {
        self.subterms()
            .slots
            .into_iter()
            .flatten()
            .map(|(handle, _)| handle)
    }

    /// Each immediate subterm with the number of binders this term puts
    /// around it, in the constructor's order.
    pub fn bound_children(&self) -> impl Iterator<Item = (TermHandle, u32)> {
        self.subterms().slots.into_iter().flatten()
    }

    /// The same term former around other subterms, one per slot in order.
    pub fn rebuilt(&self, children: &[TermHandle]) -> Term {
        self.with_children(children)
    }
}

macro_rules! term_layout {
($($variant:ident { $($field:ident : $binders:expr),* $(,)? })+) => {
    impl Term {
        pub(crate) fn subterms(&self) -> Subterms {
            let mut slots = Subterms::new();
            match self {
                $(Term::$variant { $($field),* } => slots.extend([$((*$field, $binders)),*]),)+
                Term::Dummy
                | Term::Variable(_)
                | Term::Sort(_)
                | Term::Two
                | Term::TwoZero
                | Term::TwoOne
                | Term::Empty
                | Term::Constant { .. } => {}
            }
            slots
        }

        fn payloads_equal(&self, other: &Term) -> bool {
            match (self, other) {
                $((Term::$variant { .. }, Term::$variant { .. }) => true,)+
                (Term::Variable(left), Term::Variable(right)) => left == right,
                (Term::Sort(left), Term::Sort(right)) => left == right,
                (Term::Constant { declaration: left, levels: left_levels },
                 Term::Constant { declaration: right, levels: right_levels }) => {
                    left == right && left_levels == right_levels
                }
                (Term::Two, Term::Two)
                | (Term::TwoZero, Term::TwoZero)
                | (Term::TwoOne, Term::TwoOne)
                | (Term::Empty, Term::Empty) => true,
                _ => false,
            }
        }

        pub(crate) fn with_children(&self, children: &[TermHandle]) -> Term {
            debug_assert_eq!(
                children.len(),
                self.subterms().len(),
                "every slot of a term takes exactly one child"
            );
            let mut next = children.iter().copied();
            match self {
                $(Term::$variant { .. } => Term::$variant {
                    $($field: next.next().expect("one child per term slot")),*
                },)+
                Term::Dummy
                | Term::Variable(_)
                | Term::Sort(_)
                | Term::Two
                | Term::TwoZero
                | Term::TwoOne
                | Term::Empty
                | Term::Constant { .. } => self.clone(),
            }
        }
    }
};
}

term_layout! {
    Pi { domain: 0, codomain: 1 }
    Lambda { domain: 0, body: 1 }
    Apply { function: 0, argument: 0 }
    Sigma { domain: 0, codomain: 1 }
    Pair { first: 0, second: 0 }
    Fst { pair: 0 }
    Snd { pair: 0 }
    CaseTwo { motive: 0, zero_branch: 0, one_branch: 0, scrutinee: 0 }
    Id { ty: 0, left: 0, right: 0 }
    Refl { ty: 0, value: 0 }
    IdElim { motive: 0, base: 0, endpoint: 0, proof: 0 }
    EmptyElim { ty: 0, scrutinee: 0 }
    Squash { ty: 0 }
    SquashIntro { ty: 0, value: 0 }
    SquashElim { proposition: 0, function: 0, scrutinee: 0 }
    Box { ty: 0 }
    BoxIntro { ty: 0, value: 0 }
    BoxElim { motive: 0, body: 0, scrutinee: 0 }
}
#[derive(Clone, record_storage::Record)]
pub struct TermArena {
    terms: Arena<Term>,
    /// Constructor-derived syntax only: `(generation, required_bindings, has_level_params)`.
    /// Missing summaries after decoding, or from unresolved children, retain ordinary remapping traversal.
    #[record(skip)]
    binding_requirements: Vec<Option<(u32, u32, bool)>>,
}

impl std::fmt::Debug for TermArena {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("TermArena")
            .field("terms", &self.terms)
            .finish()
    }
}

impl PartialEq for TermArena {
    fn eq(&self, other: &Self) -> bool {
        self.terms == other.terms
    }
}

impl Eq for TermArena {}

impl TermArena {
    pub fn new() -> Self {
        Self {
            terms: Arena::new(),
            binding_requirements: Vec::new(),
        }
    }

    pub fn insert(&mut self, term: Term) -> TermHandle {
        let required = match &term {
            Term::Variable(index) => index.checked_add(1),
            _ => term
                .subterms()
                .iter()
                .try_fold(0u32, |required, (child, binders)| {
                    Some(required.max(self.required_bindings(child)?.saturating_sub(binders)))
                }),
        };
        let has_level_params = match &term {
            Term::Sort(sort) => Some(sort.level().parameter_arity() > 0),
            Term::Constant { levels, .. } => {
                Some(levels.iter().any(|level| level.parameter_arity() > 0))
            }
            _ => term.subterms().iter().try_fold(false, |has, (child, _)| {
                Some(has || self.has_level_params(child)?)
            }),
        };
        let handle = self.terms.insert(term);
        let slot = handle.arena_index() as usize;
        if slot >= self.binding_requirements.len() {
            self.binding_requirements.resize(slot + 1, None);
        }
        self.binding_requirements[slot] = match (required, has_level_params) {
            (Some(required), Some(has_level_params)) => {
                Some((handle.generation(), required, has_level_params))
            }
            _ => None,
        };
        handle
    }

    pub(super) fn required_bindings(&self, term: TermHandle) -> Option<u32> {
        // Authenticate the complete handle before consulting its arena slot.
        self.terms.try_get(term)?;
        let (generation, required, _) = self
            .binding_requirements
            .get(term.arena_index() as usize)
            .copied()
            .flatten()?;
        (generation == term.generation()).then_some(required)
    }

    pub(super) fn has_level_params(&self, term: TermHandle) -> Option<bool> {
        self.terms.try_get(term)?;
        let (generation, _, has_level_params) = self
            .binding_requirements
            .get(term.arena_index() as usize)
            .copied()
            .flatten()?;
        (generation == term.generation()).then_some(has_level_params)
    }

    /// Skip the tree walk when the constructor's recorded bound rules out this
    /// variable. Terms without a usable bound need the full scan.
    pub fn contains_free_variable(&self, term: TermHandle, index: u32) -> bool {
        if self
            .required_bindings(term)
            .is_some_and(|required| required <= index)
        {
            return false;
        }
        let mut pending = vec![(term, index)];
        while let Some((term, index)) = pending.pop() {
            let node = self.node(term);
            if let Term::Variable(position) = node {
                if *position == index {
                    return true;
                }
                continue;
            }
            for (child, binders) in node.subterms().iter().rev() {
                if let Some(nested) = index.checked_add(binders) {
                    pending.push((child, nested));
                }
            }
        }
        false
    }

    pub fn apply_arguments(
        &mut self,
        function: TermHandle,
        arguments: &[TermHandle],
    ) -> TermHandle {
        let mut spine = function;
        for &argument in arguments {
            spine = self.insert(Term::Apply {
                function: spine,
                argument,
            });
        }
        spine
    }

    /// The type of a tuple of `components`, nested to the right:
    /// `Σ c0 (Σ c1 c2)`. No component depends on another, so each is shifted
    /// past the binders of the components before it. One component is its
    /// own type. None when empty.
    pub fn product_type(&mut self, components: &[TermHandle]) -> Option<TermHandle> {
        let (&last, before) = components.split_last()?;
        let mut ty = last;
        for &domain in before.iter().rev() {
            let codomain = crate::substitution::shift(self, ty, 0, 1);
            ty = self.insert(Term::Sigma { domain, codomain });
        }
        Some(ty)
    }

    /// A tuple of `components` in the shape [`Self::product_type`] gives
    /// its type. None when empty.
    pub fn tuple(&mut self, components: &[TermHandle]) -> Option<TermHandle> {
        let (&last, before) = components.split_last()?;
        let mut tuple = last;
        for &first in before.iter().rev() {
            tuple = self.insert(Term::Pair {
                first,
                second: tuple,
            });
        }
        Some(tuple)
    }

    /// Component `position` of a tuple of `count` components in the shape
    /// [`Self::product_type`] gives: `position` second projections, then a
    /// first projection unless the component is the last.
    pub fn tuple_component(
        &mut self,
        tuple: TermHandle,
        position: usize,
        count: usize,
    ) -> TermHandle {
        let mut term = tuple;
        for _ in 0..position {
            term = self.insert(Term::Snd { pair: term });
        }
        if position + 1 < count {
            term = self.insert(Term::Fst { pair: term });
        }
        term
    }

    pub fn apply_binary(
        &mut self,
        relation: TermHandle,
        left: TermHandle,
        right: TermHandle,
    ) -> TermHandle {
        self.apply_arguments(relation, &[left, right])
    }

    pub fn get(&self, handle: TermHandle) -> Term {
        self.node(handle).clone()
    }

    pub(crate) fn node(&self, handle: TermHandle) -> &Term {
        self.terms.get(handle)
    }

    pub(crate) fn try_node(&self, handle: TermHandle) -> Option<&Term> {
        self.terms.try_get(handle)
    }

    pub fn len(&self) -> usize {
        self.terms.len()
    }

    pub fn is_empty(&self) -> bool {
        self.terms.is_empty()
    }

    pub fn structurally_equal(&self, left: TermHandle, right: TermHandle) -> bool {
        self.structurally_equal_shifted(left, right, 0)
    }

    /// Structural equality for well-formed acyclic DAG terms (checked by the kernel),
    /// short-circuiting identical handles in O(1) to avoid re-traversing shared subtrees.
    pub(crate) fn structurally_equal_dag(&self, left: TermHandle, right: TermHandle) -> bool {
        if left == right {
            return true;
        }
        let mut pending = vec![(left, right)];
        let mut complete = HashSet::new();
        while let Some((l, r)) = pending.pop() {
            if l == r || !complete.insert((l, r)) {
                continue;
            }
            let left_node = self.node(l);
            let right_node = self.node(r);
            if !left_node.payloads_equal(right_node) {
                return false;
            }
            for ((lc, _), (rc, _)) in left_node
                .subterms()
                .iter()
                .zip(right_node.subterms().iter())
                .rev()
            {
                if lc != rc {
                    pending.push((lc, rc));
                }
            }
        }
        true
    }

    /// Compare a term with the same value carried under additional binders.
    /// Locally bound variables keep their indices; only free variables cross
    /// the inserted telescope. This is structural correspondence, not a new
    /// conversion or equality rule, and allocates no shifted term tree.
    pub fn structurally_equal_shifted(
        &self,
        source: TermHandle,
        shifted: TermHandle,
        inserted_binders: u32,
    ) -> bool {
        let mut pending = vec![(false, (source, shifted, 0u32))];
        let mut active = HashSet::new();
        let mut complete = HashSet::new();
        while let Some((finish, pair)) = pending.pop() {
            if finish {
                active.remove(&(pair.0, pair.1));
                complete.insert(pair);
                continue;
            }
            if complete.contains(&pair) {
                continue;
            }
            // The local depth belongs to the key: a shared node may occur
            // both under its own binder and in the surrounding telescope.
            if !active.insert((pair.0, pair.1)) {
                return false;
            }
            let left = self.node(pair.0);
            let right = self.node(pair.1);
            let payloads_equal = match (left, right) {
                (Term::Variable(left), Term::Variable(right)) if *left >= pair.2 => {
                    left.checked_add(inserted_binders) == Some(*right)
                }
                _ => left.payloads_equal(right),
            };
            if !payloads_equal {
                return false;
            }
            pending.push((true, pair));
            for ((left, binders), (right, _)) in
                left.subterms().iter().zip(right.subterms().iter()).rev()
            {
                let depth = if inserted_binders == 0 {
                    0
                } else {
                    let Some(depth) = pair.2.checked_add(binders) else {
                        return false;
                    };
                    depth
                };
                pending.push((false, (left, right, depth)));
            }
        }
        true
    }
}

impl Default for TermArena {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod shifted_correspondence_tests {
    use super::{Term, TermArena};

    #[test]
    fn shifted_comparison_matches_capture_avoiding_shift_on_shared_scopes() {
        let mut arena = TermArena::new();
        let two = arena.insert(Term::Two);
        let zero = arena.insert(Term::Variable(0));
        let one = arena.insert(Term::Variable(1));
        let relation = arena.insert(Term::Id {
            ty: two,
            left: zero,
            right: one,
        });
        let lambda = arena.insert(Term::Lambda {
            domain: two,
            body: relation,
        });
        let source = arena.insert(Term::Sigma {
            domain: relation,
            codomain: lambda,
        });
        for amount in [0, 1, 3] {
            let shifted = crate::shift(&mut arena, source, 0, amount);
            assert!(arena.structurally_equal_shifted(source, shifted, amount));
            assert!(!arena.structurally_equal_shifted(source, shifted, amount + 1));
        }
    }

    #[test]
    fn a_binder_cycle_is_not_correspondence_at_any_offset() {
        let mut arena = TermArena::new();
        let two = arena.insert(Term::Two);
        let cyclic = arena.insert(Term::Dummy);
        *arena.terms.get_mut(cyclic) = Term::Lambda {
            domain: two,
            body: cyclic,
        };
        for amount in [0, 1] {
            assert!(!arena.structurally_equal_shifted(cyclic, cyclic, amount));
        }
    }
}
