//! Bit-exact port of CPython's `random.Random` (MT19937 + the seeding / helper algorithms the game
//! relies on). Seeded generators must reproduce the same star fields, stripes and daily missions.

use sha2::{Digest, Sha512};

const N: usize = 624;
const M: usize = 397;

#[derive(Clone)]
pub struct PyRandom {
    mt: [u32; N],
    mti: usize,
}

impl PyRandom {
    fn init_genrand(&mut self, s: u32) {
        self.mt[0] = s;
        for i in 1..N {
            self.mt[i] = 1812433253u32.wrapping_mul(self.mt[i - 1] ^ (self.mt[i - 1] >> 30)).wrapping_add(i as u32);
        }
        self.mti = N;
    }

    fn init_by_array(&mut self, key: &[u32]) {
        self.init_genrand(19650218);
        let mut i = 1usize;
        let mut j = 0usize;
        let key_length = key.len();
        let mut k = if N > key_length { N } else { key_length };
        while k > 0 {
            self.mt[i] = (self.mt[i] ^ (self.mt[i - 1] ^ (self.mt[i - 1] >> 30)).wrapping_mul(1664525))
                .wrapping_add(key[j])
                .wrapping_add(j as u32);
            i += 1;
            j += 1;
            if i >= N {
                self.mt[0] = self.mt[N - 1];
                i = 1;
            }
            if j >= key_length {
                j = 0;
            }
            k -= 1;
        }
        k = N - 1;
        while k > 0 {
            self.mt[i] = (self.mt[i] ^ (self.mt[i - 1] ^ (self.mt[i - 1] >> 30)).wrapping_mul(1566083941))
                .wrapping_sub(i as u32);
            i += 1;
            if i >= N {
                self.mt[0] = self.mt[N - 1];
                i = 1;
            }
            k -= 1;
        }
        self.mt[0] = 0x8000_0000;
    }

    fn from_key(key: &[u32]) -> PyRandom {
        let mut r = PyRandom { mt: [0; N], mti: N + 1 };
        r.init_by_array(if key.is_empty() { &[0] } else { key });
        r
    }

    /// random.Random(n) for a non-negative integer seed.
    pub fn from_int(n: u64) -> PyRandom {
        let mut key = vec![(n & 0xFFFF_FFFF) as u32];
        if n >> 32 != 0 {
            key.push((n >> 32) as u32);
        }
        PyRandom::from_key(&key)
    }

    /// random.Random("some string") (seed version 2: bytes + sha512(bytes) as a big integer).
    pub fn from_str(s: &str) -> PyRandom {
        let mut bytes = s.as_bytes().to_vec();
        let digest = Sha512::digest(s.as_bytes());
        bytes.extend_from_slice(&digest);
        // big-endian integer -> little-endian 32-bit words, most-significant zero words dropped
        let mut le: Vec<u8> = bytes.iter().rev().cloned().collect();
        while le.len() % 4 != 0 {
            le.push(0);
        }
        let mut key: Vec<u32> = le.chunks(4).map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect();
        while key.len() > 1 && *key.last().unwrap() == 0 {
            key.pop();
        }
        PyRandom::from_key(&key)
    }

    /// A generator seeded from the OS (like the module-level `random`).
    pub fn from_entropy() -> PyRandom {
        use std::io::Read;
        let mut seed = [0u8; 32];
        let ok = std::fs::File::open("/dev/urandom").and_then(|mut f| f.read_exact(&mut seed)).is_ok();
        if !ok {
            let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
            seed[..16].copy_from_slice(&t.as_nanos().to_le_bytes());
        }
        let key: Vec<u32> = seed.chunks(4).map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect();
        PyRandom::from_key(&key)
    }

