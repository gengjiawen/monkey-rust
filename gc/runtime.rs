use crate::header::{
    GcId, GcListKind, GcObjectHeader, GcObjectType, GcPhase, RefCountHeader, RefCountId,
};
use crate::list::{GcList, GcListIter};
use crate::malloc::{MallocState, DEFAULT_GC_THRESHOLD};
use std::any::Any;

/// Child-mark callback mode used during the three GC phases.
/// Matches QuickJS `gc_decref_child`, `gc_scan_incref_child`, `gc_scan_incref_child2`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarkFunc {
    Decref,
    ScanIncref,
    ScanIncref2,
}

/// Trait for objects stored in the GC heap. Implementors expose graph edges via `trace`.
pub trait GcObject: Any {
    fn trace(&self, visit: &mut dyn FnMut(GcId));
    fn on_free(&mut self, _rt: &mut GcRuntime) {}
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

struct GcObjectEntry {
    header: GcObjectHeader,
    object: Option<Box<dyn GcObject>>,
}

struct RefCountEntry {
    header: RefCountHeader,
    payload: Box<dyn FnOnce(&mut GcRuntime)>,
}

/// QuickJS-style runtime heap: reference counting + three-phase cycle removal.
pub struct GcRuntime {
    objects: Vec<Option<GcObjectEntry>>,
    free_slots: Vec<GcId>,
    ref_counts: Vec<Option<RefCountEntry>>,
    ref_count_free_slots: Vec<RefCountId>,

    gc_obj_list: GcList,
    gc_zero_ref_count_list: GcList,
    tmp_obj_list: GcList,
    gc_phase: GcPhase,
    malloc_state: MallocState,
    malloc_gc_threshold: usize,
}

impl GcRuntime {
    pub fn new() -> Self {
        GcRuntime {
            objects: Vec::new(),
            free_slots: Vec::new(),
            ref_counts: Vec::new(),
            ref_count_free_slots: Vec::new(),
            gc_obj_list: GcList::new(),
            gc_zero_ref_count_list: GcList::new(),
            tmp_obj_list: GcList::new(),
            gc_phase: GcPhase::None,
            malloc_state: MallocState::new(),
            malloc_gc_threshold: DEFAULT_GC_THRESHOLD,
        }
    }

    pub fn malloc_state(&self) -> &MallocState {
        &self.malloc_state
    }

    pub fn malloc_state_mut(&mut self) -> &mut MallocState {
        &mut self.malloc_state
    }

    pub fn gc_threshold(&self) -> usize {
        self.malloc_gc_threshold
    }

    /// `threshold == usize::MAX` disables automatic GC, matching `JS_SetGCThreshold(rt, -1)`.
    pub fn set_gc_threshold(&mut self, threshold: usize) {
        self.malloc_gc_threshold = threshold;
    }

    pub fn gc_phase(&self) -> GcPhase {
        self.gc_phase
    }

    pub fn gc_object_count(&self) -> usize {
        GcListIter {
            rt: self,
            current: self.gc_obj_list.head,
        }
        .count()
    }

    pub(crate) fn header(&self, id: GcId) -> &GcObjectHeader {
        &self.objects[id].as_ref().expect("invalid GcId").header
    }

    pub(crate) fn header_mut(&mut self, id: GcId) -> &mut GcObjectHeader {
        &mut self.objects[id].as_mut().expect("invalid GcId").header
    }

    fn list_ptr(&mut self, kind: GcListKind) -> *mut GcList {
        (match kind {
            GcListKind::GcObj => &mut self.gc_obj_list,
            GcListKind::Tmp => &mut self.tmp_obj_list,
            GcListKind::ZeroRef => &mut self.gc_zero_ref_count_list,
        }) as *mut GcList
    }

    fn list_push_back(&mut self, kind: GcListKind, id: GcId) {
        let list_ptr = self.list_ptr(kind);
        let tail = unsafe { (*list_ptr).tail };

        {
            let header = self.header_mut(id);
            assert!(
                header.list_kind.is_none(),
                "object already belongs to a GC list: {:?}",
                header.list_kind
            );
            header.list_kind = Some(kind);
            header.list_prev = tail;
            header.list_next = None;
        }

        if let Some(tail_id) = tail {
            self.header_mut(tail_id).list_next = Some(id);
        } else {
            unsafe {
                (*list_ptr).head = Some(id);
            }
        }
        unsafe {
            (*list_ptr).tail = Some(id);
        }
    }

