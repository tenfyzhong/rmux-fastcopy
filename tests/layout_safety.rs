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

#[test]
fn configured_binding_delegates_popup_setup_to_fastcopy() {
    let readme = include_str!("../README.md");
    let binding = readme
        .lines()
        .find(|line| line.starts_with("bind f "))
        .expect("README must document the rmux key binding");

    assert!(binding.starts_with("bind f run-shell "));
    assert!(binding.contains("rmux-fastcopy --pane '#{pane_id}'"));
    assert!(binding.contains("--client '#{client_pid}'"));
    assert!(!binding.contains("display-popup"));
    assert!(!binding.contains("#{pane_left}"));
    assert!(!binding.contains("#{pane_top}"));
    assert!(!binding.contains("#{pane_width}"));
    assert!(!binding.contains("#{pane_height}"));
}
