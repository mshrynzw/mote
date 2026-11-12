//! Main 画面。
//!
//! ブラウザの HTML はプラグインに埋め込まない。色と並びだけを egui で描き直す。
//! 音は Filter が 1 基なので、画面も 1 基である。2 基目は Phase 5 の音のあと。

use std::sync::Arc;

use nice_plug::context::gui::GuiContext;
use nice_plug::prelude::*;
use egui::{
    self, Color32, CornerRadius, FontId, Frame, RichText, ScrollArea, Sense, Stroke, Vec2, vec2,
};
use nice_plug_egui::{EguiEditor, EguiEditorState, EguiNiceSettings, NiceEguiApp, create_egui_editor};

use crate::envelope::AmpParams;
use crate::filter::FilterParams;
use crate::filter_env::FilterEnvParams;
use crate::lfo::LfoParams;
use crate::master::MasterParams;
use crate::mod_env::ModEnvParams;
use crate::oscillator::OscParams;
use crate::params::MoteParams;

/// Main 画面の型。`Plugin::Editor` に渡す。
pub type MoteEditor = EguiEditor<MoteGui>;

const NAVY: Color32 = Color32::from_rgb(15, 24, 57);
const INK: Color32 = Color32::from_rgb(241, 241, 241);
const LINE: Color32 = Color32::from_rgb(164, 182, 183);
const ON: Color32 = Color32::from_rgb(159, 197, 233);
const LETTERS: [&str; 4] = ["A", "B", "C", "D"];

/// 開いている画面の状態。ノブの値そのものは `params` が持つ。
pub struct MoteGui {
    params: Arc<MoteParams>,
    /// ウィンドウが開いたあと、ノブを動かすために使う。
    gui: Option<GuiContext>,
    /// `None` は ALL。`Some` はその列だけ。
    chain: Option<usize>,
    mod_env: usize,
    lfo: usize,
}

/// プラグインの Main 画面を作る。
pub fn create(params: Arc<MoteParams>, state: Arc<EguiEditorState>) -> Option<MoteEditor> {
    create_egui_editor(
        state,
        nice_plug_egui::RepaintNotifier::new(),
        EguiNiceSettings::new().with_tile("Mote"),
        MoteGui {
            params,
            gui: None,
            chain: Some(0),
            mod_env: 0,
            lfo: 0,
        },
    )
}

/// 起動時の画面の大きさ。単位は論理ピクセル。
pub fn initial_state() -> Arc<EguiEditorState> {
    EguiEditorState::from_size(
        nice_plug::editor::dpi::LogicalSize::new(1188.0, 760.0),
        1.0,
    )
}

impl NiceEguiApp for MoteGui {
    fn build(
        &mut self,
        egui_ctx: egui::Context,
        nice_gui_ctx: GuiContext,
        _frame: &mut nice_plug_egui::Frame,
    ) -> Result<(), nice_plug_egui::baseview::HandlerError> {
        self.gui = Some(nice_gui_ctx);
        let mut visuals = egui::Visuals::dark();
        visuals.panel_fill = NAVY;
        visuals.window_fill = NAVY;
        visuals.extreme_bg_color = NAVY;
        visuals.override_text_color = Some(INK);
        egui_ctx.set_visuals(visuals);
        install_fonts(&egui_ctx);
        Ok(())
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut nice_plug_egui::Frame) {
        let Some(gui) = self.gui.clone() else {
            return;
        };
        let setter = gui.param_setter();
        ui.painter()
            .rect_filled(ui.max_rect(), CornerRadius::ZERO, NAVY);
        ScrollArea::both()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                self.header(ui);
                ui.add_space(8.0);
                ui.horizontal_top(|ui| {
                    ui.vertical(|ui| self.chains(ui, &setter));
                    ui.add_space(12.0);
                    panel(ui, "Master", |ui| {
                        master_controls(ui, &setter, &self.params.master);
                    });
                });
                ui.add_space(16.0);
                ui.horizontal_top(|ui| {
                    ui.vertical(|ui| self.mod_envs(ui, &setter));
                    ui.add_space(12.0);
                    ui.vertical(|ui| self.lfos(ui, &setter));
                });
            });
    }

    fn editor_closed(&mut self) {
        self.gui = None;
    }
}

