#![no_std]
//! Deliberately large benchmark fixture for the wasm-size / deploy-cost gap
//! series (issue #417).
//!
//! Size comes from *real code* — a handful of independent, self-contained
//! algorithms — rather than from padding with constants or `include_bytes!`,
//! because padding compresses differently under the release profile's `lto` +
//! `opt-level = "z"` and would misrepresent the size/cost relationship.
//!
//! Three points of materially different compiled size feed the measurement:
//! `host-function-contract` (tiny), `amm-pool-contract` (~25 KB), and this
//! crate. Each exported `run_*` entry point pulls its algorithm into the
//! module so dead-code elimination cannot shrink it back down.

use soroban_sdk::{contract, contractimpl, Bytes, Env, Vec};

#[contract]
pub struct BloatContract;

#[contractimpl]
impl BloatContract {
    /// CRC-32 (IEEE) over `data`, table-free (bitwise) so the polynomial loop
    /// is real instruction volume.
    pub fn run_crc32(_env: Env, data: Bytes) -> u32 {
        let mut crc: u32 = 0xFFFF_FFFF;
        for byte in data.iter() {
            crc ^= byte as u32;
            for _ in 0..8 {
                let mask = (crc & 1).wrapping_neg();
                crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
            }
        }
        !crc
    }

    /// FNV-1a 64-bit hash, folded to `u32`.
    pub fn run_fnv1a(_env: Env, data: Bytes) -> u32 {
        let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
        for byte in data.iter() {
            hash ^= byte as u64;
            hash = hash.wrapping_mul(0x0000_0100_0000_01B3);
        }
        (hash ^ (hash >> 32)) as u32
    }

    /// djb2 string hash.
    pub fn run_djb2(_env: Env, data: Bytes) -> u32 {
        let mut hash: u32 = 5381;
        for byte in data.iter() {
            hash = hash
                .wrapping_shl(5)
                .wrapping_add(hash)
                .wrapping_add(byte as u32);
        }
        hash
    }

    /// Adler-32 checksum.
    pub fn run_adler32(_env: Env, data: Bytes) -> u32 {
        let mut a: u32 = 1;
        let mut b: u32 = 0;
        for byte in data.iter() {
            a = (a + byte as u32) % 65521;
            b = (b + a) % 65521;
        }
        (b << 16) | a
    }

    /// xorshift128 PRNG advanced `steps` times, returning the final word.
    pub fn run_xorshift(_env: Env, seed: u32, steps: u32) -> u32 {
        let mut x = seed | 1;
        let mut y = seed ^ 0x9E37_79B9;
        let mut z = seed.rotate_left(13);
        let mut w = seed.rotate_right(7);
        for _ in 0..steps {
            let t = x ^ (x << 11);
            x = y;
            y = z;
            z = w;
            w = (w ^ (w >> 19)) ^ (t ^ (t >> 8));
        }
        w
    }

    /// Integer square root by Newton's method.
    pub fn run_isqrt_newton(_env: Env, value: u64) -> u64 {
        if value < 2 {
            return value;
        }
        let mut x = value;
        let mut y = value.div_ceil(2);
        while y < x {
            x = y;
            y = (x + value / x) / 2;
        }
        x
    }

    /// Modular exponentiation `base^exp mod modulus`, square-and-multiply.
    pub fn run_modpow(_env: Env, base: u64, exp: u64, modulus: u64) -> u64 {
        if modulus <= 1 {
            return 0;
        }
        let mut result: u128 = 1;
        let mut b = (base % modulus) as u128;
        let m = modulus as u128;
        let mut e = exp;
        while e > 0 {
            if e & 1 == 1 {
                result = (result * b) % m;
            }
            e >>= 1;
            b = (b * b) % m;
        }
        result as u64
    }

    /// GCD via the binary (Stein's) algorithm.
    pub fn run_binary_gcd(_env: Env, a: u64, b: u64) -> u64 {
        let (mut a, mut b) = (a, b);
        if a == 0 {
            return b;
        }
        if b == 0 {
            return a;
        }
        let shift = (a | b).trailing_zeros();
        a >>= a.trailing_zeros();
        loop {
            b >>= b.trailing_zeros();
            if a > b {
                core::mem::swap(&mut a, &mut b);
            }
            b -= a;
            if b == 0 {
                break;
            }
        }
        a << shift
    }

