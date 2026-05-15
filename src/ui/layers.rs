//! Layer Manager — floating window.

use crate::app::Message;
use crate::ui::properties::{color_picker_dropdown, lw_options, LinetypeItem, LwItem};
use crate::ui::ROW_H;
use acadrust::tables::layer::Layer as DocLayer;
use acadrust::tables::Table;
use acadrust::types::aci_table::aci_to_rgb;
use acadrust::types::{Color as AcadColor, LineWeight};
use acadrust::Handle;
use iced::widget::{
    button, column, combo_box, container, mouse_area, row, scrollable, text, text_input,
};
use iced::Padding;
use iced::{Background, Border, Color, Element, Fill, Length, Theme};

// ── Per-viewport column descriptor ───────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct VpCol {
    pub handle: Handle,
    pub label: String,
}

// ── Row-height-derived constants ─────────────────────────────────────────
/// SVG icon size inside a layer-table cell.
const ICON_SZ: f32 = ROW_H * 0.62; // ≈16 px at ROW_H=26
/// Font size for cell text.
const FONT_SZ: f32 = ROW_H * 0.42; // ≈11 px at ROW_H=26
/// Vertical padding for combo_box / text_input so their total height = ROW_H.
const COMBO_PAD_V: f32 = (ROW_H - FONT_SZ * 1.3 - 2.0) / 2.0;

// ── Layer data ────────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct Layer {
    pub name: String,
    pub visible: bool,
    pub frozen: bool,
    pub locked: bool,
    pub color: Color,
    pub linetype: String,
    pub lineweight: LineWeight,
    pub transparency: i32,
    /// Freeze state per-viewport, indexed parallel to LayerPanel::vp_cols.
    pub vp_frozen: Vec<bool>,
}

impl Layer {
    pub fn new(name: &str, color: Color) -> Self {
        Self {
            name: name.to_string(),
            visible: true,
            frozen: false,
            locked: false,
            color,
            linetype: "Continuous".to_string(),
            lineweight: LineWeight::Default,
            transparency: 0,
            vp_frozen: vec![],
        }
    }
}

// ── Panel state ───────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct LayerPanel {
    pub layers: Vec<Layer>,
    #[allow(dead_code)]
    pub visible: bool,
    pub selected: Option<usize>,
    pub editing: Option<usize>,
    pub edit_buf: String,
    pub current_layer: String,
    pub linetype_items: Vec<LinetypeItem>,
    pub color_picker_row: Option<usize>,
    pub color_full_palette: bool,
    pub linetype_combo: combo_box::State<LinetypeItem>,
    pub lw_combo: combo_box::State<LwItem>,
    /// Per-viewport columns (only populated when in a paper layout with viewports).
    pub vp_cols: Vec<VpCol>,
}

impl Default for LayerPanel {
    fn default() -> Self {
        Self {
            visible: false,
            layers: vec![Layer::new("0", Color::WHITE)],
            selected: None,
            editing: None,
            edit_buf: String::new(),
            current_layer: "0".to_string(),
            linetype_items: vec![LinetypeItem {
                name: "Continuous".into(),
                art: String::new(),
            }],
            color_picker_row: None,
            color_full_palette: false,
            linetype_combo: combo_box::State::new(vec![LinetypeItem {
                name: "Continuous".into(),
                art: String::new(),
            }]),
            lw_combo: combo_box::State::new(lw_options()),
            vp_cols: vec![],
        }
    }
}

impl LayerPanel {
    /// Sync layers + update per-viewport freeze columns.
    /// `vp_info`: list of (vp_handle, vp_label, frozen_layer_handles) from scene.
    pub fn sync_with_viewports(
        &mut self,
        doc_layers: &Table<DocLayer>,
        vp_info: Vec<(Handle, String, Vec<Handle>)>,
    ) {
        self.vp_cols = vp_info
            .iter()
            .map(|(h, label, _)| VpCol {
                handle: *h,
                label: label.clone(),
            })
            .collect();

        self.layers = doc_layers
            .iter()
            .map(|l| {
                let layer_handle = l.handle;
                let vp_frozen = vp_info
                    .iter()
                    .map(|(_, _, frozen_handles)| frozen_handles.contains(&layer_handle))
                    .collect();
                Layer {
                    name: l.name.clone(),
                    visible: !l.flags.off,
                    frozen: l.flags.frozen,
                    locked: l.flags.locked,
                    color: iced_color_from_acad(&l.color),
                    linetype: if l.line_type.is_empty() {
                        "Continuous".to_string()
                    } else {
                        l.line_type.clone()
                    },
                    lineweight: l.line_weight,
                    transparency: 0,
                    vp_frozen,
                }
            })
            .collect();
    }

