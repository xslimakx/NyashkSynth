//! FX rack tab: any number of effects per bus (the same kind may appear many times), a detail view
//! for the selected one built from the effect descriptors (`fxdesc`), and live graphs.
use nih_plug::prelude::*;
use nih_plug_egui::egui::{self, epaint::PathStroke, Align2, Color32, CornerRadius, Pos2, Rect, Sense, Stroke, StrokeKind, Ui, UiBuilder, Vec2};

use crate::editor::{cell, dark_box, fill_under, Gui};
use crate::fx::{eq_response_db, BqCoef};
use crate::fxdesc::{self, D, NKINDS, NV};
use crate::params::*;
use crate::ui::*;

fn slot_of(p: &SynthParams, bus: usize, i: usize) -> &FxSlotP {
    &p.rk[bus.min(1) * RACK_SLOTS + i.min(RACK_SLOTS - 1)]
}

/// Everything a slot stores, so slots can be moved around.
#[derive(Clone, Copy)]
struct Snap {
    ty: i32,
    on: bool,
    v: [f32; NV],
}

fn snap(s: &FxSlotP) -> Snap {
    let mut v = [0.0f32; NV];
    for (i, x) in s.v.iter().enumerate() {
        v[i] = x.v.unmodulated_normalized_value();
    }
    Snap { ty: s.ty.value(), on: s.on.value(), v }
}

fn write(setter: &ParamSetter, s: &FxSlotP, sn: &Snap) {
    set_i(setter, &s.ty, sn.ty);
    set_b(setter, &s.on, sn.on);
    for (i, x) in s.v.iter().enumerate() {
        set_norm(setter, &x.v, sn.v[i]);
    }
}

/// Indices of the slots of a bus that hold an effect, in chain order.
pub fn used(p: &SynthParams, bus: usize) -> Vec<usize> {
    (0..RACK_SLOTS).filter(|&i| slot_of(p, bus, i).ty.value() > 0).collect()
}

/// The knobs of a slot as real values.
fn slot_reals(s: &FxSlotP, kind: usize) -> [f32; NV] {
    fxdesc::reals(kind, &snap(s).v)
}

/// Is module `m` switched on in the old fixed bank?
pub fn legacy_on(b: &FxBank, m: usize) -> bool {
    match m {
        0 => b.hy_on.value(),
        1 => b.ds_on.value(),
        2 => b.fg_on.value(),
        3 => b.ph_on.value(),
        4 => b.ch_on.value(),
        5 => b.dl_on.value(),
        6 => b.cp_on.value(),
        7 => b.rv_on.value(),
        8 => b.eq_on.value(),
        9 => b.ff_on.value(),
        10 => b.bd_on.value(),
        _ => b.ut_on.value(),
    }
}

/// Projects saved before the rack still hold their effects in the fixed banks: move them into the rack and switch to it.
/// (Presets are converted when they are loaded, see `presets::with_rack`; this covers old host sessions.)
/// Returns how many effects each bus now has.
pub fn migrate_legacy(setter: &ParamSetter, p: &SynthParams) -> [usize; 2] {
    let mut count = [0usize; 2];
    if p.rackver.value() != 0 {
        return count;
    }
    for bus in 0..2 {
        let b = p.bank(bus);
        let mut k = 0;
        let mut seen = [false; 12];
        for o in b.order_params() {
            let m = (o.value().clamp(0, 11)) as usize;
            if seen[m] {
                continue;
            }
            seen[m] = true;
            if !legacy_on(b, m) || k >= RACK_SLOTS {
                continue;
            }
            let v = fxdesc::norms(m, &fxdesc::reals_from_bank(m, b));
            write(setter, slot_of(p, bus, k), &Snap { ty: m as i32 + 1, on: true, v });
            k += 1;
        }
        for i in k..RACK_SLOTS {
            let s = slot_of(p, bus, i);
            if s.ty.value() != 0 {
                set_i(setter, &s.ty, 0);
            }
        }
        count[bus] = k;
    }
    set_i(setter, &p.rackver, 1);
    count
}

/// Puts a new effect (with its default settings) into the first free slot. Returns its index.
pub fn add_effect(setter: &ParamSetter, p: &SynthParams, bus: usize, kind: usize) -> Option<usize> {
    let i = if p.rackver.value() == 0 {
        // the old bank is still in use: convert it first, the new effect goes behind the converted ones
        let n = migrate_legacy(setter, p)[bus.min(1)];
        if n >= RACK_SLOTS {
            return None;
        }
        n
    } else {
        (0..RACK_SLOTS).find(|&i| slot_of(p, bus, i).ty.value() == 0)?
    };
    write(setter, slot_of(p, bus, i), &Snap { ty: kind as i32 + 1, on: true, v: fxdesc::defaults(kind) });
    Some(i)
}

/// Mod destination of the first effect of its kind (the engine applies modulation there).
fn mod_dst(kind: usize, idx: usize) -> Option<ModDst> {
    Some(match (kind, idx) {
        (0, 4) => ModDst::FxHyperMix,
        (1, 1) => ModDst::FxDistDrive,
        (1, 4) => ModDst::FxDistMix,
        (2, 3) => ModDst::FxFlangerDepth,
        (3, 4) => ModDst::FxPhaserFreq,
        (4, 7) => ModDst::FxChorusMix,
        (5, 6) => ModDst::FxDelayFb,
        (5, 11) => ModDst::FxDelayMix,
        (7, 1) => ModDst::FxReverbSize,
        (7, 7) => ModDst::FxReverbMix,
        (9, 1) => ModDst::FxFilterCut,
        (9, 2) => ModDst::FxFilterRes,
        _ => return None,
    })
}

/// Which knobs of a kind are visible given the current values (sync hides RATE or DIV, link hides the right side ...).
fn visible(kind: usize, r: &[f32; NV]) -> Vec<usize> {
    let n = fxdesc::descs(kind).len();
    (0..n)
        .filter(|&i| match (kind, i) {
            (2 | 3 | 4, 0) => r[1] < 0.5,
            (2 | 3 | 4, 2) => r[1] > 0.5,
            (5, 0) => r[3] < 0.5,
            (5, 1) => r[3] < 0.5 && r[2] < 0.5,
            (5, 4) => r[3] > 0.5,
            (5, 5) => r[3] > 0.5 && r[2] < 0.5,
            (6, 6 | 7) => r[5] > 0.5,
            _ => true,
        })
        .collect()
}

fn has_graph(kind: usize) -> bool {
    matches!(kind, 0 | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | 10 | 11)
}

fn choice_combo(ui: &mut Ui, setter: &ParamSetter, prm: &FloatParam, d: &D, rect: Rect, id: impl std::hash::Hash) {
    let idx = d.real(prm.unmodulated_normalized_value()) as usize;
    let names = d.choices;
    ui.allocate_new_ui(UiBuilder::new().max_rect(rect), |ui| {
        ui.spacing_mut().interact_size.y = rect.height();
        egui::ComboBox::from_id_salt(id)
            .height(380.0)
            .width(rect.width())
            .truncate()
            .selected_text(egui::RichText::new(names[idx.min(names.len() - 1)]).font(mono(11.0)))
            .show_ui(ui, |ui| {
                for (i, n) in names.iter().enumerate() {
                    if ui.selectable_label(i == idx, egui::RichText::new(*n).font(mono(12.0))).clicked() {
                        set_norm(setter, prm, d.norm(i as f32));
                    }
                }
            });
    });
}

