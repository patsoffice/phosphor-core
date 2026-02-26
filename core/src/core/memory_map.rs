//! Page-table-based memory map for 16-bit address spaces.
//!
//! Provides three capabilities that hand-rolled match arms cannot:
//!
//! 1. **Watchpoints** — per-page read/write watch flags with zero cost when
//!    no watchpoints are active (a single `active_watch_count == 0` check).
//! 2. **Introspection** — named region descriptors that a debugger or memory
//!    viewer can enumerate without parsing source code.
//! 3. **Declarative mirroring** — `mirror()` copies page entries at build time
//!    instead of hand-coding `addr & mask` per machine.
//!
//! The map divides the 64 KB address space into 256 pages of 256 bytes each.
//! Each page entry carries a machine-defined `region_id` (a plain `u8`) that
//! the machine's `Bus::read`/`write` dispatches on with a small match.

/// Machine-defined region identifier. Values are assigned by each machine
/// as constants (e.g., `const VIDEO_RAM: RegionId = 1`). The MemoryMap
/// stores and reports them but does not interpret them.
pub type RegionId = u8;

/// Sentinel region ID for unmapped pages.
pub const UNMAPPED: RegionId = 0;

/// What kind of access a region supports (for introspection and display).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AccessKind {
    ReadWrite,
    ReadOnly,
    WriteOnly,
    Io,
    Unmapped,
}

/// A single entry in the 256-page table.
#[derive(Clone, Copy, Debug)]
pub struct PageEntry {
    /// Machine-defined region identifier. The machine's `Bus::read`/`write`
    /// matches on this to reach the right backing store.
    pub region_id: RegionId,

    /// Byte offset into the region for the start of this page.
    /// For a region starting at address 0x4000 mapped to pages 0x40..0x43,
    /// page 0x40 has `base_offset = 0`, page 0x41 has `base_offset = 0x100`, etc.
    pub base_offset: u16,

    /// True if read watchpoint is active on this page.
    pub watch_read: bool,

    /// True if write watchpoint is active on this page.
    pub watch_write: bool,
}

impl Default for PageEntry {
    fn default() -> Self {
        Self {
            region_id: UNMAPPED,
            base_offset: 0,
            watch_read: false,
            watch_write: false,
        }
    }
}

/// A named region descriptor for debugger introspection.
#[derive(Clone, Debug)]
pub struct RegionDescriptor {
    /// Machine-defined region ID (matches `PageEntry::region_id`).
    pub id: RegionId,
    /// Human-readable name (e.g., "Video RAM", "Widget PIA").
    pub name: &'static str,
    /// First address in this region.
    pub start_addr: u16,
    /// Last address in this region (inclusive).
    pub end_addr: u16,
    /// Access characteristics.
    pub access: AccessKind,
}

/// Details of a watchpoint hit, consumed by the debugger after each tick.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WatchpointHit {
    pub addr: u16,
    pub kind: WatchpointKind,
    pub value: u8,
}

/// Whether a watchpoint triggers on reads, writes, or both.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum WatchpointKind {
    Read,
    Write,
}

/// Page-table-based memory map for a 16-bit address space.
///
/// 256 pages of 256 bytes each. Machines build this at init time and
/// use it in `Bus::read`/`write` to look up the `region_id` for dispatch.
///
/// The debugger uses it for watchpoints (per-page flags checked only on
/// flagged pages) and region introspection (list of named regions).
pub struct MemoryMap {
    pages: [PageEntry; 256],
    regions: Vec<RegionDescriptor>,
    active_watch_count: u16,
    pending_hit: Option<WatchpointHit>,
    /// Exact watched addresses. Page flags serve as a fast filter; this vec
    /// provides address-level precision so only the exact address fires.
    watched_addrs: Vec<(u16, WatchpointKind)>,
}

impl MemoryMap {
    /// Create a new memory map with all pages unmapped.
    pub fn new() -> Self {
        Self {
            pages: [PageEntry::default(); 256],
            regions: Vec::new(),
            active_watch_count: 0,
            pending_hit: None,
            watched_addrs: Vec::new(),
        }
    }

    // -----------------------------------------------------------------------
    // Builder methods (called at machine init time)
    // -----------------------------------------------------------------------

