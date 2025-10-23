use crate::order::NodeKind;

#[derive(Debug, Default, Clone)]
pub struct JsonTreeArena {
    pub nodes: Vec<JsonTreeNode>,
    pub children: Vec<usize>,
    pub obj_keys: Vec<String>,
    pub root_id: usize,
    // Marks that the root is a synthetic wrapper object representing a fileset
    // (multi-input ingest). Rendering remains standard JSON; this is an
    // internal marker for future behaviors.
    pub is_fileset: bool,
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
        }
    }
}
