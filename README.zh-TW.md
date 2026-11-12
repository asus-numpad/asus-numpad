# numdeck

[English](README.md) | [简体中文](README.zh-CN.md) | 繁體中文 · [官網](https://numdeck-dev.github.io/)

ASUS NumberPad 的 Linux 使用者空間驅動程式。透過 evdev 獨佔觸控板，把觸點換算成小鍵盤格位，
再經 uinput 注入按鍵。不替換核心模組，不修改 ACPI 表，解除安裝即回復原狀。

時序與幾何常數取自廠商檔案 `AsusNUMPADFilter.inf` 中的 `AlgoParameters`。

## 安裝

```bash
# Debian / Ubuntu
sudo install -d -m 0755 /usr/share/keyrings
curl -fsSL https://raw.githubusercontent.com/numdeck-dev/numdeck/main/repo/keys/numdeck-repo.gpg \
  | sudo tee /usr/share/keyrings/numdeck.gpg >/dev/null
echo "deb [signed-by=/usr/share/keyrings/numdeck.gpg] https://raw.githubusercontent.com/numdeck-dev/numdeck/main/repo/apt stable main" \
  | sudo tee /etc/apt/sources.list.d/numdeck.list
sudo apt update && sudo apt install numdeck

# Fedora / RHEL / Rocky / Alma
sudo rpm --import https://raw.githubusercontent.com/numdeck-dev/numdeck/main/repo/keys/numdeck-repo.asc
sudo tee /etc/yum.repos.d/numdeck.repo >/dev/null <<'EOF'
[numdeck]
name=numdeck
baseurl=https://raw.githubusercontent.com/numdeck-dev/numdeck/main/repo/rpm/$basearch
enabled=1
gpgcheck=1
repo_gpgcheck=1
gpgkey=https://raw.githubusercontent.com/numdeck-dev/numdeck/main/repo/keys/numdeck-repo.asc
EOF
sudo dnf install numdeck

# openSUSE Leap / Tumbleweed，同一個 rpm-md 來源，zypper 直接讀
sudo rpm --import https://raw.githubusercontent.com/numdeck-dev/numdeck/main/repo/keys/numdeck-repo.asc
sudo zypper addrepo --refresh \
  "https://raw.githubusercontent.com/numdeck-dev/numdeck/main/repo/rpm/$(uname -m)" numdeck
sudo zypper install numdeck

# Arch / Manjaro
curl -fsSL https://raw.githubusercontent.com/numdeck-dev/numdeck/main/repo/keys/numdeck-repo.asc -o /tmp/numdeck.asc
sudo pacman-key --add /tmp/numdeck.asc
sudo pacman-key --lsign-key 79F282157F2D3FF8E3C6806314C2BF611986E168
sudo tee -a /etc/pacman.conf >/dev/null <<'EOF'

[numdeck]
SigLevel = Required DatabaseRequired
Server = https://raw.githubusercontent.com/numdeck-dev/numdeck/main/repo/arch/$arch
EOF
sudo pacman -Syu numdeck

# Alpine
curl -fsSL https://raw.githubusercontent.com/numdeck-dev/numdeck/main/repo/keys/numdeck-apk.rsa.pub \
  | sudo tee /etc/apk/keys/numdeck-apk.rsa.pub >/dev/null
echo "https://raw.githubusercontent.com/numdeck-dev/numdeck/main/repo/alpine/$(apk --print-arch)" | sudo tee -a /etc/apk/repositories
sudo apk update && sudo apk add numdeck
```

每個生態各一個套件。deb 在 Ubuntu 22.04 上建置，涵蓋 22.04 至 24.04 與 Debian 12、13。rpm 在
AlmaLinux 9 上建置，涵蓋 RHEL、AlmaLinux、Rocky 9 至 10、Fedora、openSUSE Leap 16 與
Tumbleweed：spec 只用標準巨集，執行期僅相依 glibc 與 systemd，倉庫格式為 rpm-md，zypper 直接
讀取，不按發行版各編一份。原生套件也在
[Releases](https://github.com/numdeck-dev/numdeck/releases)：deb、rpm、pkg.tar.zst、附
.SRCINFO 的 PKGBUILD、apk、靜態 tarball。手動建置 PKGBUILD：把 `PKGBUILD` 與
`numdeck.install` 放進同一目錄，執行 `makepkg -si`。Alpine、Gentoo、NixOS 與手動 tarball
步驟見 [docs/install.md](docs/install.md)。倉庫與 release 附件同時提供 `x86_64`/`amd64` 與 `ARM64`/`aarch64` 兩種架構，其餘架構需自行建置。

`ci.yml` 每次推送都為每個生態建置套件並檢查套件中介資料；`release.yml` 另在每個目標發行版的
乾淨容器中安裝每個套件並執行 `numdeck --version`，裝不起來的套件會擋下發佈。

套件只有一個，機型在執行期辨識：同一份二進位檔讀取 DMI 型號與觸控板模組，據此選擇參數與鍵表。
表中對不上的機器不啟動：以結束碼 3 結束並列出支援機型，不使用其他機型的參數。硬體資訊讀不準（虛擬機、白牌機殼、BIOS 佔位字串）時可
手動指定，見[機型辨識](#機型辨識)。執行期相依：glibc，Alpine 上為 musl。

ARM64 套件是例外：華碩原生 NumberPad 機型全為 x86_64，ARM 上沒有可辨識的機型，因此 ARM64
建置裡直接**不含機型表**——不讀 DMI、不辨識觸控板模組、也不依廠商過濾觸控板，啟動時不擋機型，
只走自訂佈局這條路（需自行在 `custom_layouts` 裡綁觸控板並錄入鍵位，見[自訂佈局](#自訂佈局)）。
反過來，`x86_64` 套件裡沒有為 ARM 預留任何備援。

## 使用

```bash
numdeck check          # 唯讀自我檢查：裝置、佈局換算、背光候選
numdeck dump           # 原始事件流
numdeck run            # 前景執行，由 systemd 單元啟動
numdeck --dump-config  # 目前生效設定
```

```bash
numdeck bright 0..N         # 檔位，0 為關（關閉跟隨）；N 為檔數，預設 4
numdeck bright up|down|cycle
numdeck bright follow on|off
numdeck bright status
```

```bash
numdeck levels              # 檔位表：每一檔寫入的位元組
numdeck levels 2 0x46       # 第 2 檔寫 0x46（取值 0x00 或 0x41..0x48）
numdeck levels reset        # 回復預設四檔
numdeck led on|off|0x41     # 繞過狀態機，直接送出一個背光位元組
numdeck layout              # 目前鍵表、全部已收錄鍵表與支援等級
numdeck layout --ids        # 模組名對應鍵表的清單
numdeck layout auto         # 回復自動選擇
```

```bash
numdeck lang [zh-CN|zh-TW|en|auto]
numdeck --lang en check     # 僅本次生效
```

語言優先順序：`NUMDECK_LANG`、`--lang`、設定檔中的 `language`、系統 locale。

## 設定

`~/.config/numdeck/numdeck.json`，以 root 執行時為 `/etc/numdeck/`。未寫入的鍵取廠商
預設值。

| 鍵 | 取值 | 含義 |
| --- | --- | --- |
| `numlock` | `sync` / `toggle` / `off` | 跟隨系統 NumLock 狀態 |
| `calc_gesture` | `true` / `false` | 手勢喚起計算機 |
| `calc_gesture_mode` | `long` / `off` | 觸發方式 |
| `mixed_mode` | `auto` / `on` / `off` | `auto`：點亮期間只有雙指以上交給桌面，單指留在數字區；`on`：鍵區外一律交給桌面；`off`：純數字鍵盤。`auto` / `on` 需要 `/dev/uinput` |
| `grab` | `true` / `false` | 獨佔觸控板；關閉後點擊交給桌面 |
| `led_kind` | `auto` / `hidraw` / `i2c` / `none` | 背光通道 |
| `follow_keyboard_brightness` | `true` / `false` | 跟隨鍵盤背光，預設 `true` |
| `manual_brightness_step` | `0`..`3` | 目前手動檔（0 為關閉），跟隨關閉時生效；上界為檔數 |
| `manual_brightness_levels` | 陣列 | 檔位對應取值的表，預設 `[0, 0x43, 0x45, 0x48]`；元素為 `0` 或 `0x41`..`0x48`；長度 1 到 8 即檔數；允許相鄰相等；遞減的表會被丟棄 |
| `brightness_levels` | 陣列 | 硬體檔位，預設 `0x41`..`0x48` |
| `brightness_poll_ms` | 毫秒 | 輪詢間隔，預設 `500` |
| `allow_unverified_layout` | `true` / `false` | 接受已知家族中未收錄的佈局 |
| `layout_name` | 鍵表名 / `auto` | 手動指定鍵表，`auto` 依模組選擇 |
| `sku` | 機型 id / `null` | 手動指定機型，跳過自動辨識；`null` 為自動 |
| `custom_layouts` | 陣列 | 自訂佈局，見[擴充](#擴充給表外的機型補一套小鍵盤)：`name`、`base`、`hid_ids`、`keys`，以及可選的佈局參數覆蓋 |
| `led_proto` | 物件 | 自訂點亮報文：`template` 為位元組陣列，其中一個位元組寫 `255` 當檔位佔位符 |
| `pressure_switch` | 物件 | 壓感觸控板「用力按＝切換」：`enabled`、`percent`、`frames`、`cooldown_ms` |
| `hotkeys` | 陣列 | 實體鍵盤全域組合鍵：`gesture`（如 `ctrl+alt+n`）配 `action`（`toggle` / `on` / `off`） |

## 支援機型

| 機型 id | 機型與年份 | DMI 代號前綴 | 觸控板模組 | 鍵表 | 有配置不帶該硬體 |
| --- | --- | --- | --- | --- | --- |
| `g533` | Strix SCAR 15 / Strix G15，15 吋，2022 | 前綴 `G533*`、`G513*`——2021 代的 `G533Q*` / `G513Q*` / `G513I*` 已排除 | `ASUE1403`、`ELAN1403` | `GH2XLNDZ` | Strix G15 2022 五個配置裡三個 |
| `g614` | Strix SCAR 16 / Strix G16，16 吋，2023–2024 | `G614`、`G634` | `ASUE1416`、`ASUF1416` | `GH3XLNDZ` | Strix G16 2023 十個配置裡兩個 |
| `g615` | Strix SCAR 16 / Strix G16，16 吋，2025–2026 | `G615`、`G635` | `ASCP1400`、`ASCF1400` | `GH4XLNDZ` | Strix G16 2025 二十五個配置裡一個 |
| `gx550` | Zephyrus Duo 15 / Duo 15 SE / Duo 16（西風之神），側置觸控板，15–16 吋，2020–2023 | `GX550`、`GX551`、`GX650` | `ASUE1408`、`ASUE1404` | `GVM_A`、`GVM` | 官方列表中沒有 |
| `ux582` | Zenbook Duo 14 / Zenbook Pro Duo 15 OLED，側置觸控板，14–15 吋，2020–2021 | `UX582`、`UX482` | `ASUE1406`、`ELAN1406` | `NVM` | 未逐配置核驗 |

`GVM_A` / `GVM` / `NVM` 的參數取自官方 INF；官方只列到 `GVM_06` / `NVM_12`，不足一張完整表的 15 個
格位，空缺寫 `0`——與 Windows 驅動讀不到該項時的預設一致，不做數值推測；`GVM_A` 與 `GVM` 共用同一份
參數（`NUMPADLAYOUT_GVM_05` / `_06` 的註解寫明是「for GVMA」）。這兩列狀態是 `LayoutSupport::Family`：
5 欄、網格與 GH3XLNDZ 同族，但未在實機上逐鍵核對過，啟動需加 `--allow-unverified` 允許出鍵。

同一塊觸控板跨年度沿用同一代號，代號後的字母只跟 CPU 換代，觸控板本身不變：`G614` / `G634`
涵蓋 2023 與 2024 兩個年度（SCAR 16 → SCAR 16 2024），`G615` / `G635` 涵蓋 2025 與 2026
（Strix G16 2025 → 2026、SCAR 16 2025 → 2026，其中 `G615` 為標準版、`G635` 為 SCAR 版）。
2026 年沒有更換代號，機型表因此不必新增一列。

這張表就是支援範圍。機型 id 用來手動切換參數（`numdeck sku <id>`），並讓自動辨識的結果
看得見；表外的機器拒絕啟動。每一列的歸屬依觸控板模組資料與鍵表對應判定，不按整機型號逐台驗證；
各機型目前的相容情況詳見[官網](https://numdeck-dev.github.io/)。

機型 id 命中不等於這台有硬體。同一機型的部分零售配置搭的是 4 區 RGB 觸控板、沒有
NumberPad，且 DMI 前綴與帶 NumberPad 的配置完全相同（`G614JV…-0EAGX…` 有，
`G614JV…-0EACX…` 沒有；各代數量見上表）。因此鍵表一律依觸控板模組名選擇，絕不從機型代號推；
逐配置核驗結果見 [docs/machines.md](docs/machines.md#同一型号不是都有-numpad按配置核验)。

幻16 系列（中國區名稱，對應 Zephyrus M16 / G16 / Flow X16，`GU603`–`GV601`）屬於表外而非漏支援：
官方規格頁每一代的「鍵盤與觸控板」欄都只有 `Touchpad`，沒有圖示區可驅動。帶這塊硬體的是雙螢幕
`GX650`（冰刃 6 / 7 雙屏），已由 `gx550` 列提供參數。逐代核驗見
[docs/machines.md](docs/machines.md#幻16-系列无-numberpad已核验)。

一列機型條目由該世代的 DMI 型號前綴與 `excludes` 組成：`excludes` 收的是沿用同一代號前綴、
但沒有這塊觸控板的舊世代。被排除的前綴會落到模組判據，兩者都對不上時同樣拒絕啟動；手動指定的
`sku` 優先於兩者。代號在不同區域產品線之間同樣會重用：`G614P*` 在中國市場以 2025 年款魔霸 10 /
魔霸 9 銷售，上表的逐配置統計沒有涵蓋這些產品線，鍵表仍由觸控板模組決定。

以 DMI 中的機型代碼為準，市場名稱僅供對照。舊版按機型拆分的套件名稱（`numdeck-g614`
等）留在 `Obsoletes` 中，升級時由 `numdeck` 接手。

| | |
| --- | --- |
| ![Strix SCAR 15 2022](site/assets/models/g533.webp) | ![Strix SCAR 16 2023](site/assets/models/g614.webp) |
| Strix SCAR 15 / Strix G15（2022） | Strix SCAR 16 / Strix G16（2023） |
| ![Strix SCAR 16 2025](site/assets/models/g615.webp) | ![Zephyrus Duo 16，GX650（2022）](site/assets/models/gx550.webp) |
| Strix SCAR 16 / Strix G16（2025） | Zephyrus Duo 16（側置觸控板，GX650，2022） |

產品圖取自華碩產品頁，僅用於辨識機型。

### 雙螢幕機型：側置觸控板

現狀：`GVM` / `GVM_A` / `NVM` 用的是官方原值（官方沒列的項按預設 `0`），兩列狀態為 `family`：
網格同為 5 欄，但鍵位排布還沒在實機上核對過，啟動需加 `--allow-unverified`。以下是驅動按這套
鍵表實作的取向。

Zephyrus Duo 與 Zenbook Duo 的 C 面被副螢幕佔去一半，觸控板豎排在鍵盤右側，不在掌心正下方。
這類機器的數字模式由**鍵盤上的專用切換鍵**控制（`GX550` 鍵盤上那顆 ScreenPad 鍵即屬此類），
不是長按觸控板右上角的圖示區。因此「長按 62 幀切換數字模式」這條路徑對側置觸控板不成立：

- 觸點對應與背光照舊：格心座標依該模組的鍵表平移，背光仍走 FEATURE `0x0d`；
- 模式切換交給鍵盤專用鍵——鍵盤走自己的 HID 節點，驅動只抓觸控板，不搶這個鍵；
- 需要固定數字模式時使用設定項 `numlock`（`sync` / `toggle` / `off`），不靠長按手勢。

### 機型辨識

辨識取 DMI 的 `product_name`、`board_name`、`product_family` 三者的聯集。BIOS 佔位字串視為
缺失，不阻擋啟動；明確不是 ASUS 的廠商欄位會阻擋啟動。

觸控板模組名（如 `ASUE1416`，來自 i2c-hid 與 HID）是第二判據，在 DMI 說不出機型時啟用。兩個
判據都沒對上具體機型，即判定為表外機器：以結束碼 3 拒絕啟動，`--sku` 是唯一的跳過校驗方式。
各機型的模組與鍵表見 [docs/machines.md](docs/machines.md)。

讀不到正確硬體資訊時手動指定：

```bash
numdeck sku            # 看辨識到哪一套設定、憑什麼
numdeck sku list       # 列出可選機型 id
numdeck sku g614       # 手動指定，寫進設定長期生效
numdeck sku auto       # 回復自動辨識
```

設定中的 `sku` 鍵優先於自動辨識，`--sku g614` 只對本次執行生效。

### 佈局校驗

鍵表依觸控板模組名選擇：`src/layout.rs` 的 `HID_LAYOUTS` 把 44 個模組名對應到廠商 INF 中的
鍵表名，每張鍵表帶一個支援等級。

| 等級 | 行為 |
| --- | --- |
| `verified` | 直接啟動 |
| `family` | 5x4 網格相同、官方參數已全部照錄（官方沒列的項按預設 `0`），但未在實機逐鍵驗證：預設拒絕，需明確允許 |
| `unsupported` | 參數已收錄，但無鍵位排布依據且未經實機核對：預設拒絕，並提示附上 `check` 輸出回報 |

表中沒有的模組落到 `UNKNOWN`，以同樣方式拒絕。未知面板不會被對應到會誤按鍵的鍵表上。校驗
只涵蓋鍵表，與機型 id 無關。

```bash
numdeck --allow-unverified run    # 或在設定中設 allow_unverified_layout: true
```

## 擴充：給表外的機型補一套小鍵盤

機型表之外的機器，只要觸控板報得出按鍵座標，就能自己補一套，不必等驅動加機型。依手上這台機器的
條件選：

| 手上是哪一種 | 配什麼 | 有沒有背光 |
| --- | --- | --- |
| 觸控板上有雷雕數字，或貼了一張小鍵盤貼紙 | `custom_layouts` 寫一套鍵表 | 沒有，頂多被鍵盤背光順帶照亮 |
| 一般玻璃面觸控板，只想照著貼紙出數 | 同上，`keys` 依貼紙的排布寫 | 同上 |
| 觸控板確實會亮，只是報文不是 `0d 14 03 .. ad` | `led_proto` 換報文 | 有 |
| 觸控板會報壓力軸 | `pressure_switch` 用力按切換 | 不影響 |
| 想直接用實體鍵盤開關小鍵盤 | `hotkeys` 綁組合鍵 | 不影響 |

四樣相互獨立，缺哪樣配哪樣；都配在同一個 `numdeck.json` 裡，介面「擴充」頁也在改同一份檔案。

### 自訂佈局

```json
{
  "custom_layouts": [
    {
      "name": "GH3X-貼紙",
      "base": "GH3XLNDZ",
      "hid_ids": ["ASUE1416", "ASUE1419"],
      "keys": [
        "7 8 9 / bs",
        "4 5 6 * minus",
        "1 2 3 plus enter",
        "0 0 period . enter"
      ]
    }
  ]
}
```

- `name` 就是鍵表名：`layout_name` 寫它能選中；留 `auto` 時依 `hid_ids` 命中的模組自動選。
- `base` 是參數基準，可以是內建鍵表名，也可以是同一份設定裡排在它前面的另一條自訂佈局（用來
  「照抄某套幾何，只改兩格」）。幾何參數（`columns`、`left`、`right`、`top`、`bottom`、`gap` 等）
  不寫就照抄基準；`columns` 由 `keys` 的列數自動改寫，不用手填。
- `keys` 是 4 行鍵表，列數 1..=8、行數固定 4。四行列數必須一致，不一致這張佈局直接判為不可用，
  介面上會寫明原因。留空表示沿用基準的鍵表（只改幾何、不改鍵位時這樣寫）。
- `hid_ids` 綁的是**觸控板模組的 HID 硬體 ID**（`ASUE1416` 這一層），不是整機型號：綁上之後這台
  機器不用再改 `layout_name` 也能命中。綁到內建表裡已有的模組時會提示「已覆蓋官方對映」。
- 自訂佈局按 `verified` 處理——它是你自己寫的，驅動不再用 `family` / `unsupported` 那套攔你。

鍵位怎麼寫（空白分隔，大小寫不拘）：

| 想按出來的鍵 | 寫成 |
| --- | --- |
| 數字 | `7` |
| 鍵盤上印的符號 | `+`、`*`、`/`（也認 `plus`、`asterisk`、`slash`） |
| 小數點 | `period`（`.` 是空位，不能當小數點用） |
| 減號 | `minus`（`-` 是空位） |
| 換行、退格 | `enter`、`bs` |
| 空位（這一格不出鍵） | `none`，也可寫 `.` 或 `-` |
| 其他鍵 | `kp7`、`F13`、`backspace`、`volup` 這類名字，或直接寫鍵碼 `0x64` / `100` |

介面「擴充」頁的鍵表就是這幾行文字，改完當場顯示「幾列 × 4 行，可用」或具體哪一行對不上。

### 自訂點亮報文

```json
{ "led_proto": { "template": [13, 20, 3, 255, 173] } }
```

- `template` 是整條 HID FEATURE 報文，第一個位元組是 report ID。其中一個位元組寫 `255`（`0xff`）
  當**檔位佔位符**，點亮時被替換成目前檔位值（`0x41`..`0x48`，熄滅是 `0`）。
- 必須恰好有一個 `255`，長度 2..=64 位元組；否則整條報文作廢、回落到內建報文，介面與 `--verbose`
  都會提示。
- 介面裡用十六進位編輯（`0d 14 03 ff ad`），位元組之間用空白分開。

### 壓感切換

```json
{ "pressure_switch": { "enabled": true, "percent": 85, "frames": 6, "cooldown_ms": 600 } }
```

- 觸控板報壓力軸時，壓力連續 `frames` 幀超過滿量程的 `percent`%，算一次「用力按」，翻轉小鍵盤
  開關。一幀等於 `frame_ms`（預設 8 ms），所以預設 6 幀約 50 ms。
- `cooldown_ms` 是兩次翻轉之間的最小間隔，防止按住不放連續翻轉。
- 與 `mixed_mode` 互不影響：這裡只翻開關狀態，不合成按鍵，也不把壓力事件轉發給桌面。
- 觸控板不報壓力軸時這項不起作用，設定留著無害。

### 全域熱鍵

```json
{ "hotkeys": [ { "gesture": "ctrl+alt+n", "action": "toggle" } ] }
```

- `gesture` 用 `+` 連接鍵名：`ctrl`、`alt`、`shift`、`super` 加一個普通鍵。
- `action` 三個值：`toggle` 翻轉、`on` 只開、`off` 只關。
- 只旁聽實體鍵盤事件、不 grab 鍵盤，正常打字不受影響；寫錯的組合鍵會在介面上提示並跳過。

### 自訂分區與手動校準

機型表裡的 15 項幾何參數都建立在 5x4 等分網格上。兩種機型不夠用：絲印排法不同（欄寬不等、六七欄），
或者核心上報的座標根本鋪不滿面板。校準頁管的就是這兩種。

```json
{
  "partition": {
    "layout": "auto",
    "rect": [405, 520, 3630, 2280],
    "cols": 5,
    "rows": 4,
    "col_ratios": [1, 1, 1, 1, 1],
    "row_ratios": [1, 1, 1, 1],
    "snap_outside": false
  },
  "calibration": {
    "x": { "min": 0, "max": 3600, "units_per_mm": 40 },
    "y": { "min": 0, "max": 2200 }
  }
}
```

- `rect` 是 `[x0, y0, x1, y1]`，用觸控板原始座標，與 `evtest` 裡看到的一致。為空或上下界寫反
  會被拒絕並點名，改回內建網格，不會帶著壞幾何跑。
- `cols` 取 1..=8，`rows` 取 1..=4；留空就跟隨鍵表。
- `col_ratios` / `row_ratios` 讓網格不等分（例如最右一欄寬些），填的是權重，按矩形自動歸一，
  長度須與欄數 / 列數對上。這一項只在設定裡，介面沒擺。
- `snap_outside` 預設關。打開後網格外的落點吸附到最近一格，而不是算進游標區。
- `calibration` 覆寫核心對兩軸的上報：`min` / `max` / `units_per_mm`（每公釐 30 個單位，手勢
  按公釐算出來的像素值才對）。只做線性對映，不交換軸也不翻轉；某一軸不覆寫就沿用核心值。
- 分區只對 `partition.layout` 點名的佈局生效（`auto` 跟目前生效佈局），所以一份檔案裡可以給
  多個佈局各留一套分區。
- 另有兩個鍵沒有對應控制項：`rect_mm`（用面板左上角起的公釐寫同一個矩形，原始單位與實測對不上時
  用它）與 `numlock_cell`（NumLock 圖示所在格 `[列, 欄]`，預設第 0 列最後一欄）。

量比猜準：`numdeck calibrate --samples samples.json` 收四角按壓加兩個對角格中心，直接解出
矩形與格尺寸；加 `--write` 落檔，並把上一份存成 `.bak`。校準頁的「樣本回放」把同一份樣本餵給
同一個解算函式，真機上採一次，介面裡就能反覆解。底部「設定片段」把 `partition` + `calibration`
打包，方便搬給同機型的人；片段不含鍵表，鍵表在 `custom_layouts` 裡。

![校準頁：分區、手動校準、樣本回放、設定片段](site/assets/screenshots/gui-calib.webp)

### 把設定搬給同機型的人

擴充頁最底下是「設定片段」：按「匯出」把四項擴充打包成一段 JSON，對方貼進同一個框、按「套用」
就裝上了。片段只含 `custom_layouts` / `led_proto` / `pressure_switch` / `hotkeys`，不含幀節拍、
幾何兜底、介面語言這些跟著機器走的項目——換機型時它們本來就該重設。匯入是全有或全無：四項裡任何
一項解析失敗都整段不落地，不會留下改了一半的設定；範本不合法的報文也會被點名拒絕。除了匯出到框裡，
還能「複製」進剪貼簿直接送，「存成檔案」寫一份 `.json` 到下載目錄（重複匯出不覆蓋上一份）。

### 配完先自檢

```sh
tools/verify-extend.sh
```

22 項唯讀檢查：二進位檔、擴充相關單元測試逐一具名執行、`custom_layouts` /
`led_proto` / `pressure_switch` / `hotkeys` 是否真的進了 `layout` 與 `check`、
`--lang` 切換語言、壞值是否被拒並留下提示。全程用 `mktemp` 暫存設定，不碰
`~/.config`，也不寫任何 hidraw 裝置。需要手指按觸控板的三項（壓感切換、熱鍵、
自訂報文）只印出指令，不代你執行。

![擴充頁：自訂佈局、點亮報文、壓感切換、全域熱鍵](site/assets/screenshots/gui-extend.webp)

每一行參數、每個區塊標題和按鈕都掛了懸浮說明，不用把 README 攤在旁邊：

![擴充頁的懸浮說明（淺色）](site/assets/screenshots/gui-extend-hover-light.webp)
![擴充頁的懸浮說明（深色）](site/assets/screenshots/gui-extend-hover-dark.webp)

### 與機型表、佈局校驗的關係

兩層閘門是分開的：機型（DMI / `sku`）決定「這台機器允不允許跑」，鍵表（`HID_LAYOUTS`）決定
「按哪裡出什麼鍵」。自訂佈局只解決第二層——模組在表外、或者在表裡但鍵位排布不對時用它；整機
認不出來仍然要 `--sku` 或 `sku` 設定過第一層。

## 常見問題

### 為什麼只支援華碩機型？

驅動不依賴華碩私有介面：輸入取自觸控板的 evdev 節點，按鍵經 uinput 輸出，不涉及廠商核心模組與
ACPI 物件，任意多點觸控板走的是同一條程式路徑。移植工作集中在替換機型條目（DMI 的
`product_name` 加觸控板 HID 名稱）與佈局常數（格心座標、各項閾值）。

無法移植的是硬體。NumberPad 面板提供兩項其他 OEM 觸控板通常沒有的能力：

- **可定址背光。** 落指格位與數字模式狀態由 LED 圖樣指示，經 HID FEATURE 報告 `0x0d`
  （`[0d 14 03 <v> ad]`，`0x41`..`0x48` 為亮度檔）驅動。沒有這層背光，按鍵仍能輸出，但無法
  指示手指所在格位，也無法指示數字模式是否開啟。
- **出廠校正表。** `AsusNUMPADFilter.inf` 中的 `AlgoParameters` 固定了幀長（8 ms）、幾何單位
  （0.1 mm）、輕點視窗（350 ms）、NumLock 長按（62 幀）、滑動閾值（4838 µm）與雙指間距
  （40 mm）。沒有這張表，這些值只能在從未按小鍵盤調校過的面板上擬合，輕點與滑動辨識無法與
  Windows 驅動對齊。

因此支援範圍限定在同時具備兩者的面板。`src/sku.rs` 的機型表即支援範圍，負責啟動校驗；鍵表與
支援等級在 `src/layout.rs` 的 `HID_LAYOUTS`，依觸控板模組另外選擇，表中沒有的模組停在
[佈局校驗](#佈局校驗)。具備這兩項能力的面板，接入工作是一列表項加佈局常數，歡迎提交修補。

### 為什麼做成使用者空間驅動而不是核心模組？

- 面板經 ELAN i2c-hid（`04F3:3275` / `ASUE1416`）接入，不在華碩 `0B05` 鍵盤控制器之下，現有
  核心模組涵蓋不到它，接入也不需要替換任何模組。
- 背光控制是觸控板自身 hidraw 節點上的 HID FEATURE 報告，使用者空間行程即可下發，不必進入核心。
- 時序、幾何與閾值逐項取自廠商 INF，狀態機行為與 Windows 驅動一致，機型差異集中在一張參數表裡。
- 開機路徑沒有改動。解除安裝即移除 service unit 與 udev 規則，回復原狀，核心升級不會帶來回歸。

### 會考慮 PR 進 asusctl 嗎？

暫無計畫。這塊觸控板只在少數機型上出現，驅動運行之後沒有需要持續調整的項，合併進去意味著
要跟著 asusctl 的架構與發佈節奏走，而它的其餘使用者用不到這個功能。向本專案提交修補，或在
asusctl 側實作同樣的功能，兩條路都歡迎。

## 工作原理

### 資料流

```
觸控板 --evdev(/dev/input/eventN)--> 幀緩衝(8 ms) --> 狀態機 --uinput(/dev/uinput)--> 按鍵事件
                                                          |
                                                          `--> hidraw FEATURE 報告 --> 小鍵盤背光
```

### 時序與幾何

| 參數 | 取值 | 含義 |
| --- | --- | --- |
| 幀長 | 8 ms | 狀態機時基，長按 62 幀（496 ms） |
| 長度單位 | µm / 0.1 mm | 手勢與滑動閾值以微米計，佈局偏移以 0.1 mm 計 |
| 點擊間隔 | 350 ms | `NUMPADGESTURE_03/04`：第二次點按算作獨立按鍵的最小間隔 |
| NumLock 長按 | 62 幀 | `NUMPAD_GH2XLNDZ_01` / `_GH3XLNDZ_01`，圖示區切換數字模式（置中式觸控板；側置觸控板見[雙螢幕機型](#雙螢幕機型側置觸控板)） |
| NumLock 漂移 | 2741 µm | `NUMPAD_09`：仍按長按處理的位移上限 |
| 滑動閾值 | 4838 µm | `NUMPADGESTURE_05`：超過即取消本次按鍵、轉為手勢 |
| 雙指間距 | 40 mm | `NUMPADGESTURE_08`：捲動允許的最大指距，超過則按張開雙指交給桌面 |
| 應用程式手勢 | 7000 µm | `NUMPAD_07`：多指手勢的觸發距離 |

### 鍵表

鍵表依觸控板模組與 HID 名稱選擇，不依筆電型號。`HID_LAYOUTS` 收錄 44 條 HID 名稱到鍵表的
對應，涵蓋廠商 `SupportList` 中的每個模組；`ASUE1416`、`ASCF1400` 這類模組前綴可在
`/sys/class/input/*/name` 中看到。每條對應帶支援等級，見[佈局校驗](#佈局校驗)。

小鍵盤位於觸控板右上角，5 列 4 行（側置觸控板取的是觸控板自身的右上角，只是整塊觸控板排在鍵盤
右側）。格子中心即 `NUMPADLAYOUT_GH3XLNDZ_00..14` 的座標，單位 0.1 mm。座標到格子、格子到鍵碼
的換算不含硬體相依，有單元測試。

### 背光

```
報告: 0d 14 03 <檔位> ad   # 6 位元組，HID FEATURE 報告 ID 0x0d
檔位: 0x41..0x48           # 第 1..8 檔
      0x00                 # 關閉
      0x01                 # 開啟，沿用目前檔位
寫入: HIDIOCSFEATURE       # 無需 0x60 解鎖
```

`bright follow on` 讀取 `/sys/class/leds/asus::kbd_backlight/brightness` 的 `current` 與 `max`，
把比值對應到 1..8 檔，每 500 ms 輪詢，僅在檔位變化時寫入。手動檔位與跟隨模式共用一張檔位表。

節點選擇（`led_policy.rs`）：一律排除 OMNI 接收器 `0b05:1ace`；ELAN `04f3:*` 且帶 FEATURE
`0x0d` 優先，其次 ASUS `0b05:*`，最後 i2c（`0x38` / `0x15`）。

### 圖形介面

同一份 crate 出兩個視窗：Windows 上只管燈光，Linux 上是完整的控制中心。

![Windows 上的燈光控制視窗](site/assets/screenshots/gui-win.webp)

`gui/` 送的報文與 CLI 完全相同，兩端都出自共用的 `led_proto` 層，位元組不會漂移。

Windows 的視窗只解決燈光：介面清單、亮度檔位、跟隨 NumLock、寫入日誌。小鍵盤輸入交給官方驅動，
後端是 `setupapi` + `hid.dll`，不碰 hidraw。

Linux 上是全套，分七個頁籤：

| 頁籤 | 內容 |
| --- | --- |
| 燈光 | 介面清單、鍵盤四檔 → 觸控板亮度對映（逐檔可改、可試亮、可與驅動設定互相同步）、點亮 / 熄滅、跟隨 NumLock 與鍵盤背光、報文日誌 |
| 驅動設定 | 驅動設定（`~/.config/numdeck/numdeck.json`，root 下 `/etc/numdeck/numdeck.json`）的每個欄位都做成控制項：機型、佈局、鍵碼風格、NumLock 策略、手勢閾值、背光通道與跟燈輪詢、手動檔位表、時序常數、驅動語言 |
| 系統狀態 | 設定檔路徑、機型、佈局、hidraw 通道、單元狀態，可啟動 / 停止 / 重啟 |
| 擴充 | 給表外機型補自訂佈局、換點亮報文、壓感切換開關、綁全域熱鍵，以及可轉發的設定片段 |
| 校準 | 給表外機型自訂網格分區（矩形、欄數、比例）與軸校準，附樣本回放與設定片段 |
| 命令列 | 每個 `numdeck` 子命令一張表單，執行時跑同一個執行檔 |
| 設定 | 檔位數、跟隨 NumLock、輪詢間隔、開機自啟 |

![Linux 控制中心](site/assets/screenshots/gui-main.webp)

改動在按下儲存之前只停在記憶體裡；「儲存並重啟」會寫檔並重啟 `numdeck` 使用者單元，一步到位。
介面上不重寫一遍 CLI 的邏輯：命令頁執行的就是真二進位，輸出原樣貼回。

```bash
cargo run -p numdeck-gui      # 或 cargo build --release -p numdeck-gui
```

執行條件、設定檔位置與排錯見 `gui/README.md`。

### 權限

不需要 root。udev 給 hidraw 與 event 節點標上 `TAG+="uaccess"`，守護行程以 `systemd --user`
單元執行，`/dev/uinput` 由工作階段授權。

## 原始碼結構

| 路徑 | 用途 |
| --- | --- |
| `src/lib.rs` | 程式庫入口：執行檔與 GUI 共用的模組 |
| `src/main.rs` | CLI 解析與組裝 |
| `src/engine.rs` | 廠商演算法狀態機 |
| `src/layout.rs` | 佈局幾何、44 條 HID 表、座標換算 |
| `src/config.rs` | 預設值、讀寫、舊路徑遷移 |
| `src/brightness.rs` | 鍵盤背光到小鍵盤檔位的對應 |
| `src/led_policy.rs` | 背光節點評分 |
| `src/sku.rs` | 機型表、執行期辨識、手動指定、世代排除 |
| `src/i18n.rs` | 三種語言，可用外部 `.lang` 檔覆寫 |
| `src/keycodes.rs` | Linux input 事件碼，自帶一份讓 `layout` / `engine` 跨平台可編譯 |
| `src/led_proto.rs` | 背光 FEATURE `0x0d` 報文的編解碼，CLI 與 GUI 的單一真源 |
| `src/linux/` | evdev、uinput、hidraw、i2c 後端 |
| `gui/` | egui 視窗：Windows 只管燈光，Linux 是完整控制中心 |
| `lang/` | zh-CN、zh-TW、en 文案 |
| `site/` | 官網原始碼，發佈到 https://numdeck-dev.github.io/ |
| `tools/` | `push-site.sh`（官網同步）、`repo-gen.sh`（倉庫索引） |
| `packaging/` | deb、rpm、Arch、Alpine、Gentoo、Nix、tarball 腳本與倉庫工具 |

## 建置

### 交叉建置另外兩個平台

```bash
./tools/build-cross.sh            # 兩個都建
./tools/build-cross.sh windows    # x86_64-pc-windows-gnu + aarch64-pc-windows-gnullvm
./tools/build-cross.sh linux      # x86_64-unknown-linux-gnu + aarch64-unknown-linux-gnu
```

腳本會自己下載 zig（它內建 `windows-gnu` / `linux-gnu` 要用的 MinGW 與 glibc，不用裝
mingw-w64，也不用起容器），缺 `cargo-zigbuild` 就裝。Linux 目標固定 glibc 2.17 下限，
Ubuntu 18.04 之後都能直接跑。GUI 那個 crate 不做交叉建置——它要吃目標平台的合成器。

```bash
./packaging/build.sh                        # 目前發行版
./packaging/build.sh deb rpm tarball        # 指定目標（deb/rpm/arch/alpine/tarball）
./tools/repo-gen.sh                         # 由 dist/ 產生本機倉庫到 repo/（apt/rpm/pacman/apk）
```

```bash
RUSTFLAGS="-C linker=rust-lld -C link-self-contained=yes" \
  cargo build --target x86_64-unknown-linux-musl --release
```

打包細節見 [packaging/README.md](packaging/README.md)。推送 `v*` 標籤會觸發
`.github/workflows/release.yml`，在按發行版劃分的容器裡建置、產生四套倉庫索引、發佈 release
並把 `repo/` 提交回 `main`。

官網原始碼就在本倉的 `site/`：`./tools/push-site.sh` 將它同步到
`numdeck-dev/numdeck-dev.github.io` 倉庫，由該倉庫提供 https://numdeck-dev.github.io/ 。
`docs/` 只放專案文件，`docs/index.html` 是讓舊的專案站台網址繼續可用的轉向頁。

## 授權

GPL-3.0-or-later，見 [LICENSE](LICENSE)。

Copyright (C) 2026 the numdeck authors
