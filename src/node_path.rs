use std::path::{Component, Path, PathBuf};

use crate::RankedNode;
use crate::order::{NodeId, ObjectType, PriorityOrder};

/// Composite breadcrumb key: `(file, "dot_path#hex_hash")`.
/// `file` is the input file's resolved absolute path (see
/// [`resolve_breadcrumb_file`]), or `""` when no file identity is known
/// (stdin, or library callers without an input path). The dot-path is the
/// structural address *inside* the file — never the filename — so the same
/// file produces the same key whether rendered as a single input, inside a
/// fileset, or from a different working directory.
pub type BreadcrumbKey = (String, String);

/// Resolve an input path to the canonical absolute form used as the
/// breadcrumb `file` component, so the same file yields the same key
/// regardless of cwd or path spelling (`a.json`, `./a.json`, `../x/a.json`).
///
/// Canonicalizes when possible (resolving symlinks); otherwise falls back to
/// joining onto the current directory and lexically dropping `.`/`..`
/// components. Returns `""` (unknown identity) for an empty name or when a
/// relative name cannot be anchored because the current directory is
/// unavailable — never a relative path, which could not match a
/// canonically-recorded key.
pub fn resolve_breadcrumb_file(name: &str) -> String {
    if name.is_empty() {
        return String::new();
    }
    let path = Path::new(name);
    if let Ok(canonical) = path.canonicalize() {
        return canonical.to_string_lossy().into_owned();
    }
    let joined = if path.is_absolute() {
        path.to_path_buf()
    } else {
        let Ok(cwd) = std::env::current_dir() else {
            return String::new();
        };
        cwd.join(path)
    };
    lexically_normalized(&joined).to_string_lossy().into_owned()
}

fn lexically_normalized(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    let mut has_root = false;
    for comp in path.components() {
        match comp {
            Component::Prefix(prefix) => out.push(prefix.as_os_str()),
            Component::RootDir => {
                out.push(comp.as_os_str());
                has_root = true;
            }
            Component::CurDir => {}
            Component::ParentDir => {
                if !out.pop() && !has_root {
                    out.push(comp.as_os_str());
                }
            }
            Component::Normal(part) => out.push(part),
        }
    }
    out
}

/// Maps PQ nodes to the resolved absolute path of the input file they came
/// from, for use as the breadcrumb `file` component.
///
/// Fileset slots resolve their as-typed input names (the fileset root's
/// child keys) via [`resolve_breadcrumb_file`]; nodes outside any fileset
/// slot fall back to the single-input path (threaded from the CLI through
/// `ExploreContext::file`), or `""` when unknown.
pub struct NodeFiles {
    /// Per-PQ-node index into `paths`; `None` falls back to `single`.
    slot_of: Vec<Option<usize>>,
    /// Resolved absolute path per fileset render slot.
    paths: Vec<String>,
    /// Resolved absolute path of a single (non-fileset) input.
    single: Option<String>,
}

impl NodeFiles {
    pub fn for_order(order: &PriorityOrder, single: Option<&str>) -> Self {
        let render_slots = order.fileset_render_slots().unwrap_or(&[]);
        let mut slot_of: Vec<Option<usize>> = vec![None; order.nodes.len()];
        let mut paths: Vec<String> = Vec::with_capacity(render_slots.len());
        for slot in render_slots {
            let name = order
                .nodes
                .get(slot.id.0)
                .and_then(RankedNode::key_in_object)
                .unwrap_or("");
            let path_idx = paths.len();
            paths.push(resolve_breadcrumb_file(name));
            mark_subtree(order, slot.id, path_idx, &mut slot_of);
        }
        Self {
            slot_of,
            paths,
            single: single.map(str::to_string),
        }
    }

    fn file_for(&self, node_id: NodeId) -> &str {
        match self.slot_of.get(node_id.0).copied().flatten() {
            Some(idx) => self.paths.get(idx).map_or("", String::as_str),
            None => self.single.as_deref().unwrap_or(""),
        }
    }
}

/// Assign `path_idx` to every node in the subtree rooted at `root` that has
/// not been claimed by an earlier fileset slot.
fn mark_subtree(
    order: &PriorityOrder,
    root: NodeId,
    path_idx: usize,
    slot_of: &mut [Option<usize>],
) {
    let mut stack = vec![root];
    while let Some(id) = stack.pop() {
        let Some(entry @ None) = slot_of.get_mut(id.0) else {
            continue;
        };
        *entry = Some(path_idx);
        if let Some(children) = order.children.get(id.0) {
            stack.extend(children.iter().copied());
        }
    }
}

