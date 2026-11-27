//! The shell's side of co-op: opening and joining rooms, passing controls and
//! pictures between the two computers, and keeping each player's progress on
//! their own disk. A child module of `shell`.
//!
//! The host simulates. The guest only draws a mirror of the host's game, and
//! the mirror must never be written to disk: [`Shell::save`] sees to that.

use super::{HeroArt, Shell, fresh_seed};
use crate::clipboard;
use crate::net::code::{ReplyCode, RoomCode};
use crate::net::reach::{Probe, Reach};
use crate::net::{AnswerError, GuestLink, HostLink, Joining, Room, ToGuest, ToHost, local_address};
use crate::render::fx::Fx;
use crate::render::gear::Gear;
use crate::render::pet::PetArt;
use crate::ui::coop::{CoopStatus, CoopView, INVITATION_LENGTH, Internet};
use crate::ui::hud;
use crate::ui::state::Screen;
use babel_core::game::Game;
use babel_core::game::events::Input;
use babel_core::game::profile::Profile;
use babel_core::game::sync::{Publisher, tidy_events, tidy_input};
use babel_core::save::{self, LoadOutcome};
use std::net::Ipv4Addr;

/// Menu actions of the guest carried out per frame, at most.
const ACTIONS_PER_FRAME: usize = 40;
/// A guest's progress is written at least this often, in seconds.
const GUEST_SAVE_EVERY: f32 = 20.0;

#[derive(Default)]
pub(super) enum Session {
    #[default]
    Solo,
    /// A room is open; the host may go on playing meanwhile.
    Waiting(Room),
    /// Someone is in and their character is on its way.
    Greeting(HostLink),
    Hosting(Box<Hosting>),
    Joining(Joining),
    /// Connected, waiting to be shown the host's world.
    Entering(GuestLink),
    Visiting(Box<Visit>),
}

pub(super) struct Hosting {
    link: HostLink,
    publisher: Publisher,
    /// The guest's latest controls, with presses not yet simulated.
    controls: Input,
    guest_paused: bool,
    sent_pause: Option<(bool, bool)>,
}

pub(super) struct Visit {
    link: GuestLink,
    host_paused: bool,
    sent_paused: Option<bool>,
    since_save: f32,
}

/// What a friend can be told to type.
pub(super) struct Codes {
    main: String,
    lan: Option<String>,
}

/// Why a visit ends, if it does.
type Ending = Result<(), String>;

impl Shell {
    /// Having the pause screen open, even under the options.
    fn paused(&self) -> bool {
        self.screens.contains(&Screen::Pause)
    }

    /// How co-op stands, while two are playing.
    pub fn coop_status(&self) -> Option<CoopStatus> {
        let me_paused = self.paused();
        match &self.session {
            Session::Hosting(h) => Some(CoopStatus {
                is_host: true,
                me_paused,
                partner_paused: h.guest_paused,
            }),
            Session::Visiting(v) => Some(CoopStatus {
                is_host: false,
                me_paused,
                partner_paused: v.host_paused,
            }),
            _ => None,
        }
    }

    pub(super) fn visiting(&self) -> bool {
        matches!(self.session, Session::Visiting(_))
    }

    /// The codes of the open room, while it waits: the one to give a friend
    /// and, if different, the one that works from the same network.
    pub(super) fn room_codes(&self) -> Option<Codes> {
        match &self.session {
            Session::Waiting(room) => {
                let (main, lan) = room.codes();
                Some(Codes {
                    main: main.text(),
                    lan: lan.map(|c| c.text()),
                })
            }
            _ => None,
        }
    }

    /// The code to give a friend, while the room waits.
    pub fn room_code(&self) -> Option<String> {
        self.room_codes().map(|c| c.main)
    }

