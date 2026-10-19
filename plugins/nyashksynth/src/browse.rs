//! Generic browser panel: category list on the left, searchable item list on the right.
//! Used for presets, wavetables and noise samples.

use crate::ui::*;
use nih_plug_egui::egui::{self, Align2, Color32, CornerRadius, Pos2, Rect, Sense, Stroke, StrokeKind, Ui, Vec2};

pub struct BItem {
    pub name: String,
    pub cat: String,
    pub fav: bool,
    pub user: bool,
    pub rating: u8,
    pub tags: Vec<String>,
}

#[derive(Default)]
pub struct BState {
    /// Selected category ("" = everything, "\u{2605}" = favourites).
    pub cat: String,
    pub search: String,
    pub scroll: f32,
}

pub enum BAct {
    None,
    Pick(usize),
    Fav(usize),
    Rate(usize, u8),
    Close,
}

pub const FAV_CAT: &str = "\u{2605} Favorites";
pub const TOP_CAT: &str = "\u{2605} 4+ stars";

/// Rectangle of the footer area (below the lists) for caller-drawn controls.
pub fn footer(rect: Rect) -> Rect {
    Rect::from_min_max(Pos2::new(rect.left() + 10.0, rect.bottom() - 62.0), Pos2::new(rect.right() - 10.0, rect.bottom() - 8.0))
}

