/* ============================================================
   numdeck — site script
   ============================================================ */
(function () {
  "use strict";

  /* ---------------- i18n ---------------- */
  var ZH_CN = {
    "lang.label": "页面语言",
    "hero.ledeLead": "内核与 ACPI 保持原厂状态：全部逻辑位于用户态。",
    "hero.fact1": "不装内核模块",
    "hero.fact2": "单条命令安装",
    "hero.fact3": "安装包 GPG 签名",
    "nav.features": "特性",
    "nav.how": "原理",
    "nav.models": "机型",
    "nav.usage": "命令",
    "nav.install": "安装",
    "nav.faq": "问答",

    "hero.eyebrow": "用户态驱动 · GPL-3.0",
    "hero.title1": "触控板数字小键盘",
    "hero.title2": "原生 Linux 直接可用",
    "hero.lede": "驱动通过 evdev 读取触摸板触点，按格位映射为数字键，经 uinput 发出。卸载后触摸板恢复原状。",
    "hero.cta1": "开始安装",
    "hero.cta2": "下载安装包",
    "hero.note": "脚本会添加软件源并安装软件包。分步命令见<a href=\"#install\">安装</a>。",
    "copy": "复制",
    "copy.pane": "复制当前命令",
    "theme.toggle": "切换主题",
    "a11y.skip": "跳到正文",
    "a11y.nav": "主导航",
    "a11y.code": "安装命令",
    "a11y.quick": "快速安装",

    "strip.label": "发行版打包",

    "feat.title": "不改内核与固件",
    "feat.sub": "不加载内核模块，内核更新后无需重编 dkms，也不修改固件表。",
    "feat.1.t": "不动系统组件",
    "feat.1.d": "输入取自 evdev，输出经 uinput。不替换厂商模块，不覆盖 ACPI 对象，内核升级不受影响，卸载后恢复原状。",
    "feat.2.t": "每种包格式单独构建",
    "feat.2.d": "deb 在 glibc 2.35 上编译，覆盖 Ubuntu 22.04–24.04 与 Debian 12/13；rpm 在 el9 上编译，覆盖 RHEL 9–10、Fedora 与 openSUSE。发版前 CI 在对应发行版容器里各自构建一次，并校验产物元数据。仓库与 release 附件提供 x86_64 与 ARM64（aarch64）两种架构；ARM64 包不带机型表，只按自定义布局运行。",
    "feat.3.t": "运行期识别机型",
    "feat.3.d": "由 DMI 字段与触摸板模组决定格位中心与键表，包名不含机型，因此不存在装错包的问题。<code>numdeck sku</code> 会说明识别结果和依据。机型表与 <code>0x0d</code> 探测只属于 x86_64 构建；ARM64 包按触摸板上报的能力直接用，走 <code>custom_layouts</code>。",
    "feat.4.t": "四档背光与亮度跟随",
    "feat.4.d": "默认通过 hidraw FEATURE <code>0x0d</code> 跟随键盘背光；档位值在 0x41–0x48 内可改写，手动模式提供 <code>bright up|down|cycle</code>。",
    "feat.5.t": "布局校验",
    "feat.5.d": "44 个触摸板模组映射到厂商 INF 里的键表，每份标注 verified / family / unsupported。表里没有的模组默认在启动阶段即被拒绝，自己配一份键表（<code>custom_layouts</code>）就能跑起来。",
    "feat.6.t": "英文与两种中文",
    "feat.6.d": "界面与提示分 en、zh-CN、zh-TW 三套，man 页只有英文。取值顺序为 <code>NUMDECK_LANG</code>、<code>--lang</code>、配置文件、系统语言环境。",
    "feat.7.t": "表外机型也能用",
    "feat.7.d": "自定义键表、改写点亮报文、压感切换与全局热键都在同一份配置里，分别是 <code>custom_layouts</code>、<code>led_proto</code>、<code>pressure_switch</code>、<code>hotkeys</code>。键表可以继承内置表，只改对不上的那几格。",

    "how.title": "工作原理",
    "how.sub": "单个用户态进程，三个阶段，不涉及内核代码。",
    "how.1.t": "抓取触摸板",
    "how.1.d": "驱动打开并独占触摸板的 evdev 节点，触点由驱动读取，桌面指针不受影响。",
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
    "models.sub": "下列机型构成支持范围。机型 id 用于手动切换参数；表外机型在启动时被拒绝，表内但无 NumberPad 硬件的机型同样被拒绝。此表只随 x86_64 构建发布。",
    "models.n533": "2021 款不带 NumberPad，不参与匹配",
    "models.n614": "已在 <code>G634JY</code> 真机验证；国行「魔霸新锐」为 4 区 RGB 版，不带此硬件",
    "models.n615": "2026 款沿用同一代号；国行「魔霸新锐 2025」为 4 区 RGB 版，不带此硬件",
    "models.ngx": "侧置触摸板，数字模式由键盘专用键切换",
    "models.nux": "侧置触摸板，数字模式由键盘专用键切换",
    "models.shots": "产品图取自华硕官网产品页，与上列机型一一对应。",
    "models.fine": "DMI 里的机型代号为准，宣传名称仅供参考。硬件读不准时用 <code>numdeck sku g614</code> 手动指定，<code>numdeck sku auto</code> 恢复自动识别。机型 id 只存在于 x86_64 构建。",
    "models.gate": "最终判据是触摸板硬件本身。机型代号匹配之后，驱动仍要在 HID 里找到 FEATURE 报告 <code>0x0d</code>，找不到就不启动：同型号的 4 区 RGB 配置用的是同一块触摸板模组，但没有图标区，画不出数字键。上面两道检查（机型表、<code>0x0d</code> 探测）都属于 x86_64 构建；ARM64 包两道都不做，触摸板按上报的能力直接用。",

    "inst.title": "安装",
    "inst.sub": "软件源只需添加一次，之后随包管理器升级。索引带 GPG 签名，公钥随索引一起发布。",
    "inst.oneline.n": "识别发行版、导入签名公钥、写入软件源并安装；加 <code>--repo-only</code> 仅写入软件源，不安装软件包。",
    "inst.apt.n": "Debian 12/13、Ubuntu 22.04–24.04、Mint 21+",
    "inst.dnf.n": "Fedora、RHEL / Rocky / Alma 9–10",
    "inst.zypper.n": "openSUSE Leap 16 与 Tumbleweed 直接读同一个 rpm-md 源",
    "inst.pacman.n": "Arch、Manjaro、EndeavourOS，AUR 源码包名为 <code>numdeck</code>",
    "inst.apk.n": "Alpine 3.19+（musl 构建）",
    "inst.nix.n": "Flake，无需添加源",
    "inst.arch": "这些软件源同时提供 <code>x86_64</code>/<code>amd64</code> 与 <code>ARM64</code>/<code>aarch64</code>；其他架构请从源码构建：<code>cargo build --release</code>。ARM64 包不带机型表，首次 <code>run</code> 前需在 <code>/etc/numdeck/numdeck.json</code> 的 <code>custom_layouts</code> 里绑定触摸板模组与键位。",
    "inst.notice": "从旧仓库 <code>RunningVAN/numdeck</code> 过来的用户：包名相同但来源不同，无法就地升级，换源前先执行一次 <code>sudo apt remove numdeck</code>（rpm 用 <code>dnf remove</code>）。这里的源只发布 1.0.0 起，版本号低于旧的 1.2.4，包管理器不会提示升级。",
    "inst.other1.t": "Gentoo 与静态 tarball",
    "inst.other1.d": "Releases 里有适合 ebuild 的静态 tarball，也有 deb、rpm、pkg.tar.zst、apk 供离线安装。",
    "inst.other1.a": "前往 Releases",
    "inst.other2.t": "装完先自检",
    "inst.other2.d": "只读自检，输出设备、机型、布局支持级别与背光通道。",

    "use.title": "常用命令",
    "use.sub": "守护进程由 systemd 启动，命令行用于排查与调参。",
    "use.gui": "可选的 egui 窗口（<code>numdeck-gui</code>）写的是同一份 FEATURE <code>0x0d</code> 报文。Linux 上它是完整的控制中心，分七个页签：灯光（含键盘四档到触摸板亮度的映射，逐档可改）、驱动配置的全部字段、系统状态与服务启停、每个 <code>numdeck</code> 子命令的表单、扩展（自定义布局、点亮报文、压感切换、全局热键）、给表外机型做网格分区与手动落点校准的校准页，以及窗口自身的设置。命令页执行的是同一个可执行文件，不重写逻辑，也就不会漂移。界面按卡片分组，跟随系统的深浅色主题；亮度用逐档的档位条而不是滑块，一格一档。",
    "use.guiWin": "Windows 上只做灯光：接口、档位、跟随 NumLock、写入日志。小键盘输入交给官方驱动。",
    "use.shot1": "扩展页：自定义布局、重写点亮报文、压感切换、全局热键。",
    "use.shot2": "校准页：分区、手动校准、样本回放，以及对应的配置片段。",
    "use.shot3": "驱动配置页：每个字段都在旁边标出官方 INF 会取的值。",
    "use.a.t": "运行与自检",
    "use.b.t": "背光",
    "use.c.t": "布局与语言",
    "use.d.t": "机型与校准",
    "use.fine": "配置位于 <code>~/.config/numdeck/numdeck.json</code>，以 root 运行时读 <code>/etc/numdeck/</code>。未写的键沿用厂商默认值：<code>numlock</code>、<code>mixed_mode</code>、<code>grab</code>、<code>follow_keyboard_brightness</code>、<code>allow_unverified_layout</code> 以及两张亮度表。",

    "faq.title": "常见问题",
    "faq.1.q": "会替代 asusctl 或内核的 hid-asus 吗？",
    "faq.1.d": "不会。两者并行运行：抓取触摸板 evdev 节点，另建一个虚拟键盘。asusctl 对键盘背光的控制照常可用，卸载本包只移除数字键盘功能。",
    "faq.2.q": "触摸板的指针功能会失效吗？",
    "faq.2.d": "仅在数字模式开启时抓取。关闭 <code>grab</code> 后，点击事件仍发送到桌面；开启 <code>mixed_mode</code> 且 <code>/dev/uinput</code> 可用时，数字键和指针可以同时用。",
    "faq.3.q": "我的机型不在表里，或者没有 NumberPad，能用吗？",
    "faq.3.d": "机型代号无匹配项时拒绝启动：以退出码 3 退出并列出支持的机型，不沿用其他机型的参数。<code>--sku</code> 或 <code>numdeck sku &lt;id&gt;</code> 可跳过机型校验，<code>numdeck check</code> 会列出需要反馈的信息。机型在表中但无此硬件的，同样以退出码 3 拒绝启动：驱动要求 HID 的 FEATURE 报告 <code>0x0d</code>，4 区 RGB 配置不具备该报告。以上规则不适用于 ARM64 构建：它没有机型表可缺，也不做 <code>0x0d</code> 探测，表外机型不会被拒绝。",
    "faq.4.q": "签名公钥在哪，怎么核对？",
    "faq.4.d": "apt、rpm、pacman 的索引共用一个 GPG 公钥，Alpine 的包另用一个 RSA 公钥；两个公钥均随索引放在仓库的 <code>keys/</code> 下，由 <code>install.sh</code> 导入，无需另行下载。指纹：<code>79F2 8215 7F2D 3FF8 E3C6 8063 14C2 BF61 1986 E168</code>，细节见 <code>docs/signing.md</code>。",
    "faq.5.q": "冰刃双屏、灵耀双屏这类机型呢？",
    "faq.5.d": "支持，但有差异：这类机器的触摸板是键盘右侧的竖条，数字模式由键盘专用键切换，而不是右上角的 62 帧长按。可用 <code>numlock</code> 固定模式。",

    "faq.6.q": "需要 root 或重启吗？",
    "faq.6.d": "均不需要。守护进程以 <code>systemd --user</code> 单元运行，udev 把触摸板与背光节点授权给会话。首次安装后注销再登录一次，让单元与 udev 规则生效。",
    "faq.7.q": "Wayland 还是 X11，有区别吗？",
    "faq.7.d": "没有。守护进程只与 evdev、uinput 交互，在 Wayland、X11 和纯 TTY 下行为一致。实际限制来自会话权限：无法访问 <code>/dev/uinput</code> 时，按键事件没有接收端。",

    "foot.slogan": "触控板数字小键盘，Linux 原生可用。",
    "foot.meta1": "不装内核模块",
    "foot.meta2": "六种包格式",
    "foot.note": "ASUS 触摸板数字小键盘（NumberPad）的 Linux 用户态驱动。GPL-3.0-or-later。",
    "foot.project": "项目",
    "foot.machines": "机型说明",
    "foot.docs": "文档",
    "foot.install": "安装指南",
    "foot.verify": "发布记录",
    "foot.src": "纯静态页面 · 无追踪",
    "foot.right": "© 2026 numdeck · GPL-3.0-or-later"
  };

  var ZH_TW = {
    "lang.label": "頁面語言",
    "hero.ledeLead": "核心與 ACPI 保持原廠狀態：全部邏輯位於使用者空間。",
    "hero.fact1": "不裝核心模組",
    "hero.fact2": "單條指令安裝",
    "hero.fact3": "套件 GPG 簽章",
    "nav.features": "功能",
    "nav.how": "原理",
    "nav.models": "機型",
    "nav.usage": "指令",
    "nav.install": "安裝",
    "nav.faq": "問答",

    "hero.eyebrow": "使用者空間驅動 · GPL-3.0",
    "hero.title1": "觸控板數字小鍵盤",
    "hero.title2": "原生 Linux 直接可用",
    "hero.lede": "驅動經 evdev 讀取觸控板觸點，按格位對應為數字鍵，經 uinput 送出。解除安裝後觸控板回復原狀。",
    "hero.cta1": "開始安裝",
    "hero.cta2": "下載套件",
    "hero.note": "腳本會加入軟體來源並安裝套件。分步指令見<a href=\"#install\">安裝</a>。",
    "copy": "複製",
    "copy.pane": "複製目前指令",
    "theme.toggle": "切換主題",
    "a11y.skip": "跳至正文",
    "a11y.nav": "主導覽",
    "a11y.code": "安裝指令",
    "a11y.quick": "快速安裝",

    "strip.label": "發行版打包",

    "feat.title": "不改核心與韌體",
    "feat.sub": "不載入核心模組，核心更新後無需重編 dkms，也不修改韌體表。",
    "feat.1.t": "不動系統元件",
    "feat.1.d": "輸入取自 evdev，輸出經 uinput。不替換廠商模組，不覆寫 ACPI 物件，核心升級不受影響，移除套件後回復原狀。",
    "feat.2.t": "每種套件格式分開建置",
    "feat.2.d": "deb 在 glibc 2.35 上編譯，涵蓋 Ubuntu 22.04–24.04 與 Debian 12/13；rpm 在 el9 上編譯，涵蓋 RHEL 9–10、Fedora 與 openSUSE。發版前 CI 在對應發行版容器裡各自建置一次，並校驗產物中繼資料。倉庫與 release 附件提供 x86_64 與 ARM64（aarch64）兩種架構；ARM64 套件不帶機型表，僅按自訂佈局執行。",
    "feat.3.t": "執行期辨識機型",
    "feat.3.d": "由 DMI 欄位與觸控板模組決定格位中心與鍵表，套件名稱不含機型，因此不存在裝錯套件的問題。<code>numdeck sku</code> 會說明辨識結果與依據。機型表與 <code>0x0d</code> 探測只屬於 x86_64 建置；ARM64 套件按觸控板回報的能力直接用，走 <code>custom_layouts</code>。",
    "feat.4.t": "四段背光與亮度跟隨",
    "feat.4.d": "預設透過 hidraw FEATURE <code>0x0d</code> 跟隨鍵盤背光；檔位值可在 0x41–0x48 內改寫，手動模式提供 <code>bright up|down|cycle</code>。",
    "feat.5.t": "佈局校驗",
    "feat.5.d": "44 個觸控板模組對應到廠商 INF 裡的鍵表，每份標註 verified / family / unsupported。表裡沒有的模組預設在啟動階段即被拒絕，自己配一份鍵表（<code>custom_layouts</code>）就能跑起來。",
    "feat.6.t": "英文與兩種中文",
    "feat.6.d": "介面與提示分 en、zh-CN、zh-TW 三套，man 頁只有英文。優先順序為 <code>NUMDECK_LANG</code>、<code>--lang</code>、設定檔、系統語言環境。",
    "feat.7.t": "表外機型也能用",
    "feat.7.d": "自訂鍵表、改寫點亮報文、壓感切換與全域熱鍵都在同一份設定裡，分別是 <code>custom_layouts</code>、<code>led_proto</code>、<code>pressure_switch</code>、<code>hotkeys</code>。鍵表可以繼承內建表，只改對不上的那幾格。",

    "how.title": "工作原理",
    "how.sub": "單一使用者空間行程，三個階段，不涉及核心程式碼。",
    "how.1.t": "抓取觸控板",
    "how.1.d": "驅動開啟並獨占觸控板的 evdev 節點，觸點由驅動讀取，桌面指標不受影響。",
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
    "models.sub": "下列機型構成支援範圍。機型 id 用於手動切換參數；表外機型在啟動時被拒絕，表內但無 NumberPad 硬體的機型同樣被拒絕。此表僅隨 x86_64 建置發佈。",
    "models.n533": "2021 年款不含 NumberPad，不參與比對",
    "models.n614": "已在 <code>G634JY</code> 實機驗證；中國市場「魔霸新銳」為 4 區 RGB 版，不含此硬體",
    "models.n615": "2026 年款沿用同一代號；2025 年款「魔霸新銳」同為 4 區 RGB 版",
    "models.ngx": "側置觸控板，數字模式由鍵盤專用鍵切換",
    "models.nux": "側置觸控板，數字模式由鍵盤專用鍵切換",
    "models.shots": "產品圖取自華碩官網產品頁，與上列機型一一對應。",
    "models.fine": "以 DMI 裡的機型代號為準，宣傳名稱僅供對照。硬體讀不準時用 <code>numdeck sku g614</code> 手動指定，<code>numdeck sku auto</code> 回復自動辨識。機型 id 只存在於 x86_64 建置。",
    "models.gate": "最終判據是觸控板硬體本身。機型代號相符之後，驅動仍要在 HID 裡找到 FEATURE 報告 <code>0x0d</code>，找不到就不啟動：同型號的 4 區 RGB 配置用的是同一塊觸控板模組，但沒有圖示區，畫不出數字鍵。上面兩道檢查（機型表、<code>0x0d</code> 探測）都屬於 x86_64 建置；ARM64 套件兩道都不做，觸控板按回報的能力直接用。",

    "inst.title": "安裝",
    "inst.sub": "軟體來源只需加入一次，之後隨套件管理員升級。索引帶 GPG 簽章，公鑰隨索引一起發佈。",
    "inst.oneline.n": "辨識發行版、匯入簽章公鑰、寫入軟體來源並安裝；加 <code>--repo-only</code> 僅寫入軟體來源，不安裝套件。",
    "inst.apt.n": "Debian 12/13、Ubuntu 22.04–24.04、Mint 21+",
    "inst.dnf.n": "Fedora、RHEL / Rocky / Alma 9–10",
    "inst.zypper.n": "openSUSE Leap 16 與 Tumbleweed 直接讀同一個 rpm-md 來源",
    "inst.pacman.n": "Arch、Manjaro、EndeavourOS，AUR 來源套件名為 <code>numdeck</code>",
    "inst.apk.n": "Alpine 3.19+（musl 建置）",
    "inst.nix.n": "Flake，無需加入來源",
    "inst.arch": "這些軟體來源同時提供 <code>x86_64</code>/<code>amd64</code> 與 <code>ARM64</code>/<code>aarch64</code>；其他架構請從原始碼建置：<code>cargo build --release</code>。ARM64 套件不帶機型表，首次 <code>run</code> 前需在 <code>/etc/numdeck/numdeck.json</code> 的 <code>custom_layouts</code> 裡綁定觸控板模組與鍵位。",
    "inst.notice": "從舊倉庫 <code>RunningVAN/numdeck</code> 過來的使用者：套件名稱相同但來源不同，無法就地升級，換來源前先執行一次 <code>sudo apt remove numdeck</code>（rpm 用 <code>dnf remove</code>）。這裡的源只發佈 1.0.0 起，版本號低於舊的 1.2.4，套件管理器不會提示升級。",
    "inst.other1.t": "Gentoo 與靜態 tarball",
    "inst.other1.d": "Releases 裡有適合 ebuild 的靜態 tarball，也有 deb、rpm、pkg.tar.zst、apk 供離線安裝。",
    "inst.other1.a": "前往 Releases",
    "inst.other2.t": "裝完先自我檢查",
    "inst.other2.d": "唯讀自我檢查，輸出裝置、機型、佈局支援等級與背光通道。",

    "use.title": "常用指令",
    "use.sub": "守護行程由 systemd 啟動，命令列用於排查與調參。",
    "use.gui": "可選的 egui 視窗（<code>numdeck-gui</code>）寫的是同一份 FEATURE <code>0x0d</code> 報文。Linux 上它是完整的控制中心，分七個頁籤：燈光（含鍵盤四檔到觸控板亮度的對映，逐檔可改）、驅動設定的全部欄位、系統狀態與服務啟停、每個 <code>numdeck</code> 子命令的表單、擴充（自訂佈局、點亮報文、壓感切換、全域熱鍵）、給表外機型做網格分割與手動落點校準的校準頁，以及視窗自行的設定。命令頁執行的是同一個執行檔，不重寫邏輯，也就不會漂移。介面按卡片分組，跟隨系統的深淺色主題；亮度用逐檔的檔位條而不是滑桿，一格一檔。",
    "use.guiWin": "Windows 上只做燈光：介面、檔位、跟隨 NumLock、寫入日誌。小鍵盤輸入交給官方驅動。",
    "use.shot1": "擴充頁：自訂佈局、重寫點亮報文、壓感切換、全域熱鍵。",
    "use.shot2": "校準頁：分區、手動校準、樣本回放，以及對應的設定片段。",
    "use.shot3": "驅動設定頁：每個欄位都在旁邊標出官方 INF 會取的值。",
    "use.a.t": "執行與自我檢查",
    "use.b.t": "背光",
    "use.c.t": "佈局與語言",
    "use.d.t": "機型與校準",
    "use.fine": "設定檔位於 <code>~/.config/numdeck/numdeck.json</code>，以 root 執行時讀 <code>/etc/numdeck/</code>。未寫的鍵沿用廠商預設值：<code>numlock</code>、<code>mixed_mode</code>、<code>grab</code>、<code>follow_keyboard_brightness</code>、<code>allow_unverified_layout</code> 以及兩張亮度表。",

    "faq.title": "常見問題",
    "faq.1.q": "會取代 asusctl 或核心的 hid-asus 嗎？",
    "faq.1.d": "不會。兩者並行執行：抓取觸控板 evdev 節點，另建一個虛擬鍵盤。asusctl 對鍵盤背光的控制照常可用，移除本套件只移除數字鍵盤功能。",
    "faq.2.q": "觸控板的指標功能會失效嗎？",
    "faq.2.d": "僅在數字模式開啟時抓取。關閉 <code>grab</code> 後，點擊事件仍送至桌面；開啟 <code>mixed_mode</code> 且 <code>/dev/uinput</code> 可用時，數字鍵與指標可以同時使用。",
    "faq.3.q": "我的機型不在表裡，或者沒有 NumberPad，能用嗎？",
    "faq.3.d": "機型代號無相符項時拒絕啟動：以結束碼 3 結束並列出支援機型，不沿用其他機型的參數。<code>--sku</code> 或 <code>numdeck sku &lt;id&gt;</code> 可跳過機型校驗，<code>numdeck check</code> 會列出需要回報的資訊。機型在表中但無此硬體的，同樣以結束碼 3 拒絕啟動：驅動要求 HID 的 FEATURE 報告 <code>0x0d</code>，4 區 RGB 配置不具備該報告。以上規則不適用於 ARM64 建置：它沒有機型表可缺，也不做 <code>0x0d</code> 探測，表外機型不會被拒絕。",
    "faq.4.q": "簽章公鑰在哪裡，怎麼核對？",
    "faq.4.d": "apt、rpm、pacman 的索引共用一個 GPG 公鑰，Alpine 的套件另用一個 RSA 公鑰；兩個公鑰均隨索引放在倉庫的 <code>keys/</code> 下，由 <code>install.sh</code> 匯入，無需另行下載。指紋：<code>79F2 8215 7F2D 3FF8 E3C6 8063 14C2 BF61 1986 E168</code>，細節見 <code>docs/signing.md</code>。",
    "faq.5.q": "雙螢幕機型（Zephyrus Duo、Zenbook Duo）呢？",
    "faq.5.d": "支援，但有差異：這類機器的觸控板是鍵盤右側的豎條，數字模式由鍵盤專用鍵切換，而不是右上角的 62 幀長按。可用 <code>numlock</code> 固定模式。",
    "faq.6.q": "需要 root 或重新開機嗎？",
    "faq.6.d": "均不需要。守護行程以 <code>systemd --user</code> 單元執行，udev 把觸控板與背光節點授權給工作階段。首次安裝後登出再登入一次，讓單元與 udev 規則生效。",
    "faq.7.q": "Wayland 還是 X11，有差別嗎？",
    "faq.7.d": "沒有。守護行程只與 evdev、uinput 互動，在 Wayland、X11 與純 TTY 下行為一致。實際限制來自工作階段權限：無法存取 <code>/dev/uinput</code> 時，按鍵事件沒有接收端。",

    "foot.slogan": "觸控板數字小鍵盤，Linux 原生可用。",
    "foot.meta1": "不裝核心模組",
    "foot.meta2": "六種套件格式",
    "foot.note": "ASUS 觸控板數字小鍵盤（NumberPad）的 Linux 使用者空間驅動程式。GPL-3.0-or-later。",
    "foot.project": "專案",
    "foot.machines": "機型說明",
    "foot.docs": "文件",
    "foot.install": "安裝指南",
    "foot.verify": "發佈記錄",
    "foot.src": "純靜態頁面 · 無追蹤",
    "foot.right": "© 2026 numdeck · GPL-3.0-or-later"
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
  I18N.en["a11y.quick"] = "Quick install";

  var LANGS = ["en", "zh-CN", "zh-TW"];

  var langSelect = document.getElementById("lang-select");
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
    if (langSelect) {
      langSelect.value = next;
      // 窄屏放不下全称时退成短标签，以 title 补全语义
      var full = langSelect.options[langSelect.selectedIndex]
        ? langSelect.options[langSelect.selectedIndex].text
        : next;
      langSelect.title = full;
    }
    try { localStorage.setItem("an-lang", next); } catch (e) {}
  }
  if (langSelect) {
    langSelect.addEventListener("change", function () {
      applyLang(langSelect.value);
    });
    /* 原生 select 在收起态下方向键不一定改值（各浏览器不一致），
       此处做兼容处理：未展开时上下键直接切换语言，焦点始终留在控件上。 */
    langSelect.addEventListener("keydown", function (ev) {
      if (ev.altKey || ev.ctrlKey || ev.metaKey || ev.shiftKey) return;
      var k = ev.key;
      if (k !== "ArrowDown" && k !== "ArrowUp" && k !== "Home" && k !== "End") return;
      var open = false;
      try { open = langSelect.matches(":open"); } catch (e) {}
      if (open) return;                       // 弹层已展开：交给浏览器原生处理
      var i = LANGS.indexOf(langSelect.value);
      if (i < 0) i = 0;
      if (k === "ArrowDown") i = (i + 1) % LANGS.length;
      else if (k === "ArrowUp") i = (i - 1 + LANGS.length) % LANGS.length;
      else if (k === "Home") i = 0;
      else i = LANGS.length - 1;
      ev.preventDefault();
      applyLang(LANGS[i]);
    });
  }
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
    // 移动端地址栏着色跟随实际主题：手动切换后 media 查询不再准确，
    // 所以有 JS 时收敛成一条 meta，只留第一条并改写它的值。
    var metas = document.querySelectorAll('meta[name="theme-color"]');
    if (metas.length) {
      for (var i = metas.length - 1; i > 0; i--) metas[i].remove();
      metas[0].removeAttribute("media");
      metas[0].setAttribute("content", next === "dark" ? "#0a0b0f" : "#f7f8fa");
    }
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
