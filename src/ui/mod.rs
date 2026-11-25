//! A tiny immediate-mode toolkit drawn into the 480×270 interface layer.
//! Every widget is focusable, so the whole game works with a mouse, a
//! keyboard alone or a gamepad alone.

pub mod book;
pub mod craft;
pub mod hud;
pub mod inventory;
pub mod menus;
pub mod skills;
pub mod state;

use crate::render::atlas::Atlas;
use crate::render::canvas::{Blit, Canvas, Rgba, rgb, with_alpha};
use crate::render::font::Font;
use babel_core::babel::rarity::Rarity;
use babel_core::game::events::Sfx;
use babel_core::game::vec::V2;

pub const WIDTH: i32 = 480;
pub const HEIGHT: i32 = 270;

pub const INK: Rgba = rgb(255, 246, 226);
pub const DIM: Rgba = rgb(176, 164, 206);
pub const FAINT: Rgba = rgb(118, 104, 156);
pub const GOLD: Rgba = rgb(255, 208, 92);
pub const GOOD: Rgba = rgb(132, 232, 130);
pub const BAD: Rgba = rgb(255, 112, 112);
pub const PANEL: Rgba = [38, 24, 62, 238];
pub const PANEL_EDGE: Rgba = rgb(128, 92, 190);
pub const CELL: Rgba = [62, 42, 96, 245];
pub const CELL_HOT: Rgba = [96, 66, 146, 255];

pub fn rarity_rgba(r: Rarity) -> Rgba {
    let c = r.color();
    rgb(c[0], c[1], c[2])
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

pub const fn rect(x: i32, y: i32, w: i32, h: i32) -> Rect {
    Rect { x, y, w, h }
}

impl Rect {
    pub fn contains(&self, p: V2) -> bool {
        p.x >= self.x as f32
            && p.x < (self.x + self.w) as f32
            && p.y >= self.y as f32
            && p.y < (self.y + self.h) as f32
    }

    pub fn center(&self) -> (i32, i32) {
        (self.x + self.w / 2, self.y + self.h / 2)
    }
}

/// Menu-level controls for one frame.
#[derive(Clone, Debug, Default)]
pub struct UiInput {
    /// Mouse position in view pixels.
    pub mouse: Option<V2>,
    pub mouse_moved: bool,
    pub click: bool,
    pub mouse_down: bool,
    pub release: bool,
    pub right_click: bool,
    /// Directional step with key repeat.
    pub nav: (i32, i32),
    pub confirm: bool,
    pub back: bool,
    /// Secondary action on the focused widget (recycle, withdraw…).
    pub alt: bool,
    /// Previous / next tab or page.
    pub tab: i32,
    pub scroll: i32,
    /// Text typed this frame.
    pub typed: Vec<char>,
    pub backspace: bool,
    /// Name of a key pressed this frame, for remapping.
    pub raw_key: Option<String>,
}

/// Focus memory kept between frames.
#[derive(Default)]
pub struct Focus {
    pub index: usize,
    rects: Vec<Rect>,
}

impl Focus {
    /// Widgets laid out in the last frame.
    #[cfg(test)]
    pub fn widgets(&self) -> &[Rect] {
        &self.rects
    }
}

/// The widget a directional press moves to: the nearest one that way, or the
/// next in order so that lists wrap around.
pub fn next_focus(rects: &[Rect], from: usize, (dx, dy): (i32, i32)) -> usize {
    if rects.is_empty() {
        return 0;
    }
    let from = from.min(rects.len() - 1);
    let (fx, fy) = rects[from].center();
    let score = |r: &Rect| {
        let (cx, cy) = r.center();
        let (ox, oy) = (cx - fx, cy - fy);
        let along = ox * dx + oy * dy;
        let across = (ox * dy.abs() + oy * dx.abs()).abs();
        (along > 2).then_some(along + across * 3)
    };
    let best = rects
        .iter()
        .enumerate()
        .filter_map(|(i, r)| score(r).map(|s| (s, i)))
        .min();
    match best {
        Some((_, i)) => i,
        None => {
            let step = if dx + dy > 0 { 1 } else { -1 };
            (from as i32 + step).rem_euclid(rects.len() as i32) as usize
        }
    }
}

/// What happened to a focusable widget this frame.
#[derive(Clone, Copy, Debug, Default)]
pub struct Response {
    pub focused: bool,
    /// Confirmed with keys, pad or a click.
    pub activated: bool,
    /// Mouse button went down over it.
    pub pressed: bool,
    /// Mouse button was released over it.
    pub released: bool,
    pub alt: bool,
}

pub struct Ui<'a> {
    pub canvas: &'a mut Canvas,
    pub atlas: &'a Atlas,
    pub font: &'a Font,
    pub input: &'a UiInput,
    pub focus: &'a mut Focus,
    pub sounds: &'a mut Vec<Sfx>,
    pub time: f32,
    rects: Vec<Rect>,
}