    fn list_remove(&mut self, kind: GcListKind, id: GcId) {
        assert_eq!(self.header(id).list_kind, Some(kind), "object is not on the expected GC list");

        let list_ptr = self.list_ptr(kind);
        let (prev, next) = {
            let header = self.header(id);
            (header.list_prev, header.list_next)
        };

        match prev {
            Some(p) => self.header_mut(p).list_next = next,
            None => unsafe {
                (*list_ptr).head = next;
            },
        }

        match next {
            Some(n) => self.header_mut(n).list_prev = prev,
            None => unsafe {
                (*list_ptr).tail = prev;
            },
        }

        let header = self.header_mut(id);
        header.list_kind = None;
        header.list_prev = None;
        header.list_next = None;
    }

    fn list_remove_current(&mut self, id: GcId) {
        let kind = self
            .header(id)
            .list_kind
            .expect("object is not on a GC list");
        self.list_remove(kind, id);
    }

    fn list_move(&mut self, from: GcListKind, to: GcListKind, id: GcId) {
        self.list_remove(from, id);
        self.list_push_back(to, id);
    }

    fn list_move_current_to(&mut self, to: GcListKind, id: GcId) {
        let from = self
            .header(id)
            .list_kind
            .expect("object is not on a GC list");
        self.list_move(from, to, id);
    }

    fn alloc_slot(&mut self, entry: GcObjectEntry) -> GcId {
        let id = if let Some(id) = self.free_slots.pop() {
            self.objects[id] = Some(entry);
            id
        } else {
            let id = self.objects.len();
            self.objects.push(Some(entry));
            id
        };
        self.malloc_state
            .record_alloc(std::mem::size_of::<GcObjectEntry>());
        id
    }

    fn free_slot(&mut self, id: GcId) {
        self.malloc_state
            .record_free(std::mem::size_of::<GcObjectEntry>());
        self.objects[id] = None;
        self.free_slots.push(id);
    }

    /// Register a new GC object with `ref_count = 1` on `gc_obj_list`.
    /// Matches QuickJS `add_gc_object`.
    pub fn add_gc_object(&mut self, object: Box<dyn GcObject>, gc_obj_type: GcObjectType) -> GcId {
        let id = self.alloc_slot(GcObjectEntry {
            header: GcObjectHeader::new(gc_obj_type, 1),
            object: Some(object),
        });
        self.list_push_back(GcListKind::GcObj, id);
        id
    }

    /// Increment refcount. Matches QuickJS `js_dup` for GC objects.
    pub fn dup_gc(&mut self, id: GcId) -> GcId {
        self.header_mut(id).ref_count += 1;
        id
    }

    /// Decrement refcount and free when it reaches zero.
    /// Matches QuickJS `JS_FreeValueRT` for GC objects.
    pub fn free_gc(&mut self, id: GcId) {
        if !self.object_exists(id) {
            return;
        }
        let ref_count = {
            let header = self.header_mut(id);
            header.ref_count -= 1;
            header.ref_count
        };

        if ref_count > 0 {
            return;
        }

        if self.gc_phase != GcPhase::RemoveCycles {
            self.list_move(GcListKind::GcObj, GcListKind::ZeroRef, id);
            if self.gc_phase == GcPhase::None {
                self.free_zero_refcount();
            }
        }
    }

    /// Allocate a simple refcounted payload (strings, etc.). Not cycle-collected.
    pub fn add_ref_counted<F>(&mut self, on_free: F) -> RefCountId
    where
        F: FnOnce(&mut GcRuntime) + 'static,
    {
        let entry = RefCountEntry {
            header: RefCountHeader::new(1),
            payload: Box::new(on_free),
        };
        let id = if let Some(id) = self.ref_count_free_slots.pop() {
            self.ref_counts[id] = Some(entry);
            id
        } else {
            let id = self.ref_counts.len();
            self.ref_counts.push(Some(entry));
            id
        };
        self.malloc_state
            .record_alloc(std::mem::size_of::<RefCountEntry>());
        id
    }

