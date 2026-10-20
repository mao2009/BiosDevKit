//! Differential comparisons of two independently validated Trace Event v1 JSONL inputs.
//! Order is meaningful only within each (run_id, stream_id, cpu) lane.
use std::collections::{BTreeMap, BTreeSet};
use serde_json::Value;
use crate::traces::validate_jsonl;

#[derive(Debug, PartialEq, Eq)]
pub enum DifferenceKind { Missing, Extra, Reordered, Changed }
#[derive(Debug, PartialEq)]
pub struct Difference {
    pub kind: DifferenceKind,
    pub run_id: String,
    pub stream_id: String,
    pub cpu: String,
    pub index: usize,
    pub expected: Option<Value>,
    pub actual: Option<Value>,
}
type Lane = (String,String,String);
fn lanes(input: &str) -> Result<BTreeMap<Lane,Vec<Value>>,String> {
    validate_jsonl(input)?;
    let mut output:BTreeMap<Lane,Vec<Value>>=BTreeMap::new();
    for line in input.lines() {
        let v:Value=serde_json::from_str(line).map_err(|e|e.to_string())?;
        let key=(v["run_id"].as_str().unwrap().to_owned(),
                 v["stream_id"].as_str().unwrap().to_owned(),
                 v["cpu"].as_str().unwrap().to_owned());
        output.entry(key).or_default().push(v);
    }
    Ok(output)
}
fn normalized(mut v:Value)->Value {
    // Source-local sequence numbers are not comparable across independent runs.
    if let Some(obj)=v.as_object_mut() { obj.remove("sequence"); obj.remove("run_id"); }
    v
}
/// Report the first deterministic lane/index mismatch. No cross-lane causal inference.
pub fn first_difference(expected:&str,actual:&str)->Result<Option<Difference>,String> {
    let left=lanes(expected)?;
    let right=lanes(actual)?;
    let keys:BTreeSet<_>=left.keys().chain(right.keys()).cloned().collect();
    for (run,stream,cpu) in keys {
        let key=(run.clone(),stream.clone(),cpu.clone());
        let a=left.get(&key).map(Vec::as_slice).unwrap_or(&[]);
        let b=right.get(&key).map(Vec::as_slice).unwrap_or(&[]);
        for index in 0..a.len().max(b.len()) {
            let x=a.get(index).cloned();let y=b.get(index).cloned();
            if x.as_ref().map(|v|normalized(v.clone()))==y.as_ref().map(|v|normalized(v.clone())) { continue; }
            let kind=match (&x,&y) {
                (Some(_),None)=>DifferenceKind::Missing,
                (None,Some(_))=>DifferenceKind::Extra,
                (Some(xv),Some(yv))=>{
                    if a.iter().skip(index+1).any(|z|normalized(z.clone())==normalized(yv.clone()))
                      || b.iter().skip(index+1).any(|z|normalized(z.clone())==normalized(xv.clone())) {
                        DifferenceKind::Reordered
                    } else { DifferenceKind::Changed }
                },
                (None,None)=>unreachable!()
            };
            return Ok(Some(Difference{kind,run_id:run,stream_id:stream,cpu,index,expected:x,actual:y}));
        }
    }
    Ok(None)
}
#[cfg(test)]
mod tests {
 use super::*;
 fn event(run:&str,stream:&str,cpu:&str,seq:u64,address:&str)->String {
  serde_json::json!({"schema_version":1,"run_id":run,"stream_id":stream,"cpu":cpu,
   "sequence":seq,"event":"bios_call","registers":{},"address":address}).to_string()
 }
 #[test] fn match_and_changed() {
  let a=event("r","s","huc6280",1,"0x100");
  let b=event("r","s","huc6280",10,"0x100");
  assert_eq!(first_difference(&a,&b).unwrap(),None);
  let c=event("r","s","huc6280",10,"0x200");
  assert_eq!(first_difference(&a,&c).unwrap().unwrap().kind,DifferenceKind::Changed);
 }
 #[test] fn missing_extra_and_reorder() {
  let a=format!("{}\n{}",event("r","s","cpu",1,"A"),event("r","s","cpu",2,"B"));
  let b=event("r","s","cpu",1,"A");
  assert_eq!(first_difference(&a,&b).unwrap().unwrap().kind,DifferenceKind::Missing);
  assert_eq!(first_difference(&b,&a).unwrap().unwrap().kind,DifferenceKind::Extra);
  let reordered=format!("{}\n{}",event("r","s","cpu",1,"B"),event("r","s","cpu",2,"A"));
  assert_eq!(first_difference(&a,&reordered).unwrap().unwrap().kind,DifferenceKind::Reordered);
 }
 #[test] fn separate_lanes_and_bad_input() {
  let a=format!("{}\n{}",event("r","s","cpu",1,"A"),event("r","other","cpu",1,"B"));
  let b=format!("{}\n{}",event("r","other","cpu",7,"B"),event("r","s","cpu",9,"A"));
  assert_eq!(first_difference(&a,&b).unwrap(),None);
  assert!(first_difference("broken",&b).is_err());
 }
}
