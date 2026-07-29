//! Bitwise utility functions used by the budget-report internals.
//!
//! These helpers handle low-level bit manipulation tasks such as:
//! - Rounding values up to the nearest power of two (for size alignment).
//! - Packing and unpacking multiple small resource counters into a single `u64`
//!   for compact on-disk or in-memory storage.
//! - Testing whether a value is a power of two (common pre-condition for
//!   alignment-sensitive operations).
//! - Extracting bit flags that encode which resource categories were measured
//!   during simulation.
//!
//! Each function includes inline comments that explain the intent of every
//! bitwise operation, referencing well-known patterns where applicable.

/// Returns the smallest power of two greater than or equal to `n`.
///
/// When `n` is zero the function returns 1, matching the convention used in
/// many allocators: the smallest allocation is a single unit.
///
/// # Bitwise strategy
///
/// 1. Subtract 1 so that an exact power of two (e.g. 8 → 7 → `0b111`) does
///    not round up to the next power.
/// 2. Propagate the highest set bit to all lower bit positions so that every
///    bit below the most significant one becomes 1 (e.g. `0b0101_1000` →
///    `0b0111_1111`).
/// 3. Add 1 to obtain the next power of two (`0b0111_1111 + 1` →
///    `0b1000_0000`).
///
/// This is the standard bit-twiddling "round up to power of two" algorithm.
/// It works for any `u64` value, returning 0 on overflow (wrapping)
/// when `n > 2^63` — the caller should check for that if it matters.
pub fn round_up_to_power_of_two(n: u64) -> u64 {
    if n == 0 {
        return 1;
    }
    // Subtract 1 so that exact powers of two don't get doubled.
    let mut v = n.wrapping_sub(1);
    // Shift right by 1 and OR: propagate the MSB one position down.
    v |= v >> 1;
    // Shift right by 2 and OR: cover the next two bits.
    v |= v >> 2;
    // Shift right by 4 and OR: cover the gap left by the previous OR.
    v |= v >> 4;
    // Shift right by 8 and OR: cover another 8 positions.
    v |= v >> 8;
    // Shift right by 16 and OR: cover the next 16.
    v |= v >> 16;
    // Shift right by 32 and OR: cover the remaining 32 (for 64-bit values).
    v |= v >> 32;
    // Adding 1 turns the all-ones below-MSB into the next power of two.
    // Wrapping_add ensures no panic on overflow when n > 2^63.
    v.wrapping_add(1)
}

/// Returns `true` when `n` is a power of two (including 1, but excluding 0).
///
/// # Bitwise strategy
///
/// A power of two has exactly one bit set.  Subtracting 1 clears that bit and
/// sets all lower bits.  The AND of `n` and `n - 1` is therefore zero only
/// for powers of two:
///
/// ```text
///   n           n-1         n & (n-1)
///   0b0001      0b0000      0b0000   ← power of two (1)
///   0b0010      0b0001      0b0000   ← power of two (2)
///   0b0011      0b0010      0b0010   ← not a power of two
///   0b0100      0b0011      0b0000   ← power of two (4)
///   0b0101      0b0100      0b0100   ← not a power of two
/// ```
pub fn is_power_of_two(n: u64) -> bool {
    // n > 0 is required because 0 & (0 - 1) == 0, which would be a false
    // positive.
    // n & (n - 1) == 0 is the classic power-of-two check.
    n > 0 && (n & (n.wrapping_sub(1))) == 0
}

/// Resource categories that can be measured during simulation.
///
/// These are stored as bit flags so multiple categories can be tracked
/// simultaneously in a single `u64` field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceKind {
    CpuInstructions = 0b0001,
    ReadBytes = 0b0010,
    WriteBytes = 0b0100,
}

/// A set of resource kinds, stored as a bitmask.
///
/// Using bitwise OR (`|`) to combine flags and bitwise AND (`&`) to test
/// membership makes union and intersection checks constant-time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResourceMask(u64);

impl ResourceMask {
    /// The empty resource set — no bits set.
    pub const NONE: ResourceMask = ResourceMask(0);
    /// All resource bits set.
    pub const ALL: ResourceMask = ResourceMask(
        ResourceKind::CpuInstructions as u64
            | ResourceKind::ReadBytes as u64
            | ResourceKind::WriteBytes as u64,
    );

    /// Create a mask containing only the given resource.
    ///
    /// The shift `1 << kind` converts the resource discriminant into a bit
    /// position.  A single-bit mask begins at that position and grows no
    /// further — this is the same pattern used in Linux `O_*` flags and
    /// similar bitflag APIs.
    pub fn from_kind(kind: ResourceKind) -> Self {
        ResourceMask(kind as u64)
    }

