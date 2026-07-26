//! Utility functions and unit tests for edge-case coverage (task-28).
/// Returns the index where `val` should be inserted into `arr` (sorted
/// ascending) to maintain the sort order. If `arr` is empty returns `0`.
fn find_insert_pos(arr: &[i32], val: i32) -> usize {
    let mut lo = 0usize;
    let mut hi = arr.len();
    while lo < hi {
        let mid = lo + (hi - lo) / 2;
        if arr[mid] < val {
            lo = mid + 1;
        } else {
            hi = mid;
        }
    }
    lo
}

/// Sums `u64` values returning `None` on overflow. For an empty slice
/// returns `Some(0)`.
fn sum_u64_checked(items: &[u64]) -> Option<u64> {
    let mut acc: u128 = 0;
    for &v in items {
        acc += v as u128;
        if acc > u64::MAX as u128 {
            return None;
        }
    }
    Some(acc as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_pos_empty() {
        let arr: [i32; 0] = [];
        assert_eq!(find_insert_pos(&arr, 5), 0);
    }

    #[test]
    fn insert_pos_boundaries() {
        let arr = [1, 3, 5];
        // Insert at beginning
        assert_eq!(find_insert_pos(&arr, 0), 0);
        // Insert equal to an element => before the equal element
        assert_eq!(find_insert_pos(&arr, 3), 1);
        // Insert between elements
        assert_eq!(find_insert_pos(&arr, 4), 2);
        // Insert after all elements
        assert_eq!(find_insert_pos(&arr, 6), 3);
    }

    #[test]
    fn sum_u64_empty_and_overflow() {
        let empty: [u64; 0] = [];
        assert_eq!(sum_u64_checked(&empty), Some(0));

        // Boundary: u64::MAX + 0 should be OK
        let a = [u64::MAX, 0];
        assert_eq!(sum_u64_checked(&a), Some(u64::MAX));

        // Overflow by 1 should return None (off-by-one check)
        let b = [u64::MAX, 1];
        assert_eq!(sum_u64_checked(&b), None);
    }
}
