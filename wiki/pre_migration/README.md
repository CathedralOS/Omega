# Documents needing porting

Everything in this directory still needs review and consolidation.
These documents mix intended contracts, implementation details, proposals, and
old status reports. Their placement here does not mean every statement is wrong;
it means the document has not been vetted for its destination.

| Existing group | What needs separating |
| --- | --- |
| [architecture/](architecture/architecture.md) | Public contracts, implementation ownership, and old implementation reports. |
| design_briefs/ | Current rules, genuine proposals, and superseded designs. |
| releases/ | Completion plans, tooling instructions, and any evidence with a live consumer. |
| Top-level notes | Testing instructions and measurements, proof support, and customer ownership. |

For each document, port current rules to the specification, explanations to the
guide, proposed changes to proposals, implementation details beside code, and
temporary work to drafts or the existing task board. Delete obsolete material.
Remove the source document when its useful content has been handled; do not keep
a compatibility copy.

The [documentation index](../README.md) names the subjects already migrated.
Those specification sections own their subjects. Where no replacement exists,
use this material as input to review, not as automatically approved specification.
Preserve intended contracts; raise genuine unsettled conflicts in
[owner questions](../../OWNER_QUESTIONS.md).

## Protected-location exception

The [bootstrap decision record](../architecture/bootstrap_chain/decisions.md)
cannot be moved or amended without explicit owner authorization.
[Bootstrap chain alternatives](../design_briefs/bootstrap_chain_alternatives.md)
also remains at its old path because that record links to it. Neither location
means the documentation cleanup is complete. Their authority transition remains
an explicit exception in the [cleanup plan](../drafts/documentation_cleanup.md).

This directory is a migration queue, not a permanent archive. Delete it when empty.