    pub fn sync_linetypes(&mut self, items: Vec<LinetypeItem>) {
        self.linetype_combo = combo_box::State::new(items.clone());
        self.linetype_items = items;
    }

    /// Render the layer panel as the full content of its own OS window.
    pub fn view_window(&self) -> Element<'_, Message> {
        self.view_content()
    }

    fn view_content(&self) -> Element<'_, Message> {
        let has_sel = self.selected.is_some();
        let sel_is_zero = self
            .selected
            .map(|i| self.layers.get(i).map(|l| l.name == "0").unwrap_or(false))
            .unwrap_or(false);

        // ── Toolbar ───────────────────────────────────────────────────────
        let toolbar = container(
            row![
                toolbar_btn("⊕ New", Message::LayerNew),
                toolbar_btn_cond("⊗ Delete", Message::LayerDelete, has_sel && !sel_is_zero),
                toolbar_btn_cond("✓ Set Current", Message::LayerSetCurrent, has_sel),
            ]
            .spacing(2),
        )
        .style(|_: &Theme| container::Style {
            background: Some(Background::Color(TOOLBAR_BG)),
            ..Default::default()
        })
        .width(Fill)
        .padding([4, 8]);

        // ── Column header ─────────────────────────────────────────────────
        let mut header_row = row![
            text("Status").size(10).color(DIM).width(50),
            text("Name")
                .size(10)
                .color(DIM)
                .width(Length::Fixed(COL_NAME)),
            text("On")
                .size(10)
                .color(DIM)
                .width(Length::Fixed(COL_ICON)),
            text("Freeze")
                .size(10)
                .color(DIM)
                .width(Length::Fixed(COL_ICON)),
            text("Lock")
                .size(10)
                .color(DIM)
                .width(Length::Fixed(COL_ICON)),
            text("Color")
                .size(10)
                .color(DIM)
                .width(Length::Fixed(COL_COLOR)),
            text("Linetype")
                .size(10)
                .color(DIM)
                .width(Length::Fixed(COL_LT)),
            text("Lineweight")
                .size(10)
                .color(DIM)
                .width(Length::Fixed(COL_LW)),
            text("Transparency")
                .size(10)
                .color(DIM)
                .width(Length::Fixed(COL_TRANS)),
        ]
        .spacing(4)
        .align_y(iced::Center);

        for vp in &self.vp_cols {
            header_row = header_row.push(
                text(vp.label.as_str())
                    .size(10)
                    .color(DIM)
                    .width(Length::Fixed(COL_ICON)),
            );
        }

        let col_header = container(header_row)
            .style(|_: &Theme| container::Style {
                background: Some(Background::Color(COL_HEADER_BG)),
                border: Border {
                    color: BORDER_COLOR,
                    width: 1.0,
                    radius: 0.0.into(),
                },
                ..Default::default()
            })
            .padding([4, 8])
            .width(Fill);

        // ── Layer rows ────────────────────────────────────────────────────
        let mut rows_col = column![].spacing(0);
        for (i, layer) in self.layers.iter().enumerate() {
            let is_sel = self.selected == Some(i);
            let is_current = layer.name == self.current_layer;
            let is_editing = self.editing == Some(i);
            let color_open = self.color_picker_row == Some(i);

            let (ltc, lwc) = if is_sel {
                (Some(&self.linetype_combo), Some(&self.lw_combo))
            } else {
                (None, None)
            };

            rows_col = rows_col.push(layer_row(
                i,
                layer,
                is_sel,
                is_current,
                is_editing,
                &self.edit_buf,
                color_open,
                ltc,
                lwc,
                &self.vp_cols,
            ));

            // Insert color picker dropdown below this row when open.
            if color_open {
                rows_col = rows_col.push(color_picker_palette(self.color_full_palette));
            }
        }

        let table = scrollable(rows_col).height(Fill);

        // ── Full-window frame ─────────────────────────────────────────────
        container(column![toolbar, col_header, table].spacing(0))
            .style(|_: &Theme| container::Style {
                background: Some(Background::Color(PANEL_BG)),
                ..Default::default()
            })
            .width(Fill)
            .height(Fill)
            .into()
    }
}

