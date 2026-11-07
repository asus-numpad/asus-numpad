/* ============================================================
   asus-numpad — site script
   ============================================================ */
(function () {
  "use strict";

  /* ---------------- i18n ---------------- */
  var ZH_CN = {
    "nav.features": "特性",
    "nav.how": "原理",
    "nav.models": "机型",
    "nav.usage": "命令",
    "nav.install": "安装",
    "nav.faq": "问答",

    "hero.eyebrow": "用户态驱动 · GPL-3.0",
    "hero.title1": "数字小键盘，",
    "hero.title2": "在 Linux 上。无需改内核。",
    "hero.lede": "通过 evdev 抓取触摸板，把触点映射到数字键盘格位，再经 uinput 发出按键。原厂内核、原厂 ACPI，卸载后恢复原有行为。",
    "hero.cta1": "开始安装",
    "hero.cta2": "下载安装包",
    "hero.note": "一条命令识别发行版、导入公钥并安装；逐步命令见<a href=\"#install\">安装</a>。",
    "copy": "复制",
    "theme.toggle": "切换主题",
    "a11y.skip": "跳到正文",
    "a11y.nav": "主导航",
    "a11y.code": "安装命令",

    "strip.label": "已打包适配",

    "feat.title": "不改内核与固件",
    "feat.sub": "不装内核模块、不需要每次更新后重编 dkms、也不改动任何固件表。",
    "feat.1.t": "不动系统组件",
    "feat.1.d": "输入走 evdev，输出走 uinput。不替换厂商模块、不覆盖 ACPI 对象，内核升级不受影响，卸载后回到原有行为。",
    "feat.2.t": "每种包格式各构建一次",
    "feat.2.d": "deb 在 glibc 2.35 上编译，覆盖 Ubuntu 22.04–24.04 与 Debian 12/13；rpm 在 el9 上编译，覆盖 RHEL 9–10、Fedora 与 openSUSE。发版前 CI 会把同一份包装进每个目标系统实测。仓库与 release 附件只提供 x86_64。",
    "feat.3.t": "运行期识别机型",
    "feat.3.d": "由 DMI 字段与触摸板模组决定格位中心与键表，包名不带机型，也就没有装错包的问题。<code>asus-numpad sku</code> 会说明识别结果和依据。",
    "feat.4.t": "四档背光与跟随",
    "feat.4.d": "默认通过 hidraw FEATURE <code>0x0d</code> 跟随键盘背光；档位值在 0x41–0x48 内可改写，手动模式提供 <code>bright up|down|cycle</code>。",
    "feat.5.t": "布局门禁",
    "feat.5.d": "44 个触摸板模组映射到厂商 INF 里的键表，每份标注 verified / family / unsupported。未识别的模组在启动阶段即被拒绝，不会输出按键。",
    "feat.6.t": "英文与两种中文",
    "feat.6.d": "界面与提示分 en、zh-CN、zh-TW 三套，man 页只有英文。取值顺序为 <code>ASUS_NUMPAD_LANG</code>、<code>--lang</code>、配置文件、系统语言环境。",

    "how.title": "工作原理",
    "how.sub": "一个用户态进程，三个阶段；不涉及内核代码。",
    "how.1.t": "抓取触摸板",
    "how.1.d": "打开并 grab 触摸板的多点触控 evdev 节点，触点只由驱动读取，桌面指针不再响应。",
    "how.2.t": "触点映射格位",
    "how.2.d": "格位中心、按下阈值与 62 帧长按参数取自厂商文件 <code>AsusNUMPADFilter.inf</code> 的 <code>AlgoParameters</code>，按代际微调。",
    "how.3.t": "注入按键",
    "how.3.d": "通过 uinput 建立虚拟键盘并发出 KP_* 事件；背光字节随后经 hidraw 或 i2c 写出。",

    "models.m533": "枪神6 / 魔霸6，15 英寸（2022）：G533*、G513*",
    "models.m614": "枪神7 及超竞版、枪神8 及超竞版，16 英寸（2023–2024）：G614、G634",
    "models.m615": "枪神9 及超竞版、枪神10 及超竞版，16 英寸（2025–2026）：G615、G635",
    "models.mgx": "冰刃双屏、冰刃 6 双屏、冰刃 7 双屏（Duo 15 / Duo 15 SE / Duo 16，2020–2023）：GX550、GX551、GX650",
    "models.mux": "灵耀 X 双屏、灵耀 X 双屏 Pro（UX482 / UX582，2020–2021）：UX582、UX482",
    "models.title": "支持机型",
    "models.sub": "下列机型即支持范围。机型 id 用于手动切换参数；表外的机器在启动时被拒绝，表内但没有 NumberPad 硬件的机器同样被拒绝。",
    "models.n533": "2021 款不带 NumberPad，不会被匹配",
    "models.n614": "已在 <code>G634JY</code> 真机验证；国行「魔霸新锐」为 4 区 RGB 版，不带此硬件",
    "models.n615": "2026 款沿用同一代号；国行「魔霸新锐 2025」为 4 区 RGB 版，不带此硬件",
    "models.ngx": "侧置触摸板，数字模式由键盘专用键切换",
    "models.nux": "侧置触摸板，数字模式由键盘专用键切换",
    "models.shots": "产品图取自华硕官网产品页，与上列机型一一对应。",
    "models.fine": "DMI 里的机型代号为准，宣传名称仅供参考。硬件读不准时用 <code>asus-numpad sku g614</code> 手动指定，<code>asus-numpad sku auto</code> 恢复自动识别。",
    "models.gate": "触摸板本身是最后一道判据。机型代号对上之后，驱动还要在 HID 里找到 FEATURE 报告 <code>0x0d</code>，找不到就拒绝启动：同型号的 4 区 RGB 配置用同一块触摸板模组，但没有图标区，无法绘制数字键。",

    "inst.title": "安装",
    "inst.sub": "软件源加一次，之后随包管理器升级。索引带 GPG 签名，公钥随索引一起发布。",
    "inst.oneline.n": "识别发行版、导入签名公钥、写入软件源并安装；加 <code>--repo-only</code> 只写源不安装。",
    "inst.apt.n": "Debian 12/13、Ubuntu 22.04–24.04、Mint 21+",
    "inst.dnf.n": "Fedora、RHEL / Rocky / Alma 9–10",
    "inst.zypper.n": "openSUSE Leap 16 与 Tumbleweed 直接读同一个 rpm-md 源",
    "inst.pacman.n": "Arch、Manjaro、EndeavourOS，AUR 源码包名为 <code>asus-numpad-rs</code>",
    "inst.apk.n": "Alpine 3.19+（musl 构建）",
    "inst.nix.n": "Flake，无需添加源",
    "inst.notice": "从旧仓库 <code>RunningVAN/asus-numpad</code> 过来的用户：包名相同但来源不同，无法就地升级，换源前先执行一次 <code>sudo apt remove asus-numpad</code>（rpm 用 <code>dnf remove</code>）。这里的源只发布 1.0.0 起，版本号低于旧的 1.2.4，包管理器不会提示升级。",
    "inst.other1.t": "Gentoo 与静态 tarball",
    "inst.other1.d": "Releases 里有适合 ebuild 的静态 tarball，也有 deb、rpm、pkg.tar.zst、apk 供离线安装。",
    "inst.other1.a": "前往 Releases",
    "inst.other2.t": "装完先自检",
    "inst.other2.d": "只读自检，输出设备、机型、布局支持级别与背光通道。",

    "use.title": "常用命令",
    "use.sub": "守护进程由 systemd 拉起，命令行用于排查与调参。",
    "use.a.t": "运行与自检",
    "use.b.t": "背光",
    "use.c.t": "布局与语言",
    "use.fine": "配置位于 <code>~/.config/asus-numpad/asus-numpad.json</code>，以 root 运行时读 <code>/etc/asus-numpad/</code>。未写的键沿用厂商默认值：<code>numlock</code>、<code>mixed_mode</code>、<code>grab</code>、<code>follow_keyboard_brightness</code>、<code>allow_unverified_layout</code> 以及两张亮度表。",

    "faq.title": "常见问题",
    "faq.1.q": "会替代 asusctl 或内核的 hid-asus 吗？",
    "faq.1.d": "不会，它只是并行存在：抓取触摸板 evdev 节点，另建一个虚拟键盘。asusctl 对键盘背光的控制照常可用，卸载本包即去掉数字键盘这一层。",
    "faq.2.q": "触摸板的指针功能会失效吗？",
    "faq.2.d": "仅在数字模式开启期间被 grab。把 <code>grab</code> 关掉，点击仍能到达桌面；开启 <code>mixed_mode</code> 且 <code>/dev/uinput</code> 可用时，数字键与指针可同时工作。",
    "faq.3.q": "我的机型不在表里，或者没有 NumberPad，能用吗？",
    "faq.3.d": "一行都不匹配的机器拒绝启动：闸门以退出码 3 结束并打印支持机型，不使用其他机型的参数运行。<code>--sku</code> 或 <code>asus-numpad sku &lt;id&gt;</code> 是放行通道，<code>asus-numpad check</code> 会打印需要反馈的信息。机型在表里但没这块硬件的，同样以退出码 3 拦下：驱动读 HID 的 FEATURE 报告 <code>0x0d</code>，4 区 RGB 配置没有这条。",
    "faq.4.q": "签名公钥在哪，怎么核对？",
    "faq.4.d": "apt、rpm、pacman 的索引共用一把 GPG 公钥，Alpine 的包另用一把 RSA 公钥，两把都随索引放在仓库的 <code>keys/</code> 下，<code>install.sh</code> 直接取来导入，不用另外下载。指纹：<code>79F2 8215 7F2D 3FF8 E3C6 8063 14C2 BF61 1986 E168</code>，细节见 <code>docs/signing.md</code>。",
    "faq.5.q": "冰刃双屏、灵耀双屏这类机型呢？",
    "faq.5.d": "支持，但有差异：这类机器的触摸板是键盘右侧的竖条，数字模式由键盘专用键切换，而不是右上角的 62 帧长按。可用 <code>numlock</code> 固定模式。",

    "faq.6.q": "需要 root 或重启吗？",
    "faq.6.d": "都不需要。守护进程以 <code>systemd --user</code> 单元运行，udev 把触摸板与背光节点授权给会话。首次安装后注销再登录一次，让单元与 udev 规则生效。",
    "faq.7.q": "Wayland 还是 X11，有区别吗？",
    "faq.7.d": "没有。守护进程只与 evdev、uinput 打交道，在 Wayland、X11 和纯 TTY 下表现一致。真正相关的是会话权限：拿不到 <code>/dev/uinput</code> 时按键无处可去。",

    "foot.note": "ASUS 触摸板数字小键盘（NumberPad）的 Linux 用户态驱动。GPL-3.0-or-later。",
    "foot.project": "项目",
    "foot.machines": "机型说明",
    "foot.docs": "文档",
    "foot.install": "安装指南",
    "foot.verify": "发布记录",
    "foot.right": "© 2026 asus-numpad · GPL-3.0-or-later"
  };

  var ZH_TW = {
    "nav.features": "功能",
    "nav.how": "原理",
    "nav.models": "機型",
    "nav.usage": "指令",
    "nav.install": "安裝",
    "nav.faq": "問答",

    "hero.eyebrow": "使用者空間驅動 · GPL-3.0",
    "hero.title1": "數字小鍵盤，",
    "hero.title2": "在 Linux 上。無需改核心。",
    "hero.lede": "透過 evdev 抓取觸控板，把觸點對應到數字鍵盤格位，再經 uinput 送出按鍵。原廠核心、原廠 ACPI，解除安裝後回復原有行為。",
    "hero.cta1": "開始安裝",
    "hero.cta2": "下載套件",
    "hero.note": "一行指令會辨識發行版、匯入公鑰並安裝；逐步指令見<a href=\"#install\">安裝</a>。",
    "copy": "複製",
    "theme.toggle": "切換主題",
    "a11y.skip": "跳至正文",
    "a11y.nav": "主導覽",
    "a11y.code": "安裝指令",

    "strip.label": "已打包適配",

    "feat.title": "不改核心與韌體",
    "feat.sub": "不裝核心模組、不需要每次更新後重編 dkms、也不改動任何韌體表。",
    "feat.1.t": "不動系統元件",
    "feat.1.d": "evdev 收，uinput 發。不替換廠商模組、不覆寫 ACPI 物件，核心升級不會失效，移除套件後即回復原有行為。",
    "feat.2.t": "每種套件格式各建置一次",
    "feat.2.d": "deb 在 glibc 2.35 上編譯，涵蓋 Ubuntu 22.04–24.04 與 Debian 12/13；rpm 在 el9 上編譯，涵蓋 RHEL 9–10、Fedora 與 openSUSE。發版前 CI 會把同一份套件裝進每個目標系統實測。倉庫與 release 附件只提供 x86_64。",
    "feat.3.t": "執行期辨識機型",
    "feat.3.d": "由 DMI 欄位與觸控板模組決定格位中心與鍵表，套件名稱不帶機型，也就沒有裝錯套件的問題。<code>asus-numpad sku</code> 會說明辨識結果與依據。",
    "feat.4.t": "四段背光與跟隨",
    "feat.4.d": "預設透過 hidraw FEATURE <code>0x0d</code> 跟隨鍵盤背光；檔位值可在 0x41–0x48 內改寫，手動模式提供 <code>bright up|down|cycle</code>。",
    "feat.5.t": "佈局閘門",
    "feat.5.d": "44 個觸控板模組對應到廠商 INF 裡的鍵表，每份標註 verified / family / unsupported。未辨識的模組在啟動階段即被拒絕，不會送出按鍵。",
    "feat.6.t": "英文與兩種中文",
    "feat.6.d": "介面與提示分 en、zh-CN、zh-TW 三套，man 頁只有英文。優先順序為 <code>ASUS_NUMPAD_LANG</code>、<code>--lang</code>、設定檔、系統語言環境。",

    "how.title": "工作原理",
    "how.sub": "一個使用者空間行程，三個階段；不涉及核心程式碼。",
    "how.1.t": "抓取觸控板",
    "how.1.d": "開啟並 grab 觸控板的多點觸控 evdev 節點，觸點被讀走，桌面指標不會跟著亂動。",
    "how.2.t": "觸點對應格位",
    "how.2.d": "格位中心、按下閾值與 62 幀長按參數取自廠商檔案 <code>AsusNUMPADFilter.inf</code> 的 <code>AlgoParameters</code>，依世代微調。",
    "how.3.t": "注入按鍵",
    "how.3.d": "透過 uinput 建立虛擬鍵盤並送出 KP_* 事件；背光位元組接著經 hidraw 或 i2c 寫出。",

    "models.m533": "ROG Strix SCAR 15 / Strix G15，15 吋（2022）：G533*、G513*",
    "models.m614": "ROG Strix SCAR 16 / Strix G16，16 吋（2023–2024）：G614、G634",
    "models.m615": "ROG Strix SCAR 16 / Strix G16，16 吋（2025–2026）：G615、G635",
    "models.mgx": "ROG Zephyrus Duo 15 / Duo 15 SE / Duo 16（西風之神，2020–2023）：GX550、GX551、GX650",
    "models.mux": "ASUS Zenbook Duo 14 / Zenbook Pro Duo 15 OLED（2020–2021）：UX582、UX482",
    "models.title": "支援機型",
    "models.sub": "下列機型即支援範圍。機型 id 用於手動切換參數；表外的機器在啟動時被拒絕，表內但沒有 NumberPad 硬體的機器同樣被拒絕。",
    "models.n533": "2021 年款不含 NumberPad，不會被對應",
    "models.n614": "已在 <code>G634JY</code> 實機驗證；中國市場「魔霸新銳」為 4 區 RGB 版，不含此硬體",
    "models.n615": "2026 年款沿用同一代號；2025 年款「魔霸新銳」同為 4 區 RGB 版",
    "models.ngx": "側置觸控板，數字模式由鍵盤專用鍵切換",
    "models.nux": "側置觸控板，數字模式由鍵盤專用鍵切換",
    "models.shots": "產品圖取自華碩官網產品頁，與上列機型一一對應。",
    "models.fine": "以 DMI 裡的機型代號為準，宣傳名稱僅供對照。硬體讀不準時用 <code>asus-numpad sku g614</code> 手動指定，<code>asus-numpad sku auto</code> 回復自動辨識。",
    "models.gate": "觸控板本身是最後一道判據。機型代號對上之後，驅動還要在 HID 裡找到 FEATURE 報告 <code>0x0d</code>，找不到就拒絕啟動：同型號的 4 區 RGB 配置用同一塊觸控板模組，但沒有圖示區，無法繪製數字鍵。",

    "inst.title": "安裝",
    "inst.sub": "軟體來源加入一次，之後隨套件管理員升級。索引帶 GPG 簽章，公鑰隨索引一起發佈。",
    "inst.oneline.n": "辨識發行版、匯入簽章公鑰、寫入軟體來源並安裝；加 <code>--repo-only</code> 只寫來源不安裝。",
    "inst.apt.n": "Debian 12/13、Ubuntu 22.04–24.04、Mint 21+",
    "inst.dnf.n": "Fedora、RHEL / Rocky / Alma 9–10",
    "inst.zypper.n": "openSUSE Leap 16 與 Tumbleweed 直接讀同一個 rpm-md 來源",
    "inst.pacman.n": "Arch、Manjaro、EndeavourOS，AUR 來源套件名為 <code>asus-numpad-rs</code>",
    "inst.apk.n": "Alpine 3.19+（musl 建置）",
    "inst.nix.n": "Flake，無需加入來源",
    "inst.notice": "從舊倉庫 <code>RunningVAN/asus-numpad</code> 過來的使用者：套件名稱相同但來源不同，無法就地升級，換來源前先執行一次 <code>sudo apt remove asus-numpad</code>（rpm 用 <code>dnf remove</code>）。這裡的源只發佈 1.0.0 起，版本號低於舊的 1.2.4，套件管理器不會提示升級。",
    "inst.other1.t": "Gentoo 與靜態 tarball",
    "inst.other1.d": "Releases 裡有適合 ebuild 的靜態 tarball，也有 deb、rpm、pkg.tar.zst、apk 供離線安裝。",
    "inst.other1.a": "前往 Releases",
    "inst.other2.t": "裝完先自我檢查",
    "inst.other2.d": "唯讀自我檢查，輸出裝置、機型、佈局支援等級與背光通道。",

    "use.title": "常用指令",
    "use.sub": "守護行程由 systemd 拉起，命令列用於排查與調參。",
    "use.a.t": "執行與自我檢查",
    "use.b.t": "背光",
    "use.c.t": "佈局與語言",
    "use.fine": "設定檔位於 <code>~/.config/asus-numpad/asus-numpad.json</code>，以 root 執行時讀 <code>/etc/asus-numpad/</code>。未寫的鍵沿用廠商預設值：<code>numlock</code>、<code>mixed_mode</code>、<code>grab</code>、<code>follow_keyboard_brightness</code>、<code>allow_unverified_layout</code> 以及兩張亮度表。",

    "faq.title": "常見問題",
    "faq.1.q": "會取代 asusctl 或核心的 hid-asus 嗎？",
    "faq.1.d": "不會，它只是並存：抓取觸控板 evdev 節點，另建一個虛擬鍵盤。asusctl 對鍵盤背光的控制照常可用，移除本套件即去掉數字鍵盤這一層。",
    "faq.2.q": "觸控板的指標功能會失效嗎？",
    "faq.2.d": "僅在數字模式開啟期間被 grab。把 <code>grab</code> 關掉，點擊仍能到達桌面；開啟 <code>mixed_mode</code> 且 <code>/dev/uinput</code> 可用時，數字鍵與指標可同時運作。",
    "faq.3.q": "我的機型不在表裡，或者沒有 NumberPad，能用嗎？",
    "faq.3.d": "一行都不匹配的機器拒絕啟動：閘門以結束碼 3 中止並列出支援機型，不使用其他機型的參數執行。<code>--sku</code> 或 <code>asus-numpad sku &lt;id&gt;</code> 是放行通道，<code>asus-numpad check</code> 會列出需要回報的資訊。機型在表裡但沒有這塊硬體的，同樣以結束碼 3 攔下：驅動讀 HID 的 FEATURE 報告 <code>0x0d</code>，4 區 RGB 配置沒有這一條。",
    "faq.4.q": "簽章公鑰在哪裡，怎麼核對？",
    "faq.4.d": "apt、rpm、pacman 的索引共用一把 GPG 公鑰，Alpine 的套件另用一把 RSA 公鑰，兩把都隨索引放在倉庫的 <code>keys/</code> 下，<code>install.sh</code> 直接取來匯入，不用另外下載。指紋：<code>79F2 8215 7F2D 3FF8 E3C6 8063 14C2 BF61 1986 E168</code>，細節見 <code>docs/signing.md</code>。",
    "faq.5.q": "雙螢幕機型（Zephyrus Duo、Zenbook Duo）呢？",
    "faq.5.d": "支援，但有差異：這類機器的觸控板是鍵盤右側的豎條，數字模式由鍵盤專用鍵切換，而不是右上角的 62 幀長按。可用 <code>numlock</code> 固定模式。",
    "faq.6.q": "需要 root 或重新開機嗎？",
    "faq.6.d": "都不需要。守護行程以 <code>systemd --user</code> 單元執行，udev 把觸控板與背光節點授權給工作階段。首次安裝後登出再登入一次，讓單元與 udev 規則生效。",
    "faq.7.q": "Wayland 還是 X11，有差別嗎？",
    "faq.7.d": "沒有。守護行程只與 evdev、uinput 打交道，在 Wayland、X11 與純 TTY 下表現一致。真正相關的是工作階段權限：取不到 <code>/dev/uinput</code> 時按鍵無處可去。",

    "foot.note": "ASUS 觸控板數字小鍵盤（NumberPad）的 Linux 使用者空間驅動程式。GPL-3.0-or-later。",
    "foot.project": "專案",
    "foot.machines": "機型說明",
    "foot.docs": "文件",
    "foot.install": "安裝指南",
    "foot.verify": "發佈記錄",
    "foot.right": "© 2026 asus-numpad · GPL-3.0-or-later"
  };

  var I18N = { en: {}, "zh-CN": ZH_CN, "zh-TW": ZH_TW };
  var zhNodes = document.querySelectorAll("[data-i18n]");
  var ariaNodes = document.querySelectorAll("[data-i18n-aria]");
  // snapshot the English source text once
  zhNodes.forEach(function (el) { I18N.en[el.dataset.i18n] = el.innerHTML; });
  /* keys that exist only as JS-side labels (no markup node to snapshot) */
  I18N.en["theme.toggle"] = "Toggle theme";
  /* 只出现在 aria-label 上的键：标签文字是符号，没有可快照的可见文本 */
  I18N.en["a11y.nav"] = "Main";
  I18N.en["a11y.code"] = "Install command";

  var LANGS = ["en", "zh-CN", "zh-TW"];
  var LANG_LABEL = { en: "中", "zh-CN": "繁", "zh-TW": "EN" };
  var LANG_TITLE = {
    en: "切换到简体中文",
    "zh-CN": "切换到繁体中文",
    "zh-TW": "Switch to English"
  };

  var langBtn = document.getElementById("lang-toggle");
  var stored = null;
  try { stored = localStorage.getItem("an-lang"); } catch (e) {}
  if (stored === "zh") stored = "zh-CN";
  function detectLang() {
    var n = (navigator.language || "en").toLowerCase();
    if (n.indexOf("zh") !== 0) return "en";
    if (n.indexOf("hant") >= 0 || /-(tw|hk|mo)\b/.test(n)) return "zh-TW";
    return "zh-CN";
  }
  var lang = LANGS.indexOf(stored) >= 0 ? stored : detectLang();

  function applyLang(next) {
    lang = next;
    var dict = I18N[next] || {};
    document.documentElement.lang = next;
    zhNodes.forEach(function (el) {
      var key = el.dataset.i18n;
      var v = dict[key] != null ? dict[key] : I18N.en[key];
      if (v != null) el.innerHTML = v;
    });
    ariaNodes.forEach(function (el) {
      var key = el.dataset.i18nAria;
      var v = dict[key] != null ? dict[key] : I18N.en[key];
      if (v != null) el.setAttribute("aria-label", v);
    });
    var themeBtnEl = document.getElementById("theme-toggle");
    if (themeBtnEl) {
      var themeLabel = dict["theme.toggle"] || I18N.en["theme.toggle"];
      themeBtnEl.title = themeLabel;
      themeBtnEl.setAttribute("aria-label", themeLabel);
    }
    langBtn.textContent = LANG_LABEL[next];
    langBtn.title = LANG_TITLE[next];
    langBtn.setAttribute("aria-label", LANG_TITLE[next]);
    try { localStorage.setItem("an-lang", next); } catch (e) {}
  }
  langBtn.addEventListener("click", function () {
    applyLang(LANGS[(LANGS.indexOf(lang) + 1) % LANGS.length]);
  });
  applyLang(lang);

  /* ---------------- theme ---------------- */
  /* 没手动选过就跟随系统（首屏那一下由 <head> 里的内联脚本先定好，不会闪）；
     点过开关才写 localStorage，写完之后不再跟系统走。 */
  var THEME_KEY = "an-theme";
  var themeBtn = document.getElementById("theme-toggle");
  var darkQuery = window.matchMedia("(prefers-color-scheme: dark)");
  function savedTheme() {
    var v = null;
    try { v = localStorage.getItem(THEME_KEY); } catch (e) {}
    return v === "light" || v === "dark" ? v : null;
  }
  function applyTheme(next, remember) {
    document.documentElement.setAttribute("data-theme", next);
    if (remember) {
      try { localStorage.setItem(THEME_KEY, next); } catch (e) {}
    }
  }
  var known = savedTheme();
  applyTheme(known || (darkQuery.matches ? "dark" : "light"), false);
  themeBtn.addEventListener("click", function () {
    var now = document.documentElement.getAttribute("data-theme") === "dark" ? "light" : "dark";
    applyTheme(now, true);
  });
  function onSystemTheme(e) {
    if (savedTheme()) return;
    applyTheme(e.matches ? "dark" : "light", false);
  }
  if (darkQuery.addEventListener) darkQuery.addEventListener("change", onSystemTheme);

  /* ---------------- numpad visual ---------------- */
  /* 与驱动里的键表一致：5 列 4 行，右上角是 NumLock 图标区，0 横跨两列、
     回车纵跨两行（layout.rs 的 KEY_GRID）。 */
  var padLayout = [
    { t: "7" }, { t: "8" }, { t: "9" }, { t: "/" }, { t: "\u25c9", icon: true },
    { t: "4" }, { t: "5" }, { t: "6" }, { t: "*" }, { t: "\u232b" },
    { t: "1" }, { t: "2" }, { t: "3" }, { t: "-" }, { t: "\u21b5", tall: true },
    { t: "0", wide: true }, { t: "." }, { t: "+" }
  ];
  var grid = document.getElementById("pad-grid");
  if (grid) {
    padLayout.forEach(function (c, i) {
      var d = document.createElement("div");
      d.className =
        "cell" +
        (c.icon ? " icon" : "") +
        (c.wide ? " wide" : "") +
        (c.tall ? " tall" : "") +
        (/[0-9]/.test(c.t) ? "" : " op");
      d.textContent = c.t;
      d.dataset.i = i;
      grid.appendChild(d);
    });
    var cells = grid.children;
    var seq = [0, 5, 10, 15, 16, 17, 13, 8, 3, 1];
    var step = 0;
    if (!window.matchMedia("(prefers-reduced-motion: reduce)").matches) {
      setInterval(function () {
        for (var i = 0; i < cells.length; i++) cells[i].classList.remove("lit");
        cells[seq[step % seq.length]].classList.add("lit");
        step++;
      }, 620);
    } else {
      cells[4].classList.add("lit");
    }
  }

  /* ---------------- copy buttons ---------------- */
  function copyText(text, btn) {
    var done = function () {
      var old = btn.querySelector("span") ? btn.querySelector("span").textContent : btn.textContent;
      var copied = lang === "zh-TW" ? "已複製" : lang === "zh-CN" ? "已复制" : "Copied";
      btn.classList.add("done");
      if (btn.querySelector("span")) btn.querySelector("span").textContent = copied;
      else btn.textContent = copied;
      setTimeout(function () {
        btn.classList.remove("done");
        if (btn.querySelector("span")) btn.querySelector("span").textContent = old;
        else btn.textContent = old;
      }, 1600);
    };
    if (navigator.clipboard && window.isSecureContext) {
      navigator.clipboard.writeText(text).then(done, function () { fallback(text, done); });
    } else {
      fallback(text, done);
    }
  }
  function fallback(text, done) {
    var ta = document.createElement("textarea");
    ta.value = text;
    ta.style.position = "fixed";
    ta.style.opacity = "0";
    document.body.appendChild(ta);
    ta.select();
    try { document.execCommand("copy"); done(); } catch (e) {}
    document.body.removeChild(ta);
  }

  var quickBody = document.querySelector(".quick-body");
  if (quickBody) {
    quickBody.querySelector(".copy-btn").addEventListener("click", function () {
      var shown = quickBody.querySelector(".qcode:not(.hidden)");
      if (shown) copyText(shown.textContent.trim(), this);
    });
  }

  var pane = document.querySelector(".install");
  if (pane) {
    pane.querySelector(".copy-pane").addEventListener("click", function () {
      var shown = pane.querySelector(".pane:not(.hidden)");
      if (shown) copyText(shown.querySelector("pre").textContent.replace(/\s+$/, ""), this);
    });
  }

  /* ---------------- quick tabs ---------------- */
  var qtabs = [].slice.call(document.querySelectorAll(".qtab"));
  function selectQuick(t) {
    qtabs.forEach(function (x) {
      var on = x === t;
      x.classList.toggle("active", on);
      x.setAttribute("aria-pressed", on ? "true" : "false");
      x.tabIndex = on ? 0 : -1;
    });
    document.querySelectorAll("[data-qt-body]").forEach(function (b) {
      b.classList.toggle("hidden", b.dataset.qtBody !== t.dataset.qt);
    });
  }
  qtabs.forEach(function (t) {
    t.addEventListener("click", function () { selectQuick(t); });
  });

  /* ---------------- install tabs ---------------- */
  var tabs = [].slice.call(document.querySelectorAll(".tab"));
  function selectTab(t) {
    tabs.forEach(function (x) {
      var on = x === t;
      x.classList.toggle("active", on);
      x.setAttribute("aria-selected", on ? "true" : "false");
      x.tabIndex = on ? 0 : -1;
    });
    document.querySelectorAll("[data-pane]").forEach(function (p) {
      p.classList.toggle("hidden", p.dataset.pane !== t.dataset.tab);
    });
  }
  tabs.forEach(function (t) {
    t.addEventListener("click", function () { selectTab(t); });
  });
  /* 先按当前选中项摆正 roving tabindex，否则 Tab 会依次停在所有未选中的标签上 */
  selectQuick(document.querySelector(".qtab.active") || qtabs[0]);
  selectTab(document.querySelector(".tab.active") || tabs[0]);

  /* 标签页按方向键在选项之间移动，Home / End 跳首尾 */
  [qtabs, tabs].forEach(function (group) {
    group.forEach(function (t, i) {
      t.addEventListener("keydown", function (e) {
        var to = -1;
        if (e.key === "ArrowRight" || e.key === "ArrowDown") to = (i + 1) % group.length;
        else if (e.key === "ArrowLeft" || e.key === "ArrowUp") to = (i - 1 + group.length) % group.length;
        else if (e.key === "Home") to = 0;
        else if (e.key === "End") to = group.length - 1;
        if (to < 0) return;
        e.preventDefault();
        group[to].focus();
        group[to].click();
      });
    });
  });

  /* ---------------- nav shadow ---------------- */
  var nav = document.getElementById("nav");
  var onScroll = function () {
    nav.classList.toggle("scrolled", window.scrollY > 8);
  };
  window.addEventListener("scroll", onScroll, { passive: true });
  onScroll();

  /* ---------------- reveal ---------------- */
  var reveals = document.querySelectorAll(".reveal");
  if ("IntersectionObserver" in window) {
    var io = new IntersectionObserver(function (entries) {
      entries.forEach(function (e) {
        if (e.isIntersecting) {
          e.target.classList.add("in");
          io.unobserve(e.target);
        }
      });
    }, { rootMargin: "0px 0px -8% 0px", threshold: .08 });
    reveals.forEach(function (el, i) {
      el.style.transitionDelay = (Math.min(i % 6, 5) * 45) + "ms";
      io.observe(el);
    });
  } else {
    reveals.forEach(function (el) { el.classList.add("in"); });
  }
})();
