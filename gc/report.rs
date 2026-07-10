use std::collections::BTreeMap;

use serde::Serialize;

use crate::value::{Value, ValueCell, ValueKind};
use crate::{GcHeap, GcId, GcObjectType, GcRef, GcRuntime};

pub type ValueKindCounts = BTreeMap<ValueKind, usize>;

const VALUE_KINDS: [ValueKind; 7] = [
    ValueKind::Class,
    ValueKind::Instance,
    ValueKind::BoundMethod,
    ValueKind::Closure,
    ValueKind::Array,
    ValueKind::Hash,
    ValueKind::Other,
];

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HeapSnapshot {
    pub object_count: usize,
    pub tracked_bytes: usize,
    pub by_value_kind: ValueKindCounts,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrialDeletionStats {
    pub edges_visited: usize,
    pub candidates: usize,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanStats {
    pub restored: usize,
    pub garbage_candidates: usize,
    pub restored_objects: Vec<GcObjectSummary>,
    pub garbage_candidate_objects: Vec<GcObjectSummary>,
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

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GcCollectionReport {
    pub before: HeapSnapshot,
    pub after: HeapSnapshot,
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

fn summarize_gc_object(runtime: &GcRuntime, id: GcId) -> GcObjectSummary {
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
        Value::Closure(_) => "Closure".to_string(),
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
