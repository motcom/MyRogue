-- AV・DV・重量は動作確認用の仮設定。
return {
    schema_version = 2,
    id = "item.leather_armor",
    category = "item",
    common = {
        name = "革の鎧",
        description = "胴体に着る革の防具。所持品のwで着脱する。",
        appearance = { glyph = "[", fg = {160, 110, 65}, bg = {0, 0, 0}, layer = 20 },
        collision = { movement = false, sight = false },
        durability = { max = 100 },
        tags = { "armor" },
    },
    initial_state = {},
    item = {
        length_mm = 800, volume_ml = 3000, mass_g = 2500,
        stack_limit = 1, materials = { "leather" }, tool_functions = {}, resources = {},
        armor = { slot = "body", av = 1, dv = 0 },
    },
    hooks = {},
}
