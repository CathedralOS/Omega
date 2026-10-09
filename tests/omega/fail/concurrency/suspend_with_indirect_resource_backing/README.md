# Loan backing contributes its own carry demand

Run `omega --check tests/omega/fail/concurrency/suspend_with_indirect_resource_backing/main.omg`.
All four borrowed crossings must report the suspension-forbidden backing, even though
later expressions mention only a reference or a reference stored in an aggregate.
Replacing a descriptor retains only its current backing; `previous_backing`
must not contribute a crossing diagnostic.
Backing which contains another loan must retain that loan's backing too.
The direct owned-value control also checks that portable fields do not erase
the declaration's more restrictive carry floor.
Forwarding an incoming descriptor must retain its caller-supplied referent
namespace, not introduce the parameter slot as local backing. Its missing
per-value preservation join still rejects independently.

The descriptor's independent missing preservation join may also reject today.
That implementation fence is not the behavior this fixture pins: once valid
retained loans are supported, their live resources must still satisfy carry.
