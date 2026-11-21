-- 池は初期仕様で通行不可。
-- スポーン候補・確率と経路コストはLua、配置と整合性の検査はRust。
return {
    schema_version = 2,
    id = "terrain.pond",
    category = "map",
    common = {
        name = "池",
        description = "水のたまった池。現在は泳げないため通行できない。",
        appearance = {
            glyph = "~",
            fg = { 168, 204, 202 },
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
    map = {
        spawns = {},
        movement_cost = 1.0,
    },
    hooks = {
    },
}
