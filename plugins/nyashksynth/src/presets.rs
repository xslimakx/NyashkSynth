//! Factory presets: lists of (parameter id, plain value). Everything else is reset to default.

use crate::params::{NLFO, N_SRC};
use crate::files;
use crate::shared::Shared;
use crate::SynthParams;
use nih_plug::prelude::*;
use std::path::PathBuf;

pub struct Preset {
    pub name: &'static str,
    pub cat: &'static str,
    pub values: &'static [(&'static str, f32)],
}

pub const PRESETS: &[Preset] = &[
    Preset { name: "Init",
        cat: "Init", values: &[] },
    Preset {
        name: "Pure Saw Lead",
        cat: "Lead",
        values: &[
            ("a_pos", 0.5), ("a_un", 5.0), ("a_dt", 0.28), ("fl_cut", 7000.0), ("fl_res", 0.12),
            ("e1_a", 2.0), ("e1_d", 400.0), ("e1_s", 0.8), ("e1_r", 180.0),
            ("ch_on", 1.0), ("ch_mx", 0.3), ("dl_on", 1.0), ("dl_mx", 0.18), ("dl_fb", 0.3),
        ],
    },
    Preset {
        name: "Supersaw Pad",
        cat: "Pad",
        values: &[
            ("a_pos", 0.5), ("a_un", 9.0), ("a_dt", 0.42), ("a_bl", 0.9), ("a_lv", 0.6),
            ("b_on", 1.0), ("b_pos", 0.5), ("b_un", 7.0), ("b_dt", 0.3), ("b_fn", 7.0), ("b_lv", 0.5),
            ("fl_cut", 3200.0), ("fl_res", 0.15),
            ("e1_a", 700.0), ("e1_d", 900.0), ("e1_s", 0.85), ("e1_r", 1600.0),
            ("ch_on", 1.0), ("ch_mx", 0.4), ("rv_on", 1.0), ("rv_mx", 0.3), ("rv_sz", 0.8),
        ],
    },
    Preset {
        name: "Sub Bass",
        cat: "Bass",
        values: &[
            ("a_pos", 0.0), ("a_lv", 0.8), ("sub_on", 1.0), ("sub_lv", 0.5), ("sub_wv", 1.0),
            ("fl_cut", 900.0), ("fl_res", 0.1), ("vm", 1.0), ("gl", 40.0),
            ("e1_a", 1.0), ("e1_d", 200.0), ("e1_s", 0.9), ("e1_r", 120.0),
        ],
    },
    Preset {
        name: "Reese Bass",
        cat: "Bass",
        values: &[
            ("a_pos", 0.5), ("a_un", 3.0), ("a_dt", 0.22), ("a_oc", -1.0),
            ("b_on", 1.0), ("b_pos", 0.5), ("b_un", 3.0), ("b_dt", 0.22), ("b_oc", -1.0), ("b_fn", 14.0),
            ("fl_cut", 650.0), ("fl_res", 0.3), ("fl_drv", 0.2),
            ("sh_1", 0.0), ("rt_1", 0.3), ("ms_1", 3.0), ("md_1", 9.0), ("ma_1", 0.2),
            ("ds_on", 1.0), ("ds_dr", 0.3), ("ds_mx", 0.6), ("vm", 1.0), ("gl", 60.0),
        ],
    },
    Preset {
        name: "Wobble Bass",
        cat: "Bass",
        values: &[
            ("a_pos", 0.5), ("a_un", 2.0), ("a_dt", 0.15), ("a_oc", -1.0), ("sub_on", 1.0), ("sub_lv", 0.4),
            ("fl_cut", 420.0), ("fl_res", 0.55), ("fl_drv", 0.3),
            ("sh_1", 0.0), ("sy_1", 1.0), ("dv_1", 6.0), ("ms_1", 3.0), ("md_1", 9.0), ("ma_1", 0.55),
            ("vm", 1.0), ("e1_s", 1.0),
        ],
    },
    Preset {
        name: "Fold Bass",
        cat: "Bass",
        values: &[
            ("a_tb", 2.0), ("a_pos", 0.25), ("a_oc", -1.0), ("fl_cut", 2400.0), ("fl_res", 0.2),
            ("ms_1", 1.0), ("md_1", 1.0), ("ma_1", 0.5), ("e2_a", 1.0), ("e2_d", 500.0), ("e2_s", 0.0),
            ("ds_on", 1.0), ("ds_ty", 2.0), ("ds_dr", 0.5), ("ds_mx", 0.7), ("vm", 1.0),
        ],
    },
    Preset {
        name: "Vowel Pluck",
        cat: "Pluck",
        values: &[
            ("a_tb", 3.0), ("a_pos", 0.05), ("a_un", 3.0), ("a_dt", 0.15),
            ("ms_1", 1.0), ("md_1", 1.0), ("ma_1", 0.6), ("e2_a", 1.0), ("e2_d", 450.0), ("e2_s", 0.0),
            ("e1_a", 1.0), ("e1_d", 450.0), ("e1_s", 0.0), ("e1_r", 260.0),
            ("fl_cut", 9000.0), ("dl_on", 1.0), ("dl_mx", 0.25), ("dl_sy", 1.0), ("dl_dv", 6.0), ("rv_on", 1.0), ("rv_mx", 0.22),
        ],
    },
    Preset {
        name: "Glass Bells",
        cat: "Keys",
        values: &[
            ("a_tb", 5.0), ("a_pos", 0.45), ("a_un", 2.0), ("a_dt", 0.08),
            ("ms_1", 1.0), ("md_1", 1.0), ("ma_1", -0.35), ("e2_a", 1.0), ("e2_d", 2500.0), ("e2_s", 0.0),
            ("e1_a", 1.0), ("e1_d", 2200.0), ("e1_s", 0.0), ("e1_r", 1200.0),
            ("fl_cut", 12000.0), ("rv_on", 1.0), ("rv_mx", 0.35), ("rv_sz", 0.85),
        ],
    },
    Preset {
        name: "Comb Drift Pad",
        cat: "Pad",
        values: &[
            ("a_tb", 4.0), ("a_pos", 0.4), ("a_un", 5.0), ("a_dt", 0.3), ("a_lv", 0.65),
            ("sh_1", 6.0), ("rt_1", 0.18), ("ms_1", 3.0), ("md_1", 1.0), ("ma_1", 0.5),
            ("e1_a", 900.0), ("e1_d", 1000.0), ("e1_s", 0.9), ("e1_r", 1800.0),
            ("fl_cut", 5500.0), ("ch_on", 1.0), ("rv_on", 1.0), ("rv_mx", 0.35),
        ],
    },
    Preset {
        name: "PWM Lead",
        cat: "Lead",
        values: &[
            ("a_tb", 1.0), ("a_pos", 0.3), ("a_un", 3.0), ("a_dt", 0.18),
            ("sh_1", 1.0), ("rt_1", 0.8), ("ms_1", 3.0), ("md_1", 1.0), ("ma_1", 0.45),
            ("fl_cut", 5200.0), ("fl_res", 0.15), ("e1_a", 3.0), ("e1_s", 0.85), ("e1_r", 200.0),
            ("dl_on", 1.0), ("dl_mx", 0.2),
        ],
    },
    Preset {
        name: "Organ Keys",
        cat: "Keys",
        values: &[
            ("a_tb", 6.0), ("a_pos", 0.4), ("a_un", 2.0), ("a_dt", 0.06),
            ("e1_a", 3.0), ("e1_d", 100.0), ("e1_s", 1.0), ("e1_r", 90.0),
            ("fl_cut", 9000.0), ("ch_on", 1.0), ("ch_mx", 0.35), ("rv_on", 1.0), ("rv_mx", 0.15),
        ],
    },
    Preset {
        name: "Digital Rise",
        cat: "FX",
        values: &[
            ("a_tb", 7.0), ("a_pos", 0.0), ("a_un", 4.0), ("a_dt", 0.2),
            ("ms_1", 1.0), ("md_1", 1.0), ("ma_1", 1.0), ("e2_a", 4000.0), ("e2_d", 2000.0), ("e2_s", 1.0),
            ("fl_ty", 1.0), ("fl_cut", 700.0), ("ms_2", 1.0), ("md_2", 9.0), ("ma_2", 0.7),
            ("e1_a", 1500.0), ("e1_s", 1.0), ("e1_r", 900.0), ("rv_on", 1.0), ("rv_mx", 0.3),
        ],
    },
    Preset {
        name: "Sync Screamer",
        cat: "Lead",
        values: &[
            ("a_pos", 0.5), ("a_wm", 5.0), ("a_wa", 0.3), ("a_un", 2.0), ("a_dt", 0.12),
            ("ms_1", 1.0), ("md_1", 7.0), ("ma_1", 0.7), ("e2_a", 1.0), ("e2_d", 600.0), ("e2_s", 0.2),
            ("fl_cut", 6000.0), ("fl_res", 0.2), ("e1_s", 0.85), ("e1_r", 160.0), ("vm", 1.0), ("gl", 80.0),
        ],
    },
    Preset {
        name: "Point Filter: Vowel",
        cat: "Pad",
        values: &[
            ("a_pos", 0.3), ("a_un", 3.0), ("a_dt", 0.15), ("e1_a", 30.0), ("e1_r", 500.0),
            ("fl_ty", 16.0), ("fl_res", 0.0),
            ("cbon_1", 1.0), ("cbty_1", 3.0), ("cbfr_1", 120.0), ("cbq_1", 0.8),
            ("cbon_2", 1.0), ("cbty_2", 0.0), ("cbfr_2", 700.0), ("cbgn_2", 14.0), ("cbq_2", 4.0),
            ("cbon_3", 1.0), ("cbty_3", 0.0), ("cbfr_3", 1200.0), ("cbgn_3", 10.0), ("cbq_3", 5.0),
            ("cbon_4", 1.0), ("cbty_4", 0.0), ("cbfr_4", 2600.0), ("cbgn_4", 8.0), ("cbq_4", 6.0),
            ("cbon_5", 1.0), ("cbty_5", 4.0), ("cbfr_5", 7000.0), ("cbq_5", 0.8),
        ],
    },
    Preset {
        name: "Point Filter: Telephone",
        cat: "Lead",
        values: &[
            ("a_pos", 0.6), ("a_wm", 0.0), ("e1_a", 3.0), ("e1_r", 200.0),
            ("fl_ty", 16.0), ("fl_res", 0.0),
            ("cbon_1", 1.0), ("cbty_1", 3.0), ("cbfr_1", 450.0), ("cbq_1", 1.6),
            ("cbon_2", 1.0), ("cbty_2", 4.0), ("cbfr_2", 3200.0), ("cbq_2", 1.6),
            ("cbon_3", 1.0), ("cbty_3", 0.0), ("cbfr_3", 1500.0), ("cbgn_3", 9.0), ("cbq_3", 2.0),
        ],
    },
    Preset {
        name: "Point Filter: Scooped Pad",
        cat: "Pad",
        values: &[
            ("a_pos", 0.2), ("a_un", 5.0), ("a_dt", 0.25), ("e1_a", 400.0), ("e1_r", 1200.0),
            ("fl_ty", 16.0), ("fl_res", 0.0),
            ("cbon_1", 1.0), ("cbty_1", 1.0), ("cbfr_1", 180.0), ("cbgn_1", 7.0),
            ("cbon_2", 1.0), ("cbty_2", 0.0), ("cbfr_2", 900.0), ("cbgn_2", -14.0), ("cbq_2", 0.7),
            ("cbon_3", 1.0), ("cbty_3", 2.0), ("cbfr_3", 6500.0), ("cbgn_3", 8.0),
            ("cbon_4", 1.0), ("cbty_4", 5.0), ("cbfr_4", 3000.0), ("cbq_4", 3.0),
        ],
    },
    Preset {
        name: "Dream Keys",
        cat: "Keys",
        values: &[
            ("a_pos", 0.12), ("a_un", 3.0), ("a_dt", 0.12), ("b_on", 1.0), ("b_tb", 6.0), ("b_pos", 0.7), ("b_lv", 0.3), ("b_oc", 1.0),
            ("fl_cut", 4200.0), ("e1_a", 6.0), ("e1_d", 1200.0), ("e1_s", 0.35), ("e1_r", 900.0),
            ("ch_on", 1.0), ("dl_on", 1.0), ("dl_mx", 0.22), ("dl_fb", 0.4), ("rv_on", 1.0), ("rv_mx", 0.3), ("rv_sz", 0.8),
        ],
    },
    Preset {
        name: "Brass Stab",
        cat: "Brass",
        values: &[
            ("a_pos", 0.5), ("a_un", 4.0), ("a_dt", 0.14), ("b_on", 1.0), ("b_pos", 0.5), ("b_fn", 9.0), ("b_un", 3.0),
            ("fl_cut", 450.0), ("fl_res", 0.2), ("ms_1", 1.0), ("md_1", 9.0), ("ma_1", 0.55), ("e2_a", 60.0), ("e2_d", 350.0), ("e2_s", 0.35),
            ("e1_a", 25.0), ("e1_d", 500.0), ("e1_s", 0.75), ("e1_r", 200.0), ("rv_on", 1.0), ("rv_mx", 0.18),
        ],
    },

    Preset {
        name: "Growl Bass",
        cat: "Bass",
        values: &[
            ("a_pos", 0.5), ("a_oc", -1.0), ("a_wm", 5.0), ("a_wa", 0.25), ("a_un", 3.0), ("a_dt", 0.2),
            ("sub_on", 1.0), ("sub_lv", 0.45), ("fl_ty", 6.0), ("fl_cut", 520.0), ("fl_res", 0.45), ("fl_drv", 0.35),
            ("sh_1", 6.0), ("rt_1", 2.5), ("ms_1", 3.0), ("md_1", 7.0), ("ma_1", 0.45),
            ("ms_2", 3.0), ("md_2", 9.0), ("ma_2", 0.25), ("vm", 1.0), ("gl", 30.0),
            ("ds_on", 1.0), ("ds_ty", 4.0), ("ds_dr", 0.35), ("ds_mx", 0.7),
        ],
    },
    Preset {
        name: "Acid Squelch",
        cat: "Bass",
        values: &[
            ("a_pos", 0.5), ("a_oc", -1.0), ("fl_ty", 6.0), ("fl_cut", 260.0), ("fl_res", 0.78), ("fl_drv", 0.25),
            ("ms_1", 1.0), ("md_1", 9.0), ("ma_1", 0.55), ("e2_a", 1.0), ("e2_d", 180.0), ("e2_s", 0.0),
            ("e1_a", 1.0), ("e1_d", 250.0), ("e1_s", 0.6), ("e1_r", 90.0), ("vm", 2.0), ("gl", 55.0),
        ],
    },
    Preset {
        name: "Sub Pressure",
        cat: "Bass",
        values: &[
            ("a_pos", 0.0), ("a_oc", -1.0), ("a_lv", 0.8), ("sub_on", 1.0), ("sub_oc", -1.0), ("sub_lv", 0.5),
            ("fl_cut", 600.0), ("fl_drv", 0.3), ("e1_a", 1.0), ("e1_d", 400.0), ("e1_s", 1.0), ("e1_r", 80.0), ("vm", 1.0),
            ("ds_on", 1.0), ("ds_ty", 11.0), ("ds_dr", 0.2), ("ds_mx", 0.5), ("eq_on", 1.0), ("eq_lf", 90.0), ("eq_lg", 3.0), ("eq_mg", -2.0), ("eq_mf", 350.0),
        ],
    },
    Preset {
        name: "Rubber Bass",
        cat: "Bass",
        values: &[
            ("a_tb", 2.0), ("a_pos", 0.35), ("a_oc", -1.0), ("fl_cut", 380.0), ("fl_res", 0.3),
            ("ms_1", 1.0), ("md_1", 9.0), ("ma_1", 0.5), ("e2_a", 1.0), ("e2_d", 220.0), ("e2_s", 0.1),
            ("e1_a", 1.0), ("e1_d", 380.0), ("e1_s", 0.0), ("e1_r", 120.0), ("cp_on", 1.0), ("cp_th", -20.0), ("cp_rt", 4.0), ("cp_gn", 4.0),
        ],
    },
    Preset {
        name: "Hyper Lead",
        cat: "Lead",
        values: &[
            ("a_pos", 0.5), ("a_un", 7.0), ("a_dt", 0.35), ("a_um", 1.0), ("a_bl", 0.8), ("fl_cut", 9000.0), ("fl_res", 0.1),
            ("e1_a", 2.0), ("e1_d", 300.0), ("e1_s", 0.85), ("e1_r", 220.0),
            ("hy_on", 1.0), ("hy_mx", 0.5), ("hy_un", 5.0), ("hy_dm", 0.5), ("dl_on", 1.0), ("dl_mx", 0.2), ("dl_sy", 1.0), ("dl_dv", 6.0), ("rv_on", 1.0), ("rv_mx", 0.2),
        ],
    },
    Preset {
        name: "Mono Glide Lead",
        cat: "Lead",
        values: &[
            ("a_pos", 0.55), ("a_un", 2.0), ("a_dt", 0.1), ("b_on", 1.0), ("b_pos", 0.25), ("b_lv", 0.4), ("b_oc", 0.0), ("b_fn", 6.0),
            ("fl_ty", 1.0), ("fl_cut", 4200.0), ("fl_res", 0.2), ("vm", 2.0), ("gl", 90.0),
            ("sh_1", 0.0), ("rt_1", 5.5), ("ri_1", 600.0), ("tg_1", 1.0), ("ms_1", 3.0), ("md_1", 5.0), ("ma_1", 0.04),
            ("e1_a", 5.0), ("e1_s", 0.9), ("e1_r", 200.0), ("dl_on", 1.0), ("dl_mx", 0.25),
        ],
    },
    Preset {
        name: "Square Pulse Lead",
        cat: "Lead",
        values: &[
            ("a_pos", 0.75), ("a_wm", 4.0), ("a_wa", 0.2), ("a_un", 3.0), ("a_dt", 0.15),
            ("sh_1", 1.0), ("rt_1", 3.0), ("ms_1", 3.0), ("md_1", 7.0), ("ma_1", 0.3),
            ("fl_cut", 6500.0), ("e1_a", 2.0), ("e1_s", 0.8), ("e1_r", 120.0), ("ch_on", 1.0), ("ch_mx", 0.3),
        ],
    },
    Preset {
        name: "Formant Lead",
        cat: "Lead",
        values: &[
            ("a_pos", 0.5), ("a_un", 3.0), ("a_dt", 0.2), ("fl_ty", 15.0), ("fl_cut", 160.0), ("fl_res", 0.55),
            ("ms_1", 3.0), ("md_1", 9.0), ("ma_1", 0.5), ("sh_1", 6.0), ("rt_1", 0.5),
            ("ms_2", 9.0), ("md_2", 9.0), ("ma_2", 0.2), ("e1_a", 6.0), ("e1_s", 0.9), ("e1_r", 250.0), ("rv_on", 1.0), ("rv_mx", 0.2),
        ],
    },
    Preset {
        name: "Warm Pad",
        cat: "Pad",
        values: &[
            ("a_pos", 0.4), ("a_un", 6.0), ("a_dt", 0.28), ("a_lv", 0.6), ("b_on", 1.0), ("b_tb", 1.0), ("b_pos", 0.2), ("b_un", 4.0), ("b_dt", 0.2), ("b_lv", 0.4),
            ("fl_cut", 2400.0), ("fl_res", 0.12), ("fl_kt", 0.4), ("e1_a", 900.0), ("e1_d", 1200.0), ("e1_s", 0.85), ("e1_r", 1800.0),
            ("sh_1", 6.0), ("rt_1", 0.12), ("ms_1", 3.0), ("md_1", 9.0), ("ma_1", 0.15),
            ("ch_on", 1.0), ("ch_mx", 0.35), ("rv_on", 1.0), ("rv_sz", 0.85), ("rv_mx", 0.3),
        ],
    },
    Preset {
        name: "Phaser Strings",
        cat: "Pad",
        values: &[
            ("a_pos", 0.5), ("a_un", 8.0), ("a_dt", 0.38), ("a_bl", 0.9), ("fl_cut", 5200.0), ("e1_a", 450.0), ("e1_d", 800.0), ("e1_s", 0.9), ("e1_r", 1100.0),
            ("ph_on", 1.0), ("ph_rt", 0.25), ("ph_dp", 0.7), ("ph_fb", 0.45), ("ph_mx", 0.5), ("rv_on", 1.0), ("rv_mx", 0.28),
        ],
    },
    Preset {
        name: "Glass Pad",
        cat: "Pad",
        values: &[
            ("a_tb", 5.0), ("a_pos", 0.3), ("a_un", 5.0), ("a_dt", 0.25), ("a_lv", 0.55), ("fl_cut", 14000.0),
            ("sh_1", 6.0), ("rt_1", 0.2), ("ms_1", 3.0), ("md_1", 1.0), ("ma_1", 0.35),
            ("e1_a", 1100.0), ("e1_d", 1500.0), ("e1_s", 0.8), ("e1_r", 2600.0), ("ch_on", 1.0), ("rv_on", 1.0), ("rv_sz", 0.95), ("rv_mx", 0.45), ("rv_lc", 300.0),
        ],
    },
    Preset {
        name: "Dark Atmosphere",
        cat: "Pad",
        values: &[
            ("a_tb", 7.0), ("a_pos", 0.2), ("a_oc", -1.0), ("a_un", 4.0), ("a_dt", 0.3), ("noi_lv", 0.35), ("noi_ty", 10.0),
            ("fl_ty", 1.0), ("fl_cut", 900.0), ("fl_res", 0.2), ("sh_1", 6.0), ("rt_1", 0.08), ("ms_1", 3.0), ("md_1", 9.0), ("ma_1", 0.3),
            ("e1_a", 2500.0), ("e1_d", 2000.0), ("e1_s", 0.9), ("e1_r", 3500.0), ("rv_on", 1.0), ("rv_sz", 0.95), ("rv_mx", 0.4),
        ],
    },
    Preset {
        name: "Soft Pluck",
        cat: "Pluck",
        values: &[
            ("a_pos", 0.5), ("a_un", 3.0), ("a_dt", 0.1), ("fl_cut", 800.0), ("fl_res", 0.15),
            ("ms_1", 1.0), ("md_1", 9.0), ("ma_1", 0.6), ("e2_a", 1.0), ("e2_d", 260.0), ("e2_s", 0.0),
            ("e1_a", 1.0), ("e1_d", 420.0), ("e1_s", 0.0), ("e1_r", 260.0), ("dl_on", 1.0), ("dl_mx", 0.22), ("rv_on", 1.0), ("rv_mx", 0.2),
        ],
    },
    Preset {
        name: "Kalimba",
        cat: "Pluck",
        values: &[
            ("a_tb", 5.0), ("a_pos", 0.2), ("a_oc", 1.0), ("a_un", 1.0), ("ms_1", 1.0), ("md_1", 1.0), ("ma_1", -0.15), ("e2_a", 1.0), ("e2_d", 300.0), ("e2_s", 0.0),
            ("e1_a", 1.0), ("e1_d", 900.0), ("e1_s", 0.0), ("e1_r", 500.0), ("fl_cut", 10000.0), ("rv_on", 1.0), ("rv_mx", 0.28),
        ],
    },
    Preset {
        name: "Pizzicato",
        cat: "Pluck",
        values: &[
            ("a_tb", 2.0), ("a_pos", 0.4), ("fl_ty", 1.0), ("fl_cut", 1600.0), ("fl_res", 0.1), ("ms_1", 1.0), ("md_1", 9.0), ("ma_1", 0.35), ("e2_a", 1.0), ("e2_d", 130.0), ("e2_s", 0.0),
            ("e1_a", 1.0), ("e1_d", 200.0), ("e1_s", 0.0), ("e1_r", 120.0), ("rv_on", 1.0), ("rv_mx", 0.14),
        ],
    },
    Preset {
        name: "FM Electric Piano",
        cat: "Keys",
        values: &[
            ("a_tb", 5.0), ("a_pos", 0.18), ("a_un", 2.0), ("a_dt", 0.05), ("ms_1", 7.0), ("md_1", 1.0), ("ma_1", 0.3),
            ("e1_a", 1.0), ("e1_d", 1800.0), ("e1_s", 0.0), ("e1_r", 300.0), ("fl_cut", 9000.0), ("ch_on", 1.0), ("ch_mx", 0.3), ("rv_on", 1.0), ("rv_mx", 0.18),
        ],
    },
    Preset {
        name: "Clav Comb",
        cat: "Keys",
        values: &[
            ("a_tb", 4.0), ("a_pos", 0.55), ("fl_ty", 2.0), ("fl_cut", 280.0), ("ms_1", 1.0), ("md_1", 1.0), ("ma_1", -0.4), ("e2_a", 1.0), ("e2_d", 220.0), ("e2_s", 0.0),
            ("e1_a", 1.0), ("e1_d", 500.0), ("e1_s", 0.0), ("e1_r", 90.0), ("ph_on", 1.0), ("ph_mx", 0.3), ("ph_rt", 0.4),
        ],
    },
    Preset {
        name: "Choir Ahh",
        cat: "Vocal",
        values: &[
            ("a_tb", 3.0), ("a_pos", 0.0), ("a_un", 5.0), ("a_dt", 0.18), ("a_lv", 0.6), ("ms_1", 3.0), ("md_1", 1.0), ("ma_1", 0.0),
            ("fl_cut", 7000.0), ("e1_a", 600.0), ("e1_d", 600.0), ("e1_s", 0.9), ("e1_r", 900.0), ("ch_on", 1.0), ("rv_on", 1.0), ("rv_sz", 0.85), ("rv_mx", 0.35),
        ],
    },
    Preset {
        name: "Vowel Morph",
        cat: "Vocal",
        values: &[
            ("a_tb", 3.0), ("a_pos", 0.0), ("a_un", 3.0), ("a_dt", 0.12), ("sh_1", 0.0), ("rt_1", 0.35), ("ms_1", 3.0), ("md_1", 1.0), ("ma_1", 1.0),
            ("fl_cut", 8000.0), ("e1_a", 40.0), ("e1_s", 0.9), ("e1_r", 300.0), ("dl_on", 1.0), ("dl_mx", 0.2), ("rv_on", 1.0), ("rv_mx", 0.2),
        ],
    },
    Preset {
        name: "Wavetable Morph",
        cat: "Synth",
        values: &[
            ("a_tb", 2.0), ("a_pos", 0.0), ("a_un", 4.0), ("a_dt", 0.2), ("sh_1", 1.0), ("rt_1", 0.25), ("ms_1", 3.0), ("md_1", 1.0), ("ma_1", 1.0),
            ("fl_cut", 5200.0), ("e1_a", 25.0), ("e1_s", 0.85), ("e1_r", 400.0), ("ch_on", 1.0), ("dl_on", 1.0), ("dl_mx", 0.2),
        ],
    },
    Preset {
        name: "Retro Poly",
        cat: "Synth",
        values: &[
            ("a_pos", 0.5), ("a_un", 2.0), ("a_dt", 0.08), ("b_on", 1.0), ("b_pos", 0.75), ("b_lv", 0.4), ("fl_cut", 1800.0), ("fl_res", 0.3), ("fl_kt", 0.5),
            ("ms_1", 1.0), ("md_1", 9.0), ("ma_1", 0.3), ("e2_a", 1.0), ("e2_d", 500.0), ("e2_s", 0.3),
            ("e1_a", 8.0), ("e1_d", 600.0), ("e1_s", 0.7), ("e1_r", 300.0), ("ch_on", 1.0), ("ch_mx", 0.45),
        ],
    },
    Preset {
        name: "Wind Sweep",
        cat: "FX",
        values: &[
            ("a_on", 0.0), ("noi_lv", 0.7), ("noi_ty", 10.0), ("fl_ty", 4.0), ("fl_cut", 700.0), ("fl_res", 0.5),
            ("sh_1", 6.0), ("rt_1", 0.15), ("ms_1", 3.0), ("md_1", 9.0), ("ma_1", 0.5), ("e1_a", 1500.0), ("e1_s", 1.0), ("e1_r", 2500.0), ("rv_on", 1.0), ("rv_mx", 0.35),
        ],
    },
    Preset {
        name: "Vinyl Dust",
        cat: "FX",
        values: &[
            ("a_on", 0.0), ("noi_lv", 0.6), ("noi_ty", 8.0), ("fl_cut", 9000.0), ("e1_a", 20.0), ("e1_s", 1.0), ("e1_r", 300.0), ("fl_n", 1.0),
        ],
    },
    Preset {
        name: "Noise Riser",
        cat: "FX",
        values: &[
            ("a_on", 0.0), ("noi_lv", 0.8), ("noi_ty", 0.0), ("fl_ty", 4.0), ("fl_cut", 300.0), ("fl_res", 0.4),
            ("ms_1", 1.0), ("md_1", 9.0), ("ma_1", 1.0), ("e2_a", 6000.0), ("e2_d", 100.0), ("e2_s", 1.0),
            ("e1_a", 3000.0), ("e1_s", 1.0), ("e1_r", 400.0), ("rv_on", 1.0), ("rv_mx", 0.3),
        ],
    },
    Preset {
        name: "Laser Zap",
        cat: "FX",
        values: &[
            ("a_pos", 0.5), ("a_wm", 5.0), ("a_wa", 0.6), ("ms_1", 1.0), ("md_1", 5.0), ("ma_1", 0.9), ("e2_a", 1.0), ("e2_d", 280.0), ("e2_s", 0.0),
            ("e1_a", 1.0), ("e1_d", 380.0), ("e1_s", 0.0), ("e1_r", 120.0), ("dl_on", 1.0), ("dl_mx", 0.3), ("dl_fb", 0.4),
        ],
    },
    Preset {
        name: "Tape Hiss Bed",
        cat: "FX",
        values: &[
            ("a_on", 0.0), ("noi_lv", 0.5), ("noi_ty", 9.0), ("e1_a", 100.0), ("e1_s", 1.0), ("e1_r", 400.0), ("fl_cut", 12000.0),
        ],
    },
    Preset {
        name: "Fat Brass",
        cat: "Brass",
        values: &[
            ("a_pos", 0.5), ("a_un", 5.0), ("a_dt", 0.16), ("b_on", 1.0), ("b_pos", 0.5), ("b_fn", 7.0), ("b_un", 4.0), ("b_lv", 0.55),
            ("fl_ty", 1.0), ("fl_cut", 500.0), ("fl_res", 0.2), ("ms_1", 1.0), ("md_1", 9.0), ("ma_1", 0.5), ("e2_a", 90.0), ("e2_d", 400.0), ("e2_s", 0.4),
            ("e1_a", 40.0), ("e1_d", 400.0), ("e1_s", 0.8), ("e1_r", 220.0), ("rv_on", 1.0), ("rv_mx", 0.15), ("cp_on", 1.0), ("cp_th", -16.0),
        ],
    },
    Preset {
        name: "Chorused Saw",
        cat: "Synth",
        values: &[
            ("a_pos", 0.5), ("a_un", 3.0), ("a_dt", 0.12), ("fl_cut", 7500.0), ("e1_a", 4.0), ("e1_s", 0.9), ("e1_r", 200.0),
            ("ch_on", 1.0), ("ch_mx", 0.5), ("ch_dp", 0.6), ("hy_on", 1.0), ("hy_mx", 0.25), ("hy_dm", 0.5),
        ],
    },
    // ---------------------------------------------------------------- v2.1: arpeggiator, effect modulation, slopes
    Preset {
        name: "Arp Pluck",
        cat: "Arp",
        values: &[
            ("a_tb", 2.0), ("a_pos", 0.3), ("a_un", 2.0), ("a_dt", 0.12), ("fl_cut", 2600.0), ("fl_res", 0.2),
            ("e1_a", 1.0), ("e1_d", 260.0), ("e1_s", 0.0), ("e1_r", 200.0),
            ("ms_1", 1.0), ("md_1", 9.0), ("ma_1", 0.4), ("e2_a", 1.0), ("e2_d", 220.0), ("e2_s", 0.0),
            ("ar_on", 1.0), ("ar_md", 0.0), ("ar_dv", 3.0), ("ar_oc", 2.0), ("ar_gt", 0.5),
            ("dl_on", 1.0), ("dl_mx", 0.25), ("dl_sy", 1.0), ("dl_dv", 7.0), ("dl_fb", 0.4), ("rv_on", 1.0), ("rv_mx", 0.2),
        ],
    },
    Preset {
        name: "Trance Gate Chords",
        cat: "Arp",
        values: &[
            ("a_pos", 0.5), ("a_un", 7.0), ("a_dt", 0.35), ("b_on", 1.0), ("b_pos", 0.5), ("b_un", 5.0), ("b_fn", 9.0), ("b_lv", 0.5),
            ("fl_cut", 5200.0), ("fl_res", 0.1), ("e1_a", 3.0), ("e1_d", 200.0), ("e1_s", 0.9), ("e1_r", 120.0),
            ("ar_on", 1.0), ("ar_md", 6.0), ("ar_dv", 3.0), ("ar_gt", 0.35),
            ("hy_on", 1.0), ("hy_mx", 0.4), ("rv_on", 1.0), ("rv_mx", 0.22), ("rv_sz", 0.7),
        ],
    },
    Preset {
        name: "Up Down Bell Arp",
        cat: "Arp",
        values: &[
            ("a_tb", 5.0), ("a_pos", 0.4), ("fl_cut", 9000.0), ("e1_a", 1.0), ("e1_d", 700.0), ("e1_s", 0.0), ("e1_r", 500.0),
            ("ar_on", 1.0), ("ar_md", 2.0), ("ar_dv", 3.0), ("ar_oc", 2.0), ("ar_gt", 0.6), ("ar_sw", 0.25),
            ("dl_on", 1.0), ("dl_mx", 0.3), ("dl_sy", 1.0), ("dl_dv", 8.0), ("dl_fb", 0.45), ("rv_on", 1.0), ("rv_mx", 0.35), ("rv_sz", 0.8),
        ],
    },
    Preset {
        name: "Pump Pad",
        cat: "Pad",
        values: &[
            ("a_pos", 0.5), ("a_un", 8.0), ("a_dt", 0.4), ("a_lv", 0.65), ("b_on", 1.0), ("b_pos", 0.45), ("b_un", 6.0), ("b_fn", -8.0), ("b_lv", 0.5),
            ("fl_cut", 4200.0), ("e1_a", 500.0), ("e1_d", 500.0), ("e1_s", 1.0), ("e1_r", 1200.0),
            ("sh_1", 3.0), ("sy_1", 1.0), ("dv_1", 9.0), ("tg_1", 0.0), ("ms_1", 3.0), ("md_1", 14.0), ("ma_1", -0.7), ("mu_1", 1.0),
            ("rv_on", 1.0), ("rv_mx", 0.3), ("rv_sz", 0.85), ("ch_on", 1.0), ("ch_mx", 0.3),
        ],
    },
    Preset {
        name: "Snappy Slope Pluck",
        cat: "Pluck",
        values: &[
            ("a_tb", 1.0), ("a_pos", 0.2), ("a_un", 3.0), ("a_dt", 0.16), ("fl_cut", 2200.0), ("fl_res", 0.25),
            ("e1_a", 1.0), ("e1_d", 420.0), ("e1_s", 0.0), ("e1_r", 300.0), ("e1_dc", -0.6),
            ("ms_1", 1.0), ("md_1", 9.0), ("ma_1", 0.55), ("e2_a", 1.0), ("e2_d", 300.0), ("e2_s", 0.0), ("e2_dc", -0.7),
            ("dl_on", 1.0), ("dl_mx", 0.2), ("rv_on", 1.0), ("rv_mx", 0.18),
        ],
    },
    Preset {
        name: "Slow Bloom",
        cat: "Pad",
        values: &[
            ("a_pos", 0.4), ("a_un", 8.0), ("a_dt", 0.38), ("b_on", 1.0), ("b_tb", 3.0), ("b_pos", 0.2), ("b_lv", 0.45), ("fl_cut", 1800.0), ("fl_res", 0.15),
            ("e1_a", 1800.0), ("e1_ac", 0.55), ("e1_d", 800.0), ("e1_s", 0.9), ("e1_r", 2200.0), ("e1_rc", -0.3),
            ("ms_1", 1.0), ("md_1", 9.0), ("ma_1", 0.35), ("e2_a", 2500.0), ("e2_d", 2000.0), ("e2_s", 0.6),
            ("sh_1", 0.0), ("rt_1", 0.25), ("ld_1", 1500.0), ("tg_1", 1.0), ("ms_2", 3.0), ("md_2", 1.0), ("ma_2", 0.2),
            ("rv_on", 1.0), ("rv_mx", 0.4), ("rv_sz", 0.9), ("hy_on", 1.0), ("hy_mx", 0.35),
        ],
    },
    Preset {
        name: "Delayed Vibrato Lead",
        cat: "Lead",
        values: &[
            ("a_pos", 0.55), ("a_un", 3.0), ("a_dt", 0.14), ("fl_cut", 6000.0), ("fl_res", 0.18), ("vm", 2.0), ("gl", 70.0),
            ("e1_a", 6.0), ("e1_d", 300.0), ("e1_s", 0.85), ("e1_r", 250.0),
            ("sh_1", 0.0), ("rt_1", 5.5), ("tg_1", 1.0), ("ld_1", 450.0), ("ri_1", 600.0), ("ms_1", 3.0), ("md_1", 5.0), ("ma_1", 0.045),
            ("ms_2", 9.0), ("md_2", 5.0), ("ma_2", 0.08), ("dl_on", 1.0), ("dl_mx", 0.2), ("dl_sy", 1.0), ("dl_dv", 7.0), ("rv_on", 1.0), ("rv_mx", 0.2),
        ],
    },
    Preset {
        name: "Triplet Wobble",
        cat: "Bass",
        values: &[
            ("a_pos", 0.5), ("a_un", 3.0), ("a_dt", 0.2), ("a_oc", -1.0), ("sub_on", 1.0), ("sub_lv", 0.45),
            ("fl_cut", 380.0), ("fl_res", 0.5), ("fl_drv", 0.3), ("vm", 1.0), ("e1_s", 1.0),
            ("sh_1", 1.0), ("sy_1", 1.0), ("dv_1", 5.0), ("ms_1", 3.0), ("md_1", 9.0), ("ma_1", 0.6),
            ("ds_on", 1.0), ("ds_ty", 4.0), ("ds_dr", 0.35), ("ds_mx", 0.6),
        ],
    },
    Preset {
        name: "Dotted Echo Keys",
        cat: "Keys",
        values: &[
            ("a_tb", 6.0), ("a_pos", 0.3), ("a_un", 2.0), ("a_dt", 0.08), ("fl_cut", 5500.0),
            ("e1_a", 2.0), ("e1_d", 900.0), ("e1_s", 0.3), ("e1_r", 450.0),
            ("dl_on", 1.0), ("dl_mx", 0.35), ("dl_sy", 1.0), ("dl_dv", 7.0), ("dl_fb", 0.5), ("dl_pp", 1.0), ("rv_on", 1.0), ("rv_mx", 0.25), ("ch_on", 1.0), ("ch_mx", 0.25),
        ],
    },
    Preset {
        name: "Macro Delay Throw",
        cat: "FX",
        values: &[
            ("a_pos", 0.5), ("a_un", 4.0), ("a_dt", 0.2), ("fl_cut", 5000.0), ("e1_a", 3.0), ("e1_d", 400.0), ("e1_s", 0.7), ("e1_r", 300.0),
            ("dl_on", 1.0), ("dl_mx", 0.0), ("dl_sy", 1.0), ("dl_dv", 7.0), ("dl_fb", 0.3), ("rv_on", 1.0), ("rv_mx", 0.0), ("rv_sz", 0.85),
            ("ms_1", 12.0), ("md_1", 31.0), ("ma_1", 0.7), ("ms_2", 12.0), ("md_2", 32.0), ("ma_2", 0.5), ("ms_3", 13.0), ("md_3", 33.0), ("ma_3", 0.5),
        ],
    },
    Preset {
        name: "Macro Filter Sweep",
        cat: "FX",
        values: &[
            ("a_pos", 0.5), ("a_un", 6.0), ("a_dt", 0.35), ("a_lv", 0.5), ("b_on", 1.0), ("b_pos", 0.5), ("b_un", 5.0), ("b_fn", 10.0), ("b_lv", 0.4), ("fl_on", 0.0),
            ("e1_a", 40.0), ("e1_d", 400.0), ("e1_s", 1.0), ("e1_r", 400.0),
            ("ff_on", 1.0), ("ff_ty", 1.0), ("ff_fc", 700.0), ("ff_q", 0.35), ("ms_1", 12.0), ("md_1", 35.0), ("ma_1", 0.9), ("ms_2", 13.0), ("md_2", 36.0), ("ma_2", 0.5),
            ("rv_on", 1.0), ("rv_mx", 0.2),
        ],
    },
    Preset {
        name: "Vowel Choir",
        cat: "Vocal",
        values: &[
            ("a_tb", 3.0), ("a_pos", 0.1), ("a_un", 5.0), ("a_dt", 0.18), ("fl_on", 1.0), ("fl_cut", 1600.0), ("e1_a", 200.0), ("e1_d", 600.0), ("e1_s", 0.9), ("e1_r", 700.0),
            ("sh_1", 6.0), ("rt_1", 0.5), ("ms_1", 3.0), ("md_1", 1.0), ("ma_1", 0.5), ("rv_on", 1.0), ("rv_mx", 0.35), ("rv_sz", 0.8), ("ch_on", 1.0), ("ch_mx", 0.3),
        ],
    },
    Preset {
        name: "Comb Metal",
        cat: "Synth",
        values: &[
            ("a_tb", 4.0), ("a_pos", 0.3), ("a_un", 3.0), ("a_dt", 0.1), ("fl_ty", 12.0), ("fl_cut", 440.0), ("fl_res", 0.7),
            ("e1_a", 1.0), ("e1_d", 500.0), ("e1_s", 0.2), ("e1_r", 400.0), ("ms_1", 1.0), ("md_1", 9.0), ("ma_1", 0.25), ("e2_a", 1.0), ("e2_d", 400.0), ("e2_s", 0.0),
            ("rv_on", 1.0), ("rv_mx", 0.25),
        ],
    },
    Preset {
        name: "Ladder Acid",
        cat: "Bass",
        values: &[
            ("a_pos", 0.45), ("a_tb", 2.0), ("fl_ty", 6.0), ("fl_cut", 500.0), ("fl_res", 0.7), ("fl_drv", 0.35), ("vm", 2.0), ("gl", 90.0),
            ("e1_a", 1.0), ("e1_d", 250.0), ("e1_s", 0.7), ("e1_r", 90.0), ("ms_1", 1.0), ("md_1", 9.0), ("ma_1", 0.6), ("e2_a", 1.0), ("e2_d", 260.0), ("e2_s", 0.1), ("e2_dc", -0.3),
            ("ms_2", 7.0), ("md_2", 9.0), ("ma_2", 0.25), ("ds_on", 1.0), ("ds_dr", 0.25), ("ds_mx", 0.5),
        ],
    },
    Preset {
        name: "FM Chime",
        cat: "Keys",
        values: &[
            ("a_tb", 5.0), ("a_pos", 0.55), ("b_on", 1.0), ("b_tb", 5.0), ("b_pos", 0.2), ("b_oc", 1.0), ("b_lv", 0.4),
            ("e1_a", 1.0), ("e1_d", 1400.0), ("e1_s", 0.0), ("e1_r", 900.0), ("e1_dc", -0.2),
            ("ms_1", 1.0), ("md_1", 1.0), ("ma_1", -0.3), ("e2_a", 1.0), ("e2_d", 900.0), ("e2_s", 0.0),
            ("rv_on", 1.0), ("rv_mx", 0.35), ("rv_sz", 0.8),
        ],
    },
    Preset {
        name: "Kick Thump",
        cat: "Perc",
        values: &[
            ("a_pos", 0.0), ("a_oc", -2.0), ("a_lv", 0.9), ("fl_cut", 1200.0), ("vm", 1.0),
            ("e1_a", 0.5), ("e1_d", 220.0), ("e1_s", 0.0), ("e1_r", 60.0), ("e1_dc", -0.2),
            ("ms_1", 1.0), ("md_1", 5.0), ("ma_1", 0.5), ("e2_a", 0.5), ("e2_d", 70.0), ("e2_s", 0.0), ("e2_dc", -0.4),
            ("ds_on", 1.0), ("ds_ty", 4.0), ("ds_dr", 0.35), ("ds_mx", 0.6),
        ],
    },
    Preset {
        name: "Noise Snare",
        cat: "Perc",
        values: &[
            ("a_pos", 0.0), ("a_oc", -1.0), ("a_lv", 0.4), ("noi_lv", 0.8), ("noi_ty", 0.0), ("fl_ty", 2.0), ("fl_cut", 900.0), ("fl_res", 0.15),
            ("e1_a", 0.5), ("e1_d", 170.0), ("e1_s", 0.0), ("e1_r", 120.0), ("e1_dc", -0.3),
            ("ms_1", 1.0), ("md_1", 5.0), ("ma_1", 0.2), ("e2_a", 0.5), ("e2_d", 60.0), ("e2_s", 0.0), ("rv_on", 1.0), ("rv_mx", 0.15), ("rv_sz", 0.3),
        ],
    },
    Preset {
        name: "Closed Hat",
        cat: "Perc",
        values: &[
            ("a_on", 0.0), ("noi_lv", 0.8), ("noi_ty", 6.0), ("fl_ty", 3.0), ("fl_cut", 6500.0), ("fl_res", 0.1),
            ("e1_a", 0.5), ("e1_d", 60.0), ("e1_s", 0.0), ("e1_r", 40.0), ("e1_dc", -0.4),
        ],
    },
    Preset {
        name: "Hoover",
        cat: "Lead",
        values: &[
            ("a_pos", 0.55), ("a_un", 12.0), ("a_dt", 0.5), ("a_um", 1.0), ("a_bl", 1.0), ("b_on", 1.0), ("b_pos", 0.55), ("b_un", 9.0), ("b_dt", 0.45), ("b_oc", -1.0),
            ("fl_cut", 3800.0), ("fl_res", 0.2), ("vm", 2.0), ("gl", 130.0),
            ("e1_a", 5.0), ("e1_d", 300.0), ("e1_s", 0.9), ("e1_r", 240.0), ("ms_1", 1.0), ("md_1", 5.0), ("ma_1", -0.12), ("e2_a", 90.0), ("e2_d", 200.0), ("e2_s", 0.0),
            ("ds_on", 1.0), ("ds_dr", 0.3), ("ds_mx", 0.6), ("ch_on", 1.0), ("ch_mx", 0.4),
        ],
    },
    Preset {
        name: "Bit Crush Lead",
        cat: "Lead",
        values: &[
            ("a_tb", 2.0), ("a_pos", 0.2), ("a_un", 2.0), ("a_dt", 0.1), ("fl_cut", 7000.0), ("e1_a", 2.0), ("e1_d", 300.0), ("e1_s", 0.85), ("e1_r", 160.0),
            ("ds_on", 1.0), ("ds_ty", 3.0), ("ds_dr", 0.4), ("ds_mx", 0.7), ("dl_on", 1.0), ("dl_mx", 0.2), ("dl_sy", 1.0), ("dl_dv", 6.0),
        ],
    },
    Preset {
        name: "Wind Texture",
        cat: "FX",
        values: &[
            ("a_on", 0.0), ("noi_lv", 0.7), ("noi_ty", 10.0), ("fl_ty", 4.0), ("fl_cut", 1200.0), ("fl_res", 0.5),
            ("e1_a", 1500.0), ("e1_d", 500.0), ("e1_s", 1.0), ("e1_r", 2500.0),
            ("sh_1", 6.0), ("rt_1", 0.18), ("ms_1", 3.0), ("md_1", 9.0), ("ma_1", 0.5), ("sh_2", 6.0), ("rt_2", 0.3), ("ms_2", 4.0), ("md_2", 13.0), ("ma_2", 0.6),
            ("rv_on", 1.0), ("rv_mx", 0.45), ("rv_sz", 0.9), ("ph_on", 1.0), ("ph_mx", 0.4),
        ],
    },
    Preset {
        name: "Wide Hyper Chords",
        cat: "Keys",
        values: &[
            ("a_pos", 0.5), ("a_un", 5.0), ("a_dt", 0.2), ("a_lv", 0.6), ("b_on", 1.0), ("b_tb", 1.0), ("b_pos", 0.3), ("b_un", 4.0), ("b_lv", 0.4),
            ("fl_cut", 3800.0), ("e1_a", 6.0), ("e1_d", 600.0), ("e1_s", 0.6), ("e1_r", 500.0),
            ("hy_on", 1.0), ("hy_mx", 0.55), ("hy_un", 6.0), ("hy_dm", 0.6), ("rv_on", 1.0), ("rv_mx", 0.25),
        ],
    },
    Preset {
        name: "Phaser Keys",
        cat: "Keys",
        values: &[
            ("a_tb", 6.0), ("a_pos", 0.4), ("a_un", 2.0), ("a_dt", 0.1), ("fl_cut", 6000.0), ("e1_a", 3.0), ("e1_d", 700.0), ("e1_s", 0.5), ("e1_r", 400.0),
            ("ph_on", 1.0), ("ph_mx", 0.55), ("ph_rt", 0.35), ("ph_fb", 0.5), ("fg_on", 1.0), ("fg_mx", 0.2), ("rv_on", 1.0), ("rv_mx", 0.2),
        ],
    },
    Preset {
        name: "Mod Wheel Growl",
        cat: "Synth",
        values: &[
            ("a_pos", 0.5), ("a_un", 3.0), ("a_dt", 0.2), ("a_wm", 8.0), ("b_on", 1.0), ("b_pos", 0.3), ("b_oc", -1.0), ("fl_cut", 2400.0), ("fl_res", 0.3),
            ("e1_a", 5.0), ("e1_d", 400.0), ("e1_s", 0.8), ("e1_r", 200.0),
            ("ms_1", 9.0), ("md_1", 7.0), ("ma_1", 0.8), ("ms_2", 9.0), ("md_2", 9.0), ("ma_2", 0.35), ("ms_3", 9.0), ("md_3", 26.0), ("ma_3", 0.7),
            ("ds_on", 1.0), ("ds_dr", 0.1), ("ds_mx", 0.8), ("rv_on", 1.0), ("rv_mx", 0.2),
        ],
    },
];


