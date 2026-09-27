//! Parts of the frozen value semantics (arm64 backend design §10) that every
//! backend implements over its own value representation. They live here once
//! so the interpreter, the bytecode VM, the GC VM and the arm64 runtime cannot
//! drift apart on them.

use std::cmp::Ordering;
use std::collections::HashSet;
use std::convert::Infallible;
use std::hash::Hash;

/// A hash key as the canonical display order sees it (design §10.2): by type
/// rank — integer, boolean, string — then by the bytes of the key's canonical
/// text. Integers therefore order by their decimal text, so `{2: 2, 10: 10}`
/// prints as `{10: 10, 2: 2}` and `-1` comes before `-2`.
///
/// Comparing never allocates, so sorting a hash for display costs only the
/// sort itself.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HashKeyOrder<'a> {
    Integer(i64),
    Boolean(bool),
    String(&'a str),
}

impl HashKeyOrder<'_> {
    fn rank(&self) -> u8 {
        match self {
            HashKeyOrder::Integer(_) => return 0,
            HashKeyOrder::Boolean(_) => return 1,
            HashKeyOrder::String(_) => return 2,
        }
    }
}

impl Ord for HashKeyOrder<'_> {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self, other) {
            (HashKeyOrder::Integer(left), HashKeyOrder::Integer(right)) => {
                let (mut left_text, mut right_text) = ([0; 20], [0; 20]);
                return decimal(*left, &mut left_text).cmp(decimal(*right, &mut right_text));
            }
            // "false" < "true", byte for byte.
            (HashKeyOrder::Boolean(left), HashKeyOrder::Boolean(right)) => return left.cmp(right),
            (HashKeyOrder::String(left), HashKeyOrder::String(right)) => {
                return left.as_bytes().cmp(right.as_bytes());
            }
            _ => return self.rank().cmp(&other.rank()),
        }
    }
}

impl PartialOrd for HashKeyOrder<'_> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        return Some(self.cmp(other));
    }
}

/// The decimal text of `value`, written into the end of `buffer`; 20 bytes
/// hold `i64::MIN`.
fn decimal(value: i64, buffer: &mut [u8; 20]) -> &[u8] {
    let mut magnitude = value.unsigned_abs();
    let mut start = buffer.len();
    loop {
        start -= 1;
        buffer[start] = b'0' + (magnitude % 10) as u8;
        magnitude /= 10;
        if magnitude == 0 {
            break;
        }
    }
    if value < 0 {
        start -= 1;
        buffer[start] = b'-';
    }
    return &buffer[start..];
}

/// Frozen equality's traversal (design §10.1), shared by every backend's `==`.
/// `compare` answers one pair without descending: scalars by value, identity
/// types by reference, and aggregates by their shape (length, key set) after
/// queueing their children on the [`Descend`] it is handed.
///
/// The traversal runs on an explicit worklist rather than the call stack.
/// Nesting depth is a property of the *data*, and `a == b` is one step for
/// every backend, so an array a few thousand levels deep — which no engine has
/// any other trouble with — would otherwise answer by overflowing the native
/// stack.
///
/// Two handles that are equal as `T` denote the same value and compare equal
/// without calling `compare`. Comparing two scalars allocates nothing; the
/// worklist and the memo only come into being once an aggregate needs them.
pub fn structurally_equal<T: Copy + Eq + Hash>(
    left: T,
    right: T,
    mut compare: impl FnMut(T, T, &mut Descend<T>) -> bool,
) -> bool {
    let result: Result<bool, Infallible> =
        try_structurally_equal(left, right, |left, right, descend| {
            return Ok(compare(left, right, descend));
        });
    return result.unwrap_or_else(|never| match never {});
}

/// [`structurally_equal`] for a `compare` that can fail, such as one that
/// reads handles which may not resolve.
pub fn try_structurally_equal<T: Copy + Eq + Hash, E>(
    left: T,
    right: T,
    mut compare: impl FnMut(T, T, &mut Descend<T>) -> Result<bool, E>,
) -> Result<bool, E> {
    let mut descend = Descend {
        pending: Vec::new(),
        expanded: None,
    };
    let mut next = Some((left, right));
    while let Some((left, right)) = next {
        if left != right && !compare(left, right, &mut descend)? {
            return Ok(false);
        }
        next = descend.pending.pop();
    }
    return Ok(true);
}

/// The worklist [`structurally_equal`] hands to `compare`.
pub struct Descend<T> {
    pending: Vec<(T, T)>,
    expanded: Option<HashSet<(T, T)>>,
}

impl<T: Copy + Eq + Hash> Descend<T> {
    /// Claims an aggregate pair that passed its shape check; queue its
    /// children only when this returns `true`.
    ///
    /// `false` means the pair was expanded before. That first expansion did
    /// not disprove it — any inequality ends the whole comparison — so the
    /// pair is equal, or still being compared and assumed equal. The second
    /// reading is the co-inductive one that lets two structurally identical
    /// cyclic graphs compare equal instead of looping, and the first keeps a
    /// shared subtree from being compared once per path into it. Only
    /// aggregates are recorded, so scalar elements never grow the memo.
    pub fn first_visit(&mut self, left: T, right: T) -> bool {
        return self
            .expanded
            .get_or_insert_with(HashSet::new)
            .insert((left, right));
    }

    pub fn push(&mut self, left: T, right: T) {
        self.pending.push((left, right));
    }

    pub fn extend(&mut self, pairs: impl IntoIterator<Item = (T, T)>) {
        self.pending.extend(pairs);
    }
}
