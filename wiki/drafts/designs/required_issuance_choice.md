# Required issuance choice

Ratified 2026-11-10. A domain's access plan started permissive, and
restricting who may create values of the domain was one optional call on it.
Leaving that call out produced a valid plan under which anyone could write
`as +D`, which is the wrong failure for a domain that represents a
capability. Every plan now states its issuance when it is built:

```omega
// before: the second line is forgettable
let mut plan: AccessPlan = AccessPlan::new(&schema);
AccessPlan::establish(&mut plan, [reflect::callable(choose_folder)]);

// after: no plan exists without the choice
let plan: AccessPlan = AccessPlan::new(
    &schema,
    Issuance::Only { routes: [reflect::callable(choose_folder)] }
);

// the common case, on the declaration line
domain Folder::Writable
    access IssuedBy<reflect::callable(choose_folder)>;
```

The normative owners are
[plans](../../spec/resources/access.md#plans-and-admitted-supply) and
[package review](../../spec/packages/review.md#complete-checked-findings).
Track implementation under `REQUIRED-ISSUANCE-CHOICE` in [TASKS.md](../../../TASKS.md).
Delete this record when the implementation and acceptance are complete.

## Decisions

1. `AccessPlan::new(&schema, issuance)` takes an `Issuance`. No valid plan
   omits it, including a plan built as a record literal.
2. `Issuance` is `Only(routes)` or `Unrestricted`. `Only` with an empty list
   admits nobody; existing qualified values still move, lend and forward.
3. `Only` is the first case, so zeroed storage and `..` admit nobody. No
   omitted field or zero default selects `Unrestricted`.
4. `AccessPlan::establish` is removed. Issuance is set once, at construction,
   and no operation widens it.
5. `Unrestricted` adds no restriction and removes none. Inherited and repeated
   restrictions still intersect.
6. Core supplies `IssuedBy<const R: reflect::CallableInfo>` and
   `IssuedByAny<const N: u64, const Routes: [reflect::CallableInfo; N]>`.
   Each forwards its descriptors into `Issuance::Only`, adds no reader or
   writer rule, invokes nothing and keeps the existing route validation.
7. The package review report lists each reviewed domain's effective issuance
   and its uses in parameter types, results, fields and predicates, with
   aliases and inherited restrictions expanded. It highlights unrestricted
   issuance at a boundary or service interface and any change from restricted
   to unrestricted. Unrestricted is a finding, not an error.
8. A domain with no policy stays unrestricted. The language does not change.

## Migration

This rides on the `domain-access-unification` migration tracked by
`DOMAIN-ACCESS-AUTHORITY`, which moves declarations from the retired clauses
to `access`. A text count of clause starts:

| Retired clause | `source` | `samples` | `tests` |
| --- | --- | --- | --- |
| `established by` | 14 | 27 | 62 |
| `written by`, `read by` | 0 | 39 | 28 |

No `.omg` file declares `access` on a domain. An `established by` clause with
no reader or writer clause beside it becomes `IssuedBy` or `IssuedByAny`; the
rest become a policy whose plan starts from `Issuance::Only`.

## Acceptance

- A plan built without an issuance argument does not compile, and a record
  literal for `AccessPlan` that omits it yields a plan that admits nobody.
- `let mut plan: AccessPlan;` followed by reader rules admits no issuer.
- `as +D` outside the listed routes rejects under `Only`, is accepted under
  `Unrestricted` when the predicates hold, and rejects everywhere under an
  empty `Only` while an existing qualified value still passes through a call.
- A child domain of a restricted parent stays restricted when its own policy
  says `Unrestricted`.
- `IssuedBy` and `IssuedByAny` accept a machine descriptor and a requirement
  descriptor, and reject a route the existing validation rejects.
- The review report shows the issuance of a domain used only in a parameter
  type of a `reaches none` machine, does not flag a predicate-proved domain
  on a service call as an error, and highlights a restricted-to-unrestricted
  change.
