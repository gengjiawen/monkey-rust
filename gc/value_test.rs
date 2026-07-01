#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::rc::Rc;

    use object::Object;

    use crate::value::{alloc_value, export_object, get_value, import_object, HashKey, Value};
    use crate::GcHeap;

    #[test]
    fn import_export_integer_roundtrip() {
        let mut heap = GcHeap::new();
        let original = Object::Integer(42);
        let reference = import_object(&mut heap, &original);
        assert_eq!(export_object(&heap, reference), original);
    }

    #[test]
    fn import_export_string_roundtrip() {
        let mut heap = GcHeap::new();
        let original = Object::String("monkey".to_string());
        let reference = import_object(&mut heap, &original);
        assert_eq!(export_object(&heap, reference), original);
    }

    #[test]
    fn import_export_array_roundtrip() {
        let mut heap = GcHeap::new();
        let original = Object::Array(vec![
            Rc::new(Object::Integer(1)),
            Rc::new(Object::String("two".to_string())),
            Rc::new(Object::Boolean(true)),
        ]);
        let reference = import_object(&mut heap, &original);
        assert_eq!(export_object(&heap, reference), original);
    }

    #[test]
    fn import_export_hash_roundtrip() {
        let mut heap = GcHeap::new();
        let original = Object::Hash(
            vec![
                (Rc::new(Object::Integer(1)), Rc::new(Object::Integer(10))),
                (
                    Rc::new(Object::String("k".to_string())),
                    Rc::new(Object::String("v".to_string())),
                ),
            ]
            .into_iter()
            .collect::<HashMap<_, _>>(),
        );
        let reference = import_object(&mut heap, &original);
        assert_eq!(export_object(&heap, reference), original);
    }

    #[test]
    fn import_export_nested_array_roundtrip() {
        let mut heap = GcHeap::new();
        let original = Object::Array(vec![Rc::new(Object::Array(vec![
            Rc::new(Object::Integer(1)),
            Rc::new(Object::Integer(2)),
        ]))]);
        let reference = import_object(&mut heap, &original);
        assert_eq!(export_object(&heap, reference), original);
    }

    #[test]
    fn hash_key_from_value_matches_object() {
        let mut heap = GcHeap::new();
        for object in [
            Object::Integer(7),
            Object::Boolean(false),
            Object::String("x".to_string()),
        ] {
            let reference = import_object(&mut heap, &object);
            assert_eq!(
                HashKey::from_value(get_value(&heap, reference)),
                HashKey::from_object(&object)
            );
        }
    }

    #[test]
    fn alloc_value_increments_child_refcounts() {
        let mut heap = GcHeap::new();
        let child = alloc_value(&mut heap, Value::Integer(1));
        assert_eq!(heap.ref_count(child), 1);

        let parent = alloc_value(&mut heap, Value::Array(vec![child]));
        assert_eq!(heap.ref_count(child), 2);

        heap.free(parent);
        assert_eq!(heap.ref_count(child), 1);
    }

    #[test]
    fn import_object_releases_temporary_child_refs() {
        let mut heap = GcHeap::new();
        let original = Object::Array(vec![Rc::new(Object::Array(vec![
            Rc::new(Object::Integer(1)),
            Rc::new(Object::Integer(2)),
        ]))]);

        let root = import_object(&mut heap, &original);
        let nested = match get_value(&heap, root) {
            Value::Array(items) => items[0],
            other => panic!("expected root array, got {:?}", other),
        };
        let leaves = match get_value(&heap, nested) {
            Value::Array(items) => items.clone(),
            other => panic!("expected nested array, got {:?}", other),
        };

        assert_eq!(heap.ref_count(root), 1);
        assert_eq!(heap.ref_count(nested), 1);
        for leaf in &leaves {
            assert_eq!(heap.ref_count(*leaf), 1);
        }

        heap.free(root);
        assert!(!heap.exists(root));
        assert!(!heap.exists(nested));
        for leaf in leaves {
            assert!(!heap.exists(leaf));
        }
    }

    #[test]
    fn value_cycle_collected_by_gc() {
        let mut heap = GcHeap::new();
        let node_b = alloc_value(&mut heap, Value::Null);
        let node_a = alloc_value(&mut heap, Value::Array(vec![node_b]));
        heap.free(node_b);
        let node_b = alloc_value(&mut heap, Value::Array(vec![node_a]));

        heap.free(node_a);
        heap.free(node_b);
        heap.run_gc();
        assert!(!heap.exists(node_a));
        assert!(!heap.exists(node_b));
    }
}
