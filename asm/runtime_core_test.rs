//! Dual-backend semantics tests (design §10.3): the same corpus runs on
//! `PointerStore` (native tagged pointers) and `HandleStore` (arena indices),
//! comparing canonical values, error kinds, and `CallDispatch` shapes.

use object::builtins::BuiltinId;

use crate::runtime_backend::{HandleStore, PointerStore, ValueStore};
use crate::runtime_core::*;

/// Runs the scenario on both storage backends and asserts they produce the
/// same observable outcome.
fn on_both_backends<T: PartialEq + std::fmt::Debug>(
    scenario: impl Fn(&mut dyn DynStore) -> T,
) -> T {
    let mut pointer_store = PointerStore;
    let mut handle_store = HandleStore::new();
    let from_pointers = scenario(&mut pointer_store);
    let from_handles = scenario(&mut handle_store);
    assert_eq!(from_pointers, from_handles, "PointerStore and HandleStore disagree");
    from_pointers
}

/// Object-safe wrapper so the corpus can be written once; `runtime_core`
/// itself stays generic over `impl ValueStore`.
trait DynStore {
    fn as_store(&mut self) -> &mut dyn ValueStore;
}

impl<S: ValueStore> DynStore for S {
    fn as_store(&mut self) -> &mut dyn ValueStore {
        self
    }
}

impl ValueStore for &mut dyn ValueStore {
    fn alloc(&mut self, object: HeapObject) -> Value {
        (**self).alloc(object)
    }
    fn try_get(&self, value: Value) -> Option<&HeapObject> {
        (**self).try_get(value)
    }
    fn try_get_mut(&mut self, value: Value) -> Option<&mut HeapObject> {
        (**self).try_get_mut(value)
    }
}

fn kind_of<T>(result: RuntimeResult<T>) -> RuntimeErrorKind {
    result.err().expect("expected a runtime failure").kind
}

#[test]
fn smi_encoding_boundaries() {
    assert!(i64_fits_smi(SMI_MIN));
    assert!(i64_fits_smi(SMI_MAX));
    assert!(!i64_fits_smi(SMI_MAX + 1));
    assert!(!i64_fits_smi(i64::MIN));
    assert_eq!(smi_to_i64(smi_from_i64(SMI_MIN)), SMI_MIN);
    assert_eq!(smi_to_i64(smi_from_i64(SMI_MAX)), SMI_MAX);
    assert_eq!(smi_to_i64(smi_from_i64(-1)), -1);
    assert!(is_smi(smi_from_i64(0)));
}

#[test]
fn singleton_and_tag_encodings() {
    // false/null share low three bits; detection must compare full values.
    assert_eq!(FALSE_VALUE & 0b111, NULL_VALUE & 0b111);
    assert!(!is_heap(FALSE_VALUE));
    assert!(!is_heap(NULL_VALUE));
    assert!(!is_builtin(TRUE_VALUE));
    assert_eq!(builtin_value(BuiltinId::Puts) & 0b111, 0b101);
    assert!(is_builtin(builtin_value(BuiltinId::Len)));
}

#[test]
fn integers_shrink_and_box_across_backends() {
    on_both_backends(|store| {
        let mut store = store.as_store();
        let boxed = make_int(&mut store, i64::MAX);
        assert!(is_heap(boxed));
        assert_eq!(int_value(&store, boxed), Some(i64::MAX));
        let smi = make_int(&mut store, 42);
        assert!(is_smi(smi));
        // Arithmetic results shrink back to SMI when they fit.
        let shrunk = sub(&mut store, boxed, boxed).expect("MAX - MAX");
        assert!(is_smi(shrunk));
        assert_eq!(int_value(&store, shrunk), Some(0));
        canonical_value(&store, boxed).unwrap()
    });
}