/// 設計図と同じ書体を入れる。画面の文字は Barlow Semi Condensed、名前の Mote は Unbounded。
fn install_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        "Barlow Semi Condensed".to_owned(),
        Arc::new(egui::FontData::from_static(include_bytes!(
            "../assets/fonts/BarlowSemiCondensed-SemiBold.ttf"
        ))),
    );
    fonts.font_data.insert(
        "Unbounded".to_owned(),
        Arc::new(egui::FontData::from_static(include_bytes!(
            "../assets/fonts/Unbounded-Medium.ttf"
        ))),
    );
    fonts
        .families
        .entry(egui::FontFamily::Proportional)
        .or_default()
        .insert(0, "Barlow Semi Condensed".to_owned());
    fonts.families.insert(
        egui::FontFamily::Name("Unbounded".into()),
        vec!["Unbounded".to_owned()],
    );
    ctx.set_fonts(fonts);
}

impl MoteGui {
    fn header(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            for (index, letter) in LETTERS.iter().enumerate() {
                if tab_button(ui, letter, self.chain == Some(index)) {
                    self.chain = Some(index);
                }
            }
            if tab_button(ui, "ALL", self.chain.is_none()) {
                self.chain = None;
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    RichText::new("Mote")
                        .font(FontId::new(22.0, egui::FontFamily::Name("Unbounded".into())))
                        .color(INK),
                );
            });
        });
    }

    fn chains(&self, ui: &mut egui::Ui, setter: &ParamSetter) {
        match self.chain {
            Some(index) => self.one_chain(ui, setter, index),
            None => {
                for index in 0..4 {
                    self.one_chain(ui, setter, index);
                    ui.add_space(10.0);
                }
            }
        }
    }

    fn one_chain(&self, ui: &mut egui::Ui, setter: &ParamSetter, index: usize) {
        let osc = self.params.oscillators()[index];
        let filter = self.params.filters()[index];
        let filter_env = self.params.filter_envelopes()[index];
        let amp = self.params.amp_envelopes()[index];
        ui.horizontal_top(|ui| {
            panel(ui, &format!("Oscillator {}", LETTERS[index]), |ui| {
                osc_controls(ui, setter, osc);
            });
            panel(ui, &format!("Filter {}", LETTERS[index]), |ui| {
                filter_controls(ui, setter, filter, index);
            });
            panel(ui, &format!("Filter Env {}", LETTERS[index]), |ui| {
                filter_env_controls(ui, setter, filter_env, index);
            });
            panel(ui, &format!("Amp Env {}", LETTERS[index]), |ui| {
                amp_controls(ui, setter, amp, index);
            });
        });
    }

    fn mod_envs(&mut self, ui: &mut egui::Ui, setter: &ParamSetter) {
        letter_tabs(ui, "Mod Env", &mut self.mod_env);
        let env = self.params.mod_envelopes()[self.mod_env];
        let index = self.mod_env;
        panel(ui, &format!("Mod Env {}", LETTERS[index]), |ui| {
            mod_env_controls(ui, setter, env, index);
        });
    }

    fn lfos(&mut self, ui: &mut egui::Ui, setter: &ParamSetter) {
        letter_tabs(ui, "LFO", &mut self.lfo);
        let lfo = self.params.lfos()[self.lfo];
        let index = self.lfo;
        panel(ui, &format!("LFO {}", LETTERS[index]), |ui| {
            lfo_controls(ui, setter, lfo, index);
        });
    }
}

fn letter_tabs(ui: &mut egui::Ui, title: &str, selected: &mut usize) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(title).small().strong().color(INK));
        for (index, letter) in LETTERS.iter().enumerate() {
            if tab_button(ui, letter, *selected == index) {
                *selected = index;
            }
        }
    });
}