    pub fn dup_ref_counted(&mut self, id: RefCountId) -> RefCountId {
        self.ref_counts[id]
            .as_mut()
            .expect("invalid RefCountId")
            .header
            .ref_count += 1;
        id
    }

    pub fn free_ref_counted(&mut self, id: RefCountId) {
        let ref_count = {
            let entry = self.ref_counts[id].as_mut().expect("invalid RefCountId");
            entry.header.ref_count -= 1;
            entry.header.ref_count
        };

        if ref_count <= 0 {
            let entry = self.ref_counts[id].take().expect("double free");
            self.malloc_state
                .record_free(std::mem::size_of::<RefCountEntry>());
            (entry.payload)(self);
            self.ref_count_free_slots.push(id);
        }
    }

    /// Mark a GC object header during traversal. Matches `JS_MarkValue` for object tags.
    pub fn mark_gc_header(&mut self, id: GcId, mark_func: MarkFunc) {
        match mark_func {
            MarkFunc::Decref => self.gc_decref_child(id),
            MarkFunc::ScanIncref => self.gc_scan_incref_child(id),
            MarkFunc::ScanIncref2 => self.gc_scan_incref_child2(id),
        }
    }

    /// Traverse children of a GC object. Matches QuickJS `mark_children`.
    pub fn mark_children(&mut self, id: GcId, mark_func: MarkFunc) {
        // Move the box out temporarily so `trace` can call back into the runtime without
        // materializing a child-id buffer.
        let object = self.objects[id]
            .as_mut()
            .expect("invalid GcId")
            .object
            .take()
            .expect("object already finalized");
        object.trace(&mut |child| {
            self.mark_gc_header(child, mark_func);
        });
        self.objects[id]
            .as_mut()
            .expect("object freed while tracing")
            .object = Some(object);
    }

    /// Maybe run GC when tracked malloc exceeds threshold. Matches `js_trigger_gc`.
    pub fn trigger_gc(&mut self, alloc_size: usize) {
        let force_gc =
            self.malloc_state.malloc_size.saturating_add(alloc_size) > self.malloc_gc_threshold;
        if force_gc {
            self.run_gc();
            self.malloc_gc_threshold =
                self.malloc_state.malloc_size + (self.malloc_state.malloc_size >> 1);
        }
    }

    /// Run the three-phase cycle collector. Matches `JS_RunGC`.
    pub fn run_gc(&mut self) {
        self.gc_decref();
        self.gc_scan();
        self.gc_free_cycles();
    }

    /// Return false if the object has been freed during cycle collection.
    /// Matches `JS_IsLiveObject`.
    pub fn is_live_object(&self, id: GcId) -> bool {
        self.objects
            .get(id)
            .and_then(|o| o.as_ref())
            .map_or(false, |e| !e.header.free_mark)
    }

    pub fn ref_count(&self, id: GcId) -> i32 {
        self.header(id).ref_count
    }

    pub fn object_exists(&self, id: GcId) -> bool {
        self.objects.get(id).and_then(|o| o.as_ref()).is_some()
    }

    pub fn object_downcast<T: GcObject>(&self, id: GcId) -> Option<&T> {
        let entry = self.objects.get(id)?.as_ref()?;
        entry.object.as_ref()?.as_ref().as_any().downcast_ref()
    }

    pub fn object_downcast_mut<T: GcObject>(&mut self, id: GcId) -> Option<&mut T> {
        let entry = self.objects.get_mut(id)?.as_mut()?;
        entry.object.as_mut()?.as_mut().as_any_mut().downcast_mut()
    }

    // --- Phase 1: trial deletion ---

    fn gc_decref_child(&mut self, id: GcId) {
        assert!(self.header(id).ref_count > 0);
        self.header_mut(id).ref_count -= 1;
        if self.header(id).ref_count == 0 && self.header(id).mark == 1 {
            self.list_move(GcListKind::GcObj, GcListKind::Tmp, id);
        }
    }

