use crate::{
    GcHeap,
    GcObject,
    GcObjectType,
    GcRef,
    MarkFunc,
};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

type EdgeMap = Rc<RefCell<HashMap<usize, Vec<GcRef>>>>;

struct TestNode {
    id: usize,
    edges: EdgeMap,
    freed: Rc<Cell<bool>>,
}

impl TestNode {
    fn new(id: usize, edges: EdgeMap, freed: Rc<Cell<bool>>) -> Self {
        TestNode {
            id,
            edges,
            freed,
        }
    }
}

impl GcObject for TestNode {
    fn trace(&self, visit: &mut dyn FnMut(crate::GcId)) {
        let edges = self.edges.borrow();
        if let Some(children) = edges.get(&self.id) {
            for child in children {
                visit(child.0);
            }
        }
    }

    fn on_free(&mut self, _rt: &mut crate::GcRuntime) {
        self.freed.set(true);
    }
}

struct TestHeap {
    gc: GcHeap,
    edges: EdgeMap,
    freed: Rc<Cell<bool>>,
}

impl TestHeap {
    fn new() -> Self {
        TestHeap {
            gc: GcHeap::new(),
            edges: Rc::new(RefCell::new(HashMap::new())),
            freed: Rc::new(Cell::new(false)),
        }
    }

    fn alloc(&mut self) -> GcRef {
        let id = if let Some(&id) = self.gc.runtime().free_slots_for_test().first() {
            id
        } else {
            self.gc.runtime().object_len_for_test()
        };
        let reference = self.gc.alloc(
            TestNode::new(id, self.edges.clone(), self.freed.clone()),
            GcObjectType::JsObject,
        );
        assert_eq!(reference.0, id);
        self.edges.borrow_mut().insert(id, Vec::new());
        reference
    }

    fn link(&mut self, from: GcRef, to: GcRef) {
        self.gc.dup(to);
        self.edges.borrow_mut().entry(from.0).or_default().push(to);
    }

    fn make_cycle(&mut self, size: usize) -> Vec<GcRef> {
        let nodes: Vec<GcRef> = (0..size).map(|_| self.alloc()).collect();
        for i in 0..size {
            let next = nodes[(i + 1) % size];
            self.link(nodes[i], next);
        }
        nodes
    }

    fn drop_external_refs(&mut self, ids: &[GcRef]) {
        for &id in ids {
            self.gc.free(id);
        }
    }
}

#[test]
fn refcount_frees_immediately_without_gc() {
    let mut heap = TestHeap::new();
    let a = heap.alloc();
    assert!(heap.gc.exists(a));
    heap.gc.free(a);
    assert!(!heap.gc.exists(a));
}

#[test]
fn dup_extends_lifetime() {
    let mut heap = TestHeap::new();
    let a = heap.alloc();
    let _b = heap.gc.dup(a);
    heap.gc.free(a);
    assert!(heap.gc.exists(a));
    heap.gc.free(a);
    assert!(!heap.gc.exists(a));
}

#[test]
fn collects_simple_cycle() {
    let mut heap = TestHeap::new();
    let nodes = heap.make_cycle(2);
    heap.drop_external_refs(&nodes);

    heap.gc.run_gc();
    assert!(!heap.gc.exists(nodes[0]));
    assert!(!heap.gc.exists(nodes[1]));
}

#[test]
fn cycle_with_external_root_survives() {
    let mut heap = TestHeap::new();
    let nodes = heap.make_cycle(2);

    let root = heap.gc.dup(nodes[0]);
    heap.gc.run_gc();

    assert!(heap.gc.exists(nodes[0]));
    assert!(heap.gc.exists(nodes[1]));

    heap.gc.free(root);
    heap.drop_external_refs(&nodes);
    heap.gc.run_gc();
    assert!(!heap.gc.exists(nodes[0]));
    assert!(!heap.gc.exists(nodes[1]));
}

#[test]
fn self_cycle_collected() {
    let mut heap = TestHeap::new();
    let a = heap.alloc();
    heap.link(a, a);
    heap.gc.free(a);

    heap.gc.run_gc();
    assert!(!heap.gc.exists(a));
}

