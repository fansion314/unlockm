pub(super) fn hash(key: &[u8]) -> f64 {
    let mut hash = 1_u32;
    for &value in key {
        if value == 0 {
            continue;
        }
        let next = hash.wrapping_mul(value as u32);
        if next == 0 || next <= hash {
            break;
        }
        hash = next;
    }
    hash.into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pinned_hash_vector() {
        assert_eq!(hash(b"hello world"), 4_045_008_896.0);
    }
}
