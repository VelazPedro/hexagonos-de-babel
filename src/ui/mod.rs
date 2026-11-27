//! A tiny immediate-mode toolkit drawn into the 480×270 interface layer.
//! Every widget is focusable, so the whole game works with a mouse, a
//! keyboard alone or a gamepad alone.

pub mod book;
pub mod coop;
pub mod craft;
pub mod hud;
pub mod inventory;
pub mod look;
pub mod menus;
pub mod skills;
pub mod state;
pub mod stats;
pub mod theme;

use crate::render::atlas::Atlas;
use crate::render::canvas::{Blit, Canvas, Rgba, rgb, with_alpha};
use crate::render::font::Font;
use babel_core::babel::rarity::Rarity;
use babel_core::game::events::Sfx;
use babel_core::game::vec::V2;
use theme::{ArtCache, art as drawings, frames};

pub const WIDTH: i32 = 480;
pub const HEIGHT: i32 = 270;

pub const INK: Rgba = rgb(255, 246, 226);
pub const DIM: Rgba = rgb(176, 164, 206);
pub const FAINT: Rgba = rgb(118, 104, 156);
pub const GOLD: Rgba = rgb(255, 208, 92);
pub const GOOD: Rgba = rgb(132, 232, 130);
pub const BAD: Rgba = rgb(255, 112, 112);
pub const SKY: Rgba = rgb(120, 232, 220);
/// The second decorative accent of a card; each place has its own instead.
pub const VIOLET: Rgba = rgb(190, 150, 255);
pub const PANEL_EDGE: Rgba = rgb(112, 80, 170);

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
    /// The same keys as written, for room codes and addresses.
    pub raw: Vec<char>,
    /// The characters a name may have, as written: capitals and accents too.
    pub letters: Vec<char>,
    /// Ctrl+V in a text field.
    pub paste: bool,
    pub backspace: bool,
    /// Name of a key pressed this frame, for remapping.
    pub raw_key: Option<String>,
}

/// Focus memory kept between frames.
#[derive(Default)]
pub struct Focus {
    pub index: usize,
    rects: Vec<Rect>,
    /// Smoothed position of the selection cursor.
    cursor: Option<[f32; 4]>,
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

/// Width of the arrows-and-value area at the right end of an adjustable row.
const ADJUSTER_SPAN: i32 = 112;
const ARROW_W: i32 = 14;
const SLIDER_STEPS: u8 = 10;
const SEGMENT_W: i32 = 8;

/// Hit areas of an adjustable row. What is painted and what reacts to the
/// mouse both come from here, so they can never drift apart.
#[derive(Clone, Copy, Debug)]
pub struct Zones {
    pub less: Rect,
    pub middle: Rect,
    pub more: Rect,
}

pub fn adjuster_zones(r: Rect) -> Zones {
    let left = r.x + r.w - 6 - ADJUSTER_SPAN;
    Zones {
        less: rect(left, r.y, ARROW_W, r.h),
        middle: rect(
            left + ARROW_W + 2,
            r.y,
            ADJUSTER_SPAN - 2 * ARROW_W - 4,
            r.h,
        ),
        more: rect(r.x + r.w - 6 - ARROW_W, r.y, ARROW_W, r.h),
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
    /// The gamepad was used last: prompts name its buttons.
    pub pad: bool,
    /// The look of the place being drawn.
    pub theme: theme::Theme,
    /// Whether the world should still show behind the screen.
    pub see_through: bool,
    /// Where finished art is kept; without it everything is painted anew.
    pub art: Option<&'a mut theme::ArtCache>,
    rects: Vec<Rect>,
    /// A widget used left/right, so the focus must not move sideways.
    used_horizontal: bool,
    /// A label to paint above everything else.
    tip: Option<(String, i32, i32)>,
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
            pad: false,
            theme: theme::Place::Library.theme(),
            see_through: false,
            art: None,
            rects: Vec::new(),
            used_horizontal: false,
            tip: None,
        }
    }

    /// Moves the focus toward the nearest widget in the pressed direction and
    /// paints the selection cursor on top of everything.
    pub fn finish(mut self) {
        let (mut dx, dy) = self.input.nav;
        if self.used_horizontal {
            dx = 0;
        }
        if (dx, dy) != (0, 0) && !self.rects.is_empty() {
            self.focus.index = next_focus(&self.rects, self.focus.index, (dx, dy));
            self.sounds.push(Sfx::UiMove);
        }
        if !self.rects.is_empty() {
            self.focus.index = self.focus.index.min(self.rects.len() - 1);
        }
        self.draw_cursor();
        self.draw_tip();
        self.focus.rects = std::mem::take(&mut self.rects);
    }

