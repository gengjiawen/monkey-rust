use std::cmp::Ordering;
use std::collections::BTreeMap;

use serde::Serialize;

use crate::value::{Value, ValueCell, ValueKind};
use crate::{GcHeap, GcId, GcObjectType, GcRef, GcRuntime};

pub type ValueKindCounts = BTreeMap<ValueKind, usize>;

pub const MAX_EDGE_DETAILS: usize = 500;
pub const MAX_OBJECT_DECISIONS: usize = 500;
pub const MAX_RESTORATION_WITNESSES: usize = 500;

pub use crate::value::{EdgeRelation, HashKeyKind};

const VALUE_KINDS: [ValueKind; 14] = [
    ValueKind::Class,
    ValueKind::Instance,
    ValueKind::BoundMethod,
    ValueKind::Closure,
    ValueKind::Array,
    ValueKind::Hash,
    ValueKind::Integer,
    ValueKind::Boolean,
    ValueKind::String,
    ValueKind::Null,
    ValueKind::Error,
    ValueKind::CompiledFunction,
    ValueKind::Builtin,
    ValueKind::Other,
];

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HeapSnapshot {
    pub object_count: usize,
    pub tracked_bytes: usize,
    pub by_value_kind: ValueKindCounts,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum TrialDecision {
    Candidate,
    Survivor,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum FinalFate {
    Retained,
    Freed,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ObjectDecision {
    pub object_id: GcId,
    pub ref_count_before: i32,
    pub heap_incoming_edges: usize,
    pub trial_ref_count: i32,
    pub decision: TrialDecision,
    #[serde(rename = "final")]
    pub final_fate: FinalFate,
}

impl EdgeRelation {
    pub fn kind_rank(&self) -> u8 {
        match self {
            EdgeRelation::ArrayElement {
                ..
            } => 0,
            EdgeRelation::HashValue {
                ..
            } => 1,
            EdgeRelation::ClosureFunction => 2,
            EdgeRelation::ClosureFree {
                ..
            } => 3,
            EdgeRelation::ClassConstructor => 4,
            EdgeRelation::ClassMethod {
                ..
            } => 5,
            EdgeRelation::InstanceClass => 6,
            EdgeRelation::InstanceField {
                ..
            } => 7,
            EdgeRelation::BoundMethodReceiver => 8,
            EdgeRelation::BoundMethodFunction => 9,
            EdgeRelation::Unknown => 10,
        }
    }

    pub fn sort_payload(&self) -> RelationSortKey<'_> {
        match self {
            EdgeRelation::ArrayElement {
                index,
            } => RelationSortKey::Index(*index),
            EdgeRelation::HashValue {
                key_kind,
                key,
            } => RelationSortKey::HashKey(*key_kind, key),
            EdgeRelation::ClosureFunction => RelationSortKey::None,
            EdgeRelation::ClosureFree {
                index,
            } => RelationSortKey::Index(*index),
            EdgeRelation::ClassConstructor => RelationSortKey::None,
            EdgeRelation::ClassMethod {
                name,
            } => RelationSortKey::Name(name),
            EdgeRelation::InstanceClass => RelationSortKey::None,
            EdgeRelation::InstanceField {
                name,
            } => RelationSortKey::Name(name),
            EdgeRelation::BoundMethodReceiver => RelationSortKey::None,
            EdgeRelation::BoundMethodFunction => RelationSortKey::None,
            EdgeRelation::Unknown => RelationSortKey::None,
        }
    }
}

#[derive(Eq, PartialEq)]
pub enum RelationSortKey<'a> {
    None,
    Index(usize),
    HashKey(HashKeyKind, &'a str),
    Name(&'a str),
}

impl Ord for RelationSortKey<'_> {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self, other) {
            (RelationSortKey::None, RelationSortKey::None) => Ordering::Equal,
            (RelationSortKey::None, _) => Ordering::Less,
            (_, RelationSortKey::None) => Ordering::Greater,
            (RelationSortKey::Index(a), RelationSortKey::Index(b)) => a.cmp(b),
            (RelationSortKey::Index(_), _) => Ordering::Less,
            (_, RelationSortKey::Index(_)) => Ordering::Greater,
            (RelationSortKey::HashKey(a_kind, a), RelationSortKey::HashKey(b_kind, b)) => {
                a_kind.cmp(b_kind).then(a.cmp(b))
            }
            (RelationSortKey::HashKey(_, _), RelationSortKey::Name(_)) => Ordering::Less,
            (RelationSortKey::Name(_), RelationSortKey::HashKey(_, _)) => Ordering::Greater,
            (RelationSortKey::Name(a), RelationSortKey::Name(b)) => a.cmp(b),
        }
    }
}

impl PartialOrd for RelationSortKey<'_> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VisitedEdge {
    pub from_id: GcId,
    pub to_id: GcId,
    pub relation: EdgeRelation,
}

