//! Shared NyashkTune widgets: palette, knob (drag / shift-fine / wheel / double-click reset / click-to-type),
//! pill buttons, section titles.
#![allow(dead_code)]
use crate::params::{ModDst, ModSrc, NoiseType, SynthParams, N_SRC};
use nih_plug::params::internals::ParamPtr;
use nih_plug::prelude::*;
use std::cell::{Cell, RefCell};
use std::sync::Arc;
use nih_plug_egui::egui::{self, epaint::PathStroke, text::LayoutJob, Align2, Color32, CornerRadius, FontId, Pos2, Rect, Sense, Stroke, StrokeKind, TextFormat, Ui, UiBuilder, Vec2};

pub const BG: Color32 = Color32::from_rgb(0x0a, 0x0a, 0x0a);
pub const PANEL: Color32 = Color32::from_rgb(0x11, 0x11, 0x11);
pub const PANEL2: Color32 = Color32::from_rgb(0x1c, 0x1c, 0x1c);
pub const LINE: Color32 = Color32::from_rgb(0x2e, 0x2e, 0x2e);
pub const WHITE: Color32 = Color32::from_rgb(0xf2, 0xf2, 0xf2);
/// signature tone: brushed silver for values, active states, curves
pub const ACCENT: Color32 = Color32::from_rgb(0xe4, 0xe4, 0xe4);
/// modulation tone: pure bright white (rings, live dots, source chips)
pub const MODC: Color32 = Color32::from_rgb(0xff, 0xff, 0xff);
pub fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    let l = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t.clamp(0.0, 1.0)).round() as u8;
    Color32::from_rgb(l(a.r(), b.r()), l(a.g(), b.g()), l(a.b(), b.b()))
}
pub const DIM: Color32 = Color32::from_rgb(0x80, 0x80, 0x80);
pub const MID: Color32 = Color32::from_rgb(0x7a, 0x7a, 0x7a);

// ---------------------------------------------------------------- modulation hooks
//
// Knobs that can be modulated announce their destination with `mdst()` right before they are
// drawn. The knob then shows the modulation range, accepts drops of a dragged source and lets
// the user change the amount by dragging the outer ring.

pub struct ModCtx {
    pub params: Option<Arc<SynthParams>>,
    pub rects: Vec<(Rect, usize)>,
    /// Source (ModSrc index) currently being dragged from the source strip.
    pub drag: Option<usize>,
    pub drag_label: String,
    /// where the dragged source was last seen (it is dropped there when the button goes up)
    pub drag_pos: Option<Pos2>,
    /// the complete knob list of the previous frame and the one knob under the pointer (the only one that lights up)
    pub prev_rects: Vec<(Rect, usize)>,
    pub hover_dst: Option<usize>,
    /// live value of every modulation source (for the animated dots on the knobs)
    pub live: [f32; N_SRC],
    /// MIDI learn state (set once per frame by the editor)
    pub learn_on: bool,
    pub learn_target: Option<ParamPtr>,
    pub learn_click: Option<ParamPtr>,
    pub learn_unbind: Option<ParamPtr>,
    pub bound: Vec<(ParamPtr, u8)>,
}

thread_local! {
    static MC: RefCell<ModCtx> = RefCell::new(ModCtx { params: None, rects: Vec::new(), drag: None, drag_label: String::new(), drag_pos: None, prev_rects: Vec::new(), hover_dst: None, live: [0.0; N_SRC], learn_on: false, learn_target: None, learn_click: None, learn_unbind: None, bound: Vec::new() });
    static NEXT_DST: Cell<usize> = Cell::new(0);
    static NEXT_DESC: Cell<Option<&'static crate::fxdesc::D>> = Cell::new(None);
    static FLASH: Cell<(usize, f64)> = Cell::new((0, 0.0));
}

pub fn begin_frame(p: &Arc<SynthParams>, live: [f32; N_SRC]) {
    MC.with(|m| {
        let mut m = m.borrow_mut();
        m.live = live;
        m.params = Some(p.clone());
        let done = std::mem::take(&mut m.rects);
        m.prev_rects = done;
    });
}

/// The knob that a drop at `pos` would land on: of all knobs whose hit box contains the point, the nearest one.
pub fn best_target(rects: &[(Rect, usize)], pos: Pos2) -> Option<usize> {
    rects
        .iter()
        .filter(|(r, _)| r.contains(pos))
        .min_by(|(a, _), (b, _)| a.center().distance(pos).partial_cmp(&b.center().distance(pos)).unwrap_or(std::cmp::Ordering::Equal))
        .map(|(_, d)| *d)
}

/// Runs after the whole UI of a frame has been drawn (so the knob list is complete): follows the dragged
/// source, draws its ghost, and returns `(source, destination)` when it is dropped on a knob.
/// A drag can never get stuck: it ends when no button is down, a new press starts, or Escape is hit.
pub fn end_frame(ctx: &egui::Context) -> Option<(usize, usize)> {
    let (down, pressed, esc, pos) = ctx.input(|i| (i.pointer.any_down(), i.pointer.any_pressed(), i.key_pressed(egui::Key::Escape), i.pointer.latest_pos()));
    MC.with(|m| {
        let mut m = m.borrow_mut();
        // which knob lights up next frame
        m.hover_dst = if m.drag.is_some() { pos.and_then(|pp| best_target(&m.rects, pp)) } else { None };
        let Some(src) = m.drag else { return None };
        if let Some(pp) = pos {
            if down {
                m.drag_pos = Some(pp);
            }
        }
        if down && !esc {
            if let Some(pp) = pos {
                let lp = ctx.layer_painter(egui::LayerId::new(egui::Order::Tooltip, egui::Id::new("mod_ghost")));
                let g = Rect::from_center_size(pp + Vec2::new(14.0, 14.0), Vec2::new(46.0, 22.0));
                lp.rect_filled(g, CornerRadius::same(6), WHITE);
                lp.text(g.center(), Align2::CENTER_CENTER, &m.drag_label, mono(10.0), Color32::BLACK);
            }
            ctx.request_repaint();
            return None;
        }
        // finished (released, cancelled, or the button state got lost)
        let at = m.drag_pos.take();
        m.drag = None;
        m.hover_dst = None;
        if esc || (pressed && down) {
            return None;
        }
        at.and_then(|pp| best_target(&m.rects, pp)).map(|d| (src, d))
    })
}

