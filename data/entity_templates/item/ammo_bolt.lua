-- 参照先を完結させるための弾薬・素材。
-- 数値はゲーム用の仮設定。Rust 側のホスト API は未接続。
return {
    schema_version = 2,
    id = "ammo.bolt",
    category = "item",
    common = {
        name = "クロスボウボルト",
        description = "クロスボウに装填して発射するボルト。",
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
        length_mm = 400,
        volume_ml = 50,
        mass_g = 30,
        stack_limit = 20,
        materials = { "wood", "metal" },
        tool_functions = {  },
        resources = {},
    },
    hooks = {
    },
}
