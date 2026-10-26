This check-only fixture preserves an existing selected-constructor cleanup case.
The false arm moves `first`, constructs the other result field, and leaves
`second` whole for its exact owner cleanup hook. Different arms may return
different original claim identities when every returned claim is bound and the
remaining source has an eligible disposition.

The unchanged no-hook control is
`fail/ownership/selected_linear_source_missing_drop`; its retained linear source
must still reject. This fixture retains the existing nominal-cleanup compiler
profile. The replacement for its legacy hook attachment remains design-blocked
by Q2 in `OWNER_QUESTIONS.md`; this fixture does not establish that future
language authorization syntax or claim runtime coverage.
