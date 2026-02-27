/// Fixed-capacity dirty-tracking bitset.
///
/// Each bit tracks whether an element (tile, scanline, etc.) has been
/// modified since the last `clear()`. The `force_all` flag provides
/// O(1) bulk invalidation for events like palette changes or state loads.
///
/// Capacity is `N × 64` bits. Use const generic `N` to size the bitset:
/// - MCR II tiles: `DirtyBitset<15>` (960 tiles, 15 × 64 = 960)
/// - Williams scanlines: `DirtyBitset<5>` (264 lines, 5 × 64 = 320)
pub struct DirtyBitset<const N: usize> {
    words: [u64; N],
    force_all: bool,
}

impl<const N: usize> DirtyBitset<N> {
    /// Create a new bitset with all bits marked dirty.
    pub const fn new_all_dirty() -> Self {
        Self {
            words: [u64::MAX; N],
            force_all: true,
        }
    }

    /// Mark a single element as dirty.
    #[inline]
    pub fn mark(&mut self, index: usize) {
        self.words[index / 64] |= 1u64 << (index % 64);
    }

    /// Mark all elements as dirty (O(1) — deferred to `clear()`).
    #[inline]
    pub fn mark_all(&mut self) {
        self.force_all = true;
    }

    /// Test whether an element is dirty.
    #[inline]
    pub fn is_dirty(&self, index: usize) -> bool {
        self.force_all || self.words[index / 64] & (1u64 << (index % 64)) != 0
    }

    /// Merge another bitset's dirty bits into this one (OR).
    #[inline]
    pub fn merge(&mut self, other: &Self) {
        if other.force_all {
            self.force_all = true;
        } else {
            for i in 0..N {
                self.words[i] |= other.words[i];
            }
        }
    }

    /// Clear all dirty bits. Call after processing all dirty elements.
    #[inline]
    pub fn clear(&mut self) {
        self.words = [0u64; N];
        self.force_all = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_all_dirty() {
        let bs = DirtyBitset::<2>::new_all_dirty();
        for i in 0..128 {
            assert!(
                bs.is_dirty(i),
                "bit {i} should be dirty after new_all_dirty"
            );
        }
    }

    #[test]
    fn mark_and_test() {
        let mut bs = DirtyBitset::<2>::new_all_dirty();
        bs.clear();

        assert!(!bs.is_dirty(0));
        assert!(!bs.is_dirty(63));
        assert!(!bs.is_dirty(64));
        assert!(!bs.is_dirty(127));

        bs.mark(0);
        bs.mark(63);
        bs.mark(64);

        assert!(bs.is_dirty(0));
        assert!(bs.is_dirty(63));
        assert!(bs.is_dirty(64));
        assert!(!bs.is_dirty(127));
    }

    #[test]
    fn clear_resets_all() {
        let mut bs = DirtyBitset::<2>::new_all_dirty();
        bs.clear();

        for i in 0..128 {
            assert!(!bs.is_dirty(i), "bit {i} should be clean after clear");
        }
    }

    #[test]
    fn mark_all_makes_everything_dirty() {
        let mut bs = DirtyBitset::<2>::new_all_dirty();
        bs.clear();
        assert!(!bs.is_dirty(42));

        bs.mark_all();
        for i in 0..128 {
            assert!(bs.is_dirty(i), "bit {i} should be dirty after mark_all");
        }
    }

    #[test]
    fn mark_all_then_clear() {
        let mut bs = DirtyBitset::<1>::new_all_dirty();
        bs.clear();
        bs.mark_all();
        bs.clear();

        for i in 0..64 {
            assert!(
                !bs.is_dirty(i),
                "bit {i} should be clean after mark_all+clear"
            );
        }
    }

    #[test]
    fn word_boundary_bits() {
        let mut bs = DirtyBitset::<3>::new_all_dirty();
        bs.clear();

        // Set last bit of each word and first bit of next
        bs.mark(63);
        bs.mark(64);
        bs.mark(127);
        bs.mark(128);

        assert!(!bs.is_dirty(62));
        assert!(bs.is_dirty(63));
        assert!(bs.is_dirty(64));
        assert!(!bs.is_dirty(65));
        assert!(!bs.is_dirty(126));
        assert!(bs.is_dirty(127));
        assert!(bs.is_dirty(128));
        assert!(!bs.is_dirty(129));
    }

    #[test]
    fn mcr2_tile_count() {
        // 960 tiles = 15 × 64, verify last tile is addressable
        let mut bs = DirtyBitset::<15>::new_all_dirty();
        bs.clear();

        bs.mark(959);
        assert!(bs.is_dirty(959));
        assert!(!bs.is_dirty(958));
    }
}
