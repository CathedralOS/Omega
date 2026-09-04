# graph-workbench

Root graph fixture. It depends on one pure package and one capability-bearing
package so graph audit output can name the dependency path that introduced
authority.

Expected package evidence:

- dependency aliases include `arithmetic_kernels` and `file_journal`;
- transitive audit reports the path through `file_journal` for filesystem
  reach;
- review presents the complete transitive set for a baseline decision;
- the lock records that decision under the authority of whoever lands it,
  without certifying package or lock acceptance.
