-- 池は初期仕様で通行不可。
-- 数値はゲーム用の仮設定。Rust 側のホスト API は未接続。
return {
    schema_version = 2,
    id = "terrain.pond",
    category = "static",
    common = {
        name = "池",
        description = "水のたまった池。現在は泳げないため通行できない。",
        appearance = {
            glyph = "~",
            fg = { 50, 100, 210 },
            bg = { 0, 0, 0 },
            layer = 0,
        },
        collision = {
            movement = true,
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
