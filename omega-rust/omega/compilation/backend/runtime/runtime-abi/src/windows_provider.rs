//! Windows foreign image for the classful internet-address operations.
//!
//! Win32 and the Universal CRT do not export `inet_makeaddr` or `inet_lnaof`.
//! A guest that satisfies those boundary requirements with `PeByName` still
//! needs a loader-visible library beside the published executable. This image
//! is that library: two `extern "C"` exports whose bodies are the same
//! classful definition `network_address` evaluates for Terminal execution.
//! The export names are the C names; the guest requirement path is not an
//! input. Little-endian `bswap` matches `u32::swap_bytes` on Windows x64.

pub const WINDOWS_INTERNET_PROVIDER_LIBRARY: &[u8] = b"omega_host.dll";

pub fn image_imports_windows_internet_provider(image: &[u8]) -> bool {
    image
        .windows(WINDOWS_INTERNET_PROVIDER_LIBRARY.len())
        .any(|window| window == WINDOWS_INTERNET_PROVIDER_LIBRARY)
}

pub fn windows_internet_provider_dll() -> Vec<u8> {
    let make = make_body();
    let host = host_body();
    let mut text = Vec::with_capacity(make.len() + host.len());
    text.extend_from_slice(&make);
    text.extend_from_slice(&host);
    let text_rva_make = 0x1000u32;
    let text_rva_host = 0x1000 + u32::try_from(make.len()).expect("make body fits in a u32");
    let text_raw = align_bytes(&text, 0x200);

    let name_lna = b"inet_lnaof\0";
    let name_make = b"inet_makeaddr\0";
    let dll_name = b"omega_host.dll\0";
    let names_off = 48usize;
    let ordinals_off = 56usize;
    let strings_off = 60usize;
    let mut rdata = vec![0u8; strings_off];
    rdata[40..44].copy_from_slice(&text_rva_host.to_le_bytes());
    rdata[44..48].copy_from_slice(&text_rva_make.to_le_bytes());
    let str_lna = strings_off;
    rdata.extend_from_slice(name_lna);
    let str_make = rdata.len();
    rdata.extend_from_slice(name_make);
    let str_dll = rdata.len();
    rdata.extend_from_slice(dll_name);
    put_u32(
        &mut rdata,
        names_off,
        0x2000 + u32::try_from(str_lna).unwrap(),
    );
    put_u32(
        &mut rdata,
        names_off + 4,
        0x2000 + u32::try_from(str_make).unwrap(),
    );
    rdata[ordinals_off] = 0;
    rdata[ordinals_off + 1] = 0;
    rdata[ordinals_off + 2] = 1;
    rdata[ordinals_off + 3] = 0;
    put_u32(&mut rdata, 0, 0);
    put_u32(&mut rdata, 4, 0);
    put_u16(&mut rdata, 8, 0);
    put_u16(&mut rdata, 10, 0);
    put_u32(&mut rdata, 12, 0x2000 + u32::try_from(str_dll).unwrap());
    put_u32(&mut rdata, 16, 1);
    put_u32(&mut rdata, 20, 2);
    put_u32(&mut rdata, 24, 2);
    put_u32(&mut rdata, 28, 0x2000 + 40);
    put_u32(&mut rdata, 32, 0x2000 + u32::try_from(names_off).unwrap());
    put_u32(
        &mut rdata,
        36,
        0x2000 + u32::try_from(ordinals_off).unwrap(),
    );
    let rdata_len = rdata.len();
    let rdata_raw = align_bytes(&rdata, 0x200);

    let mut pe = vec![0u8; 0x200];
    pe[0] = b'M';
    pe[1] = b'Z';
    put_u32(&mut pe, 0x3C, 0x80);
    pe[0x80..0x84].copy_from_slice(b"PE\0\0");
    put_u16(&mut pe, 0x84, 0x8664);
    put_u16(&mut pe, 0x86, 2);
    put_u16(&mut pe, 0x94, 240);
    put_u16(&mut pe, 0x96, 0x2000 | 0x0002 | 0x0020);
    let opt = 0x98;
    put_u16(&mut pe, opt, 0x20B);
    pe[opt + 2] = 14;
    put_u32(&mut pe, opt + 4, u32::try_from(text_raw.len()).unwrap());
    put_u32(&mut pe, opt + 8, u32::try_from(rdata_raw.len()).unwrap());
    put_u32(&mut pe, opt + 20, 0x1000);
    put_u64(&mut pe, opt + 24, 0x1_8000_0000);
    put_u32(&mut pe, opt + 32, 0x1000);
    put_u32(&mut pe, opt + 36, 0x200);
    put_u16(&mut pe, opt + 40, 6);
    put_u16(&mut pe, opt + 48, 6);
    put_u32(&mut pe, opt + 56, 0x3000);
    put_u32(&mut pe, opt + 60, 0x200);
    put_u16(&mut pe, opt + 68, 3);
    put_u16(&mut pe, opt + 70, 0x0160);
    put_u64(&mut pe, opt + 72, 0x100000);
    put_u64(&mut pe, opt + 80, 0x1000);
    put_u64(&mut pe, opt + 88, 0x100000);
    put_u64(&mut pe, opt + 96, 0x1000);
    put_u32(&mut pe, opt + 108, 16);
    put_u32(&mut pe, opt + 112, 0x2000);
    put_u32(&mut pe, opt + 116, u32::try_from(rdata_len).unwrap());

    write_section(
        &mut pe,
        0x188,
        b".text\0\0\0",
        u32::try_from(text.len()).unwrap(),
        0x1000,
        u32::try_from(text_raw.len()).unwrap(),
        0x200,
        0x6000_0020,
    );
    write_section(
        &mut pe,
        0x188 + 40,
        b".rdata\0\0",
        u32::try_from(rdata_len).unwrap(),
        0x2000,
        u32::try_from(rdata_raw.len()).unwrap(),
        0x200 + u32::try_from(text_raw.len()).unwrap(),
        0x4000_0040,
    );

    let mut image = pe;
    image.extend_from_slice(&text_raw);
    image.extend_from_slice(&rdata_raw);
    image
}

