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
}

#[derive(Debug, Clone)]
pub struct JsonTreeNode {
    pub kind: NodeKind,
    // For atomic leaves (null/bool/number), the exact token text.
    pub atomic_token: Option<String>,
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
            atomic_token: None,
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