#[test]
fn checked_arithmetic_is_fatal_at_the_boundaries() {
    on_both_backends(|store| {
        let mut store = store.as_store();
        let max = make_int(&mut store, i64::MAX);
        let min = make_int(&mut store, i64::MIN);
        let one = make_int(&mut store, 1);
        let two = make_int(&mut store, 2);
        let neg_one = make_int(&mut store, -1);
        let zero = make_int(&mut store, 0);
        let kinds = vec![
            kind_of(add(&mut store, max, one)),
            kind_of(sub(&mut store, min, one)),
            kind_of(mul(&mut store, max, two)),
            kind_of(div(&mut store, min, neg_one)),
            kind_of(minus(&mut store, min)),
            kind_of(div(&mut store, one, zero)),
            kind_of(add(&mut store, one, TRUE_VALUE)),
        ];
        assert_eq!(
            kinds,
            vec![
                RuntimeErrorKind::IntegerOverflow,
                RuntimeErrorKind::IntegerOverflow,
                RuntimeErrorKind::IntegerOverflow,
                RuntimeErrorKind::IntegerOverflow,
                RuntimeErrorKind::IntegerOverflow,
                RuntimeErrorKind::DivisionByZero,
                RuntimeErrorKind::TypeError,
            ]
        );
        // Valid division truncates toward zero.
        let neg_seven = make_int(&mut store, -7);
        let quotient = div(&mut store, neg_seven, two).unwrap();
        assert_eq!(int_value(&store, quotient), Some(-3));
        kinds
    });
}

#[test]
fn add_concatenates_strings() {
    on_both_backends(|store| {
        let mut store = store.as_store();
        let hello = string_from_utf8(&mut store, b"hello ").unwrap();
        let world = string_from_utf8(&mut store, b"world").unwrap();
        let joined = add(&mut store, hello, world).unwrap();
        let text = display(&store, joined).unwrap();
        assert_eq!(text, "hello world");
        text
    });
}

#[test]
fn equality_matrix() {
    on_both_backends(|store| {
        let mut store = store.as_store();
        // Integer equality is raw-value equality across representations.
        let boxed = store.alloc(HeapObject::BoxedInt(7));
        assert!(eq_values(&store, boxed, smi_from_i64(7)).unwrap());
        // Scalars by value; distinct types are unequal.
        assert!(eq_values(&store, TRUE_VALUE, TRUE_VALUE).unwrap());
        assert!(!eq_values(&store, TRUE_VALUE, NULL_VALUE).unwrap());
        assert!(!eq_values(&store, smi_from_i64(0), FALSE_VALUE).unwrap());
        // Builtins by id: print aliases puts.
        assert!(eq_values(
            &store,
            builtin_value(BuiltinId::Puts),
            builtin_value(builtin_id_for_symbol_index(6).unwrap()),
        )
        .unwrap());
        // Aggregates recursively, independent of insertion order.
        let a1 = array_from_values(&mut store, &[smi_from_i64(1), smi_from_i64(2)]);
        let a2 = array_from_values(&mut store, &[smi_from_i64(1), smi_from_i64(2)]);
        let a3 = array_from_values(&mut store, &[smi_from_i64(2), smi_from_i64(1)]);
        assert!(eq_values(&store, a1, a2).unwrap());
        assert!(!eq_values(&store, a1, a3).unwrap());
        let key_a = string_from_utf8(&mut store, b"a").unwrap();
        let key_b = string_from_utf8(&mut store, b"b").unwrap();
        let h1 =
            hash_from_pairs(&mut store, &[key_a, smi_from_i64(1), key_b, smi_from_i64(2)]).unwrap();
        let h2 =
            hash_from_pairs(&mut store, &[key_b, smi_from_i64(2), key_a, smi_from_i64(1)]).unwrap();
        assert!(eq_values(&store, h1, h2).unwrap());
        // Identity types compare by object identity.
        let c1 = closure_new(&mut store, 100, 0, &[]).unwrap();
        let c2 = closure_new(&mut store, 100, 0, &[]).unwrap();
        assert!(eq_values(&store, c1, c1).unwrap());
        assert!(!eq_values(&store, c1, c2).unwrap());
        // Different types (array vs hash) are unequal, not an error.
        assert!(!eq_values(&store, a1, h1).unwrap());
        true
    });
}

#[test]
fn gt_accepts_integers_only() {
    on_both_backends(|store| {
        let mut store = store.as_store();
        assert_eq!(gt(&store, smi_from_i64(3), smi_from_i64(2)).unwrap(), TRUE_VALUE);
        let boxed = make_int(&mut store, i64::MAX);
        assert_eq!(gt(&store, boxed, smi_from_i64(5)).unwrap(), TRUE_VALUE);
        assert_eq!(kind_of(gt(&store, TRUE_VALUE, FALSE_VALUE)), RuntimeErrorKind::TypeError);
        true
    });
}