/// Draws the FX page. Returns the free area at the bottom of the detail view (the editor draws the spectrum there).
pub fn fx_tab(ui: &mut Ui, setter: &ParamSetter, p: &SynthParams, st: &mut Gui, c: Rect) -> Rect {
    // an old project: wait until the values are stable (a preset load is still travelling to the audio side), then convert
    if p.rackver.value() == 0 {
        st.fx_mig += 1;
        if st.fx_mig > 20 {
            migrate_legacy(setter, p);
            st.fx_mig = 0;
        }
    } else {
        st.fx_mig = 0;
    }
    let bus = st.fx_bus.min(1);
    // ---- header card: busses, routing, bus 2 level, signal path
    let hdr = Rect::from_min_max(Pos2::new(c.left(), c.top() - 8.0), Pos2::new(c.right(), c.top() + 62.0));
    ui.painter().rect_filled(hdr, CornerRadius::same(7), Color32::from_rgb(15, 15, 15));
    ui.painter().rect_stroke(hdr, CornerRadius::same(7), Stroke::new(1.0_f32, Color32::from_rgb(34, 34, 34)), StrokeKind::Inside);
    let hx = c.left() + 12.0;
    let hy = c.top() + 14.0;
    for k in 0..2 {
        if pill_tip(ui, Rect::from_min_size(Pos2::new(hx + 70.0 * k as f32, hy), Vec2::new(64.0, 26.0)), if k == 0 { "BUS 1" } else { "BUS 2" }, bus == k, "Each bus is its own effect chain. How the two are joined is set by ROUTING.") {
            st.fx_bus = k;
            st.fx_sel = 0;
        }
    }
    ui.painter().text(Pos2::new(hx + 160.0, hy + 13.0), Align2::LEFT_CENTER, "ROUTING", mono(9.0), DIM);
    enum_combo(ui, setter, &p.fx_route, "fx_route", Rect::from_min_size(Pos2::new(hx + 218.0, hy), Vec2::new(190.0, 26.0)));
    let mut rx = hx + 430.0;
    match p.fx_route.value() {
        FxRoute::SplitLh => {
            knob_s(ui, setter, &p.fx_split, "SPLIT", cell(rx, c.top() - 4.0, 56.0), "Crossover: lows go to bus 1, highs to bus 2", false);
            rx += 64.0;
        }
        FxRoute::SplitLmh => {
            knob_s(ui, setter, &p.fx_split, "LOW/MID", cell(rx, c.top() - 4.0, 56.0), "Low / mid crossover. Lows go to bus 1", false);
            knob_s(ui, setter, &p.fx_split2, "MID/HI", cell(rx + 62.0, c.top() - 4.0, 56.0), "Mid / high crossover. Mids go to bus 2, highs stay dry", false);
            rx += 126.0;
        }
        _ => {}
    }
    knob_s(ui, setter, &p.fx2_level, "BUS 2 LEVEL", cell(rx, c.top() - 4.0, 64.0), "Level of bus 2 in the mix", false);
    let route = match p.fx_route.value() {
        FxRoute::Series => "bus 1, then bus 2",
        FxRoute::Parallel => "bus 1 + bus 2 in parallel",
        FxRoute::SplitLh => "lows > bus 1, highs > bus 2",
        FxRoute::SplitLmh => "lows > bus 1, mids > bus 2, highs dry",
        FxRoute::SplitMs => "mid > bus 1, side > bus 2",
    };
    ui.painter().text(Pos2::new(c.right() - 14.0, hdr.center().y), Align2::RIGHT_CENTER, format!("IN  >  {route}  >  OUT"), mono(9.5), DIM);

    // ---- chain list (left)
    let top = c.top() + 78.0;
    let lw = 292.0;
    let card = Rect::from_min_max(Pos2::new(c.left() + lw + 14.0, top - 14.0), Pos2::new(c.right(), c.bottom() - 2.0));
    ui.painter().rect_filled(card, CornerRadius::same(7), Color32::from_rgb(15, 15, 15));
    ui.painter().rect_stroke(card, CornerRadius::same(7), Stroke::new(1.0_f32, Color32::from_rgb(34, 34, 34)), StrokeKind::Inside);
    let lcard = Rect::from_min_max(Pos2::new(c.left(), top - 14.0), Pos2::new(c.left() + lw, c.bottom() - 2.0));
    ui.painter().rect_filled(lcard, CornerRadius::same(7), Color32::from_rgb(15, 15, 15));
    ui.painter().rect_stroke(lcard, CornerRadius::same(7), Stroke::new(1.0_f32, Color32::from_rgb(34, 34, 34)), StrokeKind::Inside);
    let list = used(p, bus);
    ui.painter().text(Pos2::new(c.left() + 12.0, top + 6.0), Align2::LEFT_CENTER, format!("BUS {} CHAIN   {} / {}", bus + 1, list.len(), RACK_SLOTS), mono(10.0), WHITE);
    let rows_top = top + 22.0;
    let (rh, gap) = (36.0f32, 4.0f32);
    let row = |pos: usize| Rect::from_min_size(Pos2::new(c.left() + 10.0, rows_top + (rh + gap) * pos as f32), Vec2::new(lw - 20.0, rh));
    if st.fx_sel >= RACK_SLOTS || !list.contains(&st.fx_sel) {
        st.fx_sel = list.first().copied().unwrap_or(0);
    }
    let mut clicked: Option<usize> = None;
    let mut toggle: Option<usize> = None;
    let mut remove: Option<usize> = None;
    let mut moved: Option<(usize, usize)> = None;
    for (pos, &si) in list.iter().enumerate() {
        let s = slot_of(p, bus, si);
        let kind = (s.ty.value() as usize - 1).min(NKINDS - 1);
        let r = row(pos);
        let resp = ui.interact(r, egui::Id::new(("fxrow", bus, si)), Sense::click_and_drag()).on_hover_text("Click: edit. Drag up / down: change the order. Dot: on / off. x: remove");
        let on = s.on.value();
        let sel = st.fx_sel == si;
        let dragging = st.fx_drag == Some(pos);
        {
            let painter = ui.painter();
            painter.rect_filled(r, CornerRadius::same(5), if sel { Color32::from_rgb(34, 34, 34) } else if resp.hovered() { PANEL2 } else { Color32::from_rgb(14, 14, 14) });
            painter.rect_stroke(r, CornerRadius::same(5), Stroke::new(1.0_f32, if dragging || sel { WHITE } else { LINE }), StrokeKind::Inside);
            let dot = Pos2::new(r.left() + 16.0, r.center().y);
            painter.circle_filled(dot, 5.0, if on { WHITE } else { Color32::from_rgb(50, 50, 50) });
            painter.circle_stroke(dot, 5.0, Stroke::new(1.0_f32, if on { WHITE } else { DIM }));
            painter.text(Pos2::new(r.left() + 34.0, r.center().y), Align2::LEFT_CENTER, format!("{}  {}", pos + 1, fxdesc::KIND_NAMES[kind]), mono(11.5), if on { WHITE } else { DIM });
            painter.text(Pos2::new(r.right() - 12.0, r.center().y), Align2::CENTER_CENTER, "x", mono(13.0), if resp.hovered() { WHITE } else { DIM });
        }
        if resp.drag_started() {
            st.fx_drag = Some(pos);
        }
        if resp.drag_stopped() {
            if let Some(from) = st.fx_drag.take() {
                if let Some(pt) = ui.input(|i| i.pointer.latest_pos()) {
                    let to = (((pt.y - rows_top) / (rh + gap)).floor().max(0.0) as usize).min(list.len() - 1);
                    if to != from {
                        moved = Some((from, to));
                    }
                }
            }
        } else if resp.clicked() {
            if let Some(pt) = ui.input(|i| i.pointer.interact_pos()) {
                if pt.x < r.left() + 30.0 {
                    toggle = Some(si);
                } else if pt.x > r.right() - 26.0 {
                    remove = Some(si);
                } else {
                    clicked = Some(si);
                }
            }
        }
    }
    if let Some(si) = clicked {
        st.fx_sel = si;
    }
    if let Some(si) = toggle {
        let s = slot_of(p, bus, si);
        set_b(setter, &s.on, !s.on.value());
    }
    if let Some(si) = remove {
        set_i(setter, &slot_of(p, bus, si).ty, 0);
    }
    if let Some((from, to)) = moved {
        // rotate the contents of the used slots: the dragged effect lands at position `to`
        let mut snaps: Vec<Snap> = list.iter().map(|&i| snap(slot_of(p, bus, i))).collect();
        let sel_pos = list.iter().position(|&i| i == st.fx_sel).unwrap_or(0);
        let v = snaps.remove(from);
        snaps.insert(to, v);
        for (k, &i) in list.iter().enumerate() {
            write(setter, slot_of(p, bus, i), &snaps[k]);
        }
        let new_sel = if sel_pos == from { to } else if from < sel_pos && to >= sel_pos { sel_pos - 1 } else if from > sel_pos && to <= sel_pos { sel_pos + 1 } else { sel_pos };
        st.fx_sel = list[new_sel.min(list.len() - 1)];
    }
    if let Some(from) = st.fx_drag {
        if let Some(pos) = ui.input(|i| i.pointer.latest_pos()) {
            let to = (((pos.y - rows_top) / (rh + gap)).floor().max(0.0) as usize).min(list.len().max(1) - 1);
            let y = rows_top + (rh + gap) * (if to > from { to + 1 } else { to }) as f32 - gap * 0.5;
            ui.painter().line_segment([Pos2::new(c.left() + 10.0, y), Pos2::new(c.left() + lw - 10.0, y)], Stroke::new(2.0_f32, WHITE));
            let lp = ui.ctx().layer_painter(egui::LayerId::new(egui::Order::Tooltip, egui::Id::new("fx_ghost")));
            let g = Rect::from_center_size(pos + Vec2::new(14.0, 12.0), Vec2::new(150.0, 24.0));
            lp.rect_filled(g, CornerRadius::same(5), WHITE);
            if let Some(&si) = list.get(from) {
                let kind = (slot_of(p, bus, si).ty.value() as usize - 1).min(NKINDS - 1);
                lp.text(g.center(), Align2::CENTER_CENTER, fxdesc::KIND_NAMES[kind], mono(10.5), Color32::BLACK);
            }
        }
    }
    // ---- add button + menu
    let add_r = Rect::from_min_size(Pos2::new(c.left() + 10.0, rows_top + (rh + gap) * list.len() as f32 + 4.0), Vec2::new(lw - 20.0, 32.0));
    if list.len() < RACK_SLOTS {
        if pill_tip(ui, add_r, "+  ADD EFFECT", st.fx_menu, "Add an effect to this bus. The same effect can be added as often as you like.") {
            st.fx_menu = !st.fx_menu;
        }
    } else {
        ui.painter().text(add_r.center(), Align2::CENTER_CENTER, "chain is full", mono(10.0), DIM);
    }
    if list.is_empty() {
        ui.painter().text(Pos2::new(c.left() + lw + 40.0, top + 70.0), Align2::LEFT_CENTER, "This bus has no effects yet.  Press  + ADD EFFECT  on the left.", mono(12.0), DIM);
        ui.painter().text(Pos2::new(c.left() + lw + 40.0, top + 96.0), Align2::LEFT_CENTER, "Every effect can be used several times, in any order (drag the rows to reorder).", mono(10.0), DIM);
    }
    if st.fx_menu && list.len() < RACK_SLOTS {
        let pos = Pos2::new(add_r.left(), add_r.bottom() + 2.0);
        let area = egui::Area::new(egui::Id::new("fx_add_menu")).order(egui::Order::Foreground).fixed_pos(pos).show(ui.ctx(), |ui| {
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                ui.set_min_width(lw - 16.0);
                for (kind, name, desc) in fxdesc::MENU {
                    let resp = ui.selectable_label(false, egui::RichText::new(name).font(mono(12.0))).on_hover_text(desc);
                    if resp.clicked() {
                        if let Some(i) = add_effect(setter, p, bus, kind) {
                            st.fx_sel = i;
                        }
                        st.fx_menu = false;
                    }
                    ui.label(egui::RichText::new(desc).font(mono(8.5)).color(DIM));
                    ui.add_space(2.0);
                }
            });
        });
        if area.response.clicked_elsewhere() && !ui.rect_contains_pointer(add_r) {
            st.fx_menu = false;
        }
    }

    // ---- detail (right)
    let dx = card.left() + 14.0;
    let dw = card.right() - 14.0 - dx;
    let Some(&si) = list.iter().find(|&&i| i == st.fx_sel) else {
        return Rect::from_min_max(Pos2::new(dx, top + 130.0), Pos2::new(dx + dw, card.bottom() - 12.0));
    };
    let s = slot_of(p, bus, si);
    let kind = (s.ty.value() as usize - 1).min(NKINDS - 1);
    let ds = fxdesc::descs(kind);
    let r = slot_reals(s, kind);
    let fp = fxdesc::patch_for(kind, &r, 120.0);
    let pos_in = list.iter().position(|&i| i == si).unwrap_or(0) + 1;
    ui.painter().text(Pos2::new(dx, top + 6.0), Align2::LEFT_CENTER, fxdesc::KIND_NAMES[kind], mono(13.0), WHITE);
    ui.painter().text(Pos2::new(dx + 130.0, top + 6.0), Align2::LEFT_CENTER, format!("effect {pos_in} of {} on bus {}", list.len(), bus + 1), mono(9.0), DIM);
    if pill_tip(ui, Rect::from_min_size(Pos2::new(dx + 330.0, top - 5.0), Vec2::new(60.0, 22.0)), if s.on.value() { "ON" } else { "OFF" }, s.on.value(), "Effect on / off") {
        set_b(setter, &s.on, !s.on.value());
    }
    if pill_tip(ui, Rect::from_min_size(Pos2::new(dx + 396.0, top - 5.0), Vec2::new(70.0, 22.0)), "RESET", false, "Back to the default settings of this effect") {
        write(setter, s, &Snap { ty: s.ty.value(), on: s.on.value(), v: fxdesc::defaults(kind) });
    }
    if pill_tip(ui, Rect::from_min_size(Pos2::new(dx + 472.0, top - 5.0), Vec2::new(90.0, 22.0)), "DUPLICATE", false, "Add a copy of this effect (with its settings) after it") {
        if list.len() < RACK_SLOTS {
            if let Some(i) = (0..RACK_SLOTS).find(|&i| slot_of(p, bus, i).ty.value() == 0) {
                write(setter, slot_of(p, bus, i), &snap(s));
                st.fx_sel = i;
            }
        }
    }
    ui.painter().text(Pos2::new(dx + 570.0, top + 6.0), Align2::LEFT_CENTER, fxdesc::MENU.iter().find(|m| m.0 == kind).map(|m| m.2).unwrap_or(""), mono(9.0), DIM);

    // knobs
    let vis = visible(kind, &r);
    let cw = (dw / 14.0).clamp(66.0, 84.0);
    let ky = top + 28.0;
    // modulation rings only on the first effect of its kind (the engine moves that one)
    let first_of_kind = list.iter().find(|&&i| slot_of(p, bus, i).ty.value() as usize == kind + 1) == Some(&si);
    for (n, &i) in vis.iter().enumerate() {
        let d = &ds[i];
        let x = dx + cw * n as f32;
        let prm = &s.v[i].v;
        match d.fm {
            fxdesc::Fm::Choice => {
                ui.painter().text(Pos2::new(x + cw * 0.5, ky + 14.0), Align2::CENTER_CENTER, d.name, mono(9.5), DIM);
                choice_combo(ui, setter, prm, d, Rect::from_min_size(Pos2::new(x + 2.0, ky + 28.0), Vec2::new(cw - 4.0, 26.0)), ("fxch", bus, si, i));
            }
            fxdesc::Fm::Toggle => {
                if pill_tip(ui, Rect::from_min_size(Pos2::new(x + 2.0, ky + 28.0), Vec2::new(cw - 4.0, 26.0)), d.name, r[i] > 0.5, d.tip) {
                    set_norm(setter, prm, if r[i] > 0.5 { 0.0 } else { 1.0 });
                }
            }
            _ => {
                if first_of_kind {
                    if let Some(md) = mod_dst(kind, i) {
                        mdst(if bus == 0 { md } else { md.bus2() });
                    }
                }
                use_desc(d);
                knob_s(ui, setter, prm, d.name, cell(x, ky, cw), d.tip, d.min < 0.0);
            }
        }
    }

    // graphs
    let gy = ky + 92.0;
    let gh = if has_graph(kind) { 168.0 } else { 0.0 };
    let g = Rect::from_min_size(Pos2::new(dx, gy), Vec2::new(dw, gh));
    match kind {
        1 => dist_curve(ui, g, &fp),
        5 => delay_taps(ui, g, &fp),
        6 => comp_curve(ui, g, &fp),
        7 => {
            reverb_decay(ui, Rect::from_min_size(g.min, Vec2::new(g.width(), g.height() - 0.0)), &fp);
            if fp.rv_type == ReverbType::Convolve {
                let nm = p.ir.lock().map(|gd| crate::sample::encoded_name(&gd)).unwrap_or_default();
                let label: String = if nm.is_empty() { "LOAD IR...".to_string() } else { nm.chars().take(22).collect() };
                if pill_tip(ui, Rect::from_min_size(Pos2::new(g.right() - 190.0, g.top() + 4.0), Vec2::new(184.0, 22.0)), &label, !nm.is_empty(), "Load a .wav impulse response (room, hall, cabinet, anything). SIZE trims the tail, MIX sets the wet level. One impulse is shared by all reverbs.") {
                    st.open_ir();
                }
            }
        }
        8 => {
            graph(ui, g, |f| eq_response_db(&fp, f, 48000.0), (-18.0, 18.0), Some((&st.spec, st.spec_fs)));
            eq_nodes(ui, setter, s, g, (bus, si));
        }
        9 => {
            let (fc, q, ty) = (fp.ff_cut, 0.5 + fp.ff_res * 9.0, fp.ff_type);
            graph(
                ui,
                g,
                |f| {
                    let fs = 48000.0;
                    let m = match ty {
                        FxFilterType::Lp12 => BqCoef::lowpass(fc, q, fs).mag(f, fs),
                        FxFilterType::Lp24 => BqCoef::lowpass(fc, q, fs).mag(f, fs).powi(2),
                        FxFilterType::Hp12 => BqCoef::highpass(fc, q, fs).mag(f, fs),
                        FxFilterType::Hp24 => BqCoef::highpass(fc, q, fs).mag(f, fs).powi(2),
                        FxFilterType::Band => BqCoef::bandpass(fc, q, fs).mag(f, fs),
                        FxFilterType::Notch => BqCoef::notch(fc, q, fs).mag(f, fs),
                        FxFilterType::Peak => BqCoef::peaking(fc, q, 12.0, fs).mag(f, fs),
                    };
                    20.0 * m.max(1e-6).log10()
                },
                (-36.0, 18.0),
                Some((&st.spec, st.spec_fs)),
            );
            ff_node(ui, setter, s, g, (bus, si));
        }
        10 => bode_map(ui, g, &fp),
        0 => hyper_view(ui, g, &fp),
        2 => flanger_view(ui, g, &fp, st),
        3 => phaser_view(ui, g, &fp, st),
        4 => chorus_view(ui, g, &fp),
        11 => utility_view(ui, g, &fp),
        _ => {}
    }
    Rect::from_min_max(Pos2::new(dx, gy + gh + if gh > 0.0 { 10.0 } else { 0.0 }), Pos2::new(dx + dw, card.bottom() - 12.0))
}