    /// Four corner brackets that glide to the focused widget.
    fn draw_cursor(&mut self) {
        let Some(r) = self.rects.get(self.focus.index).copied() else {
            self.focus.cursor = None;
            return;
        };
        let target = [r.x as f32, r.y as f32, r.w as f32, r.h as f32];
        let mut at = self.focus.cursor.unwrap_or(target);
        for (value, goal) in at.iter_mut().zip(target) {
            *value += (goal - *value) * 0.42;
            if (goal - *value).abs() < 0.6 {
                *value = goal;
            }
        }
        self.focus.cursor = Some(at);
        let breathe = i32::from((self.time * 5.0).sin() > 0.2);
        let [x, y, w, h] = at.map(|v| v.round() as i32);
        let (x0, y0) = (x - 1 - breathe, y - 1 - breathe);
        let (x1, y1) = (x + w + breathe, y + h + breathe);
        let len = (w / 3).clamp(2, 5);
        for (cx, cy, sx, sy) in [
            (x0, y0, 1, 1),
            (x1, y0, -1, 1),
            (x0, y1, 1, -1),
            (x1, y1, -1, -1),
        ] {
            for k in 0..len {
                self.canvas.put(cx + k * sx, cy, GOLD);
                self.canvas.put(cx, cy + k * sy, GOLD);
            }
            self.canvas.put(cx + sx, cy + sy, with_alpha(INK, 170));
        }
    }

    /// Shows a small label centred on `cx`, above whatever is drawn later.
    pub fn tooltip(&mut self, text: &str, cx: i32, y: i32) {
        self.tip = Some((text.to_string(), cx, y));
    }

    fn draw_tip(&mut self) {
        let Some((text, cx, y)) = self.tip.take() else {
            return;
        };
        let w = self.font.width(&text) + 8;
        let x = (cx - w / 2).clamp(2, WIDTH - w - 2);
        self.canvas.fill(x, y, w, 12, self.theme.well);
        self.canvas.frame(x, y, w, 12, self.theme.edge);
        self.text(&text, x + 4, y + 3, INK);
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
        let color = self.tone(color);
        self.canvas.text_outlined(self.font, text, x, y, color)
    }

    pub fn plain(&mut self, text: &str, x: i32, y: i32, color: Rgba) -> i32 {
        let color = self.tone(color);
        self.canvas.text(self.font, text, x, y, color)
    }

