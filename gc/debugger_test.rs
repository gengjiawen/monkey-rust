#[cfg(test)]
mod tests {
    use crate::debugger::{
        DebuggerHit, FrameView, SlotView, MAX_DEBUGGER_DISPLAY_CHARS, MAX_DEBUGGER_EDGES,
        MAX_DEBUGGER_HITS, MAX_DEBUGGER_MEMBERS, MAX_DEBUGGER_OBJECTS,
    };
    use crate::value::{EdgeRelation, ValueKind};
    use crate::{
        compile_source, run_source_with_debugger_classified, run_source_with_report,
        GcDebuggerRunOutcome, GcRunStage, GcVM,
    };

    const TEST_BUDGET: usize = 500_000;

    fn debug_run(source: &str) -> (String, String, Vec<DebuggerHit>, usize) {
        match run_source_with_debugger_classified(source, TEST_BUDGET) {
            GcDebuggerRunOutcome::Ok {
                result,
                stdout,
                hits,
                dropped_hits,
            } => (result, stdout, hits, dropped_hits),
            GcDebuggerRunOutcome::Error {
                error,
                ..
            } => panic!("expected success, got {:?}", error),
        }
    }

    fn frame_names(hit: &DebuggerHit) -> Vec<&str> {
        hit.frames.iter().map(|frame| frame.name.as_str()).collect()
    }