#[test]
fn truthiness_and_bang() {
    // Only false and null are falsy; !v is exactly !truthy(v).
    assert!(!truthy(FALSE_VALUE));
    assert!(!truthy(NULL_VALUE));
    assert!(truthy(TRUE_VALUE));
    assert!(truthy(smi_from_i64(0)));
    assert_eq!(bang(NULL_VALUE), TRUE_VALUE);
    assert_eq!(bang(smi_from_i64(0)), FALSE_VALUE);
    assert_eq!(bang(TRUE_VALUE), FALSE_VALUE);
}

#[test]
fn index_semantics() {
    on_both_backends(|store| {
        let mut store = store.as_store();
        let array = array_from_values(&mut store, &[smi_from_i64(10), smi_from_i64(20)]);
        assert_eq!(int_value(&store, index(&store, array, smi_from_i64(1)).unwrap()), Some(20));
        // Out of bounds and missing keys yield null.
        assert_eq!(index(&store, array, smi_from_i64(2)).unwrap(), NULL_VALUE);
        assert_eq!(index(&store, array, smi_from_i64(-1)).unwrap(), NULL_VALUE);
        let key = string_from_utf8(&mut store, b"k").unwrap();
        let hash = hash_from_pairs(&mut store, &[key, smi_from_i64(9)]).unwrap();
        let other = string_from_utf8(&mut store, b"missing").unwrap();
        assert_eq!(int_value(&store, index(&store, hash, key).unwrap()), Some(9));
        assert_eq!(index(&store, hash, other).unwrap(), NULL_VALUE);
        // Wrong container / key types.
        assert_eq!(
            kind_of(index(&store, smi_from_i64(1), smi_from_i64(0))),
            RuntimeErrorKind::TypeError
        );
        assert_eq!(kind_of(index(&store, hash, array)), RuntimeErrorKind::InvalidHashKey);
        assert_eq!(kind_of(index(&store, array, TRUE_VALUE)), RuntimeErrorKind::TypeError);
        // Unhashable hash literal key is fatal at construction.
        assert_eq!(
            kind_of(hash_from_pairs(&mut store, &[array, smi_from_i64(1)])),
            RuntimeErrorKind::InvalidHashKey
        );
        true
    });
}

#[test]
fn builtins_semantics_and_errors() {
    on_both_backends(|store| {
        let mut store = store.as_store();
        let mut sink = BufferSink::new();
        let text = string_from_utf8(&mut store, b"abc").unwrap();
        let array = array_from_values(&mut store, &[smi_from_i64(1), smi_from_i64(2)]);
        let empty = array_from_values(&mut store, &[]);

        let len = call_builtin(&mut store, &mut sink, BuiltinId::Len, &[text]).unwrap();
        assert_eq!(int_value(&store, len), Some(3));
        let first = call_builtin(&mut store, &mut sink, BuiltinId::First, &[array]).unwrap();
        assert_eq!(int_value(&store, first), Some(1));
        let last = call_builtin(&mut store, &mut sink, BuiltinId::Last, &[array]).unwrap();
        assert_eq!(int_value(&store, last), Some(2));
        assert_eq!(
            call_builtin(&mut store, &mut sink, BuiltinId::First, &[empty]).unwrap(),
            NULL_VALUE
        );
        let rest = call_builtin(&mut store, &mut sink, BuiltinId::Rest, &[array]).unwrap();
        assert_eq!(display(&store, rest).unwrap(), "[2]");
        assert_eq!(
            call_builtin(&mut store, &mut sink, BuiltinId::Rest, &[empty]).unwrap(),
            NULL_VALUE
        );
        let pushed =
            call_builtin(&mut store, &mut sink, BuiltinId::Push, &[array, smi_from_i64(3)])
                .unwrap();
        assert_eq!(display(&store, pushed).unwrap(), "[1, 2, 3]");
        // push is persistent-style: the original array is untouched.
        assert_eq!(display(&store, array).unwrap(), "[1, 2]");

        // puts writes display + newline per argument and returns null.
        let result =
            call_builtin(&mut store, &mut sink, BuiltinId::Puts, &[text, smi_from_i64(5)]).unwrap();
        assert_eq!(result, NULL_VALUE);
        assert_eq!(String::from_utf8(sink.bytes.clone()).unwrap(), "abc\n5\n");

        // Arity/type problems are terminating error kinds, not values.
        assert_eq!(
            kind_of(call_builtin(&mut store, &mut sink, BuiltinId::Len, &[text, text])),
            RuntimeErrorKind::ArityError
        );
        assert_eq!(
            kind_of(call_builtin(&mut store, &mut sink, BuiltinId::Len, &[smi_from_i64(1)])),
            RuntimeErrorKind::TypeError
        );
        assert_eq!(
            kind_of(call_builtin(&mut store, &mut sink, BuiltinId::Push, &[text, text])),
            RuntimeErrorKind::TypeError
        );
        true
    });
}

