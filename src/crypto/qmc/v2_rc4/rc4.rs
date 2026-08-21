#[derive(Clone)]
pub(super) struct Rc4 {
    state: Box<[u8]>,
    i: usize,
    j: usize,
}
impl Rc4 {
    pub(super) fn new(key: &[u8]) -> Self {
        let mut state: Box<[u8]> = (0..key.len()).map(|index| index as u8).collect();
        let mut j = 0;
        for index in 0..state.len() {
            j = (j + state[index] as usize + key[index % key.len()] as usize) % state.len();
            state.swap(index, j);
        }
        Self { state, i: 0, j: 0 }
    }
    fn generate(&mut self) -> u8 {
        let n = self.state.len();
        self.i = (self.i + 1) % n;
        self.j = (self.j + self.state[self.i] as usize) % n;
        self.state.swap(self.i, self.j);
        self.state[(self.state[self.i] as usize + self.state[self.j] as usize) % n]
    }
    pub(super) fn derive(&mut self, output: &mut [u8]) {
        for byte in output {
            *byte ^= self.generate();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pinned_rc4_vector() {
        let mut rc4 = Rc4::new(b"this is a test key");
        let mut data = *b"hello world";
        rc4.derive(&mut data);
        assert_eq!(data, *b"\x68\x75\x6b\x64\x64\x24\x7f\x60\x7c\x7d\x60");
    }
}
