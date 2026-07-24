#[test]
fn login_item_controller_does_not_invoke_the_btm_diagnostic() {
    let forbidden_command = ["sfl", "tool"].concat();
    let source = include_str!("../src/login_item.rs");

    assert!(
        !source.contains(&forbidden_command),
        "the launch-at-login adapter must not invoke the BTM diagnostic"
    );
}