    fn gc_decref(&mut self) {
        assert!(
            self.tmp_obj_list.is_empty(),
            "temporary GC list must be empty before trial deletion"
        );
        let mut current = self.gc_obj_list.head;
        while let Some(id) = current {
            let next = self.header(id).list_next;
            assert_eq!(self.header(id).mark, 0);
            self.mark_children(id, MarkFunc::Decref);
            self.header_mut(id).mark = 1;
            if self.header(id).ref_count == 0 {
                self.list_move(GcListKind::GcObj, GcListKind::Tmp, id);
            }
            current = next;
        }
    }

    // --- Phase 2: restore live refs ---

    fn gc_scan_incref_child(&mut self, id: GcId) {
        self.header_mut(id).ref_count += 1;
        if self.header(id).ref_count == 1 {
            self.list_move(GcListKind::Tmp, GcListKind::GcObj, id);
            self.header_mut(id).mark = 0;
        }
    }

    fn gc_scan_incref_child2(&mut self, id: GcId) {
        self.header_mut(id).ref_count += 1;
    }

    fn gc_scan(&mut self) {
        let mut current = self.gc_obj_list.head;
        while let Some(id) = current {
            assert!(self.header(id).ref_count > 0);
            self.header_mut(id).mark = 0;
            self.mark_children(id, MarkFunc::ScanIncref);
            current = self.header(id).list_next;
        }

        let mut current = self.tmp_obj_list.head;
        while let Some(id) = current {
            let next = self.header(id).list_next;
            self.mark_children(id, MarkFunc::ScanIncref2);
            current = next;
        }
    }

    // --- Phase 3: free cyclic garbage ---

    fn gc_free_cycles(&mut self) {
        self.gc_phase = GcPhase::RemoveCycles;

        loop {
            let id = self.tmp_obj_list.head;
            if id.is_none() {
                break;
            }
            let id = id.unwrap();
            let gc_obj_type = self.header(id).gc_obj_type;
            match gc_obj_type {
                GcObjectType::MonkeyObject | GcObjectType::FunctionBytecode => {
                    self.free_gc_object(id);
                }
                _ => {
                    self.list_move(GcListKind::Tmp, GcListKind::ZeroRef, id);
                }
            }
        }

        self.gc_phase = GcPhase::None;

        while let Some(id) = self.gc_zero_ref_count_list.head {
            let ty = self.header(id).gc_obj_type;
            assert!(
                ty == GcObjectType::MonkeyObject || ty == GcObjectType::FunctionBytecode,
                "unexpected deferred type: {:?}",
                ty
            );
            self.list_remove_current(id);
            self.free_slot(id);
        }
    }

    fn free_zero_refcount(&mut self) {
        self.gc_phase = GcPhase::Decref;
        loop {
            let id = self.gc_zero_ref_count_list.head;
            if id.is_none() {
                break;
            }
            let id = id.unwrap();
            assert_eq!(self.header(id).ref_count, 0);
            self.free_gc_object(id);
        }
        self.gc_phase = GcPhase::None;
    }

    fn free_gc_object(&mut self, id: GcId) {
        match self.header(id).gc_obj_type {
            GcObjectType::MonkeyObject | GcObjectType::FunctionBytecode => {
                self.free_heap_object(id)
            }
            other => panic!("free_gc_object: unsupported type {:?}", other),
        }
    }

    fn free_heap_object(&mut self, id: GcId) {
        self.header_mut(id).free_mark = true;

        // Process outgoing edges as `trace` reports them; GC freeing should not allocate
        // a child snapshot.
        let mut object = self.objects[id]
            .as_mut()
            .expect("invalid GcId")
            .object
            .take()
            .expect("object already finalized");
        object.trace(&mut |child| {
            self.free_gc(child);
        });
        object.on_free(self);
        drop(object);

        let defer_free = self.gc_phase == GcPhase::RemoveCycles && self.header(id).ref_count != 0;
        if !defer_free {
            self.list_remove_current(id);
            self.free_slot(id);
        } else {
            self.list_move_current_to(GcListKind::ZeroRef, id);
        }
    }
}

impl Default for GcRuntime {
    fn default() -> Self {
        Self::new()
    }
}
