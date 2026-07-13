use crate::header::GcId;
use crate::gc_runtime::GcRuntime;

/// Intrusive doubly-linked list head/tail, indexed by `GcId`.
/// Rust equivalent of QuickJS `list.h` + embedded `list_head`.
#[derive(Debug, Default, Clone)]
pub struct GcList {
    pub(crate) head: Option<GcId>,
    pub(crate) tail: Option<GcId>,
}

impl GcList {
    pub fn new() -> Self {
        GcList {
            head: None,
            tail: None,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.head.is_none()
    }

    pub fn head(&self) -> Option<GcId> {
        self.head
    }

    pub fn clear(&mut self) {
        self.head = None;
        self.tail = None;
    }
}

pub struct GcListIter<'a> {
    pub(crate) rt: &'a GcRuntime,
    pub(crate) current: Option<GcId>,
}

impl<'a> Iterator for GcListIter<'a> {
    type Item = GcId;

    fn next(&mut self) -> Option<Self::Item> {
        let id = self.current?;
        self.current = self.rt.header(id).list_next;
        Some(id)
    }
}
