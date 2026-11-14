//! Resolve the annotated Alpha Windows seed listing.
//!
//! `mbx xtask alpha-forge LISTING OUTPUT` writes the image.
//! `mbx xtask alpha-forge LISTING --check COMMITTED` compares it to a seed and
//! exits 1 when the bytes differ. The listing dialect is the one committed in
//! `bootstrap/interpreted/0_alpha/alpha_x64_windows.hex`: hex bytes, `zN` zero
//! runs, `.pad`, labels, `.rva`, `.rel32`, and `.rel8`.

use std::fs;
use std::path::Path;

pub(crate) fn run(args: &[String]) -> Result<(), u8> {
    match args {
        [listing, output] => match forge_file(Path::new(listing)) {
            Ok(image) => {
                if let Err(error) = fs::write(output, &image) {
                    eprintln!("alpha forge: {error}");
                    return Err(1);
                }
                println!("wrote {output} ({} bytes)", image.len());
                Ok(())
            }
            Err(message) => {
                eprintln!("alpha forge: {message}");
                Err(1)
            }
        },
        [listing, flag, committed] if flag == "--check" => {
            match (forge_file(Path::new(listing)), fs::read(committed)) {
                (Ok(image), Ok(committed_bytes)) => {
                    if committed_bytes == image {
                        println!(
                            "check OK — listing reproduces the committed binary byte-for-byte"
                        );
                        return Ok(());
                    }
                    let shared = committed_bytes.len().min(image.len());
                    if let Some(offset) =
                        (0..shared).find(|offset| committed_bytes[*offset] != image[*offset])
                    {
                        println!(
                            "first diff at file offset {offset:#x}: committed {:02x} forged {:02x}",
                            committed_bytes[offset], image[offset]
                        );
                    }
                    println!(
                        "lengths committed {} forged {}",
                        committed_bytes.len(),
                        image.len()
                    );
                    Err(1)
                }
                (Err(message), _) => {
                    eprintln!("alpha forge: {message}");
                    Err(1)
                }
                (_, Err(error)) => {
                    eprintln!("alpha forge: {error}");
                    Err(1)
                }
            }
        }
        _ => {
            eprintln!("usage: mbx xtask alpha-forge LISTING OUTPUT");
            eprintln!("       mbx xtask alpha-forge LISTING --check COMMITTED");
            Err(2)
        }
    }
}

fn forge_file(path: &Path) -> Result<Vec<u8>, String> {
    let listing = fs::read_to_string(path).map_err(|error| error.to_string())?;
    forge_listing(&listing)
}

enum Op {
    Bytes { at: usize, bytes: Vec<u8> },
    Rel8 { name: String, at: usize },
    Rel32 { name: String, at: usize },
    Rva { name: String, at: usize },
}

struct Section {
    virtual_address: u32,
    raw_size: u32,
    raw_pointer: u32,
}

fn forge_listing(listing: &str) -> Result<Vec<u8>, String> {
    let (ops, labels, len) = parse_listing(listing)?;
    let mut image = vec![0u8; len];
    for op in &ops {
        if let Op::Bytes { at, bytes } = op {
            let end = at + bytes.len();
            image
                .get_mut(*at..end)
                .ok_or_else(|| format!("byte run at {at:#x} exceeds the image"))?
                .copy_from_slice(bytes);
        }
    }
    let sections = section_map(&image)?;
    for op in &ops {
        match op {
            Op::Bytes { .. } => {}
            Op::Rva { name, at } => {
                let value = label_rva(&labels, &sections, name)?;
                write_at(&mut image, *at, &value.to_le_bytes())?;
            }
            Op::Rel8 { name, at } => {
                let value = relocation(&labels, &sections, name, *at, 1)?;
                write_at(&mut image, *at, &value.to_le_bytes()[..1])?;
            }
            Op::Rel32 { name, at } => {
                let value = relocation(&labels, &sections, name, *at, 4)?;
                write_at(&mut image, *at, &value.to_le_bytes())?;
            }
        }
    }
    Ok(image)
}

