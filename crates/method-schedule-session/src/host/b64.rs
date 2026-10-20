//! A file's bytes in the command contract's JSON: one base64 string (standard alphabet,
//! padded), which a browser makes and reads with `btoa` and `atob`. Method Budget's host
//! reads them the same way, so a partner hands both apps files alike.

use serde::{Deserialize, Deserializer, Serializer};

const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// `bytes` as base64.
pub fn encode(bytes: &[u8]) -> String {
    crate::elements::base64(bytes)
}

/// The bytes base64 `text` stands for. Whitespace is skipped; anything else outside the
/// alphabet, or padding in the wrong place, is refused.
pub fn decode(text: &str) -> Result<Vec<u8>, String> {
    let bad = || "the file's bytes aren't base64".to_string();
    let digits: Vec<u8> = text.bytes().filter(|c| !c.is_ascii_whitespace()).collect();
    if !digits.len().is_multiple_of(4) {
        return Err(bad());
    }
    let mut out = Vec::with_capacity(digits.len() / 4 * 3);
    let quads = digits.len() / 4;
    for (q, quad) in digits.chunks(4).enumerate() {
        let pad = quad.iter().rev().take_while(|&&c| c == b'=').count();
        if pad > 2 || (pad > 0 && q + 1 != quads) {
            return Err(bad());
        }
        let mut n = 0u32;
        for &c in &quad[..4 - pad] {
            let v = ALPHABET.iter().position(|&a| a == c).ok_or_else(bad)?;
            n = (n << 6) | v as u32;
        }
        n <<= 6 * pad as u32;
        let three = [(n >> 16) as u8, (n >> 8) as u8, n as u8];
        out.extend_from_slice(&three[..3 - pad]);
    }
    Ok(out)
}

/// For `#[serde(with = "b64")]` on a `Vec<u8>`.
pub fn serialize<S: Serializer>(bytes: &[u8], s: S) -> Result<S::Ok, S::Error> {
    s.serialize_str(&encode(bytes))
}

/// For `#[serde(with = "b64")]` on a `Vec<u8>`.
pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<u8>, D::Error> {
    let text = String::deserialize(d)?;
    decode(&text).map_err(serde::de::Error::custom)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_every_length_and_refuses_what_isnt_base64() {
        for n in 0..10u8 {
            let bytes: Vec<u8> = (0..n).map(|i| i.wrapping_mul(97)).collect();
            assert_eq!(decode(&encode(&bytes)).unwrap(), bytes);
        }
        assert_eq!(decode("TWFu").unwrap(), b"Man");
        assert_eq!(decode("TW E=\n").unwrap(), b"Ma");
        assert!(decode("TWE").is_err());
        assert!(decode("T=Wu").is_err());
        assert!(decode("TW==TWFu").is_err());
        assert!(decode("TW*u").is_err());
    }
}
