#!/usr/bin/env python3
"""Validate declared provenance schema, synthetic fixture, and fail-closed negatives."""
import copy
import json
from pathlib import Path
from jsonschema import Draft202012Validator, FormatChecker

ROOT = Path(__file__).resolve().parents[1]
schema = json.loads((ROOT / "schemas/test-evidence-v2.schema.json").read_text())
Draft202012Validator.check_schema(schema)
validator = Draft202012Validator(schema, format_checker=FormatChecker())
valid = json.loads((ROOT / "examples/test-evidence-v2.synthetic.json").read_text())
validator.validate(valid)
assert "date-time" in FormatChecker().checkers, "RFC3339 format validation unavailable"

for pointer, replacement in [
    ("/schema_version", 1),
    ("/run_provenance/source_commit", "synthetic"),
    ("/run_provenance/runner_version", ""),
    ("/run_provenance/environment", []),
    ("/run_provenance/traces", ["duplicate", "duplicate"]),
    ("/run_provenance/started_at", "not-a-date"),
    ("/run_provenance/artifacts", ["duplicated", "duplicated"]),
    ("/result", "SUCCESS"),
    ("/reason", "  "),
]:
    doc = copy.deepcopy(valid)
    parent = doc
    parts = pointer.strip("/").split("/")
    for part in parts[:-1]:
        parent = parent[part]
    parent[parts[-1]] = replacement
    assert not validator.is_valid(doc), pointer
for missing in ["runner_version", "source_commit", "environment", "started_at", "finished_at", "artifacts", "traces"]:
    doc = copy.deepcopy(valid)
    del doc["run_provenance"][missing]
    assert not validator.is_valid(doc), missing
for value in ([], None, "text", 2):
    assert not validator.is_valid(value), "non-object unexpectedly valid"
for key in ("test_id", "result", "observations", "run_provenance"):
    doc = copy.deepcopy(valid)
    del doc[key]
    assert not validator.is_valid(doc), key
doc = copy.deepcopy(valid)
doc["unexpected"] = "field"
assert not validator.is_valid(doc)
doc = copy.deepcopy(valid)
doc["run_provenance"]["unknown"] = True
assert not validator.is_valid(doc)
old = json.loads((ROOT / "examples/test-evidence.synthetic.json").read_text())
assert old["schema_version"] == 1
assert not validator.is_valid(old), "v1 input must not silently be interpreted as v2"
print("v2 provenance schema and synthetic negative tests PASS")
