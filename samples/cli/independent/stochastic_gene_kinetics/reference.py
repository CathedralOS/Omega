"""Independent direct-method oracle using Python's math.log and Poisson laws.

This does not import the Omega recurrence, logarithm approximation or output
formatter. It supplies an exact-seed cross-check as well as distributional tests.
"""
import math

MODULUS = 2147483647


def uniform(stream):
    stream[0] = stream[0] * 16807 % MODULUS
    return stream[0] / MODULUS


def trajectory(seed=1, gene=False, budget=100):
    stream = [seed]
    time, transcript, protein = 0.0, 0, 0
    events = []
    while True:
        propensities = [2.0, float(transcript) if gene else 0.0,
                        float(transcript), 0.5 * protein if gene else 0.0]
        total = sum(propensities)
        if total == 0:
            return events, transcript, protein, stream[0], 'complete'
        arrival = -math.log(uniform(stream)) / total
        if time + arrival > 2.0:
            return events, transcript, protein, stream[0], 'complete'
        if len(events) == budget:
            return events, transcript, protein, stream[0], 'budget'
        threshold = uniform(stream) * total
        accumulated = 0.0
        for channel, propensity in enumerate(propensities, start=1):
            accumulated += propensity
            if threshold < accumulated:
                break
        if channel == 1:
            transcript += 1
        elif channel == 2:
            protein += 1
        elif channel == 3:
            transcript -= 1
        else:
            protein -= 1
        assert transcript >= 0 and protein >= 0
        time += arrival
        events.append((time, channel, transcript, protein))


def ensemble(seed=1, gene=False, count=1024):
    # A continuous Park–Miller stream is passed between independent replicas.
    xs, ys = [], []
    for _ in range(count):
        events, x, y, seed, status = trajectory(seed, gene)
        assert status == 'complete'
        xs.append(x)
        ys.append(y)
    return {'n': count, 'seed_end': seed, 'sum_x': sum(xs),
            'sum_x2': sum(x*x for x in xs), 'sum_y': sum(ys),
            'sum_y2': sum(y*y for y in ys),
            'hist_x': [xs.count(0), xs.count(1), xs.count(2),
                       sum(x >= 3 for x in xs)]}


def analytic():
    # Immigration-death X(2) is Poisson with parameter b/d*(1-e^-2d).
    mean_x = 2 * (1 - math.exp(-2))
    # Protein receives translation from X and decays at rate 1/2.
    # Integrate E[X(s)] exp(-(2-s)/2) over s in [0,2].
    mean_y = 2 * ((1 - math.exp(-1)) / 0.5
                  - (math.exp(-1) - math.exp(-2)) / 0.5)
    probabilities = [math.exp(-mean_x), mean_x*math.exp(-mean_x),
                     mean_x**2*math.exp(-mean_x)/2]
    probabilities.append(1-sum(probabilities))
    return {'mean_x': mean_x, 'variance_x': mean_x,
            'second_x': mean_x + mean_x**2,
            'mean_protein': mean_y,
            'hist_x': probabilities}


if __name__ == '__main__':
    import json
    print(json.dumps({'analytic': analytic(),
                      'immigration': ensemble(), 'gene': ensemble(gene=True),
                      'immigration_events': trajectory()[0],
                      'gene_events': trajectory(gene=True)[0]}, indent=2))
