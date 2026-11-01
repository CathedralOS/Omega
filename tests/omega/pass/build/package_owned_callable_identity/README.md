Two managed packages declare the same `clock::Clock::sleep(u32)` signature. Both provider selections remain in the portable catalog. The app invokes its own provider, which returns 9; binding the dependency provider returning 17 would trap. The reject twin selects the dependency provider for the app requirement and must refuse the owner mismatch.

The original compiler checked both packages, then refused Terminal lowering because it merged their canonical callable identities. The repaired path retains existing package ownership.
