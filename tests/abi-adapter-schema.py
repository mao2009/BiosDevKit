#!/usr/bin/env python3
"""Synthetic platform ABI adapter shape tests; no hardware compatibility inference."""
import copy
import json
from pathlib import Path
from jsonschema import Draft202012Validator
ROOT = Path(__file__).resolve().parents[1]
schema = json.loads((ROOT / "schemas/abi-adapter-v1.schema.json").read_text())
Draft202012Validator.check_schema(schema)
v = Draft202012Validator(schema)
for platform in ("pce", "mcd"):
    original = json.loads((ROOT / f"examples/abi-adapter-{platform}.synthetic.json").read_text())
    v.validate(original)
    cases = [
        ("empty registers", lambda d: d.update(registers=[])),
        ("missing stack", lambda d: d.pop("stack")),
        ("missing return mechanism", lambda d: d["return_state"].pop("mechanism")),
        ("unknown flag", lambda d: d["flags"].update(unknown=1)),
        ("bad register width", lambda d: d["registers"][0].update(width_bits=0)),
        ("bad direction", lambda d: d["registers"][0].update(direction="unknown")),
        ("unknown register", lambda d: d["registers"][0].update(unknown=1)),
        ("platform mismatch", lambda d: d.update(architecture="m68000" if platform=="pce" else "huc6280")),
    ]
    for name, fn in cases:
        doc = copy.deepcopy(original)
        fn(doc)
        assert not v.is_valid(doc), f"{platform}: accepted {name}"
print("synthetic adapter schema negative tests PASS")
