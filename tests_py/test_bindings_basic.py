import json
import os
import tempfile

import headson


def test_summarize_bytes_json_valid():
    data = b'{"a": 1, "b": {"c": 2}}'
    out = headson.summarize_bytes(data, template="json")
    # Should parse as JSON
    obj = json.loads(out)
    assert isinstance(obj, dict)
    assert obj["a"] == 1
    assert obj["b"]["c"] == 2


def test_summarize_texts_multiple_inputs_json_keys():
    items = [
        {"path": "a.json", "content": "{\"a\": 1}"},
        {"path": "b.json", "content": "{\"b\": [1,2,3]}"},
    ]
    out = headson.summarize_texts(items, template="json")
    obj = json.loads(out)
    # Multi-file JSON output should be an object keyed by paths
    assert set(obj.keys()) == {"a.json", "b.json"}


def test_summarize_files_reads_from_disk(tmp_path):
    a = tmp_path / "a.json"
    b = tmp_path / "b.json"
    a.write_text("{\"x\": 1}")
    b.write_text("{\"y\": 2}")
    out = headson.summarize_files([str(a), str(b)], template="json")
    obj = json.loads(out)
    assert set(obj.keys()) == {str(a), str(b)}
    assert obj[str(a)]["x"] == 1
    assert obj[str(b)]["y"] == 2

