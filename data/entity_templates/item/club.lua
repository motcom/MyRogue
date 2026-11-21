-- 射程・威力・寸法は現実の製品仕様ではなくゲーム用の仮設定。
-- 数値はゲーム用の仮設定。Rust 側のホスト API は未接続。
return {
    schema_version = 2,
    id = "item.club",
    category = "item",
    common = {
        name = "棍棒",
        description = "木製の棍棒。近くの相手を殴るための近接武器。",
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
        tier = 1,
        length_mm = 650,
        volume_ml = 1200,
        mass_g = 1200,
        stack_limit = 1,
        materials = { "wood" },
        tool_functions = { "hammering" },
        resources = {},
        weapon = {
            skill = "melee_weapon",
            hands = "either",
            two_hand_pv_bonus = 1,
            critical_value = 17,
            base_pv = 4, strength_cap = 3, accuracy_bonus = 0,
            mode = "melee",
            damage = { dice = 1, sides = 2, bonus = 4 },
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
