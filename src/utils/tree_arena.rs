use crate::order::NodeKind;

#[derive(Debug, Default, Clone)]
pub struct JsonTreeArena {
    pub nodes: Vec<JsonTreeNode>,
    pub children: Vec<usize>,
    pub obj_keys: Vec<String>,
    // For arrays: original indices of kept children, stored contiguously per
    // array node; objects do not use this.
    pub arr_indices: Vec<usize>,
    pub root_id: usize,
    // True when root is a synthetic wrapper object for multi-input ingest.
    // Rendering remains standard JSON; used to select fileset-specific headers.
    pub is_fileset: bool,
    // Grep weak: for each arena node, whether this node or any of its
    // descendants contains a regex match (in strings or object keys).
    pub grep_subtree_match: Vec<bool>,
}

#[derive(Debug, Clone)]
pub struct JsonTreeNode {
    pub kind: NodeKind,
    pub number_value: Option<serde_json::Number>,
    pub bool_value: Option<bool>,
    pub string_value: Option<String>,
    pub children_start: usize,
    pub children_len: usize,
    pub obj_keys_start: usize,
    pub obj_keys_len: usize,
    pub array_len: Option<usize>,
    pub object_len: Option<usize>,
    // For arrays: slice into arena.arr_indices capturing original indices of
    // the kept children for this array node.
    pub arr_indices_start: usize,
    pub arr_indices_len: usize,
}

impl Default for JsonTreeNode {
    fn default() -> Self {
        Self {
            kind: NodeKind::Null,
            number_value: None,
            bool_value: None,
            string_value: None,
            children_start: 0,
            children_len: 0,
            obj_keys_start: 0,
            obj_keys_len: 0,
            array_len: None,
            object_len: None,
            arr_indices_start: 0,
            arr_indices_len: 0,
        }
    }
}
