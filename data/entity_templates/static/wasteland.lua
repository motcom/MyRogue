-- 地面・固定物も全て Entity として配置する。
-- 数値はゲーム用の仮設定。
return {
    schema_version = 2,
    id = "terrain.wasteland",
    category = "static",
    common = {
        name = "荒地",
        description = "固定マップ要素。移動コスト4。",
        appearance = {
            glyph = ",",
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
        movement_cost = 4.0,
        combat = { base_av = 0 },
        role = "terrain",
        facilities = {  },
        resources = {},
    },
    hooks = {
    },
}
