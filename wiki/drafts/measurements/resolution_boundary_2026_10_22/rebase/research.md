# Transactional rebase research, 2026-10-22

Question: remove the measured full-carrier backup while retaining exact failure
rollback. Existing normalization/ownership research does not cover this failure
boundary. Query: `site.doc.rust-lang.org slice swap_with_slice source assert length before swap`.
Read the [Rust core slice implementation](https://doc.rust-lang.org/src/core/slice/mod.rs.html),
`swap_with_slice` (lines 4442–4449 in the retrieved source): it validates lengths
before mutation. This supports preflight-then-commit as an ownership pattern,
not Omega's specific validity rules or a performance claim. No unsafe code is
introduced here. Local rebase code already validates each store before changing
that store. Prepare the expression remapping and validate every remaining store
in the same diagnostic order before committing any changes. Preserve the
existing remapping allocation and append order. Reject on any changed output,
error variant/order, or mutated returned carrier; compare against frozen old
methods and pin late failures after earlier stores would have changed.
