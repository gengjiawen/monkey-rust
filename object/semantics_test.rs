use std::cmp::Ordering;

use crate::semantics::{structurally_equal, try_structurally_equal, HashKeyOrder};

/// The §10.2 definition, spelled out with allocations: rank, then the bytes of
/// the canonical text.
fn reference_order(left: &HashKeyOrder, right: &HashKeyOrder) -> Ordering {
    let key = |order: &HashKeyOrder| match order {
        HashKeyOrder::Integer(raw) => (0, raw.to_string().into_bytes()),
        HashKeyOrder::Boolean(raw) => (1, raw.to_string().into_bytes()),
        HashKeyOrder::String(raw) => (2, raw.as_bytes().to_vec()),
    };
    return key(left).cmp(&key(right));
}

#[test]
fn hash_key_order_matches_the_canonical_text_order() {
    let keys = [
        HashKeyOrder::Integer(i64::MIN),
        HashKeyOrder::Integer(i64::MIN + 1),
        HashKeyOrder::Integer(-10),
        HashKeyOrder::Integer(-2),
        HashKeyOrder::Integer(-1),
        HashKeyOrder::Integer(0),
        HashKeyOrder::Integer(1),
        HashKeyOrder::Integer(2),
        HashKeyOrder::Integer(9),
        HashKeyOrder::Integer(10),
        HashKeyOrder::Integer(100),
        HashKeyOrder::Integer(i64::MAX),
        HashKeyOrder::Boolean(false),
        HashKeyOrder::Boolean(true),
        HashKeyOrder::String(""),
        HashKeyOrder::String("1"),
        HashKeyOrder::String("10"),
        HashKeyOrder::String("2"),
        HashKeyOrder::String("a"),
        HashKeyOrder::String("ab"),
        HashKeyOrder::String("b"),
        HashKeyOrder::String("é"),
    ];
    for left in &keys {
        for right in &keys {
            assert_eq!(left.cmp(right), reference_order(left, right), "{:?} vs {:?}", left, right);
        }
    }

    let mut sorted = vec![
        HashKeyOrder::String("a"),
        HashKeyOrder::Integer(2),
        HashKeyOrder::Boolean(true),
        HashKeyOrder::Integer(-2),
        HashKeyOrder::Integer(10),
        HashKeyOrder::Integer(-1),
        HashKeyOrder::Integer(1),
    ];
    sorted.sort();
    assert_eq!(
        sorted,
        [
            HashKeyOrder::Integer(-1),
            HashKeyOrder::Integer(-2),
            HashKeyOrder::Integer(1),
            HashKeyOrder::Integer(10),
            HashKeyOrder::Integer(2),
            HashKeyOrder::Boolean(true),
            HashKeyOrder::String("a"),
        ]
    );
}

/// A toy value graph addressed by index, so tests can build shapes the
/// language cannot: cycles, and sharing counted exactly.
enum Node {
    Leaf(i64),
    List(Vec<usize>),
}

/// Equality over `nodes`, and how many pairs `compare` was asked about.
fn equal(nodes: &[Node], left: usize, right: usize) -> (bool, usize) {
    let mut asked = 0;
    let equal = structurally_equal(left, right, |left, right, descend| {
        asked += 1;
        match (&nodes[left], &nodes[right]) {
            (Node::Leaf(left), Node::Leaf(right)) => return left == right,
            (Node::List(items), Node::List(others)) => {
                if items.len() != others.len() {
                    return false;
                }
                if descend.first_visit(left, right) {
                    descend.extend(items.iter().copied().zip(others.iter().copied()));
                }
                return true;
            }
            _ => return false,
        }
    });
    return (equal, asked);
}

#[test]
fn equality_is_iterative_and_compares_shared_subtrees_once() {
    // Two separate 100,000-deep chains `[[[...[1]...]]]`.
    let depth = 100_000;
    let mut nodes = vec![Node::Leaf(1), Node::Leaf(1)];
    for level in 0..depth {
        nodes.push(Node::List(vec![2 * level]));
        nodes.push(Node::List(vec![2 * level + 1]));
    }
    let (left, right) = (nodes.len() - 2, nodes.len() - 1);
    assert_eq!(equal(&nodes, left, right), (true, depth + 1));
    nodes[1] = Node::Leaf(2);
    assert!(!equal(&nodes, left, right).0);

    // `x = [x', x']` 64 times over: 2^64 paths, 2 * 64 + 1 distinct pairs.
    let mut nodes = vec![Node::Leaf(7), Node::Leaf(7)];
    for level in 0..64 {
        nodes.push(Node::List(vec![2 * level, 2 * level]));
        nodes.push(Node::List(vec![2 * level + 1, 2 * level + 1]));
    }
    let (left, right) = (nodes.len() - 2, nodes.len() - 1);
    let (result, asked) = equal(&nodes, left, right);
    assert!(result);
    assert!(asked <= 2 * 64 + 1, "asked {} times", asked);
}

#[test]
fn equality_terminates_on_cycles() {
    // 0 = [1, 0] and 2 = [3, 2]: two identical self-referential lists.
    let nodes = vec![
        Node::List(vec![1, 0]),
        Node::Leaf(5),
        Node::List(vec![3, 2]),
        Node::Leaf(5),
    ];
    assert!(equal(&nodes, 0, 2).0);

    let nodes = vec![
        Node::List(vec![1, 0]),
        Node::Leaf(5),
        Node::List(vec![3, 2]),
        Node::Leaf(6),
    ];
    assert!(!equal(&nodes, 0, 2).0);
}

#[test]
fn same_handles_are_equal_without_asking() {
    let nodes = vec![Node::List(vec![0])];
    assert_eq!(equal(&nodes, 0, 0), (true, 0));
}

#[test]
fn a_failing_comparison_stops_the_traversal() {
    let result: Result<bool, &str> = try_structurally_equal(0, 1, |left, _, descend| {
        if left == 0 {
            descend.push(2, 3);
            return Ok(true);
        }
        return Err("dangling handle");
    });
    assert_eq!(result, Err("dangling handle"));
}