    /// Insert a resource kind into this mask.
    ///
    /// `mask | bit` turns the target bit on without affecting other bits.
    pub fn insert(&mut self, kind: ResourceKind) {
        self.0 |= kind as u64;
    }

    /// Remove a resource kind from this mask.
    ///
    /// `mask & !bit` clears the target bit while leaving all other bits
    /// unchanged.  The bitwise NOT (`!`) inverts the single-bit value so
    /// that the AND acts as a "clear this flag" operation.
    pub fn remove(&mut self, kind: ResourceKind) {
        self.0 &= !(kind as u64);
    }

    /// Returns `true` when `kind` is present in the mask.
    ///
    /// `mask & bit == bit` verifies that the specific bit is set,
    /// distinguishing it from the case where multiple bits are set but
    /// the tested one is not.
    pub fn contains(&self, kind: ResourceKind) -> bool {
        self.0 & (kind as u64) == kind as u64
    }

    /// Returns the raw bitmask value.
    pub fn bits(&self) -> u64 {
        self.0
    }

    /// Build a mask from a raw bitmask value.  Bits that do not correspond
    /// to recognised resource kinds are silently cleared so that the mask
    /// never contains unknown flags.
    ///
    /// `raw & KNOWN` is a bitwise AND mask that strips unknown high bits.
    pub fn from_bits_truncate(raw: u64) -> Self {
        const KNOWN: u64 = ResourceKind::CpuInstructions as u64
            | ResourceKind::ReadBytes as u64
            | ResourceKind::WriteBytes as u64;
        ResourceMask(raw & KNOWN)
    }
}

/// Pack three `u32` counters (instructions, read_bytes, write_bytes) into a
/// single `u64` for compact storage.
///
/// Layout (bit positions, 0 = LSB):
///
/// ```text
///  |<── 21 bits ──>|<── 21 bits ──>|<── 22 bits ──>|
///  |  instructions |  read_bytes   |  write_bytes   |
///  0              20              41              63
/// ```
///
/// Each counter occupies a fixed bit-field so the packed value can be stored
/// as a single `u64` column in a database or serialised as a TOML integer.
/// The 21/21/22 split was chosen because the practical per-call resource
/// budgets in Soroban fit comfortably within 2²¹ (≈2 million) for
/// instructions and read/write bytes.  If any counter exceeds its field
/// width the value is truncated by the left-shift + OR logic.
pub fn pack_resources(instructions: u32, read_bytes: u32, write_bytes: u32) -> u64 {
    // Place instructions in the lowest 21 bits (bits 0..20).
    // Casting to u64 widens the value without sign extension.
    let packed = (instructions as u64)
        // Shift read_bytes left by 21 so it occupies bits 21..41.
        | ((read_bytes as u64) << 21)
        // Shift write_bytes left by 42 (21 + 21) so it occupies bits 42..63.
        | ((write_bytes as u64) << 42);
    packed
}

/// Unpack the three counters previously packed by [`pack_resources`].
///
/// Each field is extracted by shifting right to discard lower bits and then
/// applying a bitmask to discard higher bits.
pub fn unpack_resources(packed: u64) -> (u32, u32, u32) {
    // Low 21-bit mask: 0b111_1111_1111_1111_1111_111 = 0x1F_FFFF
    const LOW_21: u64 = (1 << 21) - 1;
    // Low 22-bit mask: 0b11_1111_1111_1111_1111_1111_11 = 0x3F_FFFF
    const LOW_22: u64 = (1 << 22) - 1;

    // Extract instructions: mask the lowest 21 bits.
    // packed & LOW_21 zeros out everything above bit 20.
    let instructions = (packed & LOW_21) as u32;

    // Extract read_bytes: shift right by 21 to bring the field to the LSB,
    // then mask with LOW_21 to discard any spill-over from write_bytes.
    let read_bytes = ((packed >> 21) & LOW_21) as u32;

    // Extract write_bytes: shift right by 42 to bring the field to the LSB,
    // then mask with LOW_22 (the field is 22 bits wide).
    let write_bytes = ((packed >> 42) & LOW_22) as u32;

    (instructions, read_bytes, write_bytes)
}

/// Align `offset` up to the next multiple of `alignment`.
///
/// `alignment` must be a power of two (checked at runtime).  This is the
/// standard "round up to alignment" trick used in many arenas and serialisers.
///
/// # Bitwise strategy
///
/// 1. `alignment - 1` produces a mask of the low bits that represent the
///    offset within an alignment block (e.g. alignment = 8 → mask = 0b111).
/// 2. `offset + mask` pushes the value past the next alignment boundary
///    unless it is already aligned.
/// 3. `!mask` inverts the mask so that ANDing it clears the low bits,
///    snapping the value down to the nearest multiple of alignment.
///
/// The formula is: `(offset + alignment - 1) & !(alignment - 1)`.
pub fn align_up(offset: u64, alignment: u64) -> u64 {
    debug_assert!(
        is_power_of_two(alignment),
        "alignment must be a power of two, got {}",
        alignment
    );
    // alignment - 1 is a mask of the bits below the alignment boundary.
    let mask = alignment.wrapping_sub(1);
    // (offset + mask) rounds up; & !mask truncates to the alignment boundary.
    (offset.wrapping_add(mask)) & !mask
}

