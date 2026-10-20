#!/usr/bin/env python3
"""Validate a synthetic fixture against the BiosDevKit JSON Schema."""
import json
from pathlib import Path
from jsonschema import Draft202012Validator

ROOT = Path(__file__).resolve().parent.parent
schema = json.loads((ROOT / "schemas/artifact-manifest-v1.schema.json").read_text(encoding="utf-8"))
Draft202012Validator.check_schema(schema)
validator = Draft202012Validator(schema)
fixture = (ROOT / "examples/artifact-manifest.synthetic.json").read_text(encoding="utf-8")
records = [json.loads(line) for line in fixture.splitlines() if line.strip()] if "schema" == "jsonl" else [json.loads(fixture)]
assert records, "fixture must not be empty"
for record in records:
    validator.validate(record)
    wrong = dict(record)
    wrong["schema_version"] = -1
    assert not validator.is_valid(wrong), "invalid schema version was accepted"
print("validated", len(records), "synthetic fixture record(s)")

import hashlib
import io

def verify_artifacts(document, byte_streams):
    validator.validate(document)
    seen = set()
    for item in document["artifacts"]:
        name = item["logical_name"]
        if name in seen:
            raise ValueError("duplicate logical_name")
        seen.add(name)
        if name not in byte_streams:
            raise ValueError("missing byte stream")
        digest = hashlib.sha256()
        size = 0
        while True:
            chunk = byte_streams[name].read(65536)
            if not chunk:
                break
            digest.update(chunk)
            size += len(chunk)
        if digest.hexdigest() != item["sha256"] or size != item["byte_length"]:
            raise ValueError("hash or size mismatch")

verify_artifacts(records[0], {"empty.synthetic.bin": io.BytesIO(b"")})
for bad_bytes in (b"corrupt", b"more bytes"):
    try:
        verify_artifacts(records[0], {"empty.synthetic.bin": io.BytesIO(bad_bytes)})
    except ValueError:
        pass
    else:
        raise AssertionError("mismatched data accepted")
duplicate = json.loads(json.dumps(records[0]))
duplicate["artifacts"].append(dict(duplicate["artifacts"][0]))
try:
    verify_artifacts(duplicate, {"empty.synthetic.bin": io.BytesIO(b"")})
except ValueError:
    pass
else:
    raise AssertionError("duplicate logical_name accepted")
