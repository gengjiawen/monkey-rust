/// Default GC trigger threshold, matching QuickJS `JS_NewRuntime2`.
pub const DEFAULT_GC_THRESHOLD: usize = 256 * 1024;

/// Per-allocation accounting overhead, matching QuickJS `MALLOC_OVERHEAD` on non-Apple platforms.
pub const MALLOC_OVERHEAD: usize = 8;

/// Tracked heap usage, matching QuickJS `JSMallocState`.
#[derive(Debug, Clone, Default)]
pub struct MallocState {
    pub malloc_count: usize,
    pub malloc_size: usize,
    pub malloc_limit: usize,
}

impl MallocState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_limit(&mut self, limit: usize) {
        self.malloc_limit = limit;
    }

    pub fn would_exceed(&self, size: usize) -> bool {
        if self.malloc_limit == 0 {
            return false;
        }
        self.malloc_size.saturating_add(size) > self.malloc_limit.saturating_sub(1)
    }

    pub fn record_alloc(&mut self, usable_size: usize) {
        self.malloc_count += 1;
        self.malloc_size += usable_size + MALLOC_OVERHEAD;
    }

    pub fn record_free(&mut self, usable_size: usize) {
        self.malloc_count = self.malloc_count.saturating_sub(1);
        self.malloc_size = self
            .malloc_size
            .saturating_sub(usable_size + MALLOC_OVERHEAD);
    }
}