/// Tell the knobs whether MIDI learn is active, which one is waiting for a controller and which are bound.
pub fn set_learn(on: bool, target: Option<ParamPtr>, bound: Vec<(ParamPtr, u8)>) {
    MC.with(|m| {
        let mut m = m.borrow_mut();
        m.learn_on = on;
        m.learn_target = target;
        m.bound = bound;
    });
}

/// (knob clicked in learn mode, knob right-clicked in learn mode = unbind)
pub fn take_learn_events() -> (Option<ParamPtr>, Option<ParamPtr>) {
    MC.with(|m| {
        let mut m = m.borrow_mut();
        (m.learn_click.take(), m.learn_unbind.take())
    })
}

/// Announce the modulation destination of the next knob.
/// The next knob shows / parses its value with this effect descriptor (rack knobs are plain 0..1 parameters).
pub fn use_desc(d: &'static crate::fxdesc::D) {
    NEXT_DESC.with(|c| c.set(Some(d)));
}

pub fn mdst(d: ModDst) {
    NEXT_DST.with(|c| c.set(d as usize));
}

/// Highlight the knob of a destination with a pulsing ring until `until` (egui time).
pub fn flash(dst: usize, until: f64) {
    FLASH.with(|f| f.set((dst, until)));
}

pub fn set_drag(s: Option<usize>, label: &str) {
    MC.with(|m| {
        let mut m = m.borrow_mut();
        m.drag = s;
        m.drag_label = label.to_string();
        if s.is_none() {
            m.drag_pos = None;
        }
    });
}
pub fn drag_src() -> Option<usize> {
    MC.with(|m| m.borrow().drag)
}
pub fn drop_targets() -> Vec<(Rect, usize)> {
    MC.with(|m| m.borrow().rects.clone())
}

/// How far a modulation amount of 1.0 moves the knob (in knob-travel units).
pub fn dst_scale(d: usize) -> f32 {
    match ModDst::from_index(d.min(ModDst::variants().len() - 1)) {
        ModDst::Cutoff | ModDst::Cutoff2 => 0.6,
        ModDst::Pan | ModDst::APan | ModDst::BPan | ModDst::CPan => 0.5,
        ModDst::APitch | ModDst::BPitch | ModDst::CPitch => 1.0,
        ModDst::NoisePitch => 0.25,
        _ => 1.0,
    }
}

/// Live (animated) modulation of a destination in knob-travel units.
fn live_mod(dst: usize) -> f32 {
    MC.with(|m| {
        let m = m.borrow();
        let Some(p) = m.params.as_ref() else { return 0.0 };
        let mut sum = 0.0;
        for s in p.slot.iter() {
            if s.dst.value() as usize == dst && s.src.value() != ModSrc::None && s.on.value() {
                let sl = crate::engine::SlotP { src: s.src.value(), dst: s.dst.value(), amount: s.amount.value(), aux: s.aux.value(), curve: s.curve.value(), uni: s.uni.value() };
                sum += crate::engine::slot_value(&sl, |x| m.live[x as usize]);
            }
        }
        sum * dst_scale(dst)
    })
}

/// Number of matrix slots that use a source (for the chip badges).
pub fn source_uses(src: ModSrc) -> usize {
    MC.with(|m| {
        let m = m.borrow();
        let Some(p) = m.params.as_ref() else { return 0 };
        p.slot.iter().filter(|s| s.src.value() == src && s.dst.value() != ModDst::None && s.on.value()).count()
    })
}

fn mods_for(dst: usize) -> Vec<(usize, usize, f32)> {
    MC.with(|m| {
        let m = m.borrow();
        let Some(p) = m.params.as_ref() else { return Vec::new() };
        let mut v = Vec::new();
        for (i, s) in p.slot.iter().enumerate() {
            if s.dst.value() as usize == dst && s.src.value() != ModSrc::None && s.on.value() {
                v.push((i, s.src.value() as usize, s.amount.value()));
            }
        }
        v
    })
}

/// Bypassed (switched off) slots that still point at a destination.
fn bypassed_for(dst: usize) -> Vec<(usize, usize)> {
    MC.with(|m| {
        let m = m.borrow();
        let Some(p) = m.params.as_ref() else { return Vec::new() };
        p.slot.iter().enumerate().filter(|(_, s)| s.dst.value() as usize == dst && s.src.value() != ModSrc::None && !s.on.value()).map(|(i, s)| (i, s.src.value() as usize)).collect()
    })
}

pub fn mono(size: f32) -> FontId {
    FontId::monospace(size)
}

