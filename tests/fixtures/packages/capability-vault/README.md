# capability-vault

Capability-flow fixture. Its public machine acquires a capability-bearing lease
from a host service and returns it to the caller. Further fixture revisions can
add distinct store and derive paths.

Expected package evidence:

- capability-flow rows include the current acquire/return path;
- later matrix revisions add distinct store and derive cases;
- diff guidance points reviewers at authority retention and propagation paths.
