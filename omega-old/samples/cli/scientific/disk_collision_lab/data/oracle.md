# Frozen independent oracle

Frozen before any Omega compilation or execution. Output absolute tolerance
1e-6 (six decimal places); internal contact and conservation tolerance 1e-9.
Finite fixture arithmetic has no relative tolerance. Units: m, s, kg, N s, J.

Independent analytic calculation: for head-on impact at t=2, inverse reduced
mass is 1+1/3=4/3. J=(1+e)/(4/3), giving J=3/2 for e=1,
J=9/8 for e=1/2. Velocities are (1-J,J/3). Initial energy is 1/2;
for e=1/2 final energy is 7/32 and loss is 9/32. Position at t=3
is the contact position plus one second of final velocity.

For grazing, y offset is 2, contact occurs at t=2, normal=(0,1),
radial relative velocity is zero, so J=0. For oblique, offset at
contact is (6/5,8/5), unit normal=(3/5,4/5), equal masses give
J=3/5; velocities are (16/25,-12/25) and (9/25,12/25).

The simultaneous fixture has two pairs separated by 10 m vertically.
All cross-pair candidates miss. Both events occur at t=2; lexicographic
order is (0,1), then (2,3). Their impulses are 1 and 2.
The separating case has no nonnegative approaching contact. Budget one
accepts (0,1) at t=2 then stops before the equally timed pending (2,3)
event. All positions are at contact time, only the first pair has exchanged
velocity, and momentum 3 and energy 3/2 are retained.
Cases 7,8,9 reject mass zero, overlap, restitution >1 respectively.
The JSON values are these independent rational calculations, not generated
by replaying the Omega algorithm. Checks include every final disk component,
every ordered contact, all emitted conservation rows, status and exits.
