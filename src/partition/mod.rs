//! 自定义分区与手动校准：给官方表里没有的机型准备的两条后路。
//!
//! 官方 15 个布局参数只覆盖「5 列均匀网格 + 4 行」这一种形态，且网格位置由
//! `index0/index4/level0/level3` 与有效区内缩推算。表外机型常见两类偏差：
//!
//! 1. **分区不同**：贴纸行列不是 5x4，列宽不是等分，NumLock 图标不在右上角；
//! 2. **面板参数不同**：`ABS_MT_POSITION_*` 的范围/分辨率报不准，或干脆没报。
//!
//! 对应两个配置段：
//!
//! * `partition`：直接用边界把网格切成任意行列（可非均匀），可整块平移缩放；
//! * `calibration`：覆盖轴的 `min` / `max` / `units_per_mm`。
//!
//! 两者都只描述**几何**，不碰键表——键表仍由 `custom_layouts[].keys` 给。
//!
//! 校准求解器 [`solve_calibration`] 是纯函数：给它「四角落点 + 两个格中心落点」的
//! 实测样本，它把面板范围与网格边界一并算出来。采集在 `linux` 模块，macOS 上也能
//! 用样本文件跑同一套算法做端到端验证。

mod grid;
mod panel;
mod solve;

pub use grid::*;
pub use panel::*;
pub use solve::*;

#[cfg(test)]
mod tests {
    use super::*;

    fn panel() -> Panel {
        Panel {
            xmin: 0,
            xmax: 4752,
            ymin: 0,
            ymax: 3008,
            ux: 45.0 / 1000.0,
            uy: 42.0 / 1000.0,
        }
    }

    #[test]
    fn split_even_and_weighted() {
        let e = split(0.0, 100.0, 4, &[], "col").unwrap();
        assert_eq!(e, vec![0.0, 25.0, 50.0, 75.0, 100.0]);
        let w = split(0.0, 100.0, 2, &[3.0, 1.0], "col").unwrap();
        assert_eq!(w, vec![0.0, 75.0, 100.0]);
        assert!(split(0.0, 100.0, 2, &[1.0], "col").is_err());
        assert!(split(0.0, 100.0, 2, &[1.0, 0.0], "col").is_err());
    }

    #[test]
    fn partition_resolves_uniform_over_base_grid() {
        let p = PartitionSpec {
            cols: Some(4),
            rows: Some(3),
            ..Default::default()
        };
        let r = p
            .resolve(panel(), (100.0, 100.0, 900.0, 600.0), (5, 4))
            .unwrap();
        assert_eq!(r.x_edges.len(), 5);
        assert_eq!(r.y_edges.len(), 4);
        assert_eq!(r.x_edges[0], 100.0);
        assert_eq!(r.x_edges[4], 900.0);
        assert_eq!(r.nl_cell, Some((0, 3)));
        assert!(r.notes.is_empty());
    }

    #[test]
    fn partition_rect_mm_converts_through_panel() {
        let p = PartitionSpec {
            rect_mm: Some([10.0, 10.0, 100.0, 70.0]),
            ..Default::default()
        };
        let r = p.resolve(panel(), (0.0, 0.0, 1.0, 1.0), (5, 4)).unwrap();
        // 10 mm × 45 units/mm = 450
        assert!((r.x_edges[0] - 450.0).abs() < 1.0, "{:?}", r.x_edges);
        // 70 mm × 42 units/mm = 2940
        assert!((r.y_edges[4] - 2940.0).abs() < 1.0, "{:?}", r.y_edges);
    }

    #[test]
    fn partition_rejects_bad_shapes() {
        let p = PartitionSpec {
            rect: Some([500.0, 100.0, 100.0, 600.0]),
            ..Default::default()
        };
        assert!(p.resolve(panel(), (0.0, 0.0, 1.0, 1.0), (5, 4)).is_err());
        let p = PartitionSpec {
            rect: Some([0.0, 0.0, 5000.0, 600.0]),
            ..Default::default()
        };
        assert!(p.resolve(panel(), (0.0, 0.0, 1.0, 1.0), (5, 4)).is_err());
        let p = PartitionSpec {
            cols: Some(9),
            ..Default::default()
        };
        assert!(p.resolve(panel(), (0.0, 0.0, 1.0, 1.0), (5, 4)).is_err());
        let p = PartitionSpec {
            numlock_cell: Some([9, 0]),
            ..Default::default()
        };
        assert!(p.resolve(panel(), (0.0, 0.0, 1.0, 1.0), (5, 4)).is_err());
    }

    #[test]
    fn partition_notes_when_grid_wider_than_keytable() {
        let p = PartitionSpec {
            cols: Some(7),
            ..Default::default()
        };
        let r = p
            .resolve(panel(), (100.0, 100.0, 900.0, 600.0), (5, 4))
            .unwrap();
        assert_eq!(r.notes, vec![PartitionNote::ExtraCols(5, 7)]);
    }

    #[test]
    fn partition_only_applies_to_its_layout() {
        let mut p = PartitionSpec {
            layout: "HANDMADE".to_string(),
            ..Default::default()
        };
        assert!(p.applies_to("handmade"));
        assert!(!p.applies_to("GH3XLNDZ"));
        p.layout = "auto".to_string();
        assert!(p.applies_to("GH3XLNDZ"));
    }

    #[test]
    fn calibration_overrides_only_written_items() {
        let cal = CalibrationSpec {
            x: AxisCal {
                min: Some(-10),
                max: Some(4800),
                units_per_mm: Some(40.0),
            },
            y: AxisCal::default(),
        };
        let (p, notes) = cal.apply(panel());
        assert_eq!(p.xmin, -10);
        assert_eq!(p.xmax, 4800);
        assert!((p.ux - 0.04).abs() < 1e-12);
        assert_eq!(p.ymin, 0);
        assert_eq!(p.ymax, 3008);
        assert!((p.uy - 42.0 / 1000.0).abs() < 1e-12);
        assert!(notes.is_empty());
    }

