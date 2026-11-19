//! Lay out already encoded helper exports in a minimal Windows DLL. This
//! writer creates headers, sections and exports; it never chooses instructions.
pub fn emit_windows_internet_provider(code: &machine_code::WindowsInternetProviderCode) -> Vec<u8> {
    let make = &code.make_address.bytes;
    let host = &code.local_address.bytes;
    let mut text = Vec::with_capacity(make.len() + host.len());
    text.extend_from_slice(make);
    text.extend_from_slice(host);
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
#[path = "windows_provider_reference.rs"]
mod reference;

#[cfg(test)]
mod tests {
    use super::emit_windows_internet_provider;
    fn windows_internet_provider_dll() -> Vec<u8> {
        let code = native_encoding::windows_internet::encode_windows_internet_provider(
            entry_planning::windows_internet::plan_windows_internet_provider(),
        )
        .unwrap();
        emit_windows_internet_provider(&code)
    }
    fn image_imports_windows_internet_provider(image: &[u8]) -> bool {
        image
            .windows(b"omega_host.dll".len())
            .any(|row| row == b"omega_host.dll")
    }
    #[test]
    fn complete_container_matches_the_previous_provider() {
        assert_eq!(
            windows_internet_provider_dll(),
            super::reference::windows_internet_provider_dll()
        );
    }
    #[cfg(windows)]
    use runtime_abi::HostInternetAddress;

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
            let expected = HostInternetAddress::make(network, host).unwrap();
            let local_expected = expected.host_number().unwrap();
            let stored: u32 = unsafe { std::mem::transmute(expected) };
            assert_eq!(made, stored);
            assert_eq!(local, local_expected);
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
        let make = unsafe { GetProcAddress(module, c"inet_makeaddr".as_ptr().cast()) };
        let host = unsafe { GetProcAddress(module, c"inet_lnaof".as_ptr().cast()) };
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
