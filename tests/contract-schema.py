#!/usr/bin/env python3
"""Validate a synthetic fixture against the BiosDevKit JSON Schema."""
import json
from pathlib import Path
from jsonschema import Draft202012Validator

ROOT = Path(__file__).resolve().parent.parent
schema = json.loads((ROOT / "schemas/api-contract-v1.schema.json").read_text(encoding="utf-8"))
Draft202012Validator.check_schema(schema)
validator = Draft202012Validator(schema)
fixture = (ROOT / "examples/api-contract.synthetic.json").read_text(encoding="utf-8")
records = [json.loads(line) for line in fixture.splitlines() if line.strip()] if "schema" == "jsonl" else [json.loads(fixture)]
assert records, "fixture must not be empty"
for record in records:
    validator.validate(record)
    wrong = dict(record)
    wrong["schema_version"] = -1
    assert not validator.is_valid(wrong), "invalid schema version was accepted"
print("validated", len(records), "synthetic fixture record(s)")
