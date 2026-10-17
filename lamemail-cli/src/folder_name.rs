use std::path::PathBuf;

use anyhow::{Result, bail};

pub fn decode_utf7(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    let mut rest = name;
    while let Some(start) = rest.find('&') {
        out.push_str(&rest[..start]);
        let after = &rest[start + 1..];
        let Some(end) = after.find('-') else {
            out.push_str(&rest[start..]);
            return out;
        };
        let encoded = &after[..end];
        if encoded.is_empty() {
            out.push('&');
        } else {
            match decode_shifted(encoded) {
                Some(text) => out.push_str(&text),
                None => out.push_str(&rest[start..start + 2 + end]),
            }
        }
        rest = &after[end + 1..];
    }
    out.push_str(rest);
    out
}

fn decode_shifted(encoded: &str) -> Option<String> {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+,";
    let mut bytes = Vec::with_capacity(encoded.len() * 3 / 4);
    let mut acc: u32 = 0;
    let mut bits = 0;
    for c in encoded.bytes() {
        let value = ALPHABET.iter().position(|&a| a == c)? as u32;
        acc = (acc << 6) | value;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            bytes.push((acc >> bits) as u8);
            acc &= (1 << bits) - 1;
        }
    }
    if bytes.len() % 2 != 0 {
        return None;
    }
    let units: Vec<u16> = bytes
        .chunks(2)
        .map(|pair| u16::from_be_bytes([pair[0], pair[1]]))
        .collect();
    String::from_utf16(&units).ok()
}

pub fn local_path(imap_name: &str, delimiter: Option<&str>) -> Result<PathBuf> {
    Ok(PathBuf::from(shown(imap_name, delimiter)?.to_lowercase()))
}

pub fn shown(imap_name: &str, delimiter: Option<&str>) -> Result<String> {
    let decoded = decode_utf7(imap_name);
    let segments: Vec<&str> = match delimiter.filter(|d| !d.is_empty()) {
        Some(delimiter) => decoded.split(delimiter).collect(),
        None => vec![decoded.as_str()],
    };
    let mut levels = Vec::with_capacity(segments.len());
    for segment in segments {
        let segment: String = segment
            .chars()
            .map(|c| if c == '/' || c == '\0' { '_' } else { c })
            .collect();
        if segment.is_empty() || segment == "." || segment == ".." {
            bail!("folder name `{imap_name}` cannot be stored as a path");
        }
        levels.push(segment);
    }
    Ok(levels.join("/"))
}

pub fn normalize(name: &str) -> String {
    name.trim().trim_matches('/').to_lowercase()
}

pub fn is_year(segment: &str) -> bool {
    segment.len() == 4 && segment.bytes().all(|byte| byte.is_ascii_digit())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_normalized() {
        assert_eq!(normalize(" /INBOX/Archive/ "), "inbox/archive");
    }

    #[test]
    fn a_year_is_four_digits() {
        assert!(is_year("2026"));
        assert!(!is_year("226") && !is_year("2026-") && !is_year("20x6"));
    }

    #[test]
    fn decodes_modified_utf7() {
        assert_eq!(decode_utf7("INBOX"), "INBOX");
        assert_eq!(decode_utf7("Entw&APw-rfe"), "Entwürfe");
        assert_eq!(decode_utf7("&AOQ-"), "ä");
        assert_eq!(decode_utf7("Tom &- Jerry"), "Tom & Jerry");
        assert_eq!(decode_utf7("&ZeVnLIqe-"), "日本語");
        assert_eq!(decode_utf7("broken &AOQ"), "broken &AOQ");
    }

    #[test]
    fn maps_folders_to_lowercase_paths() {
        assert_eq!(
            local_path("INBOX", Some("/")).unwrap(),
            PathBuf::from("inbox")
        );
        assert_eq!(
            local_path("INBOX.Archive.2025", Some(".")).unwrap(),
            PathBuf::from("inbox/archive/2025")
        );
        assert_eq!(
            local_path("Entw&APw-rfe", Some("/")).unwrap(),
            PathBuf::from("entwürfe")
        );
        assert_eq!(
            local_path("Flat/Name", None).unwrap(),
            PathBuf::from("flat_name")
        );
        assert!(local_path("..", Some("/")).is_err());
        assert!(local_path("a//b", Some("/")).is_err());
    }

    #[test]
    fn a_folder_is_shown_by_its_decoded_name_with_slashes_between_its_levels() {
        assert_eq!(
            shown("INBOX.Kunden.M&APw-ller &- S&APY-hne", Some(".")).unwrap(),
            "INBOX/Kunden/Müller & Söhne"
        );
        assert_eq!(shown("Entw&APw-rfe", Some("/")).unwrap(), "Entwürfe");
        assert_eq!(shown("Flat/Name", None).unwrap(), "Flat_Name");
        assert!(shown("a//b", Some("/")).is_err());
    }
}
