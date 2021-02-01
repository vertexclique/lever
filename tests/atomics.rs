#[test]
fn atomic_box_data_race_regression_test() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/samples/atomic_box_data_race.rs");
}