    /// Map a contiguous address range to a region.
    ///
    /// `start` must be page-aligned (low 8 bits == 0) and `length` must be
    /// a multiple of 256. Sets page entries and adds a region descriptor.
    pub fn region(
        &mut self,
        id: RegionId,
        name: &'static str,
        start: u16,
        length: u32,
        access: AccessKind,
    ) -> &mut Self {
        debug_assert_eq!(
            start & 0xFF,
            0,
            "region start {start:#06X} must be page-aligned"
        );
        debug_assert!(
            length >= 256 || length == 0,
            "region length must be >= 256 (one full page)"
        );

        let start_page = (start >> 8) as usize;
        let page_count = length.div_ceil(256) as usize;

        for i in 0..page_count {
            let idx = start_page + i;
            if idx < 256 {
                self.pages[idx] = PageEntry {
                    region_id: id,
                    base_offset: (i as u16) * 256,
                    watch_read: false,
                    watch_write: false,
                };
            }
        }

        let end_addr = if length == 0 {
            start
        } else {
            start.wrapping_add((length - 1) as u16)
        };

        self.regions.push(RegionDescriptor {
            id,
            name,
            start_addr: start,
            end_addr,
            access,
        });

        self
    }

    /// Copy page entries from a source range to a mirror range.
    ///
    /// All three parameters must be page-aligned and `length` must be a
    /// multiple of 256. Watch flags are not copied (mirrors start clean).
    pub fn mirror(&mut self, mirror_start: u16, source_start: u16, length: u32) -> &mut Self {
        let mirror_page = (mirror_start >> 8) as usize;
        let source_page = (source_start >> 8) as usize;
        let page_count = (length / 256) as usize;

        for i in 0..page_count {
            let src = source_page + i;
            let dst = mirror_page + i;
            if src < 256 && dst < 256 {
                self.pages[dst] = PageEntry {
                    region_id: self.pages[src].region_id,
                    base_offset: self.pages[src].base_offset,
                    watch_read: false,
                    watch_write: false,
                };
            }
        }
        self
    }

    /// Remap a range of pages to a different region (for bank switching).
    ///
    /// Called at runtime when a bank register is written.
    pub fn remap_pages(
        &mut self,
        start_page: u8,
        page_count: u8,
        new_region_id: RegionId,
        new_base_offset: u16,
    ) {
        for i in 0..page_count as usize {
            let idx = start_page as usize + i;
            if idx < 256 {
                self.pages[idx].region_id = new_region_id;
                self.pages[idx].base_offset = new_base_offset + (i as u16) * 256;
            }
        }
    }

    // -----------------------------------------------------------------------
    // Dispatch helpers (called on the hot path)
    // -----------------------------------------------------------------------

    /// Look up the page entry for an address.
    #[inline(always)]
    pub fn page(&self, addr: u16) -> &PageEntry {
        // SAFETY: (addr >> 8) is always in 0..256, matching the array bounds.
        unsafe { self.pages.get_unchecked((addr >> 8) as usize) }
    }

    /// Compute the byte offset into the region for an address.
    ///
    /// This is `page.base_offset + (addr & 0xFF)` — the region-local index.
    #[inline(always)]
    pub fn region_offset(&self, addr: u16) -> usize {
        let page = self.page(addr);
        page.base_offset as usize + (addr & 0xFF) as usize
    }

    // -----------------------------------------------------------------------
    // Watchpoint methods
    // -----------------------------------------------------------------------

    /// Check for a read watchpoint hit. Returns true if the page has an
    /// active read watchpoint, setting `pending_hit`.
    ///
    /// When no watchpoints are set anywhere (`active_watch_count == 0`),
    /// this compiles to a single branch-not-taken — effectively zero cost.
    #[inline(always)]
    pub fn check_read_watch(&mut self, addr: u16, value: u8) -> bool {
        if self.active_watch_count == 0 {
            return false;
        }
        let page = &self.pages[(addr >> 8) as usize];
        if page.watch_read
            && self
                .watched_addrs
                .iter()
                .any(|&(a, k)| a == addr && k == WatchpointKind::Read)
        {
            self.pending_hit = Some(WatchpointHit {
                addr,
                kind: WatchpointKind::Read,
                value,
            });
            return true;
        }
        false
    }

    /// Check for a write watchpoint hit. Returns true if the page has an
    /// active write watchpoint, setting `pending_hit`.
    #[inline(always)]
    pub fn check_write_watch(&mut self, addr: u16, value: u8) -> bool {
        if self.active_watch_count == 0 {
            return false;
        }
        let page = &self.pages[(addr >> 8) as usize];
        if page.watch_write
            && self
                .watched_addrs
                .iter()
                .any(|&(a, k)| a == addr && k == WatchpointKind::Write)
        {
            self.pending_hit = Some(WatchpointHit {
                addr,
                kind: WatchpointKind::Write,
                value,
            });
            return true;
        }
        false
    }