fn find(map: &[(String, ParamPtr, String)], id: &str) -> Option<ParamPtr> {
    map.iter().find(|(k, _, _)| k == id).map(|(_, p, _)| *p)
}

/// Embedded data blobs stored next to the numbers: wavetables, noise sample, drawn LFO shapes.
#[derive(Default, Clone)]
pub struct Blobs {
    pub wave: [String; 3],
    pub smp: [String; 3],
    pub ms: [String; 3],
    pub noise: String,
    pub ir: String,
    pub arp: String,
    pub clips: String,
    pub lfo: [String; NLFO],
}

impl Blobs {
    pub fn from_params(p: &SynthParams) -> Blobs {
        let g = |m: &std::sync::Mutex<String>| m.lock().map(|x| x.clone()).unwrap_or_default();
        Blobs { wave: [g(&p.a.wave), g(&p.b.wave), g(&p.c.wave)], smp: [g(&p.a.smp), g(&p.b.smp), g(&p.c.smp)], ms: [g(&p.a.msmp), g(&p.b.msmp), g(&p.c.msmp)], noise: g(&p.noise_sample), ir: g(&p.ir), arp: g(&p.arp_pat), clips: g(&p.clips), lfo: std::array::from_fn(|i| g(&p.lfo[i].draw)) }
    }

