-- 参照先を完結させるための弾薬・素材。
-- 数値はゲーム用の仮設定。Rust 側のホスト API は未接続。
return {
    schema_version = 2,
    id = "ammo.rifle",
    category = "item",
    common = {
        name = "ライフル弾",
        description = "SKSの装填に使うライフル弾。",
        appearance = {
            glyph = "=",
            fg = { 160, 130, 80 },
            bg = { 0, 0, 0 },
            layer = 20,
        },
        collision = {
            movement = false,
            sight = false,
        },
        durability = false,
        tags = { "ammunition" },
    },
    initial_state = {},
    item = {
        length_mm = 60,
        volume_ml = 20,
        mass_g = 20,
        stack_limit = 50,
        materials = { "metal" },
        tool_functions = {  },
        resources = {},
    },
    hooks = {
    },
}
