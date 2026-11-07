"""Independent 80-digit tanh-sinh quadrature in log wavelength (mpmath 1.3.0).

This does not reuse the application's dimensionless Gauss rule, exponential
series, cutoff, derived constants or panel schedule. Exact SI h,c,k are pinned.
Run to freeze before executing the application; validation never regenerates it.
"""
import hashlib
import json
from pathlib import Path
import mpmath as mp

ROOT = Path(__file__).resolve().parent.parent
mp.mp.dps = 80
H = mp.mpf('6.62607015e-34')
C = mp.mpf('299792458')
K = mp.mpf('1.380649e-23')
SIGMA = 2 * mp.pi**5 * K**4 / (15 * H**3 * C**2)


def exitance(temperature, minimum, maximum):
    if not temperature:
        return mp.mpf(0)
    lower, upper = mp.log(minimum * mp.mpf('1e-6')), mp.log(maximum * mp.mpf('1e-6'))
    def spectrum(log_wavelength):
        wavelength = mp.exp(log_wavelength)
        return 2 * mp.pi * H * C**2 / (wavelength**4 * mp.expm1(H*C/(wavelength*K*temperature)))
    # Split logarithmic wavelength into <=0.25 segments to resolve the peak
    # without requiring the reference integrator to discover it over decades.
    count = int(mp.ceil((upper-lower)/mp.mpf('.25')))
    return mp.quad(spectrum, [lower + (upper-lower)*i/count for i in range(count+1)], method='tanh-sinh')


VALID = {
    'hot_longwave': '3000 100 1 10000 1 9999.9 10000',
    'cold_longwave': '100 0 1 10000 1 9999 10000',
    'short_endpoint': '3000 0 1 10000 1 0.1 0.101',
    'negative_wide': '100 3000 1 10000 1 0.1 10000',
    'mid_band': '500 300 0.5 3 0.4 2 5',
    'ulp_close_room': '300.000000001 300 1 10000 1 8 14',
    'ulp_close_hot': '3000 2999.999999999 1 10000 1 0.1 10000',
    'ulp_narrow': '3000 300 1 10000 1 10 10.000000000000002',
    'rounding_close': '100.100000000000002 100.1 1 10000 1 0.1 10000',
    'rounding_band': '3000 300 1 10000 1 8.199999999999999 8.200000000000001',
    'tie_even': '100.00000000000000710542735760100185871124267578125 100 1 10000 1 0.1 10000',
    'tie_odd': '100.00000000000002131628207280300557613372802734375 100 1 10000 1 0.1 10000',
    'room': '300 280 0.85 2 0.7 8 14',
    'zero_ambient': '300 0 1 1 1 8 14',
    'hot_narrow': '3000 1000 0.7 0.02 0.3 0.9 0.901',
    'hot_visible': '2500 300 1 1 1 0.1 0.2',
    'equal': '300 300 0.85 2 0.7 8 14',
    'wide': '3000 0 1 1 1 0.1 10000',
    'cold_wide': '100 0 1 1 1 0.1 10000',
    'left': '300 280 0.85 2 0.7 8 11',
    'right': '300 280 0.85 2 0.7 11 14',
    'swapped': '280 300 0.85 2 0.7 8 14',
    'dark': '100 0 1 1 1 0.1 0.2',
    'zero_emissivity': '300 280 0 2 0.7 8 14',
    'zero_area': '300 280 0.85 0 0.7 8 14',
    'zero_view': '300 280 0.85 2 0 8 14',
    'maximum_scale': '3000 100 1 10000 1 0.1 10000',
    'close_narrow': '300.000001 300 1 10000 1 10 10.000001',
    'close_hot_narrow': '3000 2999.99999 1 10000 1 10 10.000001',
    'long_decimal': '300.000000000 280 0.850000000 2 0.70000000 8 14',
    'capacity_lf': ' ' * (255-len('300 0 1 1 1 8 14')) + '300 0 1 1 1 8 14',
    'embedded_cr': '300\r280 0.85 2 0.7 8 14',
    'tiny_band': '3000 300 1 1 1 10 10.000001',
}
INVALID = {
    'reversed': '300 0 1 1 1 14 8',
    'equal_band': '300 0 1 1 1 8 8',
    'cold_emitter': '0 0 1 1 1 8 14',
    'cold_ambient': '300 99 1 1 1 8 14',
    'hot_emitter': '3001 0 1 1 1 8 14',
    'hot_ambient': '300 3001 1 1 1 8 14',
    'negative': '300 -1 1 1 1 8 14',
    'emissivity': '300 0 1.01 1 1 8 14',
    'area': '300 0 1 10001 1 8 14',
    'view': '300 0 1 1 1.01 8 14',
    'low_wavelength': '300 0 1 1 1 0.09 14',
    'high_wavelength': '300 0 1 1 1 8 10001',
    'nan': 'NaN 0 1 1 1 8 14',
    'inf': '300 inf 1 1 1 8 14',
    'overflow': '999999999999999999999999 0 1 1 1 8 14',
    'missing': '300 0 1 1 1 8',
    'extra': '300 0 1 1 1 8 14 9',
    'empty': '',
    'sign': '300 - 1 1 1 8 14',
    'decimal': '300 0 1..0 1 1 8 14',
    'negative_emissivity': '300 0 -0.1 1 1 8 14',
    'negative_area': '300 0 1 -1 1 8 14',
    'negative_view': '300 0 1 1 -1 8 14',
    'exponent': '3e2 0 1 1 1 8 14',
    'plus': '+300 0 1 1 1 8 14',
    'dot': '. 0 1 1 1 8 14',
    'lone_cr': '\r',
    'length': ' ' * 256 + '300 0 1 1 1 8 14',
}


