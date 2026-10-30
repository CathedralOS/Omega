from cache_diagnostics_run import run
for cache,letter in [('on','a'),('off','a'),('off','b'),('on','b')]:
    run(cache,'timing','cache-cost-'+cache+'-'+letter,producer='edge-cache-cost-probe')
