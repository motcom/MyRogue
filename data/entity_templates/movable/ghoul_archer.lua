-- 武器と射程に応じて攻撃・追跡・待機するグール。
-- 数値はゲーム用の仮設定。Rust 側のホスト API は未接続。
return {
    schema_version = 2,
    id = "monster.ghoul_archer",
    category = "movable",
    common = {
        name = "グール アーチャー",
        description = "クロスボウを持つグール。ボルトを装填し、射程内の生存者を狙う。",
        appearance = {
            glyph = "a",
            fg = { 180, 90, 90 },
            bg = { 0, 0, 0 },
            layer = 30,
        },
        collision = {
            movement = true,
            sight = false,
        },
        durability = false,
        tags = { "ghoul" },
    },
    initial_state = {},
    movable = {
        tier = 2,
        experience_reward = 45,
        dominant_hand = "right",
        sanity = 100,
        psykick = {},
        status = {},
        combat = { base_av = 1, dv_bonus = 0, accuracy_bonus = 0 },
        role = "monster",
        controller = "lua",
        faction = "ghoul",
        hp = 40,
        body = { "head", "body", "left_arm", "right_arm", "left_leg", "right_leg" },
        attributes = {
            kinryoku = 7,
            subayasa = 8,
            kiyousa = 8,
            seishinryoku = 4,
        },
        skills = {
            melee_weapon = 0,
            bow = 2,
        },
        inventory = {
            max_length_mm = 1500,
            max_volume_ml = 20000,
            max_mass_g = 30000,
        },
        senses = {
            sight_range = 12,
        },
        attack = {
            damage = { dice = 1, sides = 3, bonus = 0 },
            base_pv = 3, strength_cap = 2,
            range = 1,
        },
        loadout = { {
                definition_id = "item.crossbow",
                quantity = 1,
                equipped = true,
            }, {
                definition_id = "ammo.bolt",
                quantity = 12,
            } },
        needs = {
            hydration = {
                max = 100,
                initial = 80,
            },
        },
    },
    hooks = {
        on_turn = function(self, ctx, event)
            local target = ctx:nearest_hostile(self.id, self.config.movable.senses.sight_range)
            if not target then return { action = { type = "wait" } } end
            if ctx:can_reload(self.id) then return { action = { type = "reload" } } end
            -- 射撃可能なら射撃、隣接していれば近接攻撃。弾薬・距離は Rust が判定する。
            if ctx:can_attack(self.id, target.id) then
                return { action = { type = "attack", target = target.id } }
            end
            local position = ctx:step_towards(self.id, target.id)
            if position then return { action = { type = "move", position = position } } end
            return { action = { type = "wait" } }
        end,
    },
}