const FNV1A_INIT: u64 = 14_695_981_039_346_656_037;
const FNV1A_PRIME: u64 = 1_099_511_628_211;

// LeafPart is a synthetic rendering node excluded from content identity.
const EXCLUDED_NODE_HASH: u64 = 0;

fn fnv1a_update(mut h: u64, data: &[u8]) -> u64 {
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(FNV1A_PRIME);
    }
    h
}

fn fnv1a(data: &[u8]) -> u64 {
    fnv1a_update(FNV1A_INIT, data)
}

fn build_json_path(order: &PriorityOrder, node_id: NodeId) -> String {
    let mut parts: Vec<String> = Vec::new();
    let mut cursor = node_id;
    while let Some(parent) = order.parent.get(cursor.0).and_then(|p| *p) {
        // A fileset root's child keys are filenames, not structural address:
        // file identity lives in the breadcrumb's `file` component instead,
        // so the same file keys identically across invocation styles.
        let parent_is_fileset_root = matches!(
            order.object_type.get(parent.0),
            Some(ObjectType::Fileset)
        );
        if !parent_is_fileset_root
            && let Some(part) = path_component(order, cursor)
        {
            parts.push(part);
        }
        cursor = parent;
    }
    parts.reverse();
    parts.join(".")
}

/// The dot-path segment contributed by `cursor`: its object key, or its
/// original index within a parent array.
fn path_component(order: &PriorityOrder, cursor: NodeId) -> Option<String> {
    if let Some(key) = order
        .nodes
        .get(cursor.0)
        .and_then(RankedNode::key_in_object)
    {
        return Some(key.to_string());
    }
    order
        .index_in_parent_array
        .get(cursor.0)
        .and_then(|x| *x)
        .map(|idx| idx.to_string())
}

fn node_hash(order: &PriorityOrder, id: usize, hashes: &[u64]) -> u64 {
    match order.nodes.get(id) {
        Some(RankedNode::AtomicLeaf { token, .. }) => fnv1a(token.as_bytes()),
        Some(RankedNode::SplittableLeaf { value, .. }) => {
            fnv1a(value.as_bytes())
        }
        None | Some(RankedNode::LeafPart { .. }) => EXCLUDED_NODE_HASH,
        Some(RankedNode::Object { .. } | RankedNode::Array { .. }) => {
            let mut h = FNV1A_INIT;
            if let Some(children) = order.children.get(id) {
                for &child_id in children {
                    if let Some(key) = order
                        .nodes
                        .get(child_id.0)
                        .and_then(|n| n.key_in_object())
                    {
                        h = fnv1a_update(h, key.as_bytes());
                    }
                    h = fnv1a_update(
                        h,
                        &hashes
                            .get(child_id.0)
                            .copied()
                            .unwrap_or(EXCLUDED_NODE_HASH)
                            .to_le_bytes(),
                    );
                }
            }
            h
        }
    }
}

/// Compute a stable FNV-1a Merkle hash for every node in `order`.
///
/// Hashes are built bottom-up:
/// - `AtomicLeaf` / `SplittableLeaf`: hash of token/value bytes.
/// - `Object`: hash of (key_bytes ++ child_hash) for each child in order.
/// - `Array`: hash of child hashes in order.
/// - `LeafPart`: 0 (synthetic split-rendering node; excluded from breadcrumbs).
///
/// The returned `Vec<u64>` is indexed by PQ node id (`NodeId.0`).
/// Output is deterministic across process restarts.
#[allow(
    clippy::cognitive_complexity,
    reason = "post-order tree traversal inherently requires nested control flow"
)]
pub fn compute_merkle_hashes(order: &PriorityOrder) -> Vec<u64> {
    let n = order.nodes.len();
    let mut hashes = vec![0u64; n];
    if n == 0 {
        return hashes;
    }
    // Root is always PQ id 0 by construction in build_order.
    let mut stack: Vec<(usize, bool)> = Vec::with_capacity(n);
    stack.push((0, false));
    while let Some((id, processed)) = stack.pop() {
        if processed {
            hashes[id] = node_hash(order, id, &hashes);
        } else {
            stack.push((id, true));
            if let Some(children) = order.children.get(id) {
                for &child in children.iter().rev() {
                    stack.push((child.0, false));
                }
            }
        }
    }
    hashes
}

