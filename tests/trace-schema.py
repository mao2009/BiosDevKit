#!/usr/bin/env python3
"""Validate a synthetic fixture against the BiosDevKit JSON Schema."""
import json
from pathlib import Path
from jsonschema import Draft202012Validator

ROOT = Path(__file__).resolve().parent.parent
schema = json.loads((ROOT / "schemas/trace-event-v1.schema.json").read_text(encoding="utf-8"))
Draft202012Validator.check_schema(schema)
validator = Draft202012Validator(schema)
fixture = (ROOT / "examples/trace.synthetic.jsonl").read_text(encoding="utf-8")
records = [json.loads(line) for line in fixture.splitlines() if line.strip()] if "jsonl" == "jsonl" else [json.loads(fixture)]
assert records, "fixture must not be empty"
for record in records:
    validator.validate(record)
    wrong = dict(record)
    wrong["schema_version"] = -1
    assert not validator.is_valid(wrong), "invalid schema version was accepted"
print("validated", len(records), "synthetic fixture record(s)")

# JSON Schema validates an event; stream invariants are checked separately.
def validate_stream(events):
    if not events:
        raise ValueError("empty trace stream")
    last = {}
    for event in events:
        validator.validate(event)
        key = (event["run_id"], event["stream_id"])
        seq = event["sequence"]
        if key in last and seq <= last[key]:
            raise ValueError("non-increasing sequence")
        last[key] = seq

validate_stream(records)
for bad in ([records[0], records[0]], list(reversed(records))):
    try:
        validate_stream(bad)
    except ValueError:
        pass
    else:
        raise AssertionError("reordered/duplicate events accepted")

def validate_stream(events):
    last = {}
    for event in events:
        validator.validate(event)
        key = (event["run_id"], event["stream_id"])
        seq = event["sequence"]
        if key in last and seq <= last[key]:
            raise ValueError("non-increasing sequence")
        last[key] = seq

validate_stream(records)
for bad in ([records[0], records[0]], list(reversed(records))):
    try:
        validate_stream(bad)
    except ValueError:
        pass
    else:
        raise AssertionError("invalid event order accepted")
