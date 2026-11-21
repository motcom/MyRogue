-- マップの基盤。スポーンせず、セルへ設定する。
-- スポーン候補・確率と経路コストはLua、配置と整合性の検査はRust。
return {
    schema_version = 2,
    id = "terrain.road",
    category = "map",
    common = {
        name = "道",
        description = "人の通った道。自由に歩くことができる。",
        appearance = {
            glyph = ":",
            fg = { 208, 205, 187 },
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
    map = {
        spawns = {},
        movement_cost = 1.0,
    },
    hooks = {
    },
}