pub fn set_norm<P: Param>(setter: &ParamSetter, p: &P, v: f32) {
    setter.begin_set_parameter(p);
    setter.set_parameter_normalized(p, v.clamp(0.0, 1.0));
    setter.end_set_parameter(p);
}
pub fn set_f(setter: &ParamSetter, p: &FloatParam, v: f32) {
    setter.begin_set_parameter(p);
    setter.set_parameter(p, v);
    setter.end_set_parameter(p);
}
pub fn set_b(setter: &ParamSetter, p: &BoolParam, v: bool) {
    setter.begin_set_parameter(p);
    setter.set_parameter(p, v);
    setter.end_set_parameter(p);
}
pub fn set_i(setter: &ParamSetter, p: &IntParam, v: i32) {
    setter.begin_set_parameter(p);
    setter.set_parameter(p, v);
    setter.end_set_parameter(p);
}

pub fn set_e<E: Enum + PartialEq>(setter: &ParamSetter, p: &EnumParam<E>, v: E) {
    setter.begin_set_parameter(p);
    setter.set_parameter(p, v);
    setter.end_set_parameter(p);
}
pub fn reset<P: Param>(setter: &ParamSetter, p: &P) {
    set_norm(setter, p, p.default_normalized_value());
}

#[derive(Clone, Copy, Default)]
pub struct KnobEv {
    pub delta: f32,
    pub started: bool,
    pub total: Option<f32>,
}

pub fn knob<P: Param>(ui: &mut Ui, setter: &ParamSetter, p: &P, label: &str, cell: Rect, tooltip: &str, bipolar: bool) -> KnobEv {
    knob_r(ui, setter, p, label, cell, tooltip, bipolar, 30.0)
}

#[allow(clippy::too_many_arguments)]
pub fn knob_r<P: Param>(ui: &mut Ui, setter: &ParamSetter, p: &P, label: &str, cell: Rect, tooltip: &str, bipolar: bool, r: f32) -> KnobEv {
    knob_impl(ui, setter, p, label, cell, tooltip, bipolar, r, false)
}

/// Compact knob for dense panels (radius ~13, small value box).
pub fn knob_s<P: Param>(ui: &mut Ui, setter: &ParamSetter, p: &P, label: &str, cell: Rect, tooltip: &str, bipolar: bool) -> KnobEv {
    knob_impl(ui, setter, p, label, cell, tooltip, bipolar, 13.0, true)
}

/// MIDI-learn support for widgets that are not knobs (the OCT / SEMI spinners): in learn mode a click picks the
/// parameter, a right-click unbinds it, and a ring + CC tag is drawn. Returns true while learn mode is on
/// (the caller then leaves the value alone).
pub fn learn_widget(ui: &Ui, ptr: ParamPtr, resp: &egui::Response, r: Rect) -> bool {
    let (learn_on, is_target, bound_cc) = MC.with(|m| {
        let m = m.borrow();
        (m.learn_on, m.learn_target == Some(ptr), m.bound.iter().find(|(q, _)| *q == ptr).map(|(_, c)| *c))
    });
    if learn_on {
        if resp.clicked() {
            MC.with(|m| m.borrow_mut().learn_click = Some(ptr));
        }
        if resp.secondary_clicked() {
            MC.with(|m| m.borrow_mut().learn_unbind = Some(ptr));
        }
        let t = ui.input(|i| i.time);
        let (w, col) = if is_target { (2.5_f32, WHITE) } else { (1.0_f32, Color32::from_rgb(90, 90, 90)) };
        let ph = if is_target { ((t * 5.0).sin() * 0.5 + 0.5) as f32 * 2.0 } else { 0.0 };
        ui.painter().rect_stroke(r.expand(2.0 + ph), CornerRadius::same(5), Stroke::new(w, col), StrokeKind::Outside);
        if is_target {
            ui.ctx().request_repaint();
        }
    }
    if let Some(cc) = bound_cc {
        let tag = Rect::from_center_size(Pos2::new(r.center().x, r.top() - 1.0), Vec2::new(34.0, 11.0));
        ui.painter().rect_filled(tag, CornerRadius::same(3), WHITE);
        ui.painter().text(tag.center(), Align2::CENTER_CENTER, format!("CC{cc}"), mono(8.0), Color32::BLACK);
    }
    learn_on
}

