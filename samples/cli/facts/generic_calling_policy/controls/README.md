# Selected calling-policy helpers

These controls retain the canonical fact's Width-8 policy and boundary result
42. They vary provider admission independently of byte-placement validation.

- `selected_rejection` selects a helper returning an explicit rejection. Its
  diagnostic must retain the selected helper's reason.
- `missing_policy_provider` omits the policy helper's Build selection. Visible
  implementations do not supply that missing choice.
- `helper_uses_own_policy` lets the helper call the checked `LocalEntry` body.
  The requirement's Calling metadata does not create an executable recursion
  edge; the selected body returns 42 without calling the policy.
- `missing_nested_provider` keeps that executable call but removes its exact
  Build selection. A unique visible implementation cannot replace it.
- `unselected_stronger_contract` adds an implementation whose Width-4
  precondition violates the requirement's public contract. Source must reject
  it even when Build selects another implementation.
- `inherited_helper` calls the parent requirement through an explicitly selected
  child implementation. Its Width-4 twin still fails byte-placement validation.
- `unused_descendant` adds an unselected child implementation whose plan would
  reject if executed. Selecting its parent must not demand or execute the child;
  the Width-4 twin still fails for incomplete byte coverage.

Producer `002791b61f14404bcf6ce65e5376131ca16e1efe` runs the inherited-helper
and unused-descendant public samples through `omega run --both` on macOS arm64;
both Width-4 twins reject for their intended byte-coverage obligation. The
focused corpus also runs `helper_uses_own_policy` at native/Terminal exit 0 and
retains the selected rejection and missing-selection controls. The fact README
records the producer identities and remaining migration boundary. Other local
folders here are retained diagnostic replay inputs, not additional completed
fact places.
