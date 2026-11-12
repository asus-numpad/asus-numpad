//! 驱动配置文件（`numdeck.json`）在界面里的编辑草稿。
//!
//! 界面直接读写驱动自己那份配置，而不是另存一份影子副本：文件格式、字段名、默认值
//! 全部来自 [`numdeck::config`]，和 `numdeck config` / 手改文件看到的是同一份东西。
//!
//! 写路径沿用驱动自己的规则（[`Config::writable_path`]）：普通用户落
//! `~/.config/numdeck/numdeck.json`，root 才写 `/etc`。界面不 sudo，所以
//! 正常情况永远不需要提权。

use std::path::PathBuf;

use numdeck::config::Config;

/// 一份可编辑的驱动配置。
pub struct Draft {
    /// 读取时的来源路径（可能是 `/etc` 下的历史配置）
    pub path: PathBuf,
    /// 实际落盘路径（普通用户即用户级路径）
    pub write_path: PathBuf,
    /// 读取时文件是否已存在；不存在说明界面里保存的是首次创建
    pub existed: bool,
    pub cfg: Config,
    /// 读取过程中的提示（解析失败、字段被夹回合法区间等）
    pub notes: Vec<String>,
    /// 界面改过但还没落盘
    pub dirty: bool,
}

impl Draft {
    pub fn load() -> Self {
        let path = Config::default_path();
        let existed = path.exists();
        // 非 Linux 只用于开发机预览 Windows/Linux 两套界面：一律写用户级路径，
        // 免得因为没有 geteuid 语义而把「可写」误判成 /etc。
        let write_path = if cfg!(target_os = "linux") {
            Config::writable_path(&path)
        } else {
            Config::user_config_path()
        };
        let (cfg, notes) = Config::load(&path);
        Self {
            path,
            write_path,
            existed,
            cfg,
            notes,
            dirty: false,
        }
    }

    /// 丢掉界面上的改动，按磁盘重新读一遍。
    pub fn reload(&mut self) {
        *self = Self::load();
    }

    /// 恢复成驱动默认值（不落盘，需要再点保存）。
    pub fn reset(&mut self) {
        self.cfg = Config::default();
        self.notes.clear();
        self.dirty = true;
    }

    /// 落盘。返回给状态栏用的文案。
    pub fn save(&mut self) -> anyhow::Result<String> {
        // `sanitize` 会把越界 / 装不上的项夹回合法值，并逐条说明原因。过去这里把
        // 返回值直接丢掉、再把 notes 清空，于是「保存成功」背后可能已经悄悄丢掉一条
        // 非法热键或者一整个装不上的布局，用户存完看不出任何异常，重启后才发现配置
        // 不是自己写的那份。现在把这些说明留下来给界面显示，并在结果里点明被改过。
        let notes = self.cfg.sanitize();
        let text = format!("{}\n", self.cfg.to_json());
        // 复用设置的原子写：临时文件 + rename，写一半崩了也不会把配置文件截断
        crate::settings::write_atomic(&self.write_path, &text)
            .map_err(|err| anyhow::anyhow!("写入 {} 失败：{err}", self.write_path.display()))?;
        let fixed = notes.len();
        self.notes = notes;
        self.dirty = false;
        self.existed = true;
        let path = self.write_path.display();
        Ok(if fixed == 0 {
            format!("驱动配置已写入 {path}")
        } else {
            // notes 里既有「夹回合法值」也有「这条设定生效后要知道的事」，不细分，统一说成提示
            format!("驱动配置已写入 {path}，另有 {fixed} 条提示，见右侧列表")
        })
    }

    /// 当前编辑内容的 JSON 预览（与 `numdeck --dump-config` 同一份序列化）。
    pub fn json(&self) -> String {
        self.cfg.to_json()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn draft_at(dir: &std::path::Path) -> Draft {
        let path = dir.join("numdeck.json");
        let mut draft = Draft {
            path: path.clone(),
            write_path: path,
            existed: false,
            cfg: Config::default(),
            notes: Vec::new(),
            dirty: true,
        };
        draft.cfg.hotkeys = Vec::new();
        draft
    }

    fn tmpdir(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "numdeck-gui-driver-{tag}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn save_reports_fixed_parameters() {
        let dir = tmpdir("fixed");
        let mut draft = draft_at(&dir);
        // 装不上的热键（只有修饰键）：sanitize 会把它挑出来并夹回可用写法
        draft.cfg.hotkeys = vec![numdeck::custom::HotkeySpec {
            gesture: "ctrl+".to_string(),
            action: "on".to_string(),
        }];
        let msg = draft.save().unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        assert!(
            msg.contains("提示"),
            "被修正过就要在结果里说明，实际是：{msg}"
        );
        assert!(!draft.notes.is_empty(), "修正原因要留给界面显示");
    }

    #[test]
    fn save_silent_when_nothing_fixed() {
        let dir = tmpdir("clean");
        let mut draft = draft_at(&dir);
        let msg = draft.save().unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        assert!(!msg.contains("提示"), "没改动就别吓人：{msg}");
        assert!(draft.notes.is_empty());
    }

    #[test]
    fn saved_file_is_readable_back() {
        let dir = tmpdir("roundtrip");
        let mut draft = draft_at(&dir);
        draft.cfg.pressure_switch.enabled = false;
        draft.save().unwrap();
        let path = dir.join("numdeck.json");
        let text = std::fs::read_to_string(&path).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        assert!(text.trim().ends_with('}'), "落盘内容应是完整 JSON");
        assert!(text.contains("pressure_switch"));
    }
}