/// Returns `(file, path)` for a leaf node, or `None` for structural nodes.
///
/// `file` is the resolved absolute path of the input file the node belongs
/// to (per-slot for filesets, the single-input path otherwise), or `""` when
/// no file identity is known (stdin, library callers without a path).
///
/// `path` is `"dot.path#<16 hex digits>"`: the structural address *inside*
/// the file combined with the FNV-1a Merkle hash of the subtree at that node.
/// The composite key is stable across restarts and invocation styles. A
/// content change produces a new hash (no match); reverting the change
/// restores the original hash (penalty re-activates).
pub fn leaf_breadcrumb_key(
    order: &PriorityOrder,
    node_id: NodeId,
    hashes: &[u64],
    files: &NodeFiles,
) -> Option<BreadcrumbKey> {
    match order.nodes.get(node_id.0)? {
        RankedNode::Array { .. }
        | RankedNode::Object { .. }
        | RankedNode::LeafPart { .. } => None,
        RankedNode::AtomicLeaf { .. } | RankedNode::SplittableLeaf { .. } => {
            let dot_path = build_json_path(order, node_id);
            let hash =
                hashes.get(node_id.0).copied().unwrap_or(EXCLUDED_NODE_HASH);
            Some((
                files.file_for(node_id).to_string(),
                format!("{dot_path}#{hash:016x}"),
            ))
        }
    }
}

/// The node that carries breadcrumb identity for a selected node: a
/// `LeafPart` resolves to its parent `SplittableLeaf`, everything else to
/// itself.
fn breadcrumb_carrier(order: &PriorityOrder, node_id: NodeId) -> NodeId {
    if matches!(
        order.nodes.get(node_id.0),
        Some(RankedNode::LeafPart { .. })
    ) && let Some(parent) = order.parent.get(node_id.0).copied().flatten()
    {
        return parent;
    }
    node_id
}