    /// Store in the parameters and rebuild the shared audio-side data.
    pub fn apply(&self, p: &SynthParams, sh: &Shared) {
        let set = |m: &std::sync::Mutex<String>, v: &str| {
            if let Ok(mut g) = m.lock() {
                *g = v.to_string();
            }
        };
        set(&p.a.wave, &self.wave[0]);
        set(&p.b.wave, &self.wave[1]);
        set(&p.c.wave, &self.wave[2]);
        set(&p.a.smp, &self.smp[0]);
        set(&p.b.smp, &self.smp[1]);
        set(&p.c.smp, &self.smp[2]);
        set(&p.a.msmp, &self.ms[0]);
        set(&p.b.msmp, &self.ms[1]);
        set(&p.c.msmp, &self.ms[2]);
        set(&p.noise_sample, &self.noise);
        set(&p.ir, &self.ir);
        set(&p.arp_pat, &self.arp);
        set(&p.clips, &self.clips);
        for i in 0..NLFO {
            set(&p.lfo[i].draw, &self.lfo[i]);
        }
        sh.sync_from_params(p);
    }
}

/// Reset every parameter, then apply a list of (id, plain value).
pub fn apply_values(setter: &ParamSetter, params: &SynthParams, vals: &[(String, f32)]) {
    let map = params.param_map();
    // SAFETY: the pointers come from `params`, which outlives this call.
    unsafe {
        for (_, ptr, _) in &map {
            setter.raw_context.raw_begin_set_parameter(*ptr);
            setter.raw_context.raw_set_parameter_normalized(*ptr, ptr.default_normalized_value());
            setter.raw_context.raw_end_set_parameter(*ptr);
        }
        for (id, v) in vals {
            if let Some(ptr) = find(&map, id) {
                let n = ptr.preview_normalized(*v);
                setter.raw_context.raw_begin_set_parameter(ptr);
                setter.raw_context.raw_set_parameter_normalized(ptr, n);
                setter.raw_context.raw_end_set_parameter(ptr);
            }
        }
    }
}

