//! Validation of the version-1 API contract document.
use std::collections::HashSet;

pub fn validate(document: &serde_json::Value) -> Result<usize, String> {
    let obj = document.as_object().ok_or("contract must be an object")?;
    if obj.get("schema_version").and_then(|v| v.as_u64()) != Some(1) {
        return Err("unsupported schema_version".into());
    }
    let platform = obj.get("platform").and_then(|v| v.as_str()).ok_or("missing platform")?;
    if platform.trim().is_empty() { return Err("empty platform".into()); }
    let apis = obj.get("apis").and_then(|v| v.as_array()).ok_or("missing apis")?;
    let mut ids = HashSet::new();
    for (index, api) in apis.iter().enumerate() {
        let api = api.as_object().ok_or_else(|| format!("apis[{index}] must be object"))?;
        for key in ["id", "name", "entry_point"] {
            if api.get(key).and_then(|v| v.as_str()).is_none_or(|s| s.is_empty()) {
                return Err(format!("apis[{index}].{key} must be nonempty string"));
            }
        }
        let id = api["id"].as_str().unwrap();
        if !ids.insert(id.to_string()) { return Err(format!("duplicate API id: {id}")); }
        for key in ["inputs", "outputs"] {
            let regs = api.get(key).and_then(|v| v.as_array()).ok_or_else(|| format!("apis[{index}].{key} must be array"))?;
            for reg in regs {
                let r = reg.as_object().ok_or("register must be object")?;
                for name in ["name", "meaning"] {
                    if r.get(name).and_then(|v| v.as_str()).is_none_or(|s| s.is_empty()) {
                        return Err(format!("invalid register {name}"));
                    }
                }
                if let Some(width) = r.get("width_bits") {
                    if width.as_u64().is_none_or(|n| n == 0) { return Err("invalid width_bits".into()); }
                }
            }
        }
        if api.get("side_effects").and_then(|v| v.as_array()).is_none_or(|a| a.iter().any(|v| !v.is_string())) {
            return Err(format!("apis[{index}].side_effects invalid"));
        }
    }
    Ok(apis.len())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn validates_fixture_and_rejects_duplicate() {
        let mut value: serde_json::Value = serde_json::from_str(include_str!("../examples/api-contract.synthetic.json")).unwrap();
        assert_eq!(validate(&value), Ok(1));
        let original = value["apis"][0].clone();
        value["apis"].as_array_mut().unwrap().push(original);
        assert!(validate(&value).unwrap_err().contains("duplicate"));
        value["apis"].as_array_mut().unwrap().pop();
        value["schema_version"] = serde_json::json!(2);
        assert!(validate(&value).is_err());
    }
    #[test]
    fn rejects_invalid_types_and_registers() {
        let mut x: serde_json::Value = serde_json::from_str(include_str!("../examples/api-contract.synthetic.json")).unwrap();
        x["apis"][0]["outputs"][0]["width_bits"] = serde_json::json!(0);
        assert!(validate(&x).is_err());
        x["apis"][0]["outputs"][0]["width_bits"] = serde_json::json!(8);
        x["apis"][0]["inputs"] = serde_json::json!("not-array");
        assert!(validate(&x).is_err());
    }
}
