use std::hash::Hasher;

pub struct StableHasher {
    state: u64,
}

impl StableHasher {
    pub fn new() -> StableHasher {
        StableHasher { state: 0 }
    }
}

impl Hasher for StableHasher {
    fn finish(&self) -> u64 {
        self.state
    }
    fn write(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.state = self.state.rotate_left(1) ^ (*byte as u64);
        }
    }
}
