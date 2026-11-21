-- 既存の金属加工・作業台設計の具体例を保持する。
-- 数値はゲーム用の仮設定。
return {
    schema_version = 2,
    id = "obstacle.table",
    category = "static",
    common = {
        name = "テーブル",
        description = "固定マップ要素。通行と視線は個別に設定する。",
        appearance = {
            glyph = "T",
            fg = { 160, 120, 70 },
            bg = { 0, 0, 0 },
            layer = 10,
        },
        collision = {
            movement = true,
            sight = false,
        },
        durability = {
            max = 100,
        },
        tags = { "facility" },
    },
    initial_state = {},
    static = {
        placement = "map_gen",
        movement_cost = 1.0,
        combat = { base_av = 2 },
        role = "facility",
        facilities = { "work_surface", "clamping" },
        resources = {},
    },
    hooks = {
    },
}