// ---------------------------------------------------------------- the preset list

#[derive(Clone, Debug)]
pub struct PresetRef {
    pub name: String,
    pub cat: String,
    pub factory: Option<usize>,
    pub path: Option<PathBuf>,
}

impl PresetRef {
    pub fn key(&self) -> String {
        format!("{}/{}", self.cat, self.name)
    }
    pub fn is_user(&self) -> bool {
        self.factory.is_none()
    }
}

/// Factory presets plus everything found in the user's preset folder.
pub fn list_all() -> Vec<PresetRef> {
    let hidden = load_hidden();
    let mut out: Vec<PresetRef> = PRESETS
        .iter()
        .enumerate()
        .map(|(i, p)| PresetRef { name: p.name.to_string(), cat: p.cat.to_string(), factory: Some(i), path: None })
        .filter(|r| !hidden.contains(&r.key()))
        .collect();
    if let Some(dir) = files::presets_dir() {
        for e in files::scan(&dir, &["nyp"]) {
            out.push(PresetRef { name: e.name, cat: e.cat, factory: None, path: Some(e.path) });
        }
    }
    out.sort_by(|a, b| (a.cat.to_lowercase(), a.name.to_lowercase()).cmp(&(b.cat.to_lowercase(), b.name.to_lowercase())));
    out
}