#[cfg(test)]
mod bitwise_tests {
    use super::*;

    // ── round_up_to_power_of_two ───────────────────────────────────────

    #[test]
    fn round_up_zero_returns_one() {
        assert_eq!(round_up_to_power_of_two(0), 1);
    }

    #[test]
    fn round_up_one_is_one() {
        assert_eq!(round_up_to_power_of_two(1), 1);
    }

    #[test]
    fn round_up_two_is_two() {
        assert_eq!(round_up_to_power_of_two(2), 2);
    }

    #[test]
    fn round_up_three_to_four() {
        // 3 -> next power of two is 4.
        assert_eq!(round_up_to_power_of_two(3), 4);
    }

    #[test]
    fn round_up_five_to_eight() {
        // 5 -> next power of two is 8.
        assert_eq!(round_up_to_power_of_two(5), 8);
    }

    #[test]
    fn round_up_seven_to_eight() {
        assert_eq!(round_up_to_power_of_two(7), 8);
    }

    #[test]
    fn round_up_exact_power_returns_self() {
        assert_eq!(round_up_to_power_of_two(1024), 1024);
    }

    #[test]
    fn round_up_large_power_of_two() {
        // 2^20 = 1,048,576.
        assert_eq!(round_up_to_power_of_two(1_048_576), 1_048_576);
    }

    #[test]
    fn round_up_one_below_large_power() {
        assert_eq!(round_up_to_power_of_two(1_048_575), 1_048_576);
    }

    #[test]
    fn round_up_u64_max_wraps_to_zero() {
        // u64::MAX rounds up to 2^64 which wraps to 0.
        assert_eq!(round_up_to_power_of_two(u64::MAX), 0);
    }

    // ── is_power_of_two ─────────────────────────────────────────────────

    #[test]
    fn is_power_of_two_zero_is_false() {
        assert!(!is_power_of_two(0));
    }

    #[test]
    fn is_power_of_two_one_is_true() {
        assert!(is_power_of_two(1));
    }

    #[test]
    fn is_power_of_two_two_is_true() {
        assert!(is_power_of_two(2));
    }

    #[test]
    fn is_power_of_two_three_is_false() {
        assert!(!is_power_of_two(3));
    }

    #[test]
    fn is_power_of_two_large_power() {
        assert!(is_power_of_two(1 << 63));
    }

    #[test]
    fn is_power_of_two_one_below_large_power() {
        assert!(!is_power_of_two((1 << 63) - 1));
    }

    // ── ResourceMask bitwise operations ────────────────────────────────

    #[test]
    fn resource_mask_none_has_no_bits() {
        assert_eq!(ResourceMask::NONE.bits(), 0);
    }

    #[test]
    fn resource_mask_all_has_all_resource_bits() {
        let all = ResourceMask::from_kind(ResourceKind::CpuInstructions)
            | ResourceMask::from_kind(ResourceKind::ReadBytes)
            | ResourceMask::from_kind(ResourceKind::WriteBytes);
        // Bitwise OR can't be used directly on structs, so we use insert.
        let mut mask = ResourceMask::NONE;
        mask.insert(ResourceKind::CpuInstructions);
        mask.insert(ResourceKind::ReadBytes);
        mask.insert(ResourceKind::WriteBytes);
        assert!(mask.contains(ResourceKind::CpuInstructions));
        assert!(mask.contains(ResourceKind::ReadBytes));
        assert!(mask.contains(ResourceKind::WriteBytes));
    }

    #[test]
    fn resource_mask_insert_and_contains() {
        let mut mask = ResourceMask::NONE;
        assert!(!mask.contains(ResourceKind::ReadBytes));
        mask.insert(ResourceKind::ReadBytes);
        assert!(mask.contains(ResourceKind::ReadBytes));
        assert!(!mask.contains(ResourceKind::CpuInstructions));
    }

    #[test]
    fn resource_mask_remove_clears_bit() {
        let mut mask = ResourceMask::ALL;
        mask.remove(ResourceKind::CpuInstructions);
        assert!(!mask.contains(ResourceKind::CpuInstructions));
        assert!(mask.contains(ResourceKind::ReadBytes));
        assert!(mask.contains(ResourceKind::WriteBytes));
    }

