//! Flat byte-addressable heap for the HolyC tree-walk interpreter.
//!
//! # Design
//!
//! The heap is a single `Vec<u8>` with a simple free-list allocator on top.
//! All HolyC pointer values are `usize` offsets into this vector (1-based;
//! 0 is the null pointer and always invalid).
//!
//! This matches TempleOS semantics well enough for the interpreter tier:
//!   - `MAlloc(n)` → returns a non-zero `Ptr(offset)` or errors on OOM.
//!   - `Free(ptr)` → marks the block as reclaimable.
//!   - `*ptr` dereference / `ptr[i]` subscript → reads bytes from the buffer.
//!   - String literals → stored as null-terminated UTF-8; `%s` reads them back.
//!
//! The heap is intentionally **not** thread-safe; each [`Interpreter`] owns
//! its own `Heap`.

use crate::runtime::RuntimeError;

// ── Block header ──────────────────────────────────────────────────────────────

/// Every allocation is preceded by an 8-byte header in the heap buffer.
#[derive(Debug, Clone, Copy)]
struct Header {
    /// Size of the user data region (bytes), NOT including this header.
    size: u32,
    /// `true` if the block is available for re-use.
    free: bool,
    /// Alignment padding consumed before user data starts (0–7 bytes).
    _pad: u8,
}

const HEADER_SIZE: usize = 8; // keep powers-of-two friendly

impl Header {
    fn write_to(self, buf: &mut [u8], offset: usize) {
        let bytes = (self.size as u64)
            | ((self.free as u64) << 32)
            | ((self._pad as u64) << 40);
        buf[offset..offset + 8].copy_from_slice(&bytes.to_le_bytes());
    }

    fn read_from(buf: &[u8], offset: usize) -> Self {
        let raw = u64::from_le_bytes(buf[offset..offset + 8].try_into().unwrap());
        Header {
            size: (raw & 0xFFFF_FFFF) as u32,
            free: ((raw >> 32) & 1) != 0,
            _pad: ((raw >> 40) & 0xFF) as u8,
        }
    }
}

// ── Heap ─────────────────────────────────────────────────────────────────────

/// The interpreter heap.
pub struct Heap {
    buf:      Vec<u8>,
    /// Byte offset of the first unallocated region.
    bump:     usize,
    /// Total number of live (non-freed) allocations.
    live:     usize,
}

/// Default heap size: 64 MiB — same as TempleOS's default user heap.
pub const DEFAULT_HEAP_SIZE: usize = 64 * 1024 * 1024;

/// Minimum alignment for all allocations (8 bytes, matching x86-64 ABI).
const ALIGN: usize = 8;

impl Heap {
    /// Create a new heap with `capacity` bytes.
    pub fn new(capacity: usize) -> Self {
        // Offset 0 is permanently reserved as the null pointer.
        let mut buf = vec![0u8; capacity];
        buf[0] = 0; // explicit null sentinel
        Heap { buf, bump: 1, live: 0 }
    }

    // ── Allocation ────────────────────────────────────────────────────────────

    /// Allocate `size` bytes. Returns a 1-based offset (never 0).
    ///
    /// Strategy: first-fit free list, then bump.
    pub fn alloc(&mut self, size: usize) -> Result<usize, RuntimeError> {
        if size == 0 {
            return Ok(1); // degenerate; return a valid non-null sentinel
        }
        let size = align_up(size, ALIGN);

        // First-fit through free blocks.
        if let Some(ptr) = self.find_free(size) {
            self.live += 1;
            return Ok(ptr);
        }

        // Bump allocate.
        let header_start = align_up(self.bump, ALIGN);
        let data_start   = header_start + HEADER_SIZE;
        let end          = data_start + size;

        if end > self.buf.len() {
            return Err(RuntimeError::Custom(format!(
                "MAlloc({size}): out of heap memory ({} bytes remaining)",
                self.buf.len().saturating_sub(self.bump)
            )));
        }

        Header { size: size as u32, free: false, _pad: 0 }
            .write_to(&mut self.buf, header_start);

        // Zero the user region.
        self.buf[data_start..end].fill(0);
        self.bump = end;
        self.live += 1;
        Ok(data_start)
    }

