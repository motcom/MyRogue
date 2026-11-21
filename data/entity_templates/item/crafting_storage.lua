-- 宝箱自体も拾えるItem。中身は個体ごとに保持し、選んだ品だけ取り出す。
return {
    schema_version = 2,
    id = "item.crafting_storage",
    category = "item",
    common = {
        name = "制作ストレージ",
        description = "制作試作の材料と工具が入った収納。近くに立てば中身を直接制作に使える。",
        appearance = { glyph = "C", fg = {210, 160, 60}, bg = {0, 0, 0}, layer = 21 },
        collision = { movement = false, sight = false },
        durability = { max = 100 },
        tags = { "container" },
    },
    initial_state = { opened = false },
    item = {
        tier = 2,
        length_mm = 600, volume_ml = 5000, mass_g = 3000, stack_limit = 1,
        contents = {
            { definition_id = "item.wood", quantity = 80 },
            { definition_id = "item.metal_stock", quantity = 80 },
            { definition_id = "item.cloth", quantity = 40 },
            { definition_id = "item.leather_piece", quantity = 40 },
            { definition_id = "item.thread", quantity = 60 },
            { definition_id = "item.charcoal", quantity = 40 },
            { definition_id = "item.wood_handle", quantity = 20 },
            { definition_id = "item.raw_meat", quantity = 10 },
            { definition_id = "item.dirty_water", quantity = 10 },
            { definition_id = "item.hand_drill", quantity = 1 },
            { definition_id = "item.metal_saw", quantity = 1 },
            { definition_id = "item.metal_file", quantity = 1 },
            { definition_id = "item.forging_hammer", quantity = 1 },
            { definition_id = "item.sewing_kit", quantity = 1 },
            { definition_id = "item.cooking_kit", quantity = 1 },
            { definition_id = "item.wood_saw", quantity = 1 },
        },
        materials = { "wood", "metal" }, tool_functions = {}, resources = {},
    },
    interactions = {
        { id = "open", name = "開く", when_state = { opened = false } },
        { id = "contents", name = "中身を見る", when_state = { opened = true }, consumes_turn = false },
        { id = "close", name = "閉じる", when_state = { opened = true } },
    },
    hooks = {
        on_interact = function(self, ctx, event)
            if event.interaction == "open" then
                local effects = {
                    { type = "set_glyph", target = self.id, glyph = "c" },
                    { type = "message", message = "宝箱を開けました" },
                }
                table.insert(effects, { type = "open_container", target = self.id })
                return { state = { opened = true }, effects = effects }
            elseif event.interaction == "contents" then
                return { effects = { { type = "open_container", target = self.id } } }
            elseif event.interaction == "close" then
                return { state = { opened = false }, effects = {
                    { type = "set_glyph", target = self.id, glyph = "C" },
                    { type = "message", message = "宝箱を閉じました" },
                } }
            end
            return {}
        end,
    },
}