// ── Toolbar buttons ───────────────────────────────────────────────────────

fn toolbar_btn(label: &str, msg: Message) -> Element<'_, Message> {
    button(text(label).size(11).color(Color::WHITE))
        .on_press(msg)
        .style(|_: &Theme, status| button::Style {
            background: Some(Background::Color(match status {
                button::Status::Hovered => Color {
                    r: 0.32,
                    g: 0.32,
                    b: 0.32,
                    a: 1.0,
                },
                button::Status::Pressed => Color {
                    r: 0.25,
                    g: 0.25,
                    b: 0.25,
                    a: 1.0,
                },
                _ => Color {
                    r: 0.26,
                    g: 0.26,
                    b: 0.26,
                    a: 1.0,
                },
            })),
            border: Border {
                radius: 3.0.into(),
                color: BORDER_COLOR,
                width: 1.0,
            },
            text_color: Color::WHITE,
            ..Default::default()
        })
        .padding([4, 10])
        .into()
}

fn toolbar_btn_cond(label: &str, msg: Message, enabled: bool) -> Element<'_, Message> {
    let mut b = button(text(label).size(11).color(if enabled {
        Color::WHITE
    } else {
        Color {
            r: 0.45,
            g: 0.45,
            b: 0.45,
            a: 1.0,
        }
    }))
    .style(|_: &Theme, status| button::Style {
        background: Some(Background::Color(match status {
            button::Status::Hovered => Color {
                r: 0.32,
                g: 0.32,
                b: 0.32,
                a: 1.0,
            },
            _ => Color {
                r: 0.26,
                g: 0.26,
                b: 0.26,
                a: 1.0,
            },
        })),
        border: Border {
            radius: 3.0.into(),
            color: BORDER_COLOR,
            width: 1.0,
        },
        text_color: Color::WHITE,
        ..Default::default()
    })
    .padding([4, 10]);
    if enabled {
        b = b.on_press(msg);
    }
    b.into()
}