    pub fn genrand_u32(&mut self) -> u32 {
        const MAG01: [u32; 2] = [0, 0x9908_b0df];
        if self.mti >= N {
            let mut kk = 0;
            while kk < N - M {
                let y = (self.mt[kk] & 0x8000_0000) | (self.mt[kk + 1] & 0x7fff_ffff);
                self.mt[kk] = self.mt[kk + M] ^ (y >> 1) ^ MAG01[(y & 1) as usize];
                kk += 1;
            }
            while kk < N - 1 {
                let y = (self.mt[kk] & 0x8000_0000) | (self.mt[kk + 1] & 0x7fff_ffff);
                self.mt[kk] = self.mt[kk + M - N] ^ (y >> 1) ^ MAG01[(y & 1) as usize];
                kk += 1;
            }
            let y = (self.mt[N - 1] & 0x8000_0000) | (self.mt[0] & 0x7fff_ffff);
            self.mt[N - 1] = self.mt[M - 1] ^ (y >> 1) ^ MAG01[(y & 1) as usize];
            self.mti = 0;
        }
        let mut y = self.mt[self.mti];
        self.mti += 1;
        y ^= y >> 11;
        y ^= (y << 7) & 0x9d2c_5680;
        y ^= (y << 15) & 0xefc6_0000;
        y ^= y >> 18;
        y
    }

    /// random.random()
    pub fn random(&mut self) -> f64 {
        let a = (self.genrand_u32() >> 5) as f64;
        let b = (self.genrand_u32() >> 6) as f64;
        (a * 67108864.0 + b) * (1.0 / 9007199254740992.0)
    }

    /// random.getrandbits(k) for k <= 32
    pub fn getrandbits(&mut self, k: u32) -> u32 {
        if k == 0 {
            return 0;
        }
        self.genrand_u32() >> (32 - k)
    }

    /// random._randbelow(n) (n >= 1, n < 2^32)
    pub fn randbelow(&mut self, n: u64) -> u64 {
        if n == 0 {
            return 0;
        }
        let k = 64 - n.leading_zeros();
        if k <= 32 {
            let mut r = self.getrandbits(k) as u64;
            while r >= n {
                r = self.getrandbits(k) as u64;
            }
            r
        } else {
            // getrandbits for k in (32, 64]: little-endian words, the last one shifted
            loop {
                let lo = self.genrand_u32() as u64;
                let hi = (self.genrand_u32() >> (64 - k)) as u64;
                let r = lo | (hi << 32);
                if r < n {
                    return r;
                }
            }
        }
    }

    /// random.randrange(n)
    pub fn randrange(&mut self, n: i64) -> i64 {
        self.randbelow(n as u64) as i64
    }

    /// random.randint(a, b)
    pub fn randint(&mut self, a: i64, b: i64) -> i64 {
        a + self.randbelow((b - a + 1) as u64) as i64
    }

    /// random.uniform(a, b)
    pub fn uniform(&mut self, a: f64, b: f64) -> f64 {
        a + (b - a) * self.random()
    }

    /// random.choice(seq) -> index
    pub fn choice_index(&mut self, len: usize) -> usize {
        self.randbelow(len as u64) as usize
    }

    /// random.sample(population, k) for small populations (the list-pool branch) -> indices
    pub fn sample_indices(&mut self, n: usize, k: usize) -> Vec<usize> {
        let mut setsize = 21usize;
        if k > 5 {
            let e = ((k * 3) as f64).ln() / 4f64.ln();
            setsize += 4usize.pow(e.ceil() as u32);
        }
        let mut result = vec![0usize; k];
        if n <= setsize {
            let mut pool: Vec<usize> = (0..n).collect();
            for (i, slot) in result.iter_mut().enumerate() {
                let j = self.randbelow((n - i) as u64) as usize;
                *slot = pool[j];
                pool[j] = pool[n - i - 1];
            }
        } else {
            let mut selected = std::collections::HashSet::new();
            for slot in result.iter_mut() {
                let mut j = self.randbelow(n as u64) as usize;
                while selected.contains(&j) {
                    j = self.randbelow(n as u64) as usize;
                }
                selected.insert(j);
                *slot = j;
            }
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn dump_rng() {
        let mut out = String::new();
        let mut r = PyRandom::from_int(1789980865);
        for _ in 0..5 {
            out.push_str(&format!("{} {} {:.17}\n", r.randrange(1422), r.randrange(800), r.uniform(0.12, 0.42)));
        }
        let mut r = PyRandom::from_str("2026-09-23:slot1");
        out.push_str(&format!("{:?}\n", r.sample_indices(8, 3)));
        let mut r = PyRandom::from_int(7);
        out.push_str(&format!("{} {:.17}\n", r.choice_index(3), r.random()));
        std::fs::write(std::env::var("PYRAND_OUT").unwrap_or("/tmp/pyrand_rs.txt".into()), out).unwrap();
    }
}