#[allow(clippy::too_many_arguments)]
fn knob_impl<P: Param>(ui: &mut Ui, setter: &ParamSetter, p: &P, label: &str, cell: Rect, tooltip: &str, bipolar: bool, r: f32, compact: bool) -> KnobEv {
    let c = Pos2::new(cell.center().x, cell.top() + r + 3.0);
    let hit = Rect::from_center_size(c, Vec2::splat(2.0 * r + 8.0));
    let dst = NEXT_DST.with(|d| d.replace(0));
    let desc = NEXT_DESC.with(|d| d.take());
    let mods = if dst != 0 { mods_for(dst) } else { Vec::new() };
    if dst != 0 {
        MC.with(|m| m.borrow_mut().rects.push((hit, dst)));
    }
    let resp = ui.allocate_rect(hit, Sense::click_and_drag());
    let ptr = p.as_ptr();
    let (learn_on, is_target, bound_cc) = MC.with(|m| {
        let m = m.borrow();
        (m.learn_on, m.learn_target == Some(ptr), m.bound.iter().find(|(q, _)| *q == ptr).map(|(_, c)| *c))
    });
    if learn_on {
        if resp.clicked() {
            MC.with(|m| m.borrow_mut().learn_click = Some(ptr));
        }
        if resp.secondary_clicked() {
            MC.with(|m| m.borrow_mut().learn_unbind = Some(ptr));
        }
    }
    let tip_full = if mods.is_empty() {
        tooltip.to_string()
    } else {
        let mut t = tooltip.to_string();
        t.push_str("\nModulated by:");
        for (_, s, a) in &mods {
            t.push_str(&format!("\n  {}  {:+.0}%", ModSrc::variants()[*s], a * 100.0));
        }
        t.push_str("\nDrag a ring (one per source) = its amount, right-click = list of sources (amount, bipolar, remove)");
        t
    };
    let resp = if tip_full.is_empty() { resp } else { resp.on_hover_text(tip_full) };

    let norm = p.unmodulated_normalized_value();
    let mut delta = 0.0f32;
    let mut ev = KnobEv::default();
    let raw_id = egui::Id::new(("knob_raw", resp.id));
    // ---- modulation: right-click removes, ring / Ctrl drag edits the amount
    let medit_id = egui::Id::new(("knob_medit", resp.id));
    let byp = if dst != 0 { bypassed_for(dst) } else { Vec::new() };
    if !learn_on {
        if let Some(pp) = MC.with(|m| m.borrow().params.clone()) {
            resp.context_menu(|ui| {
                ui.set_min_width(210.0);
                ui.label(egui::RichText::new(format!("{}  {label}", if mods.is_empty() && byp.is_empty() { "PARAMETER" } else { "MODULATION" })).monospace().size(10.0).color(DIM));
                ui.separator();
                // Lock parameter: presets leave this control alone
                if let Some(id) = pp.param_map().into_iter().find(|(_, q, _)| *q == ptr).map(|(id, _, _)| id) {
                    let is_locked = pp.locked.lock().map(|g| g.split(';').any(|s| s == id)).unwrap_or(false);
                    if ui.selectable_label(is_locked, "Lock parameter").on_hover_text("Loading a preset will not change this control").clicked() {
                        if let Ok(mut g) = pp.locked.lock() {
                            let mut v: Vec<String> = g.split(';').filter(|s| !s.is_empty() && *s != id).map(String::from).collect();
                            if !is_locked {
                                v.push(id.clone());
                            }
                            *g = v.join(";");
                        }
                        ui.close_menu();
                    }
                }
                for (i, sidx, a) in &mods {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new(format!("{:<7}", ModSrc::variants()[*sidx])).monospace().size(11.0));
                        let mut v = *a;
                        if ui.add(egui::Slider::new(&mut v, -1.0..=1.0).show_value(true).fixed_decimals(2)).changed() {
                            set_f(setter, &pp.slot[*i].amount, v);
                        }
                        let bi = pp.slot[*i].uni.value();
                        if ui.selectable_label(!bi, "BI").on_hover_text("Bipolar: the source moves the knob both ways. Off = unipolar (only up).").clicked() {
                            set_b(setter, &pp.slot[*i].uni, !bi);
                        }
                        if ui.button("off").on_hover_text("Bypass this source (keeps the routing)").clicked() {
                            set_b(setter, &pp.slot[*i].on, false);
                            ui.close_menu();
                        }
                        if ui.button("x").on_hover_text("Remove this source").clicked() {
                            set_e(setter, &pp.slot[*i].dst, ModDst::None);
                            set_e(setter, &pp.slot[*i].src, ModSrc::None);
                            set_f(setter, &pp.slot[*i].amount, 0.0);
                            ui.close_menu();
                        }
                    });
                }
                for (i, sidx) in &byp {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new(format!("{:<7}", ModSrc::variants()[*sidx])).monospace().size(11.0).color(DIM));
                        ui.label(egui::RichText::new("bypassed").size(10.0).color(DIM));
                        if ui.button("on").on_hover_text("Enable this source again").clicked() {
                            set_b(setter, &pp.slot[*i].on, true);
                            ui.close_menu();
                        }
                    });
                }
                ui.separator();
                if ui.button("Reset control").on_hover_text("Back to the default value (same as double-click)").clicked() {
                    let dn = desc.map_or_else(|| p.default_normalized_value(), |d| d.def_norm());
                    set_norm(setter, p, dn);
                    ui.close_menu();
                }
                if ui.button("Remove all").clicked() {
                    for (i, _, _) in &mods {
                        set_e(setter, &pp.slot[*i].dst, ModDst::None);
                        set_e(setter, &pp.slot[*i].src, ModSrc::None);
                        set_f(setter, &pp.slot[*i].amount, 0.0);
                    }
                    ui.close_menu();
                }
            });
        }
    }
    let mut mod_editing = false;
    if !mods.is_empty() {
        if resp.drag_started() {
            let dist = resp.interact_pointer_pos().map(|pp| ((pp.x - c.x).powi(2) + (pp.y - c.y).powi(2)).sqrt()).unwrap_or(0.0);
            if dist > r * 0.82 || ui.input(|i| i.modifiers.ctrl) {
                // each source has its own ring: pick the one under the pointer (Ctrl = the first one)
                let base = r - 2.5 + if compact { 3.6 } else { 4.6 };
                let step = if compact { 2.8 } else { 3.4 };
                let pick = (((dist - base) / step).round().max(0.0) as usize).min(mods.len().min(4) - 1);
                ui.data_mut(|d| d.insert_temp(medit_id, pick + 1));
            }
        }
        let on = ui.data(|d| d.get_temp::<usize>(medit_id)).unwrap_or(0);
        if on > 0 && resp.dragged() {
            mod_editing = true;
            if let Some(pp) = MC.with(|m| m.borrow().params.clone()) {
                let (i, _, a) = mods[(on - 1).min(mods.len() - 1)];
                let fine = ui.input(|i| i.modifiers.shift);
                let nv = (a - resp.drag_delta().y / if fine { 1500.0 } else { 220.0 }).clamp(-1.0, 1.0);
                set_f(setter, &pp.slot[i].amount, nv);
            }
        }
        if resp.drag_stopped() {
            ui.data_mut(|d| d.remove::<usize>(medit_id));
        }
        if on > 0 {
            mod_editing = true;
        }
    }
    if resp.drag_started() && !mod_editing {
        setter.begin_set_parameter(p);
        ui.data_mut(|d| d.insert_temp(raw_id, (norm, norm)));
        ev.started = true;
    }
    if resp.dragged() && !mod_editing {
        let fine = ui.input(|i| i.modifiers.shift);
        let d = -resp.drag_delta().y / if fine { 1200.0 } else { 220.0 };
        let (raw0, raw) = ui.data(|dd| dd.get_temp::<(f32, f32)>(raw_id)).unwrap_or((norm, norm));
        let raw = raw + d;
        ui.data_mut(|dd| dd.insert_temp(raw_id, (raw0, raw)));
        let new = raw.clamp(0.0, 1.0);
        delta = new - norm;
        setter.set_parameter_normalized(p, new);
        ev.total = Some(raw - raw0);
    }
    if resp.drag_stopped() && !mod_editing {
        setter.end_set_parameter(p);
    }
    if resp.double_clicked() {
        let dn = desc.map_or_else(|| p.default_normalized_value(), |d| d.def_norm());
        delta = dn - norm;
        set_norm(setter, p, dn);
        ev.started = false;
    }
    if resp.hovered() {
        let sc = ui.input(|i| i.smooth_scroll_delta.y);
        if sc != 0.0 {
            let new = (norm + sc / 1500.0).clamp(0.0, 1.0);
            delta = new - norm;
            setter.begin_set_parameter(p);
            setter.set_parameter_normalized(p, new);
            setter.end_set_parameter(p);
        }
    }

    let norm = p.unmodulated_normalized_value();
    let painter = ui.painter();
    let a0 = std::f32::consts::PI * 0.75;
    let sweep = std::f32::consts::PI * 1.5;
    let ring = r - 2.5;
    let sw = if compact { 3.0_f32 } else { 4.0 };
    let pt = |rad: f32, t: f32| -> Pos2 {
        let a = a0 + t * sweep;
        Pos2::new(c.x + rad * a.cos(), c.y + rad * a.sin())
    };
    let arc = |from: f32, to: f32| -> Vec<Pos2> {
        let n = (((to - from).abs() / sweep) * 120.0).ceil().max(2.0) as usize;
        (0..=n).map(|i| pt(ring, from + (to - from) * i as f32 / n as f32)).collect()
    };
    let arc_r = |rad: f32, from: f32, to: f32| -> Vec<Pos2> {
        let n = (((to - from).abs() / sweep) * 120.0).ceil().max(2.0) as usize;
        (0..=n).map(|i| pt(rad, from + (to - from) * i as f32 / n as f32)).collect()
    };
    // track + value arc with round caps
    painter.add(egui::Shape::line(arc(0.0, 1.0), PathStroke::new(sw, LINE)));
    painter.circle_filled(pt(ring, 0.0), 2.0, LINE);
    painter.circle_filled(pt(ring, 1.0), 2.0, LINE);
    let from = if bipolar { 0.5 } else { 0.0 };
    if (norm - from).abs() > 0.002 {
        painter.add(egui::Shape::line(arc(from, norm), PathStroke::new(sw, ACCENT)));
        painter.circle_filled(pt(ring, from), 2.0, ACCENT);
        painter.circle_filled(pt(ring, norm), 2.0, ACCENT);
    }
    // modulation ranges: one concentric ring per source (so several sources never hide each other)
    for (k, (_, sidx, amt)) in mods.iter().take(4).enumerate() {
        let sc = dst_scale(dst);
        let lo = (norm - (amt.abs() * sc)).clamp(0.0, 1.0);
        let hi = (norm + (amt.abs() * sc)).clamp(0.0, 1.0);
        let (a, b) = if *amt >= 0.0 { (norm, hi) } else { (lo, norm) };
        let n = (((b - a).abs() / 1.0) * 90.0).ceil().max(2.0) as usize;
        let rr = ring + if compact { 3.6 } else { 4.6 } + k as f32 * if compact { 2.8 } else { 3.4 };
        let pts: Vec<Pos2> = (0..=n)
            .map(|i| {
                let t = a + (b - a) * i as f32 / n as f32;
                let ang = a0 + t * sweep;
                Pos2::new(c.x + rr * ang.cos(), c.y + rr * ang.sin())
            })
            .collect();
        let shade = [255u8, 200, 150, 110][k];
        // the track of this ring (faint) shows where the full range would be
        painter.add(egui::Shape::line(arc_r(rr, 0.0, 1.0), PathStroke::new(1.0_f32, Color32::from_rgb(34, 34, 34))));
        if (b - a).abs() > 0.002 {
            painter.add(egui::Shape::line(pts, PathStroke::new(if k == 0 { 2.2_f32 } else { 1.8 }, mix(Color32::from_rgb(34, 34, 34), MODC, shade as f32 / 255.0))));
        }
        let _ = sidx;
    }
    // animated dot: where the modulation currently pushes the value
    if !mods.is_empty() {
        let t = (norm + live_mod(dst)).clamp(0.0, 1.0);
        let rr = ring + if compact { 3.6 } else { 4.6 };
        let ang = a0 + t * sweep;
        let dp = Pos2::new(c.x + rr * ang.cos(), c.y + rr * ang.sin());
        painter.circle_filled(dp, 3.2, Color32::BLACK);
        painter.circle_filled(dp, 2.2, MODC);
    }
    // pulse a freshly routed destination so the user sees where the modulation landed
    if dst != 0 {
        let (fd, until) = FLASH.with(|f| f.get());
        let now = ui.input(|i| i.time);
        if fd == dst && now < until {
            let ph = ((now * 6.0).sin() * 0.5 + 0.5) as f32;
            painter.circle_stroke(c, r + 5.0 + ph * 3.0, Stroke::new(2.5_f32, WHITE));
            ui.ctx().request_repaint();
        }
    }
    // drop target highlight while a source is dragged
    if dst != 0 && drag_src().is_some() {
        let over = MC.with(|m| m.borrow().hover_dst) == Some(dst);
        painter.circle_stroke(c, r + 3.0, Stroke::new(if over { 2.0_f32 } else { 1.0 }, if over { WHITE } else { MID }));
    }
    // MIDI learn: hint ring on every knob, bright ring on the one waiting, tag on the bound ones
    if learn_on {
        let now = ui.input(|i| i.time);
        if is_target {
            let ph = ((now * 5.0).sin() * 0.5 + 0.5) as f32;
            painter.circle_stroke(c, r + 3.0 + ph * 2.0, Stroke::new(2.5_f32, WHITE));
            ui.ctx().request_repaint();
        } else {
            painter.circle_stroke(c, r + 3.0, Stroke::new(1.0_f32, Color32::from_rgb(90, 90, 90)));
        }
    }
    if let Some(cc) = bound_cc {
        let tag = Rect::from_center_size(Pos2::new(c.x, cell.top() + 2.0), Vec2::new(34.0, 11.0));
        painter.rect_filled(tag, CornerRadius::same(3), WHITE);
        painter.text(tag.center(), Align2::CENTER_CENTER, format!("CC{cc}"), mono(8.0), Color32::BLACK);
    }
    // round knob body
    let body = r * 0.72;
    painter.circle_filled(c + Vec2::new(0.0, 1.5), body + 1.0, Color32::from_rgb(5, 5, 5));
    painter.circle_filled(c, body, Color32::from_rgb(34, 34, 34));
    painter.circle_filled(c, body * 0.84, Color32::from_rgb(24, 24, 24));
    painter.circle_stroke(c, body, Stroke::new(1.0_f32, Color32::from_rgb(70, 70, 70)));
    // pointer
    painter.line_segment([pt(body * 0.30, norm), pt(body * 0.80, norm)], Stroke::new(3.0_f32, WHITE));
    painter.circle_filled(pt(body * 0.80, norm), 1.5, WHITE);
    painter.circle_filled(pt(body * 0.30, norm), 1.5, WHITE);

    // value + label (click the value box to type a number)
    let val = match desc {
        Some(d) => d.text(norm),
        None => p.normalized_value_to_string(norm, true),
    };
    let vr = if compact {
        Rect::from_center_size(Pos2::new(cell.center().x, c.y + r + 10.0), Vec2::new(cell.width().min(58.0), 14.0))
    } else {
        Rect::from_center_size(Pos2::new(cell.center().x, c.y + r + 14.0), Vec2::new(cell.width().min(84.0), 18.0))
    };
    let edit_id = egui::Id::new(("knob_edit", resp.id));
    let vfont = if compact { mono(10.0) } else { mono(11.0) };
    let editing: Option<String> = ui.data(|d| d.get_temp::<String>(edit_id));
    if compact {
        ui.painter().text(Pos2::new(cell.center().x, vr.bottom() + 7.0), Align2::CENTER_CENTER, label, mono(9.5), DIM);
    } else {
        painter_label(ui, cell, vr, label);
    }
    if let Some(mut buf) = editing {
        let te = egui::TextEdit::singleline(&mut buf)
            .font(vfont.clone())
            .horizontal_align(egui::Align::Center)
            .desired_width(vr.width())
            .margin(egui::Margin::symmetric(2, 1));
        let r_te = ui.put(vr, te);
        if !r_te.has_focus() && !r_te.lost_focus() {
            r_te.request_focus();
        }
        let enter = ui.input(|i| i.key_pressed(egui::Key::Enter));
        let esc = ui.input(|i| i.key_pressed(egui::Key::Escape));
        if esc {
            ui.data_mut(|d| d.remove::<String>(edit_id));
        } else if enter || r_te.lost_focus() {
            let txt = buf.trim().replace(',', ".");
            let parsed = match desc {
                Some(d) => d.parse(&txt).map(|v| d.norm(v)),
                None => p.string_to_normalized_value(&txt),
            };
            if let Some(nv) = parsed {
                let nv = nv.clamp(0.0, 1.0);
                delta = nv - norm;
                set_norm(setter, p, nv);
            }
            ui.data_mut(|d| d.remove::<String>(edit_id));
        } else {
            ui.data_mut(|d| d.insert_temp(edit_id, buf));
        }
    } else {
        let vresp = ui.allocate_rect(vr, Sense::click()).on_hover_text("Click to type a value");
        let painter = ui.painter();
        painter.rect_stroke(vr, CornerRadius::same(4), Stroke::new(1.0_f32, if vresp.hovered() { DIM } else { LINE }), StrokeKind::Inside);
        painter.text(vr.center(), Align2::CENTER_CENTER, &val, vfont, WHITE);
        if vresp.clicked() {
            let plain = val.trim().replace(" kHz", "k").split(' ').next().unwrap_or("").to_string();
            ui.data_mut(|d| d.insert_temp(edit_id, plain));
        }
    }
    ev.delta = delta;
    ev
}

