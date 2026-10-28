use gtk::gdk;
use gtk::prelude::*;

use super::{LogicalRect, MonitorInfo};

/// Lists the monitors GDK reports, in display order.
///
/// # Examples
///
/// ```ignore
/// let infos: Vec<MonitorInfo> = gdk_monitors().iter().map(monitor_info).collect();
/// ```
pub fn gdk_monitors() -> Vec<gdk::Monitor> {
    let Some(display) = gdk::Display::default() else {
        return Vec::new();
    };
    let model = display.monitors();
    (0..model.n_items())
        .filter_map(|index| model.item(index).and_downcast::<gdk::Monitor>())
        .collect()
}

/// Reads a monitor's connector name and logical geometry.
///
/// # Examples
///
/// ```ignore
/// let info = monitor_info(&monitor);
/// ```
pub fn monitor_info(monitor: &gdk::Monitor) -> MonitorInfo {
    let geometry = monitor.geometry();
    MonitorInfo {
        connector: monitor
            .connector()
            .map(|name| name.to_string())
            .unwrap_or_default(),
        logical: LogicalRect {
            x: geometry.x(),
            y: geometry.y(),
            width: geometry.width(),
            height: geometry.height(),
        },
    }
}