    #[test]
    fn calibration_notes_do_not_need_a_panel() {
        let cal = CalibrationSpec {
            x: AxisCal {
                min: Some(10),
                max: Some(10),
                units_per_mm: None,
            },
            y: AxisCal {
                min: Some(-5),
                max: None,
                units_per_mm: Some(0.0),
            },
        };
        assert_eq!(
            cal.notes(),
            vec![CalNote::Inverted("x"), CalNote::BadUnitsPerMm("y")]
        );
        assert!(CalibrationSpec::default().notes().is_empty());
    }

    #[test]
    fn calibration_flags_inverted_and_bad_units() {
        let cal = CalibrationSpec {
            x: AxisCal {
                min: Some(4800),
                max: Some(10),
                units_per_mm: Some(-1.0),
            },
            y: AxisCal::default(),
        };
        let (p, notes) = cal.apply(panel());
        // 反了的整轴忽略，保持内核值
        assert_eq!((p.xmin, p.xmax), (0, 4752));
        assert_eq!(
            notes,
            vec![CalNote::Inverted("x"), CalNote::BadUnitsPerMm("x"),]
        );
    }

    #[test]
    fn solve_calibration_recovers_grid_from_probes() {
        // 真实面板 0..4752 / 0..3008，网格 x0=300 x1=4500（5 列，格宽 840）
        // y0=200 y1=2900（4 行，格高 675）
        let col_w = 840.0;
        let row_h = 675.0;
        let (gx0, gy0) = (300.0, 200.0);
        let center = |row: usize, col: usize| {
            (
                (gx0 + (col as f64 + 0.5) * col_w) as i32,
                (gy0 + (row as f64 + 0.5) * row_h) as i32,
            )
        };
        let corners = CornerSamples {
            top_left: (0, 0),
            top_right: (4752, 0),
            bottom_left: (0, 3008),
            bottom_right: (4752, 3008),
        };
        let probe = GridProbe {
            first_cell: (0, 0),
            first_point: center(0, 0),
            last_cell: (2, 4),
            last_point: center(2, 4),
            cols: 5,
            rows: 4,
        };
        let c = solve_calibration(&corners, &probe).unwrap();
        assert_eq!((c.panel.xmin, c.panel.xmax), (0, 4752));
        let rect = c.partition.rect.unwrap();
        assert!((rect[0] - gx0).abs() < 1.0, "{rect:?}");
        assert!((rect[1] - gy0).abs() < 1.0, "{rect:?}");
        assert!((rect[2] - (gx0 + 5.0 * col_w)).abs() < 1.0, "{rect:?}");
        assert!((rect[3] - (gy0 + 4.0 * row_h)).abs() < 1.0, "{rect:?}");
    }

    #[test]
    fn solve_calibration_falls_back_to_panel_split_without_span() {
        let corners = CornerSamples {
            top_left: (0, 0),
            top_right: (1000, 0),
            bottom_left: (0, 800),
            bottom_right: (1000, 800),
        };
        // 两个探针落在同一列：横向退回「面板等分」，纵向仍由探针反解
        let probe = GridProbe {
            first_cell: (0, 0),
            first_point: (100, 100),
            last_cell: (3, 0),
            last_point: (100, 700),
            cols: 5,
            rows: 4,
        };
        let c = solve_calibration(&corners, &probe).unwrap();
        let rect = c.partition.rect.unwrap();
        assert!((rect[0] - 0.0).abs() < 1.0);
        assert!((rect[2] - 1000.0).abs() < 1.0);
    }

    #[test]
    fn solve_calibration_rejects_bad_input() {
        let corners = CornerSamples {
            top_left: (0, 0),
            top_right: (4752, 0),
            bottom_left: (0, 3008),
            bottom_right: (4752, 3008),
        };
        let same = GridProbe {
            first_cell: (0, 0),
            first_point: (420, 537),
            last_cell: (0, 0),
            last_point: (420, 537),
            cols: 5,
            rows: 4,
        };
        assert!(solve_calibration(&corners, &same).is_err());

        let outside = GridProbe {
            first_cell: (0, 0),
            first_point: (420, 537),
            last_cell: (3, 4),
            last_point: (8000, 3000),
            cols: 5,
            rows: 4,
        };
        assert!(solve_calibration(&corners, &outside).is_err());

        let flipped = CornerSamples {
            top_left: (4752, 3008),
            top_right: (0, 3008),
            bottom_left: (4752, 0),
            bottom_right: (0, 0),
        };
        assert!(solve_calibration(&flipped, &outside).is_err());
    }

    #[test]
    fn partition_roundtrips_through_json() {
        let text = r#"{
            "layout": "HANDMADE",
            "rect": [300, 200, 4500, 2900],
            "cols": 5,
            "rows": 4,
            "col_ratios": [1, 1, 1, 1, 1],
            "numlock_cell": [0, 4],
            "snap_outside": true
        }"#;
        let p: PartitionSpec = serde_json::from_str(text).unwrap();
        assert_eq!(p.layout, "HANDMADE");
        assert_eq!(p.cols, Some(5));
        assert!(p.snap_outside);
        let back: PartitionSpec =
            serde_json::from_str(&serde_json::to_string(&p).unwrap()).unwrap();
        assert_eq!(back, p);
        // 拼错字段必须报错，而不是静默忽略
        assert!(serde_json::from_str::<PartitionSpec>(r#"{"rectt": [1,2,3,4]}"#).is_err());
    }
}
