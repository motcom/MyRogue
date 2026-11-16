-- 地面・固定物も全て Entity として配置する。
-- 数値はゲーム用の仮設定。Rust 側のホスト API は未接続。
return {
    schema_version = 2,
    id = "terrain.road",
    category = "static",
    common = {
        name = "道",
        description = "人の通った道。自由に歩くことができる。",
        appearance = {
            glyph = ":",
            fg = { 150, 130, 100 },
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
        movement_cost = 1.0,
        combat = { base_av = 0 },
        role = "terrain",
        facilities = {  },
        resources = {},
    },
    hooks = {
    },
}
