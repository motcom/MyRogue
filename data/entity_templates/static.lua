-- 地面・固定物も全て Entity として配置する。
-- 数値はゲーム用の仮設定。Rust 側のホスト API は未接続。
return {
    schema_version = 2,
    id = "terrain.field",
    category = "static",
    common = {
        name = "野原",
        description = "草の生えた野原。自由に歩くことができる地面。",
        appearance = {
            glyph = ".",
            fg = { 90, 150, 60 },
            bg = { 0, 0, 0 },
            layer = 0,
        },
        collision = {
            movement = false,
            sight = false,
        },
        durability = false,
        tags = { "terrain" },
    },
    initial_state = {},
    static = {
        movement_cost = 2.0,
        combat = { base_av = 0 },
        role = "terrain",
        facilities = {  },
        resources = {},
    },
    hooks = {
    },
}
