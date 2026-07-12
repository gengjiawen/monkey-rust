use crate::header::{
    GcId, GcListKind, GcObjectHeader, GcObjectType, GcPhase, RefCountHeader, RefCountId,
};
use crate::list::{GcList, GcListIter};
use crate::malloc::{MallocState, DEFAULT_GC_THRESHOLD};
use crate::report::{
    summarize_gc_object, summarize_gc_objects, FinalFate, FreeCycleStats, GcPhaseStats,
    GcStatsBundle, ObjectDecision, RestorationWitness, ScanStats, TrialDecision,
    TrialDeletionStats, VisitedEdge, MAX_EDGE_DETAILS, MAX_OBJECT_DECISIONS,
    MAX_RESTORATION_WITNESSES,
};
use crate::value::{EdgeRelation, ValueCell};
use std::any::Any;
use std::collections::{HashMap, HashSet, VecDeque};

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

    pub fn object_ids(&self) -> Vec<GcId> {
        self.objects
            .iter()
            .enumerate()
            .filter_map(|(id, entry)| entry.as_ref().map(|_| id))
            .collect()
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

    /// Run all collector phases atomically and return read-only telemetry.
    pub fn run_gc_with_stats(&mut self) -> GcStatsBundle {
        // 1. Capture immutable graph + RC before trial deletion.
        let live_ids = self.sorted_list_ids(GcListKind::GcObj);
        let mut ref_count_before = HashMap::with_capacity(live_ids.len());
        for &id in &live_ids {
            ref_count_before.insert(id, self.header(id).ref_count);
        }
        let all_edges = self.capture_visited_edges(&live_ids);
        let edges_visited = all_edges.len();
        debug_assert_eq!(
            edges_visited,
            self.gc_edge_count(),
            "semantic edge capture must match collector trace edge count"
        );
        let mut heap_incoming = HashMap::<GcId, usize>::with_capacity(live_ids.len());
        for edge in &all_edges {
            *heap_incoming.entry(edge.to_id).or_default() += 1;
        }
        let incoming_sum: usize = heap_incoming.values().sum();
        debug_assert_eq!(
            incoming_sum, edges_visited,
            "Σ heapIncomingEdges must equal edgesVisited"
        );

        // 2. Trial deletion.
        self.gc_decref();
        let candidate_ids = self.sorted_list_ids(GcListKind::Tmp);
        let candidate_set: HashSet<GcId> = candidate_ids.iter().copied().collect();
        let mut trial_ref_counts = HashMap::with_capacity(live_ids.len());
        for &id in &live_ids {
            let trial_rc = self.header(id).ref_count;
            let before = ref_count_before[&id];
            let incoming = heap_incoming.get(&id).copied().unwrap_or(0);
            debug_assert_eq!(
                trial_rc,
                before - incoming as i32,
                "trialRefCount must equal refCountBefore − heapIncomingEdges"
            );
            debug_assert_eq!(
                candidate_set.contains(&id),
                trial_rc == 0,
                "Candidate ⇔ trialRefCount == 0"
            );
            trial_ref_counts.insert(id, trial_rc);
        }

        // 3. Scan + witness forest on the immutable captured graph.
        self.gc_scan();
        let garbage_candidate_ids = self.sorted_list_ids(GcListKind::Tmp);
        let garbage_set: HashSet<GcId> = garbage_candidate_ids.iter().copied().collect();
        let restored_ids = candidate_ids
            .iter()
            .copied()
            .filter(|id| !garbage_set.contains(id))
            .collect::<Vec<_>>();
        debug_assert_eq!(
            restored_ids.len() + garbage_candidate_ids.len(),
            candidate_ids.len(),
            "Candidates = Restored + Garbage candidates"
        );
        let restored_objects = summarize_gc_objects(self, &restored_ids);
        let garbage_candidate_objects = summarize_gc_objects(self, &garbage_candidate_ids);
        let all_witnesses =
            self.build_restoration_witnesses(&all_edges, &trial_ref_counts, &restored_ids);

        // Capture labels for every live object before free cycles reclaim garbage.
        let label_by_id: HashMap<GcId, crate::report::GcObjectSummary> = live_ids
            .iter()
            .copied()
            .map(|id| (id, summarize_gc_object(self, id)))
            .collect();

        // 4. Free cycles + final fate.
        let before_free = self.object_ids().len();
        self.gc_free_cycles();
        let freed = before_free.saturating_sub(self.object_ids().len());
        debug_assert_eq!(freed, garbage_candidate_ids.len(), "Objects freed = Garbage candidates");

        let survivor_ids: Vec<GcId> = live_ids
            .iter()
            .copied()
            .filter(|id| !candidate_set.contains(id))
            .collect();

        let (object_decisions, omitted_object_decisions, selected_decision_ids) =
            select_object_decisions(
                &live_ids,
                &candidate_set,
                &survivor_ids,
                &all_witnesses,
                &ref_count_before,
                &heap_incoming,
                &trial_ref_counts,
                &garbage_set,
            );

        let (visited_edges, omitted_edge_details) =
            select_visited_edges(&all_edges, &candidate_set, &all_witnesses);

        let (restoration_witnesses, omitted_witnesses) =
            select_restoration_witnesses(&all_witnesses, &selected_decision_ids);

        let mut catalog_ids: HashSet<GcId> = HashSet::new();
        for decision in &object_decisions {
            catalog_ids.insert(decision.object_id);
        }
        for edge in &visited_edges {
            catalog_ids.insert(edge.from_id);
            catalog_ids.insert(edge.to_id);
        }
        for witness in &restoration_witnesses {
            catalog_ids.insert(witness.object_id);
            catalog_ids.insert(witness.root_id);
            catalog_ids.insert(witness.predecessor_id);
        }
        for object in restored_objects
            .iter()
            .chain(garbage_candidate_objects.iter())
        {
            catalog_ids.insert(object.id);
        }
        let mut catalog_ids = catalog_ids.into_iter().collect::<Vec<_>>();
        catalog_ids.sort_unstable();
        let objects = catalog_ids
            .iter()
            .map(|&id| {
                label_by_id
                    .get(&id)
                    .cloned()
                    .unwrap_or_else(|| crate::report::GcObjectSummary {
                        id,
                        kind: crate::value::ValueKind::Other,
                        label: format!("Object#{}", id),
                    })
            })
            .collect();

        GcStatsBundle {
            objects,
            phases: GcPhaseStats {
                trial_deletion: TrialDeletionStats {
                    edges_visited,
                    candidates: candidate_ids.len(),
                    object_decisions,
                    visited_edges,
                    omitted_object_decisions,
                    omitted_edge_details,
                },
                scan: ScanStats {
                    restored: restored_objects.len(),
                    garbage_candidates: garbage_candidate_objects.len(),
                    restored_objects,
                    garbage_candidate_objects,
                    restoration_witnesses,
                    omitted_witnesses,
                },
                free_cycles: FreeCycleStats {
                    freed,
                },
            },
        }
    }

    fn capture_visited_edges(&self, live_ids: &[GcId]) -> Vec<VisitedEdge> {
        let mut edges = Vec::new();
        for &from_id in live_ids {
            if let Some(cell) = self.object_downcast::<ValueCell>(from_id) {
                let mut ordinal = 0usize;
                cell.value.visit_edges(|relation, target| {
                    edges.push((
                        from_id,
                        ordinal,
                        VisitedEdge {
                            from_id,
                            to_id: target.0,
                            relation,
                        },
                    ));
                    ordinal += 1;
                });
            } else {
                let object = self.objects[from_id]
                    .as_ref()
                    .and_then(|entry| entry.object.as_ref())
                    .expect("live GC object must have a payload");
                let mut ordinal = 0usize;
                object.trace(&mut |to_id| {
                    edges.push((
                        from_id,
                        ordinal,
                        VisitedEdge {
                            from_id,
                            to_id,
                            relation: EdgeRelation::Unknown,
                        },
                    ));
                    ordinal += 1;
                });
            }
        }
        // Preserve per-source visit order (already HashKey-/name-sorted) while
        // ordering sources by fromId. This keeps typed hash key order stable
        // even though JSON only stores display labels.
        edges.sort_by(|left, right| left.0.cmp(&right.0).then(left.1.cmp(&right.1)));
        edges.into_iter().map(|(_, _, edge)| edge).collect()
    }

    fn build_restoration_witnesses(
        &self,
        edges: &[VisitedEdge],
        trial_ref_counts: &HashMap<GcId, i32>,
        restored_ids: &[GcId],
    ) -> Vec<RestorationWitness> {
        let mut adjacency: HashMap<GcId, Vec<&VisitedEdge>> = HashMap::new();
        for edge in edges {
            adjacency.entry(edge.from_id).or_default().push(edge);
        }
        // `edges` is already ordered by (fromId, visit ordinal); keep that order.

        let mut roots: Vec<GcId> = trial_ref_counts
            .iter()
            .filter_map(|(&id, &rc)| (rc > 0).then_some(id))
            .collect();
        roots.sort_unstable();

        // predecessor[object] = (predecessor_id, relation, root_id)
        let mut predecessor: HashMap<GcId, (GcId, EdgeRelation, GcId)> = HashMap::new();
        let mut visited: HashSet<GcId> = HashSet::new();
        let mut queue: VecDeque<(GcId, GcId)> = VecDeque::new(); // (node, root)

        for root in roots {
            if visited.insert(root) {
                queue.push_back((root, root));
            }
        }

        while let Some((node, root)) = queue.pop_front() {
            let Some(outs) = adjacency.get(&node) else {
                continue;
            };
            for edge in outs {
                if visited.insert(edge.to_id) {
                    predecessor.insert(edge.to_id, (edge.from_id, edge.relation.clone(), root));
                    queue.push_back((edge.to_id, root));
                }
            }
        }

        let mut witnesses = Vec::new();
        for &object_id in restored_ids {
            if let Some((predecessor_id, relation, root_id)) = predecessor.get(&object_id) {
                witnesses.push(RestorationWitness {
                    object_id,
                    root_id: *root_id,
                    predecessor_id: *predecessor_id,
                    relation: relation.clone(),
                });
            }
        }
        witnesses.sort_by_key(|witness| witness.object_id);
        witnesses
    }

    fn sorted_list_ids(&self, kind: GcListKind) -> Vec<GcId> {
        let current = match kind {
            GcListKind::GcObj => self.gc_obj_list.head,
            GcListKind::Tmp => self.tmp_obj_list.head,
            GcListKind::ZeroRef => self.gc_zero_ref_count_list.head,
        };
        let mut ids = GcListIter {
            rt: self,
            current,
        }
        .collect::<Vec<_>>();
        ids.sort_unstable();
        ids
    }

    fn gc_edge_count(&self) -> usize {
        let mut count = 0;
        for id in (GcListIter {
            rt: self,
            current: self.gc_obj_list.head,
        }) {
            let object = self.objects[id]
                .as_ref()
                .and_then(|entry| entry.object.as_ref())
                .expect("live GC object must have a payload");
            object.trace(&mut |_| count += 1);
        }
        count
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
        (entry.object.as_ref()?.as_ref() as &dyn Any).downcast_ref()
    }

    pub fn object_downcast_mut<T: GcObject>(&mut self, id: GcId) -> Option<&mut T> {
        let entry = self.objects.get_mut(id)?.as_mut()?;
        (entry.object.as_mut()?.as_mut() as &mut dyn Any).downcast_mut()
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

fn edge_priority(
    edge: &VisitedEdge,
    candidates: &HashSet<GcId>,
    witness_edges: &[(GcId, GcId, EdgeRelation)],
) -> u8 {
    if witness_edges.iter().any(|(from, to, relation)| {
        *from == edge.from_id && *to == edge.to_id && *relation == edge.relation
    }) {
        return 0;
    }
    let from_c = candidates.contains(&edge.from_id);
    let to_c = candidates.contains(&edge.to_id);
    match (from_c, to_c) {
        (true, true) => 1,
        (false, true) => 2,
        (true, false) => 3,
        (false, false) => 4,
    }
}

fn select_visited_edges(
    all_edges: &[VisitedEdge],
    candidates: &HashSet<GcId>,
    witnesses: &[RestorationWitness],
) -> (Vec<VisitedEdge>, usize) {
    let witness_edges: Vec<(GcId, GcId, EdgeRelation)> = witnesses
        .iter()
        .map(|witness| (witness.predecessor_id, witness.object_id, witness.relation.clone()))
        .collect();

    let mut ranked = all_edges
        .iter()
        .enumerate()
        .map(|(index, edge)| (edge_priority(edge, candidates, &witness_edges), index))
        .collect::<Vec<_>>();
    ranked.sort_by(|left, right| left.0.cmp(&right.0).then(left.1.cmp(&right.1)));

    let selected_indices: HashSet<usize> = ranked
        .into_iter()
        .take(MAX_EDGE_DETAILS)
        .map(|(_, index)| index)
        .collect();
    let kept = all_edges
        .iter()
        .enumerate()
        .filter_map(|(index, edge)| selected_indices.contains(&index).then(|| edge.clone()))
        .collect::<Vec<_>>();
    let omitted = all_edges.len().saturating_sub(kept.len());
    (kept, omitted)
}

fn witness_chain_ids(
    witness: &RestorationWitness,
    by_object: &HashMap<GcId, &RestorationWitness>,
) -> Option<Vec<GcId>> {
    let mut ids = vec![witness.object_id, witness.root_id, witness.predecessor_id];
    let mut current = witness.object_id;
    let mut seen = HashSet::new();
    seen.insert(current);

    while let Some(entry) = by_object.get(&current) {
        if entry.predecessor_id == entry.root_id {
            break;
        }
        if !seen.insert(entry.predecessor_id) {
            return None;
        }
        ids.push(entry.predecessor_id);
        current = entry.predecessor_id;
        if current == witness.root_id {
            break;
        }
        if !by_object.contains_key(&current) {
            break;
        }
    }

    ids.sort_unstable();
    ids.dedup();
    Some(ids)
}

fn select_object_decisions(
    live_ids: &[GcId],
    candidates: &HashSet<GcId>,
    survivor_ids: &[GcId],
    witnesses: &[RestorationWitness],
    ref_count_before: &HashMap<GcId, i32>,
    heap_incoming: &HashMap<GcId, usize>,
    trial_ref_counts: &HashMap<GcId, i32>,
    garbage_set: &HashSet<GcId>,
) -> (Vec<ObjectDecision>, usize, HashSet<GcId>) {
    let witness_by_object: HashMap<GcId, &RestorationWitness> = witnesses
        .iter()
        .map(|witness| (witness.object_id, witness))
        .collect();

    let mut witness_survivor_ids = HashSet::new();
    for witness in witnesses {
        if let Some(ids) = witness_chain_ids(witness, &witness_by_object) {
            for id in ids {
                if !candidates.contains(&id) {
                    witness_survivor_ids.insert(id);
                }
            }
        }
        witness_survivor_ids.insert(witness.root_id);
    }

    let mut candidate_ids: Vec<GcId> = candidates.iter().copied().collect();
    candidate_ids.sort_unstable();
    let mut witness_survivors: Vec<GcId> = witness_survivor_ids.iter().copied().collect();
    witness_survivors.sort_unstable();
    let mut other_survivors: Vec<GcId> = survivor_ids
        .iter()
        .copied()
        .filter(|id| !witness_survivor_ids.contains(id))
        .collect();
    other_survivors.sort_unstable();

    let mut ordered = Vec::new();
    ordered.extend(candidate_ids);
    ordered.extend(witness_survivors);
    ordered.extend(other_survivors);
    for &id in live_ids {
        if !ordered.contains(&id) {
            ordered.push(id);
        }
    }

    let selected: Vec<GcId> = ordered.into_iter().take(MAX_OBJECT_DECISIONS).collect();
    let selected_set: HashSet<GcId> = selected.iter().copied().collect();
    let omitted = live_ids.len().saturating_sub(selected.len());

    let mut decisions = selected
        .iter()
        .map(|&object_id| {
            let trial_ref_count = trial_ref_counts[&object_id];
            let decision = if trial_ref_count == 0 {
                TrialDecision::Candidate
            } else {
                TrialDecision::Survivor
            };
            let final_fate =
                if decision == TrialDecision::Candidate && garbage_set.contains(&object_id) {
                    FinalFate::Freed
                } else {
                    FinalFate::Retained
                };
            ObjectDecision {
                object_id,
                ref_count_before: ref_count_before[&object_id],
                heap_incoming_edges: heap_incoming.get(&object_id).copied().unwrap_or(0),
                trial_ref_count,
                decision,
                final_fate,
            }
        })
        .collect::<Vec<_>>();
    decisions.sort_by_key(|decision| decision.object_id);
    (decisions, omitted, selected_set)
}

fn select_restoration_witnesses(
    all_witnesses: &[RestorationWitness],
    selected_decision_ids: &HashSet<GcId>,
) -> (Vec<RestorationWitness>, usize) {
    let by_object: HashMap<GcId, &RestorationWitness> = all_witnesses
        .iter()
        .map(|witness| (witness.object_id, witness))
        .collect();

    let mut kept = Vec::new();
    for witness in all_witnesses {
        if kept.len() >= MAX_RESTORATION_WITNESSES {
            break;
        }
        let Some(chain) = witness_chain_ids(witness, &by_object) else {
            continue;
        };
        if chain.iter().any(|id| !selected_decision_ids.contains(id)) {
            continue;
        }
        kept.push(witness.clone());
    }
    let omitted = all_witnesses.len().saturating_sub(kept.len());
    (kept, omitted)
}