impl VisitedEdge {
    pub fn sort_key(&self) -> (GcId, u8, RelationSortKey<'_>, GcId) {
        (self.from_id, self.relation.kind_rank(), self.relation.sort_payload(), self.to_id)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RestorationWitness {
    pub object_id: GcId,
    pub root_id: GcId,
    pub predecessor_id: GcId,
    pub relation: EdgeRelation,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrialDeletionStats {
    pub edges_visited: usize,
    pub candidates: usize,
    pub object_decisions: Vec<ObjectDecision>,
    pub visited_edges: Vec<VisitedEdge>,
    pub omitted_object_decisions: usize,
    pub omitted_edge_details: usize,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanStats {
    pub restored: usize,
    pub garbage_candidates: usize,
    pub restored_objects: Vec<GcObjectSummary>,
    pub garbage_candidate_objects: Vec<GcObjectSummary>,
    pub restoration_witnesses: Vec<RestorationWitness>,
    pub omitted_witnesses: usize,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GcObjectSummary {
    pub id: GcId,
    pub kind: ValueKind,
    pub label: String,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FreeCycleStats {
    pub freed: usize,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GcPhaseStats {
    pub trial_deletion: TrialDeletionStats,
    pub scan: ScanStats,
    pub free_cycles: FreeCycleStats,
}

/// Telemetry returned by `run_gc_with_stats_bundle`: object catalog plus phase stats.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GcStatsBundle {
    pub objects: Vec<GcObjectSummary>,
    pub phases: GcPhaseStats,
}

/// A global variable name and the object its slot references at report time.
/// This is a present-tense fact read from the VM's global table — the defined
/// root set, not a guess about aliases.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GlobalRoot {
    pub name: String,
    pub object_id: GcId,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GcCollectionReport {
    pub before: HeapSnapshot,
    pub after: HeapSnapshot,
    pub objects: Vec<GcObjectSummary>,
    pub global_roots: Vec<GlobalRoot>,
    pub phases: GcPhaseStats,
    pub collected_by_value_kind: ValueKindCounts,
}

pub(crate) fn empty_value_kind_counts() -> ValueKindCounts {
    VALUE_KINDS.iter().copied().map(|kind| (kind, 0)).collect()
}

pub(crate) fn summarize_gc_objects(runtime: &GcRuntime, ids: &[GcId]) -> Vec<GcObjectSummary> {
    ids.iter()
        .copied()
        .map(|id| summarize_gc_object(runtime, id))
        .collect()
}

pub(crate) fn summarize_gc_object(runtime: &GcRuntime, id: GcId) -> GcObjectSummary {
    let Some(cell) = runtime.object_downcast::<ValueCell>(id) else {
        let name = match runtime.header(id).gc_obj_type {
            GcObjectType::MonkeyObject => "Object",
            GcObjectType::FunctionBytecode => "FunctionBytecode",
            GcObjectType::Shape => "Shape",
            GcObjectType::VarRef => "VarRef",
            GcObjectType::AsyncFunction => "AsyncFunction",
            GcObjectType::MonkeyContext => "MonkeyContext",
        };
        return GcObjectSummary {
            id,
            kind: ValueKind::Other,
            label: format!("{}#{}", name, id),
        };
    };

    let kind = cell.value.kind();
    let name = match &cell.value {
        Value::Class(class) => format!("Class({})", class.name),
        Value::Instance(instance) => {
            format!("Instance({})", class_name(runtime, instance.class))
        }
        Value::BoundMethod(method) => format!(
            "BoundMethod({}.{})",
            instance_class_name(runtime, method.receiver),
            method.name
        ),
        Value::Closure(closure) => closure_name(runtime, closure.func)
            .map(|name| format!("Closure({})", name))
            .unwrap_or_else(|| "Closure".to_string()),
        Value::Array(_) => "Array".to_string(),
        Value::Hash(_) => "Hash".to_string(),
        Value::Integer(_) => "Integer".to_string(),
        Value::Boolean(_) => "Boolean".to_string(),
        Value::String(_) => "String".to_string(),
        Value::Null => "Null".to_string(),
        Value::Error(_) => "Error".to_string(),
        Value::CompiledFunction(_) => "CompiledFunction".to_string(),
        Value::Builtin(_) => "Builtin".to_string(),
    };

    GcObjectSummary {
        id,
        kind,
        label: format!("{}#{}", name, id),
    }
}

fn class_name(runtime: &GcRuntime, reference: GcRef) -> &str {
    runtime
        .object_downcast::<ValueCell>(reference.0)
        .and_then(|cell| match &cell.value {
            Value::Class(class) => Some(class.name.as_str()),
            _ => None,
        })
        .unwrap_or("<unknown>")
}

fn closure_name(runtime: &GcRuntime, reference: GcRef) -> Option<&str> {
    runtime
        .object_downcast::<ValueCell>(reference.0)
        .and_then(|cell| match &cell.value {
            Value::CompiledFunction(function) if !function.name.is_empty() => {
                Some(function.name.as_str())
            }
            _ => None,
        })
}

fn instance_class_name(runtime: &GcRuntime, reference: GcRef) -> &str {
    runtime
        .object_downcast::<ValueCell>(reference.0)
        .and_then(|cell| match &cell.value {
            Value::Instance(instance) => Some(class_name(runtime, instance.class)),
            _ => None,
        })
        .unwrap_or("<unknown>")
}

impl GcHeap {
    pub fn snapshot(&self) -> HeapSnapshot {
        let mut by_value_kind = empty_value_kind_counts();
        for kind in self.value_kinds_by_id().values() {
            *by_value_kind.entry(*kind).or_default() += 1;
        }
        HeapSnapshot {
            object_count: self.runtime().gc_object_count(),
            tracked_bytes: self.malloc_state().malloc_size,
            by_value_kind,
        }
    }

    pub(crate) fn value_kinds_by_id(&self) -> BTreeMap<GcId, ValueKind> {
        self.runtime()
            .object_ids()
            .into_iter()
            .filter_map(|id| {
                self.runtime()
                    .object_downcast::<ValueCell>(id)
                    .map(|cell| (id, cell.value.kind()))
            })
            .collect()
    }
}
