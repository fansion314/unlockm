pub(super) fn get_segment_key(id: u64, seed: u8, hash: f64) -> u64 {
    if seed == 0 {
        0
    } else {
        (hash / ((id + 1).wrapping_mul(seed.into()) as f64) * 100.0) as u64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pinned_segment_key_vectors() {
        assert_eq!(get_segment_key(1, 0, 12_345.0), 0);
        assert_eq!(get_segment_key(1, 123, 12_345.0), 5_018);
        assert_eq!(get_segment_key(51, 35, 516_402_887.0), 28_373_784);
        assert_eq!(get_segment_key(0, 66, 3_908_240_000.0), 5_921_575_757);
    }
}