fn graph(ui: &mut Ui, g: Rect, f: impl Fn(f32) -> f32, range: (f32, f32), spec: Option<(&[f32], f32)>) {
    dark_box(ui, g);
    let pa = ui.painter().with_clip_rect(g.shrink(1.0));
    if let Some((sp, fs)) = spec {
        crate::editor::draw_spectrum(&pa, g.shrink(1.0), sp, fs, -80.0, Color32::from_rgb(46, 46, 46));
    }
    let (lo, hi) = (20.0f32, 20000.0f32);
    let xf = |fr: f32| g.left() + g.width() * ((fr / lo).ln() / (hi / lo).ln());
    let yd = |db: f32| g.bottom() - g.height() * ((db.clamp(range.0, range.1) - range.0) / (range.1 - range.0));
    for fr in [100.0f32, 1000.0, 10000.0] {
        pa.line_segment([Pos2::new(xf(fr), g.top()), Pos2::new(xf(fr), g.bottom())], Stroke::new(1.0_f32, Color32::from_rgb(24, 24, 24)));
    }
    pa.line_segment([Pos2::new(g.left(), yd(0.0)), Pos2::new(g.right(), yd(0.0))], Stroke::new(1.0_f32, Color32::from_rgb(34, 34, 34)));
    let pts: Vec<Pos2> = (0..=240).map(|i| {
        let fr = lo * (hi / lo).powf(i as f32 / 240.0);
        Pos2::new(xf(fr), yd(f(fr)))
    }).collect();
    fill_under(&pa, &pts, yd(0.0).min(g.bottom()), Color32::from_rgb(30, 30, 30));
    pa.add(egui::Shape::line(pts, PathStroke::new(1.6_f32, ACCENT)));
}

