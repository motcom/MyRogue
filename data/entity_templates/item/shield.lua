-- AV・DV・重量は動作確認用の仮設定。
return {
    schema_version = 2,
    id = "item.shield",
    category = "item",
    common = {
        name = "盾",
        description = "片手で構える盾。使用可能な手で装備している間AVを加算する。",
        appearance = { glyph = "[", fg = {160, 110, 65}, bg = {0, 0, 0}, layer = 20 },
        collision = { movement = false, sight = false },
        durability = { max = 100 },
        tags = { "shield" },
    },
    initial_state = {},
    item = {
        tier = 2,
        length_mm = 800, volume_ml = 3000, mass_g = 2500,
        stack_limit = 1, materials = { "leather" }, tool_functions = {}, resources = {},
        armor = { slot = "body", av = 2, dv = 0 },
    },
    hooks = {},
}
