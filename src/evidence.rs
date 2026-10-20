//! Strict accounting of test outcomes; BLOCKED and SKIP are never successful.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status { Pass, Fail, Blocked, Skip }
#[derive(Debug)]
pub struct Record<'a> { pub status: Status, pub reason: Option<&'a str> }
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Summary { pub total: usize, pub pass: usize, pub fail: usize, pub blocked: usize, pub skip: usize }
pub fn aggregate(records: &[Record<'_>]) -> Result<Summary, String> {
    let mut summary = Summary::default();
    for (index, record) in records.iter().enumerate() {
        if record.status != Status::Pass && record.reason.is_none_or(|s| s.trim().is_empty()) {
            return Err(format!("record[{index}]: missing reason"));
        }
        summary.total += 1;
        match record.status {
            Status::Pass => summary.pass += 1,
            Status::Fail => summary.fail += 1,
            Status::Blocked => summary.blocked += 1,
            Status::Skip => summary.skip += 1,
        }
    }
    Ok(summary)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mixed_statuses() {
        let records = [Record { status: Status::Pass, reason: None }, Record { status: Status::Fail, reason: Some("wrong result") }, Record { status: Status::Blocked, reason: Some("no emulator") }, Record { status: Status::Skip, reason: Some("disabled") }];
        assert_eq!(aggregate(&records).unwrap(), Summary { total:4, pass:1, fail:1, blocked:1, skip:1 });
    }
    #[test]
    fn rejects_empty_reason_and_handles_empty_batch() {
        assert_eq!(aggregate(&[]).unwrap(), Summary::default());
        assert!(aggregate(&[Record { status: Status::Blocked, reason: Some("  ") }]).is_err());
    }
}