fn tab_button(ui: &mut egui::Ui, label: &str, on: bool) -> bool {
    let fill = if on { ON } else { NAVY };
    let text = if on { NAVY } else { INK };
    let stroke = Stroke::new(1.0, if on { ON } else { LINE });
    ui.add(
        egui::Button::new(RichText::new(label).color(text).strong())
            .fill(fill)
            .stroke(stroke)
            .corner_radius(8.0)
            .min_size(vec2(36.0, 26.0)),
    )
    .clicked()
}

fn panel(ui: &mut egui::Ui, title: &str, body: impl FnOnce(&mut egui::Ui)) {
    Frame::new()
        .fill(Color32::from_rgba_unmultiplied(15, 24, 57, 210))
        .stroke(Stroke::new(1.0, LINE))
        .corner_radius(7.0)
        .inner_margin(10.0)
        .show(ui, |ui| {
            ui.set_min_width(210.0);
            ui.label(
                RichText::new(title.to_uppercase())
                    .size(11.0)
                    .strong()
                    .color(INK),
            );
            ui.add_space(6.0);
            body(ui);
        });
}

fn wrapped(ui: &mut egui::Ui, body: impl FnOnce(&mut egui::Ui)) {
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(6.0, 8.0);
        body(ui);
    });
}

fn osc_controls(ui: &mut egui::Ui, setter: &ParamSetter, osc: &OscParams) {
    wrapped(ui, |ui| {
        control(ui, setter, &osc.on, "On");
        control(ui, setter, &osc.octave, "Octave");
        control(ui, setter, &osc.note, "Note");
        control(ui, setter, &osc.fine, "Fine");
        control(ui, setter, &osc.volume, "Volume");
        control(ui, setter, &osc.pan, "Pan");
        control(ui, setter, &osc.stereo, "Stereo");
        control(ui, setter, &osc.wave, "Wave");
        control(ui, setter, &osc.phase, "Phase");
        control(ui, setter, &osc.inv, "Inv");
        control(ui, setter, &osc.retrigger, "Retrigger");
        control(ui, setter, &osc.voices, "Voices");
        control(ui, setter, &osc.detune, "Detune");
    });
}

fn filter_controls(ui: &mut egui::Ui, setter: &ParamSetter, filter: &FilterParams, index: usize) {
    wrapped(ui, |ui| {
        control(ui, setter, &filter.kind, "Type");
        control(ui, setter, &filter.cutoff, "Cutoff");
        control(ui, setter, &filter.resonance, "Resonance");
        control(ui, setter, &filter.drive, "Drive");
        control(ui, setter, &filter.amount, "Amount");
    });
    if index == 0 {
        sync_row(
            ui,
            setter,
            &filter.sync,
            &filter.link_b,
            &filter.link_c,
            &filter.link_d,
        );
    }
}

fn filter_env_controls(
    ui: &mut egui::Ui,
    setter: &ParamSetter,
    env: &FilterEnvParams,
    index: usize,
) {
    envelope_controls(
        ui,
        setter,
        &env.attack,
        &env.attack_curve,
        &env.decay,
        &env.decay_curve,
        &env.sustain,
        &env.sustain_curve,
        &env.release,
        &env.release_curve,
    );
    wrapped(ui, |ui| control(ui, setter, &env.amount, "Amount"));
    if index == 0 {
        sync_row(
            ui,
            setter,
            &env.sync,
            &env.link_b,
            &env.link_c,
            &env.link_d,
        );
    }
}

fn amp_controls(ui: &mut egui::Ui, setter: &ParamSetter, amp: &AmpParams, index: usize) {
    envelope_controls(
        ui,
        setter,
        &amp.attack,
        &amp.attack_curve,
        &amp.decay,
        &amp.decay_curve,
        &amp.sustain,
        &amp.sustain_curve,
        &amp.release,
        &amp.release_curve,
    );
    if index == 0 {
        sync_row(ui, setter, &amp.sync, &amp.link_b, &amp.link_c, &amp.link_d);
    }
}