#[test]
fn display_uses_stable_hash_ordering() {
    on_both_backends(|store| {
        let mut store = store.as_store();
        let key_str = string_from_utf8(&mut store, b"s").unwrap();
        let hash = hash_from_pairs(
            &mut store,
            &[
                key_str,
                smi_from_i64(3),
                TRUE_VALUE,
                smi_from_i64(2),
                smi_from_i64(10),
                smi_from_i64(1),
            ],
        )
        .unwrap();
        // Rank: integer < boolean < string.
        let text = display(&store, hash).unwrap();
        assert_eq!(text, "{10: 1, true: 2, s: 3}");
        let nested = array_from_values(&mut store, &[hash, NULL_VALUE]);
        assert_eq!(display(&store, nested).unwrap(), "[{10: 1, true: 2, s: 3}, null]");
        text
    });
}

#[test]
fn canonical_value_json_shapes() {
    on_both_backends(|store| {
        let mut store = store.as_store();
        let max = make_int(&mut store, i64::MAX);
        assert_eq!(
            canonical_value(&store, max).unwrap(),
            "{\"type\":\"integer\",\"value\":\"9223372036854775807\"}"
        );
        assert_eq!(
            canonical_value(&store, smi_from_i64(-42)).unwrap(),
            "{\"type\":\"integer\",\"value\":\"-42\"}"
        );
        assert_eq!(
            canonical_value(&store, TRUE_VALUE).unwrap(),
            "{\"type\":\"boolean\",\"value\":true}"
        );
        assert_eq!(canonical_value(&store, NULL_VALUE).unwrap(), "{\"type\":\"null\"}");
        let text = string_from_utf8(&mut store, "a\"b\\c\nd".as_bytes()).unwrap();
        assert_eq!(
            canonical_value(&store, text).unwrap(),
            "{\"type\":\"string\",\"value\":\"a\\\"b\\\\c\\nd\"}"
        );
        assert_eq!(
            canonical_value(&store, builtin_value(BuiltinId::Puts)).unwrap(),
            "{\"type\":\"builtin\",\"id\":\"puts\"}"
        );
        let closure = closure_new(&mut store, 7, 0, &[]).unwrap();
        assert_eq!(canonical_value(&store, closure).unwrap(), "{\"type\":\"function\"}");
        let key_int = smi_from_i64(2);
        let key_str = string_from_utf8(&mut store, b"k").unwrap();
        let hash =
            hash_from_pairs(&mut store, &[key_str, NULL_VALUE, key_int, TRUE_VALUE]).unwrap();
        let array = array_from_values(&mut store, &[hash]);
        let json = canonical_value(&store, array).unwrap();
        assert_eq!(
            json,
            "{\"type\":\"array\",\"elements\":[{\"type\":\"hash\",\"entries\":[\
             {\"key\":{\"type\":\"integer\",\"value\":\"2\"},\"value\":{\"type\":\"boolean\",\"value\":true}},\
             {\"key\":{\"type\":\"string\",\"value\":\"k\"},\"value\":{\"type\":\"null\"}}]}]}"
        );
        json
    });
}

