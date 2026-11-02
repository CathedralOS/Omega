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

Fresh integrated checking and runtime observations are still pending. The
fact README records the qualification frontier; a control's presence is not
acceptance evidence. Other local folders here are retained diagnostic replay
inputs, not additional completed fact places.
