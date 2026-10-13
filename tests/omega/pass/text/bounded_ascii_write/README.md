# bounded_ascii_write

`domain [u8;8]::Ascii` + `bytes[at]=value` indexed write through `&mut [u8;8] in Ascii` under `at<len && value<128` — qualified owner, not raw byte view.

Compile-checked only: run a check target, not native.
