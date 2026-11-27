//! Art that never changes is painted once and copied from then on.

use super::{Place, Theme, backdrop, frames};
use crate::render::canvas::Canvas;
use std::collections::HashMap;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Piece {
    /// A whole background, opaque.
    Backdrop(Place),
    /// Only its ornament, to lay over the world.
    Ornament(Place),
    /// A framed panel of a size.
    Panel(Place, i32, i32),
}

#[derive(Default)]
pub struct ArtCache {
    kept: HashMap<Piece, Canvas>,
    /// How many pieces have been painted from scratch.
    pub builds: u32,
}

impl ArtCache {
    fn piece(&mut self, key: Piece, paint: impl FnOnce() -> Canvas) -> &Canvas {
        self.kept.entry(key).or_insert_with(|| {
            self.builds += 1;
            paint()
        })
    }

    pub fn backdrop(&mut self, theme: &Theme) -> &Canvas {
        self.piece(Piece::Backdrop(theme.place), || backdrop::opaque(theme))
    }

    pub fn ornament(&mut self, theme: &Theme) -> &Canvas {
        self.piece(Piece::Ornament(theme.place), || backdrop::ornament(theme))
    }

    /// A panel `w`×`h`, painted with room below and to the right for its
    /// shadow.
    pub fn panel(&mut self, theme: &Theme, w: i32, h: i32) -> &Canvas {
        self.piece(Piece::Panel(theme.place, w, h), || {
            frames::panel(theme, w, h)
        })
    }
}