    /// Consume the pending watchpoint hit (polled by debugger after each tick).
    #[inline]
    pub fn take_hit(&mut self) -> Option<WatchpointHit> {
        self.pending_hit.take()
    }

    /// True if any watchpoint is set on any page.
    #[inline]
    pub fn has_any_watchpoints(&self) -> bool {
        self.active_watch_count > 0
    }

    /// Set a watchpoint on the exact address `addr`.
    ///
    /// The page-level flag is set as a fast filter; the exact address is
    /// recorded in `watched_addrs` so only that address fires.
    pub fn set_watchpoint(&mut self, addr: u16, kind: WatchpointKind) {
        // Record exact address (avoid duplicates)
        if !self
            .watched_addrs
            .iter()
            .any(|&(a, k)| a == addr && k == kind)
        {
            self.watched_addrs.push((addr, kind));
        }
        let page = &mut self.pages[(addr >> 8) as usize];
        let was_active = page.watch_read || page.watch_write;
        match kind {
            WatchpointKind::Read => page.watch_read = true,
            WatchpointKind::Write => page.watch_write = true,
        }
        if !was_active {
            self.active_watch_count += 1;
        }
    }

    /// Clear a watchpoint on the exact address `addr`.
    ///
    /// The page-level flag is only cleared if no other watched addresses
    /// on the same page still need it.
    pub fn clear_watchpoint(&mut self, addr: u16, kind: WatchpointKind) {
        // Remove exact address entry
        self.watched_addrs
            .retain(|&(a, k)| !(a == addr && k == kind));

        // Check if any remaining entries share this page and kind
        let page_idx = (addr >> 8) as usize;
        let still_has_kind = self
            .watched_addrs
            .iter()
            .any(|&(a, k)| (a >> 8) as usize == page_idx && k == kind);

        let page = &mut self.pages[page_idx];
        let was_active = page.watch_read || page.watch_write;
        if !still_has_kind {
            match kind {
                WatchpointKind::Read => page.watch_read = false,
                WatchpointKind::Write => page.watch_write = false,
            }
        }
        let is_active = page.watch_read || page.watch_write;
        if was_active && !is_active {
            self.active_watch_count = self.active_watch_count.saturating_sub(1);
        }
    }

    /// Clear all watchpoints on all pages.
    pub fn clear_all_watchpoints(&mut self) {
        for page in &mut self.pages {
            page.watch_read = false;
            page.watch_write = false;
        }
        self.active_watch_count = 0;
        self.pending_hit = None;
        self.watched_addrs.clear();
    }

    // -----------------------------------------------------------------------
    // Introspection (for debugger / memory viewer)
    // -----------------------------------------------------------------------

    /// Get all named region descriptors.
    pub fn regions(&self) -> &[RegionDescriptor] {
        &self.regions
    }

    /// Get the region descriptor for the page containing `addr`, if mapped.
    pub fn region_at(&self, addr: u16) -> Option<&RegionDescriptor> {
        let id = self.page(addr).region_id;
        if id == UNMAPPED {
            return None;
        }
        self.regions.iter().find(|r| r.id == id)
    }
}

impl Default for MemoryMap {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    const RAM: RegionId = 1;
    const ROM: RegionId = 2;
    const IO: RegionId = 3;

    #[test]
    fn new_map_is_all_unmapped() {
        let map = MemoryMap::new();
        for page_idx in 0..=255u8 {
            let addr = (page_idx as u16) << 8;
            assert_eq!(map.page(addr).region_id, UNMAPPED);
        }
        assert!(map.regions().is_empty());
    }

    #[test]
    fn region_populates_pages_and_descriptor() {
        let mut map = MemoryMap::new();
        map.region(RAM, "RAM", 0x0000, 0x8000, AccessKind::ReadWrite);

        // Pages 0x00..0x7F should be RAM
        for page_idx in 0x00..0x80u8 {
            let addr = (page_idx as u16) << 8;
            let entry = map.page(addr);
            assert_eq!(entry.region_id, RAM);
            assert_eq!(entry.base_offset, (page_idx as u16) * 256);
        }

        // Pages 0x80..0xFF should still be unmapped
        for page_idx in 0x80..=0xFFu8 {
            let addr = (page_idx as u16) << 8;
            assert_eq!(map.page(addr).region_id, UNMAPPED);
        }

        assert_eq!(map.regions().len(), 1);
        assert_eq!(map.regions()[0].name, "RAM");
        assert_eq!(map.regions()[0].start_addr, 0x0000);
        assert_eq!(map.regions()[0].end_addr, 0x7FFF);
    }

