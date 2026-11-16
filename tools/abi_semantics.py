"""Conservative synthetic ABI consistency checks; not evidence of runtime ABI behavior."""

REGISTER_WIDTHS = {
    "huc6280": {"A": 8, "X": 8, "Y": 8, "P": 8, "S": 8, "PC": 16},
    "m68000": {**{f"D{i}": 32 for i in range(8)},
               **{f"A{i}": 32 for i in range(8)}, "PC": 32, "SR": 16},
}

def validate_semantics(doc):
    """Return structural-independent CPU register/flag consistency problems."""
    errors = []
    if not isinstance(doc, dict):
        return ["adapter must be object"]
    architecture = doc.get("architecture")
    platform = doc.get("platform")
    if not isinstance(architecture, str):
        return ["architecture must be string"]
    if not isinstance(platform, str):
        return ["platform must be string"]
    widths = REGISTER_WIDTHS.get(architecture)
    if widths is None:
        return ["unsupported architecture"]
    pairs = {"pce": "huc6280", "mcd": "m68000"}
    if pairs.get(doc.get("platform")) != doc.get("architecture"):
        errors.append("platform/architecture mismatch")
    seen = set()
    registers = doc.get("registers", [])
    if not isinstance(registers, list):
        return errors + ["registers must be array"]
    for reg in registers:
        if not isinstance(reg, dict):
            errors.append("register must be object")
            continue
        logical = reg.get("logical")
        if not isinstance(logical, str):
            errors.append("logical register must be string")
            continue
        if logical in seen:
            errors.append(f"duplicate logical register: {logical}")
        seen.add(logical)
        physical = reg.get("physical")
        if not isinstance(physical, str):
            errors.append("physical register must be string")
            continue
        expected = widths.get(physical)
        if expected is None:
            errors.append(f"unknown CPU register: {physical}")
        elif reg.get("width_bits") != expected:
            errors.append(f"invalid width for {physical}: expected {expected}")
    flags = doc.get("flags", {})
    if not isinstance(flags, dict):
        return errors + ["flags must be object"]
    sets = {}
    for key in ("preserved", "clobbered"):
        values = flags.get(key, [])
        if not isinstance(values, list) or any(not isinstance(v, str) for v in values):
            errors.append(f"flags.{key} must be array of strings")
            sets[key] = set()
        else:
            sets[key] = set(values)
    both = sets["preserved"] & sets["clobbered"]
    if both:
        errors.append(f"flags both preserved and clobbered: {sorted(both)}")
    return errors
