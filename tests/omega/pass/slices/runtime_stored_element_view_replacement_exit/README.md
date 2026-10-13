# Record-carried view replacement

Replace one record through a mutable element view stored in a borrowed record,
then return that view across a state and call boundary. Both execution engines
must observe the replacement's two fields in the original backing and leave
the surrounding sentinels unchanged. This varies the direct-view replacement
fixture without changing the required storage operation.
