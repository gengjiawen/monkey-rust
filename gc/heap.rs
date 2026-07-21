use crate::header::{GcId, GcObjectType, GcPhase};
use crate::malloc::MallocState;
use crate::report::{GcPhaseStats, GcStatsBundle};
use crate::runtime::{GcObject, GcRuntime, MarkFunc};

/// Opaque handle to a GC-managed object.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct GcRef(pub GcId);

/// High-level heap API wrapping QuickJS-style `GcRuntime`.
pub struct GcHeap {
    rt: GcRuntime,
}

impl GcHeap {
    pub fn new() -> Self {
        GcHeap {
            rt: GcRuntime::new(),
        }
    }

    pub fn malloc_state(&self) -> &MallocState {
        self.rt.malloc_state()
    }

    pub fn malloc_state_mut(&mut self) -> &mut MallocState {
        self.rt.malloc_state_mut()
    }

    pub fn gc_threshold(&self) -> usize {
        self.rt.gc_threshold()
    }

    pub fn set_gc_threshold(&mut self, threshold: usize) {
        self.rt.set_gc_threshold(threshold);
    }

    pub fn gc_phase(&self) -> GcPhase {
        self.rt.gc_phase()
    }

    pub fn alloc<O: GcObject>(&mut self, object: O, ty: GcObjectType) -> GcRef {
        self.trigger_gc(std::mem::size_of::<O>());
        GcRef(self.rt.add_gc_object(Box::new(object), ty))
    }

    pub fn dup(&mut self, reference: GcRef) -> GcRef {
        GcRef(self.rt.dup_gc(reference.0))
    }

    pub fn free(&mut self, reference: GcRef) {
        self.rt.free_gc(reference.0);
    }

    pub fn run_gc(&mut self) {
        self.rt.run_gc();
    }

    pub fn run_gc_with_stats(&mut self) -> GcPhaseStats {
        self.rt.run_gc_with_stats()
    }

    pub fn run_gc_with_stats_bundle(&mut self) -> GcStatsBundle {
        self.rt.run_gc_with_stats_bundle()
    }

    pub fn trigger_gc(&mut self, alloc_size: usize) {
        self.rt.trigger_gc(alloc_size);
    }

    pub fn is_live(&self, reference: GcRef) -> bool {
        self.rt.is_live_object(reference.0)
    }

    pub fn exists(&self, reference: GcRef) -> bool {
        self.rt.object_exists(reference.0)
    }

    pub fn ref_count(&self, reference: GcRef) -> i32 {
        self.rt.ref_count(reference.0)
    }

    pub fn mark_children(&mut self, reference: GcRef, mark_func: MarkFunc) {
        self.rt.mark_children(reference.0, mark_func);
    }

    #[cfg(test)]
    pub(crate) fn header_mut(&mut self, reference: GcRef) -> &mut crate::header::GcObjectHeader {
        self.rt.header_mut(reference.0)
    }

    pub fn runtime(&self) -> &GcRuntime {
        &self.rt
    }

    pub fn runtime_mut(&mut self) -> &mut GcRuntime {
        &mut self.rt
    }
}

impl Default for GcHeap {
    fn default() -> Self {
        Self::new()
    }
}
