//! Computes a perfect hash table using [the CHD
//! algorithm](http://cmph.sourceforge.net/papers/esa09.pdf).
//!
//! Strongly inspired by <https://github.com/sfackler/rust-phf>

use rand::prelude::{RngExt, SeedableRng, SliceRandom, StdRng};

static NOVAL: char = '\0';

/// FNV
fn hash(s: &str, h: u64) -> u64 {
    let mut g = 0xcbf29ce484222325 ^ h;
    for b in s.bytes() {
        g ^= b as u64;
        g = g.wrapping_mul(0x100000001b3);
    }
    g
}

pub fn displace(f1: u32, f2: u32, d1: u32, d2: u32) -> u32 {
    d2.wrapping_add(f1.wrapping_mul(d1)).wrapping_add(f2)
}
fn split(hash: u64) -> Hash {
    let bits = 21;
    let mask = (1 << bits) - 1;
    Hash {
        g: (hash & mask) as u32,
        f1: ((hash >> bits) & mask) as u32,
        f2: ((hash >> (2 * bits)) & mask) as u32,
    }
}

#[derive(Copy, Clone)]
struct Hash {
    g: u32,
    f1: u32,
    f2: u32,
}

#[allow(clippy::type_complexity)]
fn try_phf_table(
    values: &[(char, &str)],
    lambda: usize,
    seed: u64,
    rng: &mut StdRng,
) -> Option<(Vec<(u32, u32)>, Vec<char>)> {
    let hashes: Vec<_> = values
        .iter()
        .map(|&(n, s)| (split(hash(s, seed)), n))
        .collect();

    let table_len = hashes.len();
    let buckets_len = table_len.div_ceil(lambda);

    let mut buckets = (0..buckets_len).map(|i| (i, vec![])).collect::<Vec<_>>();
    for &(h, cp) in hashes.iter() {
        buckets[h.g as usize % buckets_len].1.push((h, cp))
    }

    buckets.sort_by_key(|(_index, keys)| std::cmp::Reverse(keys.len()));

    let mut map: Vec<_> = std::iter::repeat_n(NOVAL, table_len).collect();
    let mut disps: Vec<_> = std::iter::repeat_n((0, 0), buckets_len).collect();

    let mut generation = 0;
    let mut try_map: Vec<_> = std::iter::repeat_n(0u64, table_len).collect();
    let mut values_to_add = vec![];

    let mut d1s = (0..(table_len as u32)).collect::<Vec<_>>();
    let mut d2s = d1s.clone();
    d1s.shuffle(rng);
    d2s.shuffle(rng);

    'next_bucket: for &(bkt_idx, ref bkt_keys) in buckets.iter() {
        for &d1 in d1s.iter() {
            'next_disp: for &d2 in d2s.iter() {
                generation += 1;
                values_to_add.clear();

                for &(h, cp) in bkt_keys.iter() {
                    let idx = (displace(h.f1, h.f2, d1, d2) % table_len as u32) as usize;

                    if map[idx] != NOVAL || try_map[idx] == generation {
                        continue 'next_disp;
                    }
                    try_map[idx] = generation;
                    values_to_add.push((idx, cp));
                }

                disps[bkt_idx] = (d1, d2);
                for &(idx, cp) in values_to_add.iter() {
                    map[idx] = cp
                }
                continue 'next_bucket;
            }
        }

        return None;
    }

    Some((disps, map))
}

pub fn create_phf(
    data: &[(char, &str)],
    lambda: usize,
    max_tries: usize,
) -> (u64, Vec<(u32, u32)>, Vec<char>) {
    let mut rng = StdRng::seed_from_u64(0xf0f0f0f0);

    for i in 0..(max_tries) {
        println!("PHF #{}", i);

        let seed = rng.random();
        if let Some((disp, map)) = try_phf_table(data, lambda, seed, &mut rng) {
            return (seed, disp, map);
        }
    }
    panic!(
        "could not create a length {} PHF with {}, {}",
        data.len(),
        lambda,
        max_tries
    );
}
