//! The multiplayer screen: opening a room, joining one and leaving.

use super::hud::framed;
use super::state::{Cmd, UiData, text_field};
use super::{BAD, DIM, GOLD, GOOD, HEIGHT, INK, SKY, Ui, WIDTH, rect};
use crate::net::{Failure, Punching};
use crate::render::scene::Camera;
use babel_core::game::Game;
use babel_core::game::vec::v2;

/// Longest invitation: a code, `@` and an address.
pub const INVITATION_LENGTH: usize = 48;

/// How co-op stands, for the screens that mention it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CoopStatus {
    pub is_host: bool,
    pub me_paused: bool,
    pub partner_paused: bool,
}

impl CoopStatus {
    /// The world only stops when both have paused.
    pub fn stopped(&self) -> bool {
        self.me_paused && self.partner_paused
    }
}

/// Whether people on other networks can come in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Internet {
    /// A room for this computer only; nothing is asked of the router.
    Local,
    Searching,
    /// The router opened the port by itself.
    Ready,
    /// The router did not: a guest who cannot walk in sends a reply code.
    Direct,
    Offline,
}

/// What the screen has to offer right now.
pub enum CoopView<'a> {
    /// Not connected.
    Menu {
        has_save: bool,
    },
    /// A room is open and nobody has come yet.
    Waiting {
        /// What to tell a friend.
        code: &'a str,
        /// What works from the same network, when that is not the same.
        lan: Option<&'a str>,
        internet: Internet,
        /// How knocking back at the guest is going.
        punching: Punching,
    },
    Connecting {
        /// What the host needs, if the front door turned out to be closed.
        reply: Option<String>,
    },
    Playing(CoopStatus),
}

/// What the playing screen adds with a partner: how they are, where they are
/// and what the two are waiting for.
pub fn hud(ui: &mut Ui, game: &Game, camera: &Camera, status: CoopStatus) {
    let Some(other) = game.partner() else {
        return;
    };
    framed(ui, rect(4, 48, 78, 22));
    let (word, color) = if other.body.downed {
        ("Cayó", BAD)
    } else if status.partner_paused {
        ("En pausa", GOLD)
    } else if other.player.name.is_empty() {
        ("Compañero", SKY)
    } else {
        (other.player.name.as_str(), SKY)
    };
    ui.text(word, 8, 51, color);
    let share = (other.player.hp / other.derived.max_hp.max(1.0)).clamp(0.0, 1.0);
    ui.canvas.fill(8, 62, 70, 4, [20, 10, 34, 255]);
    let fill = (70.0 * share).round() as i32;
    ui.canvas
        .fill(8, 62, fill, 4, if share < 0.35 { BAD } else { GOOD });

    // An arrow on the edge shows which way to find them.
    let at = camera.to_view(other.body.pos);
    let (x, y) = (at.x as i32, at.y as i32);
    if !(0..WIDTH).contains(&x) || !(0..HEIGHT).contains(&y) {
        let (ax, ay) = (x.clamp(10, WIDTH - 11), y.clamp(10, HEIGHT - 11));
        let toward = (at - v2(ax as f32, ay as f32)).norm();
        for step in 0..9 {
            let back = step as f32;
            for side in [-1.0f32, 0.0, 1.0] {
                let spread = side * back * 0.6;
                let px = ax as f32 - toward.x * back - toward.y * spread;
                let py = ay as f32 - toward.y * back + toward.x * spread;
                ui.canvas.fill(px as i32, py as i32, 2, 2, SKY);
            }
        }
    }

    let message = if game.body.downed {
        Some((
            "Caíste: tu compañero puede levantarte si se queda a tu lado".to_string(),
            BAD,
        ))
    } else if let Some(exit) = game.body.waiting {
        Some((
            format!("Esperando a tu compañero ({}/2)", game.votes(exit)),
            GOLD,
        ))
    } else if other.body.waiting.is_some() {
        Some(("Tu compañero te espera para salir (1/2)".to_string(), SKY))
    } else if status.partner_paused {
        Some((
            "Tu compañero puso pausa: pausá vos para detener el juego".into(),
            GOLD,
        ))
    } else {
        None
    };
    if let Some((text, color)) = message {
        let w = ui.font.width(&text) + 14;
        framed(ui, rect((WIDTH - w) / 2, HEIGHT - 88, w, 13));
        ui.centered(&text, WIDTH / 2, HEIGHT - 85, color);
    }
}