// ── Layer row ─────────────────────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
fn layer_row<'a>(
    index: usize,
    layer: &'a Layer,
    is_selected: bool,
    is_current: bool,
    is_editing: bool,
    edit_buf: &'a str,
    color_picker_open: bool,
    lt_combo: Option<&'a combo_box::State<LinetypeItem>>,
    lw_combo_state: Option<&'a combo_box::State<LwItem>>,
    vp_cols: &'a [VpCol],
) -> Element<'a, Message> {
    let svg_btn = |bytes: &'static [u8], on_press: Message| -> Element<'a, Message> {
        button(
            iced::widget::svg(iced::widget::svg::Handle::from_memory(bytes))
                .width(ICON_SZ)
                .height(ICON_SZ),
        )
        .on_press(on_press)
        .style(|_: &Theme, status| button::Style {
            background: Some(Background::Color(match status {
                button::Status::Hovered => Color {
                    r: 0.35,
                    g: 0.35,
                    b: 0.35,
                    a: 1.0,
                },
                _ => Color::TRANSPARENT,
            })),
            ..Default::default()
        })
        .padding(Padding {
            top: COMBO_PAD_V,
            bottom: COMBO_PAD_V,
            left: 4.0,
            right: 4.0,
        })
        .height(Length::Fixed(ROW_H))
        .into()
    };

    let vis_svg: &'static [u8] = if layer.visible {
        include_bytes!("../../assets/icons/layers/layon.svg")
    } else {
        include_bytes!("../../assets/icons/layers/layoff.svg")
    };
    let frz_svg: &'static [u8] = if layer.frozen {
        include_bytes!("../../assets/icons/layers/layfrz.svg")
    } else {
        include_bytes!("../../assets/icons/layers/laythw.svg")
    };
    let lck_svg: &'static [u8] = if layer.locked {
        include_bytes!("../../assets/icons/layers/laylck.svg")
    } else {
        include_bytes!("../../assets/icons/layers/layulk.svg")
    };

    let status_dot: Element<'_, Message> = if is_current {
        text("✓")
            .size(13)
            .color(Color {
                r: 0.25,
                g: 0.85,
                b: 0.45,
                a: 1.0,
            })
            .into()
    } else {
        text("▣")
            .size(11)
            .color(Color {
                r: 0.55,
                g: 0.55,
                b: 0.55,
                a: 1.0,
            })
            .into()
    };

    // Name cell
    let name_cell: Element<'_, Message> = if is_editing {
        text_input("", edit_buf)
            .on_input(Message::LayerRenameEdit)
            .on_submit(Message::LayerRenameCommit)
            .size(FONT_SZ)
            .padding(Padding {
                top: COMBO_PAD_V,
                bottom: COMBO_PAD_V,
                left: 4.0,
                right: 4.0,
            })
            .style(|_: &Theme, _| iced::widget::text_input::Style {
                background: iced::Background::Color(Color {
                    r: 0.12,
                    g: 0.12,
                    b: 0.12,
                    a: 1.0,
                }),
                border: Border {
                    radius: 2.0.into(),
                    width: 1.0,
                    color: Color {
                        r: 0.45,
                        g: 0.65,
                        b: 0.90,
                        a: 1.0,
                    },
                },
                icon: Color::WHITE,
                placeholder: Color {
                    r: 0.4,
                    g: 0.4,
                    b: 0.4,
                    a: 1.0,
                },
                value: Color::WHITE,
                selection: Color {
                    r: 0.25,
                    g: 0.45,
                    b: 0.75,
                    a: 0.5,
                },
            })
            .width(Length::Fixed(COL_NAME))
            .into()
    } else {
        button(text(&layer.name).size(FONT_SZ).color(ROW_TEXT))
            .on_press(Message::LayerRenameStart(index))
            .style(|_: &Theme, status| button::Style {
                background: Some(Background::Color(match status {
                    button::Status::Hovered => Color {
                        r: 0.30,
                        g: 0.30,
                        b: 0.30,
                        a: 1.0,
                    },
                    _ => Color::TRANSPARENT,
                })),
                ..Default::default()
            })
            .padding(Padding {
                top: COMBO_PAD_V,
                bottom: COMBO_PAD_V,
                left: 4.0,
                right: 4.0,
            })
            .height(Length::Fixed(ROW_H))
            .width(Length::Fixed(COL_NAME))
            .into()
    };

    // Color cell — looks like a combo_box input; click opens swatch dropdown below row.
    let aci = iced_to_aci(layer.color);
    let cur_color_name = color_label_aci(aci).to_string();
    let layer_color = layer.color;

    let swatch = container(text(""))
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(layer_color)),
            border: Border {
                color: Color {
                    r: 0.0,
                    g: 0.0,
                    b: 0.0,
                    a: 0.5,
                },
                width: 1.0,
                radius: 1.0.into(),
            },
            ..Default::default()
        })
        .width(12)
        .height(12);

    let color_cell: Element<'_, Message> = button(
        row![
            swatch,
            text(cur_color_name).size(FONT_SZ).color(ROW_TEXT),
            iced::widget::Space::new().width(Fill),
            text(if color_picker_open { "▲" } else { "▼" })
                .size(FONT_SZ * 0.75)
                .color(DIM),
        ]
        .spacing(4)
        .align_y(iced::Center),
    )
    .on_press(Message::LayerColorPickerToggle(index))
    .style(move |_: &Theme, status| button::Style {
        background: Some(Background::Color(match status {
            button::Status::Hovered => Color {
                r: 0.20,
                g: 0.20,
                b: 0.20,
                a: 1.0,
            },
            _ => Color {
                r: 0.13,
                g: 0.13,
                b: 0.13,
                a: 1.0,
            },
        })),
        border: Border {
            radius: 2.0.into(),
            width: 1.0,
            color: if color_picker_open {
                Color {
                    r: 0.45,
                    g: 0.65,
                    b: 0.90,
                    a: 1.0,
                }
            } else {
                BORDER_COLOR
            },
        },
        ..Default::default()
    })
    .padding(Padding {
        top: COMBO_PAD_V,
        bottom: COMBO_PAD_V,
        left: 4.0,
        right: 4.0,
    })
    .height(Length::Fixed(ROW_H))
    .width(Length::Fixed(COL_COLOR))
    .into();

    // Linetype cell — uses LinetypeItem (with ASCII art) same as Properties panel
    let cur_lt_item = LinetypeItem {
        name: layer.linetype.clone(),
        art: String::new(), // art comes from combo state items; just match by name
    };
    let lt_cell: Element<'_, Message> = if let Some(state) = lt_combo {
        combo_box(
            state,
            "linetype",
            Some(&cur_lt_item),
            |item: LinetypeItem| Message::LayerLinetypeSet(item.name),
        )
        .size(FONT_SZ)
        .padding(Padding {
            top: COMBO_PAD_V,
            bottom: COMBO_PAD_V,
            left: 4.0,
            right: 4.0,
        })
        .width(Length::Fixed(COL_LT))
        .input_style(combo_input_style)
        .into()
    } else {
        text(layer.linetype.as_str())
            .size(FONT_SZ)
            .color(DIM)
            .width(Length::Fixed(COL_LT))
            .into()
    };

    // Lineweight cell
    let cur_lw_item = LwItem(layer.lineweight);
    let lw_cell: Element<'_, Message> = if let Some(state) = lw_combo_state {
        combo_box(state, "lineweight", Some(&cur_lw_item), |item: LwItem| {
            Message::LayerLineweightSet(item.0)
        })
        .size(FONT_SZ)
        .padding(Padding {
            top: COMBO_PAD_V,
            bottom: COMBO_PAD_V,
            left: 4.0,
            right: 4.0,
        })
        .width(Length::Fixed(COL_LW))
        .input_style(combo_input_style)
        .into()
    } else {
        text(cur_lw_item.to_string())
            .size(FONT_SZ)
            .color(DIM)
            .width(Length::Fixed(COL_LW))
            .into()
    };

    // Transparency cell
    let trans_str = layer.transparency.to_string();
    let trans_cell = text_input("0", &trans_str)
        .on_input(move |s| Message::LayerTransparencyEdit(index, s))
        .size(FONT_SZ)
        .padding(Padding {
            top: COMBO_PAD_V,
            bottom: COMBO_PAD_V,
            left: 4.0,
            right: 4.0,
        })
        .style(|_: &Theme, _| iced::widget::text_input::Style {
            background: iced::Background::Color(Color::TRANSPARENT),
            border: Border {
                radius: 2.0.into(),
                width: 1.0,
                color: BORDER_COLOR,
            },
            icon: Color::WHITE,
            placeholder: DIM,
            value: ROW_TEXT,
            selection: Color {
                r: 0.25,
                g: 0.45,
                b: 0.75,
                a: 0.5,
            },
        })
        .width(Length::Fixed(COL_TRANS));

    let bg = if is_selected {
        ROW_SEL
    } else if index % 2 == 0 {
        ROW_EVEN
    } else {
        ROW_ODD
    };

    let mut row_content = row![
        container(status_dot)
            .width(50)
            .align_x(iced::Center)
            .align_y(iced::Center),
        name_cell,
        container(svg_btn(vis_svg, Message::LayerToggleVisible(index)))
            .width(Length::Fixed(COL_ICON))
            .align_x(iced::Center),
        container(svg_btn(frz_svg, Message::LayerToggleFreeze(index)))
            .width(Length::Fixed(COL_ICON))
            .align_x(iced::Center),
        container(svg_btn(lck_svg, Message::LayerToggleLock(index)))
            .width(Length::Fixed(COL_ICON))
            .align_x(iced::Center),
        color_cell,
        lt_cell,
        lw_cell,
        trans_cell,
    ]
    .spacing(4)
    .align_y(iced::Center);

    // Per-viewport freeze columns
    for (vp_idx, _vp_col) in vp_cols.iter().enumerate() {
        let is_vp_frozen = layer.vp_frozen.get(vp_idx).copied().unwrap_or(false);
        let vp_frz_svg: &'static [u8] = if is_vp_frozen {
            include_bytes!("../../assets/icons/layers/layfrz.svg")
        } else {
            include_bytes!("../../assets/icons/layers/laythw.svg")
        };
        row_content = row_content.push(
            container(svg_btn(
                vp_frz_svg,
                Message::LayerToggleVpFreeze(index, vp_idx),
            ))
            .width(Length::Fixed(COL_ICON))
            .align_x(iced::Center),
        );
    }

    mouse_area(
        container(row_content)
            .style(move |_: &Theme| container::Style {
                background: Some(Background::Color(bg)),
                ..Default::default()
            })
            .padding(Padding {
                top: 0.0,
                bottom: 0.0,
                left: 8.0,
                right: 8.0,
            })
            .height(Length::Fixed(ROW_H))
            .width(Fill),
    )
    .on_press(Message::LayerSelect(index))
    .into()
}