fn parse_file(txt: &str) -> (Vec<(String, f32)>, Blobs) {
    let mut vals = Vec::new();
    let mut bl = Blobs::default();
    for line in txt.lines() {
        if line.starts_with('#') {
            continue;
        }
        let Some((k, v)) = line.split_once('=') else { continue };
        let (k, v) = (k.trim(), v.trim());
        if let Some(b) = k.strip_prefix('@') {
            match b {
                "a_wave" => bl.wave[0] = v.to_string(),
                "b_wave" => bl.wave[1] = v.to_string(),
                "c_wave" => bl.wave[2] = v.to_string(),
                "a_smp" => bl.smp[0] = v.to_string(),
                "b_smp" => bl.smp[1] = v.to_string(),
                "c_smp" => bl.smp[2] = v.to_string(),
                "a_ms" => bl.ms[0] = v.to_string(),
                "b_ms" => bl.ms[1] = v.to_string(),
                "c_ms" => bl.ms[2] = v.to_string(),
                "noise" => bl.noise = v.to_string(),
                "ir" => bl.ir = v.to_string(),
                "arp" => bl.arp = v.to_string(),
                "clips" => bl.clips = v.to_string(),
                "lfo1" => bl.lfo[0] = v.to_string(),
                "lfo2" => bl.lfo[1] = v.to_string(),
                "lfo3" => bl.lfo[2] = v.to_string(),
                "lfo4" => bl.lfo[3] = v.to_string(),
                "lfo5" => bl.lfo[4] = v.to_string(),
                "lfo6" => bl.lfo[5] = v.to_string(),
                "lfo7" => bl.lfo[6] = v.to_string(),
                "lfo8" => bl.lfo[7] = v.to_string(),
                _ => {}
            }
        } else if let Some(x) = v.parse::<f32>().ok().filter(|x| x.is_finite()) {
            vals.push((k.to_string(), x));
        }
    }
    (vals, bl)
}

