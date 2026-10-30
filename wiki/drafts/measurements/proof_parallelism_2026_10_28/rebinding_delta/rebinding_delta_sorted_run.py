from rebinding_delta_run import run
run('rebinding-delta-sorted','delta-sorted-counts',counts=True)
for producer,label in [('payload','delta-sorted-base-a'),('rebinding-delta-sorted','delta-sorted-candidate-a'),
 ('rebinding-delta-sorted','delta-sorted-candidate-b'),('payload','delta-sorted-base-b'),
 ('payload','delta-sorted-base-c'),('rebinding-delta-sorted','delta-sorted-candidate-c'),
 ('rebinding-delta-sorted','delta-sorted-candidate-d'),('payload','delta-sorted-base-d')]:
    run(producer,label)
run('rebinding-delta-sorted','delta-sorted-verified',comparison=True)