#[test]
fn class_property_and_bound_method_semantics() {
    on_both_backends(|store| {
        let mut store = store.as_store();
        let class = class_new(&mut store, "Point");
        let method = closure_new(&mut store, 500, 1, &[]).unwrap(); // this only
        class_add_method(&mut store, class, "show", method, false).unwrap();

        assert_eq!(display(&store, class).unwrap(), "[class Point]");
        assert_eq!(
            canonical_value(&store, class).unwrap(),
            "{\"type\":\"class\",\"name\":\"Point\"}"
        );

        // Instances via construct dispatch (no constructor, zero args).
        let dispatch = dispatch_construct(&mut store, class, &[]).unwrap();
        let instance = match dispatch {
            CallDispatch::Return(instance) => instance,
            other => panic!("expected Return, got {:?}", other),
        };
        assert_eq!(display(&store, instance).unwrap(), "[object Point]");
        assert_eq!(
            canonical_value(&store, instance).unwrap(),
            "{\"type\":\"instance\",\"class\":\"Point\"}"
        );

        // Field write/read; fields shadow methods.
        set_property(&mut store, instance, "x", smi_from_i64(4)).unwrap();
        let x_value = get_property(&mut store, instance, "x").unwrap();
        assert_eq!(int_value(&store, x_value), Some(4));
        let bound = get_property(&mut store, instance, "show").unwrap();
        assert_eq!(display(&store, bound).unwrap(), "[bound method Point.show]");
        assert_eq!(
            canonical_value(&store, bound).unwrap(),
            "{\"type\":\"bound_method\",\"class\":\"Point\",\"method\":\"show\"}"
        );
        assert_eq!(
            kind_of(get_property(&mut store, instance, "missing")),
            RuntimeErrorKind::MissingProperty
        );
        assert_eq!(
            kind_of(set_property(&mut store, smi_from_i64(1), "x", NULL_VALUE)),
            RuntimeErrorKind::TypeError
        );
        assert_eq!(kind_of(get_property(&mut store, class, "show")), RuntimeErrorKind::TypeError);
        true
    });
}

#[test]
fn call_dispatch_matrix() {
    on_both_backends(|store| {
        let mut store = store.as_store();
        let mut sink = BufferSink::new();

        // Closure: arity checked, Invoke carries code/closure/args.
        let closure = closure_new(&mut store, 777, 2, &[NULL_VALUE]).unwrap();
        let dispatch =
            dispatch_call(&mut store, &mut sink, closure, &[smi_from_i64(1), smi_from_i64(2)])
                .unwrap();
        match dispatch {
            CallDispatch::Invoke {
                code,
                closure: dispatched,
                args,
                return_policy,
            } => {
                assert_eq!(code, 777);
                assert_eq!(dispatched, closure);
                assert_eq!(args, vec![smi_from_i64(1), smi_from_i64(2)]);
                assert_eq!(return_policy, ReturnPolicy::Direct);
            }
            other => panic!("expected Invoke, got {:?}", other),
        }
        assert_eq!(
            kind_of(dispatch_call(&mut store, &mut sink, closure, &[smi_from_i64(1)])),
            RuntimeErrorKind::ArityError
        );

        // Builtins resolve immediately.
        let puts = builtin_value(BuiltinId::Puts);
        match dispatch_call(&mut store, &mut sink, puts, &[smi_from_i64(3)]).unwrap() {
            CallDispatch::Return(value) => assert_eq!(value, NULL_VALUE),
            other => panic!("expected Return, got {:?}", other),
        }
        assert_eq!(String::from_utf8(sink.bytes.clone()).unwrap(), "3\n");

        // Free variables only via get_free.
        assert_eq!(get_free(&store, closure, 0).unwrap(), NULL_VALUE);
        assert_eq!(kind_of(get_free(&store, closure, 1)), RuntimeErrorKind::InternalError);

        // Classes are not callable; other values neither.
        let class = class_new(&mut store, "C");
        assert_eq!(
            kind_of(dispatch_call(&mut store, &mut sink, class, &[])),
            RuntimeErrorKind::NotCallable
        );
        assert_eq!(
            kind_of(dispatch_call(&mut store, &mut sink, smi_from_i64(1), &[])),
            RuntimeErrorKind::NotCallable
        );

        // Bound method: receiver injected as first argument.
        let method = closure_new(&mut store, 900, 2, &[]).unwrap(); // this + 1
        class_add_method(&mut store, class, "m", method, false).unwrap();
        let instance = match dispatch_construct(&mut store, class, &[]).unwrap() {
            CallDispatch::Return(instance) => instance,
            other => panic!("expected Return, got {:?}", other),
        };
        let bound = get_property(&mut store, instance, "m").unwrap();
        match dispatch_call(&mut store, &mut sink, bound, &[smi_from_i64(9)]).unwrap() {
            CallDispatch::Invoke {
                code,
                args,
                ..
            } => {
                assert_eq!(code, 900);
                assert_eq!(args, vec![instance, smi_from_i64(9)]);
            }
            other => panic!("expected Invoke, got {:?}", other),
        }
        assert_eq!(
            kind_of(dispatch_call(&mut store, &mut sink, bound, &[])),
            RuntimeErrorKind::ArityError
        );
        true
    });
}