/// Draggable EQ nodes on top of the response graph (drag = frequency / gain, wheel on the mid node = Q).
fn eq_nodes(ui: &mut Ui, setter: &ParamSetter, s: &FxSlotP, g: Rect, key: (usize, usize)) {
    let ds = fxdesc::descs(8);
    let (lo, hi) = (20.0f32, 20000.0f32);
    let xf = |fr: f32| g.left() + g.width() * ((fr / lo).ln() / (hi / lo).ln());
    let fx = |x: f32| lo * (hi / lo).powf(((x - g.left()) / g.width()).clamp(0.0, 1.0));
    let yd = |db: f32| g.bottom() - g.height() * ((db.clamp(-18.0, 18.0) + 18.0) / 36.0);
    let dbf = |y: f32| (((g.bottom() - y) / g.height()) * 36.0 - 18.0).clamp(-18.0, 18.0);
    let nodes: [(usize, usize, &str); 3] = [(0, 1, "LOW"), (2, 4, "MID"), (5, 6, "HIGH")];
    for (i, (fi, gi, name)) in nodes.iter().enumerate() {
        let (fp, gp) = (&s.v[*fi].v, &s.v[*gi].v);
        let c = Pos2::new(xf(ds[*fi].real(fp.unmodulated_normalized_value())), yd(ds[*gi].real(gp.unmodulated_normalized_value())));
        let hit = Rect::from_center_size(c, Vec2::splat(20.0));
        let resp = ui.interact(hit, egui::Id::new(("eqnode", key, i)), Sense::click_and_drag()).on_hover_text(format!("{name} band: drag = frequency / gain{}", if i == 1 { ", wheel = Q" } else { "" }));
        let offk = egui::Id::new(("eqnode_off", key, i));
        if resp.drag_started() {
            setter.begin_set_parameter(fp);
            setter.begin_set_parameter(gp);
            // keep the grab point: the node must not jump to the pointer
            let off = resp.interact_pointer_pos().map_or(Vec2::ZERO, |pp| pp - c);
            ui.data_mut(|d| d.insert_temp(offk, off));
        }
        if resp.dragged() {
            if let Some(pt) = resp.interact_pointer_pos() {
                let off = ui.data(|d| d.get_temp::<Vec2>(offk)).unwrap_or(Vec2::ZERO);
                let pt = pt - off;
                setter.set_parameter_normalized(fp, ds[*fi].norm(fx(pt.x)));
                setter.set_parameter_normalized(gp, ds[*gi].norm(dbf(pt.y)));
            }
        }
        if resp.drag_stopped() {
            setter.end_set_parameter(fp);
            setter.end_set_parameter(gp);
        }
        if resp.double_clicked() {
            set_norm(setter, gp, ds[*gi].norm(0.0));
        }
        if i == 1 && resp.hovered() {
            let sc = ui.input(|i| i.smooth_scroll_delta.y);
            if sc != 0.0 {
                let q = &s.v[3].v;
                set_norm(setter, q, q.unmodulated_normalized_value() + sc / 1200.0);
            }
        }
        let hot = resp.hovered() || resp.dragged();
        ui.painter().circle_filled(c, if hot { 7.0 } else { 5.5 }, Color32::BLACK);
        ui.painter().circle_stroke(c, if hot { 7.0 } else { 5.5 }, Stroke::new(1.6_f32, WHITE));
        ui.painter().text(c + Vec2::new(0.0, -13.0), Align2::CENTER_CENTER, format!("{}", i + 1), mono(8.5), DIM);
    }
}