    pub fn centered(&mut self, text: &str, cx: i32, y: i32, color: Rgba) {
        let color = self.tone(color);
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

    /// The quiet and heading colours stand for roles, which each place paints
    /// in its own tone.
    pub fn tone(&self, color: Rgba) -> Rgba {
        let theme = &self.theme;
        [(DIM, theme.dim), (FAINT, theme.faint), (SKY, theme.heading)]
            .into_iter()
            .find(|(role, _)| role[..3] == color[..3])
            .map_or(color, |(_, own)| with_alpha(own, color[3]))
    }

    /// Draws something with the look of another place.
    pub fn with_place<R>(&mut self, place: theme::Place, draw: impl FnOnce(&mut Self) -> R) -> R {
        let before = self.theme;
        self.theme = place.theme();
        let out = draw(self);
        self.theme = before;
        out
    }

    /// The title of a screen on its plaque, with the emblem of the place at
    /// both sides. `y` is where the letters start.
    pub fn heading(&mut self, text: &str, y: i32) {
        let theme = self.theme;
        let emblem = drawings::emblem(theme.place);
        let (ew, eh) = (emblem[0].len() as i32, emblem.len() as i32);
        let width = self.font.width(text);
        // A title too long for the large letters takes the small ones.
        let scale = if width * 2 + 22 > WIDTH - 2 * (ew + 10) {
            1
        } else {
            2
        };
        let w = width * scale + 22;
        let (x, top) = ((WIDTH - w) / 2, (y - 4).max(0));
        // Tall enough for the letters that hang below the line.
        frames::plaque(self.canvas, &theme, (x, top, w, 22));
        let letters = if scale == 2 { y } else { y + 3 };
        self.title(text, WIDTH / 2, letters, scale, theme.accent);
        let (left, right) = (x - ew - 6, x + w + 6);
        if left < 2 {
            return;
        }
        let ey = top + 10 - eh / 2;
        for ex in [left, right] {
            let (cx, cy) = (ex + ew / 2, ey + eh / 2);
            let glow = with_alpha(theme.accent, 80);
            self.canvas.glow(cx as f32, cy as f32, 20.0, glow);
            drawings::stamp(self.canvas, emblem, ex, ey, &theme);
        }
        let line = top + 10;
        // As far to one side as to the other, clear of the corner buttons.
        const MARGIN: i32 = 90;
        frames::rule(self.canvas, &theme, MARGIN, left - 5, line, theme.accent);
        let from = right + ew + 5;
        frames::rule(
            self.canvas,
            &theme,
            from,
            WIDTH - MARGIN,
            line,
            theme.accent,
        );
    }

    /// A heading inside a panel, with a rule that runs to `end`.
    pub fn section(&mut self, text: &str, x: i32, end: i32, y: i32, color: Rgba) {
        let color = self.tone(color);
        let after = self.text(text, x, y, color);
        let theme = self.theme;
        frames::rule(self.canvas, &theme, after + 5, end, y + 3, color);
    }

    /// A band of colour at the left of an option, to say what it belongs to.
    pub fn stripe(&mut self, r: Rect, color: Rgba) {
        self.canvas.fill(r.x + 2, r.y + 2, 2, r.h - 4, color);
    }

    /// A hexagonal frame behind a large icon.
    pub fn medallion(&mut self, cx: i32, cy: i32, size: i32) {
        let theme = self.theme;
        frames::medallion(self.canvas, &theme, cx, cy, size);
    }

    pub fn title(&mut self, text: &str, cx: i32, y: i32, scale: i32, color: Rgba) {
        let x = cx - self.font.width(text) * scale / 2;
        self.canvas.text_scaled(self.font, text, x, y, scale, color);
    }

    pub fn icon(&mut self, name: &str, x: i32, y: i32) {
        self.icon_with(name, x, y, Blit::default());
    }

    pub fn icon_with(&mut self, name: &str, x: i32, y: i32, opts: Blit) {
        let frame = self.atlas.animate(name, self.time, 4.0);
        self.canvas.sprite(self.atlas, frame, x, y, opts);
    }

    /// An icon centred on a point.
    pub fn icon_at(&mut self, name: &str, cx: i32, cy: i32, opts: Blit) {
        let frame = self.atlas.animate(name, self.time, 4.0);
        let w = (frame.w as f32 * opts.scale) as i32;
        let h = (frame.h as f32 * opts.scale) as i32;
        self.canvas
            .sprite(self.atlas, frame, cx - w / 2, cy - h / 2, opts);
    }

    /// Runs with the kept art, or with a throwaway one.
    fn with_art(&mut self, use_it: impl FnOnce(&mut ArtCache, &mut Canvas)) {
        match self.art.as_deref_mut() {
            Some(art) => use_it(art, self.canvas),
            None => use_it(&mut ArtCache::default(), self.canvas),
        }
    }

    /// A framed panel in the manner of the place.
    pub fn panel(&mut self, r: Rect) {
        let theme = self.theme;
        self.with_art(|art, canvas| {
            canvas.stamp(art.panel(&theme, r.w, r.h), r.x, r.y, None);
        });
    }

    /// A panel for details. An accent that says something, such as a rarity,
    /// runs along its top.
    pub fn card(&mut self, r: Rect, accent: Rgba) {
        self.panel(r);
        if accent != SKY && accent != VIOLET {
            self.canvas.fill(r.x + 3, r.y + 2, r.w - 6, 2, accent);
        }
    }

    /// The background of a screen: the place's own, with whatever floats in
    /// its air. `alpha` is how much it darkens a world that shows through.
    pub fn dim_background(&mut self, alpha: u8) {
        let theme = self.theme;
        let see_through = self.see_through;
        if see_through {
            for y in 0..HEIGHT {
                let tone = with_alpha(theme::tone_at(&theme, y), alpha);
                self.canvas.fill(0, y, WIDTH, 1, tone);
            }
        }
        self.with_art(|art, canvas| {
            if see_through {
                canvas.stamp(art.ornament(&theme), 0, 0, None);
            } else {
                canvas.cover(art.backdrop(&theme));
            }
        });
        theme::draw_air(self.canvas, &theme, self.time);
    }

    /// Button body; the gliding cursor marks the selection on top. Every one
    /// has a border; an `edge` other than the plain one says something.
    pub fn cell(&mut self, r: Rect, response: &Response, edge: Option<Rgba>) {
        let theme = self.theme;
        let (meaning, strength) = match edge {
            Some(c) if c[..3] == PANEL_EDGE[..3] => (None, c[3]),
            other => (other, 255),
        };
        let area = (r.x, r.y, r.w, r.h);
        let focused = response.focused;
        frames::option(self.canvas, &theme, area, focused, meaning, strength);
    }

    pub fn button(&mut self, r: Rect, label: &str) -> bool {
        self.button_enabled(r, label, true)
    }

    pub fn button_enabled(&mut self, r: Rect, label: &str, enabled: bool) -> bool {
        let response = self.focusable(r);
        self.cell(r, &response, None);
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

    /// The call to action of a screen.
    pub fn primary_button(&mut self, r: Rect, label: &str) -> bool {
        let pulse = ((self.time * 3.0).sin() * 0.5 + 0.5) * 90.0;
        self.canvas.frame(
            r.x - 2,
            r.y - 2,
            r.w + 4,
            r.h + 4,
            with_alpha(GOLD, 40 + pulse as u8),
        );
        let clicked = self.button_enabled(r, label, true);
        self.canvas.fill(r.x + 2, r.y + r.h - 3, r.w - 4, 1, GOLD);
        clicked
    }

    /// A button with an icon before its label.
    pub fn icon_button(&mut self, r: Rect, icon: &str, label: &str) -> bool {
        let response = self.focusable(r);
        self.cell(r, &response, None);
        let color = if response.focused { GOLD } else { INK };
        let total = self.font.width(label) + 16;
        let x = r.x + (r.w - total) / 2;
        self.icon_at(icon, x + 6, r.y + r.h / 2, Blit::default());
        self.text(label, x + 16, r.y + (r.h - 7) / 2, color);
        if response.activated {
            self.sounds.push(Sfx::UiConfirm);
        }
        response.activated
    }

    /// A large square button: icon over label, with an optional badge.
    pub fn tile(&mut self, r: Rect, icon: &str, label: &str, badge: Option<&str>) -> bool {
        let response = self.focusable(r);
        self.cell(r, &response, None);
        let lift = i32::from(response.focused);
        let opts = Blit {
            scale: 2.0,
            ..Default::default()
        };
        self.icon_at(icon, r.x + r.w / 2, r.y + (r.h - 12) / 2 - lift, opts);
        let color = if response.focused { GOLD } else { INK };
        self.centered(label, r.x + r.w / 2, r.y + r.h - 11, color);
        if let Some(badge) = badge {
            self.badge(badge, r.x + r.w - 2, r.y + 2);
        }
        if response.activated {
            self.sounds.push(Sfx::UiConfirm);
        }
        response.activated
    }

    /// A small red counter whose right edge is at `right`.
    pub fn badge(&mut self, text: &str, right: i32, y: i32) {
        let w = self.font.width(text) + 6;
        self.canvas.fill(right - w, y, w, 11, rgb(206, 68, 100));
        self.canvas
            .fill(right - w + 1, y, w - 2, 1, [255, 255, 255, 90]);
        self.text(text, right - w + 3, y + 2, INK);
    }

    /// Focus, background, label and the two arrows shared by adjustable rows.
    /// Returns the step asked for: -1, 0 or +1.
    fn adjuster(&mut self, r: Rect, label: &str) -> (Response, Zones, i32) {
        let response = self.focusable(r);
        let zones = adjuster_zones(r);
        self.cell(r, &response, None);
        let y = r.y + (r.h - 7) / 2;
        self.text(label, r.x + 6, y, if response.focused { INK } else { DIM });
        let mut change = 0;
        if response.focused && self.input.nav.0 != 0 {
            change = self.input.nav.0.signum();
            self.used_horizontal = true;
        }
        let mouse = self.input.mouse;
        let over = |zone: Rect| mouse.is_some_and(|m| zone.contains(m));
        let (less_hot, more_hot) = (over(zones.less), over(zones.more));
        if self.input.click {
            if less_hot {
                change = -1;
            } else if more_hot {
                change = 1;
            }
        }
        for (zone, glyph, hot) in [(zones.less, "<", less_hot), (zones.more, ">", more_hot)] {
            let box_y = zone.y + (zone.h - 11) / 2;
            let fill = if hot {
                self.theme.frame
            } else {
                self.theme.well
            };
            self.canvas.fill(zone.x + 1, box_y, zone.w - 2, 11, fill);
            let accent = self.theme.accent;
            self.centered(glyph, zone.x + zone.w / 2, box_y + 2, accent);
        }
        (response, zones, change)
    }

    /// A list of choices cycled with the arrows. Returns -1, 0 or +1.
    pub fn selector(&mut self, r: Rect, label: &str, value: &str, swatch: Option<Rgba>) -> i32 {
        let (response, zones, mut change) = self.adjuster(r, label);
        let over_middle = self.input.mouse.is_some_and(|m| zones.middle.contains(m));
        if (response.focused && self.input.confirm) || (self.input.click && over_middle) {
            change = 1;
        }
        let y = r.y + (r.h - 7) / 2;
        let mut cx = zones.middle.x + zones.middle.w / 2;
        if let Some(color) = swatch {
            let sx = zones.middle.x;
            self.canvas.fill(sx, y - 2, 11, 11, rgb(18, 8, 32));
            self.canvas.fill(sx + 1, y - 1, 9, 9, color);
            self.canvas.fill(sx + 1, y - 1, 9, 1, [255, 255, 255, 80]);
            cx += 6;
        }
        self.centered(value, cx, y, GOLD);
        if change != 0 {
            self.sounds.push(Sfx::UiMove);
        }
        change
    }

    /// A 0..=10 bar: arrows, left/right, or clicking and dragging along it.
    pub fn slider(&mut self, r: Rect, label: &str, value: u8) -> u8 {
        let (_, zones, change) = self.adjuster(r, label);
        let top = i32::from(SLIDER_STEPS);
        let mut next = (i32::from(value) + change).clamp(0, top) as u8;
        let track = zones.middle;
        if (self.input.mouse_down || self.input.click)
            && let Some(m) = self.input.mouse.filter(|&m| track.contains(m))
        {
            let steps = ((m.x - track.x as f32 - 1.0) / SEGMENT_W as f32).ceil();
            next = (steps as i32).clamp(0, top) as u8;
        }
        for i in 0..SLIDER_STEPS {
            let x = track.x + 1 + i32::from(i) * SEGMENT_W;
            // Segments grow from left to right, like a volume wedge.
            let h = 4 + i32::from(i) / 2;
            let y = r.y + (r.h + 8) / 2 - h;
            let lit = i < next;
            let color = if lit { GOLD } else { self.theme.well };
            self.canvas.fill(x, y, SEGMENT_W - 2, h, color);
            if lit {
                self.canvas
                    .fill(x, y, SEGMENT_W - 2, 1, [255, 255, 255, 130]);
            }
        }
        let y = r.y + (r.h - 7) / 2;
        self.right(&next.to_string(), zones.less.x - 5, y, GOLD);
        if next != value {
            self.sounds.push(Sfx::UiMove);
        }
        next
    }

    pub fn bar(&mut self, r: Rect, fraction: f32, color: Rgba) {
        let (back, empty) = (self.theme.frame_dark, self.theme.cell);
        self.canvas.fill(r.x - 1, r.y - 1, r.w + 2, r.h + 2, back);
        self.canvas.fill(r.x, r.y, r.w, r.h, empty);
        let w = (r.w as f32 * fraction.clamp(0.0, 1.0)).round() as i32;
        self.canvas.fill(r.x, r.y, w, r.h, color);
        self.canvas.fill(r.x, r.y, w, 1, [255, 255, 255, 90]);
    }

    /// A boxed key name, as printed on a keycap. Returns the end x.
    pub fn keycap(&mut self, key: &str, x: i32, y: i32) -> i32 {
        let w = keycap_width(self.font, key);
        self.canvas.fill(x + 1, y - 2, w - 2, 12, rgb(18, 8, 32));
        self.canvas.fill(x, y - 1, w, 10, rgb(18, 8, 32));
        self.canvas.fill(x + 1, y - 1, w - 2, 9, rgb(228, 214, 246));
        self.canvas.fill(x + 1, y + 7, w - 2, 1, rgb(142, 120, 182));
        let tx = x + (w - self.font.width(key)) / 2;
        self.canvas.text(self.font, key, tx, y, rgb(44, 24, 74));
        x + w
    }

    /// Picks the keyboard or gamepad name of a control.
    pub fn glyph(&self, keyboard: &'static str, pad: &'static str) -> &'static str {
        if self.pad { pad } else { keyboard }
    }

    /// Control hints along the bottom: a few keycaps, never a paragraph.
    pub fn hints(&mut self, items: &[(&str, &str)]) {
        const GAP: i32 = 12;
        let width: i32 = items
            .iter()
            .map(|(k, l)| keycap_width(self.font, k) + 4 + self.font.width(l) + GAP)
            .sum::<i32>()
            - GAP;
        let mut x = (WIDTH - width) / 2;
        let y = HEIGHT - 9;
        let (back, edge) = (with_alpha(self.theme.well, 220), self.theme.edge);
        self.canvas.fill(x - 7, y - 4, width + 14, 13, back);
        self.canvas.frame(x - 8, y - 5, width + 16, 15, edge);
        for (key, label) in items {
            x = self.keycap(key, x, y) + 4;
            x = self.text(label, x, y, DIM) + GAP;
        }
    }
}

pub fn keycap_width(font: &Font, key: &str) -> i32 {
    (font.width(key) + 6).max(11)
}