#[test]
fn construct_dispatch_matrix() {
    on_both_backends(|store| {
        let mut store = store.as_store();

        // new on non-class.
        let closure = closure_new(&mut store, 1, 0, &[]).unwrap();
        assert_eq!(
            kind_of(dispatch_construct(&mut store, closure, &[])),
            RuntimeErrorKind::NotConstructable
        );
        assert_eq!(
            kind_of(dispatch_construct(&mut store, NULL_VALUE, &[])),
            RuntimeErrorKind::NotConstructable
        );

        // Without a constructor only zero arguments are allowed.
        let plain = class_new(&mut store, "Plain");
        assert_eq!(
            kind_of(dispatch_construct(&mut store, plain, &[smi_from_i64(1)])),
            RuntimeErrorKind::ArityError
        );

        // With a constructor: Invoke with the fresh instance as `this` and
        // ConstructorInstance return policy.
        let with_ctor = class_new(&mut store, "WithCtor");
        let ctor = closure_new(&mut store, 4242, 2, &[]).unwrap(); // this + 1
        class_add_method(&mut store, with_ctor, "constructor", ctor, true).unwrap();
        assert_eq!(
            kind_of(dispatch_construct(&mut store, with_ctor, &[])),
            RuntimeErrorKind::ArityError
        );
        match dispatch_construct(&mut store, with_ctor, &[smi_from_i64(5)]).unwrap() {
            CallDispatch::Invoke {
                code,
                args,
                return_policy,
                ..
            } => {
                assert_eq!(code, 4242);
                assert_eq!(args.len(), 2);
                assert_eq!(args[1], smi_from_i64(5));
                let instance = args[0];
                assert_eq!(display(&store, instance).unwrap(), "[object WithCtor]");
                assert_eq!(return_policy, ReturnPolicy::ConstructorInstance(instance));
            }
            other => panic!("expected Invoke, got {:?}", other),
        }
        true
    });
}

#[test]
fn closure_parameter_limit_is_enforced() {
    on_both_backends(|store| {
        let mut store = store.as_store();
        assert!(closure_new(&mut store, 1, 7, &[]).is_ok());
        assert_eq!(kind_of(closure_new(&mut store, 1, 8, &[])), RuntimeErrorKind::ResourceLimit);
        true
    });
}

#[test]
fn runtime_error_kind_numbering_is_frozen() {
    // The .s ↔ runtime ABI (design §8): renumbering breaks compiled programs.
    let kinds = [
        (0, RuntimeErrorKind::InternalError),
        (1, RuntimeErrorKind::TypeError),
        (2, RuntimeErrorKind::ArityError),
        (3, RuntimeErrorKind::NotCallable),
        (4, RuntimeErrorKind::NotConstructable),
        (5, RuntimeErrorKind::MissingProperty),
        (6, RuntimeErrorKind::InvalidHashKey),
        (7, RuntimeErrorKind::DivisionByZero),
        (8, RuntimeErrorKind::IntegerOverflow),
        (9, RuntimeErrorKind::ResourceLimit),
    ];
    for (number, kind) in kinds.iter() {
        assert_eq!(*kind as u64, *number);
        assert_eq!(RuntimeErrorKind::from_u64(*number), Some(*kind));
    }
    assert_eq!(RuntimeErrorKind::from_u64(10), None);
}
