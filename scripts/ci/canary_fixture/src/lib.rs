fn accepts_evidence(executed: usize) -> bool {
    executed > 0
}

#[test]
fn rejects_zero_executed() {
    assert!(!accepts_evidence(0));
}
