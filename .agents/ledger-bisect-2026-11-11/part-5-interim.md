# Ledger bisect part 5 (interim)
PIN: `51e8c45c793007f211d0d9baee7de16767172444`

## Cluster 0 statement-successor (PIN still broken: 10)
- witness: `pass/facts/selected_native_code_layout_through_other_state_exit`
- good: `5c6c16d4d186`
- breaking: `d26fa952d579` PR #1909
- mechanism: psi: fuse terminal calls into the caller activation (#1909)
- status: bisected