/// Load a preset (factory or file). Returns an error text if a file cannot be read.
pub fn load(setter: &ParamSetter, params: &SynthParams, shared: &Shared, r: &PresetRef) -> Result<(), String> {
    if let Some(i) = r.factory {
        let p = PRESETS.get(i).ok_or("no such preset")?;
        let vals: Vec<(String, f32)> = p.values.iter().map(|(k, v)| (k.to_string(), *v)).collect();
        apply_values(setter, params, &vals);
        Blobs::default().apply(params, shared);
    } else if let Some(path) = &r.path {
        let txt = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
        let (vals, bl) = parse_file(&txt);
        apply_values(setter, params, &vals);
        bl.apply(params, shared);
    }
    if let Ok(mut g) = params.preset_name.lock() {
        *g = r.name.clone();
    }
    Ok(())
}

/// Save the current sound into `<Presets>/<category>/<name>.nyp`.
pub fn save_user(params: &SynthParams, name: &str, cat: &str) -> Result<PathBuf, String> {
    let name = files::clean_name(name);
    if name.is_empty() {
        return Err("empty name".into());
    }
    let cat = files::clean_name(cat);
    let mut dir = files::presets_dir().ok_or("no config folder")?;
    if !cat.is_empty() && cat != "User" {
        dir = dir.join(&cat);
    }
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let mut txt = String::from("# NyashkSynth preset v2\n");
    for (id, ptr, _) in params.param_map() {
        // SAFETY: the pointers come from `params`, which is alive.
        let v = unsafe { ptr.unmodulated_plain_value() };
        txt.push_str(&format!("{id}={v}\n"));
    }
    let bl = Blobs::from_params(params);
    if !bl.wave[0].is_empty() {
        txt.push_str(&format!("@a_wave={}\n", bl.wave[0]));
    }
    if !bl.wave[1].is_empty() {
        txt.push_str(&format!("@b_wave={}\n", bl.wave[1]));
    }
    if !bl.wave[2].is_empty() {
        txt.push_str(&format!("@c_wave={}\n", bl.wave[2]));
    }
    for (i, k) in ["a", "b", "c"].iter().enumerate() {
        if !bl.smp[i].is_empty() {
            txt.push_str(&format!("@{k}_smp={}\n", bl.smp[i]));
        }
    }
    if !bl.clips.is_empty() {
        txt.push_str(&format!("@clips={}\n", bl.clips));
    }
    if !bl.arp.is_empty() {
        txt.push_str(&format!("@arp={}\n", bl.arp));
    }
    for (i, k) in ["a_ms", "b_ms", "c_ms"].iter().enumerate() {
        if !bl.ms[i].is_empty() {
            txt.push_str(&format!("@{}={}\n", k, bl.ms[i]));
        }
    }
    if !bl.noise.is_empty() {
        txt.push_str(&format!("@noise={}\n", bl.noise));
    }
    if !bl.ir.is_empty() {
        txt.push_str(&format!("@ir={}\n", bl.ir));
    }
    for i in 0..NLFO {
        if !bl.lfo[i].is_empty() {
            txt.push_str(&format!("@lfo{}={}\n", i + 1, bl.lfo[i]));
        }
    }
    let path = dir.join(format!("{name}.nyp"));
    std::fs::write(&path, txt).map_err(|e| e.to_string())?;
    Ok(path)
}

