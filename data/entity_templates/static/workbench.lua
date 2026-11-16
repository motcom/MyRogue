-- 既存の金属加工・作業台設計の具体例を保持する。
-- 数値はゲーム用の仮設定。Rust 側のホスト API は未接続。
return {
    schema_version = 2,
    id = "facility.workbench",
    category = "static",
    common = {
        name = "万力付き作業台",
        description = "万力の付いた作業台。製作レシピは現在未対応。",
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
        movement_cost = 1.0,
        combat = { base_av = 2 },
        role = "facility",
        facilities = { "work_surface", "clamping" },
        resources = {},
    },
    hooks = {
    },
}