/// Draggable node on the FX filter graph: left / right = cutoff, up / down = resonance.
fn ff_node(ui: &mut Ui, setter: &ParamSetter, s: &FxSlotP, g: Rect, key: (usize, usize)) {
    let ds = fxdesc::descs(9);
    let (cut, resp_p) = (&s.v[1].v, &s.v[2].v);
    let (lo, hi) = (20.0f32, 20000.0f32);
    let (dlo, dhi) = (-36.0f32, 18.0f32);
    let xf = |fr: f32| g.left() + g.width() * ((fr / lo).ln() / (hi / lo).ln());
    let ty = <FxFilterType as Enum>::from_index(ds[0].real(s.v[0].v.unmodulated_normalized_value()) as usize);
    let pw = match ty {
        FxFilterType::Lp24 | FxFilterType::Hp24 => Some(2.0f32),
        FxFilterType::Lp12 | FxFilterType::Hp12 | FxFilterType::Band => Some(1.0),
        _ => None,
    };
    let yd = |db: f32| g.bottom() - g.height() * ((db.clamp(dlo, dhi) - dlo) / (dhi - dlo));
    let res = resp_p.unmodulated_normalized_value();
    let y_of_res = |r: f32| -> f32 {
        match pw {
            Some(p) => yd(20.0 * p * (0.5 + r * 9.0).log10()),
            None => g.bottom() - 6.0 - (g.height() - 12.0) * r,
        }
    };
    let res_of_y = |y: f32| -> f32 {
        match pw {
            Some(p) => {
                let db = dlo + (dhi - dlo) * ((g.bottom() - y) / g.height()).clamp(0.0, 1.0);
                (((10f32.powf(db / (20.0 * p))) - 0.5) / 9.0).clamp(0.0, 1.0)
            }
            None => ((g.bottom() - 6.0 - y) / (g.height() - 12.0)).clamp(0.0, 1.0),
        }
    };
    let c = Pos2::new(xf(ds[1].real(cut.unmodulated_normalized_value())).clamp(g.left() + 7.0, g.right() - 7.0), y_of_res(res).clamp(g.top() + 7.0, g.bottom() - 7.0));
    let area = ui.interact(g, egui::Id::new(("ffgraph", key)), Sense::click_and_drag()).on_hover_text("Drag: left / right = cutoff, up / down = resonance");
    let offk = egui::Id::new(("ffnode_off", key));
    if area.drag_started() {
        setter.begin_set_parameter(cut);
        setter.begin_set_parameter(resp_p);
        // grabbing the dot itself keeps the grab point; grabbing elsewhere jumps the dot there
        let off = area.interact_pointer_pos().map_or(Vec2::ZERO, |pp| if (pp - c).length() < 16.0 { pp - c } else { Vec2::ZERO });
        ui.data_mut(|d| d.insert_temp(offk, off));
    }
    if area.dragged() {
        if let Some(pt) = area.interact_pointer_pos() {
            let pt = pt - ui.data(|d| d.get_temp::<Vec2>(offk)).unwrap_or(Vec2::ZERO);
            let fx = ((pt.x - g.left()) / g.width()).clamp(0.0, 1.0);
            setter.set_parameter_normalized(cut, fx);
            setter.set_parameter_normalized(resp_p, res_of_y(pt.y));
        }
    }
    if area.drag_stopped() {
        setter.end_set_parameter(cut);
        setter.end_set_parameter(resp_p);
    }
    let hot = area.hovered() || area.dragged();
    ui.painter().circle_filled(c, if hot { 7.0 } else { 5.5 }, Color32::BLACK);
    ui.painter().circle_stroke(c, if hot { 7.0 } else { 5.5 }, Stroke::new(1.6_f32, WHITE));
}


