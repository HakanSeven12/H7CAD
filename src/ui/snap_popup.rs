//! H7CAD-style OSNAP dropdown popup panel.
//!
//! Rendered as a floating overlay above the status bar.  The popup is only
//! inserted into the view stack when `snap_popup_open` is true.

use iced::widget::{button, column, container, mouse_area, row, text};
use iced::{Background, Border, Color, Element, Fill, Length, Padding, Theme};

use crate::app::Message;
use crate::snap::{SnapType, Snapper, ALL_SNAP_MODES};

/// Returns a full-screen overlay element:
///   - a transparent click-catcher that closes the popup on outside click
///   - the popup panel pinned to the bottom-right (above the status bar)
pub fn snap_popup_overlay<'a>(snapper: &'a Snapper, right_offset: f32) -> Element<'a, Message> {
    // ── Panel content ─────────────────────────────────────────────────────
    let all_on = snapper.all_on();
    let none_on = snapper.none_on();

    // "Select All / Clear All" header row
    let header = row![
        header_btn("Select All", Message::SnapSelectAll, !all_on),
        header_btn("Clear All", Message::SnapClearAll, !none_on),
    ]
    .spacing(1)
    .padding([4u16, 8]);

    // Divider
    let divider = container(iced::widget::Space::new().height(1))
        .style(|_: &Theme| container::Style {
            background: Some(Background::Color(DIVIDER)),
            ..Default::default()
        })
        .width(Fill)
        .padding([0, 4]);

    // Snap mode rows
    let mut rows: Vec<Element<'_, Message>> = Vec::new();
    for &(snap_type, icon, label) in ALL_SNAP_MODES {
        rows.push(snap_row(snap_type, icon, label, snapper.is_on(snap_type)));
    }

    // "Object Snap Settings…" footer
    let footer_divider = container(iced::widget::Space::new().height(1))
        .style(|_: &Theme| container::Style {
            background: Some(Background::Color(DIVIDER)),
            ..Default::default()
        })
        .width(Fill)
        .padding([0, 4]);

    let footer = button(text("Object Snap Settings…").size(11).color(Color {
        r: 0.75,
        g: 0.75,
        b: 0.75,
        a: 1.0,
    }))
    .on_press(Message::Command("DSETTINGS".into()))
    .style(|_: &Theme, status| button::Style {
        background: Some(Background::Color(match status {
            button::Status::Hovered => ROW_HOVER,
            _ => Color::TRANSPARENT,
        })),
        ..Default::default()
    })
    .width(Fill)
    .padding([5, 12]);

    let panel_content = column![header, divider, column(rows), footer_divider, footer,];

    let panel = container(panel_content)
        .style(|_: &Theme| container::Style {
            background: Some(Background::Color(PANEL_BG)),
            border: Border {
                color: PANEL_BORDER,
                width: 1.0,
                radius: 3.0.into(),
            },
            ..Default::default()
        })
        .width(Length::Fixed(210.0));

    // ── Position: bottom-right, above the status bar (26 px) ─────────────
    let positioned = container(panel)
        .align_right(Fill)
        .align_bottom(Fill)
        .padding(Padding {
            bottom: 27.0,
            right: right_offset,
            top: 0.0,
            left: 0.0,
        })
        .width(Fill)
        .height(Fill);

    // ── Click-catcher: closes popup on any outside click ──────────────────
    mouse_area(positioned)
        .on_press(Message::CloseSnapPopup)
        .into()
}

// ── Individual snap row ───────────────────────────────────────────────────

fn snap_row<'a>(
    snap_type: SnapType,
    icon: &'a str,
    label: &'a str,
    active: bool,
) -> Element<'a, Message> {
    let checkmark = text(if active { "✓" } else { "  " })
        .size(11)
        .color(if active {
            CHECK_COLOR
        } else {
            Color::TRANSPARENT
        })
        .width(Length::Fixed(14.0));

    let icon_el = text(icon)
        .size(11)
        .color(ICON_COLOR)
        .width(Length::Fixed(16.0));

    let label_el = text(label)
        .size(11)
        .color(if active { LABEL_ON } else { LABEL_OFF });

    let content = row![checkmark, icon_el, label_el]
        .spacing(4)
        .align_y(iced::Center);

    button(content)
        .on_press(Message::ToggleSnap(snap_type))
        .style(|_: &Theme, status| button::Style {
            background: Some(Background::Color(match status {
                button::Status::Hovered => ROW_HOVER,
                _ => Color::TRANSPARENT,
            })),
            ..Default::default()
        })
        .width(Fill)
        .padding([3, 8])
        .into()
}

fn header_btn(label: &str, msg: Message, enabled: bool) -> Element<'_, Message> {
    let b = button(text(label).size(10).color(if enabled {
        Color {
            r: 0.70,
            g: 0.70,
            b: 0.70,
            a: 1.0,
        }
    } else {
        Color {
            r: 0.38,
            g: 0.38,
            b: 0.38,
            a: 1.0,
        }
    }));
    let b = if enabled { b.on_press(msg) } else { b };
    b.style(|_: &Theme, status| button::Style {
        background: Some(Background::Color(match status {
            button::Status::Hovered => ROW_HOVER,
            _ => BTN_BG,
        })),
        border: Border {
            color: PANEL_BORDER,
            width: 1.0,
            radius: 2.0.into(),
        },
        ..Default::default()
    })
    .padding([3, 8])
    .into()
}

// ── Colours ───────────────────────────────────────────────────────────────

const PANEL_BG: Color = Color {
    r: 0.16,
    g: 0.16,
    b: 0.16,
    a: 0.98,
};
const PANEL_BORDER: Color = Color {
    r: 0.32,
    g: 0.32,
    b: 0.32,
    a: 1.0,
};
const DIVIDER: Color = Color {
    r: 0.28,
    g: 0.28,
    b: 0.28,
    a: 1.0,
};
const ROW_HOVER: Color = Color {
    r: 0.24,
    g: 0.24,
    b: 0.24,
    a: 1.0,
};
const BTN_BG: Color = Color {
    r: 0.20,
    g: 0.20,
    b: 0.20,
    a: 1.0,
};
const CHECK_COLOR: Color = Color {
    r: 0.20,
    g: 0.75,
    b: 0.35,
    a: 1.0,
}; // green ✓
const ICON_COLOR: Color = Color {
    r: 0.25,
    g: 0.75,
    b: 0.45,
    a: 1.0,
}; // green icon
const LABEL_ON: Color = Color {
    r: 0.92,
    g: 0.92,
    b: 0.92,
    a: 1.0,
};
const LABEL_OFF: Color = Color {
    r: 0.52,
    g: 0.52,
    b: 0.52,
    a: 1.0,
};

