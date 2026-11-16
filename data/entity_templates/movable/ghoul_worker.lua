-- 武器と射程に応じて攻撃・追跡・待機するグール。
-- 数値はゲーム用の仮設定。Rust 側のホスト API は未接続。
return {
    schema_version = 2,
    id = "monster.ghoul_worker",
    category = "movable",
    common = {
        name = "グール ワーカー",
        description = "近くの生存者を追いかけ、近接攻撃を行うグール。",
        appearance = {
            glyph = "g",
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
        experience_reward = 25,
        dominant_hand = "right",
        sanity = 100,
        psykick = {},
        status = {},
        combat = { base_av = 0, dv_bonus = 0, accuracy_bonus = 0 },
        role = "monster",
        controller = "lua",
        faction = "ghoul",
        hp = 35,
        body = { "head", "body", "left_arm", "right_arm", "left_leg", "right_leg" },
        attributes = {
            kinryoku = 6,
            tairyoku = 6,
            subayasa = 8,
            kiyousa = 8,
            tiryoku = 4,
            seishinryoku = 4,
        },
        skills = {
            melee_weapon = 0,
            bow = 0,
        },
        inventory = {
            max_length_mm = 1500,
            max_volume_ml = 20000,
            max_mass_g = 30000,
        },
        senses = {
            sight_range = 7,
        },
        attack = {
            damage = { dice = 1, sides = 3, bonus = 0 },
            base_pv = 3, strength_cap = 2,
            range = 1,
        },
        loadout = {  },
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
