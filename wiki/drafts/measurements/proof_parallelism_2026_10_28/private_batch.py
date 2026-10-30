"""Equal full-check workloads, isolated from the shared source-graph cache."""
from run import OUT, measure, verify_inputs
import json

verify_inputs()
cases = [
    ('candidate', 'private-8-a', 'timing', 8),
    ('candidate', 'private-off-a', 'off', 0),
    ('candidate', 'private-1-a', 'timing', 1),
    ('candidate', 'private-32-a', 'timing', 32),
    ('baseline', 'private-baseline-a', 'off', 0),
    ('candidate', 'private-32-b', 'timing', 32),
    ('candidate', 'private-1-b', 'timing', 1),
    ('candidate', 'private-8-b', 'timing', 8),
    ('candidate', 'private-off-b', 'off', 0),
    ('baseline', 'private-baseline-b', 'off', 0),
]
for producer, label, mode, workers in cases:
    measure(producer, label, mode, workers, cold_cache=True)
measure('candidate', 'private-counts', 'counts', 8, cold_cache=True)
measure('candidate', 'private-attribution', 'off', 32, phase_profile=True, cold_cache=True)
measure('candidate', 'candidate-verified', 'off', 0, comparison=True)
verify_inputs()
labels = [label for _, label, _, _ in cases] + ['private-counts', 'private-attribution']
images = {json.loads((OUT / label / 'result.json').read_text())['image_sha256'] for label in labels}
assert len(images) == 1, images
print('All fixed sources and native image digests agree.', flush=True)
