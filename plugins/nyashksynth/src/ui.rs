//! Shared NyashkTune widgets: palette, knob (drag / shift-fine / wheel / double-click reset / click-to-type),
//! pill buttons, section titles.
#![allow(dead_code)]
use crate::params::{ModDst, ModSrc, SynthParams, N_SRC};
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
    static MC: RefCell<ModCtx> = RefCell::new(ModCtx { params: None, rects: Vec::new(), drag: None, live: [0.0; N_SRC], learn_on: false, learn_target: None, learn_click: None, learn_unbind: None, bound: Vec::new() });
    static NEXT_DST: Cell<usize> = Cell::new(0);
    static FLASH: Cell<(usize, f64)> = Cell::new((0, 0.0));
}

pub fn begin_frame(p: &Arc<SynthParams>, live: [f32; N_SRC]) {
    MC.with(|m| {
        let mut m = m.borrow_mut();
        m.live = live;
        m.params = Some(p.clone());
        m.rects.clear();
    });
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
pub fn mdst(d: ModDst) {
    NEXT_DST.with(|c| c.set(d as usize));
}

/// Highlight the knob of a destination with a pulsing ring until `until` (egui time).
pub fn flash(dst: usize, until: f64) {
    FLASH.with(|f| f.set((dst, until)));
}

pub fn set_drag(s: Option<usize>) {
    MC.with(|m| m.borrow_mut().drag = s);
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
        t.push_str("\nDrag the outer ring (or Ctrl+drag) = amount, right-click = remove");
        t
    };
    let resp = if tip_full.is_empty() { resp } else { resp.on_hover_text(tip_full) };

    let norm = p.unmodulated_normalized_value();
    let mut delta = 0.0f32;
    let mut ev = KnobEv::default();
    let raw_id = egui::Id::new(("knob_raw", resp.id));
    // ---- modulation: right-click removes, ring / Ctrl drag edits the amount
    let medit_id = egui::Id::new(("knob_medit", resp.id));
    if dst != 0 && !learn_on && resp.secondary_clicked() && !mods.is_empty() {
        if let Some(pp) = MC.with(|m| m.borrow().params.clone()) {
            for (i, _, _) in &mods {
                set_e(setter, &pp.slot[*i].dst, ModDst::None);
                set_e(setter, &pp.slot[*i].src, ModSrc::None);
                set_f(setter, &pp.slot[*i].amount, 0.0);
            }
        }
    }
    let mut mod_editing = false;
    if !mods.is_empty() {
        if resp.drag_started() {
            let on_ring = resp.interact_pointer_pos().map(|pp| {
                let d = ((pp.x - c.x).powi(2) + (pp.y - c.y).powi(2)).sqrt();
                d > r * 0.82
            }).unwrap_or(false);
            if on_ring || ui.input(|i| i.modifiers.ctrl) {
                ui.data_mut(|d| d.insert_temp(medit_id, true));
            }
        }
        let on = ui.data(|d| d.get_temp::<bool>(medit_id)).unwrap_or(false);
        if on && resp.dragged() {
            mod_editing = true;
            if let Some(pp) = MC.with(|m| m.borrow().params.clone()) {
                let (i, _, a) = mods[0];
                let fine = ui.input(|i| i.modifiers.shift);
                let nv = (a - resp.drag_delta().y / if fine { 1500.0 } else { 220.0 }).clamp(-1.0, 1.0);
                set_f(setter, &pp.slot[i].amount, nv);
            }
        }
        if resp.drag_stopped() {
            ui.data_mut(|d| d.remove::<bool>(medit_id));
        }
        if on {
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
        delta = p.default_normalized_value() - norm;
        set_norm(setter, p, p.default_normalized_value());
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
    // track + value arc with round caps
    painter.add(egui::Shape::line(arc(0.0, 1.0), PathStroke::new(sw, LINE)));
    painter.circle_filled(pt(ring, 0.0), 2.0, LINE);
    painter.circle_filled(pt(ring, 1.0), 2.0, LINE);
    let from = if bipolar { 0.5 } else { 0.0 };
    if (norm - from).abs() > 0.002 {
        painter.add(egui::Shape::line(arc(from, norm), PathStroke::new(sw, WHITE)));
        painter.circle_filled(pt(ring, from), 2.0, WHITE);
        painter.circle_filled(pt(ring, norm), 2.0, WHITE);
    }
    // modulation range(s) on an outer ring
    for (_, _, amt) in &mods {
        let sc = dst_scale(dst);
        let lo = (norm - (amt.abs() * sc)).clamp(0.0, 1.0);
        let hi = (norm + (amt.abs() * sc)).clamp(0.0, 1.0);
        let (a, b) = if *amt >= 0.0 { (norm, hi) } else { (lo, norm) };
        let n = (((b - a).abs() / 1.0) * 90.0).ceil().max(2.0) as usize;
        let rr = ring + if compact { 3.6 } else { 4.6 };
        let pts: Vec<Pos2> = (0..=n)
            .map(|i| {
                let t = a + (b - a) * i as f32 / n as f32;
                let ang = a0 + t * sweep;
                Pos2::new(c.x + rr * ang.cos(), c.y + rr * ang.sin())
            })
            .collect();
        if (b - a).abs() > 0.002 {
            painter.add(egui::Shape::line(pts, PathStroke::new(2.0_f32, Color32::from_rgb(0xff, 0xff, 0xff))));
        }
    }
    // animated dot: where the modulation currently pushes the value
    if !mods.is_empty() {
        let t = (norm + live_mod(dst)).clamp(0.0, 1.0);
        let rr = ring + if compact { 3.6 } else { 4.6 };
        let ang = a0 + t * sweep;
        let dp = Pos2::new(c.x + rr * ang.cos(), c.y + rr * ang.sin());
        painter.circle_filled(dp, 3.2, Color32::BLACK);
        painter.circle_filled(dp, 2.2, WHITE);
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
        let over = ui.input(|i| i.pointer.hover_pos()).map(|pp| hit.contains(pp)).unwrap_or(false);
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
    let val = p.normalized_value_to_string(norm, true);
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
            if let Some(nv) = p.string_to_normalized_value(&txt) {
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
            let plain = val.trim().split(' ').next().unwrap_or("").to_string();
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
    let painter = ui.painter();
    let hov = resp.hovered();
    painter.rect_filled(rect, CornerRadius::same(6), if on { WHITE } else if hov { PANEL2 } else { Color32::TRANSPARENT });
    painter.rect_stroke(rect, CornerRadius::same(6), Stroke::new(1.0_f32, if on { WHITE } else if hov { DIM } else { LINE }), StrokeKind::Inside);
    painter.text(rect.center(), Align2::CENTER_CENTER, text, mono(11.0), if on { Color32::BLACK } else { WHITE });
    resp.clicked()
}

pub fn enum_combo<E: Enum + PartialEq + 'static>(ui: &mut Ui, setter: &ParamSetter, p: &EnumParam<E>, id: &str, rect: Rect) {
    let names = E::variants();
    let idx = p.value().to_index();
    ui.allocate_new_ui(UiBuilder::new().max_rect(rect), |ui| {
        egui::ComboBox::from_id_salt(id)
            .height(380.0)
            .width(rect.width() - 18.0)
            .truncate()
            .selected_text(egui::RichText::new(names[idx]).font(mono(12.0)))
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
