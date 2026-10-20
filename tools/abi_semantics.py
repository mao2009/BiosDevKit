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
    widths = REGISTER_WIDTHS.get(doc.get("architecture"))
    if widths is None:
        return ["unsupported architecture"]
    pairs = {"pce": "huc6280", "mcd": "m68000"}
    if pairs.get(doc.get("platform")) != doc.get("architecture"):
        errors.append("platform/architecture mismatch")
    seen = set()
    for reg in doc.get("registers", []):
        if not isinstance(reg, dict):
            errors.append("register must be object")
            continue
        logical = reg.get("logical")
        if logical in seen:
            errors.append(f"duplicate logical register: {logical}")
        seen.add(logical)
        physical = reg.get("physical")
        expected = widths.get(physical)
        if expected is None:
            errors.append(f"unknown CPU register: {physical}")
        elif reg.get("width_bits") != expected:
            errors.append(f"invalid width for {physical}: expected {expected}")
    flags = doc.get("flags", {})
    if isinstance(flags, dict):
        both = set(flags.get("preserved", [])) & set(flags.get("clobbered", []))
        if both:
            errors.append(f"flags both preserved and clobbered: {sorted(both)}")
    return errors