/// Transfer curve of the distortion.
fn dist_curve(ui: &mut Ui, g: Rect, fp: &crate::fx::FxPatch) {
    dark_box(ui, g);
    let pa = ui.painter().with_clip_rect(g.shrink(1.0));
    pa.line_segment([Pos2::new(g.left(), g.center().y), Pos2::new(g.right(), g.center().y)], Stroke::new(1.0_f32, Color32::from_rgb(28, 28, 28)));
    pa.line_segment([Pos2::new(g.center().x, g.top()), Pos2::new(g.center().x, g.bottom())], Stroke::new(1.0_f32, Color32::from_rgb(28, 28, 28)));
    let drv = 1.0 + fp.ds_drive * 14.0;
    let pts: Vec<Pos2> = (0..=160)
        .map(|i| {
            let x = i as f32 / 80.0 - 1.0;
            let v = x * drv + fp.ds_bias * 0.5;
            let y = match fp.ds_type {
                DistType::Hard => v.clamp(-1.0, 1.0),
                DistType::Fold | DistType::SinFold => (v * std::f32::consts::FRAC_PI_2).sin(),
                DistType::Crush | DistType::Downsample => (v * 6.0).round() / 6.0,
                DistType::Rectify => v.abs().min(1.0) * 2.0 - 1.0,
                DistType::Asym | DistType::Diode1 | DistType::Diode2 => if v > 0.0 { v.tanh() } else { (v * 0.5).tanh() * 1.4 },
                _ => v.tanh(),
            };
            Pos2::new(g.left() + g.width() * i as f32 / 160.0, g.center().y - y.clamp(-1.1, 1.1) * g.height() * 0.42)
        })
        .collect();
    pa.add(egui::Shape::line(pts, PathStroke::new(1.6_f32, ACCENT)));
    pa.text(Pos2::new(g.left() + 8.0, g.top() + 9.0), Align2::LEFT_CENTER, "TRANSFER CURVE  (input > output)", mono(9.0), DIM);
}

/// Where every input frequency lands after the Bode shifter.
fn bode_map(ui: &mut Ui, g: Rect, fp: &crate::fx::FxPatch) {
    dark_box(ui, g);
    let pa = ui.painter().with_clip_rect(g.shrink(1.0));
    let sq = Rect::from_min_size(Pos2::new(g.left() + 16.0, g.top() + 24.0), Vec2::new(g.width() - 32.0, g.height() - 34.0));
    let fmax = 6000.0f32;
    let xy = |fi: f32, fo: f32| Pos2::new(sq.left() + sq.width() * (fi / fmax).clamp(0.0, 1.0), sq.bottom() - sq.height() * (fo / fmax).clamp(0.0, 1.0));
    for f in [1000.0f32, 2000.0, 3000.0, 4000.0, 5000.0] {
        pa.line_segment([xy(f, 0.0), xy(f, fmax)], Stroke::new(1.0_f32, Color32::from_rgb(22, 22, 22)));
        pa.line_segment([xy(0.0, f), xy(fmax, f)], Stroke::new(1.0_f32, Color32::from_rgb(22, 22, 22)));
    }
    pa.line_segment([xy(0.0, 0.0), xy(fmax, fmax)], Stroke::new(1.0_f32, Color32::from_rgb(52, 52, 52)));
    let sh = fp.bd_shift + fp.bd_fine;
    // a negative output frequency folds back to a positive one (and is heard that way)
    let line = |shift: f32, col: Color32| {
        let pts: Vec<Pos2> = (0..=120).map(|i| {
            let fi = fmax * i as f32 / 120.0;
            xy(fi, (fi + shift).abs())
        }).collect();
        pa.add(egui::Shape::line(pts, PathStroke::new(1.8_f32, col)));
    };
    match fp.bd_mode {
        1 => {
            line(sh, WHITE);
            line(-sh, Color32::from_rgb(190, 190, 190));
        }
        2 => {
            line(sh, WHITE);
            line(-sh, Color32::from_rgb(130, 130, 130));
        }
        _ => line(sh, WHITE),
    }
    if fp.bd_lfo_depth > 1.0 {
        let d = fp.bd_lfo_depth;
        for s in [sh + d, sh - d] {
            let pts: Vec<Pos2> = (0..=60).map(|i| { let fi = fmax * i as f32 / 60.0; xy(fi, (fi + s).abs()) }).collect();
            pa.add(egui::Shape::line(pts, PathStroke::new(1.0_f32, Color32::from_rgb(70, 70, 70))));
        }
    }
    let note = match fp.bd_mode {
        1 => format!("RING MOD  every partial f becomes f+{:.0} and f-{:.0}", sh.abs(), sh.abs()),
        2 => format!("SPREAD  left f{:+.0}  (white)   right f{:+.0}  (grey)", sh, -sh),
        _ => format!("SHIFT  every partial moves by {:+.1} Hz  (not a pitch shift: the sound turns inharmonic)", sh),
    };
    pa.text(Pos2::new(g.left() + 8.0, g.top() + 9.0), Align2::LEFT_CENTER, note, mono(9.0), DIM);
    pa.text(Pos2::new(sq.right() - 6.0, sq.bottom() - 8.0), Align2::RIGHT_CENTER, "x: input Hz   y: output Hz   (0 - 6 kHz)", mono(8.5), DIM);
}

fn comp_curve(ui: &mut Ui, g: Rect, fp: &crate::fx::FxPatch) {
    dark_box(ui, g);
    let pa = ui.painter().with_clip_rect(g.shrink(1.0));
    let side = g.height().min(g.width()) - 16.0;
    let sq = Rect::from_min_size(Pos2::new(g.left() + 14.0, g.top() + 8.0), Vec2::splat(side));
    let xy = |inp: f32, out: f32| Pos2::new(sq.left() + sq.width() * ((inp + 60.0) / 60.0).clamp(0.0, 1.0), sq.bottom() - sq.height() * ((out + 60.0) / 60.0).clamp(0.0, 1.0));
    for db in [-48.0f32, -36.0, -24.0, -12.0] {
        pa.line_segment([xy(db, -60.0), xy(db, 0.0)], Stroke::new(1.0_f32, Color32::from_rgb(22, 22, 22)));
        pa.line_segment([xy(-60.0, db), xy(0.0, db)], Stroke::new(1.0_f32, Color32::from_rgb(22, 22, 22)));
    }
    pa.line_segment([xy(-60.0, -60.0), xy(0.0, 0.0)], Stroke::new(1.0_f32, Color32::from_rgb(44, 44, 44)));
    let pts: Vec<Pos2> = (0..=60)
        .map(|i| {
            let inp = -60.0 + i as f32;
            let out = if inp > fp.cp_thresh { fp.cp_thresh + (inp - fp.cp_thresh) / fp.cp_ratio.max(1.0) } else { inp } + fp.cp_gain;
            xy(inp, out)
        })
        .collect();
    pa.add(egui::Shape::line(pts, PathStroke::new(1.8_f32, ACCENT)));
    let tp = xy(fp.cp_thresh, fp.cp_thresh + fp.cp_gain);
    pa.circle_filled(tp, 4.0, Color32::BLACK);
    pa.circle_stroke(tp, 4.0, Stroke::new(1.5_f32, WHITE));
    pa.text(Pos2::new(sq.right() + 14.0, sq.top() + 6.0), Align2::LEFT_CENTER, "INPUT dB  >  OUTPUT dB (dot = threshold)", mono(9.0), DIM);
}