fn mod_env_controls(ui: &mut egui::Ui, setter: &ParamSetter, env: &ModEnvParams, index: usize) {
    envelope_controls(
        ui,
        setter,
        &env.attack,
        &env.attack_curve,
        &env.decay,
        &env.decay_curve,
        &env.sustain,
        &env.sustain_curve,
        &env.release,
        &env.release_curve,
    );
    wrapped(ui, |ui| {
        control(ui, setter, &env.amount, "Amount");
        control(ui, setter, &env.target, "Target");
        control(ui, setter, &env.chain, "Chain");
    });
    if index == 0 {
        sync_row(ui, setter, &env.sync, &env.link_b, &env.link_c, &env.link_d);
    }
}

fn lfo_controls(ui: &mut egui::Ui, setter: &ParamSetter, lfo: &LfoParams, index: usize) {
    wrapped(ui, |ui| {
        control(ui, setter, &lfo.wave, "Wave");
        control(ui, setter, &lfo.rate, "Rate");
        control(ui, setter, &lfo.tempo, "Tempo");
        control(ui, setter, &lfo.phase, "Phase");
        control(ui, setter, &lfo.retrigger, "Retrigger");
        control(ui, setter, &lfo.amount, "Amount");
        control(ui, setter, &lfo.target, "Target");
        control(ui, setter, &lfo.chain, "Chain");
    });
    if index == 0 {
        sync_row(ui, setter, &lfo.sync, &lfo.link_b, &lfo.link_c, &lfo.link_d);
    }
}

fn master_controls(ui: &mut egui::Ui, setter: &ParamSetter, master: &MasterParams) {
    wrapped(ui, |ui| {
        control(ui, setter, &master.volume, "Master Volume");
        control(ui, setter, &master.transpose, "Transpose");
        control(ui, setter, &master.bend, "Bend Range");
        control(ui, setter, &master.mode, "Mode");
        control(ui, setter, &master.legato, "Mono Legato");
        control(ui, setter, &master.portamento, "Portamento");
        control(ui, setter, &master.polyphony, "Polyphony");
    });
}

#[allow(clippy::too_many_arguments)]
fn envelope_controls(
    ui: &mut egui::Ui,
    setter: &ParamSetter,
    attack: &impl Param,
    attack_curve: &impl Param,
    decay: &impl Param,
    decay_curve: &impl Param,
    sustain: &impl Param,
    sustain_curve: &impl Param,
    release: &impl Param,
    release_curve: &impl Param,
) {
    wrapped(ui, |ui| {
        control(ui, setter, attack, "Attack");
        control(ui, setter, attack_curve, "Attack Curve");
        control(ui, setter, decay, "Decay");
        control(ui, setter, decay_curve, "Decay Curve");
        control(ui, setter, sustain, "Sustain");
        control(ui, setter, sustain_curve, "Sustain Curve");
        control(ui, setter, release, "Release");
        control(ui, setter, release_curve, "Release Curve");
    });
}

fn sync_row(
    ui: &mut egui::Ui,
    setter: &ParamSetter,
    sync: &impl Param,
    link_b: &impl Param,
    link_c: &impl Param,
    link_d: &impl Param,
) {
    ui.add_space(4.0);
    wrapped(ui, |ui| {
        control(ui, setter, sync, "Sync");
        control(ui, setter, link_b, "B");
        control(ui, setter, link_c, "C");
        control(ui, setter, link_d, "D");
    });
}

/// 選択肢はボタン、数値の幅が広いものは丸いノブにする。
fn control(ui: &mut egui::Ui, setter: &ParamSetter, param: &impl Param, label: &str) {
    match param.step_count() {
        Some(steps) if steps <= 6 => choice(ui, setter, param, label),
        _ => knob(ui, setter, param, label),
    }
}

