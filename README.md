# numdeck

English · [简体中文](README.zh-CN.md) · [繁體中文](README.zh-TW.md) · [Website](https://numdeck-dev.github.io/)

Linux userspace driver for the ASUS NumberPad. The touchpad is grabbed through evdev, contacts
are mapped to numpad cells, key events are injected through uinput. No kernel module is replaced,
no ACPI table is patched; uninstalling restores stock behaviour.

Timing and geometry constants come from `AlgoParameters` in the vendor file `AsusNUMPADFilter.inf`.

## Installation

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

# openSUSE Leap / Tumbleweed, same rpm-md repo, zypper reads it directly
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

One package per ecosystem. The deb is built on Ubuntu 22.04 and covers 22.04 to 24.04 plus Debian
12 and 13. The rpm is built on AlmaLinux 9 and covers RHEL, AlmaLinux, Rocky 9 to 10, Fedora,
openSUSE Leap 16 and Tumbleweed: the spec uses standard macros only, runtime dependencies are glibc
and systemd, and the repository is rpm-md, which zypper reads directly. Nothing is rebuilt per
distribution. Native packages are also on
[Releases](https://github.com/numdeck-dev/numdeck/releases): deb, rpm, pkg.tar.zst, PKGBUILD with
.SRCINFO, apk, static tarball. Building the PKGBUILD by hand: put `PKGBUILD` and
`numdeck.install` in one directory and run `makepkg -si`. Alpine, Gentoo, NixOS and manual
tarball steps are in [docs/install.md](docs/install.md). The repositories and the release assets
ship both `x86_64`/`amd64` and `ARM64`/`aarch64` builds. On anything else, build from source.

`ci.yml` builds a package per ecosystem on every push and checks the package metadata. `release.yml`
additionally installs each package in a clean container of every target distribution and runs
`numdeck --version`; a package that does not install blocks the release.

Runtime model detection means a package is not tied to a single model: the same binary reads the DMI
fields and the touchpad module, then picks the matching parameter set and keymap. A machine that
matches no row of the table is not started: the startup check exits with code 3 and prints the supported
models, instead of applying parameters intended for other hardware. If the hardware cannot be read
reliably (virtual machines, whitebox chassis, BIOS placeholder strings), pin the model by hand, see
[Model detection](#model-detection). Runtime dependency: glibc, or musl on Alpine.

The ARM64 package is the exception: every ASUS machine with a native NumberPad is x86_64, so an
ARM64 build carries **no model table** at all. It does not read the DMI strings, does not recognise
touchpad modules and does not filter touchpads by vendor; startup does not reject any machine and the
custom layout path is the only route (bind the touchpad and record its key positions in
`custom_layouts`, see [Custom layouts](#custom-layouts)). Conversely, the `x86_64` package reserves no fallback for ARM.

## Usage

```bash
numdeck check          # read-only self test: devices, layout conversion, backlight candidates
numdeck dump           # raw event stream
numdeck run            # foreground, started by the systemd unit
numdeck --dump-config  # effective configuration
```

```bash
numdeck bright 0..N         # step, 0 = off (follow disabled); N is the step count (4 by default)
numdeck bright up|down|cycle
numdeck bright follow on|off
numdeck bright status
```

```bash
numdeck levels              # step table: the byte each step writes
numdeck levels 2 0x46       # step 2 writes 0x46 (0x00 or 0x41..0x48)
numdeck levels reset        # back to the default four steps
numdeck led on|off|0x41     # bypass the state machine, send one backlight byte
numdeck layout              # keymap in use, every recorded keymap, support level
numdeck layout --ids        # module name to keymap table
numdeck layout auto         # back to automatic selection
```

```bash
numdeck lang [zh-CN|zh-TW|en|auto]
numdeck --lang en check     # this invocation only
```

Language precedence: `NUMDECK_LANG`, `--lang`, `language` in the config, system locale.

## Configuration

`~/.config/numdeck/numdeck.json`, or `/etc/numdeck/` when run by root. Unwritten keys
use vendor defaults.

| Key | Value | Meaning |
| --- | --- | --- |
| `numlock` | `sync` / `toggle` / `off` | Follow the system NumLock state |
| `calc_gesture` | `true` / `false` | Gesture launches the calculator |
| `calc_gesture_mode` | `long` / `off` | Trigger type |
| `mixed_mode` | `auto` / `on` / `off` | `auto` keeps the pointer only for two-finger gestures while the pad is lit; `on` always passes touches outside the key grid to the desktop; `off` = keys only. `auto` / `on` need `/dev/uinput` |
| `grab` | `true` / `false` | Grab the touchpad; when off, clicks reach the desktop |
| `led_kind` | `auto` / `hidraw` / `i2c` / `none` | Backlight channel |
| `follow_keyboard_brightness` | `true` / `false` | Mirror the keyboard backlight, default `true` |
| `manual_brightness_step` | `0`..`3` | Current manual step (0 = off), used when follow is off; the range ends at the step count |
| `manual_brightness_levels` | array | Step to value table, default `[0, 0x43, 0x45, 0x48]`; entries `0` or `0x41`..`0x48`; length 1 to 8 sets the step count; equal neighbours allowed; a decreasing table is discarded |
| `brightness_levels` | array | Hardware steps, default `0x41`..`0x48` |
| `brightness_poll_ms` | ms | Poll interval, default `500` |
| `allow_unverified_layout` | `true` / `false` | Accept an unlisted layout of a known family |
| `layout_name` | keymap name / `auto` | Pin a keymap by name, `auto` selects it from the module |
| `sku` | model id / `null` | Pin a model and skip detection; `null` means auto |
| `custom_layouts` | array | Custom layouts, see [Extending](#extending-adding-a-numpad-to-other-models): `name`, `base`, `hid_ids`, `keys`, plus optional geometry overrides |
| `led_proto` | object | Custom illumination report: `template` is a byte array with exactly one `255` marking the level slot |
| `pressure_switch` | object | Force-press toggling on pressure-sensitive touchpads: `enabled`, `percent`, `frames`, `cooldown_ms` |
| `hotkeys` | array | Global combos on the physical keyboard: `gesture` (e.g. `ctrl+alt+n`) plus `action` (`toggle` / `on` / `off`) |

## Supported models

| Model id | Models and model year | DMI prefixes | Touchpad module | Keymap | Configurations without the hardware |
| --- | --- | --- | --- | --- | --- |
| `g533` | Strix SCAR 15 / Strix G15, 15", 2022 | prefix `G533*`, `G513*` — 2021 `G533Q*` / `G513Q*` / `G513I*` excluded | `ASUE1403`, `ELAN1403` | `GH2XLNDZ` | 3 of 5 Strix G15 2022 |
| `g614` | Strix SCAR 16 / Strix G16, 16", 2023–2024 | `G614`, `G634` | `ASUE1416`, `ASUF1416` | `GH3XLNDZ` | 2 of 10 Strix G16 2023 |
| `g615` | Strix SCAR 16 / Strix G16, 16", 2025–2026 | `G615`, `G635` | `ASCP1400`, `ASCF1400` | `GH4XLNDZ` | 1 of 25 Strix G16 2025 |
| `gx550` | Zephyrus Duo 15 / Duo 15 SE / Duo 16, side-mounted, 15"–16", 2020–2023 | `GX550`, `GX551`, `GX650` | `ASUE1408`, `ASUE1404` | `GVM_A`, `GVM` | none in the official lists |
| `ux582` | Zenbook Duo 14 / Zenbook Pro Duo 15 OLED, side-mounted, 14"–15", 2020–2021 | `UX582`, `UX482` | `ASUE1406`, `ELAN1406` | `NVM` | not verified per configuration |

The `GVM_A` / `GVM` / `NVM` parameters are copied verbatim from the official filter, which lists fewer
entries than a full 15-cell table holds: `GVM` ends at `_06`, `NVM` at `_12`. The gaps are written as
`0`, the default the Windows driver reads when an entry is absent; no value is inferred. `GVM_A` shares
the `GVM` values — `NUMPADLAYOUT_GVM_05` / `_06` are annotated "for GVMA" in the filter. Both rows
carry `LayoutSupport::Family`: 5 columns, same grid, not checked key by key on real hardware, so they
are refused at startup unless `--allow-unverified` is given.

This table is the support list: a model id switches parameters by hand (`numdeck sku <id>`) and
makes the detection result visible, and a machine outside the table is refused at startup. `G614` and `G634` cover the
2023 and 2024 model years (SCAR 16 -> SCAR 16 2024), `G615` and `G635` cover 2025 and 2026 (Strix
G16 2025 -> 2026 and SCAR 16 2025 -> 2026; `G615` is the standard model, `G635` the SCAR variant):
the trailing characters track CPU refreshes, the touchpad does not change. 2026 keeps the same codes,
so the table needs no extra row. A row is assigned from touchpad module data and keymap tables rather
than validated machine by machine; per-model compatibility lives on the
[website](https://numdeck-dev.github.io/).

A model id is not proof that the machine carries the hardware. Some retail configurations of the
same model ship a 4-zone RGB touchpad with no NumberPad while keeping an identical DMI prefix
(`G614JV…-0EAGX…` has it, `G614JV…-0EACX…` does not; counts per generation in the table above). The
keymap therefore still follows the touchpad module rather than the model code; the per-configuration check
is in [docs/machines.md](docs/machines.md#同一型号不是都有-numpad按配置核验).

The Zephyrus M16 / G16 / Flow X16 line (幻16 in China; `GU603` through `GV601`) is absent by design,
not by omission: every generation lists a plain `Touchpad` in the official keyboard field, so there is
no icon area to drive. The dual-screen `GX650` (冰刃 6 / 7 双屏) does carry the hardware and is served
by `gx550`. Evidence per generation:
[docs/machines.md](docs/machines.md#幻16-系列无-numberpad已核验).

A model row holds the DMI name prefixes of the generation it serves, plus an `excludes` list for
generations that reuse the same code prefix without the hardware. An excluded prefix falls through to
the module check and, when that misses as well, to the same refusal. A manual `sku` pin outranks
both. Prefixes are reused across regional line-ups as well: `G614P*` ships in China as the 2025
Strix (魔霸 10 / 魔霸 9), and the per-configuration counts above do not cover those listings; the
touchpad module still decides the keymap.

The model code in DMI is authoritative; marketing names are for orientation only. The old per-model
package names (`numdeck-g614` and friends) stay in `Obsoletes`, so upgrades move to `numdeck`.

| | |
| --- | --- |
| ![SCAR 15 / Strix G15 2022](site/assets/models/g533.webp) | ![SCAR 16 / Strix G16 2023](site/assets/models/g614.webp) |
| SCAR 15 / Strix G15 (2022) | SCAR 16 / Strix G16 (2023) |
| ![SCAR 16 / Strix G16 2025](site/assets/models/g615.webp) | ![Zephyrus Duo 16, GX650 (2022)](site/assets/models/gx550.webp) |
| SCAR 16 / Strix G16 (2025) | Zephyrus Duo 16 (GX650, 2022), side-mounted touchpad |

Product images come from the ASUS product pages and are used to identify the model only.

### Dual-screen models: side-mounted touchpad

Status: `GVM` / `GVM_A` / `NVM` carry the official parameters (the entries the filter omits are `0`),
so these two rows are `family`: the grid is the usual 5 columns, but no one has confirmed key positions
on this hardware yet. Startup needs `--allow-unverified`. What follows is the behaviour the driver is
built for.

On the Zephyrus Duo and Zenbook Duo the secondary screen takes half of the palm rest, so the touchpad
is a vertical strip to the right of the keyboard instead of sitting under the palms. Number mode on
these machines is switched by a **dedicated key on the keyboard** (the ScreenPad key on `GX550` is one
of them), not by a long press on the icon area at the top-right corner of the touchpad. The 62-frame
icon-area long press therefore does not apply to a side-mounted pad:

- Contact mapping and backlight are unchanged: cell centres shift with the keymap of that module, and
  the backlight still uses FEATURE `0x0d`.
- The mode switch belongs to the keyboard key. The keyboard is its own HID device and the driver grabs
  the touchpad only, so it does not compete for that key.
- To pin number mode, use the `numlock` option (`sync` / `toggle` / `off`) rather than the long press.

### Model detection

Detection uses the union of the DMI fields `product_name`, `board_name` and `product_family`. BIOS
placeholder strings count as missing and do not block startup; a vendor field that is clearly not
ASUS blocks startup.

Touchpad module name (such as `ASUE1416`, read from i2c-hid and HID) is the second signal, used
when DMI names no model. When neither signal names a row, the machine is outside the support list and
the startup check refuses to start with exit code 3; `--sku` skips the check. Module and keymap per model:
[docs/machines.md](docs/machines.md).

When the hardware cannot be read reliably, pin the model:

```bash
numdeck sku            # what was detected, and on what evidence
numdeck sku list       # available model ids
numdeck sku g614       # pin a model, persisted to the config
numdeck sku auto       # back to auto-detection
```

`sku` in the config wins over detection; `--sku g614` applies to one invocation only.

### Layout validation

Keymaps are keyed by module name: `HID_LAYOUTS` in `src/layout.rs` maps 44 module names to keymap
names from the vendor INF. Each keymap carries a support level.

| Level | Behaviour |
| --- | --- |
| `verified` | starts as is |
| `family` | same 5x4 grid, official parameters copied in full (entries the INF omits default to `0`), not verified key by key on hardware: refused unless explicitly allowed |
| `unsupported` | parameters recorded but no key-position reference and not checked on hardware: refused; `check` output is requested |

A module the table does not know resolves to `UNKNOWN` and is refused the same way, so an unfamiliar
panel is not mapped onto a keymap that would emit the wrong keys. The check applies to the keymap
only, not to the model id.

```bash
numdeck --allow-unverified run    # or set allow_unverified_layout in the config
```

## Extending: adding a numpad to other models

Any model outside the table can get its own numpad as long as the touchpad reports key coordinates.
Pick by what the hardware in hand actually offers:

| What you have | What to configure | Backlight |
| --- | --- | --- |
| Laser-etched digits on the touchpad, or a keypad sticker | A keymap in `custom_layouts` | None, at best lit by the keyboard backlight spill |
| Plain glass touchpad, keys only where the sticker is | Same, with `keys` matching the sticker | Same |
| The touchpad does light up, just not with `0d 14 03 .. ad` | New report in `led_proto` | Yes |
| The touchpad reports a pressure axis | `pressure_switch` to toggle by force | Unaffected |
| You would rather toggle from the physical keyboard | Bind a combo in `hotkeys` | Unaffected |

The four are independent; configure only the ones the machine needs. All of them live in the same
`numdeck.json`, and the GUI's Extension page edits that same file.

### Custom layouts

```json
{
  "custom_layouts": [
    {
      "name": "GH3X-sticker",
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

- `name` is the keymap name: set `layout_name` to it to select it, or leave `auto` and let `hid_ids`
  pick it on the matching module.
- `base` is the parameter source: a built-in keymap name, or another custom layout listed earlier in
  the same file (copy a geometry, change two cells). Geometry fields (`columns`, `left`, `right`,
  `top`, `bottom`, `gap`, ...) are inherited when omitted; `columns` follows the width of `keys` and
  is rewritten automatically.
- `keys` is a 4-row keymap with 1..=8 columns. All four rows must have the same width, otherwise the
  layout is rejected and the reason is shown in the GUI. An empty list keeps the keymap of `base`
  (use this when only the geometry differs).
- `hid_ids` binds the **touchpad module HID hardware ID** (`ASUE1416` level), not the laptop model, so
  the machine matches without touching `layout_name`. Binding a module that already has an official
  mapping prints an "overrides official mapping" note.
- Custom layouts count as `verified`: you wrote them, so the `family` / `unsupported` gate no longer
  applies to them.

How to write a key (space separated, case insensitive):

| Want this key | Write |
| --- | --- |
| Digit | `7` |
| Symbols printed on the key | `+`, `*`, `/` (also `plus`, `asterisk`, `slash`) |
| Decimal point | `period` (`.` means empty cell, not a period) |
| Minus | `minus` (`-` means empty cell) |
| Enter, backspace | `enter`, `bs` |
| Empty cell | `none`, or `.` / `-` |
| Anything else | `kp7`, `F13`, `backspace`, `volup`, or a raw code such as `0x64` / `100` |

The Extension page shows those exact lines and reports "N columns x 4 rows, usable" or which row is
off right after an edit.

### Custom illumination report

```json
{ "led_proto": { "template": [13, 20, 3, 255, 173] } }
```

- `template` is the whole HID FEATURE report; the first byte is the report ID. Exactly one byte is
  written as `255` (`0xff`) to mark the **level slot**, which is replaced with the current level
  (`0x41`..`0x48`, `0` for off) whenever the backlight is set.
- The template must contain exactly one `255` and be 2..=64 bytes long; otherwise it is discarded,
  the built-in report comes back, and both the GUI and `--verbose` say so.
- The GUI edits it in hex (`0d 14 03 ff ad`), bytes separated by spaces.

### Force-press toggling

```json
{ "pressure_switch": { "enabled": true, "percent": 85, "frames": 6, "cooldown_ms": 600 } }
```

- When the touchpad reports a pressure axis, pressure above `percent`% of full scale for `frames`
  consecutive frames counts as one force press and toggles the numpad. One frame is `frame_ms`
  (8 ms by default), so 6 frames is about 50 ms.
- `cooldown_ms` is the minimum gap between two toggles, so holding the pad down cannot flip it
  repeatedly.
- It does not interact with `mixed_mode`: only the on/off state flips, no keys are synthesised and no
  pressure events are forwarded to the desktop.
- Touchpads without a pressure axis simply ignore it; leaving it configured is harmless.

### Global hotkeys

```json
{ "hotkeys": [ { "gesture": "ctrl+alt+n", "action": "toggle" } ] }
```

- `gesture` joins key names with `+`: `ctrl`, `alt`, `shift`, `super` plus one regular key.
- `action` is `toggle`, `on` or `off`.
- The keyboard is only observed, never grabbed, so normal typing is unaffected. Combos that fail to
  parse are reported in the GUI and skipped.

### Custom partition and manual calibration

The model table carries 15 geometry fields, all of which assume the stock 5x4 grid. Two cases need
more than that: a silkscreen laid out differently (uneven column widths, 6 or 7 columns), and a
kernel that reports coordinates which don't actually span the panel. The Calibrate tab handles both.

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

- `rect` is `[x0, y0, x1, y1]` in raw touchpad coordinates, the same numbers `evtest` prints. It must
  be non-empty and not inverted; a rejected rect says so and the built-in grid stays in charge.
- `cols` is 1..=8, `rows` is 1..=4. Leave either out and it follows the keymap.
- `col_ratios` / `row_ratios` make the grid uneven (the rightmost column wider, say). They are
  weights, normalised to the rect, and must match the column / row count. This one is config-only;
  the GUI doesn't draw a row for it.
- `snap_outside` defaults to off. On, a touch outside the grid snaps to the nearest cell instead of
  landing in the cursor area.
- `calibration` overrides what the kernel reports for the axes: `min` / `max` / `units_per_mm`
  (30 units per mm means gestures measured in millimetres convert correctly). It is a linear remap,
  nothing more - no axis swap, no inversion. Leave a section out to keep the kernel value.
- `partition` applies to the layout named in `partition.layout` (`auto` follows the active one), so
  one file can carry partitions for several layouts.
- Two more keys have no control of their own: `rect_mm` (`[x0, y0, x1, y1]` in millimetres from the
  panel corner, for when raw units differ from what the panel measures) and `numlock_cell`
  (`[row, col]` for the NumLock icon; the default is the last column of row 0).

Measuring beats guessing: `numdeck calibrate --samples samples.json` takes four corner presses
plus the centres of two diagonal cells and solves the rect and cell size. `--write` stores the result
in the config, keeping a `.bak` of the previous file. The Calibrate tab replays the same sample file
through the same solver, so a sample taken on the machine can be solved from the GUI without
re-running the CLI. The tab also packs `partition` + `calibration` into a snippet for copying to
another machine of the same model - the keymap is not included, that lives in `custom_layouts`.

Values that can be judged wrong without touching the panel - a column or row count outside 1..=8, a
rect with the right and left edges swapped (or no area at all), a ratio list whose length does not
match the column count or that contains zero/negative weights, a `numlock_cell` outside the grid - are
rejected as the config is loaded and named in the notes, so `numdeck check` reports them instead of
silently drawing a wrong grid. Only the offending field is dropped; everything else still applies.

![Calibrate tab: partition, manual calibration, sample replay, snippet](site/assets/screenshots/gui-calib.webp)

### Handing a config to someone with the same model

The bottom card on the Extend page is the config snippet: hit Export to pack the four extension
settings into one JSON blob, and whoever receives it pastes it into the same box and hits Apply. The
snippet carries only `custom_layouts` / `led_proto` / `pressure_switch` / `hotkeys`, none of the
per-machine bits (frame cadence, geometry fallback, UI language), which need redoing when the machine
changes anyway. Import is all-or-nothing: if any of the four fails to parse, nothing is written, so you
never end up with a half-applied config. A template that isn't a valid report is rejected by name.
Besides the box, Copy puts the snippet on the system clipboard to forward as-is, and Save to file
writes a `.json` into the download folder without overwriting an earlier export.

### Checking an extension config

```sh
tools/verify-extend.sh
```

22 read-only checks: the binary, the extension unit tests by name, whether
`custom_layouts` / `led_proto` / `pressure_switch` / `hotkeys` actually reach `layout`
and `check`, `--lang` switching, and that bad values are rejected with a note instead
of silently applied. It works on a `mktemp` config - nothing under `~/.config` or any
hidraw device is touched. Items that need fingers on the touchpad (hard-press toggle,
hotkey, custom report) are printed as commands to paste, not executed.

![Extension page: custom layouts, LED report, pressure toggle, global hotkeys](site/assets/screenshots/gui-extend.webp)

Every field, block title and button carries a hover explanation, so the page is usable
without the README open:

![Hover explanation on the extension page (light)](site/assets/screenshots/gui-extend-hover-light.webp)
![Hover explanation on the extension page (dark)](site/assets/screenshots/gui-extend-hover-dark.webp)

### How this relates to model detection and layout validation

The two gates are separate: the model (DMI / `sku`) decides whether the driver may run at all, the
keymap (`HID_LAYOUTS`) decides which coordinate produces which key. Custom layouts only solve the
second gate - use them when the module is unknown, or known but arranged differently. An unrecognised
machine still needs `--sku` or the `sku` setting to pass the first gate.

## FAQ

### Why is support limited to ASUS models?

The driver does not depend on ASUS-specific interfaces: input comes from the touchpad's evdev node,
keys go out through uinput, and no vendor kernel module or ACPI object is involved, so any multitouch
pad follows the same code path. A port means replacing the model entry (DMI `product_name` plus the
touchpad HID name) and the layout constants (cell centres, thresholds).

What cannot be ported is the hardware. The NumberPad panel provides two things other OEM touchpads
generally do not:

- **Addressable backlight.** Cell position and number-mode state are indicated by an LED pattern
  driven through HID FEATURE report `0x0d` (`[0d 14 03 <v> ad]`, `0x41`..`0x48` select the brightness
  step). Without it, keys are still emitted, but there is no indication of which cell the finger is on or
  whether number mode is active.
- **A factory calibration table.** `AlgoParameters` in `AsusNUMPADFilter.inf` fixes frame length
  (8 ms), geometry unit (0.1 mm), tap window (350 ms), NumLock long press (62 frames), swipe
  threshold (4838 um) and two-finger gap (40 mm). Without it those values have to be fitted on a
  panel not tuned for numpad use, and tap and swipe recognition will not match the
  Windows driver.

Support therefore stops at panels that provide both. The model table in `src/sku.rs` is the support
list and gates startup; keymaps and their support level live in `HID_LAYOUTS` (`src/layout.rs`) and are
picked separately by the touchpad module, so an uncovered module stops at the
[layout validation](#layout-validation). Adding a panel that has both takes one model row and one set of layout
constants.

### Why a userspace driver instead of a kernel module?

- The panel is attached through ELAN i2c-hid (`04F3:3275` / `ASUE1416`), not through the ASUS `0B05`
  keyboard controller, so no existing kernel module covers it and none has to be replaced to add
  support.
- Backlight control is a HID FEATURE report on the touchpad's own hidraw node, which a userspace
  process can issue without kernel changes.
- Timing, geometry and threshold values are taken from the vendor INF verbatim, so state machine
  behaviour matches the Windows driver, and model differences stay in one parameter table.
- The boot path is untouched. Uninstalling removes the service unit and the udev rule and the pad
  returns to stock behaviour; kernel upgrades cannot regress it.

### Will this be merged into asusctl?

Not planned. The touchpad ships on a handful of models, the driver has nothing left to configure once
it runs, and folding it into asusctl would mean following that project's architecture and release
cadence for a feature most of its users do not have. Patches to this project, or an implementation of
these features in asusctl, are both welcome.

## How it works

### Data path

```
touchpad --evdev(/dev/input/eventN)--> frame buffer (8 ms) --> state machine --uinput(/dev/uinput)--> key events
                                                                   |
                                                                   `--> hidraw FEATURE report --> numpad backlight
```

### Timing and geometry

| Parameter | Value | Meaning |
| --- | --- | --- |
| Frame length | 8 ms | Time base of the state machine; a long press is 62 frames (496 ms) |
| Length units | µm / 0.1 mm | Gesture and swipe thresholds are micrometres, layout offsets are 0.1 mm steps |
| Tap interval | 350 ms | `NUMPADGESTURE_03/04`: minimum gap before a second tap counts as a separate one |
| NumLock long press | 62 frames | `NUMPAD_GH2XLNDZ_01` / `_GH3XLNDZ_01`; toggles number mode from the icon area (centre-mounted pads; see [dual-screen models](#dual-screen-models-side-mounted-touchpad)) |
| NumLock drift | 2741 µm | `NUMPAD_09`: movement still counted as a long press |
| Swipe threshold | 4838 µm | `NUMPADGESTURE_05`: movement above this cancels the key and becomes a gesture |
| Scroll finger distance | 40 mm | `NUMPADGESTURE_08`: largest finger gap that still scrolls; a wider gap goes to the desktop |
| App gesture | 7000 µm | `NUMPAD_07`: distance that triggers the multi-finger gesture |

### Key layout

Keymaps are selected by touchpad module and HID name, not by laptop model. `HID_LAYOUTS` holds 44
HID-name-to-keymap entries and covers every module in the vendor `SupportList`; module prefixes
such as `ASUE1416` and `ASCF1400` appear in `/sys/class/input/*/name`. Each entry carries a support
level, see the [layout validation](#layout-validation).

The numpad sits in the top-right corner of the touchpad, 5 rows by 4 columns. Cell centres are
the coordinates of `NUMPADLAYOUT_GH3XLNDZ_00..14` in 0.1 mm units. Coordinate to cell to keycode
conversion has no hardware dependency and is unit tested.

### Backlight

```
report: 0d 14 03 <level> ad   # 6 bytes, HID FEATURE report ID 0x0d
level:  0x41..0x48            # steps 1..8
        0x00                  # off
        0x01                  # on, current step kept
write:  HIDIOCSFEATURE        # no 0x60 unlock required
```

`bright follow on` reads `current` and `max` from `/sys/class/leds/asus::kbd_backlight/brightness`,
maps the ratio onto steps 1..8, polls every 500 ms and writes only on a step change. Manual steps
and follow mode share one step table.

Node selection (`led_policy.rs`): the OMNI receiver `0b05:1ace` is always excluded, ELAN `04f3:*`
with FEATURE `0x0d` takes priority, then ASUS `0b05:*`, then i2c (`0x38` / `0x15`).

### GUI

Two windows, one crate. Windows gets a lighting panel; Linux gets the full control center.

![ASUS NumberPad lighting window on Windows](site/assets/screenshots/gui-win.webp)

`gui/` writes the same FEATURE `0x0d` report the CLI does, so the traffic on both platforms comes
from the shared `led_proto` layer and stays byte-identical.

On Windows the window is only about light: interface list, brightness steps, NumLock follow, write
log. Numpad input stays with the vendor driver — the hidraw backend gives way to `setupapi` and
`hid.dll`.

On Linux it is the whole thing, split into seven tabs:

| Tab | Contents |
| --- | --- |
| Lighting | interface list, keyboard 4 steps → touchpad brightness map (editable per step), on / off, NumLock follow, keyboard-brightness follow, report log |
| Driver config | every field of the driver config (`~/.config/numdeck/numdeck.json`, root: `/etc/numdeck/numdeck.json`) as a control: model, layout, keypad style, NumLock policy, gesture thresholds, backlight channel and follow poll, manual step table, timing constants, driver language |
| System | config path, detected model, layout, hidraw channel, unit state, start / stop / restart |
| Extension | custom layouts, LED report template, force-press toggle, global hotkeys, and a config snippet to hand to someone else |
| Calibrate | custom partition (rect, columns, ratios) and manual axis calibration for models the table doesn't cover, plus sample replay and a snippet |
| Command line | one form per `numdeck` subcommand, spawning the real binary |
| Settings | step count, NumLock follow, poll interval, enable on login |

![Linux control center](site/assets/screenshots/gui-main.webp)

Edits live in memory until you press save, or "save and restart", which writes the file and restarts
the `numdeck` user unit in one go. Nothing duplicates the CLI: the command tab spawns the same
executable and pastes its output back.

```bash
cargo run -p numdeck-gui      # or: cargo build --release -p numdeck-gui
```

See `gui/README.md` for the runtime requirements, settings path and troubleshooting.

### Permissions

No root. udev tags hidraw and event nodes with `TAG+="uaccess"`, the daemon runs as a
`systemd --user` unit, `/dev/uinput` is granted by the session.

## Source layout

| Path | Purpose |
| --- | --- |
| `src/lib.rs` | Library root: modules shared by the binary and the GUI |
| `src/main.rs` | CLI parsing and wiring |
| `src/engine.rs` | Vendor algorithm state machine |
| `src/layout.rs` | Layout geometry, 44 HID entries, coordinate conversion |
| `src/config.rs` | Defaults, load and save, migration of old paths |
| `src/brightness.rs` | Keyboard backlight to numpad step mapping |
| `src/led_policy.rs` | Backlight node scoring |
| `src/sku.rs` | Model table, runtime detection, manual pin, per-generation excludes |
| `src/i18n.rs` | Three languages, overridable with external `.lang` files |
| `src/keycodes.rs` | Linux input event codes, kept local so `layout` / `engine` compile on any platform |
| `src/led_proto.rs` | Backlight FEATURE `0x0d` report encoding, the single source for CLI and GUI |
| `src/linux/` | evdev, uinput, hidraw and i2c backends |
| `gui/` | egui window: lighting only on Windows, full control center on Linux |
| `lang/` | zh-CN, zh-TW, en strings |
| `site/` | Website sources, published to https://numdeck-dev.github.io/ |
| `tools/` | `push-site.sh` (site publish), `repo-gen.sh` (repository indexes) |
| `packaging/` | deb, rpm, Arch, Alpine, Gentoo, Nix, tarball scripts and repo tools |

## Building

### Cross-building the other targets

```bash
./tools/build-cross.sh            # all four
./tools/build-cross.sh windows    # x86_64-pc-windows-gnu + aarch64-pc-windows-gnullvm
./tools/build-cross.sh linux      # x86_64-unknown-linux-gnu + aarch64-unknown-linux-gnu
```

The script fetches zig (which ships the MinGW and glibc pieces `windows-gnu` / `linux-gnu`
need, so no mingw-w64 install and no container) and installs `cargo-zigbuild` if missing.
The Linux binaries are pinned to glibc 2.17, so they run on anything from Ubuntu 18.04 up.
The ARM64 targets matter for machines like the RTX Spark and Snapdragon X laptops: the build
machine does not have to be ARM, zig cross-compiles from x86_64 or Apple silicon.
The GUI crate is not cross-built - it needs the platform's compositor stack.

```bash
./packaging/build.sh                        # current distribution
./packaging/build.sh deb rpm tarball        # explicit targets (deb/rpm/arch/alpine/tarball)
./packaging/build.sh --arch arm64 deb       # cross-build an ARM64 package (needs zig)
./tools/repo-gen.sh                         # dist/ to local repositories in repo/ (apt/rpm/pacman/apk)
```

```bash
RUSTFLAGS="-C linker=rust-lld -C link-self-contained=yes" \
  cargo build --target x86_64-unknown-linux-musl --release
```

Packaging details: [packaging/README.md](packaging/README.md). A `v*` tag runs
`.github/workflows/release.yml`, which builds in per-distribution containers, generates four
repository indexes, publishes the release and commits `repo/` back to `main`.

The website sources are in `site/`: `./tools/push-site.sh` copies them to the
`numdeck-dev/numdeck-dev.github.io` repository, which serves https://numdeck-dev.github.io/.
`docs/` holds project documentation only; `docs/index.html` is the redirect that keeps the old
project-pages URL working.

## License

GPL-3.0-or-later. See [LICENSE](LICENSE).

Copyright (C) 2026 the numdeck authors