#[test]
fn three_node_cycle_collected() {
    let mut heap = TestHeap::new();
    let nodes = heap.make_cycle(3);
    heap.drop_external_refs(&nodes);

    heap.gc.run_gc();
    for &id in &nodes {
        assert!(!heap.gc.exists(id));
    }
}

#[test]
fn ref_counted_freed_without_gc() {
    let mut gc = GcHeap::new();
    let freed = Rc::new(Cell::new(false));
    let flag = freed.clone();
    let id = gc.runtime_mut().add_ref_counted(move |_| {
        flag.set(true);
    });
    let dup = gc.runtime_mut().dup_ref_counted(id);
    gc.runtime_mut().free_ref_counted(id);
    assert!(!freed.get());
    gc.runtime_mut().free_ref_counted(dup);
    assert!(freed.get());
}

#[test]
fn trigger_gc_on_threshold() {
    let mut heap = TestHeap::new();
    heap.gc.set_gc_threshold(0);

    let nodes = heap.make_cycle(2);
    heap.drop_external_refs(&nodes);

    heap.gc.trigger_gc(1);
    assert!(!heap.gc.exists(nodes[0]));
    assert!(!heap.gc.exists(nodes[1]));
}

#[test]
fn mark_func_decref_zeros_isolated_cycle() {
    let mut heap = TestHeap::new();
    let nodes = heap.make_cycle(2);
    heap.drop_external_refs(&nodes);

    for &id in &nodes {
        heap.gc.mark_children(id, MarkFunc::Decref);
        heap.gc.header_mut(id).mark = 1;
    }

    assert_eq!(heap.gc.ref_count(nodes[0]), 0);
    assert_eq!(heap.gc.ref_count(nodes[1]), 0);
}

#[test]
fn four_node_cycle_collected() {
    let mut heap = TestHeap::new();
    let nodes = heap.make_cycle(4);
    heap.drop_external_refs(&nodes);
    heap.gc.run_gc();
    for &id in &nodes {
        assert!(!heap.gc.exists(id));
    }
}

#[test]
fn acyclic_graph_freed_without_gc() {
    let mut heap = TestHeap::new();
    let c = heap.alloc();
    let b = heap.alloc();
    heap.link(b, c);
    let a = heap.alloc();
    heap.link(a, b);

    heap.drop_external_refs(&[a, b, c]);
    assert!(!heap.gc.exists(a));
    assert!(!heap.gc.exists(b));
    assert!(!heap.gc.exists(c));
}

#[test]
fn on_free_called_when_collected() {
    let mut heap = TestHeap::new();
    let a = heap.alloc();
    assert!(!heap.freed.get());
    heap.gc.free(a);
    assert!(heap.freed.get());
    assert!(!heap.gc.exists(a));
}

#[test]
fn external_ref_to_cycle_entry_survives_gc() {
    let mut heap = TestHeap::new();
    let nodes = heap.make_cycle(2);
    let holder = heap.alloc();
    heap.link(holder, nodes[0]);

    heap.gc.free(nodes[0]);
    heap.gc.free(nodes[1]);

    heap.gc.run_gc();

    // Holder keeps the cycle entry alive through GC.
    assert!(heap.gc.exists(nodes[0]));
    assert!(heap.gc.exists(holder));

    heap.gc.free(holder);
    heap.gc.run_gc();
    assert!(!heap.gc.exists(nodes[0]));
}

#[test]
fn acyclic_holder_extends_child_lifetime() {
    let mut heap = TestHeap::new();
    let child = heap.alloc();
    let holder = heap.alloc();
    heap.link(holder, child);

    heap.gc.free(child);
    assert!(heap.gc.exists(child));

    heap.gc.free(holder);
    assert!(!heap.gc.exists(child));
}

#[test]
fn repeated_gc_is_idempotent() {
    let mut heap = TestHeap::new();
    let nodes = heap.make_cycle(2);
    heap.drop_external_refs(&nodes);

    heap.gc.run_gc();
    heap.gc.run_gc();
    assert!(!heap.gc.exists(nodes[0]));
    assert!(!heap.gc.exists(nodes[1]));
}

#[test]
fn malloc_state_tracks_allocations() {
    let mut heap = TestHeap::new();
    let before = heap.gc.malloc_state().malloc_count;
    let a = heap.alloc();
    assert!(heap.gc.malloc_state().malloc_count > before);
    heap.gc.free(a);
}
