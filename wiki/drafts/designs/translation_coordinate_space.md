# Translation coordinate spaces

Ratified 2026-11-10. The spec refused to let an installed virtual-to-physical
mapping be split into independently owned pieces until a new compiler-owned
"joint correspondence" algebra existed, because conserving the virtual and
physical ranges separately lets two children swap their backing. No new
algebra is needed for translations. Within one virtual and one physical address
space, a contiguous mapping from `v` to `v + Delta` is an interval of virtual
addresses in a coordinate space indexed by those spaces and by `Delta`, and
the existing interval algebra already refuses to combine intervals from
different spaces:

```omega
data Translated<Virtual, Physical, Delta: Int> { }

data Mapping<Virtual, Physical, Delta: Int> [linear] {
    virtual_base: u64;
    length: u64;
}
```

The normative owner is [related content](../../spec/resources/content_custody.md#independent-and-related-content).
Implementation belongs to the address-translation canary under
`EXTENT-LIBRARY-TRANSFORMS` in [TASKS.md](../../../TASKS.md). Delete this
record when the acceptance below passes.

## Why it works

An interval `I` in `Translated<V, P, Delta>` denotes the relation of every `v`
in `I` to `v + Delta`. If interval conservation establishes that `I` is the
disjoint union of a left and a right interval, and both children keep the same
`Delta`, the relation they denote together is the parent's. A child given the
other child's backing either keeps the parent's `Delta`, and fails the
predicate tying its physical base to its virtual base, or takes the `Delta`
that describes its new backing, and is in a different space. Both parts are
needed: an offset label with no tie to the actual backing proves nothing.

## Decisions

1. The association between related resources must ride on conserved content.
   A relation stated only in a machine's `ensures` is not enough: a split that
   swaps its children and says so accurately would be accepted, because a
   guarantee is checked against its own body and not against the parent it
   consumed. Mappings meet the same standard extents already meet.
2. A coordinate space is an ordinary type argument and may be indexed by a
   value, including a runtime value. Two applications are one space only when
   their indices are the same canonical value or are proved equal.
3. A translation's space is indexed by its virtual address space, its physical
   address space and its offset. Equal intervals and offsets in different
   address-space instances are different content.
4. The offset is the exact mathematical integer `embed(physical) -
   embed(virtual)`. For 64-bit addresses it spans `-(2^64 - 1)` to `2^64 - 1`,
   which no 64-bit signed carrier holds, so it is proof data. Executable code
   that needs it stores a representation, such as a sign and a `u64` magnitude
   with one form of zero, with the equation relating it to the index.
5. The virtual range and physical backing remain their own claims. A predicate
   ties each physical base to its virtual base plus the offset.
6. A mapping of several contiguous runs holds one claim per run, each in its
   own space. A collection may own runs with different offsets. Combining two
   into one translation claim requires an established common offset.
7. Rights, installation evidence and release obligations stay ordinary linear
   data and domains. A split preserves the installed association; a remap
   changes it through the existing mapping lifecycle.
8. This decision covers translations only. It supplies no construction for
   other correspondences and does not rule out a similar one for them.

No library-defined algebra, new algebra kind or new trust in the mapping
package is introduced.

## Observed implementation state

A simplified form of the construction, with one `u64` offset and no address
space parameters, was tried in a std-free scratch package against a build of
`eb37f41` made with the stable toolchain, not the pinned reference toolchain.
A baseline interval split copied from
`tests/omega/pass/facts/content_reunion_through_call_result_exit` checked
clean. No indexed variant checked. The result is therefore limited: neither
the positive translation split nor the rejection of the swapped split has been
demonstrated, and both remain acceptance work.

Variant B below is the program as tried. The others differ from it as noted.

```omega
use omega::language::core::content;
use omega::language::core::nat;

data Shift<Delta: u64> { }

data Mapping<Delta: u64> [linear] dropped by drop_mapping<Delta> { virt: u64; length: u64; }
domain<T> T::Installed established by install;

machine Installed::content<Delta: u64>(mapping: &Mapping<Delta>) -> IntervalSet<Shift<Delta>>
satisfies Content<Mapping<Delta>, IntervalSet<Shift<Delta>>>::project {
    IntervalSet { start: embed(mapping.virt) as Nat,
        end: (embed(mapping.virt) + embed(mapping.length)) as Nat }
}

machine install<Delta: u64>() -> Mapping<Delta> in Installed
ensures result.virt == 0, result.length == 32
{ Mapping { virt: 0, length: 32 } }

machine drop_mapping<Delta: u64>(mapping: &mut Mapping<Delta>) {}

data Parts<Delta: u64> { left: Mapping<Delta> in Installed; right: Mapping<Delta> in Installed; }

machine split<Delta: u64>(whole: Mapping<Delta> in Installed, at: u64) -> Parts<Delta>
requires whole.virt <= 64 && whole.length <= 64 - whole.virt && at <= whole.length
ensures
    result.left.virt == whole.virt,
    result.left.length == at,
    result.right.virt == whole.virt + at,
    result.right.length == whole.length - at,
    core::Identity<IntervalSet<Shift<Delta>>>(
        Installed::content(old(&whole)),
        separate(Installed::content(&result.left), Installed::content(&result.right))
    )
{
    let { virt, length } = whole;
    let left: Mapping<Delta> in Installed = Mapping { virt: virt, length: at };
    let right: Mapping<Delta> in Installed = Mapping { virt: virt + at, length: length - at };
    Parts { left: left, right: right }
}

data Main { marker: u64; }
machine Main::main(main: &mut Main) {
    main.marker = 0;
}
```

| Variant | Difference from B | Diagnostic |
| --- | --- | --- |
| A | Every `Delta: u64` is `const Delta: u64`; the domain is `domain<const Delta: u64> Mapping<Delta>::Installed<Delta>` and is written `in Installed<Delta>` | `source W descriptions currently require a Type/Value telescope; const/machine telescope denotation remains unsupported` |
| B | As listed | `` `Delta` is not a mathematical term: its exact binding is unavailable in split `` |
| C | `split` returns `Parts { left: Mapping { ... }, right: Mapping { ... } }` directly, without the two typed locals | `constructor occurrence has conflicting receiving applications; preserve separate evaluation contexts` |
| D | `split(delta: u64, whole: Mapping<delta> in Installed, at: u64) -> Parts<delta>`, with `delta` in place of `Delta` in its body and contract | `applied callable has an unrepresented static type operand` |

A and D name an unsupported construction outright. B and C may be compiler
limits or faults in these particular programs; the listing is kept so that
whoever takes the canary can tell which. Declaration syntax also constrained
the attempt: a domain binder accepted type and `const` parameters only, and an
indexed domain had to use each index in its family header. Nothing in the tree
uses an indexed coordinate space; the existing ones are `Nat` and `ByteUnit`.

## Acceptance

| Must pass | Must reject |
| --- | --- |
| Split and recombination of one run for a symbolic length | A swapped split whose contract describes the swap accurately |
| Partial unmap that retains the untouched run | Coalescing two runs with different offsets into one translation claim |
| The offset surviving a helper call | Substituting same-sized unrelated backing |
| A runtime-chosen offset, including one outside the `i64` range | Treating equal geometry as the same space |
| Two runs with different runtime offsets entering one collection, surviving a helper call, being extracted and independently consumed | Substituting an equal interval and offset from a different address-space instance |
| Failure and pending outcomes carrying every surviving obligation | Reuse before translation release completes |

Compilation and proof cost must depend on the number of runs, not pages.

## Left open

- The worked collection example. Each entry must carry its offset
  representation, the owned mapping, and the checked relation between the
  mapping's index and that representation. An owning `some` package that
  publishes that relation is a plausible carrier; a bare hidden type is not
  enough to later coalesce two entries on equal offsets.
- Whether a value binder may range over proof integers directly, or the index
  is spelled through a representation type with a proved denotation.
- Compact representations for correspondences that are not translations.
