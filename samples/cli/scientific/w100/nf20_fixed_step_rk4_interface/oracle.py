"""Independent decimal fixture; never reads or translates Omega source."""
from decimal import Decimal, localcontext
import json
from pathlib import Path
fixtures = json.loads(Path(__file__).with_name('oracles.json').read_text())
with localcontext() as context:
    context.prec = 80
    step = Decimal('0.025')
    for name in ['decay', 'driven_growth']:
        value, time = Decimal(1), Decimal(0)
        def rhs(time, value):
            return -2 * value if name == 'decay' else value + time
        for index in range(40):
            first = rhs(time, value)
            second = rhs(time + step/2, value + step*first/2)
            third = rhs(time + step/2, value + step*second/2)
            fourth = rhs(time + step, value + step*third)
            value += step*(first + 2*second + 2*third + fourth)/6
            time += step
        expected = (-2*time).exp() if name == 'decay' else 2*time.exp()-time-1
        tolerance = Decimal(str(fixtures[name]['absolute_tolerance']))
        assert abs(value-expected) <= tolerance
        assert abs(expected-Decimal(str(fixtures[name]['expected']))) < Decimal('1e-15')
        print(name, 'RK4=', value, 'analytic=', expected, 'error=', abs(value-expected))