    pub(super) fn coop_view<'a>(&self, codes: &'a Option<Codes>) -> CoopView<'a> {
        if let Some(status) = self.coop_status() {
            return CoopView::Playing(status);
        }
        match (&self.session, codes) {
            (Session::Solo, _) => CoopView::Menu {
                has_save: self.has_save || self.game.is_some(),
            },
            (Session::Waiting(room), Some(codes)) => CoopView::Waiting {
                code: &codes.main,
                lan: codes.lan.as_deref(),
                punching: room.punching(),
                internet: if self.net_bind.is_some() && !self.direct_loopback {
                    Internet::Local
                } else {
                    match room.reach() {
                        Reach::Searching => Internet::Searching,
                        Reach::Open(_) => Internet::Ready,
                        Reach::Direct(_) => Internet::Direct,
                        Reach::Offline => Internet::Offline,
                    }
                },
            },
            _ => CoopView::Connecting {
                reply: self.reply_code(),
            },
        }
    }

    fn coop_problem(&mut self, text: String) {
        // The multiplayer screen writes it itself; elsewhere it floats by.
        if self.top() != Some(Screen::Coop) {
            self.fx.toast(text.clone());
        }
        self.ui_data.coop_error = text;
    }

    /// Opens a room around the current game, or the saved one.
    pub(super) fn host_room(&mut self) {
        self.ui_data.coop_error.clear();
        if !matches!(self.session, Session::Solo) {
            return;
        }
        if self.game.is_none() {
            if !self.has_save {
                self.coop_problem("Primero creá tu partida.".into());
                return;
            }
            self.continue_game();
        }
        if self.game.is_none() {
            return;
        }
        let bind = self.net_bind.unwrap_or(Ipv4Addr::UNSPECIFIED);
        let advertised = self.net_bind.unwrap_or_else(local_address);
        match Room::open(bind, advertised) {
            Ok(room) => {
                // Only a room meant for other computers is offered to the internet.
                if self.direct_loopback {
                    room.reach_the_internet(Probe::loopback());
                } else if self.net_bind.is_none() {
                    room.reach_the_internet(Probe::real(advertised));
                }
                self.ui_data.reply.clear();
                self.session = Session::Waiting(room);
                if self.top() != Some(Screen::Coop) {
                    self.push(Screen::Coop);
                }
            }
            Err(e) => self.coop_problem(e.message()),
        }
    }

    /// The character a guest brings: their own saved one.
    fn own_profile(&mut self) -> Result<Profile, String> {
        self.flush_saves();
        match save::load(&self.save_path) {
            LoadOutcome::Loaded(p) | LoadOutcome::Recovered(p, _) => Ok(*p),
            LoadOutcome::NoSave => {
                let mut profile = Profile::new(fresh_seed());
                self.dress(&mut profile);
                Ok(profile)
            }
            // Joining would end up writing over it.
            LoadOutcome::Failed(_) => {
                Err("Tu partida guardada está dañada: creá una nueva antes de unirte.".into())
            }
        }
    }

    pub(super) fn join_room(&mut self) {
        self.ui_data.coop_error.clear();
        if !matches!(self.session, Session::Solo) {
            return;
        }
        let Some(room) = RoomCode::parse_invitation(&self.ui_data.invitation) else {
            self.coop_problem("Ese código no es válido. Revisalo.".into());
            return;
        };
        // What was being played is put away first.
        self.save();
        match self.own_profile() {
            Ok(profile) => {
                self.guest_home = Some(Box::new(profile));
                let probe = if self.direct_loopback {
                    Probe::loopback()
                } else {
                    Probe::real(local_address())
                };
                self.session = Session::Joining(Joining::start(room, probe));
            }
            Err(text) => self.coop_problem(text),
        }
    }

    pub(super) fn copy_code(&mut self) {
        if let Some(code) = self.room_code() {
            let text = if clipboard::write(&code) {
                "Código copiado"
            } else {
                "No se pudo copiar el código"
            };
            self.fx.toast(text.into());
        }
    }

    /// The part of the clipboard a code could be made of.
    fn pasted_code(&mut self) -> Option<String> {
        let Some(text) = clipboard::read() else {
            self.fx.toast("No hay texto para pegar".into());
            return None;
        };
        let code: String = text
            .chars()
            .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '.' | '@'))
            .take(INVITATION_LENGTH)
            .collect();
        Some(code.to_ascii_uppercase())
    }

    pub(super) fn paste_code(&mut self) {
        if let Some(code) = self.pasted_code() {
            self.ui_data.invitation = code;
        }
    }

    pub(super) fn paste_reply(&mut self) {
        if let Some(code) = self.pasted_code() {
            self.ui_data.reply = code;
            self.send_reply();
        }
    }

    /// The host takes the reply code in the field and knocks back.
    pub(super) fn send_reply(&mut self) {
        self.ui_data.coop_error.clear();
        let Session::Waiting(room) = &self.session else {
            return;
        };
        let typed = &self.ui_data.reply;
        let problem = match ReplyCode::parse(typed) {
            None if RoomCode::parse_invitation(typed).is_some() => {
                Some("Ese es un código de sala: acá va el de respuesta.")
            }
            None => Some("Ese no es un código de respuesta."),
            Some(reply) => match room.answer(reply) {
                Ok(()) => None,
                Err(AnswerError::OtherRoom) => Some("Esa respuesta es de otra sala."),
                Err(AnswerError::NotReady) => {
                    Some("La sala todavía se está preparando. Probá de nuevo.")
                }
            },
        };
        // Said under the field: a floating notice would only cover it.
        if let Some(problem) = problem {
            self.ui_data.coop_error = problem.into();
        }
    }

    pub(super) fn copy_reply(&mut self) {
        if let Some(code) = self.reply_code() {
            let text = if clipboard::write(&code) {
                "Código de respuesta copiado"
            } else {
                "No se pudo copiar el código"
            };
            self.fx.toast(text.into());
        }
    }

    /// Ends whatever co-op is going on, because the player chose to.
    pub(super) fn leave_session(&mut self) {
        match std::mem::take(&mut self.session) {
            Session::Solo => {}
            Session::Waiting(_) => {}
            Session::Greeting(link) => link.send(&ToGuest::Bye("La sala se cerró.".into())),
            Session::Hosting(host) => {
                host.link
                    .send(&ToGuest::Bye("El anfitrión cerró la sala.".into()));
                self.guest_gone();
            }
            Session::Joining(_) | Session::Entering(_) => self.guest_home = None,
            Session::Visiting(visit) => {
                visit.link.send(&ToHost::Bye);
                self.end_visit("Saliste de la sala.".into());
            }
        }
    }

    /// The host goes on alone.
    fn guest_gone(&mut self) {
        if let Some(game) = self.game.as_mut() {
            game.leave();
        }
        self.partner_art = None;
        self.partner_pet = None;
        self.autosave = true;
    }

    /// Back to the title, with the guest's progress safe on disk.
    fn end_visit(&mut self, reason: String) {
        self.save();
        self.game = None;
        self.guest_home = None;
        self.partner_art = None;
        self.partner_pet = None;
        self.art = None;
        self.fx = Fx::new();
        self.close_all();
        self.push(Screen::Title);
        self.notice = Some(format!("{reason} Tu progreso quedó guardado."));
    }

    /// Shows the host's world.
    fn start_visit(&mut self, mirror: Game) {
        self.fx = Fx::new();
        self.art = None;
        self.hud = hud::HudState::default();
        self.game = Some(mirror);
        self.death_shown = false;
        self.close_all();
    }

    /// What a guest keeps: their own world, with the character as it is now.
    /// `None` rather than ever saving the host's world.
    pub(super) fn guest_save(&self, game: &Game) -> Option<Profile> {
        let mut own = self.guest_home.as_deref()?.clone();
        own.player = game.profile.player.clone();
        Some(own)
    }

    /// Takes in what arrived since the last frame.
    pub(super) fn net_receive(&mut self, dt: f32) {
        match std::mem::take(&mut self.session) {
            Session::Solo => {}
            Session::Waiting(room) => match room.arrival() {
                None => self.session = Session::Waiting(room),
                Some(Ok(link)) => self.session = Session::Greeting(link),
                Some(Err(e)) => self.coop_problem(e.message()),
            },
            Session::Greeting(link) => self.greet(link),
            Session::Hosting(host) => self.hear_guest(host),
            Session::Joining(joining) => match joining.outcome() {
                None => self.session = Session::Joining(joining),
                Some(Ok(link)) => {
                    if let Some(own) = &self.guest_home {
                        link.send(&ToHost::Hello(Box::new(own.player.clone())));
                        self.session = Session::Entering(link);
                    }
                }
                Some(Err(e)) => {
                    self.guest_home = None;
                    self.coop_problem(e.message());
                }
            },
            Session::Entering(link) => self.enter_host_world(link),
            Session::Visiting(mut visit) => {
                visit.since_save += dt;
                if visit.since_save >= GUEST_SAVE_EVERY {
                    visit.since_save = 0.0;
                    self.autosave = true;
                }
                let (messages, open) = visit.link.receive();
                self.hear_host(visit, messages, open);
            }
        }
    }

    fn greet(&mut self, link: HostLink) {
        let (messages, open) = link.receive();
        let player = messages.into_iter().find_map(|m| match m {
            ToHost::Hello(player) => Some(player),
            _ => None,
        });
        let Some(game) = self.game.as_mut() else {
            // The host left their game; there is nothing to show.
            link.send(&ToGuest::Bye("El anfitrión dejó la partida.".into()));
            return;
        };
        let Some(player) = player else {
            if open {
                self.session = Session::Greeting(link);
            } else {
                self.coop_problem("Tu compañero se desconectó.".into());
            }
            return;
        };
        game.join(*player);
        let mut publisher = Publisher::default();
        let world_seed = game.profile.world_seed;
        if let Some(state) = publisher.capture(game) {
            link.send(&ToGuest::Start {
                world_seed,
                state: Box::new(state),
            });
        }
        self.session = Session::Hosting(Box::new(Hosting {
            link,
            publisher,
            controls: Input::default(),
            guest_paused: false,
            sent_pause: None,
        }));
        self.close_all();
        self.fx.toast("¡Llegó tu compañero!".into());
    }

    fn hear_guest(&mut self, mut host: Box<Hosting>) {
        let (messages, mut open) = host.link.receive();
        let mut actions = 0;
        for message in messages {
            match message {
                ToHost::Controls(controls) => {
                    let mut next = tidy_input(controls);
                    // A press must not be lost if two arrive in one frame.
                    next.absorb_presses(&host.controls);
                    host.controls = next;
                }
                ToHost::Action(action) => {
                    if actions < ACTIONS_PER_FRAME
                        && let Some(game) = self.game.as_mut()
                    {
                        actions += 1;
                        game.apply_action(action);
                        // Whether it worked or not, the guest gets the truth.
                        host.publisher.resend();
                    }
                }
                ToHost::Paused(paused) => host.guest_paused = paused,
                ToHost::Bye => open = false,
                ToHost::Hello(_) => {}
            }
        }
        if open && self.game.is_some() {
            self.session = Session::Hosting(host);
        } else {
            self.guest_gone();
            self.fx.toast("Tu compañero salió de la sala".into());
        }
    }

    fn enter_host_world(&mut self, link: GuestLink) {
        let (messages, open) = link.receive();
        let mut messages = messages.into_iter();
        for message in messages.by_ref() {
            match message {
                ToGuest::Start { world_seed, state } => {
                    match Game::mirror(self.data.clone(), world_seed, *state) {
                        Ok(mirror) => {
                            self.start_visit(mirror);
                            let visit = Box::new(Visit {
                                link,
                                host_paused: false,
                                sent_paused: None,
                                since_save: 0.0,
                            });
                            self.hear_host(visit, messages.collect(), open);
                        }
                        Err(_) => {
                            self.guest_home = None;
                            self.coop_problem("El anfitrión envió datos inválidos.".into());
                        }
                    }
                    return;
                }
                ToGuest::Bye(reason) => {
                    self.guest_home = None;
                    self.coop_problem(reason.chars().take(120).collect());
                    return;
                }
                _ => {}
            }
        }
        if open {
            self.session = Session::Entering(link);
        } else {
            self.guest_home = None;
            self.coop_problem("Se perdió la conexión con el anfitrión.".into());
        }
    }

    fn hear_host(&mut self, mut visit: Box<Visit>, messages: Vec<ToGuest>, open: bool) {
        let mut ending: Ending = Ok(());
        for message in messages {
            match message {
                ToGuest::State(state) => {
                    let applied = self.game.as_mut().map(|g| g.apply_state(*state));
                    if !matches!(applied, Some(Ok(()))) {
                        ending = Err("El anfitrión envió datos inválidos.".into());
                    }
                }
                ToGuest::Events(events) => self.handle_events(&tidy_events(events)),
                ToGuest::Pause { host, .. } => visit.host_paused = host,
                ToGuest::Bye(reason) => {
                    ending = Err(reason.chars().take(120).collect());
                }
                ToGuest::Start { .. } => {}
            }
            if ending.is_err() {
                break;
            }
        }
        if ending.is_ok() && !open {
            ending = Err("Se perdió la conexión con el anfitrión.".into());
        }
        match ending {
            Ok(()) => self.session = Session::Visiting(visit),
            Err(reason) => self.end_visit(reason),
        }
    }

    /// The guest's controls for this step of the simulation.
    pub(super) fn take_guest_controls(&mut self) -> Input {
        let Session::Hosting(host) = &mut self.session else {
            return Input::default();
        };
        let controls = host.controls;
        // What is held stays held; each press counts once.
        host.controls = Input {
            movement: controls.movement,
            aim_at: controls.aim_at,
            aim_dir: controls.aim_dir,
            attack_held: controls.attack_held,
            ..Input::default()
        };
        controls
    }

    /// Sends the other computer what it needs after this frame.
    pub(super) fn net_send(&mut self, local: &Input) {
        // Leaving the screen that was connecting calls it off.
        if matches!(self.session, Session::Joining(_) | Session::Entering(_))
            && !self.screens.contains(&Screen::Coop)
        {
            self.leave_session();
        }
        let paused = self.paused();
        match &mut self.session {
            Session::Hosting(host) => {
                let Some(game) = self.game.as_mut() else {
                    return;
                };
                let events = game.drain_partner_events();
                if !events.is_empty() {
                    host.link.send(&ToGuest::Events(events));
                }
                if let Some(state) = host.publisher.capture(game) {
                    let carries_changes = state.guest.is_some() || state.world.is_some();
                    let message = ToGuest::State(Box::new(state));
                    if carries_changes {
                        host.link.send(&message);
                    } else {
                        host.link.send_latest(&message);
                    }
                }
                let pause = (paused, host.guest_paused);
                if host.sent_pause != Some(pause) {
                    host.sent_pause = Some(pause);
                    host.link.send(&ToGuest::Pause {
                        host: pause.0,
                        guest: pause.1,
                    });
                }
            }
            Session::Visiting(visit) => {
                if let Some(game) = self.game.as_mut() {
                    for action in game.take_actions() {
                        visit.link.send(&ToHost::Action(action));
                    }
                }
                visit.link.send(&ToHost::Controls(*local));
                if visit.sent_paused != Some(paused) {
                    visit.sent_paused = Some(paused);
                    visit.link.send(&ToHost::Paused(paused));
                }
            }
            _ => {}
        }
    }

    /// Keeps the partner's sprites in step with what they wear.
    pub(super) fn refresh_partner(&mut self) {
        let pet = self
            .game
            .as_ref()
            .and_then(Game::partner)
            .map(|other| other.player.pet.look);
        match pet {
            None => self.partner_pet = None,
            Some(look) => {
                if self.partner_pet.as_ref().is_none_or(|a| a.look != look) {
                    self.partner_pet = Some(PetArt::new(&self.atlas, look));
                }
            }
        }
        let wanted = self
            .game
            .as_ref()
            .and_then(Game::partner)
            .map(|other| (other.player.look, Gear::of(&self.data, &other.player)));
        match wanted {
            None => self.partner_art = None,
            Some((look, gear)) => {
                let stale = self
                    .partner_art
                    .as_ref()
                    .is_none_or(|a| a.look != look || a.gear != gear);
                if stale {
                    self.partner_art = Some(HeroArt::new(&self.atlas, look, gear));
                }
            }
        }
    }

    /// Developer and test entry: a room that only this computer can reach.
    pub fn open_local_room(&mut self) -> Option<String> {
        self.net_bind = Some(Ipv4Addr::LOCALHOST);
        if self.game.is_none() && !self.has_save {
            self.new_game(Some("7".into()));
        }
        self.host_room();
        self.room_code()
    }

    /// Tests: shows the waiting room as if the internet search had ended so.
    #[cfg(test)]
    pub fn pretend_reach(&mut self, outcome: impl Fn(RoomCode) -> Reach) {
        self.net_bind = None;
        if let Session::Waiting(room) = &self.session {
            room.pretend(outcome(room.code));
        }
    }

    /// Developer and test entry: a room for this computer whose front door is
    /// closed, so that only the direct way leads in.
    pub fn open_direct_room(&mut self) -> Option<String> {
        self.direct_loopback = true;
        self.open_local_room()?;
        // The code of the direct way is ready a moment later.
        let waiting = std::time::Instant::now();
        while waiting.elapsed() < std::time::Duration::from_secs(20) {
            match &self.session {
                Session::Waiting(room) if matches!(room.reach(), Reach::Direct(_)) => {
                    return self.room_code();
                }
                Session::Waiting(_) => std::thread::sleep(std::time::Duration::from_millis(5)),
                _ => break,
            }
        }
        None
    }

    /// Developer and test entry: joins a room made by `open_direct_room`.
    pub fn join_direct(&mut self, invitation: &str) {
        self.direct_loopback = true;
        self.join_invitation(invitation);
    }

    /// The guest's reply code, while it is waiting to reach the host.
    pub fn reply_code(&self) -> Option<String> {
        match &self.session {
            Session::Joining(joining) => joining.reply().map(|r| r.text()),
            _ => None,
        }
    }

    /// Developer and test entry: the host gets this reply.
    pub fn answer_with(&mut self, reply: &str) {
        self.ui_data.reply = reply.to_string();
        self.send_reply();
    }

    /// Developer and test entry: joins as if the code had been typed.
    pub fn join_invitation(&mut self, invitation: &str) {
        self.ui_data.invitation = invitation.to_string();
        self.push(Screen::Coop);
        self.join_room();
    }
}