/// Repeats of the delay: bar height = level, left channel up, right channel down.
fn delay_taps(ui: &mut Ui, g: Rect, fp: &crate::fx::FxPatch) {
    dark_box(ui, g);
    let pa = ui.painter().with_clip_rect(g.shrink(1.0));
    let (l, r) = (fp.dl_ms_l.max(1.0), fp.dl_ms_r.max(1.0));
    let tmax = (l.max(r) * 8.0).max(400.0);
    let mid = g.center().y;
    pa.line_segment([Pos2::new(g.left(), mid), Pos2::new(g.right(), mid)], Stroke::new(1.0_f32, Color32::from_rgb(34, 34, 34)));
    let xt = |t: f32| g.left() + 14.0 + (g.width() - 28.0) * (t / tmax);
    let h = g.height() * 0.42;
    pa.line_segment([Pos2::new(xt(0.0), mid - h), Pos2::new(xt(0.0), mid + h)], Stroke::new(3.0_f32, WHITE));
    let fb = fp.dl_fb.clamp(0.0, 0.98);
    let mut amp = 1.0f32;
    for k in 1..=64 {
        amp *= if k == 1 { 1.0 } else { fb };
        if amp < 0.02 {
            break;
        }
        // ping-pong: odd repeats on the left, even on the right; otherwise both channels repeat together
        let (tl, tr) = (l * k as f32, r * k as f32);
        let (show_l, show_r) = if fp.dl_pp { (k % 2 == 1, k % 2 == 0) } else { (true, true) };
        if show_l && tl <= tmax {
            pa.line_segment([Pos2::new(xt(tl), mid), Pos2::new(xt(tl), mid - h * amp)], Stroke::new(3.0_f32, Color32::from_rgb(220, 220, 220)));
        }
        if show_r && tr <= tmax {
            pa.line_segment([Pos2::new(xt(tr) + 4.0, mid), Pos2::new(xt(tr) + 4.0, mid + h * amp)], Stroke::new(3.0_f32, Color32::from_rgb(150, 150, 150)));
        }
    }
    pa.text(Pos2::new(g.left() + 8.0, g.top() + 9.0), Align2::LEFT_CENTER, format!("REPEATS  (L up, R down)   0 - {:.0} ms", tmax), mono(9.0), DIM);
}

/// Rough decay curve of the reverb (level over time).
fn reverb_decay(ui: &mut Ui, g: Rect, fp: &crate::fx::FxPatch) {
    dark_box(ui, g);
    let pa = ui.painter().with_clip_rect(g.shrink(1.0));
    let t60 = 0.3 + fp.rv_size.clamp(0.0, 1.0) * 7.0;
    let tmax = 8.0f32;
    let pre = fp.rv_pre * 0.001;
    let pts: Vec<Pos2> = (0..=200)
        .map(|i| {
            let t = tmax * i as f32 / 200.0;
            let db = if t < pre { -80.0 } else { -60.0 * (t - pre) / t60 };
            Pos2::new(g.left() + 14.0 + (g.width() - 28.0) * t / tmax, g.bottom() - 10.0 - (g.height() - 30.0) * ((db + 60.0) / 60.0).clamp(0.0, 1.0))
        })
        .collect();
    fill_under(&pa, &pts, g.bottom() - 10.0, Color32::from_rgb(30, 30, 30));
    pa.add(egui::Shape::line(pts, PathStroke::new(1.8_f32, ACCENT)));
    pa.text(Pos2::new(g.left() + 8.0, g.top() + 9.0), Align2::LEFT_CENTER, format!("DECAY (about {:.1} s to -60 dB)   axis 0 - 8 s", t60 + pre), mono(9.0), DIM);
}

// ---------------------------------------------------------------- animated pictures of the modulation effects

fn live_phase(ui: &Ui, rate_hz: f32) -> f32 {
    ui.ctx().request_repaint();
    ((ui.input(|i| i.time) as f32 * rate_hz.max(0.01)) % 1.0 + 1.0) % 1.0
}

fn caption(pa: &egui::Painter, g: Rect, t: String) {
    pa.text(Pos2::new(g.left() + 8.0, g.top() + 9.0), Align2::LEFT_CENTER, t, mono(9.0), DIM);
}

/// Comb filter of the flanger, moving with its LFO (the notches sweep exactly like the sound does).
fn flanger_view(ui: &mut Ui, g: Rect, fp: &crate::fx::FxPatch, st: &Gui) {
    let ph = live_phase(ui, fp.fg_rate);
    let tau = (fp.fg_delay * (1.0 + fp.fg_depth * (ph * std::f32::consts::TAU).sin() * 0.9)).max(0.05) * 0.001;
    let (fb, mix) = (fp.fg_fb.clamp(-0.97, 0.97), fp.fg_mix.clamp(0.0, 1.0));
    graph(
        ui,
        g,
        move |f| {
            let w = std::f32::consts::TAU * f * tau;
            // wet = z / (1 - fb z), z = e^{-jw}
            let (zr, zi) = (w.cos(), -w.sin());
            let (dr, di) = (1.0 - fb * zr, -fb * zi);
            let dd = (dr * dr + di * di).max(1e-6);
            let (wr, wi) = ((zr * dr + zi * di) / dd, (zi * dr - zr * di) / dd);
            let (hr, hi) = (1.0 + mix * wr, mix * wi);
            10.0 * (hr * hr + hi * hi).max(1e-8).log10()
        },
        (-30.0, 18.0),
        Some((&st.spec, st.spec_fs)),
    );
    let pa = ui.painter();
    caption(pa, g, format!("COMB FILTER  delay now {:.2} ms   (sweeps with the LFO)", tau * 1000.0));
}

/// Notches of the phaser (all-pass chain + dry), moving with its LFO.
fn phaser_view(ui: &mut Ui, g: Rect, fp: &crate::fx::FxPatch, st: &Gui) {
    let ph = live_phase(ui, fp.ph_rate);
    let fc = (fp.ph_freq * 2f32.powf(fp.ph_depth * 2.0 * (ph * std::f32::consts::TAU).sin())).clamp(40.0, 16000.0);
    let n = fp.ph_stages.clamp(2, 12);
    let mix = fp.ph_mix.clamp(0.0, 1.0);
    graph(
        ui,
        g,
        move |f| {
            let fs = 48000.0f32;
            let t = (std::f32::consts::PI * fc / fs).tan();
            let a = (t - 1.0) / (t + 1.0);
            let w = std::f32::consts::TAU * f / fs;
            let (zr, zi) = (w.cos(), -w.sin());
            // one all-pass stage: (a + z) / (1 + a z)
            let (nr, ni) = (a + zr, zi);
            let (dr, di) = (1.0 + a * zr, a * zi);
            let dd = (dr * dr + di * di).max(1e-9);
            let (sr, si) = ((nr * dr + ni * di) / dd, (ni * dr - nr * di) / dd);
            let (mut pr, mut pi) = (1.0f32, 0.0f32);
            for _ in 0..n {
                let (a2, b2) = (pr * sr - pi * si, pr * si + pi * sr);
                pr = a2;
                pi = b2;
            }
            // dry + wet through the all-pass chain: (1 - mix) + mix * P
            let (hr, hi) = ((1.0 - mix) + mix * pr, mix * pi);
            20.0 * (hr * hr + hi * hi).max(1e-8).sqrt().log10()
        },
        (-36.0, 6.0),
        Some((&st.spec, st.spec_fs)),
    );
    caption(ui.painter(), g, format!("{} notches   centre now {:.0} Hz   (sweeps with the LFO)", n / 2, fc));
}

