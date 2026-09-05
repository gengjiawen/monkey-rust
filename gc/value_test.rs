#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::rc::Rc;

    use object::builtins::BuiltinId;
    use object::{CompiledFunction, Object};

    use crate::value::{
        alloc_value, call_builtin, export_object, get_value, get_value_mut, import_object,
        value_to_string, GcClass, GcInstance, HashKey, Value, ValueCell, ValueKind,
        MAX_VALUE_DISPLAY_CHARS, MAX_VALUE_DISPLAY_DEPTH,
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
    fn legacy_call_builtin_signature_still_works() {
        let mut heap = GcHeap::new();
        let null = alloc_value(&mut heap, Value::Null);
        let string = alloc_value(&mut heap, Value::String("monkey".to_string()));
        let result = call_builtin(&mut heap, BuiltinId::Len, &[string], null);

        assert_eq!(get_value(&heap, result), &Value::Integer(6));
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

    /// `[[…], […]]` nested `levels` deep: `levels` allocations, 2^levels leaves.
    fn shared_dag(heap: &mut GcHeap, levels: usize) -> crate::GcRef {
        let mut node = alloc_value(heap, Value::Integer(1));
        for _ in 0..levels {
            let left = heap.dup(node);
            let right = heap.dup(node);
            let parent = alloc_value(heap, Value::Array(vec![left, right]));
            heap.free(node);
            node = parent;
        }
        node
    }

    #[test]
    fn rendering_shared_structure_stays_within_the_character_budget() {
        // Every level doubles the fully expanded text, so an 18-level DAG —
        // 18 allocations — used to render 1.8 MB, and 22 levels took 20 s.
        let mut heap = GcHeap::new();
        let root = shared_dag(&mut heap, 24);

        let rendered = value_to_string(&heap, root);

        assert_eq!(rendered.chars().count(), MAX_VALUE_DISPLAY_CHARS);
        assert!(rendered.ends_with('…'), "expected a truncation marker: {}", &rendered[..40]);
    }

    #[test]
    fn rendering_deep_nesting_stops_at_the_depth_cap() {
        // Recursion is per level, so without a cap a few thousand levels —
        // well inside any instruction budget — overflow the native stack.
        let mut heap = GcHeap::new();
        let mut node = alloc_value(&mut heap, Value::Integer(1));
        for _ in 0..5_000 {
            node = alloc_value(&mut heap, Value::Array(vec![node]));
        }

        let rendered = value_to_string(&heap, node);

        // The cap's own `[…]` marker contributes the extra bracket.
        assert_eq!(rendered.matches('[').count(), MAX_VALUE_DISPLAY_DEPTH + 1);
        assert!(rendered.contains("[…]"), "expected a depth marker: {}", rendered);
    }

    #[test]
    fn rendering_ordinary_values_is_unchanged() {
        let mut heap = GcHeap::new();
        let one = alloc_value(&mut heap, Value::Integer(1));
        let two = alloc_value(&mut heap, Value::Integer(2));
        let array = alloc_value(&mut heap, Value::Array(vec![one, two]));
        let nested = alloc_value(&mut heap, Value::Array(vec![array]));
        let text = alloc_value(&mut heap, Value::String("hi".to_string()));

        assert_eq!(value_to_string(&heap, nested), "[[1, 2]]");
        assert_eq!(value_to_string(&heap, text), "hi");
    }

    #[test]
    fn tracked_bytes_grow_with_the_payload() {
        // The budget compares against this number, so a long string has to
        // cost more than a short one; slot-sized accounting made `"a" + "a"`
        // free no matter how many times it doubled.
        let mut heap = GcHeap::new();
        let before = heap.malloc_state().malloc_size;
        alloc_value(&mut heap, Value::String("a".to_string()));
        let small = heap.malloc_state().malloc_size - before;

        let before = heap.malloc_state().malloc_size;
        alloc_value(&mut heap, Value::String("a".repeat(4096)));
        let large = heap.malloc_state().malloc_size - before;

        assert!(large >= 4096, "small: {}, large: {}", small, large);
        assert!(small < 128, "a one-character string should not cost {} bytes", small);
    }

    #[test]
    fn freeing_returns_exactly_what_allocating_charged() {
        let mut heap = GcHeap::new();
        let baseline = heap.malloc_state().malloc_size;
        let reference = alloc_value(&mut heap, Value::String("a".repeat(4096)));
        assert!(heap.malloc_state().malloc_size > baseline);

        heap.free(reference);

        assert_eq!(heap.malloc_state().malloc_size, baseline);
    }
}
