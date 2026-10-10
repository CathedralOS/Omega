# Diagonal readonly SPD fixture

A = diag(4,9,16); b=(4,-18,48); L=diag(2,3,4);
y=(2,-6,12); x=(1,-2,3). All backing is column-major. The fixed-size helper
accepts borrowed readonly source and distinct mutable factor output. The source
must retain its original values. This tests existing sqrt, scalar division,
fixed-array borrowing and readonly source; shared descriptor API is pending.

Exit 0 requires factor, solution, exact residual zero and preserved input.
Exact equality is a stronger check than frozen 1e-12 absolute/relative limits
for these exactly representable integer fixtures. No user input is required.
The parent README gives exact check/native/Terminal commands and limitations.