// ── Combo style ────────────────────────────────────────────────────────────

fn combo_input_style(
    _theme: &Theme,
    _status: iced::widget::text_input::Status,
) -> iced::widget::text_input::Style {
    iced::widget::text_input::Style {
        background: iced::Background::Color(Color {
            r: 0.13,
            g: 0.13,
            b: 0.13,
            a: 1.0,
        }),
        border: Border {
            radius: 2.0.into(),
            width: 1.0,
            color: BORDER_COLOR,
        },
        icon: Color::WHITE,
        placeholder: DIM,
        value: Color::WHITE,
        selection: Color {
            r: 0.25,
            g: 0.45,
            b: 0.75,
            a: 0.5,
        },
    }
}

// ── Color picker palette (shown below row) ────────────────────────────────

fn color_picker_palette<'a>(full: bool) -> Element<'a, Message> {
    // Horizontal offset to align dropdown under the Color column.
    const LEFT_OFFSET: f32 =
        20.0 + 4.0 + COL_NAME + 4.0 + COL_ICON + 4.0 + COL_ICON + 4.0 + COL_ICON + 4.0 + 8.0;

    // Use the shared color_picker_dropdown from properties — no ByLayer/ByBlock for layers.
    let dropdown = color_picker_dropdown(full, Message::LayerColorMorePalette, None, None, |aci| {
        Message::LayerColorSet(aci)
    });

    row![iced::widget::Space::new().width(LEFT_OFFSET), dropdown,].into()
}

