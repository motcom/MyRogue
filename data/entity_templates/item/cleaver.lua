-- 射程・威力・寸法は現実の製品仕様ではなくゲーム用の仮設定。
-- 数値はゲーム用の仮設定。
return {
    schema_version = 2,
    id = "item.cleaver",

    category = "item",
    common = {
        name = "大鉈",
        description = "ゲーム用の仮設定の近接武器。所持品から持ち方を選んで装備する。",
        appearance = {
            glyph = "/",
            fg = { 200, 200, 200 },
            bg = { 0, 0, 0 },
            layer = 20,
        },
        collision = {
            movement = false,
            sight = false,
        },
        durability = {
            max = 100,
        },
        tags = { "weapon" },
    },
    initial_state = {},
    item = {
        tier = 3,
        length_mm = 1100,
        volume_ml = 150,
        mass_g = 3000,
        stack_limit = 1,
        materials = { "metal" },
        tool_functions = { "cutting", "butchering" },
        resources = {},
        weapon = {
            skill = "melee_weapon",
            hands = "two",
            critical_value = 14,
            base_pv = 8, strength_cap = 2, accuracy_bonus = 0,
            mode = "melee",
            damage = { dice = 1, sides = 10, bonus = 0 },
            range = 1,
        },
    },
    hooks = {
        on_use = function(self, ctx, event)
            if not event.target or not ctx:can_attack(event.user, event.target, self.id) then return {} end
            return { effects = {
                { type = "weapon_attack", attacker = event.user, target = event.target, weapon = self.id },
            } }
        end,
    },
}
