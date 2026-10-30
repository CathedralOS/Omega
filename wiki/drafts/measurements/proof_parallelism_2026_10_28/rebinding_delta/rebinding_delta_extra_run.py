from rebinding_delta_run import run
for producer,label in [('payload','delta-sorted-base-e'),('rebinding-delta-sorted','delta-sorted-candidate-e'),('rebinding-delta-sorted','delta-sorted-candidate-f'),('payload','delta-sorted-base-f')]:
 run(producer,label)