    #[test]
    fn region_offset_calculation() {
        let mut map = MemoryMap::new();
        map.region(ROM, "ROM", 0xD000, 0x3000, AccessKind::ReadOnly);

        // Address 0xD000 → page 0xD0, base_offset=0, low byte=0x00 → offset 0
        assert_eq!(map.region_offset(0xD000), 0);
        // Address 0xD042 → page 0xD0, base_offset=0, low byte=0x42 → offset 0x42
        assert_eq!(map.region_offset(0xD042), 0x42);
        // Address 0xD100 → page 0xD1, base_offset=0x100, low byte=0x00 → offset 0x100
        assert_eq!(map.region_offset(0xD100), 0x100);
        // Address 0xFFFF → page 0xFF, base_offset=0x2F00, low byte=0xFF → offset 0x2FFF
        assert_eq!(map.region_offset(0xFFFF), 0x2FFF);
    }

    #[test]
    fn multiple_regions() {
        let mut map = MemoryMap::new();
        map.region(RAM, "RAM", 0x0000, 0x8000, AccessKind::ReadWrite)
            .region(ROM, "ROM", 0x8000, 0x8000, AccessKind::ReadOnly);

        assert_eq!(map.page(0x0000).region_id, RAM);
        assert_eq!(map.page(0x7F00).region_id, RAM);
        assert_eq!(map.page(0x8000).region_id, ROM);
        assert_eq!(map.page(0xFF00).region_id, ROM);
        assert_eq!(map.regions().len(), 2);
    }

    #[test]
    fn mirror_copies_entries() {
        let mut map = MemoryMap::new();
        map.region(RAM, "RAM", 0x0000, 0x8000, AccessKind::ReadWrite)
            .region(ROM, "ROM", 0x8000, 0x8000, AccessKind::ReadOnly);

        // Pac-Man style: mirror lower 32K to upper 32K
        // (In practice you'd mirror before setting ROM, but this tests the mechanic)
        let mut map2 = MemoryMap::new();
        map2.region(RAM, "RAM", 0x0000, 0x8000, AccessKind::ReadWrite)
            .mirror(0x8000, 0x0000, 0x8000);

        // Upper half should mirror lower half
        assert_eq!(map2.page(0x8000).region_id, RAM);
        assert_eq!(map2.page(0x8000).base_offset, 0); // same as page 0x00
        assert_eq!(map2.page(0xFF00).region_id, RAM);
        assert_eq!(map2.page(0xFF00).base_offset, map2.page(0x7F00).base_offset);
    }

    #[test]
    fn mirror_preserves_region_offsets() {
        let mut map = MemoryMap::new();
        map.region(RAM, "Work RAM", 0x4C00, 0x0400, AccessKind::ReadWrite)
            .mirror(0xCC00, 0x4C00, 0x0400);

        let src = map.page(0x4C00);
        let dst = map.page(0xCC00);
        assert_eq!(src.region_id, dst.region_id);
        assert_eq!(src.base_offset, dst.base_offset);
    }

    #[test]
    fn remap_pages_for_bank_switching() {
        let mut map = MemoryMap::new();
        map.region(RAM, "Video RAM", 0x0000, 0x9000, AccessKind::ReadWrite);

        // Bank in ROM over pages 0x00..0x90
        map.remap_pages(0x00, 0x90, ROM, 0);

        assert_eq!(map.page(0x0000).region_id, ROM);
        assert_eq!(map.page(0x0000).base_offset, 0);
        assert_eq!(map.page(0x8F00).region_id, ROM);

        // Bank back to RAM
        map.remap_pages(0x00, 0x90, RAM, 0);
        assert_eq!(map.page(0x0000).region_id, RAM);
    }

    // -----------------------------------------------------------------------
    // Watchpoint tests
    // -----------------------------------------------------------------------

    #[test]
    fn no_watchpoints_by_default() {
        let map = MemoryMap::new();
        assert!(!map.has_any_watchpoints());
        assert_eq!(map.active_watch_count, 0);
    }