/// Collect breadcrumb keys for the leaves actually selected by the budget
/// search: the top-k prefix of the ordering the search indexed into (the
/// per-slot `selection_order` when present, `by_priority` otherwise), plus
/// any strong-grep must-keep nodes forced into the render outside that
/// prefix. Selected `LeafPart` nodes count as their parent string being
/// shown.
///
/// Reuses the Merkle hash table stored by explore penalty matching when
/// available so the whole pipeline performs at most one full hash pass.
///
/// `single_file` is the resolved absolute path of a single (non-fileset)
/// input, used as the `file` component for nodes outside any fileset slot.
pub(crate) fn collect_shown_leaves(
    order: &PriorityOrder,
    search: &crate::pruner::budget::BudgetSearchResult,
    single_file: Option<&str>,
) -> Vec<BreadcrumbKey> {
    use std::borrow::Cow;
    use std::collections::HashSet;

    let hashes: Cow<'_, [u64]> = order.merkle_hashes.as_deref().map_or_else(
        || Cow::Owned(compute_merkle_hashes(order)),
        Cow::Borrowed,
    );
    let files = NodeFiles::for_order(order, single_file);
    let base: &[NodeId] = search
        .selection_order
        .as_deref()
        .unwrap_or(&order.by_priority);
    let bound = search.top_k.min(base.len());
    let mut seen: HashSet<NodeId> = HashSet::with_capacity(bound);
    let mut out = Vec::new();
    for &selected in base[..bound].iter().chain(&search.grep_must_keep) {
        // A selected LeafPart renders its parent string (the SplittableLeaf
        // is reinserted as an ancestor), so the parent carries the breadcrumb
        // identity. Fileset selection orders can include parts without their
        // parent in the top-k prefix.
        let node_id = breadcrumb_carrier(order, selected);
        if !seen.insert(node_id) {
            continue;
        }
        if let Some(key) = leaf_breadcrumb_key(order, node_id, &hashes, &files)
        {
            out.push(key);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ingest::{parse_json_one, parse_text_one_with_mode};
    use crate::order::PriorityConfig;
    use crate::order::build_order;

    fn make_order(json: &[u8]) -> PriorityOrder {
        let cfg = PriorityConfig::new(usize::MAX, usize::MAX);
        let arena =
            parse_json_one(json.to_vec(), &cfg).expect("parse must succeed");
        build_order(&arena, &cfg).expect("build_order must succeed")
    }

    fn no_files(order: &PriorityOrder) -> NodeFiles {
        NodeFiles::for_order(order, None)
    }

    // A. compute_merkle_hashes is stable across two separate build_order calls
    // for the same JSON bytes.
    #[test]
    fn merkle_hashes_stable_across_builds() {
        let json = b"{\"x\": 42, \"y\": \"hello\"}";
        let order1 = make_order(json);
        let order2 = make_order(json);
        let hashes1 = compute_merkle_hashes(&order1);
        let hashes2 = compute_merkle_hashes(&order2);
        assert_eq!(
            hashes1, hashes2,
            "Merkle hashes must be identical across two builds of the same input"
        );
    }

    // B. Root-node hash differs when a leaf value changes.
    #[test]
    fn merkle_hash_changes_when_leaf_value_changes() {
        let order_a = make_order(b"{\"a\": 1}");
        let order_b = make_order(b"{\"a\": 2}");
        let hashes_a = compute_merkle_hashes(&order_a);
        let hashes_b = compute_merkle_hashes(&order_b);
        // Root is node 0 in PriorityOrder convention.
        assert_ne!(
            hashes_a[crate::order::types::ROOT_PQ_ID],
            hashes_b[crate::order::types::ROOT_PQ_ID],
            "Root hash must differ when leaf value changes"
        );
    }

    // C. The hash of a leaf that did NOT change is unaffected by a sibling change.
    #[test]
    fn merkle_hash_sibling_unchanged_when_other_sibling_changes() {
        // {"a": 1, "b": "hello"} vs {"a": 1, "b": "world"}
        // The leaf for value 1 (key "a") must hash identically in both.
        let order_hello = make_order(b"{\"a\": 1, \"b\": \"hello\"}");
        let order_world = make_order(b"{\"a\": 1, \"b\": \"world\"}");
        let hashes_hello = compute_merkle_hashes(&order_hello);
        let hashes_world = compute_merkle_hashes(&order_world);

        // Find the node id for the leaf value of key "a" (AtomicLeaf with token "1")
        // in each order.
        let find_a_leaf_id = |order: &PriorityOrder| -> usize {
            order.nodes.iter().position(|n| {
                matches!(n, RankedNode::AtomicLeaf { token, key_in_object, .. }
                    if token == "1" && key_in_object.as_deref() == Some("a"))
            }).expect("must find leaf for key 'a' with value 1")
        };

        let id_hello = find_a_leaf_id(&order_hello);
        let id_world = find_a_leaf_id(&order_world);

        assert_eq!(
            hashes_hello[id_hello], hashes_world[id_world],
            "Hash for unchanged leaf ('a': 1) must be the same regardless of sibling change"
        );
    }

    // D. The composite key format for a leaf contains '#' separating dot_path
    // from a 16-hex-char hash string.
    #[test]
    fn composite_key_format_contains_hash_separator() {
        let json = b"{\"name\": \"alice\"}";
        let order = make_order(json);
        let hashes = compute_merkle_hashes(&order);

        // Find the SplittableLeaf for value "alice"
        let alice_id = order.nodes.iter().position(|n| {
            matches!(n, RankedNode::SplittableLeaf { value, key_in_object, .. }
                if value == "alice" && key_in_object.as_deref() == Some("name"))
        }).expect("must find 'alice' leaf");

        let result = leaf_breadcrumb_key(
            &order,
            NodeId(alice_id),
            &hashes,
            &no_files(&order),
        );
        let (_, path) =
            result.expect("leaf_breadcrumb_key must return Some for a leaf");

        let parts: Vec<&str> = path.splitn(2, '#').collect();
        assert_eq!(
            parts.len(),
            2,
            "path must contain exactly one '#' separator; got: {path:?}"
        );
        assert_eq!(
            parts[0], "name",
            "dot_path before '#' must be 'name'; got: {path:?}"
        );
        assert_eq!(
            parts[1].len(),
            16,
            "hex hash after '#' must be 16 chars; got: {path:?}"
        );
        assert!(
            parts[1].chars().all(|c| c.is_ascii_hexdigit()),
            "hash part must be hex digits; got: {path:?}"
        );
    }

    // D2. Without any file identity (stdin, bare library use), the file
    // component is "".
    #[test]
    fn file_component_empty_without_identity() {
        let order = make_order(b"{\"k\": \"v\"}");
        let hashes = compute_merkle_hashes(&order);
        let leaf_id = order
            .nodes
            .iter()
            .position(|n| matches!(n, RankedNode::SplittableLeaf { .. }))
            .expect("must find a leaf");
        let (file, _) = leaf_breadcrumb_key(
            &order,
            NodeId(leaf_id),
            &hashes,
            &no_files(&order),
        )
        .expect("leaf must produce a key");
        assert_eq!(file, "", "file must be empty without file identity");
    }

    // E. The composite key is stable across two separate builds of the same input.
    #[test]
    fn composite_key_stable_across_builds() {
        let json = b"{\"k\": \"v\"}";
        let order1 = make_order(json);
        let order2 = make_order(json);
        let hashes1 = compute_merkle_hashes(&order1);
        let hashes2 = compute_merkle_hashes(&order2);

        // Find the leaf in each order.
        let find_leaf = |order: &PriorityOrder| -> NodeId {
            NodeId(order.nodes.iter().position(|n| {
                matches!(n, RankedNode::SplittableLeaf { value, .. } if value == "v")
            }).expect("must find leaf 'v'"))
        };

        let key1 = leaf_breadcrumb_key(
            &order1,
            find_leaf(&order1),
            &hashes1,
            &no_files(&order1),
        );
        let key2 = leaf_breadcrumb_key(
            &order2,
            find_leaf(&order2),
            &hashes2,
            &no_files(&order2),
        );
        assert_eq!(
            key1, key2,
            "composite key must be identical across two builds of the same input"
        );
    }

    // F. Composite key for unchanged leaf is stable when a sibling leaf changes.
    #[test]
    fn composite_key_stable_when_sibling_changes() {
        let order_hello = make_order(b"{\"a\": 1, \"b\": \"hello\"}");
        let order_world = make_order(b"{\"a\": 1, \"b\": \"world\"}");
        let hashes_hello = compute_merkle_hashes(&order_hello);
        let hashes_world = compute_merkle_hashes(&order_world);

        let find_a = |order: &PriorityOrder| -> NodeId {
            NodeId(order.nodes.iter().position(|n| {
                matches!(n, RankedNode::AtomicLeaf { token, key_in_object, .. }
                    if token == "1" && key_in_object.as_deref() == Some("a"))
            }).expect("must find AtomicLeaf for key 'a'"))
        };

        let key_hello = leaf_breadcrumb_key(
            &order_hello,
            find_a(&order_hello),
            &hashes_hello,
            &no_files(&order_hello),
        );
        let key_world = leaf_breadcrumb_key(
            &order_world,
            find_a(&order_world),
            &hashes_world,
            &no_files(&order_world),
        );
        assert_eq!(
            key_hello, key_world,
            "composite key for unchanged leaf must be stable when a sibling changes"
        );
    }

    // G. leaf_breadcrumb_key returns None for all Array and Object structural nodes.
    #[test]
    fn structural_nodes_return_none() {
        // {"a": [1, 2]} — contains a root Object and one Array child.
        let order = make_order(b"{\"a\": [1, 2]}");
        let hashes = compute_merkle_hashes(&order);
        let files = no_files(&order);

        for (idx, node) in order.nodes.iter().enumerate() {
            if matches!(
                node,
                RankedNode::Array { .. } | RankedNode::Object { .. }
            ) {
                let result =
                    leaf_breadcrumb_key(&order, NodeId(idx), &hashes, &files);
                assert!(
                    result.is_none(),
                    "leaf_breadcrumb_key must return None for structural node at index {idx}: {node:?}"
                );
            }
        }
    }

    // H. Code mode: AtomicLeaf composite key path contains '#' and the dot_path
    // part (before '#') is NOT a standalone hex string (old behavior was to
    // return only a hash with no dot-path prefix).
    #[test]
    #[allow(
        clippy::cognitive_complexity,
        reason = "test iterates nodes and checks multiple properties"
    )]
    fn code_mode_no_longer_special_cased() {
        let code = b"fn foo() {\n    let x = 1;\n}\n";
        let cfg = PriorityConfig::new(usize::MAX, usize::MAX);
        let arena = parse_text_one_with_mode(code.to_vec(), &cfg, true)
            .expect("parse must succeed");
        let order =
            build_order(&arena, &cfg).expect("build_order must succeed");
        let hashes = compute_merkle_hashes(&order);
        let files = no_files(&order);

        let mut found_leaf = false;
        for (idx, node) in order.nodes.iter().enumerate() {
            if matches!(node, RankedNode::AtomicLeaf { .. }) {
                if let Some((_, path)) =
                    leaf_breadcrumb_key(&order, NodeId(idx), &hashes, &files)
                {
                    found_leaf = true;
                    assert!(
                        path.contains('#'),
                        "code-mode leaf path must contain '#'; got: {path:?}"
                    );
                    let dot_path = path.split('#').next().unwrap_or("");
                    // Old behavior was a bare hex hash with no dot_path prefix.
                    // The dot_path must NOT be a purely hex string (or must be non-empty
                    // with non-hex characters like digits-only index or a line label).
                    // Specifically, it must not look like a standalone 16-hex-char hash.
                    let is_standalone_hex_hash = dot_path.len() >= 8
                        && dot_path.chars().all(|c| c.is_ascii_hexdigit());
                    assert!(
                        !is_standalone_hex_hash,
                        "dot_path before '#' must NOT be a standalone hex hash; got dot_path={dot_path:?} in path={path:?}"
                    );
                }
            }
        }
        assert!(
            found_leaf,
            "must have found at least one AtomicLeaf in code input"
        );
    }

    fn make_fileset_order(
        files: Vec<(&str, &[u8])>,
    ) -> (PriorityOrder, Vec<u64>) {
        use crate::ingest::fileset::{FilesetInput, FilesetInputKind};
        let cfg = PriorityConfig::new(usize::MAX, usize::MAX);
        let inputs = files
            .into_iter()
            .map(|(name, bytes)| FilesetInput {
                name: name.to_string(),
                bytes: bytes.to_vec(),
                kind: FilesetInputKind::Json,
            })
            .collect();
        let out = crate::ingest::ingest_into_arena(
            crate::InputKind::Fileset(inputs),
            &cfg,
            &crate::GrepConfig::default(),
        )
        .expect("fileset ingest must succeed");
        let order =
            build_order(&out.arena, &cfg).expect("build_order must succeed");
        let hashes = compute_merkle_hashes(&order);
        (order, hashes)
    }

    fn keys_of(order: &PriorityOrder, hashes: &[u64]) -> Vec<BreadcrumbKey> {
        let files = no_files(order);
        (0..order.nodes.len())
            .filter_map(|idx| {
                leaf_breadcrumb_key(order, NodeId(idx), hashes, &files)
            })
            .collect()
    }

    // I. Fileset keys carry the resolved absolute file path and an inner
    // dot-path that does NOT embed the filename.
    #[test]
    fn fileset_keys_use_resolved_file_and_inner_dot_path() {
        let (order, hashes) =
            make_fileset_order(vec![("f1.json", br#"{"x": 1}"#)]);
        let keys = keys_of(&order, &hashes);
        let (file, path) = keys
            .iter()
            .find(|(_, p)| p.starts_with("x#"))
            .expect("must find leaf at inner path 'x'");
        assert_eq!(
            file,
            &resolve_breadcrumb_file("f1.json"),
            "file must be the resolved absolute input path"
        );
        assert!(
            Path::new(file).is_absolute(),
            "file must be absolute; got {file:?}"
        );
        assert!(
            !path.contains("f1.json"),
            "inner dot-path must not embed the filename; got {path:?}"
        );
    }

    // J. Two fileset files with identical content produce keys that differ
    // only in the file component — same inner path, same hash.
    #[test]
    fn identical_files_share_paths_but_not_file_identity() {
        let content: &[u8] = br#"{"version": "1.0"}"#;
        let (order, hashes) =
            make_fileset_order(vec![("a.json", content), ("b.json", content)]);
        let keys = keys_of(&order, &hashes);
        let version_keys: Vec<&BreadcrumbKey> = keys
            .iter()
            .filter(|(_, p)| p.starts_with("version#"))
            .collect();
        assert_eq!(version_keys.len(), 2, "one 'version' leaf per file");
        assert_eq!(
            version_keys[0].1, version_keys[1].1,
            "identical content must hash identically"
        );
        assert_ne!(
            version_keys[0].0, version_keys[1].0,
            "different files must have distinct file identity"
        );
    }

    // K. resolve_breadcrumb_file is spelling-independent: `./x` and `x`
    // resolve identically, `..` components are removed, and the result is
    // absolute even for paths that do not exist.
    #[test]
    fn resolve_breadcrumb_file_normalizes_spellings() {
        let plain = resolve_breadcrumb_file("no_such_dir/x.json");
        assert_eq!(resolve_breadcrumb_file("./no_such_dir/x.json"), plain);
        assert_eq!(
            resolve_breadcrumb_file("no_such_dir/sub/../x.json"),
            plain
        );
        assert!(
            Path::new(&plain).is_absolute(),
            "fallback resolution must produce an absolute path; got {plain:?}"
        );
    }

    #[test]
    fn resolve_breadcrumb_file_maps_empty_name_to_unknown_identity() {
        assert_eq!(resolve_breadcrumb_file(""), "");
    }
}