pub fn painter_label(ui: &Ui, cell: Rect, vr: Rect, label: &str) {
    ui.painter().text(Pos2::new(cell.center().x, vr.bottom() + 9.0), Align2::CENTER_CENTER, label, mono(10.0), DIM);
}

pub fn pill(ui: &mut Ui, rect: Rect, text: &str, on: bool) -> bool {
    pill_tip(ui, rect, text, on, "")
}

pub fn pill_tip(ui: &mut Ui, rect: Rect, text: &str, on: bool, tip: &str) -> bool {
    let mut resp = ui.allocate_rect(rect, Sense::click());
    if !tip.is_empty() {
        resp = resp.on_hover_text(tip);
    }
    let id = egui::Id::new(("pill", rect.min.x as i32, rect.min.y as i32, text));
    let a = ui.ctx().animate_bool_with_time(id, on, 0.12);
    let h = ui.ctx().animate_bool_with_time(id.with("h"), resp.hovered(), 0.10);
    let painter = ui.painter();
    let fill = mix(mix(Color32::from_rgb(0x12, 0x12, 0x12), PANEL2, h), ACCENT, a);
    let edge = mix(mix(LINE, DIM, h), ACCENT, a);
    painter.rect_filled(rect, CornerRadius::same(6), fill);
    painter.rect_stroke(rect, CornerRadius::same(6), Stroke::new(1.0_f32, edge), StrokeKind::Inside);
    painter.text(rect.center(), Align2::CENTER_CENTER, text, mono(11.0), mix(WHITE, Color32::BLACK, a));
    resp.clicked()
}

