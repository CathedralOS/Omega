# Intended-invalid coordinate access

Project role: intended-invalid control. This separate complete program must
reject `cannot prove index` and `within length 48` for `coordinate::entry`.
The source helper's runtime index has no coordinate-array bound. Passing 48
from the explicit entry also cannot discharge that obligation. No runtime
execution, malformed input or package-admission refusal counts as this result.