    #[test]
    fn resource_mask_from_bits_truncate_clears_unknown() {
        // Only the lowest 3 bits are recognised; higher bits are truncated.
        let raw: u64 = 0b1111_1111;
        let mask = ResourceMask::from_bits_truncate(raw);
        assert_eq!(mask.bits(), 0b0000_0111);
    }

    #[test]
    fn resource_mask_from_bits_truncate_preserves_known() {
<<<<<<< HEAD
        let raw: u64 = ResourceKind::CpuInstructions as u64 | ResourceKind::WriteBytes as u64;
=======
        let raw: u64 = ResourceKind::CpuInstructions as u64
            | ResourceKind::WriteBytes as u64;
>>>>>>> origin/main
        let mask = ResourceMask::from_bits_truncate(raw);
        assert!(mask.contains(ResourceKind::CpuInstructions));
        assert!(mask.contains(ResourceKind::WriteBytes));
        assert!(!mask.contains(ResourceKind::ReadBytes));
    }

    // ── pack_resources / unpack_resources ──────────────────────────────

    #[test]
    fn pack_unpack_zeros() {
        let packed = pack_resources(0, 0, 0);
        assert_eq!(packed, 0);
        assert_eq!(unpack_resources(packed), (0, 0, 0));
    }

    #[test]
    fn pack_unpack_small_values() {
        let packed = pack_resources(100, 200, 300);
        assert_eq!(unpack_resources(packed), (100, 200, 300));
    }

    #[test]
    fn pack_unpack_max_values_within_field_width() {
        // 21-bit max = 2^21 - 1 = 2,097,151
        // 22-bit max = 2^22 - 1 = 4,194,303
        let max_21: u32 = (1 << 21) - 1;
        let max_22: u32 = (1 << 22) - 1;
        let packed = pack_resources(max_21, max_21, max_22);
        assert_eq!(unpack_resources(packed), (max_21, max_21, max_22));
    }

    #[test]
    fn pack_unpack_values_exceeding_field_width_truncate() {
        // Each field is truncated by left-shift overflow.  A 22-bit value
        // written into the 21-bit instructions field loses its MSB.
        let too_big: u32 = (1 << 22) - 1; // 4,194,303 — needs 22 bits
        let expected_instructions = too_big & ((1 << 21) - 1);
        let packed = pack_resources(too_big, 0, 0);
        let (i, r, w) = unpack_resources(packed);
        assert_eq!(i, expected_instructions, "instructions truncated");
        assert_eq!(r, 0);
        assert_eq!(w, 0);
    }

    #[test]
    fn pack_unpack_round_trip_u32_max() {
        // u32::MAX is 4,294,967,295.  The 21-bit instructions and read_bytes
        // fields will truncate; the 22-bit write_bytes field will also
        // truncate.  This test documents the truncation behaviour.
        let packed = pack_resources(u32::MAX, u32::MAX, u32::MAX);
        let (i, r, w) = unpack_resources(packed);
        assert_eq!(i, (u32::MAX as u64 & ((1 << 21) - 1)) as u32);
        assert_eq!(r, (u32::MAX as u64 & ((1 << 21) - 1)) as u32);
        assert_eq!(w, (u32::MAX as u64 & ((1 << 22) - 1)) as u32);
    }

    // ── align_up ───────────────────────────────────────────────────────

    #[test]
    fn align_up_zero_to_any_is_zero() {
        assert_eq!(align_up(0, 1), 0);
        assert_eq!(align_up(0, 4), 0);
        assert_eq!(align_up(0, 256), 0);
    }

    #[test]
    fn align_up_aligned_value_is_unchanged() {
        assert_eq!(align_up(4, 4), 4);
        assert_eq!(align_up(8, 4), 8);
        assert_eq!(align_up(256, 256), 256);
    }

    #[test]
    fn align_up_rounds_up_to_next_boundary() {
        assert_eq!(align_up(5, 4), 8);
        assert_eq!(align_up(7, 4), 8);
        assert_eq!(align_up(9, 8), 16);
    }

    #[test]
    fn align_up_one_below_boundary() {
        assert_eq!(align_up(3, 4), 4);
        assert_eq!(align_up(7, 8), 8);
        assert_eq!(align_up(255, 256), 256);
    }

    #[test]
    fn align_up_large_alignment() {
        assert_eq!(align_up(1_000_000, 4096), 1_003_520);
    }

    #[test]
    fn align_up_alignment_one_is_identity() {
        // Alignment of 1 means every value is already aligned.
        assert_eq!(align_up(0, 1), 0);
        assert_eq!(align_up(42, 1), 42);
        assert_eq!(align_up(u64::MAX, 1), u64::MAX);
    }
}
