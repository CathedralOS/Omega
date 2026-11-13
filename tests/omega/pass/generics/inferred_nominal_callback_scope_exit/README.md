This witness advances `STATIC-MACHINE-MIGRATION`'s inferred callable parameter
types under [generics](../../../../../wiki/spec/language/generics.md#inferred-callable-parameter-types).

The typed `origin` parameter supplies `Step<Item>::apply`. Exact application
substitutions carry that type through `relay` and the lexical child of `forward`.
The two uses in `relay` require the same type despite copied type-table handles.
The selected `identity` satisfies the nominal requirement, and the executable
oracle checks that the complete forwarding path returns 43. All four hosted
entries are authored; qualification records identify the hosts actually run.

The invalid controls are
[`inferred_callable_origin_free_cycle`](../../../fail/generics/inferred_callable_origin_free_cycle/main.omg)
and [`inferred_callable_conflicting_nominal_routes`](../../../fail/generics/inferred_callable_conflicting_nominal_routes/main.omg).
The first has forwarding and a direct callback call but no originating type.
The second is an uncalled public wrapper with independent conflicting binders;
its diagnostic pin requires both different nominal declarations and different
substituted arguments of the same declaration to be reported.

The companion acquired-claim positive and suspending twin under `concurrency`
retain the existing conditional carry and acquisition provenance oracles while
the forwarding binder uses an inferred type. Structural constraint conjunction,
compound/value substitutions, applied requirement-member origins and deferred
lifetime/domain comparisons remain compiler work. These witnesses do not close
the parent task or claim a complete fact row.
