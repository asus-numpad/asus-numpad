//! 官方参数一览（`check` 输出用）；键名与说明随界面语言变化。

use crate::keycodes as kc;

use super::*;

/// 官方参数一览（`check` 输出用）；键名与说明随界面语言变化。
pub fn official_parameters() -> Vec<(String, String)> {
    let frames = |n: u32, ms_key: &str| {
        crate::i18n::tf(
            ms_key,
            &[
                ("frames", &n.to_string()),
                ("ms", &(n * FRAME_MS).to_string()),
            ],
        )
    };
    vec![
        (
            crate::i18n::t("param.frame_model"),
            format!("1 frame {FRAME_MS}ms = {FRAMES_PER_SEC} Hz"),
        ),
        (
            "NUMPAD_00 ContSendKey_Trig_FC".to_string(),
            frames(NUM_KEY_CONT_TRIGGER_FRAMES, "param.cont_sendkey"),
        ),
        (
            "NUMPAD_01 NumLock_Trigger_FC".to_string(),
            frames(NUMLOCK_TRIGGER_FRAMES, "param.numlock"),
        ),
        (
            "NUMPAD_GH3XLNDZ_01".to_string(),
            frames(NUMLOCK_TRIGGER_FRAMES_GH3XLNDZ, "param.numlock_gh3"),
        ),
        (
            "NUMPAD_02 Max_Numpad_Work_Time_T".to_string(),
            format!("{MAX_WORK_TIME_MS} ms"),
        ),
        (
            "NUMPAD_06 NumAdv_Trigger_FC".to_string(),
            frames(NUMADV_TRIGGER_FRAMES, "param.cont_sendkey"),
        ),
        (
            "NUMPAD_07 EnAppGesture_Trig_UM".to_string(),
            format!("{APP_GESTURE_TRIG_UM:.0} µm"),
        ),
        (
            "NUMPAD_09 NumLockMaxVaildDist".to_string(),
            format!("{NUMLOCK_MAX_VALID_DIST_UM:.0} µm"),
        ),
        (
            "GESTURE_01 TouchSendKey_Trig_FC".to_string(),
            frames(GESTURE_TOUCH_SENDKEY_FRAMES, "param.cont_sendkey"),
        ),
        (
            "GESTURE_05 Gesture_Trig_UM".to_string(),
            format!("{GESTURE_TRIG_UM:.0} µm"),
        ),
        (
            "GESTURE_06 iTAPPING_THRESHOLD_MM".to_string(),
            format!("{TAP_THRESHOLD_MM:.0} mm"),
        ),
        (
            "GESTURE_08 iscroll_finger_dist_MM".to_string(),
            format!("{SCROLL_FINGER_DIST_MM:.0} mm"),
        ),
        (
            crate::i18n::t("param.led_values_name"),
            crate::i18n::tf(
                "param.led_values",
                &[
                    ("off", &format!("{VAL_OFF:02x}")),
                    ("on", &format!("{VAL_ON:02x}")),
                ],
            ),
        ),
        (
            crate::i18n::t("param.i2c_unlock_name"),
            crate::i18n::tf(
                "param.i2c_unlock",
                &[("unlock", &format!("{VAL_UNLOCK:02x}"))],
            ),
        ),
        (
            crate::i18n::t("param.key_count"),
            format!("{}", kc::ALL_KEYS.len()),
        ),
    ]
}