fn knob(ui: &mut egui::Ui, setter: &ParamSetter, param: &impl Param, label: &str) {
    ui.vertical(|ui| {
        ui.set_width(72.0);
        let (rect, response) = ui.allocate_exact_size(Vec2::splat(34.0), Sense::click_and_drag());
        let id = response.id;
        if response.double_clicked() {
            setter.begin_set_parameter(param);
            setter.set_parameter_normalized(param, param.default_normalized_value());
            setter.end_set_parameter(param);
        } else if response.drag_started() {
            setter.begin_set_parameter(param);
            let start = param.unmodulated_normalized_value();
            ui.ctx().data_mut(|data| data.insert_temp(id, start));
        }
        if response.dragged() {
            let speed = if ui.input(|input| input.modifiers.shift) {
                0.0004
            } else {
                0.004
            };
            let start = ui
                .ctx()
                .data(|data| data.get_temp::<f32>(id))
                .unwrap_or_else(|| param.unmodulated_normalized_value());
            let mut next = (start - response.drag_delta().y * speed).clamp(0.0, 1.0);
            if let Some(steps) = param.step_count().filter(|steps| *steps > 0) {
                next = (next * steps as f32).round() / steps as f32;
            }
            ui.ctx().data_mut(|data| data.insert_temp(id, next));
            setter.set_parameter_normalized(param, next);
        }
        if response.drag_stopped() {
            setter.end_set_parameter(param);
        }
        paint_knob(ui, rect.center(), param.modulated_normalized_value());
        ui.label(
            RichText::new(label)
                .font(FontId::proportional(11.0))
                .color(INK),
        );
        ui.label(
            RichText::new(
                param.normalized_value_to_string(param.modulated_normalized_value(), true),
            )
            .font(FontId::proportional(10.0))
            .color(ON),
        );
    });
}

fn paint_knob(ui: &egui::Ui, center: egui::Pos2, normalized: f32) {
    let painter = ui.painter();
    let radius = 15.0;
    painter.circle_stroke(center, radius, Stroke::new(2.0, LINE));
    let start = -135.0_f32.to_radians();
    let sweep = 270.0_f32.to_radians() * normalized.clamp(0.0, 1.0);
    let mut points = Vec::with_capacity(25);
    for step in 0..=24 {
        let angle = start + sweep * (step as f32 / 24.0);
        points.push(center + vec2(angle.sin(), -angle.cos()) * radius);
    }
    if points.len() >= 2 {
        painter.add(egui::Shape::line(points, Stroke::new(2.5, ON)));
    }
    let tip = start + sweep;
    painter.circle_filled(
        center + vec2(tip.sin(), -tip.cos()) * (radius - 4.0),
        2.2,
        INK,
    );
}

fn choice(ui: &mut egui::Ui, setter: &ParamSetter, param: &impl Param, label: &str) {
    ui.vertical(|ui| {
        ui.set_min_width(54.0);
        let value = param.normalized_value_to_string(param.unmodulated_normalized_value(), false);
        let on = param.step_count() == Some(1) && param.unmodulated_normalized_value() > 0.5;
        let fill = if on { ON } else { NAVY };
        let text = if on { NAVY } else { INK };
        let clicked = ui
            .add(
                egui::Button::new(RichText::new(value).color(text).size(11.0))
                    .fill(fill)
                    .stroke(Stroke::new(1.0, if on { ON } else { LINE }))
                    .corner_radius(6.0)
                    .min_size(vec2(54.0, 24.0)),
            )
            .clicked();
        if clicked {
            step(setter, param, 1);
        }
        ui.label(
            RichText::new(label)
                .font(FontId::proportional(11.0))
                .color(INK),
        );
    });
}

fn step(setter: &ParamSetter, param: &impl Param, direction: i32) {
    let Some(steps) = param.step_count() else {
        return;
    };
    let current = (param.unmodulated_normalized_value() * steps as f32).round() as i32;
    let next = (current + direction).rem_euclid(steps as i32 + 1);
    setter.begin_set_parameter(param);
    setter.set_parameter_normalized(param, next as f32 / steps as f32);
    setter.end_set_parameter(param);
}
