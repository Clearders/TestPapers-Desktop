//! Small, dependency-free SHA-256 implementation used for archive and sidecar verification.

use std::fmt;
use std::fs::File;
use std::io::{self, Read};
use std::path::Path;

const INITIAL_STATE: [u32; 8] = [
    0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19,
];

const ROUND_CONSTANTS: [u32; 64] = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
    0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
    0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
    0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
    0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
    0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
    0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
];

#[derive(Clone, Copy, Eq, Hash, PartialEq)]
pub(crate) struct Sha256Digest([u8; 32]);

impl Sha256Digest {
    pub(crate) fn from_hex(value: &str) -> Result<Self, DigestParseError> {
        if value.len() != 64 {
            return Err(DigestParseError);
        }
        let mut bytes = [0_u8; 32];
        for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
            bytes[index] = (hex_nibble(pair[0]).ok_or(DigestParseError)? << 4)
                | hex_nibble(pair[1]).ok_or(DigestParseError)?;
        }
        Ok(Self(bytes))
    }

    pub(crate) fn to_hex(self) -> String {
        let mut output = String::with_capacity(64);
        for byte in self.0 {
            use fmt::Write;
            let _ = write!(&mut output, "{byte:02x}");
        }
        output
    }

    pub(crate) fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl fmt::Debug for Sha256Digest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.to_hex())
    }
}

impl fmt::Display for Sha256Digest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.to_hex())
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) struct DigestParseError;

impl fmt::Display for DigestParseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("expected a 64-character lowercase SHA-256 digest")
    }
}

pub(crate) fn sha256(bytes: &[u8]) -> Sha256Digest {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hasher.finish()
}

pub(crate) fn sha256_reader(mut reader: impl Read) -> io::Result<Sha256Digest> {
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = reader.read(&mut buffer)?;
        if read == 0 {
            return Ok(hasher.finish());
        }
        hasher.update(&buffer[..read]);
    }
}

pub(crate) fn sha256_file(path: &Path) -> io::Result<Sha256Digest> {
    sha256_reader(File::open(path)?)
}

struct Sha256 {
    state: [u32; 8],
    block: [u8; 64],
    block_len: usize,
    total_len: u64,
}

impl Sha256 {
    fn new() -> Self {
        Self {
            state: INITIAL_STATE,
            block: [0; 64],
            block_len: 0,
            total_len: 0,
        }
    }

    fn update(&mut self, mut input: &[u8]) {
        self.total_len = self.total_len.wrapping_add(input.len() as u64);
        if self.block_len > 0 {
            let copied = (64 - self.block_len).min(input.len());
            self.block[self.block_len..self.block_len + copied].copy_from_slice(&input[..copied]);
            self.block_len += copied;
            input = &input[copied..];
            if self.block_len == 64 {
                compress(&mut self.state, &self.block);
                self.block_len = 0;
            }
        }
        while input.len() >= 64 {
            let block: &[u8; 64] = input[..64].try_into().expect("exact SHA-256 block");
            compress(&mut self.state, block);
            input = &input[64..];
        }
        self.block[..input.len()].copy_from_slice(input);
        self.block_len = input.len();
    }

    fn finish(mut self) -> Sha256Digest {
        let bit_len = self.total_len.wrapping_mul(8);
        self.block[self.block_len] = 0x80;
        self.block_len += 1;
        if self.block_len > 56 {
            self.block[self.block_len..].fill(0);
            compress(&mut self.state, &self.block);
            self.block = [0; 64];
        } else {
            self.block[self.block_len..56].fill(0);
        }
        self.block[56..].copy_from_slice(&bit_len.to_be_bytes());
        compress(&mut self.state, &self.block);

        let mut digest = [0_u8; 32];
        for (chunk, value) in digest.chunks_exact_mut(4).zip(self.state) {
            chunk.copy_from_slice(&value.to_be_bytes());
        }
        Sha256Digest(digest)
    }
}

fn compress(state: &mut [u32; 8], block: &[u8; 64]) {
    let mut schedule = [0_u32; 64];
    for (index, chunk) in block.chunks_exact(4).enumerate() {
        schedule[index] = u32::from_be_bytes(chunk.try_into().expect("four-byte word"));
    }
    for index in 16..64 {
        let s0 = schedule[index - 15].rotate_right(7)
            ^ schedule[index - 15].rotate_right(18)
            ^ (schedule[index - 15] >> 3);
        let s1 = schedule[index - 2].rotate_right(17)
            ^ schedule[index - 2].rotate_right(19)
            ^ (schedule[index - 2] >> 10);
        schedule[index] = schedule[index - 16]
            .wrapping_add(s0)
            .wrapping_add(schedule[index - 7])
            .wrapping_add(s1);
    }

    let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h] = *state;
    for index in 0..64 {
        let upper_e = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
        let choose = (e & f) ^ ((!e) & g);
        let first = h
            .wrapping_add(upper_e)
            .wrapping_add(choose)
            .wrapping_add(ROUND_CONSTANTS[index])
            .wrapping_add(schedule[index]);
        let upper_a = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
        let majority = (a & b) ^ (a & c) ^ (b & c);
        let second = upper_a.wrapping_add(majority);
        h = g;
        g = f;
        f = e;
        e = d.wrapping_add(first);
        d = c;
        c = b;
        b = a;
        a = first.wrapping_add(second);
    }
    state[0] = state[0].wrapping_add(a);
    state[1] = state[1].wrapping_add(b);
    state[2] = state[2].wrapping_add(c);
    state[3] = state[3].wrapping_add(d);
    state[4] = state[4].wrapping_add(e);
    state[5] = state[5].wrapping_add(f);
    state[6] = state[6].wrapping_add(g);
    state[7] = state[7].wrapping_add(h);
}

fn hex_nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_published_sha256_vectors() {
        assert_eq!(
            sha256(b"").to_hex(),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            sha256(b"abc").to_hex(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn digest_parser_rejects_uppercase_and_wrong_length() {
        assert!(Sha256Digest::from_hex(&"a".repeat(64)).is_ok());
        assert!(Sha256Digest::from_hex(&"A".repeat(64)).is_err());
        assert!(Sha256Digest::from_hex("abc").is_err());
    }
}