fn make_body() -> Vec<u8> {
    let mut code = Vec::new();
    code.extend_from_slice(&[0x81, 0xF9, 0x80, 0x00, 0x00, 0x00]);
    let jae_a = code.len();
    code.extend_from_slice(&[0x73, 0x00]);
    code.extend_from_slice(&[0x89, 0xD0]);
    code.extend_from_slice(&[0x25, 0xFF, 0xFF, 0xFF, 0x00]);
    code.extend_from_slice(&[0xC1, 0xE1, 0x18]);
    code.extend_from_slice(&[0x09, 0xC8]);
    let jmp_swap_a = code.len();
    code.extend_from_slice(&[0xEB, 0x00]);
    let not_a = code.len();
    code[jae_a + 1] = u8::try_from(not_a - (jae_a + 2)).unwrap();
    code.extend_from_slice(&[0x81, 0xF9, 0x00, 0x00, 0x01, 0x00]);
    let jae_b = code.len();
    code.extend_from_slice(&[0x73, 0x00]);
    code.extend_from_slice(&[0x89, 0xD0]);
    code.extend_from_slice(&[0x25, 0xFF, 0xFF, 0x00, 0x00]);
    code.extend_from_slice(&[0xC1, 0xE1, 0x10]);
    code.extend_from_slice(&[0x09, 0xC8]);
    let jmp_swap_b = code.len();
    code.extend_from_slice(&[0xEB, 0x00]);
    let not_b = code.len();
    code[jae_b + 1] = u8::try_from(not_b - (jae_b + 2)).unwrap();
    code.extend_from_slice(&[0x81, 0xF9, 0x00, 0x00, 0x00, 0x01]);
    let jae_c = code.len();
    code.extend_from_slice(&[0x73, 0x00]);
    code.extend_from_slice(&[0x89, 0xD0]);
    code.extend_from_slice(&[0x25, 0xFF, 0x00, 0x00, 0x00]);
    code.extend_from_slice(&[0xC1, 0xE1, 0x08]);
    code.extend_from_slice(&[0x09, 0xC8]);
    let jmp_swap_c = code.len();
    code.extend_from_slice(&[0xEB, 0x00]);
    let class_other = code.len();
    code[jae_c + 1] = u8::try_from(class_other - (jae_c + 2)).unwrap();
    code.extend_from_slice(&[0x89, 0xC8]);
    code.extend_from_slice(&[0x09, 0xD0]);
    let swap = code.len();
    for site in [jmp_swap_a, jmp_swap_b, jmp_swap_c] {
        code[site + 1] = u8::try_from(swap - (site + 2)).unwrap();
    }
    code.extend_from_slice(&[0x0F, 0xC8]);
    code.push(0xC3);
    code
}

