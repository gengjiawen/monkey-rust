//! Copy-out debugger snapshots (playground debugger design §5).
//!
//! Every `debugger;` statement executed by the GC VM materializes one
//! [`DebuggerHit`]: call frames, named slots, and a bounded projection of the
//! current heap. A hit holds only strings, integers, and plain vectors — never
//! a `GcRef` — so recording cannot root objects or change later GC behavior.
//!
//! Values are projected by teaching semantics, not by allocation reality
//! (design §2.4): Integer/Boolean/Null/Builtin are inlined into slots and
//! parent members with `heap_id: None`; String/Array/Hash/Closure/Class/
//! Instance/BoundMethod/Error become heap nodes referenced as `ref #id`;
//! CompiledFunction and VM infrastructure are hidden entirely and are not
//! counted as omitted.

use std::collections::{HashSet, VecDeque};

use compiler::compiler::DebugInfo;
use parser::lexer::token::Span;
use serde::Serialize;

use crate::report::summarize_gc_object;
use crate::value::{format_hash_key_label, EdgeRelation, HashKey, Value, ValueCell, ValueKind};
use crate::{Frame, GcHeap, GcId, GcRef};

pub const MAX_DEBUGGER_HITS: usize = 25;
pub const MAX_DEBUGGER_OBJECTS: usize = 100; // per hit
pub const MAX_DEBUGGER_EDGES: usize = 250; // per hit
pub const MAX_DEBUGGER_DISPLAY_CHARS: usize = 64;
pub const MAX_DEBUGGER_SUMMARY_DEPTH: usize = 2;
pub const MAX_DEBUGGER_MEMBERS: usize = 8; // per container/object

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DebuggerHit {
    /// 1-based position of this hit in execution order.
    pub index: usize,
    /// Source span of the `debugger;` statement itself.
    pub span: Option<Span>,
    /// `[0]` is main, the last entry is the currently executing frame.
    pub frames: Vec<FrameView>,
    /// Global slots in definition-ledger order, rebindings included.
    pub globals: Vec<SlotView>,
    pub heap: HeapView,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FrameView {
    pub name: String,
    pub current_span: Option<Span>,
    /// The value being called, read from `base_pointer - 1`; None for main.
    pub callee: Option<ValueView>,
    pub locals: Vec<SlotView>,
    pub captures: Vec<CaptureView>,
    /// Operand-stack values above the locals window. Only the current frame
    /// reports these; a suspended caller's window also covers its callee's
    /// frame, which would misclassify arguments as temporaries.
    pub temporaries: Vec<StackSlotView>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SlotView {
    pub name: String,
    pub slot: usize,
    /// False until the slot's `OpSetLocal`/`OpSetGlobal` ran. The prefilled
    /// null in an uninitialized slot is VM plumbing, not a user value, so
    /// `value` is None whenever this is false.
    pub initialized: bool,
    pub value: Option<ValueView>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptureView {
    pub name: String,
    pub index: usize,
    pub value: ValueView,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StackSlotView {
    /// Absolute VM stack slot, so hits stay comparable across frames.
    pub slot: usize,
    pub value: ValueView,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ValueView {
    pub kind: ValueKind,
    pub display: String,
    /// Present exactly for heap-node kinds, even when the object itself was
    /// dropped from `HeapView.objects` by the budget — the UI shows those as
    /// "not captured" instead of losing the reference.
    pub heap_id: Option<usize>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HeapObjectView {
    pub id: usize,
    pub kind: ValueKind,
    pub label: String,
    /// Inlined scalar elements/fields, in `visit_edges` order.
    pub members: Vec<HeapMemberView>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HeapMemberView {
    pub relation: EdgeRelation,
    pub display: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HeapEdgeView {
    pub from: usize,
    pub to: usize,
    pub relation: EdgeRelation,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HeapView {
    pub objects: Vec<HeapObjectView>,
    pub edges: Vec<HeapEdgeView>,
    /// Heap-node objects dropped by the object budget. Inlined scalars and
    /// deliberately hidden kinds are presentation policy, not truncation, and
    /// are never counted here.
    pub omitted_objects: usize,
    /// Node-to-node edges not emitted because an endpoint missed the object
    /// budget or the edge budget ran out.
    pub omitted_edges: usize,
}

/// Borrowed view of everything `collect_hit` reads from the VM. The VM keeps
/// its fields private; this struct is the explicit contract of what a
/// snapshot depends on.
pub(crate) struct HitContext<'a> {
    pub heap: &'a GcHeap,
    /// Active frames only: `frames[0]` is main, the last is current.
    pub frames: &'a [Frame],
    pub stack: &'a [GcRef],
    pub sp: usize,
    pub globals: &'a [GcRef],
    pub global_bindings: &'a [compiler::compiler::BindingDebugInfo],
    pub globals_initialized: &'a [bool],
    pub main_debug_info: &'a DebugInfo,
    pub function_debug_info: &'a std::collections::HashMap<GcRef, DebugInfo>,
    /// 1-based index this hit will get.
    pub index: usize,
}

pub(crate) fn collect_hit(ctx: HitContext) -> DebuggerHit {
    // User-object roots in the documented stable order: globals, then each
    // frame's callee/locals/captures (main -> current), then the current
    // frame's temporaries. Views are built in the same pass so the graph can
    // never disagree with the slot listing.
    let mut roots: Vec<GcRef> = Vec::new();

    let mut globals = Vec::with_capacity(ctx.global_bindings.len());
    for binding in ctx.global_bindings {
        globals.push(slot_view(
            ctx.heap,
            &binding.name,
            binding.slot,
            ctx.globals_initialized
                .get(binding.slot)
                .copied()
                .unwrap_or(false),
            ctx.globals.get(binding.slot).copied(),
            &mut roots,
        ));
    }

    let mut frames = Vec::with_capacity(ctx.frames.len());
    for (frame_number, frame) in ctx.frames.iter().enumerate() {
        let is_main = frame_number == 0;
        let is_current = frame_number == ctx.frames.len() - 1;
        let debug_info = if is_main {
            Some(ctx.main_debug_info)
        } else {
            ctx.function_debug_info.get(&frame.cl.func)
        };

        let callee = (!is_main)
            .then(|| frame.base_pointer.checked_sub(1))
            .flatten()
            .and_then(|slot| ctx.stack.get(slot))
            .map(|reference| rooted_value_view(ctx.heap, *reference, &mut roots));

        let mut locals = Vec::new();
        if let Some(debug_info) = debug_info {
            for binding in &debug_info.local_bindings {
                locals.push(slot_view(
                    ctx.heap,
                    &binding.name,
                    binding.slot,
                    frame
                        .initialized
                        .get(binding.slot)
                        .copied()
                        .unwrap_or(false),
                    ctx.stack.get(frame.base_pointer + binding.slot).copied(),
                    &mut roots,
                ));
            }
        }

        let free_names = debug_info
            .map(|info| info.free_names.as_slice())
            .unwrap_or(&[]);
        let captures = frame
            .cl
            .free
            .iter()
            .enumerate()
            .map(|(index, reference)| CaptureView {
                name: free_names
                    .get(index)
                    .cloned()
                    .unwrap_or_else(|| format!("<free {}>", index)),
                index,
                value: rooted_value_view(ctx.heap, *reference, &mut roots),
            })
            .collect();

        frames.push(FrameView {
            name: frame_name(ctx.heap, frame, is_main),
            current_span: (frame.ip >= 0)
                .then_some(frame.ip as usize)
                .and_then(|pc| debug_info.and_then(|info| info.span_for_pc(pc)))
                .cloned(),
            callee,
            locals,
            captures,
            // Filled below for the current frame only, after its locals are
            // rooted, to keep the documented root order.
            temporaries: Vec::new(),
        });

        if is_current {
            let first_temporary = frame.base_pointer + frame.initialized.len();
            let temporaries = (first_temporary..ctx.sp)
                .filter_map(|slot| ctx.stack.get(slot).map(|reference| (slot, *reference)))
                .map(|(slot, reference)| StackSlotView {
                    slot,
                    value: rooted_value_view(ctx.heap, reference, &mut roots),
                })
                .collect();
            frames
                .last_mut()
                .expect("current frame was just pushed")
                .temporaries = temporaries;
        }
    }

    let span = frames.last().and_then(|frame| frame.current_span.clone());
    let heap = project_heap(ctx.heap, &roots);

    DebuggerHit {
        index: ctx.index,
        span,
        frames,
        globals,
        heap,
    }
}

fn slot_view(
    heap: &GcHeap,
    name: &str,
    slot: usize,
    initialized: bool,
    reference: Option<GcRef>,
    roots: &mut Vec<GcRef>,
) -> SlotView {
    let value = match reference {
        Some(reference) if initialized => Some(rooted_value_view(heap, reference, roots)),
        _ => None,
    };
    SlotView {
        name: name.to_string(),
        slot,
        // A binding whose slot the VM never materialized (defensive: hostile
        // debug metadata) reads as uninitialized rather than inventing a value.
        initialized: initialized && reference.is_some(),
        value,
    }
}

fn frame_name(heap: &GcHeap, frame: &Frame, is_main: bool) -> String {
    if is_main {
        return "main".to_string();
    }
    match try_value(heap, frame.cl.func) {
        Some(Value::CompiledFunction(function)) if !function.name.is_empty() => {
            function.name.clone()
        }
        _ => "<anonymous>".to_string(),
    }
}

fn rooted_value_view(heap: &GcHeap, reference: GcRef, roots: &mut Vec<GcRef>) -> ValueView {
    let view = value_view(heap, reference);
    if view.heap_id.is_some() {
        roots.push(reference);
    }
    view
}

pub(crate) fn value_view(heap: &GcHeap, reference: GcRef) -> ValueView {
    let kind = try_value(heap, reference)
        .map(Value::kind)
        .unwrap_or(ValueKind::Other);
    ValueView {
        kind,
        display: bounded_display(heap, reference),
        heap_id: is_heap_node(kind).then_some(reference.0),
    }
}

/// Kinds that become graph nodes; slots reference them as `ref #id`.
fn is_heap_node(kind: ValueKind) -> bool {
    matches!(
        kind,
        ValueKind::String
            | ValueKind::Array
            | ValueKind::Hash
            | ValueKind::Closure
            | ValueKind::Class
            | ValueKind::Instance
            | ValueKind::BoundMethod
            | ValueKind::Error
    )
}

/// Kinds inlined into slots and parent members; they never get nodes or edges.
fn is_inline_scalar(kind: ValueKind) -> bool {
    matches!(kind, ValueKind::Integer | ValueKind::Boolean | ValueKind::Null | ValueKind::Builtin)
}

fn try_value(heap: &GcHeap, reference: GcRef) -> Option<&Value> {
    heap.runtime()
        .object_downcast::<ValueCell>(reference.0)
        .map(|cell| &cell.value)
}

/// Character-budgeted string builder. The budget is enforced while
/// appending — a huge string or array never materializes in full before
/// truncation (design §5.3).
struct BoundedText {
    out: String,
    remaining: usize,
    truncated: bool,
}

impl BoundedText {
    fn new(limit: usize) -> Self {
        BoundedText {
            out: String::new(),
            remaining: limit,
            truncated: false,
        }
    }

    fn push(&mut self, text: &str) {
        if self.truncated {
            return;
        }
        for ch in text.chars() {
            if self.remaining == 0 {
                self.truncated = true;
                return;
            }
            self.out.push(ch);
            self.remaining -= 1;
        }
    }

    fn finish(mut self) -> String {
        if self.truncated {
            self.out.push('…');
        }
        self.out
    }
}

fn bounded_display(heap: &GcHeap, reference: GcRef) -> String {
    let mut text = BoundedText::new(MAX_DEBUGGER_DISPLAY_CHARS);
    let mut visiting = HashSet::new();
    append_reference(heap, reference, MAX_DEBUGGER_SUMMARY_DEPTH, &mut visiting, &mut text);
    text.finish()
}

fn append_reference(
    heap: &GcHeap,
    reference: GcRef,
    depth: usize,
    visiting: &mut HashSet<GcId>,
    text: &mut BoundedText,
) {
    if text.truncated {
        return;
    }
    let Some(value) = try_value(heap, reference) else {
        text.push("<invalid>");
        return;
    };
    if !visiting.insert(reference.0) {
        text.push(&format!("[cycle #{}]", reference.0));
        return;
    }
    append_value(heap, value, depth, visiting, text);
    visiting.remove(&reference.0);
}

fn append_value(
    heap: &GcHeap,
    value: &Value,
    depth: usize,
    visiting: &mut HashSet<GcId>,
    text: &mut BoundedText,
) {
    match value {
        Value::Integer(value) => text.push(&value.to_string()),
        Value::Boolean(value) => text.push(&value.to_string()),
        Value::String(value) => text.push(value),
        Value::Null => text.push("null"),
        Value::Error(message) => text.push(message),
        Value::Builtin(_) => text.push("[builtin function]"),
        Value::CompiledFunction(_) => text.push("[compiled function]"),
        Value::Closure(_) => text.push("[closure function]"),
        Value::Class(class) => {
            text.push("[class ");
            text.push(&class.name);
            text.push("]");
        }
        Value::Instance(instance) => {
            text.push("[object ");
            text.push(&referenced_class_name(heap, instance.class));
            text.push("]");
        }
        Value::BoundMethod(method) => {
            text.push("[bound method ");
            text.push(&receiver_class_name(heap, method.receiver));
            text.push(".");
            text.push(&method.name);
            text.push("]");
        }
        Value::Array(items) => {
            if depth == 0 {
                text.push("[…]");
                return;
            }
            text.push("[");
            for (position, item) in items.iter().take(MAX_DEBUGGER_MEMBERS).enumerate() {
                if position > 0 {
                    text.push(", ");
                }
                append_reference(heap, *item, depth - 1, visiting, text);
            }
            if items.len() > MAX_DEBUGGER_MEMBERS {
                text.push(", …");
            }
            text.push("]");
        }
        Value::Hash(map) => {
            if depth == 0 {
                text.push("{…}");
                return;
            }
            let mut entries: Vec<(&HashKey, &GcRef)> = map.iter().collect();
            entries.sort_by_key(|(key, _)| *key);
            text.push("{");
            for (position, (key, value)) in entries.iter().take(MAX_DEBUGGER_MEMBERS).enumerate() {
                if position > 0 {
                    text.push(", ");
                }
                text.push(&format_hash_key_label(key));
                text.push(": ");
                append_reference(heap, **value, depth - 1, visiting, text);
            }
            if entries.len() > MAX_DEBUGGER_MEMBERS {
                text.push(", …");
            }
            text.push("}");
        }
    }
}

fn referenced_class_name(heap: &GcHeap, class: GcRef) -> String {
    match try_value(heap, class) {
        Some(Value::Class(class)) => class.name.clone(),
        _ => "<invalid class>".to_string(),
    }
}

fn receiver_class_name(heap: &GcHeap, receiver: GcRef) -> String {
    match try_value(heap, receiver) {
        Some(Value::Instance(instance)) => referenced_class_name(heap, instance.class),
        _ => "<invalid receiver>".to_string(),
    }
}

/// Deterministic bounded heap projection (design §5.3 steps 5-6): BFS from
/// the roots in order, then remaining user objects by ascending id, then one
/// edge pass over the selected set.
fn project_heap(heap: &GcHeap, roots: &[GcRef]) -> HeapView {
    let kinds = heap.value_kinds_by_id();
    let node_ids: Vec<GcId> = kinds
        .iter()
        .filter(|(_, kind)| is_heap_node(**kind))
        .map(|(id, _)| *id)
        .collect();
    let node_id_set: HashSet<GcId> = node_ids.iter().copied().collect();

    let mut selected: Vec<GcId> = Vec::new();
    let mut selected_set: HashSet<GcId> = HashSet::new();
    let mut queue: VecDeque<GcId> = VecDeque::new();
    let try_select = |id: GcId,
                      selected: &mut Vec<GcId>,
                      selected_set: &mut HashSet<GcId>,
                      queue: &mut VecDeque<GcId>| {
        if selected.len() >= MAX_DEBUGGER_OBJECTS
            || !node_id_set.contains(&id)
            || !selected_set.insert(id)
        {
            return;
        }
        selected.push(id);
        queue.push_back(id);
    };

    for root in roots {
        try_select(root.0, &mut selected, &mut selected_set, &mut queue);
    }
    while let Some(id) = queue.pop_front() {
        if let Some(cell) = heap.runtime().object_downcast::<ValueCell>(id) {
            let mut targets = Vec::new();
            cell.value.visit_edges(|_, target| targets.push(target));
            for target in targets {
                try_select(target.0, &mut selected, &mut selected_set, &mut queue);
            }
        }
    }
    for id in &node_ids {
        try_select(*id, &mut selected, &mut selected_set, &mut queue);
    }

    let mut objects = Vec::with_capacity(selected.len());
    let mut edges = Vec::new();
    let mut omitted_edges = 0usize;
    for &id in &selected {
        let summary = summarize_gc_object(heap.runtime(), id);
        let mut members = Vec::new();
        if let Some(cell) = heap.runtime().object_downcast::<ValueCell>(id) {
            cell.value.visit_edges(|relation, target| {
                let target_kind = kinds.get(&target.0).copied().unwrap_or(ValueKind::Other);
                if is_inline_scalar(target_kind) {
                    if members.len() < MAX_DEBUGGER_MEMBERS {
                        members.push(HeapMemberView {
                            relation,
                            display: bounded_display(heap, target),
                        });
                    }
                } else if is_heap_node(target_kind) {
                    if selected_set.contains(&target.0) && edges.len() < MAX_DEBUGGER_EDGES {
                        edges.push(HeapEdgeView {
                            from: id,
                            to: target.0,
                            relation,
                        });
                    } else {
                        omitted_edges += 1;
                    }
                }
                // Hidden kinds (CompiledFunction, Other): neither member,
                // edge, nor omission — presentation policy, not truncation.
            });
        }
        objects.push(HeapObjectView {
            id,
            kind: summary.kind,
            label: summary.label,
            members,
        });
    }

    HeapView {
        objects,
        edges,
        omitted_objects: node_ids.len() - selected.len(),
        omitted_edges,
    }
}