#[allow(clippy::too_many_arguments)]
pub fn browser(ui: &mut Ui, rect: Rect, title: &str, items: &[BItem], sel: Option<usize>, st: &mut BState, allow_fav: bool, id: &str) -> BAct {
    let mut act = BAct::None;
    ui.painter().rect_filled(rect, CornerRadius::same(8), Color32::from_rgb(8, 8, 8));
    ui.painter().rect_stroke(rect, CornerRadius::same(8), Stroke::new(1.0_f32, MID), StrokeKind::Inside);
    ui.painter().text(Pos2::new(rect.left() + 16.0, rect.top() + 22.0), Align2::LEFT_CENTER, title, mono(12.0), WHITE);
    if pill_tip(ui, Rect::from_min_size(Pos2::new(rect.right() - 44.0, rect.top() + 10.0), Vec2::new(32.0, 24.0)), "x", false, "Close (Esc)") || ui.input(|i| i.key_pressed(egui::Key::Escape)) && !ui.memory(|m| m.focused().is_some()) {
        act = BAct::Close;
    }

    // search box
    let sr = Rect::from_min_size(Pos2::new(rect.right() - 290.0, rect.top() + 10.0), Vec2::new(236.0, 24.0));
    let te = egui::TextEdit::singleline(&mut st.search).font(mono(12.0)).hint_text("search...").desired_width(sr.width()).margin(egui::Margin::symmetric(6, 4));
    ui.put(sr, te);

    let body = Rect::from_min_max(Pos2::new(rect.left() + 10.0, rect.top() + 46.0), Pos2::new(rect.right() - 10.0, rect.bottom() - 70.0));
    let cats_w = 190.0;
    let cat_r = Rect::from_min_size(body.min, Vec2::new(cats_w, body.height()));
    let list_r = Rect::from_min_max(Pos2::new(body.left() + cats_w + 8.0, body.top()), body.max);
    ui.painter().rect_stroke(cat_r, CornerRadius::same(4), Stroke::new(1.0_f32, LINE), StrokeKind::Inside);
    ui.painter().rect_stroke(list_r, CornerRadius::same(4), Stroke::new(1.0_f32, LINE), StrokeKind::Inside);

    // categories
    let mut cats: Vec<String> = items.iter().map(|i| i.cat.clone()).collect();
    cats.sort_by_key(|c| c.to_lowercase());
    cats.dedup();
    let mut cat_rows: Vec<(String, String)> = vec![(String::new(), format!("ALL  ({})", items.len()))];
    if allow_fav {
        cat_rows.push((FAV_CAT.to_string(), format!("{}  ({})", FAV_CAT, items.iter().filter(|i| i.fav).count())));
        cat_rows.push((TOP_CAT.to_string(), format!("{}  ({})", TOP_CAT, items.iter().filter(|i| i.rating >= 4).count())));
    }
    for c in cats {
        let n = items.iter().filter(|i| i.cat == c).count();
        cat_rows.push((c.clone(), format!("{}  ({})", c, n)));
    }
    if allow_fav {
        let mut tags: Vec<String> = items.iter().flat_map(|i| i.tags.iter().cloned()).collect();
        tags.sort();
        tags.dedup();
        for t in tags {
            let n = items.iter().filter(|i| i.tags.contains(&t)).count();
            cat_rows.push((format!("#{t}"), format!("# {t}  ({n})")));
        }
    }
    let cpt = ui.painter().with_clip_rect(cat_r.shrink(1.0));
    let row_h = 24.0;
    let cat_scroll_id = egui::Id::new(("bcatscroll", id));
    let mut cscroll: f32 = ui.data(|d| d.get_temp(cat_scroll_id)).unwrap_or(0.0);
    let total_c = cat_rows.len() as f32 * row_h;
    if ui.input(|i| i.pointer.hover_pos()).map(|p| cat_r.contains(p)).unwrap_or(false) {
        cscroll -= ui.input(|i| i.smooth_scroll_delta.y);
    }
    cscroll = cscroll.clamp(0.0, (total_c - cat_r.height()).max(0.0));
    ui.data_mut(|d| d.insert_temp(cat_scroll_id, cscroll));
    for (i, (key, label)) in cat_rows.iter().enumerate() {
        let rr = Rect::from_min_size(Pos2::new(cat_r.left() + 2.0, cat_r.top() + 2.0 + i as f32 * row_h - cscroll), Vec2::new(cat_r.width() - 4.0, row_h - 2.0));
        if rr.bottom() < cat_r.top() || rr.top() > cat_r.bottom() {
            continue;
        }
        let vis = rr.intersect(cat_r);
        let resp = ui.interact(vis, egui::Id::new(("bcat", id, i)), Sense::click());
        let on = st.cat == *key;
        if on {
            cpt.rect_filled(rr, CornerRadius::same(4), WHITE);
        } else if resp.hovered() {
            cpt.rect_filled(rr, CornerRadius::same(4), PANEL2);
        }
        cpt.text(Pos2::new(rr.left() + 8.0, rr.center().y), Align2::LEFT_CENTER, label, mono(11.0), if on { Color32::BLACK } else { WHITE });
        if resp.clicked() {
            st.cat = key.clone();
            st.scroll = 0.0;
        }
    }

    // items
    let q = st.search.to_lowercase();
    let vis_idx: Vec<usize> = items
        .iter()
        .enumerate()
        .filter(|(_, it)| {
            (st.cat.is_empty() || (st.cat == FAV_CAT && it.fav) || (st.cat == TOP_CAT && it.rating >= 4) || (st.cat.starts_with('#') && it.tags.iter().any(|t| *t == st.cat[1..])) || it.cat == st.cat) && (q.is_empty() || it.name.to_lowercase().contains(&q) || it.cat.to_lowercase().contains(&q) || it.tags.iter().any(|t| t.contains(&q)))
        })
        .map(|(i, _)| i)
        .collect();
    let lpt = ui.painter().with_clip_rect(list_r.shrink(1.0));
    let total = vis_idx.len() as f32 * row_h;
    let hovering = ui.input(|i| i.pointer.hover_pos()).map(|p| list_r.contains(p)).unwrap_or(false);
    if hovering {
        st.scroll -= ui.input(|i| i.smooth_scroll_delta.y);
    }
    // keyboard: up / down moves the selection
    let (up, down) = ui.input(|i| (i.key_pressed(egui::Key::ArrowUp), i.key_pressed(egui::Key::ArrowDown)));
    if up || down {
        if let Some(pos) = sel.and_then(|s| vis_idx.iter().position(|v| *v == s)) {
            let np = if down { (pos + 1).min(vis_idx.len().saturating_sub(1)) } else { pos.saturating_sub(1) };
            if np != pos {
                act = BAct::Pick(vis_idx[np]);
                let ny = np as f32 * row_h;
                if ny < st.scroll {
                    st.scroll = ny;
                } else if ny + row_h > st.scroll + list_r.height() {
                    st.scroll = ny + row_h - list_r.height();
                }
            }
        } else if let Some(first) = vis_idx.first() {
            act = BAct::Pick(*first);
        }
    }
    st.scroll = st.scroll.clamp(0.0, (total - list_r.height() + 4.0).max(0.0));
    for (n, &i) in vis_idx.iter().enumerate() {
        let rr = Rect::from_min_size(Pos2::new(list_r.left() + 2.0, list_r.top() + 2.0 + n as f32 * row_h - st.scroll), Vec2::new(list_r.width() - 4.0, row_h - 2.0));
        if rr.bottom() < list_r.top() || rr.top() > list_r.bottom() {
            continue;
        }
        let it = &items[i];
        let vis = rr.intersect(list_r);
        let resp = ui.interact(vis, egui::Id::new(("brow", id, i)), Sense::click());
        let on = sel == Some(i);
        if on {
            lpt.rect_filled(rr, CornerRadius::same(4), WHITE);
        } else if resp.hovered() {
            lpt.rect_filled(rr, CornerRadius::same(4), PANEL2);
        }
        let fg = if on { Color32::BLACK } else { WHITE };
        lpt.text(Pos2::new(rr.left() + 10.0, rr.center().y), Align2::LEFT_CENTER, &it.name, mono(12.0), fg);
        if allow_fav {
            // five clickable stars, then tags
            let mut rated: Option<u8> = None;
            for k in 0..5u8 {
                let sr = Rect::from_center_size(Pos2::new(rr.right() - 244.0 + 12.0 * k as f32, rr.center().y), Vec2::new(12.0, 18.0));
                let r = ui.interact(sr.intersect(list_r), egui::Id::new(("brate", id, i, k)), Sense::click());
                let filled = k < it.rating;
                lpt.text(sr.center(), Align2::CENTER_CENTER, if filled { "\u{2605}" } else { "\u{2606}" }, mono(9.5), if filled { if on { Color32::BLACK } else { WHITE } } else if r.hovered() { WHITE } else if on { Color32::from_rgb(120, 120, 120) } else { MID });
                if r.clicked() {
                    rated = Some(if it.rating == k + 1 { 0 } else { k + 1 });
                }
            }
            if let Some(v) = rated {
                act = BAct::Rate(i, v);
                continue;
            }
            if !it.tags.is_empty() {
                let t = it.tags.iter().take(3).map(|t| format!("#{t}")).collect::<Vec<_>>().join(" ");
                lpt.text(Pos2::new(rr.right() - 262.0, rr.center().y), Align2::RIGHT_CENTER, t, mono(9.0), if on { Color32::from_rgb(70, 70, 70) } else { DIM });
            }
        }
        if st.cat.is_empty() || st.cat == FAV_CAT {
            lpt.text(Pos2::new(rr.right() - 70.0, rr.center().y), Align2::RIGHT_CENTER, &it.cat, mono(9.5), if on { Color32::from_rgb(60, 60, 60) } else { DIM });
        }
        if it.user {
            lpt.text(Pos2::new(rr.right() - 34.0, rr.center().y), Align2::RIGHT_CENTER, "user", mono(9.0), if on { Color32::from_rgb(60, 60, 60) } else { MID });
        }
        if allow_fav {
            let star = Rect::from_center_size(Pos2::new(rr.right() - 14.0, rr.center().y), Vec2::splat(18.0));
            let sresp = ui.interact(star.intersect(list_r), egui::Id::new(("bstar", id, i)), Sense::click());
            lpt.text(star.center(), Align2::CENTER_CENTER, if it.fav { "\u{2605}" } else { "\u{2606}" }, mono(13.0), if it.fav { if on { Color32::BLACK } else { WHITE } } else if on { Color32::from_rgb(90, 90, 90) } else { MID });
            if sresp.clicked() {
                act = BAct::Fav(i);
                continue;
            }
        }
        if resp.clicked() {
            act = BAct::Pick(i);
        }
    }
    // scroll bar
    if total > list_r.height() {
        let th = (list_r.height() / total).clamp(0.08, 1.0) * list_r.height();
        let ty = list_r.top() + (list_r.height() - th) * (st.scroll / (total - list_r.height() + 4.0).max(1.0));
        ui.painter().rect_filled(Rect::from_min_size(Pos2::new(list_r.right() - 5.0, ty), Vec2::new(3.0, th)), CornerRadius::same(2), MID);
    }
    if vis_idx.is_empty() {
        ui.painter().text(list_r.center(), Align2::CENTER_CENTER, "nothing here", mono(11.0), DIM);
    }
    act
}
