#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::rc::Rc;

    use object::builtins::BuiltinId;
    use object::{CompiledFunction, Object};

    use crate::value::{
        alloc_value, export_object, get_value, get_value_mut, import_object, GcClass, GcInstance,
        HashKey, Value, ValueCell, ValueKind,
    };
    use crate::GcHeap;

    #[test]
    fn scalar_and_vm_support_value_kinds_are_distinct() {
        let values = [
            (Value::Integer(1), ValueKind::Integer),
            (Value::Boolean(true), ValueKind::Boolean),
            (Value::String("value".to_string()), ValueKind::String),
            (Value::Null, ValueKind::Null),
            (Value::Error("error".to_string()), ValueKind::Error),
            (
                Value::CompiledFunction(CompiledFunction {
                    name: "function".to_string(),
                    instructions: Vec::new(),
                    num_locals: 0,
                    num_parameters: 0,
                }),
                ValueKind::CompiledFunction,
            ),
            (Value::Builtin(BuiltinId::Len), ValueKind::Builtin),
        ];

        for (value, expected) in values {
            assert_eq!(value.kind(), expected);
        }
    }

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
        let node_a = alloc_value(&mut heap, Value::Array(vec![]));
        let node_b = alloc_value(&mut heap, Value::Array(vec![node_a]));

        let node_b_edge = heap.dup(node_b);
        match &mut heap
            .runtime_mut()
            .object_downcast_mut::<ValueCell>(node_a.0)
            .expect("node_a should be a ValueCell")
            .value
        {
            Value::Array(items) => items.push(node_b_edge),
            other => panic!("expected node_a array, got {:?}", other),
        }

        heap.free(node_a);
        heap.free(node_b);
        heap.run_gc();
        assert!(!heap.exists(node_a));
        assert!(!heap.exists(node_b));
    }

    #[test]
    fn scan_report_distinguishes_restored_class_and_instance_labels() {
        let mut heap = GcHeap::new();
        let class = alloc_value(
            &mut heap,
            Value::Class(GcClass {
                name: "Node".to_string(),
                constructor: None,
                methods: HashMap::new(),
            }),
        );
        let node_a = alloc_value(
            &mut heap,
            Value::Instance(GcInstance {
                class,
                fields: HashMap::new(),
            }),
        );
        let node_b = alloc_value(
            &mut heap,
            Value::Instance(GcInstance {
                class,
                fields: HashMap::new(),
            }),
        );

        let node_b_edge = heap.dup(node_b);
        match get_value_mut(&mut heap, node_a) {
            Value::Instance(instance) => {
                instance.fields.insert("next".to_string(), node_b_edge);
            }
            other => panic!("expected node_a instance, got {:?}", other),
        }
        let node_a_edge = heap.dup(node_a);
        match get_value_mut(&mut heap, node_b) {
            Value::Instance(instance) => {
                instance.fields.insert("next".to_string(), node_a_edge);
            }
            other => panic!("expected node_b instance, got {:?}", other),
        }

        // Keep node_a as the only external root. Scan must restore node_b and
        // their class after trial deletion temporarily moves both to `tmp`.
        heap.free(class);
        heap.free(node_b);
        let stats = heap.run_gc_with_stats();

        assert_eq!(stats.scan.restored, 2);
        assert_eq!(stats.scan.garbage_candidates, 0);
        assert_eq!(
            stats
                .scan
                .restored_objects
                .iter()
                .map(|object| (object.kind, object.label.clone()))
                .collect::<Vec<_>>(),
            vec![
                (ValueKind::Class, format!("Class(Node)#{}", class.0)),
                (ValueKind::Instance, format!("Instance(Node)#{}", node_b.0),),
            ]
        );
        assert!(stats.scan.garbage_candidate_objects.is_empty());
    }
}
