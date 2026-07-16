#[cfg(test)]
mod tests {
    use parser::parse;

    use crate::compiler::Compiler;
    use crate::snapshot::{
        bytecode_abi_fingerprint, write_bytecode, SnapshotError, FORMAT_VERSION,
    };
    use crate::snapshot_layout::{
        describe_bytecode, SnapshotLayout, SnapshotRegion, SnapshotSection,
    };

    /// One function, one integer, and one string constant, so every tag shows up.
    const SOURCE: &str = r#"let add = fn(a, b) { a + b }; add(1, "hi")"#;

    fn snapshot_of(source: &str, strip_debug: bool) -> Vec<u8> {
        let program = parse(source).expect("source parses");
        let mut compiler = Compiler::new();
        let bytecode = compiler.compile(&program).expect("source compiles");
        write_bytecode(&bytecode, strip_debug).expect("bytecode serializes")
    }

    fn region<'a>(layout: &'a SnapshotLayout, label: &str) -> &'a SnapshotRegion {
        layout
            .regions
            .iter()
            .find(|region| region.label == label)
            .unwrap_or_else(|| panic!("missing region {:?}", label))
    }

    #[test]
    fn regions_tile_the_entire_buffer() {
        for strip_debug in [false, true] {
            let bytes = snapshot_of(SOURCE, strip_debug);
            let layout = describe_bytecode(&bytes).expect("valid snapshot describes");
            assert_eq!(layout.byte_length, bytes.len());

            let mut cursor = 0;
            for region in &layout.regions {
                assert_eq!(region.offset, cursor, "gap or overlap at {:?}", region.label);
                assert!(region.length >= 1, "empty region {:?}", region.label);
                cursor = region.offset + region.length;
            }
            assert_eq!(cursor, layout.byte_length);
        }
    }

    #[test]
    fn header_regions_expose_format_facts() {
        let layout = describe_bytecode(&snapshot_of(SOURCE, false)).unwrap();
        assert_eq!(layout.format_version, FORMAT_VERSION);
        assert_eq!(layout.abi_fingerprint, format!("0x{:08x}", bytecode_abi_fingerprint()));
        assert!(layout.has_debug_info);

        let labels: Vec<&str> = layout.regions[..4]
            .iter()
            .map(|region| region.label.as_str())
            .collect();
        assert_eq!(labels, ["magic", "version", "abi fingerprint", "flags"]);
        assert!(layout.regions[..4]
            .iter()
            .all(|region| region.section == SnapshotSection::Header));
        assert!(region(&layout, "abi fingerprint")
            .detail
            .contains(&layout.abi_fingerprint));
        assert!(region(&layout, "flags")
            .detail
            .contains("debug info present"));

        let stripped = describe_bytecode(&snapshot_of(SOURCE, true)).unwrap();
        assert!(!stripped.has_debug_info);
        assert!(region(&stripped, "flags")
            .detail
            .contains("debug info stripped"));
    }

    #[test]
    fn instruction_regions_disassemble_both_streams() {
        let layout = describe_bytecode(&snapshot_of(SOURCE, false)).unwrap();

        let call = region(&layout, "OpCall 2");
        assert_eq!(call.section, SnapshotSection::Main);
        assert!(call.detail.starts_with("main pc "));

        let add = region(&layout, "OpAdd");
        assert_eq!(add.section, SnapshotSection::Constants);
        assert!(add.detail.starts_with("fn add pc "));
    }

    #[test]
    fn constant_regions_annotate_tags_and_values() {
        let layout = describe_bytecode(&snapshot_of(SOURCE, false)).unwrap();

        assert_eq!(region(&layout, "constant count").detail, "3 constants (ULEB128)");
        assert!(region(&layout, "const[0] tag")
            .detail
            .contains("TAG_FUNCTION"));
        assert_eq!(region(&layout, "const[0] name").detail, "\"add\"");
        assert_eq!(region(&layout, "const[0] locals").detail, "2 local slots");
        assert_eq!(region(&layout, "const[0] params").detail, "2 parameters");
        assert!(region(&layout, "const[1] tag")
            .detail
            .contains("TAG_INTEGER"));
        assert_eq!(region(&layout, "const[1] value").detail, "1 (SLEB128)");
        assert!(region(&layout, "const[2] tag")
            .detail
            .contains("TAG_STRING"));
        assert_eq!(region(&layout, "const[2] text").detail, "\"hi\"");
    }

    #[test]
    fn debug_regions_map_pc_to_source_and_stripping_removes_them() {
        let layout = describe_bytecode(&snapshot_of(SOURCE, false)).unwrap();
        let main_spans = region(&layout, "main span count");
        assert_eq!(main_spans.section, SnapshotSection::Debug);
        assert!(layout.regions.iter().any(|region| {
            region.section == SnapshotSection::Debug
                && region.label.starts_with("main pc ")
                && region.detail.starts_with("source ")
        }));
        assert!(region(&layout, "debug fn index").detail.contains("fn add"));

        let stripped = describe_bytecode(&snapshot_of(SOURCE, true)).unwrap();
        assert!(stripped
            .regions
            .iter()
            .all(|region| region.section != SnapshotSection::Debug));
    }

    #[test]
    fn describe_rejects_what_the_loader_rejects() {
        let mut bytes = snapshot_of(SOURCE, false);
        bytes[0] = b'X';
        assert_eq!(describe_bytecode(&bytes), Err(SnapshotError::BadMagic));
    }
}
