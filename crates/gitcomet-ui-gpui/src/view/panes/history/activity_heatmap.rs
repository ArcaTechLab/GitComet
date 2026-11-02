//! A compact GitHub-style heatmap of commits per day over the last year.
//!
//! It lives in its own entity behind a cached view boundary, so scrolling the
//! log never re-lays-out its few hundred cells; only new data or a theme
//! change does.

use super::super::super::*;
use gitcomet_core::domain::{COMMIT_ACTIVITY_DAYS, CommitActivity, weekday_of_unix_day};
use gitcomet_ui_kit::theme::with_alpha;
use std::time::SystemTime;

/// Cells share the full width of the pane, so only their height is fixed.
const CELL_HEIGHT_PX: f32 = 6.0;
const CELL_GAP_PX: f32 = 2.0;
const WEEKS: usize = COMMIT_ACTIVITY_DAYS / 7;
const VERTICAL_PADDING_PX: f32 = 4.0;
/// Height of the whole strip: seven rows, six gaps, padding and a hairline.
pub(in crate::view) const ACTIVITY_HEATMAP_HEIGHT_PX: f32 = CELL_HEIGHT_PX * 7.0
    + CELL_GAP_PX * 6.0
    + VERTICAL_PADDING_PX * 2.0
    + 1.0;
/// Number of shades above "no commits".
const LEVELS: u8 = 4;

pub(in crate::view) struct ActivityHeatmapView {
    theme: AppTheme,
    /// `None` while the first load is in flight; an empty grid is shown.
    activity: Option<Arc<CommitActivity>>,
}

impl ActivityHeatmapView {
    pub(in crate::view) fn new(theme: AppTheme) -> Self {
        Self {
            theme,
            activity: None,
        }
    }

    pub(in crate::view) fn set_theme(&mut self, theme: AppTheme, cx: &mut gpui::Context<Self>) {
        if self.theme != theme {
            self.theme = theme;
            cx.notify();
        }
    }

    pub(in crate::view) fn set_activity(
        &mut self,
        activity: Option<Arc<CommitActivity>>,
        cx: &mut gpui::Context<Self>,
    ) {
        if self.activity != activity {
            self.activity = activity;
            cx.notify();
        }
    }
}

impl Render for ActivityHeatmapView {
    fn render(&mut self, _window: &mut Window, _cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let theme = self.theme;
        let loading = self.activity.is_none();
        let empty;
        let activity: &CommitActivity = match &self.activity {
            Some(activity) => activity,
            None => {
                empty = CommitActivity::empty_ending_at(SystemTime::now());
                &empty
            }
        };
        let thresholds = level_thresholds(&activity.counts);
        let today = unix_day_of_now();

        let empty_color = with_alpha(theme.colors.foreground.secondary, 0.14);
        let level_colors = [
            with_alpha(theme.colors.accent.solid, 0.35),
            with_alpha(theme.colors.accent.solid, 0.55),
            with_alpha(theme.colors.accent.solid, 0.78),
            with_alpha(theme.colors.accent.solid, 1.0),
        ];
        let color_for = |level: u8| match level {
            0 => empty_color,
            n => level_colors[usize::from(n.min(LEVELS)) - 1],
        };

        let mut grid = div()
            .flex()
            .flex_1()
            .min_w(px(0.0))
            .h_full()
            .items_center()
            .gap(px(CELL_GAP_PX));
        for week in 0..WEEKS {
            let mut column = div()
                .flex()
                .flex_col()
                .flex_1()
                .min_w(px(1.0))
                .gap(px(CELL_GAP_PX));
            for weekday in 0..7 {
                let index = week * 7 + weekday;
                let day = activity.first_day + index as i64;
                let count = activity.counts.get(index).copied().unwrap_or(0);
                // The final column is the current week: days after today stay blank.
                if day > today {
                    column = column.child(div().w_full().h(px(CELL_HEIGHT_PX)).flex_none());
                    continue;
                }
                let cell = div()
                    .id(("history_activity_cell", index))
                    .w_full()
                    .h(px(CELL_HEIGHT_PX))
                    .flex_none()
                    .rounded(px(1.0))
                    .bg(color_for(level_for(count, thresholds)));
                column = column.child(if loading {
                    cell
                } else {
                    cell.gitcomet_tooltip(theme, day_tooltip(day, count))
                });
            }
            grid = grid.child(column);
        }

        div()
            .id("history_activity_heatmap")
            .debug_selector(|| "history_activity_heatmap".to_string())
            .flex()
            .items_center()
            .w_full()
            .h(px(ACTIVITY_HEATMAP_HEIGHT_PX))
            .px_2()
            .overflow_hidden()
            .bg(theme.colors.surface.canvas)
            .border_b_1()
            .border_color(theme.colors.stroke.subtle)
            .child(grid)
    }
}

