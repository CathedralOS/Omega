# suspend_message_carrier

Suspension with a message carrier: `Message` holds a borrowed `&Cell` member read before `suspend scheduler.park()` — the record contributes until its last field read. A `boundary data Lease` (opaque, no constructor) rides as a parameter whose custody ends at the call boundary.

- `suspend` surface: `machine park() suspends` on a `pub boundary trait`; callers mark their own `suspends;` axis and `reaches Scheduler`.
- `boundary data` declares an opaque type visible across the boundary; usable in data/params without local construction.

Compile-checked only: run a check target, not native.