def freeze():
    cases = {}
    for name, text in VALID.items():
        # Inputs are binary64 quantities, not exact decimal differences. Python's
        # independent correctly-rounded decimal conversion establishes those
        # quantities; mpmath then integrates their exact binary denotations.
        values = [mp.mpf(float(token)) for token in text.split()]
        emitter, ambient, emissivity, area, view, minimum, maximum = values
        emitted_exitance = exitance(emitter, minimum, maximum)
        ambient_exitance = exitance(ambient, minimum, maximum)
        scale = emissivity * area * view
        emitted, background = emitted_exitance*scale, ambient_exitance*scale
        cases[name] = dict(exit=0, expected=[mp.nstr(x, 75) for x in
            (emitted, background, emitted-background, emitted_exitance/(SIGMA*emitter**4))])
    for name in INVALID:
        cases[name] = dict(exit=64)
    for name, text in {**VALID, **INVALID}.items():
        (ROOT/'data'/f'{name}.stdin').write_text(text+'\n')
    (ROOT/'data/eof.stdin').write_bytes(VALID['room'].encode())
    cases['eof'] = cases['room']
    (ROOT/'data/crlf.stdin').write_bytes((VALID['room']+'\r\n').encode())
    cases['crlf'] = cases['room']
    (ROOT/'data/capacity_eof.stdin').write_bytes(VALID['capacity_lf'].encode())
    cases['capacity_eof'] = cases['capacity_lf']
    (ROOT/'data/capacity_overflow.stdin').write_bytes((VALID['capacity_lf']+' ').encode())
    cases['capacity_overflow'] = dict(exit=64)
    oracle = dict(method='mpmath 1.3.0; 80-digit log-wavelength tanh-sinh; binary64 inputs',
                  h=str(H), c=str(C), k=str(K), sigma=mp.nstr(SIGMA,75),
                  relative_tolerance='1e-6', absolute_floor_W='1e-12',
                  absolute_floor_fraction='1e-12', cases=cases)
    path = ROOT/'validation/frozen.json'
    path.write_text(json.dumps(oracle, indent=2)+'\n')
    paths = [Path(__file__), path, *sorted((ROOT/'data').glob('*.stdin'))]
    (ROOT/'validation/FROZEN_SHA256.txt').write_text(''.join(
        hashlib.sha256(p.read_bytes()).hexdigest()+'  '+str(p.relative_to(ROOT))+'\n' for p in paths))


if __name__ == '__main__':
    freeze()
