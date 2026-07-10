use std::collections::BTreeMap;

use serde::Serialize;

use crate::value::{ValueCell, ValueKind};
use crate::{GcHeap, GcId};

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