    fn local<'a>(frame: &'a FrameView, name: &str, slot: usize) -> &'a SlotView {
        frame
            .locals
            .iter()
            .find(|local| local.name == name && local.slot == slot)
            .unwrap_or_else(|| panic!("no local {}@{} in frame {}", name, slot, frame.name))
    }

    fn global<'a>(hit: &'a DebuggerHit, name: &str) -> &'a SlotView {
        hit.globals
            .iter()
            .find(|global| global.name == name)
            .unwrap_or_else(|| panic!("no global named {}", name))
    }

    #[test]
    fn hits_arrive_in_execution_order_with_full_frame_stacks() {
        let source = r#"
            let makePoint = fn(x, y) {
                let p = {"x": x, "y": y};
                debugger;
                return p;
            };
            let sum = fn(a, b) {
                let p = makePoint(a, b);
                debugger;
                return p["x"] + p["y"];
            };
            sum(1, 2);
        "#;
        let (result, _, hits, dropped) = debug_run(source);
        assert_eq!(result, "3");
        assert_eq!(dropped, 0);
        assert_eq!(hits.len(), 2);

        let first = &hits[0];
        let second = &hits[1];
        assert_eq!(first.index, 1);
        assert_eq!(second.index, 2);
        assert_eq!(frame_names(first), vec!["main", "sum", "makePoint"]);
        assert_eq!(frame_names(second), vec!["main", "sum"]);

        // Each hit's span points at its own `debugger;` statement.
        let first_span = first.span.clone().expect("first hit span");
        let second_span = second.span.clone().expect("second hit span");
        assert!(source[first_span.start..first_span.end].contains("debugger"));
        assert!(source[second_span.start..second_span.end].contains("debugger"));
        assert_ne!(first_span, second_span);

        // Scalars inline; the hash is a heap reference with a summary display.
        let make_point = &first.frames[2];
        assert_eq!(local(make_point, "x", 0).value.as_ref().unwrap().display, "1");
        assert_eq!(local(make_point, "x", 0).value.as_ref().unwrap().heap_id, None);
        assert_eq!(local(make_point, "y", 1).value.as_ref().unwrap().display, "2");
        let p = local(make_point, "p", 2).value.as_ref().unwrap();
        assert_eq!(p.kind, ValueKind::Hash);
        assert_eq!(p.display, "{x: 1, y: 2}");
        let p_id = p.heap_id.expect("hash lives on the heap");

        // The callee slot shows the closure being executed; main has none.
        let callee = make_point.callee.as_ref().expect("makePoint callee");
        assert_eq!(callee.kind, ValueKind::Closure);
        assert_eq!(callee.display, "[closure function]");
        assert_eq!(callee.heap_id, global(first, "makePoint").value.as_ref().unwrap().heap_id);
        assert!(first.frames[0].callee.is_none());
        assert!(first.frames[0].locals.is_empty());

        // Suspended frames never report temporaries, even though the callee
        // and its arguments physically sit inside their stack windows.
        for hit in &hits {
            for frame in &hit.frames {
                assert!(frame.temporaries.is_empty());
                assert!(frame.current_span.is_some());
            }
        }

        // The hash node carries its inline entries as members, in key order.
        let hash_node = first
            .heap
            .objects
            .iter()
            .find(|object| object.id == p_id)
            .expect("hash node selected");
        let member_views: Vec<(String, &EdgeRelation)> = hash_node
            .members
            .iter()
            .map(|member| (member.display.clone(), &member.relation))
            .collect();
        assert_eq!(member_views.len(), 2);
        assert_eq!(member_views[0].0, "1");
        assert_eq!(member_views[1].0, "2");
        assert!(matches!(
            member_views[0].1,
            EdgeRelation::HashValue { key, .. } if key == "x"
        ));

        // Compiled functions stay hidden without inflating omission counts.
        assert!(first
            .heap
            .objects
            .iter()
            .all(|object| object.kind != ValueKind::CompiledFunction));
        assert_eq!(first.heap.omitted_objects, 0);
        assert_eq!(first.heap.omitted_edges, 0);
    }

    #[test]
    fn uninitialized_globals_and_user_null_are_distinguished() {
        let source = r#"
            let ready = if (false) { 1 };
            let tool = len;
            debugger;
            let late = 2;
        "#;
        let (_, _, hits, _) = debug_run(source);
        let hit = &hits[0];

        // `ready` ran and produced a real null; it must render as a value.
        let ready = global(hit, "ready");
        assert!(ready.initialized);
        let ready_value = ready.value.as_ref().expect("user-assigned null is a value");
        assert_eq!(ready_value.kind, ValueKind::Null);
        assert_eq!(ready_value.display, "null");
        assert_eq!(ready_value.heap_id, None);

        let tool = global(hit, "tool").value.as_ref().expect("builtin value");
        assert_eq!(tool.kind, ValueKind::Builtin);
        assert_eq!(tool.display, "[builtin function]");
        assert_eq!(tool.heap_id, None);

        // `late` has a ledger entry but its `let` has not executed yet.
        let late = global(hit, "late");
        assert_eq!(late.slot, 2);
        assert!(!late.initialized);
        assert!(late.value.is_none());
    }

    #[test]
    fn branch_skipped_and_rebound_locals_report_slot_state() {
        let source = r#"
            let probe = fn() {
                let x = 1;
                let x = x + 1;
                if (false) { let ghost = 9; }
                debugger;
                return x;
            };
            probe();
        "#;
        let (result, _, hits, _) = debug_run(source);
        assert_eq!(result, "2");
        let frame = &hits[0].frames[1];

        // Rebinding keeps both slots visible with their own values.
        assert_eq!(local(frame, "x", 0).value.as_ref().unwrap().display, "1");
        assert_eq!(local(frame, "x", 1).value.as_ref().unwrap().display, "2");

        // The never-taken branch left its slot unwritten.
        let ghost = local(frame, "ghost", 2);
        assert!(!ghost.initialized);
        assert!(ghost.value.is_none());
    }

    #[test]
    fn captures_use_free_names_in_capture_order() {
        let source = r#"
            let outer = fn(a, b) {
                let inner = fn() {
                    debugger;
                    return b + a;
                };
                return inner();
            };
            outer(10, 20);
        "#;
        let (result, _, hits, _) = debug_run(source);
        assert_eq!(result, "30");
        let inner = &hits[0].frames[2];
        assert_eq!(inner.name, "inner");

        let captures: Vec<(&str, usize, &str)> = inner
            .captures
            .iter()
            .map(|capture| (capture.name.as_str(), capture.index, capture.value.display.as_str()))
            .collect();
        // `b` is referenced first inside `inner`, so it is capture 0.
        assert_eq!(captures, vec![("b", 0, "20"), ("a", 1, "10")]);
    }

    #[test]
    fn shared_references_appear_once_with_edges_from_both_owners() {
        let source = r#"
            let shared = [1, 2];
            let a = [shared];
            let b = [shared];
            debugger;
        "#;
        let (_, _, hits, _) = debug_run(source);
        let hit = &hits[0];

        let shared_id = global(hit, "shared")
            .value
            .as_ref()
            .unwrap()
            .heap_id
            .unwrap();
        let a_id = global(hit, "a").value.as_ref().unwrap().heap_id.unwrap();
        let b_id = global(hit, "b").value.as_ref().unwrap().heap_id.unwrap();
        assert_eq!(global(hit, "a").value.as_ref().unwrap().display, "[[1, 2]]");

        assert_eq!(hit.heap.objects.len(), 3);
        assert!(hit
            .heap
            .objects
            .iter()
            .all(|object| object.kind == ValueKind::Array));
        assert_eq!(
            hit.heap
                .objects
                .iter()
                .filter(|object| object.id == shared_id)
                .count(),
            1
        );

        let mut edge_pairs: Vec<(usize, usize)> = hit
            .heap
            .edges
            .iter()
            .map(|edge| (edge.from, edge.to))
            .collect();
        edge_pairs.sort_unstable();
        let mut expected = vec![(a_id, shared_id), (b_id, shared_id)];
        expected.sort_unstable();
        assert_eq!(edge_pairs, expected);
        assert_eq!(hit.heap.omitted_objects, 0);
        assert_eq!(hit.heap.omitted_edges, 0);
    }

    #[test]
    fn hidden_function_objects_stay_out_of_graph_and_counts() {
        let (_, _, hits, _) = debug_run("let f = fn() { return 1; };\ndebugger;");
        let hit = &hits[0];

        assert_eq!(hit.heap.objects.len(), 1);
        let closure = &hit.heap.objects[0];
        assert_eq!(closure.kind, ValueKind::Closure);
        assert_eq!(Some(closure.id), global(hit, "f").value.as_ref().unwrap().heap_id);
        // The closure's edge to its compiled function is presentation policy,
        // not truncation: no member, no edge, no omission.
        assert!(closure.members.is_empty());
        assert!(hit.heap.edges.is_empty());
        assert_eq!(hit.heap.omitted_objects, 0);
        assert_eq!(hit.heap.omitted_edges, 0);
    }

    #[test]
    fn object_budget_truncates_deterministically_without_dangling_edges() {
        let source = r#"
            let make = fn(n) {
                if (n == 0) { return [0]; }
                return [make(n - 1)];
            };
            let deep = make(120);
            debugger;
        "#;
        let (_, _, hits, _) = debug_run(source);
        let heap = &hits[0].heap;

        // 121 arrays + 1 closure exist; the budget keeps the first 100
        // reached from the roots (closure first, then the array chain).
        assert_eq!(heap.objects.len(), MAX_DEBUGGER_OBJECTS);
        assert_eq!(heap.omitted_objects, 22);
        assert_eq!(heap.objects[0].kind, ValueKind::Closure);

        let selected: std::collections::HashSet<usize> =
            heap.objects.iter().map(|object| object.id).collect();
        assert!(heap
            .edges
            .iter()
            .all(|edge| selected.contains(&edge.from) && selected.contains(&edge.to)));
        // The chain's 99 selected arrays link internally 98 times; the last
        // selected array's child missed the budget, dropping exactly one edge.
        assert_eq!(heap.edges.len(), 98);
        assert_eq!(heap.omitted_edges, 1);
    }

    #[test]
    fn edge_budget_counts_overflow_without_dropping_objects() {
        // 62 nodes fit the object budget, but kids' 60 element edges plus
        // spam's 200 repeated references to kids total 260 edges.
        let source = format!(
            "let kids = [{}];\nlet spam = [{}];\ndebugger;",
            vec!["[0]"; 60].join(", "),
            vec!["kids"; 200].join(", "),
        );
        let (_, _, hits, _) = debug_run(&source);
        let heap = &hits[0].heap;

        assert_eq!(heap.objects.len(), 62);
        assert_eq!(heap.omitted_objects, 0);
        assert_eq!(heap.edges.len(), MAX_DEBUGGER_EDGES);
        assert_eq!(heap.omitted_edges, 10);
    }

    #[test]
    fn display_depth_and_length_stay_bounded() {
        let long_text = "a".repeat(100);
        let source = format!(
            r#"
                let deep = [[[[1]]]];
                let wide = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9];
                let mixed = {{"b": 1, 3: 2, "a": 3}};
                let text = "{}";
                debugger;
            "#,
            long_text
        );
        let (_, _, hits, _) = debug_run(&source);
        let hit = &hits[0];

        // Depth 2: two array layers render, the third collapses.
        assert_eq!(global(hit, "deep").value.as_ref().unwrap().display, "[[[…]]]");
        // Element cap: eight entries, then an ellipsis entry.
        assert_eq!(
            global(hit, "wide").value.as_ref().unwrap().display,
            "[0, 1, 2, 3, 4, 5, 6, 7, …]"
        );
        // Hash entries sort by key (integers before strings).
        assert_eq!(global(hit, "mixed").value.as_ref().unwrap().display, "{3: 2, a: 3, b: 1}");
        // Character budget: 64 chars survive plus the ellipsis.
        let text = &global(hit, "text").value.as_ref().unwrap().display;
        assert_eq!(text.chars().count(), MAX_DEBUGGER_DISPLAY_CHARS + 1);
        assert!(text.ends_with('…'));

        let wide_id = global(hit, "wide").value.as_ref().unwrap().heap_id.unwrap();
        let wide_node = hit
            .heap
            .objects
            .iter()
            .find(|object| object.id == wide_id)
            .unwrap();
        assert_eq!(wide_node.members.len(), MAX_DEBUGGER_MEMBERS);
    }

    #[test]
    fn hits_cap_at_max_and_count_dropped() {
        let source = r#"
            let f = fn(n) {
                if (n > 0) {
                    debugger;
                    f(n - 1);
                }
            };
            f(30);
        "#;
        let (_, _, hits, dropped) = debug_run(source);
        assert_eq!(hits.len(), MAX_DEBUGGER_HITS);
        assert_eq!(dropped, 5);
        let indexes: Vec<usize> = hits.iter().map(|hit| hit.index).collect();
        assert_eq!(indexes, (1..=MAX_DEBUGGER_HITS).collect::<Vec<_>>());
        // Hit k fires k recursion levels deep: main plus k copies of f.
        assert_eq!(hits[0].frames.len(), 2);
        assert_eq!(hits[24].frames.len(), 26);
    }

    #[test]
    fn load_bytecode_clears_recorded_hits() {
        let mut vm = GcVM::new(compile_source("debugger;").expect("compiles"));
        vm.run_with_budget_classified(TEST_BUDGET).expect("runs");
        vm.load_bytecode(compile_source("1;").expect("compiles"));
        let (hits, dropped) = vm.take_debugger_hits();
        assert!(hits.is_empty());
        assert_eq!(dropped, 0);
    }

    #[test]
    fn snapshots_do_not_disturb_gc_reports() {
        let with_debugger =
            run_source_with_report("let a = [1, 2]; let b = [a]; debugger;", TEST_BUDGET)
                .expect("runs");
        let without_debugger =
            run_source_with_report("let a = [1, 2]; let b = [a];", TEST_BUDGET).expect("runs");
        assert_eq!(with_debugger.report, without_debugger.report);
    }

    #[test]
    fn runtime_failures_keep_hits_and_stdout() {
        let source = r#"
            puts("before");
            debugger;
            let x = 1;
            x();
        "#;
        match run_source_with_debugger_classified(source, TEST_BUDGET) {
            GcDebuggerRunOutcome::Error {
                error,
                stdout,
                hits,
                dropped_hits,
            } => {
                assert_eq!(error.stage, GcRunStage::Runtime);
                assert_eq!(error.kind, "call");
                assert_eq!(stdout, "before\n");
                assert_eq!(hits.len(), 1);
                assert_eq!(dropped_hits, 0);
            }
            GcDebuggerRunOutcome::Ok {
                result,
                ..
            } => panic!("expected runtime error, got {}", result),
        }
    }

    #[test]
    fn execution_limit_keeps_hits() {
        let source = r#"
            debugger;
            let f = fn() { return f(); };
            f();
        "#;
        match run_source_with_debugger_classified(source, 1_000) {
            GcDebuggerRunOutcome::Error {
                error,
                hits,
                ..
            } => {
                assert_eq!(error.kind, "executionLimit");
                assert_eq!(hits.len(), 1);
            }
            GcDebuggerRunOutcome::Ok {
                result,
                ..
            } => panic!("expected execution limit, got {}", result),
        }
    }

    #[test]
    fn frontend_failures_return_empty_recordings() {
        for (source, stage) in [
            ("let;", GcRunStage::Parse),
            ("unknown;", GcRunStage::Compile),
        ] {
            match run_source_with_debugger_classified(source, TEST_BUDGET) {
                GcDebuggerRunOutcome::Error {
                    error,
                    stdout,
                    hits,
                    dropped_hits,
                } => {
                    assert_eq!(error.stage, stage);
                    assert!(stdout.is_empty());
                    assert!(hits.is_empty());
                    assert_eq!(dropped_hits, 0);
                }
                GcDebuggerRunOutcome::Ok {
                    result,
                    ..
                } => panic!("expected {:?} error, got {}", stage, result),
            }
        }
    }

    #[test]
    fn methods_report_this_and_frame_name() {
        let source = r#"
            class Point {
                constructor(x) { this.x = x; }
                scaled(factor) {
                    let result = this.x * factor;
                    debugger;
                    return result;
                }
            }
            let p = new Point(3);
            p.scaled(2);
        "#;
        let (result, _, hits, _) = debug_run(source);
        assert_eq!(result, "6");
        let hit = &hits[0];
        // The compiler qualifies method names, and the debugger keeps that.
        assert_eq!(frame_names(hit), vec!["main", "Point.scaled"]);

        let frame = &hit.frames[1];
        let this = local(frame, "this", 0).value.as_ref().unwrap();
        assert_eq!(this.kind, ValueKind::Instance);
        assert_eq!(this.display, "[object Point]");
        let instance_id = this.heap_id.expect("instances live on the heap");
        assert_eq!(local(frame, "factor", 1).value.as_ref().unwrap().display, "2");
        assert_eq!(local(frame, "result", 2).value.as_ref().unwrap().display, "6");

        let point_class = global(hit, "Point").value.as_ref().unwrap();
        assert_eq!(point_class.kind, ValueKind::Class);
        assert_eq!(point_class.display, "[class Point]");

        // The instance node inlines its integer field and links to its class.
        let instance = hit
            .heap
            .objects
            .iter()
            .find(|object| object.id == instance_id)
            .expect("instance node selected");
        assert!(instance.label.starts_with("Instance(Point)"));
        assert!(instance.members.iter().any(|member| {
            matches!(&member.relation, EdgeRelation::InstanceField { name } if name == "x")
                && member.display == "3"
        }));
        assert!(hit.heap.edges.iter().any(|edge| {
            edge.from == instance_id
                && edge.to == point_class.heap_id.unwrap()
                && edge.relation == EdgeRelation::InstanceClass
        }));
    }

    #[test]
    fn self_referencing_instances_report_cycle_edges() {
        let source = r#"
            class N { constructor() { this.next = 0; } }
            let n = new N();
            n.next = n;
            debugger;
        "#;
        let (_, _, hits, _) = debug_run(source);
        let hit = &hits[0];
        let n_id = global(hit, "n").value.as_ref().unwrap().heap_id.unwrap();
        assert!(hit.heap.edges.iter().any(|edge| {
            edge.from == n_id
                && edge.to == n_id
                && edge.relation
                    == EdgeRelation::InstanceField {
                        name: "next".to_string(),
                    }
        }));
    }

    #[test]
    fn anonymous_functions_fall_back_to_placeholder_name() {
        let (result, _, hits, _) = debug_run("fn(x) { debugger; return x; }(5);");
        assert_eq!(result, "5");
        let hit = &hits[0];
        assert_eq!(frame_names(hit), vec!["main", "<anonymous>"]);
        assert_eq!(
            local(&hit.frames[1], "x", 0)
                .value
                .as_ref()
                .unwrap()
                .display,
            "5"
        );
    }
}
