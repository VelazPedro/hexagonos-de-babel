//! Contextual signs for the first minutes: one short line at a time.

use super::Game;
use super::events::Input;
use crate::data::{Resource, Station};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    Move,
    Harvest,
    Build,
    Read,
    Explore,
    Lamp,
    Stairs,
}

impl Step {
    pub fn bit(self) -> u16 {
        1 << self as u16
    }
}

impl Game {
    pub fn tutorial_done(&self, step: Step) -> bool {
        self.profile.world.tutorial & step.bit() != 0
    }

    pub(crate) fn mark_tutorial(&mut self, step: Step) {
        self.profile.world.tutorial |= step.bit();
    }

    pub(crate) fn update_tutorial(&mut self, input: &Input) {
        if input.movement.len() > 0.4 {
            self.mark_tutorial(Step::Move);
        }
    }

    /// The sign to show right now, if any. Keys are written between brackets and
    /// replaced by the interface with the bound control.
    pub fn tutorial_hint(&self) -> Option<&'static str> {
        let home = self.site.key.is_home();
        if !self.tutorial_done(Step::Move) {
            return Some("Movete con [move]. Este es tu hexágono.");
        }
        if !self.tutorial_done(Step::Harvest) {
            return Some("Golpeá una estantería o una piedra con [attack].");
        }
        if !self.stations_built().contains(&Station::Binder) && home {
            let p = &self.profile.player;
            return Some(
                if p.resource(Resource::Paper) >= 6 && p.resource(Resource::Stone) >= 4 {
                    "Acercate a una baldosa de obra y pulsá [interact] para la Mesa."
                } else {
                    "Juntá 6 de papel y 4 de piedra para una Mesa de encuadernador."
                },
            );
        }
        if !self.tutorial_done(Step::Read) {
            return Some("Leé el libro del sitio con [read]. Las palabras dan poder.");
        }
        if !self.tutorial_done(Step::Explore) {
            return Some("Cruzá una puerta. Cada hexágono nace de sus palabras.");
        }
        if !home
            && !self.tutorial_done(Step::Lamp)
            && self.profile.player.resource(Resource::Oil) >= 4
        {
            return Some("Encendé una lámpara de retorno con [lamp].");
        }
        if !self.tutorial_done(Step::Stairs) && self.profile.world.sites_cleared >= 1 {
            return Some("Buscá una escalera: hay una a pocos hexágonos. Mirá el mapa [map].");
        }
        if self.profile.player.skill_points > 0 && self.profile.player.skills.is_empty() {
            return Some("¡Subiste de nivel! Elegí una habilidad con [skills].");
        }
        None
    }
}
