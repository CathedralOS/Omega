# Translation coordinate spaces

Ratified 2026-11-10. The spec refused to let an installed virtual-to-physical
mapping be split into independently owned pieces until a new compiler-owned
"joint correspondence" algebra existed, because conserving the virtual and
physical ranges separately lets two children swap their backing. No new
algebra is needed for the common case. A contiguous mapping from `v` to
`v + Delta` is an interval of virtual addresses in a coordinate space indexed
by `Delta`, and the existing interval algebra already refuses to combine
intervals from different spaces:

```omega
data Translated<Delta: i64> { }

data Mapping<Delta: i64> [linear] { virtual_base: u64; length: u64; }

machine Installed::content<Delta: i64>(mapping: &Mapping<Delta>)
    -> IntervalSet<Translated<Delta>>
satisfies Content<Mapping<Delta>, IntervalSet<Translated<Delta>>>::project
{
    IntervalSet {
        start: embed(mapping.virtual_base) as Nat,
        end: (embed(mapping.virtual_base) + embed(mapping.length)) as Nat
    }
}
```

The normative owner is [related content](../../spec/resources/content_custody.md#independent-and-related-content).
Implementation belongs to the address-translation canary under
`EXTENT-LIBRARY-TRANSFORMS` in [TASKS.md](../../../TASKS.md). Delete this
record when the acceptance below passes.

## Decisions

1. The association between related resources must ride on conserved content.
   A relation stated only in a machine's `ensures` is not enough: a split that
   swaps its children and says so accurately would be accepted, because a
   guarantee is checked against its own body and not against the parent it
   consumed. Mappings meet the same standard extents already meet.
2. A coordinate space is an ordinary type argument and may be indexed by a
   value, including a runtime value. Two applications are one space only when
   their indices are the same canonical value or are proved equal.
3. A translation is carried by an offset-indexed space, as above. Swapped
   children would be translations by `Delta + n` and `Delta - n`, which are
   different spaces, so `separate` rejects them whatever their contract says.
4. The virtual range and physical backing remain their own claims. A predicate
   ties each physical base to its virtual base plus the offset. A mapping of
   several contiguous runs holds one claim per run.
5. Rights, installation evidence and release obligations stay ordinary linear
   data and domains.
6. A correspondence that is not a union of translations still needs a joint
   algebra and still rejects until one is specified.

No library-defined algebra, new algebra kind or new trust in the mapping
package is introduced.

## Observed implementation state

The construction was tried in a std-free scratch package against a build of
`eb37f41` made with the stable toolchain, not the pinned reference toolchain.
A baseline interval split copied from
`tests/omega/pass/facts/content_reunion_through_call_result_exit` checked
clean. Value-indexed data types and a carrier-generic domain parsed and
resolved. Each variant of the offset-indexed projection then stopped at a
stated implementation limit, none at a rule of the language:

| Variant | Diagnostic |
| --- | --- |
| `const Delta` index on the space | `source W descriptions currently require a Type/Value telescope; const/machine telescope denotation remains unsupported` |
| Runtime-capable `Delta` as a machine generic | `` `Delta` is not a mathematical term: its exact binding is unavailable in split `` |
| `delta` as an ordinary parameter indexing the types | `applied callable has an unrepresented static type operand` |
| Children built inside a generic result record | `constructor occurrence has conflicting receiving applications` |

Declaration syntax also constrained the attempt: a domain binder accepted type
and `const` parameters, and an indexed domain had to use each index in its
family header. Nothing in the tree uses an indexed coordinate space; the
existing ones are `Nat` and `ByteUnit`.

## Acceptance

| Must pass | Must reject |
| --- | --- |
| Split and recombination of one run for a symbolic length | A swapped split whose contract describes the swap accurately |
| Partial unmap that retains the untouched run | Recombining runs with different offsets |
| The offset surviving a helper call | Substituting same-sized unrelated backing |
| A runtime-chosen offset | Treating equal geometry as the same space |
| Failure and pending outcomes carrying every surviving obligation | Reuse before translation release completes |

Compilation and proof cost must depend on the number of runs, not pages.

## Left open

- Holding runs with different runtime offsets in one collection needs their
  index packaged per element. Whether an owning `some` package is the right
  carrier is untested.
- Which integer type indexes the offset when physical addresses lie below
  virtual ones.
- Joint algebras for correspondences that are not unions of translations.
