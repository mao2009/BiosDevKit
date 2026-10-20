#!/usr/bin/env python3
"""Synthetic platform ABI adapter shape tests; no hardware compatibility inference."""
import copy
import json
from pathlib import Path
import sys
from jsonschema import Draft202012Validator
ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / 'tools'))
from abi_semantics import validate_semantics
schema = json.loads((ROOT / "schemas/abi-adapter-v1.schema.json").read_text())
Draft202012Validator.check_schema(schema)
v = Draft202012Validator(schema)
for platform in ("pce", "mcd"):
    original = json.loads((ROOT / f"examples/abi-adapter-{platform}.synthetic.json").read_text())
    v.validate(original)
    assert not validate_semantics(original), platform
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
    semantic_cases = [
        ("foreign CPU register", lambda d: d["registers"][0].update(physical="D0" if platform == "pce" else "X")),
        ("wrong CPU register width", lambda d: d["registers"][0].update(width_bits=32 if platform == "pce" else 8)),
        ("duplicate logical register", lambda d: d["registers"].append({**d["registers"][0], "physical": "X" if platform == "pce" else "D1"})),
        ("contradictory flags", lambda d: d["flags"]["clobbered"].append("I")),
    ]
    for name, fn in semantic_cases:
        doc = copy.deepcopy(original)
        fn(doc)
        assert v.is_valid(doc), f"{platform}: structural schema unexpectedly rejected {name}"
        assert validate_semantics(doc), f"{platform}: semantics accepted {name}"
# The semantic entry point must return diagnostics, never TypeError, even if
# a future caller bypasses JSON Schema validation.
for malformed in (
    None, [], "not-an-adapter",
    {"architecture": "huc6280", "platform": "pce", "registers": None},
    {"architecture": "huc6280", "platform": "pce", "registers": [{"logical": [], "physical": "A", "width_bits": 8}]},
    {"architecture": "huc6280", "platform": "pce", "registers": [{"logical": "a", "physical": [], "width_bits": 8}]},
    {"architecture": "huc6280", "platform": "pce", "registers": [], "flags": {"preserved": [[]], "clobbered": []}},
    {"architecture": "m68000", "platform": "mcd", "registers": [], "flags": {"preserved": "I", "clobbered": []}},
):
    assert validate_semantics(malformed), f"accepted malformed semantic input: {malformed!r}"

for field in ("architecture", "platform"):
    for value in (None, [], {}, 23):
        malformed = {"architecture": "huc6280", "platform": "pce", "registers": [], "flags": {}}
        malformed[field] = value
        result = validate_semantics(malformed)
        assert result and isinstance(result, list), (field, value, result)
print("synthetic adapter schema and semantic negatives PASS")
