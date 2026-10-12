//! MTProto transport adaptation from Flowseal tg-ws-proxy (MIT), revision
//! caa949bee0873d2b95dfb4fbeb1b7868b0ee3843. AES uses system OpenSSL, not custom crypto.
use openssl::{
    hash::{MessageDigest, hash},
    rand::rand_bytes,
    symm::{Cipher, Crypter, Mode},
};
use std::io;
pub const MAX_PACKET: usize = 16 * 1024 * 1024;
pub const ABRIDGED: [u8; 4] = [0xef; 4];
pub const INTERMEDIATE: [u8; 4] = [0xee; 4];
pub const SECURE: [u8; 4] = [0xdd; 4];
fn error(_: openssl::error::ErrorStack) -> io::Error {
    io::Error::other("transport cipher failed")
}
pub struct Ctr(Crypter);
impl Ctr {
    pub fn new(key: &[u8], iv: &[u8]) -> io::Result<Self> {
        if key.len() != 32 || iv.len() != 16 {
            return Err(io::Error::other("invalid cipher key length"));
        }
        let mut cipher =
            Crypter::new(Cipher::aes_256_ctr(), Mode::Encrypt, key, Some(iv)).map_err(error)?;
        cipher.pad(false);
        Ok(Self(cipher))
    }
    pub fn update(&mut self, data: &[u8]) -> io::Result<Vec<u8>> {
        if data.len() > MAX_PACKET {
            return Err(io::Error::other("cipher input exceeds limit"));
        }
        let mut out = vec![0; data.len() + 16];
        let n = self.0.update(data, &mut out).map_err(error)?;
        out.truncate(n);
        Ok(out)
    }
}
fn keyed(seed: &[u8], secret: &[u8]) -> io::Result<Vec<u8>> {
    let mut bytes = Vec::with_capacity(seed.len() + secret.len());
    bytes.extend_from_slice(seed);
    bytes.extend_from_slice(secret);
    Ok(hash(MessageDigest::sha256(), &bytes)
        .map_err(error)?
        .to_vec())
}
pub fn client_handshake(handshake: &[u8], secret: &[u8]) -> io::Result<(i16, [u8; 4])> {
    if handshake.len() != 64 || secret.len() != 16 {
        return Err(io::Error::other("invalid handshake size"));
    }
    let plain =
        Ctr::new(&keyed(&handshake[8..40], secret)?, &handshake[40..56])?.update(handshake)?;
    let tag: [u8; 4] = plain[56..60].try_into().expect("fixed handshake slice");
    let dc = i16::from_le_bytes(plain[60..62].try_into().expect("fixed handshake slice"));
    if ![ABRIDGED, INTERMEDIATE, SECURE].contains(&tag)
        || ![1, 2, 3, 4, 5, 203].contains(&dc.unsigned_abs())
    {
        return Err(io::Error::other("invalid MTProto handshake"));
    }
    Ok((dc, tag))
}
pub fn relay_with_random(
    mut random: [u8; 64],
    tail: [u8; 2],
    tag: [u8; 4],
    dc: i16,
) -> io::Result<[u8; 64]> {
    let cipher = Ctr::new(&random[8..40], &random[40..56])?.update(&random)?;
    let mut plain = [0; 8];
    plain[..4].copy_from_slice(&tag);
    plain[4..6].copy_from_slice(&dc.to_le_bytes());
    plain[6..].copy_from_slice(&tail);
    for i in 0..8 {
        random[56 + i] ^= plain[i] ^ cipher[56 + i];
    }
    Ok(random)
}
pub fn relay_handshake(tag: [u8; 4], dc: i16) -> io::Result<[u8; 64]> {
    // CSPRNG rejections have a finite bound; no caller-controlled retry loop.
    for _ in 0..32 {
        let mut random = [0; 64];
        rand_bytes(&mut random).map_err(error)?;
        let reserved = [
            *b"HEAD",
            *b"POST",
            *b"GET ",
            INTERMEDIATE,
            SECURE,
            [0x16, 0x03, 0x01, 0x02],
        ];
        if random[0] == 0xef
            || reserved.iter().any(|prefix| random[..4] == *prefix)
            || random[4..8] == [0; 4]
        {
            continue;
        }
        let mut tail = [0; 2];
        rand_bytes(&mut tail).map_err(error)?;
        return relay_with_random(random, tail, tag, dc);
    }
    Err(io::Error::other("transport random admission failed"))
}
pub struct Crypto {
    pub client_dec: Ctr,
    pub client_enc: Ctr,
    pub telegram_enc: Ctr,
    pub telegram_dec: Ctr,
}
impl Crypto {
    pub fn new(handshake: &[u8; 64], secret: &[u8; 16], relay: &[u8; 64]) -> io::Result<Self> {
        let reversed: Vec<_> = handshake[8..56].iter().copied().rev().collect();
        let reverse_relay: Vec<_> = relay[8..56].iter().copied().rev().collect();
        let mut owner = Self {
            client_dec: Ctr::new(&keyed(&handshake[8..40], secret)?, &handshake[40..56])?,
            client_enc: Ctr::new(&keyed(&reversed[..32], secret)?, &reversed[32..])?,
            telegram_enc: Ctr::new(&relay[8..40], &relay[40..56])?,
            telegram_dec: Ctr::new(&reverse_relay[..32], &reverse_relay[32..])?,
        };
        owner.client_dec.update(&[0; 64])?;
        owner.telegram_enc.update(&[0; 64])?;
        Ok(owner)
    }
}
/// Pure development projection for frozen transport vectors; never opens I/O.
pub fn project(value: &serde_json::Value) -> serde_json::Value {
    use serde_json::json;
    let decode = |key: &str| -> io::Result<Vec<u8>> {
        let text = value[key]
            .as_str()
            .ok_or_else(|| io::Error::other("invalid fixture"))?;
        if text.len() > 2 * MAX_PACKET || text.len() % 2 != 0 {
            return Err(io::Error::other("invalid fixture"));
        }
        text.as_bytes()
            .as_chunks::<2>()
            .0
            .iter()
            .map(|p| {
                u8::from_str_radix(
                    std::str::from_utf8(p).map_err(|_| io::Error::other("invalid fixture"))?,
                    16,
                )
                .map_err(|_| io::Error::other("invalid fixture"))
            })
            .collect()
    };
    let hex = |bytes: &[u8]| bytes.iter().map(|b| format!("{b:02x}")).collect::<String>();
    let operation = || -> io::Result<serde_json::Value> {
        Ok(match value["kind"].as_str() {
            Some("ctr") => json!(hex(
                &Ctr::new(&decode("key")?, &decode("iv")?)?.update(&decode("data")?)?
            )),
            Some("client") => {
                let (dc, tag) = client_handshake(&decode("handshake")?, &decode("secret")?)?;
                json!({"dc":dc,"tag":hex(&tag)})
            }
            Some("relay") => {
                let random = decode("random")?
                    .try_into()
                    .map_err(|_| io::Error::other("invalid fixture"))?;
                let tail = decode("tail")?
                    .try_into()
                    .map_err(|_| io::Error::other("invalid fixture"))?;
                let tag = decode("tag")?
                    .try_into()
                    .map_err(|_| io::Error::other("invalid fixture"))?;
                json!(hex(&relay_with_random(
                    random,
                    tail,
                    tag,
                    value["dc"].as_i64().unwrap_or(0) as i16
                )?))
            }
            Some("split") => {
                let relay = decode("relay")?
                    .try_into()
                    .map_err(|_| io::Error::other("invalid fixture"))?;
                let tag = decode("tag")?
                    .try_into()
                    .map_err(|_| io::Error::other("invalid fixture"))?;
                let mut splitter = Splitter::new(&relay, tag)?;
                let data = decode("data")?;
                let mut parts = vec![];
                let step = value["step"].as_u64().unwrap_or(1).clamp(1, 65536) as usize;
                for chunk in data.chunks(step) {
                    parts.extend(splitter.split(chunk)?.into_iter().map(|p| hex(&p)));
                }
                json!(parts)
            }
            _ => return Err(io::Error::other("invalid fixture")),
        })
    };
    operation().unwrap_or_else(|_| json!({"error":true}))
}
pub struct Splitter {
    dec: Ctr,
    tag: [u8; 4],
    cipher: Vec<u8>,
    plain: Vec<u8>,
}
impl Splitter {
    pub fn new(relay: &[u8; 64], tag: [u8; 4]) -> io::Result<Self> {
        let mut dec = Ctr::new(&relay[8..40], &relay[40..56])?;
        dec.update(&[0; 64])?;
        Ok(Self {
            dec,
            tag,
            cipher: Vec::new(),
            plain: Vec::new(),
        })
    }
    pub fn split(&mut self, data: &[u8]) -> io::Result<Vec<Vec<u8>>> {
        if self.cipher.len().saturating_add(data.len()) > MAX_PACKET + 4 {
            return Err(io::Error::other("packet buffer exceeds limit"));
        }
        self.cipher.extend_from_slice(data);
        self.plain.extend(self.dec.update(data)?);
        let mut offset = 0;
        let mut parts = vec![];
        while offset < self.plain.len() {
            let available = self.plain.len() - offset;
            let (header, size) = if self.tag == ABRIDGED {
                let first = self.plain[offset];
                let header = if matches!(first, 0x7f | 0xff) { 4 } else { 1 };
                if available < header {
                    break;
                }
                let count = if header == 4 {
                    u32::from_le_bytes([
                        self.plain[offset + 1],
                        self.plain[offset + 2],
                        self.plain[offset + 3],
                        0,
                    ])
                } else {
                    u32::from(first & 0x7f)
                };
                (header, count as usize * 4)
            } else {
                if available < 4 {
                    break;
                }
                (
                    4,
                    (u32::from_le_bytes(
                        self.plain[offset..offset + 4]
                            .try_into()
                            .expect("checked header"),
                    ) & 0x7fffffff) as usize,
                )
            };
            if size == 0 || size > MAX_PACKET {
                return Err(io::Error::other("invalid packet length"));
            }
            if available < size + header {
                break;
            }
            parts.push(self.cipher[offset..offset + header + size].to_vec());
            offset += header + size;
        }
        if offset > 0 {
            self.cipher.drain(..offset);
            self.plain.drain(..offset);
        }
        Ok(parts)
    }
}