#[derive(Clone, Copy, PartialEq)]
pub enum Icon { Save, Undo, Redo, Prev, Next, Midi, Trash }

/// Square icon button (Serum style): a drawn glyph instead of a text label.
pub fn icon_btn(ui: &mut Ui, rect: Rect, icon: Icon, on: bool, tip: &str) -> bool {
    let resp = ui.allocate_rect(rect, Sense::click()).on_hover_text(tip);
    let id = egui::Id::new(("icon", rect.min.x as i32, rect.min.y as i32));
    let a = ui.ctx().animate_bool_with_time(id, on, 0.12);
    let h = ui.ctx().animate_bool_with_time(id.with("h"), resp.hovered(), 0.10);
    let p = ui.painter();
    let fill = mix(mix(Color32::from_rgb(0x12, 0x12, 0x12), PANEL2, h), ACCENT, a);
    p.rect_filled(rect, CornerRadius::same(6), fill);
    p.rect_stroke(rect, CornerRadius::same(6), Stroke::new(1.0_f32, mix(mix(LINE, DIM, h), ACCENT, a)), StrokeKind::Inside);
    let col = mix(mix(Color32::from_rgb(0xb0, 0xb0, 0xb0), WHITE, h), Color32::BLACK, a);
    let c = rect.center();
    let s = Stroke::new(1.6_f32, col);
    let r = rect.height().min(rect.width()) * 0.27;
    let arc = |cx: f32, cy: f32, rad: f32, a0: f32, a1: f32| -> Vec<Pos2> {
        (0..=24).map(|i| { let t = a0 + (a1 - a0) * i as f32 / 24.0; Pos2::new(cx + rad * t.cos(), cy + rad * t.sin()) }).collect()
    };
    match icon {
        Icon::Prev | Icon::Next => {
            let d = if icon == Icon::Prev { -1.0 } else { 1.0 };
            p.add(egui::Shape::line(vec![Pos2::new(c.x - d * r * 0.5, c.y - r), Pos2::new(c.x + d * r * 0.5, c.y), Pos2::new(c.x - d * r * 0.5, c.y + r)], PathStroke::new(1.8_f32, col)));
        }
        Icon::Save => {
            // floppy disk
            let b = Rect::from_center_size(c, Vec2::splat(r * 2.1));
            p.rect_stroke(b, CornerRadius::same(2), s, StrokeKind::Inside);
            p.rect_filled(Rect::from_min_size(b.min + Vec2::new(r * 0.45, 0.0), Vec2::new(r * 1.2, r * 0.75)), CornerRadius::ZERO, col);
            p.rect_stroke(Rect::from_min_max(Pos2::new(b.left() + r * 0.4, b.center().y + r * 0.1), Pos2::new(b.right() - r * 0.4, b.bottom())), CornerRadius::ZERO, s, StrokeKind::Inside);
        }
        Icon::Undo | Icon::Redo => {
            // open circular arrow with a clean arrowhead (Redo = mirrored Undo)
            let m = if icon == Icon::Undo { 1.0_f32 } else { -1.0 };
            let rr = r * 1.05;
            let deg = std::f32::consts::PI / 180.0;
            let pt = |t: f32| Pos2::new(c.x + m * rr * (t * deg).cos(), c.y + rr * (t * deg).sin());
            let pts: Vec<Pos2> = (0..=20).map(|i| pt(-150.0 + 250.0 * i as f32 / 20.0)).collect();
            p.add(egui::Shape::line(pts, PathStroke::new(1.7_f32, col)));
            let tp = pt(-150.0);
            // direction of travel at the start, reversed (arrow points back along the arc)
            let d = Vec2::new(m * (-150.0_f32 * deg).sin(), -(-150.0_f32 * deg).cos()).normalized();
            let rot = |v: Vec2, a: f32| Vec2::new(v.x * a.cos() - v.y * a.sin(), v.x * a.sin() + v.y * a.cos());
            let h = r * 0.8;
            p.add(egui::Shape::line(vec![tp - rot(d, 0.6) * h, tp, tp - rot(d, -0.6) * h], PathStroke::new(1.7_f32, col)));
        }
        Icon::Midi => {
            // 5-pin DIN socket
            p.circle_stroke(c, r * 1.1, s);
            for k in 0..5 {
                let t = std::f32::consts::PI * (1.0 + 0.25 * k as f32);
                p.circle_filled(Pos2::new(c.x + r * 0.62 * t.cos(), c.y + r * 0.62 * t.sin() + r * 0.1), 1.3, col);
            }
            p.line_segment([Pos2::new(c.x - r * 0.3, c.y + r * 0.95), Pos2::new(c.x + r * 0.3, c.y + r * 0.95)], s);
        }
        Icon::Trash => {
            p.line_segment([Pos2::new(c.x - r, c.y - r * 0.7), Pos2::new(c.x + r, c.y - r * 0.7)], s);
            p.line_segment([Pos2::new(c.x - r * 0.35, c.y - r * 0.95), Pos2::new(c.x + r * 0.35, c.y - r * 0.95)], s);
            p.add(egui::Shape::line(vec![Pos2::new(c.x - r * 0.75, c.y - r * 0.7), Pos2::new(c.x - r * 0.6, c.y + r), Pos2::new(c.x + r * 0.6, c.y + r), Pos2::new(c.x + r * 0.75, c.y - r * 0.7)], PathStroke::new(1.6_f32, col)));
        }
    }
    resp.clicked()
}