    /// Insertion sort of the first `n` bytes of `data`, returning the median.
    pub fn run_insertion_sort_median(env: Env, data: Bytes) -> u32 {
        let mut buf: Vec<u32> = Vec::new(&env);
        for byte in data.iter() {
            buf.push_back(byte as u32);
        }
        let len = buf.len();
        for i in 1..len {
            let key = buf.get(i).unwrap();
            let mut j = i;
            while j > 0 && buf.get(j - 1).unwrap() > key {
                let prev = buf.get(j - 1).unwrap();
                buf.set(j, prev);
                j -= 1;
            }
            buf.set(j, key);
        }
        if len == 0 {
            0
        } else {
            buf.get(len / 2).unwrap()
        }
    }

    /// Sum of the first `n` Fibonacci numbers, wrapping.
    pub fn run_fib_sum(_env: Env, n: u32) -> u64 {
        let mut a: u64 = 0;
        let mut b: u64 = 1;
        let mut total: u64 = 0;
        for _ in 0..n {
            total = total.wrapping_add(a);
            let next = a.wrapping_add(b);
            a = b;
            b = next;
        }
        total
    }

    /// Collatz stopping time for `start`.
    pub fn run_collatz_steps(_env: Env, start: u64) -> u32 {
        let mut n = start.max(1);
        let mut steps = 0u32;
        while n != 1 {
            n = if n & 1 == 0 {
                n / 2
            } else {
                3u64.wrapping_mul(n).wrapping_add(1)
            };
            steps += 1;
            if steps > 100_000 {
                break;
            }
        }
        steps
    }

    /// Count set bits across `data` (popcount).
    pub fn run_popcount(_env: Env, data: Bytes) -> u32 {
        let mut count = 0u32;
        for byte in data.iter() {
            count += byte.count_ones();
        }
        count
    }

    /// Reverse the bytes of `data` and re-hash with FNV-1a — exercises Bytes
    /// construction plus a second pass.
    pub fn run_reverse_rehash(env: Env, data: Bytes) -> u32 {
        let mut reversed = Bytes::new(&env);
        let len = data.len();
        for i in 0..len {
            reversed.push_back(data.get(len - 1 - i).unwrap());
        }
        Self::run_fnv1a(env, reversed)
    }

    /// Naive matrix multiply of two `size`x`size` matrices filled from `seed`,
    /// returning the trace of the product.
    pub fn run_matmul_trace(_env: Env, seed: u32, size: u32) -> u64 {
        let s = size.min(16) as usize;
        let mut a = [[0u64; 16]; 16];
        let mut b = [[0u64; 16]; 16];
        let mut r = seed as u64 | 1;
        for row in a.iter_mut().take(s) {
            for cell in row.iter_mut().take(s) {
                r = r.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
                *cell = r >> 33;
            }
        }
        for row in b.iter_mut().take(s) {
            for cell in row.iter_mut().take(s) {
                r = r.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
                *cell = r >> 33;
            }
        }
        let mut trace = 0u64;
        for i in 0..s {
            let mut acc = 0u64;
            for k in 0..s {
                acc = acc.wrapping_add(a[i][k].wrapping_mul(b[k][i]));
            }
            trace = trace.wrapping_add(acc);
        }
        trace
    }

    /// CRC-16/CCITT-FALSE over `data`.
    pub fn run_crc16_ccitt(_env: Env, data: Bytes) -> u32 {
        let mut crc: u16 = 0xFFFF;
        for byte in data.iter() {
            crc ^= (byte as u16) << 8;
            for _ in 0..8 {
                crc = if crc & 0x8000 != 0 {
                    (crc << 1) ^ 0x1021
                } else {
                    crc << 1
                };
            }
        }
        crc as u32
    }

