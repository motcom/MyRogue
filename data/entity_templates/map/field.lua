-- マップの基盤。スポーンせず、セルへ設定する。
-- スポーン候補・確率と経路コストはLua、配置と整合性の検査はRust。
return {
    schema_version = 2,
    id = "terrain.field",
    category = "map",
    common = {
        name = "野原",
        description = "草の生えた野原。自由に歩くことができる地面。",
        appearance = {
            glyph = ".",
            fg = { 153, 160, 107 },
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
            { definition_id = "obstacle.tree", chance = 0.025 },
            { definition_id = "obstacle.conifer", chance = 0.015 },
            -- 樹木の判定後に空いている野原で合計2%。
            -- 各chanceは目標の出現率を、それより上のゴブリンが出ない確率で割った値。
            { definition_id = "monster.goblin_scavenger", chance = 0.01 },
            { definition_id = "monster.goblin_spearman", chance = 0.004444444444 },
            { definition_id = "monster.goblin_scrap_guard", chance = 0.003043831169 },
            { definition_id = "monster.goblin_crossbowman", chance = 0.001628332994 },
            { definition_id = "monster.goblin_caller", chance = 0.000815494393 },
            {
                definition_id = "monster.goblin_chief",
                chance = 0.000204039992,
                with_item = "item.treasure_chest",
            },
        },
        movement_cost = 2.0,
    },
    hooks = {
    },
}
