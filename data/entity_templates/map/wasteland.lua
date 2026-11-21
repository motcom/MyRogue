-- マップの基盤。スポーンせず、セルへ設定する。
-- 数値はゲーム用の仮設定。
return {
    schema_version = 2,
    id = "terrain.wasteland",
    category = "map",
    common = {
        name = "荒地",
        description = "固定マップ要素。移動コスト4。",
        appearance = {
            glyph = ",",
            fg = { 165, 175, 171 },
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
        spawns = {
            { definition_id = "obstacle.conifer", chance = 0.005 },
        },
        movement_cost = 4.0,
    },
    hooks = {
    },
}
