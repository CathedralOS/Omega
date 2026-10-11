"""Independent fixture calculation; this is an oracle, not the application.

Use raw-observation sensitivities, not the corrected-mass covariance expression
implemented by Omega. Expected.json is frozen separately before execution.
"""
from fractions import Fraction
from decimal import Decimal, localcontext

def reference(gross_a, gross_b, tare, scale, shared):
    a = (Fraction(gross_a) - tare) * scale
    b = (Fraction(gross_b) - tare) * scale
    total = a + b
    if min(a, b) < 0 or total < Fraction(1, 100):
        raise ValueError('inadmissible corrected mass')
    derivative_a = b / total**2
    derivative_b = -a / total**2
    gross_variance = Fraction(1, 100) * scale**2
    tare_variance = Fraction(1, 25) * scale**2
    fraction_variance = (derivative_a**2 + derivative_b**2) * gross_variance
    if shared:
        fraction_variance += (derivative_a + derivative_b)**2 * tare_variance
    else:
        fraction_variance += (derivative_a**2 + derivative_b**2) * tare_variance
    return {'mass_a': a, 'mass_b': b, 'total': total,
            'mass_variance': gross_variance + tare_variance,
            'covariance': tare_variance if shared else Fraction(0),
            'total_variance': 2*gross_variance + (4 if shared else 2)*tare_variance,
            'fraction_a': a/total, 'fraction_b': b/total,
            'fraction_variance': fraction_variance}

def outputs(values):
    factors = {'mass_a_ug': ('mass_a', 10**6), 'mass_b_ug': ('mass_b', 10**6),
               'total_ug': ('total', 10**6), 'mass_variance_nano_g2': ('mass_variance', 10**9),
               'covariance_nano_g2': ('covariance', 10**9), 'total_variance_nano_g2': ('total_variance', 10**9),
               'fraction_a_ppm': ('fraction_a', 10**6), 'fraction_b_ppm': ('fraction_b', 10**6),
               'fraction_variance_nano': ('fraction_variance', 10**9),
               'standard_uncertainty_ppm': ('uncertainty', 10**6),
               'expanded_uncertainty_ppm': ('expanded', 10**6),
               'interval_lower_ppm': ('lower', 10**6), 'interval_upper_ppm': ('upper', 10**6)}
    with localcontext() as context:
        context.prec = 70
        result = {key: Decimal(value.numerator)/Decimal(value.denominator) for key, value in values.items()}
        result['uncertainty'] = result['fraction_variance'].sqrt()
        result['expanded'] = 2 * result['uncertainty']
        result['lower'] = result['fraction_a'] - result['expanded']
        result['upper'] = result['fraction_a'] + result['expanded']
        return {key: str(result[field] * factor) for key, (field, factor) in factors.items()}