    /// Release a previously allocated block. Marks it free for reuse.
    pub fn free(&mut self, ptr: usize) -> Result<(), RuntimeError> {
        if ptr == 0 {
            return Ok(()); // free(NULL) is a no-op
        }
        let header_start = ptr - HEADER_SIZE;
        if header_start >= self.buf.len() {
            return Err(RuntimeError::Custom(format!("Free(0x{ptr:x}): invalid pointer")));
        }
        let mut h = Header::read_from(&self.buf, header_start);
        if h.free {
            return Err(RuntimeError::Custom(format!("Free(0x{ptr:x}): double-free detected")));
        }
        h.free = true;
        h.write_to(&mut self.buf, header_start);
        self.live = self.live.saturating_sub(1);
        Ok(())
    }

    // ── Memory access ─────────────────────────────────────────────────────────

    /// Read `width` bytes at `ptr` as a little-endian unsigned integer.
    pub fn read_uint(&self, ptr: usize, width: usize) -> Result<u64, RuntimeError> {
        self.check_bounds(ptr, width)?;
        let mut val = 0u64;
        for i in 0..width {
            val |= (self.buf[ptr + i] as u64) << (i * 8);
        }
        Ok(val)
    }

    /// Write `val` (little-endian) as `width` bytes at `ptr`.
    pub fn write_uint(&mut self, ptr: usize, width: usize, val: u64) -> Result<(), RuntimeError> {
        self.check_bounds(ptr, width)?;
        for i in 0..width {
            self.buf[ptr + i] = ((val >> (i * 8)) & 0xFF) as u8;
        }
        Ok(())
    }

    /// Read a null-terminated C string from `ptr`.
    pub fn read_cstr(&self, ptr: usize) -> Result<String, RuntimeError> {
        if ptr == 0 {
            return Err(RuntimeError::Custom("null pointer dereference in string read".into()));
        }
        let start = ptr;
        let mut end = start;
        while end < self.buf.len() && self.buf[end] != 0 {
            end += 1;
        }
        if end >= self.buf.len() {
            return Err(RuntimeError::Custom(format!(
                "string at 0x{ptr:x} not null-terminated within heap bounds"
            )));
        }
        String::from_utf8(self.buf[start..end].to_vec())
            .map_err(|_| RuntimeError::Custom(format!("invalid UTF-8 string at 0x{ptr:x}")))
    }

    /// Intern a Rust `&str` into the heap as a null-terminated C string.
    /// Returns the pointer to the first byte.
    pub fn intern_str(&mut self, s: &str) -> Result<usize, RuntimeError> {
        let bytes = s.as_bytes();
        let ptr   = self.alloc(bytes.len() + 1)?; // +1 for NUL
        self.buf[ptr..ptr + bytes.len()].copy_from_slice(bytes);
        self.buf[ptr + bytes.len()] = 0;
        Ok(ptr)
    }

    /// Zero `len` bytes starting at `ptr`.
    pub fn memset(&mut self, ptr: usize, val: u8, len: usize) -> Result<(), RuntimeError> {
        self.check_bounds(ptr, len)?;
        self.buf[ptr..ptr + len].fill(val);
        Ok(())
    }

    /// Copy `len` bytes from `src` to `dst` (may overlap — uses copy_within).
    pub fn memcpy(&mut self, dst: usize, src: usize, len: usize) -> Result<(), RuntimeError> {
        self.check_bounds(src, len)?;
        self.check_bounds(dst, len)?;
        self.buf.copy_within(src..src + len, dst);
        Ok(())
    }

    // ── Diagnostics ───────────────────────────────────────────────────────────

    /// How many bytes have been bump-allocated (includes headers + free blocks).
    pub fn used_bytes(&self) -> usize { self.bump }

    /// Total heap capacity.
    pub fn capacity(&self) -> usize { self.buf.len() }

    /// Number of live (non-freed) allocations.
    pub fn live_allocs(&self) -> usize { self.live }

    // ── Private helpers ───────────────────────────────────────────────────────

    fn check_bounds(&self, ptr: usize, len: usize) -> Result<(), RuntimeError> {
        if ptr == 0 {
            return Err(RuntimeError::Custom("null pointer dereference".into()));
        }
        if len == 0 {
            return Ok(());
        }
        let end = ptr.checked_add(len).ok_or_else(|| {
            RuntimeError::Custom("pointer arithmetic overflow".into())
        })?;
        if end > self.buf.len() {
            Err(RuntimeError::Custom(format!(
                "out-of-bounds heap access: ptr=0x{ptr:x} len={len} heap_size={}",
                self.buf.len()
            )))
        } else {
            Ok(())
        }
    }

