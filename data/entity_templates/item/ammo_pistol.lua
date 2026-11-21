-- 参照先を完結させるための弾薬・素材。
-- 数値はゲーム用の仮設定。Rust 側のホスト API は未接続。
return {
    schema_version = 2,
    id = "ammo.pistol",
    category = "item",
    common = {
        name = "拳銃弾",
        description = "グロックの装填に使う拳銃弾。対応する武器を装備してrで装填する。",
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
        tier = 2,
        length_mm = 30,
        volume_ml = 10,
        mass_g = 10,
        stack_limit = 50,
        materials = { "metal" },
        tool_functions = {  },
        resources = {},
    },
    hooks = {
    },
}
