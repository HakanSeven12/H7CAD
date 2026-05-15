//! Scale picker dropdown — annotation scale (model space) or viewport scale (paper space).
//!
//! Rendered as a floating overlay above the status bar, same pattern as snap_popup.

use iced::widget::{button, column, container, mouse_area, row, text};
use iced::{Background, Border, Color, Element, Fill, Length, Padding, Theme};

use crate::app::Message;

/// (label, annotation_scale_multiplier, viewport_scale_factor)
/// annotation_scale = multiplier for text/dim sizes (50.0 for "1:50")
/// vp_scale = custom_scale value on the Viewport entity (0.02 for "1:50")
const COMMON_SCALES: &[(&str, f32, f64)] = &[
    ("1:100", 100.0, 0.01),
    ("1:50", 50.0, 0.02),
    ("1:20", 20.0, 0.05),
    ("1:10", 10.0, 0.10),
    ("1:5", 5.0, 0.20),
    ("1:2", 2.0, 0.50),
    ("1:1", 1.0, 1.00),
    ("2:1", 0.5, 2.00),
    ("5:1", 0.2, 5.00),
    ("10:1", 0.1, 10.00),
];

/// Full-screen overlay: transparent click-catcher + scale list panel pinned bottom-right.
///
/// - `is_model`: true = model space (dispatches SetAnnotationScale), false = paper space (SetViewportScale).
/// - `current_anno_scale`: current annotation_scale from Scene (used to highlight active row in model space).
/// - `viewport_scale`: current vp.custom_scale (used to highlight in paper space).
pub fn scale_popup_overlay(
    is_model: bool,
    current_anno_scale: f32,
    viewport_scale: Option<f64>,
) -> Element<'static, Message> {
    let rows: Vec<Element<'static, Message>> = COMMON_SCALES
        .iter()
        .map(|&(label, anno_scale, vp_scale)| {
            let active = if is_model {
                (current_anno_scale - anno_scale).abs() < 0.001 * current_anno_scale.max(0.001)
            } else {
                viewport_scale
                    .map(|vs| (vs - vp_scale).abs() < 0.001 * vp_scale.max(0.001))
                    .unwrap_or(false)
            };
            let msg = if is_model {
                Message::SetAnnotationScale(anno_scale)
            } else {
                Message::SetViewportScale(vp_scale)
            };
            scale_row(label, active, msg)
        })
        .collect();

    let panel = container(column(rows))
        .style(|_: &Theme| container::Style {
            background: Some(Background::Color(PANEL_BG)),
            border: Border {
                color: PANEL_BORDER,
                width: 1.0,
                radius: 3.0.into(),
            },
            ..Default::default()
        })
        .width(Length::Fixed(120.0));

    let positioned = container(panel)
        .align_right(Fill)
        .align_bottom(Fill)
        .padding(Padding {
            bottom: 27.0,
            right: 4.0,
            top: 0.0,
            left: 0.0,
        })
        .width(Fill)
        .height(Fill);

    mouse_area(positioned)
        .on_press(Message::CloseScalePopup)
        .into()
}

fn scale_row(label: &'static str, active: bool, msg: Message) -> Element<'static, Message> {
    let check = text(if active { "✓" } else { "  " })
        .size(11)
        .color(if active {
            CHECK_COLOR
        } else {
            Color::TRANSPARENT
        })
        .width(Length::Fixed(14.0));

    let lbl = text(label)
        .size(11)
        .color(if active { LABEL_ON } else { LABEL_OFF });

    let content = row![check, lbl].spacing(6).align_y(iced::Center);

    button(content)
        .on_press(msg)
        .style(|_: &Theme, status| button::Style {
            background: Some(Background::Color(match status {
                button::Status::Hovered => ROW_HOVER,
                _ => Color::TRANSPARENT,
            })),
            ..Default::default()
        })
        .width(Fill)
        .padding([4, 10])
        .into()
}

// ── Colours ───────────────────────────────────────────────────────────────

const PANEL_BG: Color = Color {
    r: 0.15,
    g: 0.15,
    b: 0.15,
    a: 1.0,
};
const PANEL_BORDER: Color = Color {
    r: 0.32,
    g: 0.32,
    b: 0.32,
    a: 1.0,
};
const ROW_HOVER: Color = Color {
    r: 0.22,
    g: 0.22,
    b: 0.22,
    a: 1.0,
};
const CHECK_COLOR: Color = Color {
    r: 0.35,
    g: 0.75,
    b: 1.00,
    a: 1.0,
};
const LABEL_ON: Color = Color {
    r: 0.92,
    g: 0.92,
    b: 0.92,
    a: 1.0,
};
const LABEL_OFF: Color = Color {
    r: 0.65,
    g: 0.65,
    b: 0.65,
    a: 1.0,
};