/// Dress a widget like the dark value boxes under the knobs (near-black fill, thin line, no grey button look).
pub fn dark_field_style(ui: &mut Ui) {
    let v = ui.visuals_mut();
    let fill = Color32::from_rgb(5, 5, 5);
    for w in [&mut v.widgets.inactive, &mut v.widgets.hovered, &mut v.widgets.active, &mut v.widgets.open] {
        w.bg_fill = fill;
        w.weak_bg_fill = fill;
        w.corner_radius = CornerRadius::same(4);
        w.expansion = 0.0;
    }
    v.widgets.inactive.bg_stroke = Stroke::new(1.0_f32, LINE);
    v.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, DIM);
    v.widgets.active.bg_stroke = Stroke::new(1.0_f32, WHITE);
    v.widgets.open.bg_stroke = Stroke::new(1.0_f32, DIM);
    v.widgets.hovered.fg_stroke = Stroke::new(1.0_f32, WHITE);
}

pub fn enum_combo<E: Enum + PartialEq + 'static>(ui: &mut Ui, setter: &ParamSetter, p: &EnumParam<E>, id: &str, rect: Rect) {
    enum_combo_sz(ui, setter, p, id, rect, 12.0);
}

pub fn enum_combo_sz<E: Enum + PartialEq + 'static>(ui: &mut Ui, setter: &ParamSetter, p: &EnumParam<E>, id: &str, rect: Rect, fs: f32) {
    enum_combo_styled(ui, setter, p, id, rect, fs, false);
}

