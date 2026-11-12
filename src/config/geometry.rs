//! 由配置推导渲染用的几何、键表与布局来源；自定义布局在此注册进全局表。

use super::*;

impl Config {
    pub fn install_custom(&self) -> Vec<String> {
        crate::custom::install_layouts(&self.custom_layouts)
    }

    pub fn dedup_notes(notes: &mut Vec<String>) {
        let mut seen = std::collections::HashSet::new();
        notes.retain(|n| seen.insert(n.clone()));
    }

    pub fn key_table(&self) -> crate::custom::KeyTable {
        crate::custom::table_for(&self.layout_name)
    }

    pub fn build_geometry(
        &self,
        base: crate::partition::Panel,
        layout_name: &'static str,
    ) -> (crate::layout::Geometry, Vec<String>) {
        let (panel, _cal_notes) = self.calibration.apply(base);
        let geo = crate::layout::Geometry::new(
            panel.xmin,
            panel.xmax,
            panel.ymin,
            panel.ymax,
            panel.ux,
            panel.uy,
            self.layout(),
        )
        .with_layout_name(layout_name);
        let mut notes = Vec::new();
        if self.partition.is_unset() || !self.partition.applies_to(layout_name) {
            return (geo, notes);
        }
        let table = self.key_table();
        let keys = (table.cols(), table.rows().len());
        let base_grid = geo.grid_rect();
        match self.partition.resolve(panel, base_grid, keys) {
            Ok(r) => match geo.clone().with_partition(r.x_edges, r.y_edges, r.nl_cell) {
                Ok(g) => {
                    for n in &r.notes {
                        let args = n.args();
                        let refs: Vec<(&str, &str)> =
                            args.iter().map(|(k, v)| (*k, v.as_str())).collect();
                        notes.push(crate::i18n::tf(n.key(), &refs));
                    }
                    (g.with_snap_outside(r.snap_outside), notes)
                }
                Err(err) => {
                    notes.push(crate::i18n::tf("note.partition_invalid", &[("err", &err)]));
                    (geo, notes)
                }
            },
            Err(err) => {
                notes.push(crate::i18n::tf("note.partition_invalid", &[("err", &err)]));
                (geo, notes)
            }
        }
    }

    pub fn led_proto_is_builtin(&self) -> bool {
        self.led_proto.is_builtin()
    }

    pub fn layout_entry(&self) -> &'static crate::layout::LayoutEntry {
        crate::layout::by_name(&self.layout_name)
            .or_else(|| crate::layout::by_name(crate::layout::DEFAULT_LAYOUT_NAME))
            .expect("内置布局表必须包含默认布局")
    }

    pub fn layout(&self) -> crate::layout::Layout {
        self.layout_entry().layout
    }
}