pub fn delete_user(r: &PresetRef) {
    if let Some(p) = &r.path {
        let _ = std::fs::remove_file(p);
        // remove the category folder (and parents) if it is now empty, but never the Presets root
        if let Some(root) = files::presets_dir() {
            let mut d = p.parent().map(|x| x.to_path_buf());
            while let Some(dir) = d {
                if dir == root || !dir.starts_with(&root) || std::fs::remove_dir(&dir).is_err() {
                    break;
                }
                d = dir.parent().map(|x| x.to_path_buf());
            }
        }
    }
}

// ---------------------------------------------------------------- hidden factory presets

fn hidden_path() -> Option<PathBuf> {
    files::base_dir().map(|b| b.join("hidden.txt"))
}

/// Keys (`Category/Name`) of factory presets the user chose to hide. The presets are built into the plugin,
/// so they cannot be deleted from disk, only hidden from the browser.
pub fn load_hidden() -> Vec<String> {
    hidden_path().and_then(|p| std::fs::read_to_string(p).ok()).map(|t| t.lines().map(|l| l.trim().to_string()).filter(|l| !l.is_empty()).collect()).unwrap_or_default()
}

pub fn save_hidden(h: &[String]) {
    if let Some(p) = hidden_path() {
        if h.is_empty() {
            let _ = std::fs::remove_file(p);
            return;
        }
        if let Some(d) = p.parent() {
            let _ = std::fs::create_dir_all(d);
        }
        let _ = std::fs::write(p, h.join("\n"));
    }
}

// ---------------------------------------------------------------- favourites

fn fav_path() -> Option<PathBuf> {
    files::base_dir().map(|b| b.join("favorites.txt"))
}

pub fn load_favs() -> Vec<String> {
    fav_path().and_then(|p| std::fs::read_to_string(p).ok()).map(|t| t.lines().map(|l| l.trim().to_string()).filter(|l| !l.is_empty()).collect()).unwrap_or_default()
}

pub fn save_favs(f: &[String]) {
    if let Some(p) = fav_path() {
        if let Some(d) = p.parent() {
            let _ = std::fs::create_dir_all(d);
        }
        let _ = std::fs::write(p, f.join("\n"));
    }
}

// ---------------------------------------------------------------- ratings and tags

/// Rating (0-5) and tags per preset key (`Category/Name`), stored in `meta.txt` next to the library.
pub type Meta = std::collections::BTreeMap<String, (u8, Vec<String>)>;

fn meta_path() -> Option<PathBuf> {
    files::base_dir().map(|b| b.join("meta.txt"))
}

pub fn parse_meta(t: &str) -> Meta {
    let mut m = Meta::new();
    for l in t.lines() {
        let mut it = l.splitn(3, '\t');
        let (Some(k), Some(r)) = (it.next(), it.next()) else { continue };
        let tags: Vec<String> = it.next().unwrap_or("").split(',').map(|x| x.trim().to_lowercase()).filter(|x| !x.is_empty()).collect();
        m.insert(k.to_string(), (r.trim().parse::<u8>().unwrap_or(0).min(5), tags));
    }
    m
}

pub fn encode_meta(m: &Meta) -> String {
    m.iter().filter(|(_, (r, t))| *r > 0 || !t.is_empty()).map(|(k, (r, t))| format!("{k}\t{r}\t{}", t.join(","))).collect::<Vec<_>>().join("\n")
}

pub fn load_meta() -> Meta {
    meta_path().and_then(|p| std::fs::read_to_string(p).ok()).map(|t| parse_meta(&t)).unwrap_or_default()
}