/// `dark`: look like the value boxes under the knobs.
pub fn enum_combo_styled<E: Enum + PartialEq + 'static>(ui: &mut Ui, setter: &ParamSetter, p: &EnumParam<E>, id: &str, rect: Rect, fs: f32, dark: bool) {
    let names = E::variants();
    let idx = p.value().to_index();
    ui.allocate_new_ui(UiBuilder::new().max_rect(rect), |ui| {
        if dark {
            dark_field_style(ui);
        }
        // the button fills the whole rect (same height and width as the pills and boxes next to it)
        ui.spacing_mut().interact_size.y = rect.height();
        egui::ComboBox::from_id_salt(id)
            .height(380.0)
            .width(rect.width())
            .truncate()
            .selected_text(egui::RichText::new(names[idx]).font(mono(fs)))
            .show_ui(ui, |ui| {
                for (i, n) in names.iter().enumerate() {
                    if ui.selectable_label(i == idx, egui::RichText::new(*n).font(mono(12.0))).clicked() {
                        set_e(setter, p, E::from_index(i));
                    }
                }
            });
    });
}

pub fn section_title(ui: &Ui, pos: Pos2, num: &str, title: &str) {
    let mut job = LayoutJob::default();
    job.append("\u{2116}", 0.0, TextFormat { font_id: FontId::proportional(11.0), color: DIM, ..Default::default() });
    job.append(&format!(" {num}  {title}"), 0.0, TextFormat { font_id: mono(10.5), color: DIM, ..Default::default() });
    let galley = ui.fonts(|f| f.layout_job(job));
    ui.painter().galley(pos, galley, DIM);
}

/// Noise-type picker: "None" first, then the generators, the sample file last.
pub fn noise_combo(ui: &mut Ui, setter: &ParamSetter, p: &EnumParam<NoiseType>, id: &str, rect: Rect) {
    let names = NoiseType::variants();
    let cur = p.value().to_index();
    let none_i = NoiseType::None.to_index();
    let order: Vec<usize> = std::iter::once(none_i).chain((0..names.len()).filter(|&i| i != none_i)).collect();
    ui.allocate_new_ui(UiBuilder::new().max_rect(rect), |ui| {
        ui.spacing_mut().interact_size.y = rect.height();
        egui::ComboBox::from_id_salt(id)
            .height(380.0)
            .width(rect.width())
            .truncate()
            .selected_text(egui::RichText::new(names[cur]).font(mono(12.0)))
            .show_ui(ui, |ui| {
                for &i in &order {
                    if ui.selectable_label(i == cur, egui::RichText::new(names[i]).font(mono(12.0))).clicked() {
                        set_e(setter, p, NoiseType::from_index(i));
                    }
                }
            });
    });
}