    /// Walk existing blocks looking for a free block of at least `size` bytes.
    fn find_free(&mut self, size: usize) -> Option<usize> {
        let mut pos = 1usize; // skip null sentinel byte

        while pos + HEADER_SIZE <= self.bump {
            let h_off = align_up(pos, ALIGN);
            if h_off + HEADER_SIZE > self.bump { break; }

            let h = Header::read_from(&self.buf, h_off);
            let data_off = h_off + HEADER_SIZE;

            if h.free && (h.size as usize) >= size {
                // Mark as used.
                let mut h2 = h;
                h2.free = false;
                h2.write_to(&mut self.buf, h_off);
                self.buf[data_off..data_off + size].fill(0);
                return Some(data_off);
            }

            // Advance past this block.
            pos = data_off + (h.size as usize);
        }
        None
    }
}

// ── Utility ───────────────────────────────────────────────────────────────────

#[inline]
fn align_up(val: usize, align: usize) -> usize {
    (val + align - 1) & !(align - 1)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn small() -> Heap { Heap::new(4096) }

    #[test]
    fn alloc_non_null() {
        let mut h = small();
        let p = h.alloc(8).unwrap();
        assert_ne!(p, 0, "alloc must never return null");
    }

    #[test]
    fn alloc_zeroed() {
        let mut h = small();
        let p = h.alloc(16).unwrap();
        for i in 0..16 {
            assert_eq!(h.buf[p + i], 0, "fresh allocation must be zeroed");
        }
    }

    #[test]
    fn alloc_non_overlapping() {
        let mut h = small();
        let a = h.alloc(16).unwrap();
        let b = h.alloc(16).unwrap();
        // Ranges must not overlap.
        let a_end = a + 16;
        let b_end = b + 16;
        let overlap = a.max(b) < a_end.min(b_end);
        assert!(!overlap, "allocations a=[{a},{a_end}) and b=[{b},{b_end}) overlap");
    }

    #[test]
    fn read_write_uint() {
        let mut h = small();
        let p = h.alloc(8).unwrap();
        h.write_uint(p, 8, 0xDEAD_BEEF_CAFE_1234).unwrap();
        let v = h.read_uint(p, 8).unwrap();
        assert_eq!(v, 0xDEAD_BEEF_CAFE_1234);
    }

    #[test]
    fn intern_and_read_str() {
        let mut h = small();
        let p = h.intern_str("hello").unwrap();
        let s = h.read_cstr(p).unwrap();
        assert_eq!(s, "hello");
    }

    #[test]
    fn free_and_reuse() {
        let mut h = small();
        let p = h.alloc(32).unwrap();
        let bump_before = h.used_bytes();
        h.free(p).unwrap();
        let q = h.alloc(32).unwrap();
        // Must reuse the freed block, so bump doesn't advance.
        assert_eq!(
            h.used_bytes(), bump_before,
            "reused free block should not advance bump pointer"
        );
        assert_ne!(q, 0);
    }

    #[test]
    fn double_free_errors() {
        let mut h = small();
        let p = h.alloc(8).unwrap();
        h.free(p).unwrap();
        assert!(h.free(p).is_err(), "double-free must return an error");
    }

    #[test]
    fn null_deref_errors() {
        let h = small();
        assert!(h.read_uint(0, 8).is_err(), "null dereference must error");
    }

    #[test]
    fn out_of_bounds_errors() {
        let h = Heap::new(64);
        assert!(h.read_uint(60, 8).is_err(), "OOB read must error");
    }

    #[test]
    fn memset_works() {
        let mut h = small();
        let p = h.alloc(16).unwrap();
        h.memset(p, 0xFF, 16).unwrap();
        for i in 0..16 {
            assert_eq!(h.buf[p + i], 0xFF);
        }
    }

    #[test]
    fn memcpy_works() {
        let mut h = small();
        let src = h.alloc(8).unwrap();
        let dst = h.alloc(8).unwrap();
        h.write_uint(src, 8, 0x1122_3344_5566_7788).unwrap();
        h.memcpy(dst, src, 8).unwrap();
        assert_eq!(h.read_uint(dst, 8).unwrap(), 0x1122_3344_5566_7788);
    }

    #[test]
    fn oom_returns_error() {
        let mut h = Heap::new(32);
        assert!(h.alloc(256).is_err(), "OOM must return an error");
    }

    #[test]
    fn live_allocs_tracked() {
        let mut h = small();
        assert_eq!(h.live_allocs(), 0);
        let p = h.alloc(8).unwrap();
        assert_eq!(h.live_allocs(), 1);
        let _ = h.alloc(8).unwrap();
        assert_eq!(h.live_allocs(), 2);
        h.free(p).unwrap();
        assert_eq!(h.live_allocs(), 1);
    }
}
