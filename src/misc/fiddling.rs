use anyhow::{bail, Result};
use regex::Regex;
use std::str;

// Encodes raw bytes into a hex string (upper) case hex valid)
pub fn enhex(bytes: &[u8]) -> String {
    // Encodes data as hex string using uppercase characters.
    hex::encode_upper(bytes)
}

// Decodes a hex string into raw bytes (upper and lower case hex valid)
pub fn unhex(hex_string: &str) -> Result<Vec<u8>> {
    // strip whitespaces
    let mut str_striped: String = hex_string.chars().filter(|c| !c.is_whitespace()).collect();

    // padding
    if str_striped.len() % 2 != 0 {
        str_striped = format!("{}{}", String::from("0"), str_striped);
    }

    Ok(hex::decode(&str_striped).unwrap())
}

// Encodes raw bytes into a base64 string
pub fn base64enc(bytes: &[u8]) -> String {
    base64::encode(bytes)
}

// Decodes a base64 string into raw bytes
pub fn base64dec(bytes: &str) -> Result<Vec<u8>> {
    Ok(base64::decode(bytes).unwrap())
}

// Encodes utf8 string into raw bytes
pub fn to_bytes(string: &str) -> Vec<u8> {
    string.as_bytes().to_vec()
}

// Decodes raw bytes into a utf8 string
pub fn to_str(bytes: &[u8]) -> Result<String> {
    Ok(str::from_utf8(&bytes).unwrap().to_string())
}

// url-encodes a string.
pub fn urlencode(url: &str) -> String {
    let mut url_encoded = "".to_owned();
    for c in url.to_string().chars() {
        let char_enc = format!("%{:x}", c as u32);
        url_encoded.push_str(&char_enc);
    }
    url_encoded
}

// url-decodes a string.
pub fn urldecode(url: &str) -> Result<String> {
    let mut url_decoded = "".to_string();
    let url = url.to_string();
    let url_chars: Vec<char> = url.chars().collect();
    let mut n = 0;
    while n < url.len() {
        if url_chars[n] != '%' {
            url_decoded.push(url_chars[n]);
            n += 1;
        } else {
            let cur = &url[n + 1..n + 3];
            let check = Regex::new("[0-9a-fA-F]{2}").unwrap();
            if let Some(cpts) = check.captures(cur) {
                let numb = cpts.get(0).unwrap().as_str();
                let e: u8 = u8::from_str_radix(numb, 16).unwrap();
                url_decoded.push(e as char);
                n += 3
            } else {
                bail!("Invalid input to urldecode");
            }
        }
    }

    Ok(url_decoded)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_enhex() {
        assert_eq!(
            enhex(&vec![
                72, 101, 108, 108, 111, 44, 32, 87, 111, 114, 108, 100, 33
            ]),
            "48656C6C6F2C20576F726C6421"
        );
    }

    #[test]
    fn test_unhex_standard() {
        assert_eq!(
            unhex("48656C6C6F2C20576F726C6421").unwrap(),
            vec![72, 101, 108, 108, 111, 44, 32, 87, 111, 114, 108, 100, 33]
        );
    }

    #[test]
    fn test_unhex_whitespaces() {
        assert_eq!(
            unhex("   48656C6C6F2C20576F726C6421 ").unwrap(),
            vec![72, 101, 108, 108, 111, 44, 32, 87, 111, 114, 108, 100, 33]
        );
    }

    #[test]
    fn test_unhex_odd() {
        assert_eq!(
            unhex("148656C6C6F2C20576F726C6421 ").unwrap(),
            vec![1, 72, 101, 108, 108, 111, 44, 32, 87, 111, 114, 108, 100, 33]
        );
    }

    #[test]
    fn test_base64_encoding() {
        assert_eq!(base64enc(b"testing"), "dGVzdGluZw==");
    }

    #[test]
    fn test_base64_decoding() {
        assert_eq!(base64dec("dGVzdGluZw==").unwrap(), b"testing");
    }

    #[test]
    fn test_urlencode() {
        assert_eq!(
            urlencode("https://bochumoxid.com"),
            "%68%74%74%70%73%3a%2f%2f%62%6f%63%68%75%6d%6f%78%69%64%2e%63%6f%6d"
        );
    }

    #[test]
    fn test_urldecode() {
        assert_eq!(
            urldecode("%68%74%74%70%73%3a%2f%2f%62%6f%63%68%75%6d%6f%78%69%64%2e%63%6f%6d")
                .unwrap(),
            "https://bochumoxid.com"
        );
    }
}
