/// Finds the first monitor that shows a frozen frame but is no longer connected.
///
/// Added monitors and monitors without a connector name, which cannot be told apart, are never
/// reported.
///
/// # Examples
///
/// ```ignore
/// let lost = lost_monitor(&["DP-1".into(), "HDMI-A-1".into()], &["DP-1".into()]);
/// assert_eq!(lost, Some("HDMI-A-1"));
/// ```
pub fn lost_monitor<'a>(shown: &'a [String], connected: &[String]) -> Option<&'a str> {
    shown
        .iter()
        .filter(|connector| !connector.is_empty())
        .find(|connector| !connected.contains(connector))
        .map(String::as_str)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(connectors: &[&str]) -> Vec<String> {
        connectors
            .iter()
            .map(|connector| (*connector).to_owned())
            .collect()
    }

    #[test]
    fn keeps_the_selection_while_every_framed_monitor_is_connected() {
        let shown = names(&["DP-1", "HDMI-A-1"]);
        assert_eq!(lost_monitor(&shown, &names(&["HDMI-A-1", "DP-1"])), None);
    }

    #[test]
    fn reports_the_framed_monitor_that_went_away() {
        let shown = names(&["DP-1", "HDMI-A-1"]);
        assert_eq!(lost_monitor(&shown, &names(&["DP-1"])), Some("HDMI-A-1"));
        assert_eq!(lost_monitor(&shown, &[]), Some("DP-1"));
    }

    #[test]
    fn ignores_monitors_that_were_added_or_showed_no_frame() {
        let shown = names(&["DP-1"]);
        assert_eq!(lost_monitor(&shown, &names(&["DP-1", "DP-2"])), None);
        assert_eq!(lost_monitor(&shown, &names(&["DP-1"])), None);
    }

    #[test]
    fn never_reports_a_monitor_without_a_connector_name() {
        let shown = names(&["", "DP-1"]);
        assert_eq!(lost_monitor(&shown, &names(&["DP-1"])), None);
    }
}