/// Delay time of the chorus over two LFO cycles, with the live position.
fn chorus_view(ui: &mut Ui, g: Rect, fp: &crate::fx::FxPatch) {
    let ph = live_phase(ui, fp.ch_rate);
    dark_box(ui, g);
    let pa = ui.painter().with_clip_rect(g.shrink(1.0));
    let span = (fp.ch_delay * fp.ch_depth * 0.8).max(0.05);
    let (lo, hi) = ((fp.ch_delay - span).max(0.0), fp.ch_delay + span);
    let (ymin, ymax) = (0.0f32, (fp.ch_delay * 1.9).max(hi * 1.1).max(4.0));
    let yv = |ms: f32| g.bottom() - 18.0 - (g.height() - 40.0) * ((ms - ymin) / (ymax - ymin)).clamp(0.0, 1.0);
    let xv = |u: f32| g.left() + 14.0 + (g.width() - 28.0) * u;
    pa.rect_filled(Rect::from_min_max(Pos2::new(g.left() + 1.0, yv(hi)), Pos2::new(g.right() - 1.0, yv(lo))), CornerRadius::ZERO, Color32::from_rgb(20, 20, 20));
    pa.line_segment([Pos2::new(g.left(), yv(fp.ch_delay)), Pos2::new(g.right(), yv(fp.ch_delay))], Stroke::new(1.0_f32, Color32::from_rgb(40, 40, 40)));
    let f = |u: f32| fp.ch_delay + span * (u * 2.0 * std::f32::consts::TAU).sin();
    let pts: Vec<Pos2> = (0..=160).map(|i| Pos2::new(xv(i as f32 / 160.0), yv(f(i as f32 / 160.0)))).collect();
    pa.add(egui::Shape::line(pts, PathStroke::new(1.6_f32, ACCENT)));
    let u = ph * 0.5;
    let dot = Pos2::new(xv(u), yv(f(u)));
    pa.circle_filled(dot, 4.5, WHITE);
    pa.circle_stroke(dot, 7.0, Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(255, 255, 255, 110)));
    pa.text(Pos2::new(dot.x + 12.0, dot.y - 10.0), Align2::LEFT_CENTER, format!("{:.1} ms", f(u)), mono(9.0), WHITE);
    caption(&pa, g, format!("DELAY TIME  {:.1} - {:.1} ms around {:.1} ms   (higher delay = lower pitch)", lo, hi, fp.ch_delay));
}

/// Unison copies of the hyper effect: their detune positions move around, wider = more detune.
fn hyper_view(ui: &mut Ui, g: Rect, fp: &crate::fx::FxPatch) {
    let ph = live_phase(ui, fp.hy_rate);
    dark_box(ui, g);
    let pa = ui.painter().with_clip_rect(g.shrink(1.0));
    let n = fp.hy_uni.clamp(2, 8);
    let mid = g.center().y;
    pa.line_segment([Pos2::new(g.left() + 10.0, mid), Pos2::new(g.right() - 10.0, mid)], Stroke::new(1.0_f32, Color32::from_rgb(34, 34, 34)));
    let half = (g.width() - 60.0) * 0.5;
    let cx = g.center().x;
    pa.line_segment([Pos2::new(cx, g.top() + 24.0), Pos2::new(cx, g.bottom() - 10.0)], Stroke::new(1.0_f32, Color32::from_rgb(46, 46, 46)));
    for j in 0..n {
        let base = j as f32 / (n - 1) as f32 * 2.0 - 1.0;
        let wob = (ph * std::f32::consts::TAU + j as f32 * 1.7).sin() * 0.25;
        let pos = ((base + wob * base.abs().max(0.3)) * fp.hy_detune.clamp(0.0, 1.0)).clamp(-1.0, 1.0);
        let x = cx + pos * half;
        let h = (g.height() - 50.0) * (0.45 + 0.55 * (1.0 - base.abs() * 0.5));
        pa.line_segment([Pos2::new(x, mid + h * 0.5), Pos2::new(x, mid - h * 0.5)], Stroke::new(3.0_f32, Color32::from_rgb(230, 230, 230)));
        // dimension: the same copy mirrored a little to the other side
        let xd = cx - pos * half * fp.hy_dim.clamp(0.0, 1.0) * 0.6;
        pa.circle_filled(Pos2::new(xd, mid + h * 0.5 + 10.0), 2.5, Color32::from_rgb(130, 130, 130));
    }
    pa.text(Pos2::new(cx - half, g.bottom() - 10.0), Align2::LEFT_CENTER, "flat", mono(8.5), DIM);
    pa.text(Pos2::new(cx + half, g.bottom() - 10.0), Align2::RIGHT_CENTER, "sharp", mono(8.5), DIM);
    caption(&pa, g, format!("{n} COPIES drifting apart (detune)  -  dots = stereo dimension"));
}

/// Stereo picture of the utility: width, balance, mono, polarity.
fn utility_view(ui: &mut Ui, g: Rect, fp: &crate::fx::FxPatch) {
    dark_box(ui, g);
    let pa = ui.painter().with_clip_rect(g.shrink(1.0));
    let c = Pos2::new(g.center().x, g.center().y + 6.0);
    let r = (g.height() * 0.5 - 22.0).max(20.0);
    // L / R diamond
    let dia = [Pos2::new(c.x, c.y - r), Pos2::new(c.x + r, c.y), Pos2::new(c.x, c.y + r), Pos2::new(c.x - r, c.y), Pos2::new(c.x, c.y - r)];
    pa.add(egui::Shape::line(dia.to_vec(), PathStroke::new(1.0_f32, Color32::from_rgb(46, 46, 46))));
    pa.line_segment([Pos2::new(c.x, c.y - r), Pos2::new(c.x, c.y + r)], Stroke::new(1.0_f32, Color32::from_rgb(30, 30, 30)));
    pa.line_segment([Pos2::new(c.x - r, c.y), Pos2::new(c.x + r, c.y)], Stroke::new(1.0_f32, Color32::from_rgb(30, 30, 30)));
    for (t, dx, dy) in [("MID", 0.0, -r - 9.0), ("L", -r - 10.0, 0.0), ("R", r + 10.0, 0.0)] {
        pa.text(Pos2::new(c.x + dx, c.y + dy), Align2::CENTER_CENTER, t, mono(9.0), DIM);
    }
    // the sound: an ellipse as wide as the stereo width, shifted by the balance, as tall as the gain
    let gain = 10f32.powf(fp.ut_gain / 20.0).clamp(0.05, 2.0);
    let wdt = if fp.ut_mono { 0.0 } else { fp.ut_width.clamp(0.0, 2.0) };
    let (rx, ry) = (r * 0.55 * wdt * gain.min(1.4), r * 0.8 * gain.min(1.4));
    let ox = fp.ut_pan.clamp(-1.0, 1.0) * r * 0.5;
    let pts: Vec<Pos2> = (0..=64).map(|i| {
        let a = i as f32 / 64.0 * std::f32::consts::TAU;
        Pos2::new(c.x + ox + rx * a.cos(), c.y - ry * a.sin())
    }).collect();
    let col = if fp.ut_invert { Color32::from_rgb(200, 200, 200) } else { WHITE };
    if rx < 1.5 {
        pa.line_segment([Pos2::new(c.x + ox, c.y - ry), Pos2::new(c.x + ox, c.y + ry)], Stroke::new(3.0_f32, col));
    } else {
        pa.add(egui::Shape::line(pts, PathStroke::new(2.0_f32, col)));
    }
    caption(&pa, g, format!("STEREO FIELD   gain {:+.1} dB   width {:.0}%   balance {:+.0}%{}{}", fp.ut_gain, fp.ut_width * 100.0, fp.ut_pan * 100.0, if fp.ut_mono { "   MONO" } else { "" }, if fp.ut_invert { "   INVERTED" } else { "" }));
}
