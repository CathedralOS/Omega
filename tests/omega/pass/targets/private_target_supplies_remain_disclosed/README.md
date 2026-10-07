# Private target supplies remain disclosed

The dependency contains unused private Linux and macOS external leaves and a
pure body with an uncatalogued deployment tag. Package review retains every
leaf's authored target even when one member is promoted for this activation.
Neither unused leaf becomes a selected provider. The application returns 0.

From the repository root, use the public command chain:

```sh
omega update --project tests/omega/pass/targets/private_target_supplies_remain_disclosed --target macos_arm64
# Accept the displayed external claims, then resume the review.
omega update --resume --project tests/omega/pass/targets/private_target_supplies_remain_disclosed
omega run tests/omega/pass/targets/private_target_supplies_remain_disclosed/main.omg --target macos_arm64 --both
omega audit packages --project tests/omega/pass/targets/private_target_supplies_remain_disclosed --target macos_arm64 --details
```

The readable findings retain both `linux_x86_64` and `macos_arm64` target-member
contexts. Changing only the inactive Linux syscall must change its risk row,
not disappear from review. Selecting Linux likewise retains both disclosures;
the unknown pure tag must not demand native realization.