impl<'a> Ui<'a> {
    pub fn new(
        canvas: &'a mut Canvas,
        atlas: &'a Atlas,
        font: &'a Font,
        input: &'a UiInput,
        focus: &'a mut Focus,
        sounds: &'a mut Vec<Sfx>,
        time: f32,
    ) -> Ui<'a> {
        Ui {
            canvas,
            atlas,
            font,
            input,
            focus,
            sounds,
            time,
            rects: Vec::new(),
        }
    }

    /// Moves the focus toward the nearest widget in the pressed direction.
    pub fn finish(self) {
        let (dx, dy) = self.input.nav;
        let rects = self.rects;
        if (dx, dy) != (0, 0) && !rects.is_empty() {
            self.focus.index = next_focus(&rects, self.focus.index, (dx, dy));
            self.sounds.push(Sfx::UiMove);
        }
        if !rects.is_empty() {
            self.focus.index = self.focus.index.min(rects.len() - 1);
        }
        self.focus.rects = rects;
    }

    /// Gives focus to whichever widget is registered next.
    pub fn focus_next_widget(&mut self) {
        self.focus.index = self.rects.len();
    }

    pub fn focusable(&mut self, area: Rect) -> Response {
        let index = self.rects.len();
        self.rects.push(area);
        let hovered = self.input.mouse.is_some_and(|m| area.contains(m));
        if hovered && (self.input.mouse_moved || self.input.click) && self.focus.index != index {
            self.focus.index = index;
        }
        let focused = self.focus.index == index;
        let activated = (focused && self.input.confirm) || (hovered && self.input.click);
        Response {
            focused,
            activated,
            pressed: hovered && self.input.click,
            released: hovered && self.input.release,
            alt: (focused && self.input.alt) || (hovered && self.input.right_click),
        }
    }

    pub fn text(&mut self, text: &str, x: i32, y: i32, color: Rgba) -> i32 {
        self.canvas.text_outlined(self.font, text, x, y, color)
    }

    pub fn plain(&mut self, text: &str, x: i32, y: i32, color: Rgba) -> i32 {
        self.canvas.text(self.font, text, x, y, color)
    }

    pub fn centered(&mut self, text: &str, cx: i32, y: i32, color: Rgba) {
        self.canvas.text_centered(self.font, text, cx, y, color);
    }

    pub fn right(&mut self, text: &str, right: i32, y: i32, color: Rgba) {
        let x = right - self.font.width(text);
        self.text(text, x, y, color);
    }

    /// Word-wrapped paragraph; returns the y below it.
    pub fn paragraph(&mut self, text: &str, x: i32, y: i32, width: i32, color: Rgba) -> i32 {
        let mut line_y = y;
        for line in self.font.wrap(text, width) {
            self.plain(&line, x, line_y, color);
            line_y += 10;
        }
        line_y
    }

    pub fn title(&mut self, text: &str, cx: i32, y: i32, scale: i32, color: Rgba) {
        let x = cx - self.font.width(text) * scale / 2;
        self.canvas.text_scaled(self.font, text, x, y, scale, color);
    }

    pub fn icon(&mut self, name: &str, x: i32, y: i32) {
        let frame = self.atlas.animate(name, self.time, 4.0);
        self.canvas.sprite(self.atlas, frame, x, y, Blit::default());
    }

    pub fn icon_with(&mut self, name: &str, x: i32, y: i32, opts: Blit) {
        let frame = self.atlas.animate(name, self.time, 4.0);
        self.canvas.sprite(self.atlas, frame, x, y, opts);
    }

    /// Rounded panel with a bright edge.
    pub fn panel(&mut self, r: Rect) {
        self.canvas.fill(r.x + 1, r.y, r.w - 2, r.h, PANEL);
        self.canvas.fill(r.x, r.y + 1, 1, r.h - 2, PANEL);
        self.canvas.fill(r.x + r.w - 1, r.y + 1, 1, r.h - 2, PANEL);
        self.canvas
            .frame(r.x + 1, r.y + 1, r.w - 2, r.h - 2, PANEL_EDGE);
        self.canvas
            .fill(r.x + 2, r.y + 2, r.w - 4, 1, with_alpha(INK, 40));
    }

    pub fn dim_background(&mut self, alpha: u8) {
        self.canvas.fill(0, 0, WIDTH, HEIGHT, [18, 8, 32, alpha]);
    }

    pub fn cell(&mut self, r: Rect, response: &Response, edge: Option<Rgba>) {
        let fill = if response.focused { CELL_HOT } else { CELL };
        self.canvas.fill(r.x, r.y, r.w, r.h, fill);
        if let Some(edge) = edge {
            self.canvas.frame(r.x, r.y, r.w, r.h, edge);
        }
        if response.focused {
            let blink = if (self.time * 5.0).sin() > 0.0 {
                GOLD
            } else {
                INK
            };
            self.canvas.frame(r.x - 1, r.y - 1, r.w + 2, r.h + 2, blink);
        }
    }

    pub fn button(&mut self, r: Rect, label: &str) -> bool {
        self.button_enabled(r, label, true)
    }

    pub fn button_enabled(&mut self, r: Rect, label: &str, enabled: bool) -> bool {
        let response = self.focusable(r);
        self.cell(r, &response, Some(PANEL_EDGE));
        let color = match (enabled, response.focused) {
            (false, _) => FAINT,
            (true, true) => GOLD,
            (true, false) => INK,
        };
        self.centered(label, r.x + r.w / 2, r.y + (r.h - 7) / 2, color);
        if response.activated {
            self.sounds.push(if enabled {
                Sfx::UiConfirm
            } else {
                Sfx::UiError
            });
        }
        response.activated && enabled
    }

    /// A horizontal value adjusted with left/right while focused.
    pub fn stepper(&mut self, r: Rect, label: &str, value: &str) -> i32 {
        let response = self.focusable(r);
        self.cell(r, &response, None);
        let y = r.y + (r.h - 7) / 2;
        self.text(label, r.x + 6, y, INK);
        let right = r.x + r.w - 6;
        self.right(
            &format!("‹ {value} ›").replace('‹', "<").replace('›', ">"),
            right,
            y,
            GOLD,
        );
        let mut change = 0;
        if response.focused {
            change = self.input.nav.0;
            if self.input.confirm {
                change = 1;
            }
        }
        if response.pressed
            && let Some(m) = self.input.mouse
        {
            change = if (m.x as i32) < r.x + r.w - 36 { -1 } else { 1 };
        }
        if change != 0 {
            self.sounds.push(Sfx::UiMove);
        }
        change
    }

    pub fn bar(&mut self, r: Rect, fraction: f32, color: Rgba) {
        self.canvas
            .fill(r.x - 1, r.y - 1, r.w + 2, r.h + 2, rgb(22, 12, 38));
        self.canvas.fill(r.x, r.y, r.w, r.h, [74, 52, 104, 255]);
        let w = (r.w as f32 * fraction.clamp(0.0, 1.0)).round() as i32;
        self.canvas.fill(r.x, r.y, w, r.h, color);
        self.canvas.fill(r.x, r.y, w, 1, [255, 255, 255, 90]);
    }

    /// Control hints along the bottom of a screen.
    pub fn footer(&mut self, text: &str) {
        self.canvas
            .fill(0, HEIGHT - 11, WIDTH, 11, [20, 10, 34, 220]);
        self.centered(text, WIDTH / 2, HEIGHT - 9, DIM);
    }
}
