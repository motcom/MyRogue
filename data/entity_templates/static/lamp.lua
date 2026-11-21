-- 地面・固定物も全て Entity として配置する。
-- 数値はゲーム用の仮設定。
return {
    schema_version = 2,
    id = "obstacle.lamp",
    category = "static",
    common = {
        name = "ランプ",
        description = "固定マップ要素。通行と視線は個別に設定する。",
        appearance = {
            glyph = "!",
            fg = { 90, 150, 60 },
            bg = { 0, 0, 0 },
            layer = 10,
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
        placement = "map_gen",
        movement_cost = 1.0,
        combat = { base_av = 0 },
        role = "terrain",
        facilities = {  },
        resources = {},
    },
    hooks = {
    },
}