fn parse_listing(listing: &str) -> Result<(Vec<Op>, Vec<(String, usize)>, usize), String> {
    let mut tokens = Vec::new();
    for line in listing.lines() {
        let code = line.split(';').next().unwrap_or("");
        tokens.extend(code.split_whitespace().map(str::to_owned));
    }
    let mut ops = Vec::new();
    let mut labels = Vec::new();
    let mut cursor = 0usize;
    let mut index = 0usize;
    while index < tokens.len() {
        let token = &tokens[index];
        if let Some(name) = token.strip_suffix(':') {
            labels.push((name.to_owned(), cursor));
            index += 1;
            continue;
        }
        if token == ".pad" {
            let target = tokens.get(index + 1).ok_or("missing operand for .pad")?;
            let target = parse_hex(target).map_err(|error| format!(".pad {error}"))?;
            if target < cursor {
                return Err(format!(".pad to {target:#x} while at {cursor:#x}"));
            }
            cursor = target;
            index += 2;
            continue;
        }
        if matches!(token.as_str(), ".rel8" | ".rel32" | ".rva") {
            let name = tokens
                .get(index + 1)
                .ok_or_else(|| format!("missing operand for {token}"))?
                .clone();
            let width = if token == ".rel8" { 1 } else { 4 };
            match token.as_str() {
                ".rel8" => ops.push(Op::Rel8 { name, at: cursor }),
                ".rel32" => ops.push(Op::Rel32 { name, at: cursor }),
                _ => ops.push(Op::Rva { name, at: cursor }),
            }
            cursor += width;
            index += 2;
            continue;
        }
        if let Some(count) = parse_zero_run(token) {
            cursor += count;
            index += 1;
            continue;
        }
        if let Some(byte) = parse_byte(token) {
            if let Some(Op::Bytes { at, bytes }) = ops.last_mut()
                && *at + bytes.len() == cursor
            {
                bytes.push(byte);
            } else {
                ops.push(Op::Bytes {
                    at: cursor,
                    bytes: vec![byte],
                });
            }
            cursor += 1;
            index += 1;
            continue;
        }
        return Err(format!("unrecognized token {token:?}"));
    }
    Ok((ops, labels, cursor))
}

fn parse_hex(token: &str) -> Result<usize, String> {
    let digits = token
        .strip_prefix("0x")
        .or_else(|| token.strip_prefix("0X"))
        .unwrap_or(token);
    if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(format!("value {token:?} is not hexadecimal"));
    }
    usize::from_str_radix(digits, 16).map_err(|_| format!("value {token:?} does not fit"))
}

fn parse_zero_run(token: &str) -> Option<usize> {
    let digits = token.strip_prefix('z')?;
    if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    digits.parse().ok()
}

fn parse_byte(token: &str) -> Option<u8> {
    if token.len() == 2 && token.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        u8::from_str_radix(token, 16).ok()
    } else {
        None
    }
}

fn section_map(image: &[u8]) -> Result<Vec<Section>, String> {
    let pe = read_u32(image, 0x3C)? as usize;
    let section_count = read_u16(image, pe + 6)? as usize;
    let optional_size = read_u16(image, pe + 20)? as usize;
    let table = pe + 24 + optional_size;
    let mut sections = Vec::with_capacity(section_count);
    for index in 0..section_count {
        let base = table + index * 40;
        sections.push(Section {
            virtual_address: read_u32(image, base + 12)?,
            raw_size: read_u32(image, base + 16)?,
            raw_pointer: read_u32(image, base + 20)?,
        });
    }
    Ok(sections)
}

fn label_rva(labels: &[(String, usize)], sections: &[Section], name: &str) -> Result<u32, String> {
    let offset = labels
        .iter()
        .rev()
        .find(|(label, _)| label == name)
        .map(|(_, offset)| *offset)
        .ok_or_else(|| format!("undefined label {name}"))?;
    file_rva(sections, offset)
}

fn relocation(
    labels: &[(String, usize)],
    sections: &[Section],
    name: &str,
    at: usize,
    width: i64,
) -> Result<u32, String> {
    let target = i64::from(label_rva(labels, sections, name)?);
    let site = i64::from(file_rva(sections, at)?);
    let displacement = target - (site + width);
    if width == 1 && !(-128..=127).contains(&displacement) {
        return Err(format!("rel8 to {name} out of range: {displacement}"));
    }
    Ok(displacement as u32)
}

