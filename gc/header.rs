/// GC object type tags, adapted from QuickJS `JSGCObjectTypeEnum`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GcObjectType {
    MonkeyObject,
    FunctionBytecode,
    Shape,
    VarRef,
    AsyncFunction,
    JsContext,
}

/// Reentrancy guard during cascade free and cycle removal.
/// Matches QuickJS `JSGCPhaseEnum`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GcPhase {
    None,
    Decref,
    RemoveCycles,
}

/// Header shared by all cycle-GC'd objects.
/// Matches QuickJS `JSGCObjectHeader`.
#[derive(Debug, Clone)]
pub struct GcObjectHeader {
    pub ref_count: i32,
    pub gc_obj_type: GcObjectType,
    /// GC-phase flag (not a permanent mark bit). Set to 1 after `gc_decref` processes the object.
    pub mark: u8,
    /// Zombie detection during cycle free, inspired by QuickJS `free_mark`.
    pub free_mark: bool,
    pub list_prev: Option<GcId>,
    pub list_next: Option<GcId>,
}

/// Header for simple refcounted values (strings, bigints, etc.) that are not cycle-collected.
/// Matches QuickJS `JSRefCountHeader`.
#[derive(Debug, Clone)]
pub struct RefCountHeader {
    pub ref_count: i32,
}

pub type GcId = usize;
pub type RefCountId = usize;

impl GcObjectHeader {
    pub fn new(gc_obj_type: GcObjectType, ref_count: i32) -> Self {
        GcObjectHeader {
            ref_count,
            gc_obj_type,
            mark: 0,
            free_mark: false,
            list_prev: None,
            list_next: None,
        }
    }
}

impl RefCountHeader {
    pub fn new(ref_count: i32) -> Self {
        RefCountHeader {
            ref_count,
        }
    }
}
