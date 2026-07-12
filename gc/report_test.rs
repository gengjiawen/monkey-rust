#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use crate::report::{EdgeRelation, FinalFate, HashKeyKind, TrialDecision, MAX_EDGE_DETAILS};
    use crate::value::{
        alloc_value, format_hash_key_label, get_value_mut, GcBoundMethod, GcClass, GcClosure,
        GcInstance, HashKey, Value, ValueKind, MAX_HASH_KEY_LABEL_LEN,
    };
    use crate::{run_source_with_report, GcHeap, GcObject, GcRef};

    #[test]
    fn unrooted_two_node_cycle_reports_rc_formula_and_freed() {
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
        let edge_b = heap.dup(node_b);
        match get_value_mut(&mut heap, node_a) {
            Value::Instance(instance) => {
                instance.fields.insert("next".to_string(), edge_b);
            }
            other => panic!("{:?}", other),
        }
        let edge_a = heap.dup(node_a);
        match get_value_mut(&mut heap, node_b) {
            Value::Instance(instance) => {
                instance.fields.insert("next".to_string(), edge_a);
            }
            other => panic!("{:?}", other),
        }
        heap.free(class);
        heap.free(node_a);
        heap.free(node_b);

        let report = heap.run_gc_with_stats_bundle();
        // Class + two instances all reach trial RC 0 when the cycle is unrooted.
        assert_eq!(report.phases.trial_deletion.candidates, 3);
        assert_eq!(report.phases.scan.garbage_candidates, 3);
        assert_eq!(report.phases.free_cycles.freed, 3);

        let decisions = &report.phases.trial_deletion.object_decisions;
        let a = decisions
            .iter()
            .find(|d| d.object_id == node_a.0)
            .expect("node_a decision");
        assert_eq!(a.ref_count_before - a.heap_incoming_edges as i32, a.trial_ref_count);
        assert_eq!(a.trial_ref_count, 0);
        assert_eq!(a.decision, TrialDecision::Candidate);
        assert_eq!(a.final_fate, FinalFate::Freed);

        let next_edges: Vec<_> = report
            .phases
            .trial_deletion
            .visited_edges
            .iter()
            .filter(|edge| {
                matches!(
                    &edge.relation,
                    EdgeRelation::InstanceField { name } if name == "next"
                )
            })
            .collect();
        assert_eq!(next_edges.len(), 2);
    }

    #[test]
    fn self_cycle_reports_single_self_edge() {
        let mut heap = GcHeap::new();
        let class = alloc_value(
            &mut heap,
            Value::Class(GcClass {
                name: "Node".to_string(),
                constructor: None,
                methods: HashMap::new(),
            }),
        );
        let node = alloc_value(
            &mut heap,
            Value::Instance(GcInstance {
                class,
                fields: HashMap::new(),
            }),
        );
        let self_edge = heap.dup(node);
        match get_value_mut(&mut heap, node) {
            Value::Instance(instance) => {
                instance.fields.insert("next".to_string(), self_edge);
            }
            other => panic!("{:?}", other),
        }
        heap.free(class);
        heap.free(node);

        let report = heap.run_gc_with_stats_bundle();
        // Class and self-cyclic instance both become candidates.
        assert_eq!(report.phases.trial_deletion.candidates, 2);
        assert_eq!(report.phases.free_cycles.freed, 2);
        assert!(report
            .phases
            .trial_deletion
            .visited_edges
            .iter()
            .any(|edge| {
                edge.from_id == node.0
                    && edge.to_id == node.0
                    && matches!(
                        &edge.relation,
                        EdgeRelation::InstanceField { name } if name == "next"
                    )
            }));
    }

    #[test]
    fn rooted_nested_array_builds_witness_chain() {
        let mut heap = GcHeap::new();
        let leaf = alloc_value(&mut heap, Value::Array(vec![]));
        // alloc_value dups child edges, so pass the owned handle directly.
        let mid = alloc_value(&mut heap, Value::Array(vec![leaf]));
        heap.free(leaf);
        let root = alloc_value(&mut heap, Value::Array(vec![mid]));
        heap.free(mid);

        let report = heap.run_gc_with_stats_bundle();
        assert_eq!(report.phases.free_cycles.freed, 0);
        assert!(
            report.phases.scan.restored >= 2,
            "restored={}, garbage={:?}, decisions={:?}",
            report.phases.scan.restored,
            report.phases.scan.garbage_candidate_objects,
            report.phases.trial_deletion.object_decisions
        );

        let mid_witness = report
            .phases
            .scan
            .restoration_witnesses
            .iter()
            .find(|w| w.object_id == mid.0)
            .expect("mid witness");
        assert_eq!(mid_witness.root_id, root.0);
        assert_eq!(mid_witness.predecessor_id, root.0);
        assert!(matches!(
            mid_witness.relation,
            EdgeRelation::ArrayElement {
                index: 0
            }
        ));

        let leaf_witness = report
            .phases
            .scan
            .restoration_witnesses
            .iter()
            .find(|w| w.object_id == leaf.0)
            .expect("leaf witness");
        assert_eq!(leaf_witness.root_id, root.0);
        assert_eq!(leaf_witness.predecessor_id, mid.0);
        let _ = root;
    }

    #[test]
    fn duplicate_array_refs_count_two_incoming_edges() {
        let mut heap = GcHeap::new();
        let target = alloc_value(&mut heap, Value::Array(vec![]));
        let holder = alloc_value(&mut heap, Value::Array(vec![target, target]));
        heap.free(target);

        let report = heap.run_gc_with_stats_bundle();
        let target_decision = report
            .phases
            .trial_deletion
            .object_decisions
            .iter()
            .find(|d| d.object_id == target.0)
            .expect("target decision");
        assert_eq!(target_decision.heap_incoming_edges, 2);

        let element_edges: Vec<_> = report
            .phases
            .trial_deletion
            .visited_edges
            .iter()
            .filter(|edge| edge.from_id == holder.0 && edge.to_id == target.0)
            .collect();
        assert_eq!(element_edges.len(), 2);
        assert!(matches!(
            element_edges[0].relation,
            EdgeRelation::ArrayElement {
                index: 0
            }
        ));
        assert!(matches!(
            element_edges[1].relation,
            EdgeRelation::ArrayElement {
                index: 1
            }
        ));
        let _ = holder;
    }

    #[test]
    fn class_instance_and_method_relations_are_typed() {
        let mut heap = GcHeap::new();
        let func = alloc_value(&mut heap, Value::Integer(0));
        let free = heap.dup(func);
        let ctor = alloc_value(
            &mut heap,
            Value::Closure(GcClosure {
                func,
                free: vec![free],
            }),
        );
        let ctor_for_constructor = heap.dup(ctor);
        let ctor_for_method = heap.dup(ctor);
        let class = alloc_value(
            &mut heap,
            Value::Class(GcClass {
                name: "Node".to_string(),
                constructor: Some(ctor_for_constructor),
                methods: HashMap::from([("connect".to_string(), ctor_for_method)]),
            }),
        );
        let other = alloc_value(
            &mut heap,
            Value::Instance(GcInstance {
                class,
                fields: HashMap::new(),
            }),
        );
        let next = heap.dup(other);
        let node = alloc_value(
            &mut heap,
            Value::Instance(GcInstance {
                class,
                fields: HashMap::from([("next".to_string(), next)]),
            }),
        );

        let report = heap.run_gc_with_stats_bundle();
        let relations: Vec<_> = report
            .phases
            .trial_deletion
            .visited_edges
            .iter()
            .filter(|edge| {
                edge.from_id == class.0 || edge.from_id == node.0 || edge.from_id == ctor.0
            })
            .map(|edge| &edge.relation)
            .collect();
        assert!(relations
            .iter()
            .any(|r| matches!(r, EdgeRelation::ClassConstructor)));
        assert!(relations.iter().any(|r| matches!(
            r,
            EdgeRelation::ClassMethod { name } if name == "connect"
        )));
        assert!(relations
            .iter()
            .any(|r| matches!(r, EdgeRelation::InstanceClass)));
        assert!(relations.iter().any(|r| matches!(
            r,
            EdgeRelation::InstanceField { name } if name == "next"
        )));
        assert!(relations
            .iter()
            .any(|r| matches!(r, EdgeRelation::ClosureFunction)));
        assert!(relations.iter().any(|r| matches!(
            r,
            EdgeRelation::ClosureFree {
                index: 0
            }
        )));
        let _ = node;
    }

    #[test]
    fn hash_edges_are_stable_and_typed() {
        let mut heap = GcHeap::new();
        let value = alloc_value(&mut heap, Value::Integer(1));
        let v1 = heap.dup(value);
        let v2 = heap.dup(value);
        let v3 = heap.dup(value);
        let v4 = heap.dup(value);
        let map = alloc_value(
            &mut heap,
            Value::Hash(HashMap::from([
                (HashKey::String("b".to_string()), v1),
                (HashKey::String("a".to_string()), v2),
                (HashKey::Integer(1), v3),
                (HashKey::Boolean(true), v4),
            ])),
        );
        let report = heap.run_gc_with_stats_bundle();
        let hash_edges: Vec<_> = report
            .phases
            .trial_deletion
            .visited_edges
            .iter()
            .filter(|edge| edge.from_id == map.0)
            .collect();
        assert_eq!(hash_edges.len(), 4);
        assert!(matches!(
            &hash_edges[0].relation,
            EdgeRelation::HashValue { key_kind: HashKeyKind::Integer, key } if key == "1"
        ));
        assert!(matches!(
            &hash_edges[1].relation,
            EdgeRelation::HashValue { key_kind: HashKeyKind::Boolean, key } if key == "true"
        ));
        assert!(matches!(
            &hash_edges[2].relation,
            EdgeRelation::HashValue { key_kind: HashKeyKind::String, key } if key == "a"
        ));
        assert!(matches!(
            &hash_edges[3].relation,
            EdgeRelation::HashValue { key_kind: HashKeyKind::String, key } if key == "b"
        ));
    }

    #[test]
    fn non_value_gc_object_falls_back_to_unknown_relation() {
        struct Link {
            next: Option<crate::GcId>,
        }
        impl GcObject for Link {
            fn trace(&self, visit: &mut dyn FnMut(crate::GcId)) {
                if let Some(next) = self.next {
                    visit(next);
                }
            }
        }

        let mut heap = GcHeap::new();
        let child = heap.alloc(
            Link {
                next: None,
            },
            crate::GcObjectType::MonkeyObject,
        );
        let child_id = child.0;
        let child_edge = heap.dup(child).0;
        let parent = heap.alloc(
            Link {
                next: Some(child_edge),
            },
            crate::GcObjectType::MonkeyObject,
        );
        heap.free(child);

        let report = heap.run_gc_with_stats_bundle();
        assert!(report
            .phases
            .trial_deletion
            .visited_edges
            .iter()
            .any(|edge| {
                edge.from_id == parent.0
                    && edge.to_id == child_id
                    && matches!(edge.relation, EdgeRelation::Unknown)
            }));
        assert_eq!(
            report
                .phases
                .trial_deletion
                .visited_edges
                .iter()
                .filter(|edge| edge.from_id == parent.0)
                .count(),
            1
        );
        let _ = parent;
    }

    #[test]
    fn visit_edges_targets_match_trace_targets() {
        fn assert_same_target_multiset(value: &Value) {
            let mut via_trace = Vec::new();
            value.trace(&mut |reference| via_trace.push(reference.0));
            via_trace.sort_unstable();

            let mut via_visit = Vec::new();
            value.visit_edges(|_relation, reference| via_visit.push(reference.0));
            via_visit.sort_unstable();

            assert_eq!(via_trace, via_visit);
        }

        let values = [
            Value::Array(vec![GcRef(1), GcRef(2)]),
            Value::Hash(HashMap::from([
                (HashKey::Integer(1), GcRef(3)),
                (HashKey::String("key".to_string()), GcRef(4)),
            ])),
            Value::Closure(GcClosure {
                func: GcRef(5),
                free: vec![GcRef(6)],
            }),
            Value::Class(GcClass {
                name: "Node".to_string(),
                constructor: Some(GcRef(7)),
                methods: HashMap::from([("method".to_string(), GcRef(8))]),
            }),
            Value::Instance(GcInstance {
                class: GcRef(9),
                fields: HashMap::from([("field".to_string(), GcRef(10))]),
            }),
            Value::BoundMethod(GcBoundMethod {
                receiver: GcRef(11),
                method: GcRef(12),
                name: "method".to_string(),
            }),
        ];

        for value in &values {
            assert_same_target_multiset(value);
        }
    }

    #[test]
    fn hash_key_labels_truncate_only_at_encoded_boundaries() {
        let exact = "a".repeat(MAX_HASH_KEY_LABEL_LEN);
        assert_eq!(format_hash_key_label(&HashKey::String(exact.clone())), exact);

        let unicode_boundary = format!("{}中", "a".repeat(MAX_HASH_KEY_LABEL_LEN - 1));
        assert_eq!(
            format_hash_key_label(&HashKey::String(unicode_boundary)),
            format!("{}…", "a".repeat(MAX_HASH_KEY_LABEL_LEN - 1))
        );

        let escape_boundary = format!("{}\n", "a".repeat(MAX_HASH_KEY_LABEL_LEN - 1));
        assert_eq!(
            format_hash_key_label(&HashKey::String(escape_boundary)),
            format!("{}…", "a".repeat(MAX_HASH_KEY_LABEL_LEN - 1))
        );
    }

    #[test]
    fn aggregate_incoming_equals_edges_visited() {
        let success = run_source_with_report(
            r#"
                class Node { connect(other) { this.next = other; } }
                let makeCycle = fn() {
                  let a = new Node();
                  let b = new Node();
                  a.connect(b);
                  b.connect(a);
                };
                makeCycle();
            "#,
            10_000,
        )
        .unwrap();
        let incoming_sum: usize = success
            .report
            .phases
            .trial_deletion
            .object_decisions
            .iter()
            .map(|d| d.heap_incoming_edges)
            .sum();
        // When no truncation of decisions, sum of reported incoming may be less than
        // edgesVisited if some objects were omitted. With default limits this should match.
        if success
            .report
            .phases
            .trial_deletion
            .omitted_object_decisions
            == 0
        {
            assert_eq!(incoming_sum, success.report.phases.trial_deletion.edges_visited);
        }
        assert!(success
            .report
            .objects
            .iter()
            .any(|o| o.kind == ValueKind::Instance));
    }

    #[test]
    fn second_empty_gc_has_consistent_empty_details() {
        let mut vm = {
            let program = parser::parse(
                r#"
                    class Node { connect(other) { this.next = other; } }
                    let makeCycle = fn() {
                      let a = new Node();
                      let b = new Node();
                      a.connect(b);
                      b.connect(a);
                    };
                    makeCycle();
                "#,
            )
            .unwrap();
            let mut compiler = compiler::compiler::Compiler::new();
            let bytecode = compiler.compile(&program).unwrap();
            let mut vm = crate::GcVM::new(bytecode);
            vm.heap_mut().set_gc_threshold(usize::MAX);
            vm.run();
            vm
        };
        let _ = vm.collect_garbage();
        let second = vm.collect_garbage();
        assert_eq!(second.phases.free_cycles.freed, 0);
        assert_eq!(
            second.phases.trial_deletion.visited_edges.len()
                + second.phases.trial_deletion.omitted_edge_details,
            second.phases.trial_deletion.edges_visited
        );
        assert_eq!(
            second.phases.scan.restoration_witnesses.len() + second.phases.scan.omitted_witnesses,
            second.phases.scan.restored
        );
    }

    #[test]
    fn truncation_preserves_aggregates_and_omitted_counts() {
        let mut heap = GcHeap::new();
        let target = alloc_value(&mut heap, Value::Integer(1));
        let mut items = Vec::new();
        for _ in 0..(MAX_EDGE_DETAILS + 20) {
            items.push(heap.dup(target));
        }
        let _holder = alloc_value(&mut heap, Value::Array(items));
        let report = heap.run_gc_with_stats_bundle();
        assert!(report.phases.trial_deletion.edges_visited > MAX_EDGE_DETAILS);
        assert_eq!(report.phases.trial_deletion.visited_edges.len(), MAX_EDGE_DETAILS);
        assert_eq!(
            report.phases.trial_deletion.visited_edges.len()
                + report.phases.trial_deletion.omitted_edge_details,
            report.phases.trial_deletion.edges_visited
        );
        for edge in &report.phases.trial_deletion.visited_edges {
            assert!(report.objects.iter().any(|o| o.id == edge.from_id));
            assert!(report.objects.iter().any(|o| o.id == edge.to_id));
        }
    }

    #[test]
    fn vm_bookkeeping_null_survivor_keeps_rc_formula() {
        let success = run_source_with_report("1;", 10_000).unwrap();
        let null_decision = success
            .report
            .phases
            .trial_deletion
            .object_decisions
            .iter()
            .find(|d| {
                success
                    .report
                    .objects
                    .iter()
                    .any(|o| o.id == d.object_id && o.label.starts_with("Null#"))
            });
        if let Some(decision) = null_decision {
            assert!(decision.ref_count_before > 1_000);
            assert_eq!(
                decision.trial_ref_count,
                decision.ref_count_before - decision.heap_incoming_edges as i32
            );
            assert_eq!(decision.decision, TrialDecision::Survivor);
        }
    }
}
