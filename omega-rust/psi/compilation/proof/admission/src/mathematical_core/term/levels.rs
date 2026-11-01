//! Universe syntax is immutable data. Every traversal, including retirement,
//! uses invocation-owned work so a deep level never consumes one call frame
//! per constructor. Syntactic identity and semantic max-normalization remain
//! distinct operations; neither supplies a universe scope judgment.

use super::Level;
use std::collections::BTreeMap;
use std::fmt;
use std::hash::{Hash, Hasher};

impl Level {
    /// Closed constants fold, with the original checked overflow boundary.
    pub fn successor(self) -> Option<Level> {
        match self {
            Level::Constant(value) => value.checked_add(1).map(Level::Constant),
            _ => Some(Level::Successor(Box::new(self))),
        }
    }

    /// Syntax involving parameters stays explicit until semantic comparison.
    pub fn maximum(self, other: Level) -> Level {
        match (self, other) {
            (Level::Constant(left), Level::Constant(right)) => Level::Constant(left.max(right)),
            (left, right) => Level::Maximum(Box::new(left), Box::new(right)),
        }
    }
}

/// One postorder rebuilding owner serves cloning and parameter instantiation.
/// It keeps the exact original syntax instead of folding a newly closed level.
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
            Level::Constant(value) => return output.debug_tuple("Constant").field(value).finish(),
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
            // Its remaining children are constant placeholders. Automatic
            // field drop therefore has bounded depth, including on refusal.
        }
    }
}

/// Canonical max(k, p0+o0, ...). Successor offsets are accumulated on the
/// worklist instead of recursively rebuilding and merging maps per node.
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
            Level::Constant(value) => form.constant = form.constant.max(u64::from(*value) + offset),
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

/// Semantic comparison keeps the original free max/successor algebra.
pub(crate) fn levels_equal(left: &Level, right: &Level) -> bool {
    normal_form(left) == normal_form(right)
}

/// Parameters are judgment scope, never term binders. Missing arguments keep
/// the first left-to-right offending index and supply no well-formedness claim.
pub(crate) fn instantiate_level(level: &Level, arguments: &[Level]) -> Result<Level, u32> {
    map_parameters(level, &mut |index| {
        arguments
            .get(usize::try_from(index).unwrap_or(usize::MAX))
            .cloned()
            .ok_or(index)
    })
}
