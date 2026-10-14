# Freestanding review does not select hosted completion

The public package-review reproduction is:

```text
omega update --project tests/omega/pass/build/freestanding_unselected_completion --target uefi_x86_64
```

It must check both packages and reach an ordinary pending dependency review
(exit 3), not reject std's source-nominated Console completion because UEFI
selects no hosted Console provider. No review decisions need accepting for
this control. Discard a pending review before changing target or rerunning
with a changed producer.

The source library remains target-neutral. Unsupported demanded operations
still require realizations; this fixture deliberately has no executable entry
and does not claim a firmware boot or permission to call hosted services.