fn unix_day_of_now() -> i64 {
    gitcomet_core::domain::unix_day_of(SystemTime::now())
}

/// Counts at or above these values step up a shade (GitHub-style quartiles of
/// the days that have any commits), so one busy day does not wash out the rest.
pub(in crate::view) fn level_thresholds(counts: &[u32]) -> [u32; 3] {
    let mut active: Vec<u32> = counts.iter().copied().filter(|&c| c > 0).collect();
    if active.is_empty() {
        return [1, 1, 1];
    }
    active.sort_unstable();
    let at = |q: usize| active[(active.len() * q / 4).min(active.len() - 1)];
    [at(1), at(2), at(3)]
}

/// 0 for no commits, otherwise 1..=4.
pub(in crate::view) fn level_for(count: u32, thresholds: [u32; 3]) -> u8 {
    if count == 0 {
        0
    } else if count < thresholds[0] {
        1
    } else if count < thresholds[1] {
        2
    } else if count < thresholds[2] {
        3
    } else {
        4
    }
}

fn day_tooltip(day: i64, count: u32) -> SharedString {
    const WEEKDAYS: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
    const MONTHS: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    let (year, month, date) = civil_from_unix_day(day);
    let when = format!(
        "{}, {} {} {}",
        WEEKDAYS[weekday_of_unix_day(day)],
        MONTHS[(month as usize).saturating_sub(1).min(11)],
        date,
        year
    );
    match count {
        0 => format!("No commits on {when}").into(),
        1 => format!("1 commit on {when}").into(),
        n => format!("{n} commits on {when}").into(),
    }
}

/// Proleptic Gregorian (year, month 1-12, day 1-31) for a Unix day number.
fn civil_from_unix_day(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn civil_date_matches_known_days() {
        assert_eq!(civil_from_unix_day(0), (1970, 1, 1));
        assert_eq!(civil_from_unix_day(19_723), (2024, 1, 1));
        assert_eq!(civil_from_unix_day(11_016), (2000, 2, 29));
        assert_eq!(civil_from_unix_day(-1), (1969, 12, 31));
    }

    #[test]
    fn tooltip_names_weekday_and_pluralizes() {
        // 2024-01-01 was a Monday.
        assert_eq!(&*day_tooltip(19_723, 0), "No commits on Mon, Jan 1 2024");
        assert_eq!(&*day_tooltip(19_723, 1), "1 commit on Mon, Jan 1 2024");
        assert_eq!(&*day_tooltip(19_723, 3), "3 commits on Mon, Jan 1 2024");
    }

    #[test]
    fn levels_use_quartiles_of_active_days() {
        assert_eq!(level_thresholds(&[0, 0]), [1, 1, 1]);
        let counts = [0, 1, 2, 3, 4, 5, 6, 7, 8];
        let t = level_thresholds(&counts);
        assert_eq!(level_for(0, t), 0);
        assert_eq!(level_for(1, t), 1);
        assert_eq!(level_for(8, t), 4);
        let levels: Vec<u8> = counts.iter().map(|&c| level_for(c, t)).collect();
        assert!(levels.windows(2).all(|w| w[0] <= w[1]));
    }

    #[test]
    fn height_is_compact() {
        assert!(ACTIVITY_HEATMAP_HEIGHT_PX < 60.0);
    }
}
