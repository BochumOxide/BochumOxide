use anyhow::Result;

// recursive De Bruijn sequence builder
fn _db(t: usize, p: usize, n: usize, k: usize, sequence: &mut Vec<u8>, a: &mut Vec<u8>) {
    if t > n {
        if n % p == 0 {
            sequence.extend(a[1..=p].to_vec());
        }
    } else {
        a[t] = a[t - p];
        _db(t + 1, p, n, k, sequence, a);
        let start = a[t - p] + 1;
        let end: u8 = k as u8;
        for j in start..end {
            a[t] = j;
            _db(t + 1, t, n, k, sequence, a);
        }
    }
}

// algo: https://en.wikipedia.org/wiki/De_Bruijn_sequence
pub fn de_bruijn_int(k: usize, n: usize) -> Vec<u8> {
    // alphabet: numbers 0 - k
    // n: length of unique subsequences
    // return: genereated De Bruijn sequence
    let mut sequence: Vec<u8> = vec![];

    let mut a: Vec<u8> = vec![0; k * n];
    _db(1, 1, n, k, &mut sequence, &mut a);
    return sequence.to_vec();
}

// algo: https://en.wikipedia.org/wiki/De_Bruijn_sequence
pub fn de_bruijn_string(alphabet: &[u8], n: usize) -> String {
    // alphabet: byte sice
    // n: length of unique subsequences
    // return: genereated De Bruijn sequence
    let k: usize = alphabet.len();
    let mut sequence: Vec<u8> = vec![];

    let mut a: Vec<u8> = vec![0; k * n];
    _db(1, 1, n, k, &mut sequence, &mut a);

    let seq_char: Vec<char> = sequence
        .iter()
        .map(|elem| alphabet[*elem as usize] as char)
        .collect();

    seq_char.into_iter().collect()
}

// get the first position of subseq in the generator
fn _gen_find(subseq: &[u8], generator: &[u8]) -> Option<usize> {
    // subseq: subsequence to find
    // generator: total sequence
    // return:  first position of subseq in the generator (or None if not present)
    let mut pos: usize = 0;
    let mut saved = vec![];

    for c in generator {
        saved.append(&mut vec![c.to_owned()]);
        if saved.len() > subseq.len() {
            saved.drain(0..1);
            pos += 1;
        }
        if saved == subseq {
            return Some(pos);
        }
    }
    None
}

// wrapper over de_bruijn
pub fn cyclic(length: usize, n: usize) -> Result<Vec<u8>> {
    // length: wanted length of sequence
    // alphabet: list of bytes/ints to generate the sequence over.
    // n: length of unique subsequences
    // return: at most length elements of sequence
    let alphabet = b"abcd";
    let max_sequence = alphabet.len().pow(n as u32);
    if max_sequence < length {
        panic!(
            "Can't create a pattern of length = {} with alphabet length = {} and n = {}",
            length,
            alphabet.len(),
            n
        );
    }
    let generator = de_bruijn_string(alphabet, n);
    Ok(generator[..length].as_bytes().to_vec())
}

// Calculates the position of a substring into a De Bruijn sequence
pub fn cyclic_find(subseq: &[u8], n: usize) -> Option<usize> {
    // subseq: subsequence to find
    // alphabet: listto generate the sequence over
    // n: length of unique subsequences
    // return: position of a substring into a De Bruijn sequence

    let alphabet = b"abcd";
    if subseq.len() != n {
        // subseq = &subseq[..n];
        panic!("len(subseq) != n");
    }

    if subseq.iter().any(|i| !alphabet.contains(i)) {
        panic!(
            "Can't create a pattern length={} with len(alphabet)=={} and n=={}",
            alphabet.len(),
            alphabet.len(),
            n
        );
    }
    let k = alphabet.len();
    _gen_find(subseq, &de_bruijn_string(alphabet, n).as_bytes())
}

#[derive(Debug)]
pub struct CyclicGen {
    _generator: Vec<u8>,
    _alphabet: Vec<u8>,
    _total_length: usize,
    _n: usize,
    _chunks: Vec<usize>,
}

impl CyclicGen {
    // generate cyclic generator to generate sequential chunks of de Bruijn sequences
    pub fn new(alphabet: &[u8], n: usize) -> Self {
        // alphabet: numbers 0 - k
        // n: length of unique subsequences
        CyclicGen {
            _generator: de_bruijn_int(alphabet.len(), n),
            _alphabet: alphabet.to_vec(),
            _total_length: 0,
            _n: n,
            _chunks: vec![],
        }
    }

    // Get the next de Bruijn sequence from this generator.
    pub fn get(&mut self, length: usize) -> Result<Vec<u8>> {
        // length: size of chunk to get
        // return: a chunk of length
        self._chunks.append(&mut vec![length]);
        self._total_length += length;
        let max_sequence = self._alphabet.len().pow(self._n as u32);

        if max_sequence < self._total_length {
            panic!(
                "Can't create a pattern length={} with len(alphabet)=={} and n=={}",
                self._total_length,
                self._alphabet.len(),
                self._n
            )
        }

        let res = self._generator.drain(..length).collect();

        Ok(res)
    }

    // Find a chunk and subindex from all the generates de Bruijn sequences.
    pub fn find(self, subseq: &[u8]) -> Option<(usize, usize, usize)> {
        // subseq: subsequence to find
        // return: tuple (total_idx, chunk_idx, inside_chunk_idx) or None if not present
        let total_idx = cyclic_find(subseq, self._n).unwrap();
        let mut inside_chunk_idx = total_idx;
        for chunk_idx in 0..=self._chunks.len() {
            let chunk = self._chunks[chunk_idx];
            if inside_chunk_idx < chunk {
                return Some((total_idx, chunk_idx, inside_chunk_idx));
            }
            inside_chunk_idx -= chunk;
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_de_bruijn_string() {
        let alphabet = b"ab";
        assert_eq!(de_bruijn_string(alphabet, 3), "aaababbb");
    }

    #[test]
    fn test_de_bruijn_int() {
        assert_eq!(de_bruijn_int(2, 3), vec![0, 0, 0, 1, 0, 1, 1, 1]);
    }

    #[test]
    fn test_cyclic() {
        assert_eq!(
            cyclic(10, 3).unwrap(),
            vec![97, 97, 97, 98, 97, 97, 99, 97, 97, 100]
        );
    }

    #[test]
    fn test_cyclic_find() {
        assert_eq!(cyclic_find(&[97, 97, 97, 98], 4).unwrap(), 1);
    }

    #[test]
    fn test_generator_get() {
        let mut gen = CyclicGen::new(&[0, 1, 2], 3);
        assert_eq!(gen.get(2).unwrap(), vec![0, 0]);
        assert_eq!(gen.get(6).unwrap(), vec![0, 1, 0, 0, 2, 0]);
    }
}
