use super::{hash::hash, rc4::Rc4, segment_key::get_segment_key};
const FIRST: usize = 0x80;
const SEGMENT: usize = 0x1400;
const CACHE: usize = SEGMENT + 512;
#[derive(Debug, Clone)]
pub(crate) struct Qmc2Rc4 {
    hash: f64,
    key: Box<[u8]>,
    stream: Box<[u8; CACHE]>,
}
impl Qmc2Rc4 {
    pub(crate) fn new(key: &[u8]) -> Self {
        let mut rc4 = Rc4::new(key);
        let mut stream = Box::new([0; CACHE]);
        rc4.derive(&mut stream[..]);
        Self {
            hash: hash(key),
            key: key.into(),
            stream,
        }
    }
    fn first(&self, data: &mut [u8], offset: usize) {
        let n = self.key.len();
        for (byte, offset) in data.iter_mut().zip(offset..) {
            let index = get_segment_key(offset as u64, self.key[offset % n], self.hash) % n as u64;
            *byte ^= self.key[index as usize];
        }
    }
    fn other(&self, data: &mut [u8], offset: usize) {
        let id = offset / SEGMENT;
        let block = offset % SEGMENT;
        let skip =
            (get_segment_key(id as u64, self.key[id % self.key.len()], self.hash) & 0x1ff) as usize;
        for (byte, key) in data.iter_mut().zip(self.stream.iter().skip(skip + block)) {
            *byte ^= *key;
        }
    }
    pub(crate) fn decrypt(&self, data: &mut [u8], mut offset: usize) {
        let mut data = data;
        if offset < FIRST {
            let n = (FIRST - offset).min(data.len());
            let (a, b) = data.split_at_mut(n);
            self.first(a, offset);
            data = b;
            offset += n;
        }
        if offset % SEGMENT != 0 {
            let n = (SEGMENT - offset % SEGMENT).min(data.len());
            let (a, b) = data.split_at_mut(n);
            self.other(a, offset);
            data = b;
            offset += n;
        }
        while !data.is_empty() {
            let n = SEGMENT.min(data.len());
            let (a, b) = data.split_at_mut(n);
            self.other(a, offset);
            data = b;
            offset += n;
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn upstream_rc4_vector() {
        let key = (b'a'..=b'z')
            .chain(b'A'..=b'Z')
            .chain(b'0'..=b'9')
            .cycle()
            .take(512)
            .collect::<Vec<_>>();
        let cipher = Qmc2Rc4::new(&key);
        let mut data = vec![0; 0x1480];
        cipher.decrypt(&mut data, 0);
        cipher.decrypt(&mut data, 0);
        assert_eq!(data, vec![0; 0x1480]);
    }
}