fn file_rva(sections: &[Section], offset: usize) -> Result<u32, String> {
    let offset = offset as u64;
    for section in sections {
        let start = u64::from(section.raw_pointer);
        let end = start + u64::from(section.raw_size);
        if start <= offset && offset < end {
            let rva = u64::from(section.virtual_address) + (offset - start);
            return u32::try_from(rva).map_err(|_| format!("RVA {rva:#x} exceeds 32 bits"));
        }
    }
    Err(format!(
        "offset {offset:#x} not inside any section raw data"
    ))
}

fn read_u16(image: &[u8], offset: usize) -> Result<u16, String> {
    let bytes = image
        .get(offset..offset + 2)
        .ok_or_else(|| format!("image ends before {offset:#x}"))?;
    Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
}

fn read_u32(image: &[u8], offset: usize) -> Result<u32, String> {
    let bytes = image
        .get(offset..offset + 4)
        .ok_or_else(|| format!("image ends before {offset:#x}"))?;
    Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}

fn write_at(image: &mut [u8], offset: usize, bytes: &[u8]) -> Result<(), String> {
    let destination = image
        .get_mut(offset..offset + bytes.len())
        .ok_or_else(|| format!("relocation at {offset:#x} exceeds the image"))?;
    destination.copy_from_slice(bytes);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn listing_resolves_padding_zeros_and_relocations() {
        let listing = "\
4D 5A
z58
40 00 00 00
50 45 00 00
64 86 01 00
00 00 00 00 00 00 00 00
00 00 00 00
20 00
00 00
00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
2E 74 65 78 74 00 00 00
10 00 00 00
00 10 00 00
10 00 00 00
00 02 00 00
00 00 00 00 00 00 00 00 00 00 00 00
.pad 0x200
start:
90
E8 .rel32 start
74 .rel8 start
.rva start
";
        let image = forge_listing(listing).unwrap();
        assert_eq!(image.len(), 0x200 + 1 + 5 + 2 + 4);
        assert_eq!(&image[0..2], b"MZ");
        assert_eq!(image[0x3C..0x40], [0x40, 0, 0, 0]);
        assert_eq!(image[0x42], 0, "the z58 run is zero");
        assert_eq!(image[0x200], 0x90);
        assert_eq!(image[0x201], 0xE8);
        // rel32 at file 0x202; its end is 0x206, so the displacement to start is -6.
        assert_eq!(&image[0x202..0x206], &(-6i32).to_le_bytes());
        assert_eq!(image[0x206], 0x74);
        // rel8 at file 0x207; its end is 0x208, so the displacement to start is -8.
        assert_eq!(image[0x207], (-8i8) as u8);
        assert_eq!(&image[0x208..0x20C], &0x1000u32.to_le_bytes());
    }

    #[test]
    fn rel8_out_of_range_and_unknown_tokens_refuse() {
        let listing = "\
4D 5A z58 40 00 00 00
50 45 00 00 64 86 01 00 00 00 00 00 00 00 00 00 00 00 00 00 20 00 00 00
00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
2E 74 65 78 74 00 00 00
00 01 00 00
00 10 00 00
00 01 00 00
00 02 00 00
00 00 00 00 00 00 00 00 00 00 00 00
.pad 0x200
90
far:
.pad 0x280
EB .rel8 far
";
        let error = forge_listing(listing).unwrap_err();
        assert!(error.contains("rel8 to far out of range"), "{error}");
        let error = forge_listing("nope").unwrap_err();
        assert!(error.contains("unrecognized token"), "{error}");
    }

    #[test]
    fn windows_listing_reproduces_the_committed_seed() {
        let root = crate::repo_root().expect("test runs in the Omega workspace");
        let listing = root.join("bootstrap/interpreted/0_alpha/alpha_x64_windows.hex");
        let committed = root.join("bootstrap/interpreted/0_alpha/alpha_x64_windows.exe");
        let image = forge_file(&listing).unwrap();
        let expected = fs::read(&committed).unwrap();
        assert_eq!(image, expected);
    }
}