fn host_body() -> Vec<u8> {
    let mut code = Vec::new();
    code.extend_from_slice(&[0x89, 0xC8]);
    code.extend_from_slice(&[0x0F, 0xC8]);
    code.extend_from_slice(&[0x89, 0xC1]);
    code.extend_from_slice(&[0xF7, 0xC1, 0x00, 0x00, 0x00, 0x80]);
    let jnz = code.len();
    code.extend_from_slice(&[0x75, 0x00]);
    code.extend_from_slice(&[0x25, 0xFF, 0xFF, 0xFF, 0x00]);
    code.push(0xC3);
    let not_a = code.len();
    code[jnz + 1] = u8::try_from(not_a - (jnz + 2)).unwrap();
    code.extend_from_slice(&[0x89, 0xCA]);
    code.extend_from_slice(&[0x81, 0xE2, 0x00, 0x00, 0x00, 0xC0]);
    code.extend_from_slice(&[0x81, 0xFA, 0x00, 0x00, 0x00, 0x80]);
    let jne = code.len();
    code.extend_from_slice(&[0x75, 0x00]);
    code.extend_from_slice(&[0x25, 0xFF, 0xFF, 0x00, 0x00]);
    code.push(0xC3);
    let class_c = code.len();
    code[jne + 1] = u8::try_from(class_c - (jne + 2)).unwrap();
    code.extend_from_slice(&[0x25, 0xFF, 0x00, 0x00, 0x00]);
    code.push(0xC3);
    code
}

fn align_bytes(bytes: &[u8], boundary: usize) -> Vec<u8> {
    let size = bytes.len().div_ceil(boundary) * boundary;
    let mut aligned = bytes.to_vec();
    aligned.resize(size, 0);
    aligned
}

fn put_u16(bytes: &mut [u8], offset: usize, value: u16) {
    bytes[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn put_u32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn put_u64(bytes: &mut [u8], offset: usize, value: u64) {
    bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}

fn write_section(
    pe: &mut [u8],
    offset: usize,
    name: &[u8; 8],
    virtual_size: u32,
    virtual_address: u32,
    raw_size: u32,
    raw_pointer: u32,
    flags: u32,
) {
    pe[offset..offset + 8].copy_from_slice(name);
    put_u32(pe, offset + 8, virtual_size);
    put_u32(pe, offset + 12, virtual_address);
    put_u32(pe, offset + 16, raw_size);
    put_u32(pe, offset + 20, raw_pointer);
    put_u32(pe, offset + 36, flags);
}

#[cfg(test)]
mod tests {
    use super::{image_imports_windows_internet_provider, windows_internet_provider_dll};
    use crate::network_address::{classful_internet_address, classful_local_address};

    #[test]
    fn provider_image_names_its_library_and_exports() {
        let image = windows_internet_provider_dll();
        assert!(image_imports_windows_internet_provider(&image));
        assert!(
            image
                .windows(b"inet_makeaddr\0".len())
                .any(|window| window == b"inet_makeaddr\0")
        );
        assert!(
            image
                .windows(b"inet_lnaof\0".len())
                .any(|window| window == b"inet_lnaof\0")
        );
        assert_eq!(&image[0..2], b"MZ");
    }

    #[cfg(windows)]
    #[test]
    fn loaded_provider_matches_the_classful_definition() {
        let image = windows_internet_provider_dll();
        let path =
            std::env::temp_dir().join(format!("omega_host_provider_{}.dll", std::process::id()));
        std::fs::write(&path, &image).unwrap();
        let observed = call_loaded(&path).expect("omega_host.dll loaded");
        let _ = std::fs::remove_file(&path);
        for (network, host, made, local) in observed {
            let stored = classful_internet_address(network, host);
            assert_eq!(made, stored);
            assert_eq!(local, classful_local_address(stored));
        }
    }

    #[cfg(windows)]
    fn call_loaded(path: &std::path::Path) -> Result<Vec<(u32, u32, u32, u32)>, String> {
        unsafe extern "system" {
            fn LoadLibraryA(name: *const u8) -> *mut core::ffi::c_void;
            fn GetProcAddress(
                module: *mut core::ffi::c_void,
                name: *const u8,
            ) -> *mut core::ffi::c_void;
            fn FreeLibrary(module: *mut core::ffi::c_void) -> i32;
        }
        let mut file = path.to_string_lossy().into_owned();
        file.push('\0');
        let module = unsafe { LoadLibraryA(file.as_ptr()) };
        if module.is_null() {
            return Err("LoadLibrary failed".to_owned());
        }
        let make = unsafe { GetProcAddress(module, b"inet_makeaddr\0".as_ptr()) };
        let host = unsafe { GetProcAddress(module, b"inet_lnaof\0".as_ptr()) };
        if make.is_null() || host.is_null() {
            unsafe { FreeLibrary(module) };
            return Err("export missing".to_owned());
        }
        let make: unsafe extern "C" fn(u32, u32) -> u32 = unsafe { std::mem::transmute(make) };
        let host: unsafe extern "C" fn(u32) -> u32 = unsafe { std::mem::transmute(host) };
        let pairs = [(10u32, 66051u32), (44048, 513), (10, 1027), (200, 3)]
            .into_iter()
            .map(|(network, host_number)| {
                let made = unsafe { make(network, host_number) };
                let local = unsafe { host(made) };
                (network, host_number, made, local)
            })
            .collect();
        unsafe { FreeLibrary(module) };
        Ok(pairs)
    }
}
