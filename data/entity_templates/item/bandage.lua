-- 効果がある場合だけ一個を消費する。
-- 一部位に一個。出血ダメージを即時に止め、100世界ターン後に処置を完了する。
return {
    schema_version = 2,
    id = "item.bandage",
    category = "item",
    common = {
        name = "包帯",
        description = "巻いた部位の出血ダメージをその場で止める。100ターン後に処置が完了し、包帯は外れる。",
        appearance = {
            glyph = "!",
            fg = { 220, 200, 140 },
            bg = { 0, 0, 0 },
            layer = 20,
        },
        collision = {
            movement = false,
            sight = false,
        },
        durability = false,
        tags = { "consumable" },
    },
    initial_state = {},
    item = {
        tier = 1,
        length_mm = 150,
        volume_ml = 100,
        mass_g = 50,
        stack_limit = 10,
        materials = { "cloth" },
        tool_functions = {  },
        resources = {},
        use = { kind = "bandage", amount = 100 },
    },
    hooks = {
        on_use = function(self, ctx, event)
            local target_id = event.target or event.user
            local target = ctx:entity(target_id)
            local part = event.body_part
            if target_id ~= event.user or not target or not part then return {} end
            if not target.bleeding or (target.bleeding[part] or 0) <= 0 then return {} end
            if target.bandages and target.bandages[part] then return {} end
            return { effects = {
                { type = "apply_bandage", target = target_id, body_part = part },
                { type = "consume_item", target = self.id, quantity = 1 },
            } }
        end,
    },
}