pub fn save_meta(m: &Meta) {
    if let Some(p) = meta_path() {
        if let Some(d) = p.parent() {
            let _ = std::fs::create_dir_all(d);
        }
        let _ = std::fs::write(p, encode_meta(m));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn meta_roundtrip() {
        let mut m = Meta::new();
        m.insert("Bass/Wob".into(), (4, vec!["dark".into(), "wobble".into()]));
        m.insert("Lead/X".into(), (0, vec![]));
        let back = parse_meta(&encode_meta(&m));
        assert_eq!(back.get("Bass/Wob"), Some(&(4, vec!["dark".to_string(), "wobble".to_string()])));
        assert!(back.get("Lead/X").is_none(), "empty entries are dropped");
    }

    use super::*;

    #[test]
    fn user_preset_roundtrip() {
        let dir = std::env::temp_dir().join("nyashk_preset_test2");
        let _ = std::fs::remove_dir_all(&dir);
        std::env::set_var("XDG_CONFIG_HOME", &dir);
        std::env::set_var("APPDATA", &dir);
        let params = SynthParams::default();
        if let Ok(mut g) = params.a.wave.lock() {
            *g = crate::wavetable::encode_table("T", &crate::wavetable::tables().t[2]);
        }
        let path = save_user(&params, "My Test/Sound", "Bass").expect("save");
        assert!(path.ends_with("Bass/My Test_Sound.nyp"));
        let all = list_all();
        let me = all.iter().find(|r| r.name == "My Test_Sound").expect("listed");
        assert_eq!(me.cat, "Bass");
        let txt = std::fs::read_to_string(path).unwrap();
        let (vals, bl) = parse_file(&txt);
        assert_eq!(vals.len(), params.param_map().len());
        assert!(bl.wave[0].starts_with("NWT1|T|"));
        delete_user(me);
        assert!(!list_all().iter().any(|r| r.name == "My Test_Sound"));
    }

    #[test]
    fn all_preset_ids_exist() {
        let params = SynthParams::default();
        let map = params.param_map();
        for p in PRESETS {
            for (id, _) in p.values {
                assert!(find(&map, id).is_some(), "preset '{}' uses unknown id '{}'", p.name, id);
            }
        }
    }
}

/// Numbers and blobs of a preset without applying it.
pub fn values_of(r: &PresetRef) -> Option<(Vec<(String, f32)>, Blobs)> {
    if let Some(i) = r.factory {
        let p = PRESETS.get(i)?;
        Some((p.values.iter().map(|(k, v)| (k.to_string(), *v)).collect(), Blobs::default()))
    } else {
        let txt = std::fs::read_to_string(r.path.as_ref()?).ok()?;
        Some(parse_file(&txt))
    }
}

/// Mix the current sound with another preset: every section (oscillator, filter, one LFO, one
/// matrix row, one effect...) comes from one or the other, decided by chance.
pub fn hybridize(setter: &ParamSetter, params: &SynthParams, shared: &Shared, other: &PresetRef, seed: u64) -> Result<(), String> {
    use std::collections::HashMap;
    let (vals, blob_b) = values_of(other).ok_or("cannot read the other preset")?;
    let map = params.param_map();
    let mut rng = seed | 1;
    let mut coin = move || {
        rng ^= rng << 13;
        rng ^= rng >> 7;
        rng ^= rng << 17;
        (rng >> 11) as f64 / (1u64 << 53) as f64 > 0.5
    };
    let key = |id: &str, grp: &str| -> String {
        if grp.starts_with("FX") {
            let rest = id.strip_prefix("b2_").unwrap_or(id);
            // the 12 chain-order parameters (fo1..fo12) must come from ONE preset, or modules drop out of the chain
            let bus = if id.starts_with("b2_") { "2" } else { "1" };
            if rest.len() > 2 && rest.starts_with("fo") && rest[2..].chars().all(|c| c.is_ascii_digit()) {
                return format!("FX/order{bus}");
            }
            format!("{grp}/{}", rest.split('_').next().unwrap_or(rest))
        } else if grp.is_empty() {
            if id.starts_with("ar_") {
                "ARP".to_string()
            } else if id.starts_with("noi_") {
                "NOISE".to_string()
            } else if id.starts_with("clp_") {
                "CLIPSEQ".to_string()
            } else {
                id.to_string()
            }
        } else {
            grp.to_string()
        }
    };
    let mut choice: HashMap<String, bool> = HashMap::new();
    // SAFETY: pointers come from `params`, which outlives this call; GUI thread only.
    unsafe {
        for (id, ptr, grp) in &map {
            let k = key(id, grp);
            let from_b = *choice.entry(k).or_insert_with(&mut coin);
            if !from_b {
                continue;
            }
            let n = match vals.iter().find(|(i, _)| i == id) {
                Some((_, v)) => ptr.preview_normalized(*v),
                None => ptr.default_normalized_value(),
            };
            setter.raw_context.raw_begin_set_parameter(*ptr);
            setter.raw_context.raw_set_parameter_normalized(*ptr, n);
            setter.raw_context.raw_end_set_parameter(*ptr);
        }
    }
    let mut bl = Blobs::from_params(params);
    for (i, g) in ["Osc A", "Osc B", "Osc C"].iter().enumerate() {
        if choice.get(*g).copied().unwrap_or(false) {
            bl.wave[i] = blob_b.wave[i].clone();
            bl.smp[i] = blob_b.smp[i].clone();
            bl.ms[i] = blob_b.ms[i].clone();
        }
    }
    for i in 0..NLFO {
        if choice.get(&format!("LFO {}", i + 1)).copied().unwrap_or(false) {
            bl.lfo[i] = blob_b.lfo[i].clone();
        }
    }
    if choice.get("FX/rv").copied().unwrap_or(false) {
        bl.ir = blob_b.ir.clone();
    }
    if choice.get("ARP").copied().unwrap_or(false) {
        bl.arp = blob_b.arp.clone();
    }
    if choice.get("NOISE").copied().unwrap_or(false) {
        bl.noise = blob_b.noise.clone();
    }
    if choice.get("CLIPSEQ").copied().unwrap_or(false) {
        bl.clips = blob_b.clips.clone();
    }
    bl.apply(params, shared);
    if let Ok(mut g) = params.preset_name.lock() {
        *g = format!("Hybrid {}", other.name);
    }
    Ok(())
}

// ---------------------------------------------------------------- preset packs

const PACK_MAGIC: &str = "#NYASHKPACK 1";

/// Bundle all user presets (or one category) into a single `.nypack` file in the Packs folder.
pub fn export_pack(name: &str, only_cat: Option<&str>) -> Result<(PathBuf, usize), String> {
    let dir = files::packs_dir().ok_or("no folder")?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let mut txt = format!("{PACK_MAGIC}\n");
    let mut n = 0;
    for r in list_all() {
        let Some(path) = &r.path else { continue };
        if only_cat.map_or(false, |c| c != r.cat) {
            continue;
        }
        let Ok(body) = std::fs::read_to_string(path) else { continue };
        txt.push_str(&format!("@@@PRESET {}/{}\n{}\n@@@END\n", r.cat, r.name, body.trim_end()));
        n += 1;
    }
    if n == 0 {
        return Err("no user presets to export".into());
    }
    let path = dir.join(format!("{}.nypack", files::clean_name(name)));
    std::fs::write(&path, txt).map_err(|e| e.to_string())?;
    Ok((path, n))
}

/// Unpack every `.nypack` in the Packs folder into the preset folder. Existing presets are never overwritten.
pub fn import_packs() -> Result<usize, String> {
    let dir = files::packs_dir().ok_or("no folder")?;
    let pdir = files::presets_dir().ok_or("no folder")?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let mut added = 0;
    for e in files::scan(&dir, &["nypack"]) {
        let Ok(txt) = std::fs::read_to_string(&e.path) else { continue };
        added += unpack(&txt, &pdir);
    }
    Ok(added)
}

pub fn unpack(txt: &str, pdir: &std::path::Path) -> usize {
    let mut lines = txt.lines();
    if lines.next().map(|l| l.trim()) != Some(PACK_MAGIC) {
        return 0;
    }
    let mut added = 0;
    let mut cur: Option<(String, String, String)> = None;
    for l in lines {
        if let Some(h) = l.strip_prefix("@@@PRESET ") {
            let (cat, name) = h.rsplit_once('/').unwrap_or(("User", h));
            cur = Some((files::clean_name(cat), files::clean_name(name), String::new()));
        } else if l.trim() == "@@@END" {
            if let Some((cat, name, body)) = cur.take() {
                if name.is_empty() {
                    continue;
                }
                let d = if cat.is_empty() || cat == "User" { pdir.to_path_buf() } else { pdir.join(&cat) };
                if std::fs::create_dir_all(&d).is_err() {
                    continue;
                }
                let path = d.join(format!("{name}.nyp"));
                if path.exists() {
                    continue;
                }
                if std::fs::write(&path, body).is_ok() {
                    added += 1;
                }
            }
        } else if let Some((_, _, body)) = cur.as_mut() {
            body.push_str(l);
            body.push('\n');
        }
    }
    added
}

#[cfg(test)]
mod pack_tests {
    use super::*;

    #[test]
    fn unpack_writes_presets_and_keeps_existing() {
        let dir = std::env::temp_dir().join(format!("nyashk_pack_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let txt = "#NYASHKPACK 1\n@@@PRESET Bass/Wobble\na_lv=0.5\n@a_wave=NWT1|x\n@@@END\n@@@PRESET Lead/Zip\nmaster=-3\n@@@END\n";
        assert_eq!(unpack(txt, &dir), 2);
        assert_eq!(std::fs::read_to_string(dir.join("Bass").join("Wobble.nyp")).unwrap(), "a_lv=0.5\n@a_wave=NWT1|x\n");
        // second import adds nothing and overwrites nothing
        assert_eq!(unpack(txt, &dir), 0);
        // not a pack
        assert_eq!(unpack("hello", &dir), 0);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
