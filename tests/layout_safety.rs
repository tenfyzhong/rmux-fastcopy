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
fn configured_popup_expands_target_pane_geometry_before_display() {
    let readme = include_str!("../README.md");
    let binding = readme
        .lines()
        .find(|line| line.starts_with("bind f "))
        .expect("README must document the rmux key binding");

    assert!(
        binding.starts_with("bind f run-shell -C "),
        "rmux 0.10 does not expand pane formats in display-popup sizes directly"
    );
    for argument in [
        "-x #{pane_left}",
        "-y #{pane_top}",
        "-w #{pane_width}",
        "-h #{pane_height}",
    ] {
        assert!(
            binding.contains(argument),
            "popup binding must include {argument}"
        );
    }
    assert!(!binding.contains("-w 100%"));
    assert!(!binding.contains("-h 100%"));
}