/// Adds what was typed or pasted to a field for a code.
fn edit(ui: &Ui, text: &mut String) {
    for &c in &ui.input.raw {
        let useful = c.is_ascii_alphanumeric() || matches!(c, '-' | '.' | '@');
        if useful && text.len() < INVITATION_LENGTH {
            text.push(c.to_ascii_uppercase());
        }
    }
    if ui.input.backspace {
        text.pop();
    }
}

fn lines(ui: &mut Ui, text: &[&str], y: i32) {
    for (i, line) in text.iter().enumerate() {
        ui.centered(line, WIDTH / 2, y + i as i32 * 11, DIM);
    }
}

/// A text too long for one line, centred on as many as it needs.
fn paragraph(ui: &mut Ui, text: &str, y: i32, width: i32, color: [u8; 4]) {
    for (i, line) in ui.font.wrap(text, width).iter().enumerate() {
        ui.centered(line, WIDTH / 2, y + i as i32 * 10, color);
    }
}

pub fn draw(ui: &mut Ui, view: &CoopView, data: &mut UiData, cmds: &mut Vec<Cmd>) {
    ui.dim_background(225);
    ui.heading("Multijugador", 10);
    let wide = |y: i32| rect(WIDTH / 2 - 90, y, 180, 19);
    match view {
        CoopView::Menu { has_save } => {
            ui.card(rect(60, 34, WIDTH - 120, 64), SKY);
            let explain = if *has_save {
                "Tu mundo y tu partida guardada. Tu compañero entra con un código."
            } else {
                "Primero creá tu partida con «Nueva partida»."
            };
            ui.centered(explain, WIDTH / 2, 42, INK);
            if ui.primary_button(wide(58), "Crear una sala") && *has_save {
                cmds.push(Cmd::HostRoom);
            }
            ui.card(rect(60, 106, WIDTH - 120, 70), GOLD);
            ui.centered(
                "Unirte a la sala de otra persona, con tu personaje:",
                WIDTH / 2,
                114,
                INK,
            );
            edit(ui, &mut data.invitation);
            text_field(
                ui,
                rect(76, 128, WIDTH - 152, 20),
                &data.invitation,
                "Código de la sala",
            );
            if ui.button(rect(WIDTH / 2 - 92, 154, 88, 17), "Pegar") || ui.input.paste {
                cmds.push(Cmd::PasteCode);
            }
            if ui.button(rect(WIDTH / 2 + 4, 154, 88, 17), "Unirme") {
                cmds.push(Cmd::JoinRoom);
            }
            lines(
                ui,
                &[
                    "Por internet, sin VPN ni cambios en el router. Tu progreso queda en tu PC.",
                    "En una red virtual (Tailscale, Radmin): CÓDIGO@dirección del anfitrión.",
                ],
                182,
            );
            paragraph(ui, &data.coop_error, 207, WIDTH - 60, BAD);
            if ui.button(rect(WIDTH / 2 - 44, 230, 88, 15), "Volver") {
                cmds.push(Cmd::Pop);
            }
        }
        CoopView::Waiting {
            code,
            lan,
            internet,
            punching,
        } => {
            ui.card(rect(40, 30, WIDTH - 80, 78), SKY);
            let (head, color) = match internet {
                Internet::Ready | Internet::Direct => (
                    "Pasale este código a tu compañero (sirve por internet):",
                    GOOD,
                ),
                Internet::Searching => ("Buscando cómo abrir la sala a internet...", SKY),
                Internet::Local | Internet::Offline => ("Pasale este código a tu compañero:", INK),
            };
            ui.centered(head, WIDTH / 2, 36, color);
            ui.title(code, WIDTH / 2, 48, 2, GOLD);
            if ui.primary_button(rect(WIDTH / 2 - 90, 69, 180, 17), "Copiar el código") {
                cmds.push(Cmd::CopyCode);
            }
            if let Some(lan) = lan {
                ui.centered(&format!("Desde tu misma red: {lan}"), WIDTH / 2, 94, DIM);
            }
            ui.card(rect(40, 112, WIDTH - 80, 90), GOLD);
            match internet {
                Internet::Ready | Internet::Direct => {
                    ui.centered(
                        "¿No logra entrar? Pegá acá su código de respuesta:",
                        WIDTH / 2,
                        118,
                        INK,
                    );
                    edit(ui, &mut data.reply);
                    text_field(
                        ui,
                        rect(76, 130, WIDTH - 152, 18),
                        &data.reply,
                        "Código de respuesta",
                    );
                    if ui.button(rect(WIDTH / 2 - 92, 153, 88, 16), "Pegar") || ui.input.paste {
                        cmds.push(Cmd::PasteReply);
                    }
                    if ui.button(rect(WIDTH / 2 + 4, 153, 88, 16), "Conectar") {
                        cmds.push(Cmd::SendReply);
                    }
                    let blocked = Failure::DirectBlocked.message();
                    let (status, tone) = match punching {
                        _ if !data.coop_error.is_empty() => (data.coop_error.as_str(), BAD),
                        Punching::Trying => ("Conectando directo...", SKY),
                        Punching::Blocked => (blocked.as_str(), BAD),
                        Punching::Idle if *internet == Internet::Ready => (
                            "Tu router abrió el puerto: lo normal es que entre sin este paso.",
                            DIM,
                        ),
                        Punching::Idle => ("Al pegarlo se conectan solos.", DIM),
                    };
                    paragraph(ui, status, 176, WIDTH - 100, tone);
                }
                Internet::Searching => {
                    ui.centered("Esto tarda unos segundos.", WIDTH / 2, 150, DIM);
                }
                Internet::Offline => paragraph(
                    ui,
                    "No se detectó internet: solo se puede entrar desde tu misma red.",
                    150,
                    WIDTH - 100,
                    BAD,
                ),
                Internet::Local => {}
            }
            ui.centered("Esperando a tu compañero...", WIDTH / 2, 206, SKY);
            lines(
                ui,
                &["Si Windows pregunta, permití el acceso en redes privadas y públicas."],
                218,
            );
            if ui.button(rect(WIDTH / 2 - 90, 232, 180, 15), "Cerrar la sala") {
                cmds.push(Cmd::LeaveRoom);
            }
        }
        CoopView::Connecting { reply: None } => {
            let dots = ".".repeat(1 + (ui.time * 3.0) as usize % 3);
            ui.centered(&format!("Conectando{dots}"), WIDTH / 2, 100, SKY);
            if ui.button(wide(150), "Cancelar") {
                cmds.push(Cmd::LeaveRoom);
            }
        }
        CoopView::Connecting { reply: Some(reply) } => {
            ui.card(rect(40, 40, WIDTH - 80, 124), GOLD);
            ui.centered(
                "Por la vía normal no se pudo entrar. Falta un paso:",
                WIDTH / 2,
                48,
                DIM,
            );
            ui.centered(
                "Mandale este código de respuesta a quien creó la sala:",
                WIDTH / 2,
                64,
                INK,
            );
            ui.title(reply, WIDTH / 2, 80, 2, GOLD);
            if ui.primary_button(wide(104), "Copiar") {
                cmds.push(Cmd::CopyReply);
            }
            ui.centered("Cuando lo pegue, se conectan solos.", WIDTH / 2, 132, INK);
            ui.centered("No cierres esta pantalla.", WIDTH / 2, 146, GOLD);
            let dots = ".".repeat(1 + (ui.time * 3.0) as usize % 3);
            ui.centered(&format!("Esperando{dots}"), WIDTH / 2, 180, SKY);
            if ui.button(wide(200), "Cancelar") {
                cmds.push(Cmd::LeaveRoom);
            }
        }
        CoopView::Playing(status) => {
            ui.card(rect(90, 54, WIDTH - 180, 66), SKY);
            let role = if status.is_host {
                "Sos el anfitrión: el mundo es tuyo."
            } else {
                "Estás en el mundo de tu compañero."
            };
            ui.centered("Jugando de a dos", WIDTH / 2, 66, GOOD);
            ui.centered(role, WIDTH / 2, 84, INK);
            ui.centered("Cada uno guarda su progreso en su PC.", WIDTH / 2, 98, DIM);
            if ui.button(wide(140), "Salir de la sala") {
                cmds.push(Cmd::LeaveRoom);
            }
            if ui.button(wide(166), "Volver") {
                cmds.push(Cmd::Pop);
            }
        }
    }
    let back = ui.glyph("Esc", "B");
    ui.hints(&[(back, "volver")]);
}
