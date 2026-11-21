-- 既存テンプレートの打撃工具を保持する。
-- 数値はゲーム用の仮設定。Rust 側のホスト API は未接続。
return {
    schema_version = 2,
    id = "item.hammer",
    category = "item",
    common = {
        name = "ハンマー",
        description = "打撃加工に使う工具。打撃Lv1。金工ハンマーはLv2。",
        appearance = {
            glyph = ")",
            fg = { 180, 180, 180 },
            bg = { 0, 0, 0 },
            layer = 20,
        },
        collision = {
            movement = false,
            sight = false,
        },
        durability = {
            max = 100,
        },
        tags = { "tool" },
    },
    initial_state = {},
    item = {
        tier = 1,
        length_mm = 300,
        volume_ml = 500,
        mass_g = 1000,
        stack_limit = 1,
        materials = { "wood", "metal" },
        tool_functions = { "hammering" },
        resources = {},
    },
    hooks = {
    },
}
