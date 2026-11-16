-- document/entity_document/player_entity.md に基づくプレイヤー。
-- 数値はゲーム用の仮設定。Rust 側のホスト API は未接続。
return {
    schema_version = 2,
    id = "player.survivor",
    category = "movable",
    common = {
        name = "生存者",
        description = "この世界を探索する生存者。あなたが操作するキャラクター。死亡すると初期位置付近にリスポーンする。",
        appearance = {
            glyph = "@",
            fg = { 240, 240, 240 },
            bg = { 0, 0, 0 },
            layer = 30,
        },
        collision = {
            movement = true,
            sight = false,
        },
        durability = false,
        tags = { "player" },
    },
    initial_state = {},
    movable = {
        dominant_hand = "right",
        sanity = 100,
        psykick = {},
        status = {},
        combat = { base_av = 1, dv_bonus = 0, accuracy_bonus = 0 },
        role = "player",
        controller = "input",
        faction = "survivor",
        hp = 60,
        body = { "head", "body", "left_arm", "right_arm", "left_leg", "right_leg" },
        attributes = {
            kinryoku = 8,
            tairyoku = 8,
            subayasa = 8,
            kiyousa = 8,
            tiryoku = 8,
            seishinryoku = 8,
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
            sight_range = 10,
        },
        attack = {
            damage = { dice = 1, sides = 3, bonus = 0 },
            base_pv = 3, strength_cap = 2,
            range = 1,
        },
        loadout = { {
                definition_id = "item.knife",
                quantity = 1,
                equipped = true,
            }, {
                definition_id = "item.bandage",
                quantity = 2,
            }, {
                definition_id = "item.bread",
                quantity = 2,
            }, {
                definition_id = "item.mineral_water",
                quantity = 2,
            } },
        needs = {
            hydration = {
                max = 100,
                initial = 80,
            },
        },
        controls = {
            [";"] = { type = "open_inspect" },
            f = { type = "shoot" },
            r = { type = "reload" },
            g = { type = "pickup" },
            e = { type = "interact" },
            o = { type = "open" },
            h = {
                type = "move",
                dx = -1,
                dy = 0,
                dz = 0,
            },
            j = {
                type = "move",
                dx = 0,
                dy = 1,
                dz = 0,
            },
            k = {
                type = "move",
                dx = 0,
                dy = -1,
                dz = 0,
            },
            l = {
                type = "move",
                dx = 1,
                dy = 0,
                dz = 0,
            },
            y = {
                type = "move",
                dx = -1,
                dy = -1,
                dz = 0,
            },
            u = {
                type = "move",
                dx = 1,
                dy = -1,
                dz = 0,
            },
            b = {
                type = "move",
                dx = -1,
                dy = 1,
                dz = 0,
            },
            n = {
                type = "move",
                dx = 1,
                dy = 1,
                dz = 0,
            },
            p = {
                type = "open_crafting",
            },
            i = {
                type = "open_inventory",
            },
            m = {
                type = "open_map",
            },
        },
    },
    hooks = {
    },
}