    #[test]
    fn check_watch_is_noop_when_no_watchpoints() {
        let mut map = MemoryMap::new();
        map.region(RAM, "RAM", 0x0000, 0x10000, AccessKind::ReadWrite);

        assert!(!map.check_read_watch(0x1234, 0x42));
        assert!(!map.check_write_watch(0x1234, 0x42));
        assert!(map.take_hit().is_none());
    }

    #[test]
    fn read_watchpoint_fires() {
        let mut map = MemoryMap::new();
        map.region(RAM, "RAM", 0x0000, 0x10000, AccessKind::ReadWrite);

        map.set_watchpoint(0x4000, WatchpointKind::Read);
        assert!(map.has_any_watchpoints());
        assert_eq!(map.active_watch_count, 1);

        // Read at the exact watched address → hit
        assert!(map.check_read_watch(0x4000, 0xAB));
        let hit = map.take_hit().unwrap();
        assert_eq!(hit.addr, 0x4000);
        assert_eq!(hit.kind, WatchpointKind::Read);
        assert_eq!(hit.value, 0xAB);

        // Read at a different address on the same page → no hit (exact match only)
        assert!(!map.check_read_watch(0x4042, 0xCD));

        // Write at the watched address → no hit (only read watchpoint set)
        assert!(!map.check_write_watch(0x4000, 0xCD));
        assert!(map.take_hit().is_none());

        // Read in a different page → no hit
        assert!(!map.check_read_watch(0x5000, 0x00));
    }

    #[test]
    fn write_watchpoint_fires() {
        let mut map = MemoryMap::new();
        map.region(RAM, "RAM", 0x0000, 0x10000, AccessKind::ReadWrite);

        map.set_watchpoint(0x1000, WatchpointKind::Write);

        assert!(map.check_write_watch(0x1000, 0x99));
        let hit = map.take_hit().unwrap();
        assert_eq!(hit.addr, 0x1000);
        assert_eq!(hit.kind, WatchpointKind::Write);
        assert_eq!(hit.value, 0x99);

        // Read on same page → no hit
        assert!(!map.check_read_watch(0x1000, 0x00));
    }

    #[test]
    fn both_read_and_write_watchpoint() {
        let mut map = MemoryMap::new();
        map.region(RAM, "RAM", 0x0000, 0x10000, AccessKind::ReadWrite);

        map.set_watchpoint(0x2000, WatchpointKind::Read);
        map.set_watchpoint(0x2000, WatchpointKind::Write);
        // Same page, so active_watch_count should still be 1
        assert_eq!(map.active_watch_count, 1);

        assert!(map.check_read_watch(0x2000, 0x11));
        map.take_hit();
        assert!(map.check_write_watch(0x2000, 0x22));
    }

    #[test]
    fn clear_watchpoint() {
        let mut map = MemoryMap::new();
        map.region(RAM, "RAM", 0x0000, 0x10000, AccessKind::ReadWrite);

        map.set_watchpoint(0x3000, WatchpointKind::Read);
        map.set_watchpoint(0x3000, WatchpointKind::Write);
        assert_eq!(map.active_watch_count, 1);

        // Clear read but keep write
        map.clear_watchpoint(0x3000, WatchpointKind::Read);
        assert_eq!(map.active_watch_count, 1); // still active (write remains)
        assert!(!map.check_read_watch(0x3000, 0x00));
        assert!(map.check_write_watch(0x3000, 0x00));

        // Clear write too
        map.take_hit();
        map.clear_watchpoint(0x3000, WatchpointKind::Write);
        assert_eq!(map.active_watch_count, 0);
        assert!(!map.has_any_watchpoints());
    }

    #[test]
    fn clear_all_watchpoints() {
        let mut map = MemoryMap::new();
        map.region(RAM, "RAM", 0x0000, 0x10000, AccessKind::ReadWrite);

        map.set_watchpoint(0x1000, WatchpointKind::Read);
        map.set_watchpoint(0x2000, WatchpointKind::Write);
        map.set_watchpoint(0x3000, WatchpointKind::Read);
        assert_eq!(map.active_watch_count, 3);

        map.clear_all_watchpoints();
        assert_eq!(map.active_watch_count, 0);
        assert!(!map.has_any_watchpoints());
        assert!(!map.check_read_watch(0x1000, 0x00));
        assert!(!map.check_write_watch(0x2000, 0x00));
    }