// ── Display helpers ───────────────────────────────────────────────────────

#[allow(dead_code)]
fn aci_color_display(i: u8) -> (Color, &'static str) {
    let (r, g, b) = aci_to_rgb(i).unwrap_or((200, 200, 200));
    (
        Color::from_rgb(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0),
        "",
    )
}

fn iced_to_aci(c: Color) -> u8 {
    let r = (c.r * 255.0) as u8;
    let g = (c.g * 255.0) as u8;
    let b = (c.b * 255.0) as u8;
    for i in 1u8..=255 {
        if let Some((ar, ag, ab)) = aci_to_rgb(i) {
            if ar == r && ag == g && ab == b {
                return i;
            }
        }
    }
    7
}

fn color_label_aci(i: u8) -> &'static str {
    match i {
        1 => "red",
        2 => "yellow",
        3 => "green",
        4 => "cyan",
        5 => "blue",
        6 => "magenta",
        7 => "white",
        8 => "dark gray",
        9 => "gray",
        _ => "white",
    }
}

pub fn iced_color_from_acad(c: &AcadColor) -> Color {
    match c {
        AcadColor::Index(i) => {
            let (r, g, b) = aci_to_rgb(*i).unwrap_or((200, 200, 200));
            Color::from_rgb(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0)
        }
        AcadColor::Rgb { r, g, b } => {
            Color::from_rgb(*r as f32 / 255.0, *g as f32 / 255.0, *b as f32 / 255.0)
        }
        _ => Color::WHITE,
    }
}

// ── Column widths ─────────────────────────────────────────────────────────

const COL_NAME: f32 = 130.0;
const COL_ICON: f32 = 44.0;
const COL_COLOR: f32 = 90.0;
const COL_LT: f32 = 110.0;
const COL_LW: f32 = 90.0;
const COL_TRANS: f32 = 80.0;

// ── Colors ────────────────────────────────────────────────────────────────

const PANEL_BG: Color = Color {
    r: 0.18,
    g: 0.18,
    b: 0.18,
    a: 1.0,
};
const TOOLBAR_BG: Color = Color {
    r: 0.20,
    g: 0.20,
    b: 0.20,
    a: 1.0,
};
const COL_HEADER_BG: Color = Color {
    r: 0.21,
    g: 0.21,
    b: 0.21,
    a: 1.0,
};
const ROW_EVEN: Color = Color {
    r: 0.18,
    g: 0.18,
    b: 0.18,
    a: 1.0,
};
const ROW_ODD: Color = Color {
    r: 0.21,
    g: 0.21,
    b: 0.21,
    a: 1.0,
};
const ROW_SEL: Color = Color {
    r: 0.18,
    g: 0.32,
    b: 0.52,
    a: 1.0,
};
const ROW_TEXT: Color = Color {
    r: 0.85,
    g: 0.85,
    b: 0.85,
    a: 1.0,
};
const DIM: Color = Color {
    r: 0.50,
    g: 0.50,
    b: 0.50,
    a: 1.0,
};
const BORDER_COLOR: Color = Color {
    r: 0.30,
    g: 0.30,
    b: 0.30,
    a: 1.0,
};
#[allow(dead_code)]
const ICON_COLOR: Color = Color {
    r: 0.80,
    g: 0.80,
    b: 0.80,
    a: 1.0,
};