    /// MurmurHash3 x86 32-bit over `data`.
    pub fn run_murmur3_32(env: Env, data: Bytes, seed: u32) -> u32 {
        const C1: u32 = 0xcc9e_2d51;
        const C2: u32 = 0x1b87_3593;
        let mut h = seed;
        let len = data.len();
        let blocks = len / 4;
        for i in 0..blocks {
            let base = i * 4;
            let mut k = (data.get(base).unwrap() as u32)
                | ((data.get(base + 1).unwrap() as u32) << 8)
                | ((data.get(base + 2).unwrap() as u32) << 16)
                | ((data.get(base + 3).unwrap() as u32) << 24);
            k = k.wrapping_mul(C1).rotate_left(15).wrapping_mul(C2);
            h ^= k;
            h = h.rotate_left(13).wrapping_mul(5).wrapping_add(0xe654_6b64);
        }
        let mut k1: u32 = 0;
        let tail = blocks * 4;
        let rem = len - tail;
        if rem >= 3 {
            k1 ^= (data.get(tail + 2).unwrap() as u32) << 16;
        }
        if rem >= 2 {
            k1 ^= (data.get(tail + 1).unwrap() as u32) << 8;
        }
        if rem >= 1 {
            k1 ^= data.get(tail).unwrap() as u32;
            k1 = k1.wrapping_mul(C1).rotate_left(15).wrapping_mul(C2);
            h ^= k1;
        }
        h ^= len;
        h ^= h >> 16;
        h = h.wrapping_mul(0x85eb_ca6b);
        h ^= h >> 13;
        h = h.wrapping_mul(0xc2b2_ae35);
        h ^= h >> 16;
        let _ = env;
        h
    }

    /// Run-length-encode `data` and return the encoded length.
    pub fn run_rle_length(_env: Env, data: Bytes) -> u32 {
        let len = data.len();
        if len == 0 {
            return 0;
        }
        let mut out = 0u32;
        let mut run = 1u32;
        let mut prev = data.get(0).unwrap();
        for i in 1..len {
            let cur = data.get(i).unwrap();
            if cur == prev && run < 255 {
                run += 1;
            } else {
                out += 2;
                run = 1;
                prev = cur;
            }
        }
        out + 2
    }

    /// Discrete cosine transform (type-II) of an 8-sample vector derived from
    /// `seed`, returning the rounded energy of the coefficients.
    pub fn run_dct8_energy(_env: Env, seed: u32) -> u64 {
        let mut input = [0i64; 8];
        let mut r = seed as u64 | 1;
        for slot in input.iter_mut() {
            r = r
                .wrapping_mul(2_862_933_555_777_941_757)
                .wrapping_add(3037000493);
            *slot = ((r >> 40) as i64) - 8_388_608;
        }
        // Fixed-point cosine table * 4096, index = (2n+1)k mod 32, quarter wave.
        const COS: [i64; 9] = [4096, 4017, 3784, 3406, 2896, 2276, 1567, 799, 0];
        let mut energy = 0u64;
        for k in 0..8usize {
            let mut acc: i64 = 0;
            for (n, &x) in input.iter().enumerate() {
                let idx = ((2 * n + 1) * k) % 32;
                let (sign, q) = if idx <= 8 {
                    (1i64, idx)
                } else if idx <= 16 {
                    (-1i64, 16 - idx)
                } else if idx <= 24 {
                    (-1i64, idx - 16)
                } else {
                    (1i64, 32 - idx)
                };
                acc += sign * x * COS[q];
            }
            let coeff = acc >> 12;
            energy = energy.wrapping_add((coeff * coeff) as u64);
        }
        energy
    }

    /// Sieve of Eratosthenes up to `limit` (capped), returning the prime count.
    pub fn run_prime_count(_env: Env, limit: u32) -> u32 {
        let n = limit.min(4096) as usize;
        if n < 2 {
            return 0;
        }
        let mut sieve = [true; 4096];
        let mut count = 0u32;
        let mut p = 2usize;
        while p < n {
            if sieve[p] {
                count += 1;
                let mut m = p * p;
                while m < n {
                    sieve[m] = false;
                    m += p;
                }
            }
            p += 1;
        }
        count
    }
}
