-- 注射器は回復薬入りの架空のゲームアイテムとして仮定。
-- 数値はゲーム用の仮設定。Rust 側のホスト API は未接続。
return {
    schema_version = 2,
    id = "item.syringe",
    category = "item",
    common = {
        name = "注射器",
        description = "回復薬の入った注射器。使用すると体力を回復する。",
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
        tier = 3,
        length_mm = 150,
        volume_ml = 30,
        mass_g = 30,
        stack_limit = 10,
        materials = {  },
        tool_functions = {  },
        resources = {},
        use = {
            kind = "heal",
            amount = 15,
        },
    },
    hooks = {
        on_use = function(self, ctx, event)
            local target_id = event.target or event.user
            local target = ctx:entity(target_id)
            if not target or target.category ~= "movable" then return {} end
            local use = self.config.item.use
            local effect
            if use.kind == "remove_status" then
                if not target.statuses or not target.statuses[use.status_id] then return {} end
                if target.statuses[use.status_id] == 0 then return {} end
                effect = { type = "remove_status", target = target_id, status_id = use.status_id }
            elseif use.kind == "heal" then
                local hp = target.hp
                if not hp or hp.current <= 0 or hp.current >= hp.max then return {} end
                effect = { type = "heal", target = target_id, amount = use.amount }
            else
                -- 飲食は使用者自身に限定する。
                if target_id ~= event.user then return {} end
                local need = target.needs and target.needs[use.need]
                if not need or need.current >= need.max then return {} end
                effect = { type = "restore_need", target = target_id, need = use.need, amount = use.amount }
            end
            return { effects = {
                effect,
                { type = "consume_item", target = self.id, quantity = 1 },
            } }
        end,
    },
}