    #[test]
    fn multiple_watchpoints_on_different_pages() {
        let mut map = MemoryMap::new();
        map.region(RAM, "RAM", 0x0000, 0x10000, AccessKind::ReadWrite);

        map.set_watchpoint(0x1000, WatchpointKind::Read);
        map.set_watchpoint(0x2000, WatchpointKind::Read);
        assert_eq!(map.active_watch_count, 2);

        // Only exact addresses fire
        assert!(!map.check_read_watch(0x0000, 0x00));
        assert!(map.check_read_watch(0x1000, 0x42));
        map.take_hit();
        // Different address on same page → no hit
        assert!(!map.check_read_watch(0x1050, 0x00));
    }

    #[test]
    fn watchpoint_exact_address_only() {
        let mut map = MemoryMap::new();
        map.region(RAM, "RAM", 0x0000, 0x10000, AccessKind::ReadWrite);

        map.set_watchpoint(0x2042, WatchpointKind::Write);

        // Same page, different address → no hit
        assert!(!map.check_write_watch(0x2000, 0x11));
        assert!(!map.check_write_watch(0x2041, 0x22));
        assert!(!map.check_write_watch(0x2043, 0x33));
        assert!(!map.check_write_watch(0x20FF, 0x44));

        // Exact address → hit
        assert!(map.check_write_watch(0x2042, 0x55));
        let hit = map.take_hit().unwrap();
        assert_eq!(hit.addr, 0x2042);
        assert_eq!(hit.value, 0x55);
    }

    #[test]
    fn clear_one_of_two_on_same_page() {
        let mut map = MemoryMap::new();
        map.region(RAM, "RAM", 0x0000, 0x10000, AccessKind::ReadWrite);

        map.set_watchpoint(0x3000, WatchpointKind::Read);
        map.set_watchpoint(0x3010, WatchpointKind::Read);
        assert_eq!(map.active_watch_count, 1); // same page

        // Clear the first; page flag should stay (0x3010 still active)
        map.clear_watchpoint(0x3000, WatchpointKind::Read);
        assert_eq!(map.active_watch_count, 1);
        assert!(!map.check_read_watch(0x3000, 0x00)); // cleared
        assert!(map.check_read_watch(0x3010, 0xAA)); // still active
    }

    // -----------------------------------------------------------------------
    // Introspection tests
    // -----------------------------------------------------------------------

    #[test]
    fn region_at_returns_descriptor() {
        let mut map = MemoryMap::new();
        map.region(RAM, "Work RAM", 0x0000, 0x8000, AccessKind::ReadWrite)
            .region(ROM, "Program ROM", 0xD000, 0x3000, AccessKind::ReadOnly);

        let r = map.region_at(0x1234).unwrap();
        assert_eq!(r.name, "Work RAM");
        assert_eq!(r.id, RAM);

        let r = map.region_at(0xE000).unwrap();
        assert_eq!(r.name, "Program ROM");

        // Unmapped address
        assert!(map.region_at(0xC000).is_none());
    }

    #[test]
    fn region_descriptors_have_correct_bounds() {
        let mut map = MemoryMap::new();
        map.region(RAM, "Video RAM", 0x0000, 0xC000, AccessKind::ReadWrite)
            .region(IO, "I/O", 0xC000, 0x1000, AccessKind::Io)
            .region(ROM, "ROM", 0xD000, 0x3000, AccessKind::ReadOnly);

        assert_eq!(map.regions().len(), 3);

        assert_eq!(map.regions()[0].start_addr, 0x0000);
        assert_eq!(map.regions()[0].end_addr, 0xBFFF);

        assert_eq!(map.regions()[1].start_addr, 0xC000);
        assert_eq!(map.regions()[1].end_addr, 0xCFFF);

        assert_eq!(map.regions()[2].start_addr, 0xD000);
        assert_eq!(map.regions()[2].end_addr, 0xFFFF);
    }

    #[test]
    fn last_hit_overwrites_previous() {
        let mut map = MemoryMap::new();
        map.region(RAM, "RAM", 0x0000, 0x10000, AccessKind::ReadWrite);
        map.set_watchpoint(0x1000, WatchpointKind::Read);
        map.set_watchpoint(0x1001, WatchpointKind::Read);

        map.check_read_watch(0x1000, 0xAA);
        map.check_read_watch(0x1001, 0xBB); // overwrites previous

        let hit = map.take_hit().unwrap();
        assert_eq!(hit.addr, 0x1001);
        assert_eq!(hit.value, 0xBB);
    }
}
