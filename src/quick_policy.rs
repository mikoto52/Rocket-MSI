//! Pure completion policy shared with the embedded DLL and regression tests.
/// INSTALLEND reports Installer action-status values, not process exit codes.
pub fn install_end_result(value: i32) -> i32 {
    match value {
        1 | 5 => 0,
        2 => 1602,
        3 => 1603,
        4 => 1604,
        6 => 1609,
        0 | 7 => 1626,
        8 => 1618,
        other => other,
    }
}
pub fn launch_available(result: Option<i32>, fresh_install: bool, installing: bool) -> bool {
    fresh_install && installing && matches!(result, Some(0 | 3010))
}
pub fn auto_launch(available: bool, checked: bool, attempted: bool) -> bool {
    available && checked && !attempted
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn installer_action_success_is_not_misread_as_a_failed_exit_code() {
        for raw in [1, 5] {
            assert!(launch_available(Some(install_end_result(raw)), true, true));
        }
        for raw in [0, 2, 3, 4, 6, 7, 8] {
            assert!(!launch_available(Some(install_end_result(raw)), true, true));
        }
        assert_eq!(install_end_result(2), 1602);
        assert_eq!(install_end_result(3), 1603);
        assert_eq!(install_end_result(3010), 3010);
    }
    #[test]
    fn launch_requires_successful_new_install_and_an_explicit_choice() {
        for code in [None, Some(1602), Some(1603), Some(1641)] {
            assert!(!launch_available(code, true, true));
        }
        for code in [Some(0), Some(3010)] {
            assert!(launch_available(code, true, true));
            assert!(!launch_available(code, false, true));
            assert!(!launch_available(code, true, false));
        }
        assert!(auto_launch(true, true, false));
        assert!(!auto_launch(true, false, false));
        assert!(!auto_launch(true, true, true));
        assert!(!auto_launch(false, true, false));
    }
}
