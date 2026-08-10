#[test]
fn selection_does_not_invoke_layout_mutating_rmux_commands() {
    let implementation = include_str!("../src/main.rs");
    let forbidden_commands = [
        "swap-pane",
        "resize-pane",
        "resize-window",
        "split-window",
        "join-pane",
        "break-pane",
        "select-layout",
        "new-window",
        "kill-pane",
    ];

    for command in forbidden_commands {
        assert!(
            !implementation.contains(&format!("\"{command}\"")),
            "selection must not invoke the layout-mutating command {command}"
        );
    }
}
